use crate::{MAX_AUTHORITY_BYTES, Result, authority::State};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::Path,
};

const MAGIC: &[u8; 8] = b"TACKAU01";
pub(crate) fn read(path: &Path) -> Result<State> {
    let mut file = File::open(path).map_err(|e| e.to_string())?;
    let size = file.metadata().map_err(|e| e.to_string())?.len();
    if size < 48 || size > MAX_AUTHORITY_BYTES as u64 + 48 {
        return Err("authority file size".into());
    }
    let mut header = [0; 48];
    file.read_exact(&mut header).map_err(|e| e.to_string())?;
    if &header[..8] != MAGIC {
        return Err("authority magic".into());
    }
    let length = u64::from_le_bytes(header[8..16].try_into().map_err(|_| "authority length")?);
    if length != size - 48 {
        return Err("authority truncated or trailing bytes".into());
    }
    let mut bytes = vec![0; length as usize];
    file.read_exact(&mut bytes).map_err(|e| e.to_string())?;
    if Sha256::digest(&bytes).as_slice() != &header[16..48] {
        return Err("authority checksum".into());
    }
    serde_json::from_slice(&bytes).map_err(|e| e.to_string())
}
pub(crate) fn publish(path: &Path, state: &State) -> Result<()> {
    let bytes = serde_json::to_vec(state).map_err(|e| e.to_string())?;
    if bytes.len() > MAX_AUTHORITY_BYTES {
        return Err("authority metadata bound".into());
    }
    // Parse/domain-validate before the atomic rename changes durable authority.
    let decoded: State = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    decoded.document.to_document().map_err(|e| e.to_string())?;
    let temp = path.with_extension("pending");
    let result = (|| {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temp).map_err(|e| e.to_string())?;
        file.write_all(MAGIC).map_err(|e| e.to_string())?;
        file.write_all(&(bytes.len() as u64).to_le_bytes())
            .map_err(|e| e.to_string())?;
        file.write_all(&Sha256::digest(&bytes))
            .map_err(|e| e.to_string())?;
        file.write_all(&bytes).map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        drop(file);
        fs::rename(&temp, path).map_err(|e| e.to_string())?;
        // Windows cannot open a directory for sync; its publication remains atomic.
        #[cfg(unix)]
        sync_directory(path.parent().ok_or("authority parent")?)
            .map_err(|e| format!("authority published but directory sync failed: {e}"))?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

#[cfg(unix)]
pub(crate) fn sync_directory(path: &Path) -> std::io::Result<()> {
    #[cfg(test)]
    if FAIL_SYNC.with(|v| v.get()) {
        return Err(std::io::Error::other("injected directory sync failure"));
    }
    File::open(path)?.sync_all()
}
#[cfg(test)]
thread_local! {pub(crate) static FAIL_SYNC:std::cell::Cell<bool>=const {std::cell::Cell::new(false)};}
