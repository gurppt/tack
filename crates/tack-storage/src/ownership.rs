//! Worker-only cooperative single-writer ownership, retained across atomic replacement.
use crate::{BlobInput, Result, StorageError, TackFile};
use std::{
    fs::{File, OpenOptions},
    io::Read,
    path::{Path, PathBuf},
    sync::Mutex,
};
use tack_core::Document;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FileStamp(pub [u8; 104]);
impl FileStamp {
    pub(crate) fn read(path: &Path) -> Result<Option<Self>> {
        let metadata = match std::fs::symlink_metadata(path) {
            Ok(m) if m.is_file() => m,
            Ok(_) => return Err(StorageError::Invalid("target must be a regular file")),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        let mut bytes = [0; 104];
        let mut file = File::open(path)?;
        file.read_exact(&mut bytes[..80])?;
        bytes[80..88].copy_from_slice(&metadata.len().to_le_bytes());
        let time = metadata
            .modified()?
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| StorageError::Invalid("file time before epoch"))?
            .as_nanos();
        bytes[88..].copy_from_slice(&time.to_le_bytes());
        Ok(Some(Self(bytes)))
    }
}

/// No singleton. One stable sidecar inode is locked until the last owner drops.
/// Sidecars are never unlinked: unlinking could split ownership across inodes.
pub struct BoardLease {
    pub(crate) path: PathBuf,
    _lock: File,
    pub(crate) expected: Mutex<Option<FileStamp>>,
}
impl BoardLease {
    pub fn acquire(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let name = path
            .file_name()
            .ok_or(StorageError::Invalid("board filename"))?;
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let path = parent.canonicalize()?.join(name);
        // Resolve existing aliases for ownership, while source resolution retains its lexical base.
        let path = if path.exists() {
            path.canonicalize()?
        } else {
            path
        };
        let mut lock_name = path
            .file_name()
            .ok_or(StorageError::Invalid("board filename"))?
            .to_os_string();
        lock_name.push(".tack-lock");
        let lock_path = path.with_file_name(lock_name);
        let lock = lock_sidecar(&lock_path)?;
        let expected = Mutex::new(FileStamp::read(&path)?);
        Ok(Self {
            path,
            _lock: lock,
            expected,
        })
    }
    /// Creation must still own an absent target after taking the cooperative lock.
    pub fn acquire_new(path: impl AsRef<Path>) -> Result<Self> {
        let lease = Self::acquire(path)?;
        if lease
            .expected
            .lock()
            .map_err(|_| StorageError::Invalid("board ownership poisoned"))?
            .is_some()
        {
            return Err(StorageError::Invalid(
                "new board requires an unused filename",
            ));
        }
        Ok(lease)
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn open(&self) -> Result<TackFile> {
        let expected = self
            .expected
            .lock()
            .map_err(|_| StorageError::Invalid("board ownership poisoned"))?;
        if FileStamp::read(&self.path)? != *expected {
            return Err(StorageError::Invalid("board changed while opening"));
        }
        let board = TackFile::open(&self.path)?;
        if FileStamp::read(&self.path)? != *expected {
            return Err(StorageError::Invalid("board changed while opening"));
        }
        Ok(board)
    }
    pub fn save(&self, doc: &Document, inputs: Vec<BlobInput>) -> Result<()> {
        let mut expected = self
            .expected
            .lock()
            .map_err(|_| StorageError::Invalid("board ownership poisoned"))?;
        if FileStamp::read(&self.path)? != *expected {
            return Err(StorageError::Invalid(
                "board changed outside Tack; use Save As",
            ));
        }
        let result = crate::save::publish(&self.path, doc, inputs, |stage| {
            if matches!(stage, crate::SaveStage::FileSynced)
                && FileStamp::read(&self.path)? != *expected
            {
                return Err(StorageError::Invalid(
                    "board changed outside Tack; use Save As",
                ));
            }
            Ok(())
        });
        if result.is_ok() || matches!(result, Err(StorageError::PublishedButNotDirectorySynced(_)))
        {
            *expected = FileStamp::read(&self.path)?;
        }
        result
    }
}

/// Worker-only non-blocking cooperative lock for a trusted local sidecar path.
/// Keep the returned handle alive; never remove the sidecar while owners may exist.
pub fn lock_sidecar(lock_path: &Path) -> Result<File> {
    match std::fs::symlink_metadata(lock_path) {
        Ok(m) if !m.is_file() => return Err(StorageError::Invalid("unsafe board lock path")),
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.into()),
    }
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let lock = options.open(lock_path)?;
    if !std::fs::symlink_metadata(lock_path)?.is_file() || !lock.metadata()?.is_file() {
        return Err(StorageError::Invalid("unsafe board lock path"));
    }
    lock.try_lock().map_err(|e| match e {
        std::fs::TryLockError::WouldBlock => {
            StorageError::Invalid("board already open for editing in another instance")
        }
        std::fs::TryLockError::Error(e) => StorageError::Io(e),
    })?;
    Ok(lock)
}

impl Drop for BoardLease {
    fn drop(&mut self) {
        // Explicit release also prevents a concurrently spawned child's transient
        // inherited pre-exec handle from prolonging this owner's finished lease.
        let _ = self._lock.unlock();
    }
}
