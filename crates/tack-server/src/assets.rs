use crate::Result;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use tack_shared::{ContentHash, MAX_ASSET_BYTES, MAX_CHUNK_BYTES};

struct Accounting {
    stored: u64,
    reserved: u64,
    inflight: BTreeMap<ContentHash, u64>,
}
#[derive(Clone)]
pub struct AssetStore {
    root: PathBuf,
    quota: u64,
    accounting: Arc<Mutex<Accounting>>,
}
pub struct Upload {
    store: AssetStore,
    hash: ContentHash,
    size: u64,
    offset: u64,
    path: PathBuf,
    file: Option<File>,
    digest: Sha256,
    done: bool,
}
impl AssetStore {
    pub fn new(root: &Path, quota: u64) -> Result<Self> {
        if quota == 0 {
            return Err("asset quota must be nonzero".into());
        }
        if !root.exists() {
            tack_storage::create_private_directory(root, true).map_err(|e| e.to_string())?;
        }
        let mut stored = 0u64;
        for entry in fs::read_dir(root).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let name = entry.file_name();
            let name = name.to_str().ok_or("invalid CAS filename")?;
            let metadata = fs::symlink_metadata(entry.path()).map_err(|e| e.to_string())?;
            if !metadata.file_type().is_file() {
                return Err("CAS contains non-regular file".into());
            }
            if let Some(hash) = name.strip_suffix(".upload") {
                ContentHash::parse(hash).map_err(|e| e.to_string())?;
                fs::remove_file(entry.path()).map_err(|e| e.to_string())?;
                continue;
            }
            ContentHash::parse(name).map_err(|e| e.to_string())?;
            stored = stored
                .checked_add(metadata.len())
                .ok_or("CAS accounting overflow")?;
        }
        if stored > quota {
            return Err("existing CAS exceeds configured quota".into());
        }
        Ok(Self {
            root: root.to_path_buf(),
            quota,
            accounting: Arc::new(Mutex::new(Accounting {
                stored,
                reserved: 0,
                inflight: BTreeMap::new(),
            })),
        })
    }
    pub fn length(&self, hash: ContentHash) -> Result<Option<u64>> {
        match fs::symlink_metadata(self.root.join(hash.to_string())) {
            Ok(m) if m.file_type().is_file() => Ok(Some(m.len())),
            Ok(_) => Err("CAS is not regular file".into()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }
    pub fn begin(&self, hash: ContentHash, size: u64) -> Result<Option<Upload>> {
        if size > MAX_ASSET_BYTES {
            return Err("asset length bound".into());
        }
        if let Some(existing) = self.length(hash.clone())? {
            return if existing == size {
                Ok(None)
            } else {
                Err("existing hash length mismatch".into())
            };
        }
        let mut accounting = self.accounting.lock().map_err(|_| "CAS accounting lock")?;
        if accounting.inflight.contains_key(&hash) {
            return Err("same hash upload already in flight".into());
        }
        if accounting
            .stored
            .saturating_add(accounting.reserved)
            .saturating_add(size)
            > self.quota
        {
            return Err("CAS quota reached".into());
        }
        let path = self.root.join(format!("{hash}.upload"));
        let mut options = OpenOptions::new();
        options.create_new(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let file = options.open(&path).map_err(|e| e.to_string())?;
        accounting.reserved += size;
        accounting.inflight.insert(hash.clone(), size);
        Ok(Some(Upload {
            store: self.clone(),
            hash,
            size,
            offset: 0,
            path,
            file: Some(file),
            digest: Sha256::new(),
            done: false,
        }))
    }
    pub fn range(&self, hash: ContentHash, offset: u64, length: u32) -> Result<(u64, Vec<u8>)> {
        if length == 0 || length as usize > MAX_CHUNK_BYTES {
            return Err("asset range length".into());
        }
        let size = self.length(hash.clone())?.ok_or("asset unavailable")?;
        if offset > size {
            return Err("asset range offset".into());
        }
        let end = offset
            .checked_add(u64::from(length))
            .ok_or("asset range overflow")?
            .min(size);
        let mut file = File::open(self.root.join(hash.to_string())).map_err(|e| e.to_string())?;
        file.seek(SeekFrom::Start(offset))
            .map_err(|e| e.to_string())?;
        let mut bytes = vec![0; (end - offset) as usize];
        file.read_exact(&mut bytes).map_err(|e| e.to_string())?;
        Ok((size, bytes))
    }
    pub fn stored_bytes(&self) -> Result<u64> {
        Ok(self
            .accounting
            .lock()
            .map_err(|_| "CAS accounting lock")?
            .stored)
    }
}
impl Upload {
    pub fn hash(&self) -> ContentHash {
        self.hash.clone()
    }
    pub fn offset(&self) -> u64 {
        self.offset
    }
    pub fn append(&mut self, offset: u64, bytes: &[u8]) -> Result<u64> {
        if bytes.is_empty()
            || bytes.len() > MAX_CHUNK_BYTES
            || offset != self.offset
            || self.offset.saturating_add(bytes.len() as u64) > self.size
        {
            return Err("invalid asset upload chunk/offset".into());
        }
        self.file
            .as_mut()
            .ok_or("closed upload")?
            .write_all(bytes)
            .map_err(|e| e.to_string())?;
        self.digest.update(bytes);
        self.offset += bytes.len() as u64;
        Ok(self.offset)
    }
    pub fn commit(mut self) -> Result<u64> {
        if self.offset != self.size {
            return Err("incomplete asset upload".into());
        }
        let expected = ContentHash::parse(&format!("{:x}", self.digest.clone().finalize()))
            .map_err(|e| e.to_string())?;
        if expected != self.hash {
            return Err("asset SHA256 mismatch".into());
        }
        let file = self.file.take().ok_or("closed upload")?;
        file.sync_all().map_err(|e| e.to_string())?;
        drop(file);
        fs::rename(&self.path, self.store.root.join(self.hash.to_string()))
            .map_err(|e| e.to_string())?;
        let mut accounting = self
            .store
            .accounting
            .lock()
            .map_err(|_| "CAS accounting lock")?;
        accounting.reserved = accounting.reserved.saturating_sub(self.size);
        accounting.stored += self.size;
        accounting.inflight.remove(&self.hash);
        self.done = true;
        drop(accounting);
        #[cfg(unix)]
        crate::persistence::sync_directory(&self.store.root)
            .map_err(|e| format!("CAS published but directory sync failed: {e}"))?;
        Ok(self.size)
    }
}
impl Drop for Upload {
    fn drop(&mut self) {
        if !self.done {
            self.file.take();
            let _ = fs::remove_file(&self.path);
            if let Ok(mut a) = self.store.accounting.lock() {
                a.reserved = a.reserved.saturating_sub(self.size);
                a.inflight.remove(&self.hash);
            }
        }
    }
}

#[cfg(all(test, unix))]
mod publication_tests {
    use super::*;
    #[test]
    fn postrename_sync_failure_keeps_cas_accounting() -> Result<()> {
        let id = tack_storage::new_document_id().map_err(|e| e.to_string())?;
        let root = std::env::temp_dir().join(format!("tack-cas-sync-{}", id.value()));
        let store = AssetStore::new(&root, 8)?;
        let hash = ContentHash::digest(b"original");
        let mut upload = store.begin(hash.clone(), 8)?.ok_or("upload")?;
        upload.append(0, b"original")?;
        crate::persistence::FAIL_SYNC.with(|v| v.set(true));
        let result = upload.commit();
        crate::persistence::FAIL_SYNC.with(|v| v.set(false));
        assert!(result.is_err());
        assert_eq!(store.stored_bytes()?, 8);
        assert_eq!(store.length(hash)?, Some(8));
        assert!(store.begin(ContentHash::digest(b"x"), 1).is_err());
        std::fs::remove_dir_all(root).map_err(|e| e.to_string())?;
        Ok(())
    }
}
