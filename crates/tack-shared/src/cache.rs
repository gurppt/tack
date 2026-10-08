//! Optional disposable CAS. Every filesystem operation runs on a shared worker.
use crate::{ContentHash, Error, Result, SourceBinding, WireId, domain_error, hash_reader};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use tack_core::{Command, Document, LinkedPath, Source, SourceId, SourceLocation};
use tack_storage::TackFile;

pub const CACHE_BYTES: u64 = 512 * 1024 * 1024;
const MAX_FILES: usize = 4096;
const SENTINEL: &str = ".tack-shared-cache";
const OWNER: &[u8] = b"tack-shared-cas-v1\n";

fn require_regular(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(Error::Invalid("shared cache entry is not a regular file"));
    }
    Ok(())
}
pub(crate) fn owned_directory(root: &Path) -> Result<()> {
    if !root.is_absolute() {
        return Err(Error::Invalid("shared cache directory must be absolute"));
    }
    for ancestor in root.ancestors() {
        if fs::symlink_metadata(ancestor).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(Error::Invalid("shared cache symlink ancestry"));
        }
    }
    if !root.exists() {
        tack_storage::create_private_directory(root, true)?;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(root.join(SENTINEL))?;
        file.write_all(OWNER)?;
        file.sync_all()?;
    }
    require_regular(&root.join(SENTINEL))?;
    if fs::read(root.join(SENTINEL))? != OWNER {
        return Err(Error::Invalid("shared cache ownership sentinel"));
    }
    Ok(())
}

pub struct OriginalCache {
    root: PathBuf,
    entries: BTreeMap<ContentHash, (u64, u64)>,
    bytes: u64,
    clock: u64,
    evicted: Vec<ContentHash>,
    verified: BTreeMap<ContentHash, (u64, std::time::SystemTime)>,
}
impl OriginalCache {
    pub fn open(root: PathBuf) -> Result<Self> {
        owned_directory(&root)?;
        let mut result = Self {
            root,
            entries: BTreeMap::new(),
            bytes: 0,
            clock: 0,
            evicted: Vec::new(),
            verified: BTreeMap::new(),
        };
        let mut snapshots = 0;
        let mut snapshot_locks = 0;
        for (index, entry) in fs::read_dir(&result.root)?.enumerate() {
            if index >= MAX_FILES + 32 {
                return Err(Error::Invalid("shared cache entry count"));
            }
            let entry = entry?;
            let name = entry.file_name();
            let name = name
                .to_str()
                .ok_or(Error::Invalid("shared cache entry name"))?;
            require_regular(&entry.path())?;
            if let Ok(hash) = ContentHash::parse(name) {
                let bytes = entry.metadata()?.len();
                result.bytes = result
                    .bytes
                    .checked_add(bytes)
                    .ok_or(Error::Invalid("shared cache size overflow"))?;
                result.entries.insert(hash, (bytes, index as u64));
            } else if partial_name(name) {
                fs::remove_file(entry.path())?; // Private aborted staging is never authority.
            } else if snapshot_name(name) {
                snapshots += 1;
                if entry.metadata()?.len() > tack_storage::MAX_METADATA_BYTES as u64 + 80 {
                    return Err(Error::Invalid("shared metadata snapshot size"));
                }
            } else if snapshot_lock_name(name) {
                snapshot_locks += 1;
                if entry.metadata()?.len() != 0 {
                    return Err(Error::Invalid("shared metadata snapshot lock size"));
                }
            } else if name != SENTINEL {
                return Err(Error::Invalid("unowned file in shared cache"));
            }
        }
        if snapshots > 2 || snapshot_locks > 2 {
            return Err(Error::Invalid("shared metadata snapshot count"));
        }
        result.clock = MAX_FILES as u64;
        result.evict_for(0)?;
        Ok(result)
    }
    pub fn path(&self, hash: &ContentHash) -> PathBuf {
        self.root.join(hash.as_str())
    }
    pub fn bytes(&self) -> u64 {
        self.bytes
    }
    pub fn take_evicted(&mut self) -> Vec<ContentHash> {
        std::mem::take(&mut self.evicted)
    }
    /// Hash validation is off-thread; a filename alone never establishes authority.
    pub fn available(
        &mut self,
        hash: &ContentHash,
        size: u64,
        cancel: &AtomicBool,
    ) -> Result<Option<PathBuf>> {
        let path = self.path(hash);
        if !path.exists() {
            if let Some((old, _)) = self.entries.remove(hash) {
                self.bytes = self.bytes.saturating_sub(old);
                self.evicted.push(hash.clone());
            }
            self.verified.remove(hash);
            return Ok(None);
        }
        require_regular(&path)?;
        let mut file = File::open(&path)?;
        let metadata = file.metadata()?;
        let stamp = (metadata.len(), metadata.modified()?);
        if metadata.len() == size && self.verified.get(hash) == Some(&stamp) {
            self.clock = self.clock.saturating_add(1);
            if let Some(entry) = self.entries.get_mut(hash) {
                entry.1 = self.clock;
            }
            return Ok(Some(path));
        }
        let (observed, length) = hash_reader(&mut CancelReader {
            inner: &mut file,
            cancel,
        })?;
        if observed != *hash || length != size {
            fs::remove_file(&path)?;
            if let Some((old, _)) = self.entries.remove(hash) {
                self.bytes = self.bytes.saturating_sub(old);
            }
            self.evicted.push(hash.clone());
            self.verified.remove(hash);
            return Ok(None);
        }
        if let Some((old, _)) = self.entries.remove(hash) {
            self.bytes = self.bytes.saturating_sub(old);
        }
        self.evict_for(length)?;
        self.bytes = self
            .bytes
            .checked_add(length)
            .ok_or(Error::Invalid("shared cache size overflow"))?;
        self.clock = self.clock.saturating_add(1);
        self.entries.insert(hash.clone(), (length, self.clock));
        self.verified.insert(hash.clone(), stamp);
        Ok(Some(path))
    }
    pub fn begin(&mut self, size: u64) -> Result<(PathBuf, File)> {
        if size == 0 || size > CACHE_BYTES {
            return Err(Error::Invalid(
                "original exceeds 512 MiB client cache admission",
            ));
        }
        self.evict_for(size)?;
        let ticket = tack_storage::new_document_id()?.value();
        let path = self.root.join(format!(".asset-{ticket:032x}.part"));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        Ok((path.clone(), options.open(path)?))
    }
    pub fn finish(&mut self, temporary: &Path, hash: &ContentHash, size: u64) -> Result<PathBuf> {
        require_regular(temporary)?;
        if temporary.parent() != Some(self.root.as_path()) || fs::metadata(temporary)?.len() != size
        {
            return Err(Error::Invalid("shared cache staging length/path"));
        }
        let target = self.path(hash);
        if target.exists() {
            require_regular(&target)?;
            let (existing, length) = hash_reader(&mut File::open(&target)?)?;
            if existing != *hash || length != size {
                return Err(Error::Invalid("shared cache target hash mismatch"));
            }
            fs::remove_file(temporary)?;
        } else {
            fs::rename(temporary, &target)?;
        }
        if let Some((old, _)) = self.entries.remove(hash) {
            self.bytes = self.bytes.saturating_sub(old);
        }
        self.bytes = self
            .bytes
            .checked_add(size)
            .ok_or(Error::Invalid("shared cache size overflow"))?;
        self.clock = self.clock.saturating_add(1);
        self.entries.insert(hash.clone(), (size, self.clock));
        let metadata = fs::metadata(&target)?;
        self.verified
            .insert(hash.clone(), (metadata.len(), metadata.modified()?));
        Ok(target)
    }
    fn evict_for(&mut self, incoming: u64) -> Result<()> {
        while self
            .bytes
            .checked_add(incoming)
            .is_none_or(|b| b > CACHE_BYTES)
            || self.entries.len() >= MAX_FILES
        {
            let key = self
                .entries
                .iter()
                .min_by_key(|(_, (_, stamp))| *stamp)
                .map(|(key, _)| key.clone())
                .ok_or(Error::Invalid("client cache cannot admit source"))?;
            let path = self.path(&key);
            require_regular(&path)?;
            fs::remove_file(path)?;
            if let Some((bytes, _)) = self.entries.remove(&key) {
                self.bytes = self.bytes.saturating_sub(bytes);
            }
            self.evicted.push(key);
            if let Some(key) = self.evicted.last() {
                self.verified.remove(key);
            }
        }
        Ok(())
    }
}
pub(crate) struct CancelReader<'a, R> {
    pub inner: R,
    pub cancel: &'a AtomicBool,
}
impl<R: Read> Read for CancelReader<'_, R> {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        if self.cancel.load(Ordering::Relaxed) {
            return Err(std::io::Error::other("shared transfer cancelled"));
        }
        self.inner.read(bytes)
    }
}
fn partial_name(name: &str) -> bool {
    name.strip_prefix(".asset-")
        .and_then(|s| s.strip_suffix(".part"))
        .is_some_and(|s| WireId::parse(s).is_ok())
}
fn snapshot_name(name: &str) -> bool {
    let Some(body) = name
        .strip_prefix("snapshot-")
        .and_then(|s| s.strip_suffix(".tack"))
    else {
        return false;
    };
    body.split_once('-')
        .is_some_and(|(a, b)| WireId::parse(a).is_ok() && WireId::parse(b).is_ok())
}
fn snapshot_lock_name(name: &str) -> bool {
    name.strip_suffix(".tack-lock").is_some_and(snapshot_name)
}
fn retire_snapshots(root: &Path) -> Result<()> {
    let mut paths = BTreeMap::new();
    for (index, entry) in fs::read_dir(root)?.enumerate() {
        if index >= MAX_FILES + 32 {
            return Err(Error::Invalid("shared cache entry count"));
        }
        let entry = entry?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if snapshot_name(name) {
            require_regular(&entry.path())?;
            paths.insert(entry.path(), ());
        } else if snapshot_lock_name(name) {
            require_regular(&entry.path())?;
            if entry.metadata()?.len() != 0 {
                return Err(Error::Invalid("shared metadata snapshot lock size"));
            }
            paths.insert(root.join(name.trim_end_matches(".tack-lock")), ());
        }
    }
    let mut retained = 0;
    for (path, ()) in paths {
        let mut lock_name = path.as_os_str().to_os_string();
        lock_name.push(".tack-lock");
        let lock_path = PathBuf::from(lock_name);
        // Internal snapshot names are single-use, generated by this reader worker.
        // The cache never exposes them for editing or reuses a retired name. Respect
        // an existing lease, then retire its sidecar only after the lease is released.
        let Ok(lock) = tack_storage::lock_sidecar(&lock_path) else {
            retained += 1;
            continue;
        };
        let removed = match fs::remove_file(&path) {
            Ok(()) => true,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => true,
            Err(_) => false,
        };
        drop(lock);
        if !removed || fs::remove_file(&lock_path).is_err() {
            retained += 1;
        }
    }
    if retained >= 2 {
        return Err(Error::Invalid("shared snapshot files still in use"));
    }
    Ok(())
}
/// Canonical state stays Embedded. This local metadata-only view resolves sibling CAS files.
pub(crate) fn snapshot_view(
    root: &Path,
    document: &Document,
    bindings: &[SourceBinding],
) -> Result<(PathBuf, Arc<TackFile>)> {
    owned_directory(root)?;
    let mut view = document.clone();
    let mut bound = BTreeMap::new();
    for binding in bindings {
        binding.validate()?;
        let id = SourceId::new(binding.source.value()).map_err(domain_error)?;
        let source = view
            .source(id)
            .ok_or(Error::Invalid("binding source absent"))?;
        if source.revision() != binding.revision {
            return Err(Error::Invalid("binding revision mismatch"));
        }
        if bound.insert(id, binding).is_some() {
            return Err(Error::Invalid("duplicate source binding"));
        }
    }
    for source in document.sources() {
        let name = bound
            .get(&source.id())
            .map(|b| b.hash.to_string())
            .unwrap_or_else(|| format!(".missing-source-{:032x}", source.id().value()));
        let location =
            SourceLocation::Linked(LinkedPath::native(Path::new(&name)).map_err(domain_error)?);
        view.apply_inverse(Command::SetSource(
            Source::from_descriptor(source.id(), location, source.revision(), None)
                .map_err(domain_error)?,
        ))
        .map_err(domain_error)?;
    }
    // Old file handles retain their inode. Windows may retain the currently open view;
    // two small metadata snapshots are allowed, never an unbounded reconnect history.
    retire_snapshots(root)?;
    let path = root.join(format!(
        "snapshot-{:032x}-{:032x}.tack",
        document.id().value(),
        tack_storage::new_document_id()?.value()
    ));
    tack_storage::save(&path, &view, vec![])?;
    let board = Arc::new(TackFile::open(&path)?);
    Ok((path, board))
}

#[cfg(test)]
mod tests;
