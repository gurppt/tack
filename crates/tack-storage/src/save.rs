use crate::{self as storage, codec, *};
use std::{
    collections::BTreeSet,
    fs::{File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::Arc,
};
use tack_core::{AssetId, Document, SourceId, SourceLocation};

#[derive(Clone)]
pub enum Payload {
    File(PathBuf),
    Stored { file: Arc<File>, range: BlobRange },
}
impl Payload {
    /// Pin a bounded disposable overview's inode and checksum on a storage worker.
    /// Cache eviction after this call cannot invalidate an active save.
    pub fn pin_overview(path: impl AsRef<Path>) -> Result<Self> {
        let mut file = File::open(path)?;
        let len = file.metadata()?.len();
        if len == 0 || len > MAX_OVERVIEW_BYTES {
            return Err(StorageError::Invalid("pinned overview length"));
        }
        let mut crc = crc32fast::Hasher::new();
        let mut read = 0u64;
        let mut buffer = [0u8; 32 * 1024];
        loop {
            let count = file.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            read += count as u64;
            if read > len {
                return Err(StorageError::Invalid("overview changed while pinning"));
            }
            crc.update(&buffer[..count]);
        }
        if read != len {
            return Err(StorageError::Invalid("overview changed while pinning"));
        }
        Ok(Self::Stored {
            file: Arc::new(file),
            range: BlobRange {
                offset: 0,
                len,
                crc32: crc.finalize(),
            },
        })
    }

    pub fn len(&self) -> Result<u64> {
        match self {
            Self::File(p) => Ok(std::fs::metadata(p)?.len()),
            Self::Stored { range, .. } => Ok(range.len),
        }
    }
    pub fn is_empty(&self) -> Result<bool> {
        Ok(self.len()? == 0)
    }
    fn reader(&self) -> Result<Box<dyn Read>> {
        match self {
            Self::File(p) => Ok(Box::new(File::open(p)?)),
            Self::Stored { file, range } => {
                Ok(Box::new(RangeReader::new(Arc::clone(file), *range)))
            }
        }
    }
}
pub struct BlobInput {
    pub entry: OverviewEntry,
    pub original: bool,
    pub payload: Payload,
}
impl BlobInput {
    pub fn original(id: SourceId, revision: u64, payload: Payload) -> Self {
        Self {
            entry: OverviewEntry {
                id: id.value(),
                revision,
                role: 0,
                encoding: 0,
                generator: 0,
                width: 0,
                height: 0,
                range: BlobRange {
                    offset: 0,
                    len: 0,
                    crc32: 0,
                },
            },
            original: true,
            payload,
        }
    }
    pub fn overview(
        id: AssetId,
        revision: u64,
        size: [u32; 2],
        generator: u32,
        payload: Payload,
    ) -> Self {
        Self {
            entry: OverviewEntry {
                id: id.value(),
                revision,
                role: 1,
                encoding: 1,
                generator,
                width: size[0],
                height: size[1],
                range: BlobRange {
                    offset: 0,
                    len: 0,
                    crc32: 0,
                },
            },
            original: false,
            payload,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveStage {
    TemporaryCreated,
    PayloadCopied(usize),
    MetadataWritten,
    FileSynced,
}
struct Temporary(PathBuf);
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
/// Entire operation belongs to storage/CLI workers, never an event callback.
/// No original is buffered; prior stored CRC is checked while copying.
pub fn save(path: impl AsRef<Path>, doc: &Document, inputs: Vec<BlobInput>) -> Result<()> {
    save_with_hook(path, doc, inputs, |_| Ok(()))
}
pub fn save_with_hook(
    path: impl AsRef<Path>,
    doc: &Document,
    inputs: Vec<BlobInput>,
    mut hook: impl FnMut(SaveStage) -> Result<()>,
) -> Result<()> {
    let lease = crate::BoardLease::acquire(path.as_ref())?;
    let expected = lease
        .expected
        .lock()
        .map_err(|_| StorageError::Invalid("board ownership poisoned"))?;
    publish(path.as_ref(), doc, inputs, |stage| {
        hook(stage)?;
        if stage == SaveStage::FileSynced
            && crate::ownership::FileStamp::read(path.as_ref())? != *expected
        {
            return Err(StorageError::Invalid("board changed outside Tack"));
        }
        Ok(())
    })
}
pub(crate) fn publish(
    path: impl AsRef<Path>,
    doc: &Document,
    mut inputs: Vec<BlobInput>,
    mut hook: impl FnMut(SaveStage) -> Result<()>,
) -> Result<()> {
    if inputs.len() > MAX_RECORDS * 2 {
        return Err(StorageError::Invalid("payload count"));
    }
    inputs.sort_by_key(|i| (!i.original, i.entry.id));
    let mut originals = BTreeSet::new();
    let mut overviews = BTreeSet::new();
    for i in &mut inputs {
        i.entry.range.len = i.payload.len()?;
        if i.entry.range.len == 0 || i.entry.range.len > MAX_FILE_BYTES {
            return Err(StorageError::Invalid("payload length"));
        }
        if i.original {
            let id = SourceId::new(i.entry.id).map_err(|_| StorageError::Invalid("source ID"))?;
            let s = doc
                .source(id)
                .ok_or(StorageError::Invalid("original reference"))?;
            if !matches!(s.location(), SourceLocation::Embedded)
                || s.revision() != i.entry.revision
                || !originals.insert(id)
            {
                return Err(StorageError::Invalid("original binding"));
            }
            if i.entry.role != 0
                || i.entry.encoding != 0
                || i.entry.generator != 0
                || i.entry.width != 0
                || i.entry.height != 0
            {
                return Err(StorageError::Invalid("original metadata"));
            }
        } else {
            let id = AssetId::new(i.entry.id).map_err(|_| StorageError::Invalid("asset ID"))?;
            let s = doc
                .asset(id)
                .and_then(|a| doc.source(a.source_id()))
                .ok_or(StorageError::Invalid("overview reference"))?;
            if s.revision() != i.entry.revision
                || !overviews.insert(id)
                || i.entry.range.len > MAX_OVERVIEW_BYTES
                || i.entry.role != 1
                || i.entry.encoding != 1
                || ![1, 2].contains(&i.entry.generator)
                || i.entry.width == 0
                || i.entry.height == 0
                || i.entry.width > 512
                || i.entry.height > 512
            {
                return Err(StorageError::Invalid("overview metadata"));
            }
        }
    }
    if originals.len() > MAX_RECORDS
        || overviews.len() > MAX_RECORDS
        || doc
            .sources()
            .filter(|s| matches!(s.location(), SourceLocation::Embedded))
            .count()
            != originals.len()
    {
        return Err(StorageError::Invalid("embedded original completeness"));
    }
    let domain = codec::encode_document(doc)?;
    let auth_len = domain
        .len()
        .checked_add(originals.len() * 64)
        .filter(|n| *n <= MAX_METADATA_BYTES)
        .ok_or(StorageError::Invalid("metadata budget"))?;
    let derived_len = overviews.len() * 64;
    let mut offset = (HEADER_BYTES + auth_len + derived_len) as u64;
    for i in &mut inputs {
        i.entry.range.offset = offset;
        offset = offset
            .checked_add(i.entry.range.len)
            .filter(|n| *n <= MAX_FILE_BYTES)
            .ok_or(StorageError::Invalid("file budget"))?;
    }
    let path = path.as_ref();
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let name = path
        .file_name()
        .ok_or(StorageError::Invalid("save target name"))?;
    let mut temporary_name = name.to_os_string();
    temporary_name.push(format!(
        ".tack-tmp-{:032x}",
        storage::new_document_id()?.value()
    ));
    let temporary_path = parent.join(temporary_name);
    let mut options = OpenOptions::new();
    options.write(true).read(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let existing_permissions = match std::fs::symlink_metadata(path) {
        Ok(m) if m.is_file() => Some(m.permissions()),
        Ok(_) => return Err(StorageError::Invalid("save target is not a regular file")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(e.into()),
    };
    let mut file = options.open(&temporary_path)?;
    let temporary = Temporary(temporary_path);
    hook(SaveStage::TemporaryCreated)?;
    file.seek(SeekFrom::Start(
        (HEADER_BYTES + auth_len + derived_len) as u64,
    ))?;
    let mut buffer = vec![0u8; 128 * 1024];
    for (index, i) in inputs.iter_mut().enumerate() {
        let before = match &i.payload {
            Payload::File(p) => Some(std::fs::metadata(p)?),
            _ => None,
        };
        let mut reader = i.payload.reader()?;
        let mut remaining = i.entry.range.len;
        let mut hash = crc32fast::Hasher::new();
        while remaining > 0 {
            let size = remaining.min(buffer.len() as u64) as usize;
            let n = reader.read(&mut buffer[..size])?;
            if n == 0 {
                return Err(StorageError::Corrupt("short original/overview input"));
            }
            file.write_all(&buffer[..n])?;
            hash.update(&buffer[..n]);
            remaining -= n as u64;
        }
        let crc = hash.finalize();
        match &i.payload {
            Payload::Stored { range, .. } if range.crc32 != crc => {
                return Err(StorageError::Corrupt("copied stored payload"));
            }
            Payload::File(p) => {
                let after = std::fs::metadata(p)?;
                if before.as_ref().is_some_and(|m| {
                    m.len() != after.len() || m.modified().ok() != after.modified().ok()
                }) {
                    return Err(StorageError::Invalid("source changed while saving"));
                }
                let mut extra = [0];
                if reader.read(&mut extra)? != 0 {
                    return Err(StorageError::Invalid("source grew while saving"));
                }
            }
            _ => {}
        }
        i.entry.range.crc32 = crc;
        hook(SaveStage::PayloadCopied(index))?;
    }
    let mut auth = domain;
    let mut derived = Vec::with_capacity(derived_len);
    for i in inputs {
        if i.original {
            auth.extend(codec::encode_entry(&i.entry, 1));
        } else {
            derived.extend(codec::encode_entry(&i.entry, 2));
        }
    }
    let mut header = Vec::with_capacity(80);
    header.extend(b"TACKSN01");
    header.extend(1u32.to_le_bytes());
    header.extend(codec::spatial_schema(doc).to_le_bytes());
    header.extend((auth_len as u64).to_le_bytes());
    header.extend((derived_len as u64).to_le_bytes());
    header.extend(offset.to_le_bytes());
    header.extend(crc32fast::hash(&auth).to_le_bytes());
    header.extend(crc32fast::hash(&derived).to_le_bytes());
    for n in [
        doc.sources().count(),
        doc.assets().count(),
        doc.object_order().len(),
        originals.len(),
        overviews.len(),
    ] {
        header.extend((n as u32).to_le_bytes());
    }
    header.extend([0; 12]);
    file.seek(SeekFrom::Start(0))?;
    file.write_all(&header)?;
    file.write_all(&auth)?;
    file.write_all(&derived)?;
    hook(SaveStage::MetadataWritten)?;
    if let Some(permissions) = existing_permissions {
        file.set_permissions(permissions)?;
    }
    file.sync_all()?;
    hook(SaveStage::FileSynced)?;
    drop(file);
    // Never remove the target first. Failure leaves its prior valid generation.
    std::fs::rename(&temporary.0, path)?;
    // Process-crash consistency is tested. Directory fsync strengthens Unix durability
    // but no simulated power-loss guarantee is claimed; post-publication error is I/O.
    #[cfg(unix)]
    File::open(parent)
        .and_then(|f| f.sync_all())
        .map_err(StorageError::PublishedButNotDirectorySynced)?;
    Ok(())
}
