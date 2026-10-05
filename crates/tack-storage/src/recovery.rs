//! Separate bounded recovery snapshots. No promotion to authoritative save.
use crate::{BlobInput, BoardLease, Result, StorageError, TackFile, ownership::FileStamp};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};
use tack_core::{Document, DocumentId};
const OWNER_MAGIC: &[u8; 8] = b"TACKRC01";
const STATE_MAGIC: &[u8; 8] = b"TACKRC02";
const MANIFEST_BYTES: usize = 232;

fn regular_bytes(path: &Path, length: usize) -> Result<Vec<u8>> {
    let m = std::fs::symlink_metadata(path)?;
    if !m.is_file() || m.len() != length as u64 {
        return Err(StorageError::Invalid("recovery control file"));
    }
    let mut bytes = Vec::with_capacity(length);
    File::open(path)?
        .take(length as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() != length {
        return Err(StorageError::Invalid("recovery control length"));
    }
    Ok(bytes)
}
fn manifest_slot(bytes: &[u8]) -> Result<u64> {
    if bytes.len() != MANIFEST_BYTES || &bytes[..8] != STATE_MAGIC {
        return Err(StorageError::Invalid("recovery manifest version"));
    }
    let slot = u64::from_le_bytes(
        bytes[16..24]
            .try_into()
            .map_err(|_| StorageError::Invalid("recovery slot"))?,
    );
    if slot > 1 {
        return Err(StorageError::Invalid("recovery slot"));
    }
    Ok(slot)
}
fn create_file(path: &Path) -> std::io::Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}
fn owned_directory(dir: &Path, id: DocumentId, create: bool) -> Result<bool> {
    match std::fs::symlink_metadata(dir) {
        Ok(m) if !m.is_dir() => return Err(StorageError::Invalid("unsafe recovery directory")),
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            if !create {
                return Ok(false);
            }
            crate::create_private_directory(dir, false)?;
            let mut owner = create_file(&dir.join("owner"))?;
            owner.write_all(OWNER_MAGIC)?;
            owner.write_all(&id.value().to_le_bytes())?;
            owner.sync_all()?;
            #[cfg(unix)]
            {
                File::open(dir)?.sync_all()?;
                File::open(
                    dir.parent()
                        .ok_or(StorageError::Invalid("recovery parent"))?,
                )?
                .sync_all()?;
            }
        }
        Err(e) => return Err(e.into()),
    }
    let owner = regular_bytes(&dir.join("owner"), 24)?;
    if &owner[..8] != OWNER_MAGIC || owner[8..] != id.value().to_le_bytes() {
        return Err(StorageError::Invalid(
            "recovery directory belongs to another document",
        ));
    }
    Ok(true)
}
fn atomic_manifest(dir: &Path, bytes: &[u8]) -> Result<()> {
    let temporary = dir.join("control.tmp");
    let result = (|| -> Result<()> {
        let mut file = create_file(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        // A control symlink is never followed or replaced.
        if std::fs::symlink_metadata(dir.join("state.meta")).is_ok_and(|m| !m.is_file()) {
            return Err(StorageError::Invalid("unsafe recovery manifest"));
        }
        std::fs::rename(&temporary, dir.join("state.meta"))?;
        #[cfg(unix)]
        File::open(dir)?.sync_all()?;
        Ok(())
    })();
    let _ = std::fs::remove_file(temporary);
    result
}
fn cleanup_staging(dir: &Path) -> Result<()> {
    for (index, entry) in std::fs::read_dir(dir)?.enumerate() {
        if index >= 512 {
            return Err(StorageError::Invalid("recovery directory entry bound"));
        }
        let entry = entry?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let suffix = name
            .strip_prefix("state-0.tack.tack-tmp-")
            .or_else(|| name.strip_prefix("state-1.tack.tack-tmp-"));
        let known = name == "control.tmp"
            || suffix.is_some_and(|s| s.len() == 32 && s.bytes().all(|b| b.is_ascii_hexdigit()));
        if known {
            if !entry.file_type()?.is_file() {
                return Err(StorageError::Invalid("unsafe recovery staging entry"));
            }
            std::fs::remove_file(entry.path())?;
        }
    }
    Ok(())
}
impl BoardLease {
    pub fn owns_recovery(&self, id: DocumentId) -> Result<bool> {
        owned_directory(&self.recovery_directory(), id, false)
    }
    /// Caller owns a private Untitled seed, never a user-selected authoritative path.
    /// The stable sidecar is kept so a future slot reuse cannot split ownership.
    pub fn retire_empty_seed(&self, id: DocumentId) -> Result<()> {
        let board = self.open()?;
        if board.document.id() != id
            || board.document.objects().next().is_some()
            || board.document.sources().next().is_some()
            || board.document.assets().next().is_some()
        {
            return Err(StorageError::Invalid(
                "only an owned empty seed can be retired",
            ));
        }
        self.discard_recovery(id)?;
        let dir = self.recovery_directory();
        if dir.exists() {
            let mut entries = std::fs::read_dir(&dir)?;
            let only_owner = entries
                .next()
                .transpose()?
                .is_some_and(|e| e.file_name() == "owner")
                && entries.next().is_none();
            if !only_owner {
                return Err(StorageError::Invalid("unknown recovery entries retained"));
            }
            std::fs::remove_file(dir.join("owner"))?;
            std::fs::remove_dir(dir)?;
        }
        std::fs::remove_file(&self.path)?;
        *self
            .expected
            .lock()
            .map_err(|_| StorageError::Invalid("board ownership poisoned"))? = None;
        #[cfg(unix)]
        File::open(
            self.path
                .parent()
                .ok_or(StorageError::Invalid("seed parent"))?,
        )?
        .sync_all()?;
        Ok(())
    }
    pub fn recovery_directory(&self) -> PathBuf {
        let mut name = std::ffi::OsString::from(".");
        if let Some(leaf) = self.path.file_name() {
            name.push(leaf);
        }
        name.push(".tack-recovery");
        self.path.with_file_name(name)
    }
    pub fn save_recovery(
        &self,
        doc: &Document,
        generation: u64,
        inputs: Vec<BlobInput>,
    ) -> Result<u64> {
        self.save_recovery_with_hook(doc, generation, inputs, || Ok(()))
    }
    /// Fault-injection boundary after the inactive snapshot and before manifest publication.
    pub fn save_recovery_with_hook(
        &self,
        doc: &Document,
        generation: u64,
        inputs: Vec<BlobInput>,
        before_manifest: impl FnOnce() -> Result<()>,
    ) -> Result<u64> {
        if generation == 0 {
            return Err(StorageError::Invalid("recovery requires committed edits"));
        }
        let expected = self
            .expected
            .lock()
            .map_err(|_| StorageError::Invalid("board ownership poisoned"))?;
        let normal = expected.ok_or(StorageError::Invalid("save the new board before recovery"))?;
        if FileStamp::read(&self.path)? != Some(normal) {
            return Err(StorageError::Invalid(
                "board changed outside Tack; recovery retained",
            ));
        }
        let dir = self.recovery_directory();
        owned_directory(&dir, doc.id(), true)?;
        cleanup_staging(&dir)?;
        let previous = match regular_bytes(&dir.join("state.meta"), MANIFEST_BYTES) {
            Ok(bytes) => Some(manifest_slot(&bytes)?),
            Err(StorageError::Io(e)) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(e),
        };
        let slot = previous.map_or(0, |slot| 1 - slot);
        let path = dir.join(format!("state-{slot}.tack"));
        crate::save::publish(&path, doc, inputs, |_| Ok(()))?;
        let stamp =
            FileStamp::read(&path)?.ok_or(StorageError::Invalid("missing recovery publication"))?;
        let mut bytes = Vec::with_capacity(MANIFEST_BYTES);
        bytes.extend(STATE_MAGIC);
        bytes.extend(generation.to_le_bytes());
        bytes.extend(slot.to_le_bytes());
        bytes.extend(normal.0);
        bytes.extend(stamp.0);
        before_manifest()?;
        atomic_manifest(&dir, &bytes)?;
        Ok(std::fs::metadata(path)?.len() + MANIFEST_BYTES as u64)
    }
    pub fn recovery(&self, id: DocumentId) -> Result<Option<(TackFile, u64)>> {
        let dir = self.recovery_directory();
        if !owned_directory(&dir, id, false)? {
            return Ok(None);
        }
        let manifest = match regular_bytes(&dir.join("state.meta"), MANIFEST_BYTES) {
            Ok(bytes) => bytes,
            Err(StorageError::Io(e)) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(None);
            }
            Err(e) => return Err(e),
        };
        if &manifest[..8] != STATE_MAGIC {
            return Err(StorageError::Invalid("recovery manifest version"));
        }
        let generation = u64::from_le_bytes(
            manifest[8..16]
                .try_into()
                .map_err(|_| StorageError::Invalid("recovery generation"))?,
        );
        if generation == 0 {
            return Err(StorageError::Invalid("recovery generation"));
        }
        let expected = self
            .expected
            .lock()
            .map_err(|_| StorageError::Invalid("board ownership poisoned"))?;
        let normal = FileStamp(
            manifest[24..128]
                .try_into()
                .map_err(|_| StorageError::Invalid("recovery base"))?,
        );
        let snapshot = FileStamp(
            manifest[128..]
                .try_into()
                .map_err(|_| StorageError::Invalid("recovery snapshot"))?,
        );
        if Some(normal) != *expected || FileStamp::read(&self.path)? != Some(normal) {
            return Ok(None); // New normal generation: conservative stale retention.
        }
        let slot = manifest_slot(&manifest)?;
        let path = dir.join(format!("state-{slot}.tack"));
        if FileStamp::read(&path)? != Some(snapshot) {
            return Err(StorageError::Corrupt("recovery generation mismatch"));
        }
        let file = TackFile::open(path)?;
        if file.document.id() != id {
            return Err(StorageError::Invalid("recovery document identity"));
        }
        Ok(Some((file, generation)))
    }
    /// Explicit discard or successful exact-generation normal save only.
    /// Unknown entries and directories are never recursively removed.
    pub fn discard_recovery(&self, id: DocumentId) -> Result<()> {
        let _expected = self
            .expected
            .lock()
            .map_err(|_| StorageError::Invalid("board ownership poisoned"))?;
        let dir = self.recovery_directory();
        if !owned_directory(&dir, id, false)? {
            return Ok(());
        }
        cleanup_staging(&dir)?;
        for name in ["state.meta", "state-0.tack", "state-1.tack"] {
            let path = dir.join(name);
            match std::fs::symlink_metadata(&path) {
                Ok(m) if m.is_file() => std::fs::remove_file(path)?,
                Ok(_) => return Err(StorageError::Invalid("unsafe recovery discard target")),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e.into()),
            }
        }
        #[cfg(unix)]
        File::open(dir)?.sync_all()?;
        Ok(())
    }
}
