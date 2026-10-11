//! Versioned portable environment: no board data, cache, credentials or machine paths.
use crate::preferences::{self, Preferences};
use serde::{Deserialize, Serialize};
use std::{io::Read, path::Path};
use tack_assets::AssetError;
#[derive(Serialize, Deserialize)]
struct File {
    format: String,
    version: u32,
    preferences: Preferences,
}
pub fn portable(profile: &Preferences) -> Result<Preferences, AssetError> {
    let mut p = profile.clone();
    p.normalize_sets()?;
    p.keymap.retain(|r| r.action != "ResetAspectRatio");
    p.toolbar.floating = [48, 48];
    p.toolbar.offset = 0;
    p.toolbar.offset_set = false;
    p.recent.clear();
    p.last_board_directory = None;
    p.local_views.clear();
    p.keyset.path = None;
    p.keymap()?;
    Ok(p)
}
pub fn export(path: &Path, profile: &Preferences) -> Result<(), AssetError> {
    let target = crate::file_names::with_suffix(path, "tackprofile");
    let file = File {
        format: "tackprofile".into(),
        version: 1,
        preferences: portable(profile)?,
    };
    preferences::export_bytes(&target, path, &serde_json::to_vec_pretty(&file)?)
}
pub fn read(path: &Path) -> Result<Preferences, AssetError> {
    if !std::fs::symlink_metadata(path)?.is_file() {
        return Err("Profile must be a regular file".into());
    }
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(preferences::MAX_PROFILE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > preferences::MAX_PROFILE_BYTES {
        return Err("Profile exceeds 256 KiB limit".into());
    }
    let file: File = serde_json::from_slice(&bytes)
        .map_err(|e| format!("Invalid or incomplete Tack profile: {e}"))?;
    if file.format != "tackprofile" || file.version != 1 {
        return Err("Unsupported Tack profile format/version".into());
    }
    portable(&file.preferences)
}
