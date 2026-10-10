//! Bounded shared-copy companion metadata; authority remains the existing server.
use crate::shared_address::SharedAddress;
use serde::{Deserialize, Serialize};
use std::{
    io::Read,
    path::{Path, PathBuf},
};
use tack_assets::AssetError;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Descriptor {
    pub version: u8,
    pub board: String,
    pub owner: Option<String>,
    pub invite: String,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice {
    None,
    Host,
    Remote,
    Start,
    Stop,
    Copy,
    Advanced,
    Offline,
    Online,
    Reconnect,
    SaveLocal,
}
pub fn sidecar(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(".sharing.json");
    PathBuf::from(name)
}
pub fn proposed(path: &Path, untitled: bool) -> PathBuf {
    if untitled {
        return PathBuf::from("Untitled-shared.tack");
    }
    let mut name = path.file_stem().unwrap_or_default().to_os_string();
    name.push("-shared.tack");
    path.with_file_name(name)
}
impl Descriptor {
    pub fn address(&self) -> Result<SharedAddress, AssetError> {
        if self.version != 1 {
            return Err("unsupported shared-copy metadata".into());
        }
        let a = SharedAddress::parse(&self.invite)?;
        if a.board.to_string() != self.board {
            return Err("shared-copy identity mismatch".into());
        }
        if let Some(owner) = &self.owner {
            let _ = tack_shared::WireId::parse(owner)?;
        }
        Ok(a)
    }
    pub fn read(path: &Path, id: tack_core::DocumentId) -> Result<Option<Self>, AssetError> {
        let file = match std::fs::File::open(sidecar(path)) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        if !file.metadata()?.is_file() || file.metadata()?.len() > 4096 {
            return Err("shared-copy metadata exceeds bound".into());
        }
        let mut bytes = Vec::new();
        file.take(4097).read_to_end(&mut bytes)?;
        if bytes.len() > 4096 {
            return Err("shared-copy metadata exceeds bound".into());
        }
        let d: Self = serde_json::from_slice(&bytes)?;
        if d.address()?.board.value() != id.value() {
            return Err("shared-copy metadata belongs to a different board".into());
        }
        Ok(Some(d))
    }
    pub fn write_new(&self, path: &Path) -> Result<(), AssetError> {
        use std::io::Write;
        self.address()?;
        let bytes = serde_json::to_vec_pretty(self)?;
        if bytes.len() > 4096 {
            return Err("shared-copy metadata exceeds bound".into());
        }
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(sidecar(path))?;
        f.write_all(&bytes)?;
        f.sync_all()?;
        Ok(())
    }
}
pub fn host_identity(root: &Path) -> Result<String, AssetError> {
    use std::io::Write;
    tack_storage::create_private_directory(root, true)?;
    let path = root.join("hosting-identity");
    let _guard = tack_storage::lock_sidecar(&root.join("hosting-identity.lock"))?;
    if path.exists() {
        let file = std::fs::File::open(path)?;
        let mut text = String::new();
        file.take(65).read_to_string(&mut text)?;
        let _ = tack_shared::WireId::parse(&text)?;
        return Ok(text);
    }
    let id = tack_storage::new_document_id()?.value();
    let text = tack_shared::WireId::new(id)?.to_string();
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    f.write_all(text.as_bytes())?;
    f.sync_all()?;
    Ok(text)
}
pub fn owns(root: &Path, d: &Descriptor) -> bool {
    let Ok(file) = std::fs::File::open(root.join("hosting-identity")) else {
        return false;
    };
    let mut text = String::new();
    file.take(65).read_to_string(&mut text).is_ok()
        && tack_shared::WireId::parse(&text).is_ok()
        && d.owner.as_ref() == Some(&text)
}
