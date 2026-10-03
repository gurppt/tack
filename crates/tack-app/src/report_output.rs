//! Diagnostic exports must never truncate a document, source or other existing file.
use std::{fs, io::Write, path::Path};
use tack_assets::AssetError;
use tack_storage::{Result, StorageError};
fn normalized(path: &Path) -> Result<std::path::PathBuf> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let name = path
        .file_name()
        .ok_or(StorageError::Invalid("report/document output name"))?;
    Ok(parent.canonicalize()?.join(name))
}
pub(crate) fn preflight(path: Option<&Path>, document: Option<&Path>) -> Result<()> {
    let Some(path) = path else {
        return Ok(());
    };
    match fs::symlink_metadata(path) {
        Ok(_) => return Err(StorageError::Invalid("report output already exists")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
        Err(e) => return Err(e.into()),
    }
    if let Some(document) = document
        && normalized(path)? == normalized(document)?
    {
        return Err(StorageError::Invalid("report output aliases document"));
    }
    Ok(())
}
pub(crate) fn write_new(path: &Path, bytes: &[u8]) -> std::result::Result<(), AssetError> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    // Atomic exclusive creation also rejects aliases introduced after preflight.
    let mut file = options.open(path)?;
    if let Err(error) = file.write_all(bytes) {
        drop(file);
        let _ = fs::remove_file(path);
        return Err(error.into());
    }
    Ok(())
}
