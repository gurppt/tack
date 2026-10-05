//! Bounded readable local preferences/keymap. Filesystem work belongs to workers/startup.
use crate::{
    actions::Action,
    bindings::{Binding, Keymap, ModifierMatch, Trigger},
    input::{Modifiers, PhysicalControl},
};
use serde::{Deserialize, Serialize};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};
use tack_assets::AssetError;
const MAX_PROFILE_BYTES: usize = 256 * 1024;
pub const MAX_RECENT: usize = 16;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BindingRecord {
    pub action: String,
    pub control: PhysicalControl,
    pub modifiers: u8,
    pub modifier_match: String,
    pub trigger: String,
}
impl BindingRecord {
    pub fn from_binding(b: &Binding) -> Self {
        let (modifiers, modifier_match) = match b.modifiers {
            ModifierMatch::Any => (0, "Any"),
            ModifierMatch::Exact(m) => (m.mask(), "Exact"),
            ModifierMatch::Contains(m) => (m.mask(), "Contains"),
        };
        Self {
            action: b.action.id(),
            control: b.control,
            modifiers,
            modifier_match: modifier_match.into(),
            trigger: format!("{:?}", b.trigger),
        }
    }
    pub fn binding(&self) -> Result<Binding, AssetError> {
        let action = Action::from_id(&self.action).ok_or("unknown keymap action")?;
        let mask = Modifiers::checked_mask(self.modifiers).ok_or("invalid modifier mask")?;
        let modifiers = match self.modifier_match.as_str() {
            "Any" if self.modifiers == 0 => ModifierMatch::Any,
            "Exact" => ModifierMatch::Exact(mask),
            "Contains" => ModifierMatch::Contains(mask),
            _ => return Err("unknown modifier match".into()),
        };
        let trigger = match self.trigger.as_str() {
            "Press" => Trigger::Press,
            "Release" => Trigger::Release,
            "Hold" => Trigger::Hold,
            "Wheel" => Trigger::Wheel,
            _ => return Err("unknown keymap trigger".into()),
        };
        Ok(Binding {
            action,
            control: self.control,
            modifiers,
            trigger,
        })
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecentPath {
    encoding: u8,
    bytes: Vec<u8>,
}
impl RecentPath {
    pub fn native(path: &Path) -> Result<Self, AssetError> {
        let p = tack_core::LinkedPath::native(path)?;
        Ok(Self {
            encoding: match p.platform() {
                tack_core::PathPlatform::Unix => 1,
                tack_core::PathPlatform::Windows => 2,
            },
            bytes: p.bytes().to_vec(),
        })
    }
    fn descriptor(&self) -> Result<tack_core::LinkedPath, AssetError> {
        use tack_core::PathPlatform;
        let encoding = match self.encoding {
            1 => PathPlatform::Unix,
            2 => PathPlatform::Windows,
            _ => return Err("unknown recent path encoding".into()),
        };
        Ok(tack_core::LinkedPath::encoded(encoding, true, &self.bytes)?)
    }
    pub fn path(&self) -> Result<PathBuf, AssetError> {
        self.descriptor()?
            .to_native()
            .ok_or_else(|| "recent board belongs to another platform".into())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Preferences {
    pub version: u32,
    pub grid: bool,
    pub sampling: String,
    pub embedded_import: bool,
    /// Zero follows native DPI; positive values are integer logical presentation.
    pub ui_scale: u8,
    pub handle_size: u8,
    pub hit_radius: u8,
    pub keymap: Vec<BindingRecord>,
    pub recent: Vec<RecentPath>,
}
impl Preferences {
    pub fn defaults() -> Result<Self, AssetError> {
        Ok(Self {
            version: 1,
            grid: false,
            sampling: "Smooth".into(),
            embedded_import: true,
            ui_scale: 0,
            handle_size: 7,
            hit_radius: 9,
            keymap: crate::image_input::product_keymap()?
                .bindings()
                .iter()
                .map(BindingRecord::from_binding)
                .collect(),
            recent: Vec::new(),
        })
    }
    pub fn keymap(&self) -> Result<Keymap, AssetError> {
        if self.version != 1
            || self.keymap.len() > crate::bindings::MAX_BINDINGS
            || self.recent.len() > MAX_RECENT
            || self.ui_scale > 4
            || !(3..=21).contains(&self.handle_size)
            || !(5..=32).contains(&self.hit_radius)
            || !matches!(self.sampling.as_str(), "Smooth" | "Nearest")
        {
            return Err("unsupported or out-of-bounds preferences".into());
        }
        for recent in &self.recent {
            if recent.bytes.len() > tack_core::MAX_SOURCE_PATH_BYTES {
                return Err("recent path too long".into());
            }
            // Foreign recent paths remain descriptors, not silently rewritten.
            let _ = recent.descriptor()?;
        }
        let mut keymap = Keymap::default();
        for record in &self.keymap {
            keymap.bind(record.binding()?)?;
        }
        Ok(keymap)
    }
    pub fn remember(&mut self, path: &Path) -> Result<(), AssetError> {
        let path = std::path::absolute(path)?;
        self.recent.retain(|p| !p.path().is_ok_and(|p| p == path));
        self.recent.insert(0, RecentPath::native(&path)?);
        self.recent.truncate(MAX_RECENT);
        Ok(())
    }
}
pub fn profile_root() -> Result<PathBuf, AssetError> {
    if let Some(root) = std::env::var_os("TACK_PROFILE_DIR") {
        return Ok(root.into());
    }
    #[cfg(windows)]
    let base = std::env::var_os("LOCALAPPDATA").ok_or("LOCALAPPDATA unavailable")?;
    #[cfg(not(windows))]
    let base = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".local/state")))
        .ok_or("home directory unavailable")?
        .into_os_string();
    Ok(PathBuf::from(base).join("tack"))
}
pub fn read(path: &Path) -> Result<Preferences, AssetError> {
    let mut bytes = Vec::new();
    if !std::fs::symlink_metadata(path)?.is_file() {
        return Err("preferences must be a regular file".into());
    }
    let file = File::open(path)?;
    if !file.metadata()?.is_file() || file.metadata()?.len() > MAX_PROFILE_BYTES as u64 {
        return Err("preferences file exceeds bound".into());
    }
    file.take(MAX_PROFILE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_PROFILE_BYTES {
        return Err("preferences file exceeds bound".into());
    }
    let profile: Preferences = serde_json::from_slice(&bytes)?;
    profile.keymap()?;
    Ok(profile)
}
/// Atomic explicit export. Existing files require the caller's deliberate target choice.
pub fn write(path: &Path, profile: &Preferences) -> Result<(), AssetError> {
    let mut leaf = path
        .file_name()
        .ok_or("preferences filename")?
        .to_os_string();
    leaf.push(".tack-lock");
    let _lock = tack_storage::lock_sidecar(&path.with_file_name(leaf))?;
    write_locked(path, profile)
}
fn write_locked(path: &Path, profile: &Preferences) -> Result<(), AssetError> {
    profile.keymap()?;
    let bytes = serde_json::to_vec_pretty(profile)?;
    if bytes.len() > MAX_PROFILE_BYTES {
        return Err("preferences file exceeds bound".into());
    }
    let parent = path.parent().ok_or("preferences parent")?;
    if std::fs::symlink_metadata(path).is_ok_and(|m| !m.is_file()) {
        return Err("preferences target is not a regular file".into());
    }
    let temporary = parent.join(format!(
        ".tack-profile-{:032x}.tmp",
        tack_storage::new_document_id()?.value()
    ));
    let result = (|| -> Result<(), AssetError> {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temporary)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&temporary, path)?;
        #[cfg(unix)]
        File::open(parent)?.sync_all()?;
        Ok(())
    })();
    let _ = std::fs::remove_file(temporary);
    result
}

pub fn fingerprint(path: &Path) -> Result<Option<u32>, AssetError> {
    let mut bytes = Vec::new();
    match std::fs::symlink_metadata(path) {
        Ok(m) if !m.is_file() => return Err("preferences must be a regular file".into()),
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    }
    let file = match File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    file.take(MAX_PROFILE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_PROFILE_BYTES {
        return Err("preferences file exceeds bound".into());
    }
    Ok(Some(crc32fast::hash(&bytes)))
}
/// Independent instances merge recent paths and refuse conflicting settings edits.
pub fn save_profile(
    root: &Path,
    profile: &Preferences,
    base: Option<u32>,
    settings_changed: bool,
    board: Option<&Path>,
) -> Result<u32, AssetError> {
    tack_storage::create_private_directory(root, true)?;
    let path = root.join("preferences.json");
    let _lock = tack_storage::lock_sidecar(&root.join("preferences.json.tack-lock"))?;
    let current = fingerprint(&path)?;
    if settings_changed && current != base {
        return Err("preferences changed in another instance; reload before saving".into());
    }
    let mut next = if settings_changed || current.is_none() {
        profile.clone()
    } else {
        read(&path)?
    };
    if settings_changed && current.is_some() {
        for previous in read(&path)?.recent {
            if !next.recent.contains(&previous) && next.recent.len() < MAX_RECENT {
                next.recent.push(previous);
            }
        }
    }
    if let Some(board) = board {
        next.remember(board)?;
    }
    write_locked(&path, &next)?;
    fingerprint(&path)?.ok_or_else(|| "preferences publication missing".into())
}
