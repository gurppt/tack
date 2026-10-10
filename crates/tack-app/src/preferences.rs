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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Keyset {
    pub name: String,
    pub path: Option<RecentPath>,
    pub dirty: bool,
}
impl Default for Keyset {
    fn default() -> Self {
        Self {
            name: "Default".into(),
            path: None,
            dirty: false,
        }
    }
}
impl Keyset {
    pub fn loaded(path: &Path) -> Result<Self, AssetError> {
        let name = path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .chars()
            .take(128)
            .collect();
        Ok(Self {
            name,
            path: Some(RecentPath::native(path)?),
            dirty: false,
        })
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Preferences {
    #[serde(default)]
    pub keyset: Keyset,
    #[serde(default)]
    pub local_views: Vec<crate::camera_slots::View>,
    pub version: u32,
    #[serde(default)]
    pub toolbar: crate::toolbar::Config,
    #[serde(default = "status_default")]
    pub status_bar: bool,
    pub grid: bool,
    pub sampling: String,
    pub embedded_import: bool,
    /// Zero follows native DPI; positive values are integer logical presentation.
    pub ui_scale: u8,
    #[serde(default)]
    pub theme: crate::ui_theme::Theme,
    #[serde(default = "frame_title_default")]
    pub frame_title_scale: u8,
    /// Distinguishes the former 2x default from deliberate post-2A6 overrides.
    #[serde(default)]
    pub frame_title_default_version: u8,
    pub handle_size: u8,
    pub hit_radius: u8,
    pub keymap: Vec<BindingRecord>,
    pub recent: Vec<RecentPath>,
    #[serde(default)]
    pub last_board_directory: Option<RecentPath>,
}
impl Preferences {
    pub fn defaults() -> Result<Self, AssetError> {
        Ok(Self {
            version: 2,
            keyset: Keyset::default(),
            local_views: Vec::new(),
            toolbar: crate::toolbar::Config::default(),
            status_bar: true,
            grid: false,
            sampling: "Smooth".into(),
            embedded_import: true,
            ui_scale: 0,
            theme: crate::ui_theme::Theme::default(),
            frame_title_scale: 1,
            frame_title_default_version: 1,
            handle_size: 7,
            hit_radius: 9,
            keymap: crate::image_input::product_keymap()?
                .bindings()
                .iter()
                .map(BindingRecord::from_binding)
                .collect(),
            recent: Vec::new(),
            last_board_directory: None,
        })
    }
    pub fn keymap(&self) -> Result<Keymap, AssetError> {
        if self.keyset.name.len() > 512 || self.keyset.name.chars().any(char::is_control) {
            return Err("invalid keyset name".into());
        }
        if let Some(path) = &self.keyset.path {
            path.descriptor()?;
        }
        if !matches!(self.version, 1 | 2)
            || self.keymap.len() > crate::bindings::MAX_BINDINGS
            || self.recent.len() > MAX_RECENT
            || self.ui_scale > 4
            || !(1..=3).contains(&self.frame_title_scale)
            || !(3..=21).contains(&self.handle_size)
            || !(5..=32).contains(&self.hit_radius)
            || !matches!(self.sampling.as_str(), "Smooth" | "Nearest")
        {
            return Err("unsupported or out-of-bounds preferences".into());
        }
        crate::camera_slots::validate(&self.local_views)?;
        self.toolbar.clone().normalize()?;
        for recent in self.recent.iter().chain(self.last_board_directory.iter()) {
            if recent.bytes.len() > tack_core::MAX_SOURCE_PATH_BYTES {
                return Err("recent path too long".into());
            }
            // Foreign recent paths remain descriptors, not silently rewritten.
            let _ = recent.descriptor()?;
        }
        let mut keymap = Keymap::default();
        for record in &self.keymap {
            let mut binding = record.binding()?;
            if self.version == 1
                && let PhysicalControl::Key(winit::keyboard::PhysicalKey::Code(code)) =
                    binding.control
                && !matches!(
                    code,
                    winit::keyboard::KeyCode::NumpadAdd | winit::keyboard::KeyCode::NumpadSubtract
                )
            {
                binding.control = crate::input::LogicalKey::from_legacy(code).map(PhysicalControl::LogicalKey).ok_or("unsupported version-1 key; export with an explicit physical version-2 binding")?;
            }
            keymap.bind(binding)?;
        }
        Ok(keymap)
    }
    pub fn remember_board_directory(&mut self, path: &Path) -> Result<(), AssetError> {
        let path = std::path::absolute(path)?;
        let parent = path.parent().ok_or("board directory unavailable")?;
        self.last_board_directory = Some(RecentPath::native(parent)?);
        Ok(())
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
    let mut profile: Preferences = serde_json::from_slice(&bytes)?;
    profile.toolbar.normalize()?;
    if profile.frame_title_default_version == 0 {
        if profile.frame_title_scale == 2 {
            profile.frame_title_scale = 1;
        }
        profile.frame_title_default_version = 1;
    }
    let keymap = profile.keymap()?;
    if profile.version == 1 {
        profile.version = 2;
        profile.keymap = keymap
            .bindings()
            .iter()
            .map(BindingRecord::from_binding)
            .collect();
    }
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
/// A suffix-added target was not confirmed by the native picker: it must be unused.
pub fn export_keymap(path: &Path, profile: &Preferences) -> Result<(), AssetError> {
    let target = crate::file_names::keymap(path);
    if target == path {
        return write_keymap(path, profile);
    }
    let mut leaf = target.file_name().ok_or("keymap filename")?.to_os_string();
    leaf.push(".tack-lock");
    let _lock = tack_storage::lock_sidecar(&target.with_file_name(leaf))?;
    match std::fs::symlink_metadata(&target) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
        Ok(_) => return Err("keymap filename with .tackey already exists; select that full filename to confirm replacement".into()),
    }
    write_keymap_locked(&target, profile)
}
fn write_locked(path: &Path, profile: &Preferences) -> Result<(), AssetError> {
    profile.keymap()?;
    let bytes = serde_json::to_vec_pretty(profile)?;
    write_bytes_locked(path, &bytes)
}
fn write_bytes_locked(path: &Path, bytes: &[u8]) -> Result<(), AssetError> {
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
        file.write_all(bytes)?;
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
        next.remember_board_directory(board)?;
    }
    write_locked(&path, &next)?;
    fingerprint(&path)?.ok_or_else(|| "preferences publication missing".into())
}

fn status_default() -> bool {
    true
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct KeymapFile {
    version: u32,
    bindings: Vec<BindingRecord>,
}
fn write_keymap_locked(path: &Path, profile: &Preferences) -> Result<(), AssetError> {
    profile.keymap()?;
    write_bytes_locked(
        path,
        &serde_json::to_vec_pretty(&KeymapFile {
            version: 1,
            bindings: profile.keymap.clone(),
        })?,
    )
}
fn write_keymap(path: &Path, profile: &Preferences) -> Result<(), AssetError> {
    let mut leaf = path.file_name().ok_or("keymap filename")?.to_os_string();
    leaf.push(".tack-lock");
    let _lock = tack_storage::lock_sidecar(&path.with_file_name(leaf))?;
    write_keymap_locked(path, profile)
}
pub fn read_keymap(path: &Path) -> Result<Preferences, AssetError> {
    let mut bytes = Vec::new();
    File::open(path)?
        .take((MAX_PROFILE_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_PROFILE_BYTES {
        return Err("keymap file exceeds bound".into());
    }
    if let Ok(file) = serde_json::from_slice::<KeymapFile>(&bytes) {
        if file.version != 1 {
            return Err("unsupported keymap version".into());
        }
        let mut profile = Preferences::defaults()?;
        profile.keymap = file.bindings;
        profile.keyset = Keyset::loaded(path)?;
        profile.keymap()?;
        Ok(profile)
    } else {
        let mut profile = read(path)?;
        profile.keyset = Keyset::loaded(path)?;
        Ok(profile)
    } // Legacy combined preferences/keymap exports remain readable.
}
pub fn export_preferences(path: &Path, profile: &Preferences) -> Result<(), AssetError> {
    profile.keymap()?;
    let mut value = serde_json::to_value(profile)?;
    if let Some(object) = value.as_object_mut() {
        object.remove("keymap");
        object.remove("keyset");
        object.remove("recent");
        object.remove("last_board_directory");
        object.remove("local_views");
    }
    let mut leaf = path
        .file_name()
        .ok_or("preferences filename")?
        .to_os_string();
    leaf.push(".tack-lock");
    let _lock = tack_storage::lock_sidecar(&path.with_file_name(leaf))?;
    write_bytes_locked(path, &serde_json::to_vec_pretty(&value)?)
}

fn frame_title_default() -> u8 {
    1
}

/// Merge only settings changed by this window. Recent-board writes are not settings conflicts.
pub fn merge_settings(
    base: &Preferences,
    desired: &Preferences,
    current: &Preferences,
) -> Result<Preferences, AssetError> {
    let mut next = current.clone();
    macro_rules! field { ($($name:ident),*) => { $(
        if desired.$name != base.$name {
            if current.$name != base.$name && current.$name != desired.$name {
                return Err(concat!("Preference changed in another window: ", stringify!($name)).into());
            }
            next.$name = desired.$name.clone();
        }
    )* }; }
    field!(
        toolbar,
        status_bar,
        grid,
        sampling,
        embedded_import,
        ui_scale,
        theme,
        frame_title_scale,
        handle_size,
        hit_radius
    );
    if desired.keymap != base.keymap || desired.keyset != base.keyset {
        let current_changed = current.keymap != base.keymap || current.keyset != base.keyset;
        if current_changed && (current.keymap != desired.keymap || current.keyset != desired.keyset)
        {
            return Err("Keyset changed in another window".into());
        }
        next.keymap = desired.keymap.clone();
        next.keyset = desired.keyset.clone();
    }
    next.local_views = crate::camera_slots::merge(
        &base.local_views,
        &desired.local_views,
        &current.local_views,
    )?;
    next.keymap()?;
    Ok(next)
}
/// Preserve edits made while the bounded worker published earlier settings.
pub fn adopt_saved(local: &mut Preferences, submitted: &Preferences, saved: &Preferences) {
    macro_rules! field { ($($name:ident),*) => { $(
        if local.$name == submitted.$name { local.$name = saved.$name.clone(); }
    )* }; }
    field!(
        toolbar,
        status_bar,
        grid,
        sampling,
        embedded_import,
        ui_scale,
        theme,
        frame_title_scale,
        handle_size,
        hit_radius,
        local_views,
        recent,
        last_board_directory
    );
    if local.keymap == submitted.keymap && local.keyset == submitted.keyset {
        local.keymap = saved.keymap.clone();
        local.keyset = saved.keyset.clone();
    }
}
pub struct SavedProfile {
    pub fingerprint: u32,
    pub submitted: Preferences,
    pub saved: Preferences,
}
pub fn save_profile_merged(
    root: &Path,
    profile: Preferences,
    baseline: &Preferences,
    board: Option<&Path>,
) -> Result<SavedProfile, AssetError> {
    tack_storage::create_private_directory(root, true)?;
    let path = root.join("preferences.json");
    let _lock = tack_storage::lock_sidecar(&root.join("preferences.json.tack-lock"))?;
    let current = if fingerprint(&path)?.is_some() {
        read(&path)?
    } else {
        baseline.clone()
    };
    let mut saved = merge_settings(baseline, &profile, &current)?;
    if let Some(board) = board {
        saved.remember(board)?;
        saved.remember_board_directory(board)?;
    }
    write_locked(&path, &saved)?;
    Ok(SavedProfile {
        fingerprint: fingerprint(&path)?.ok_or("preferences publication missing")?,
        submitted: profile,
        saved,
    })
}

/// A completed export acknowledges its snapshot, without replacing newer edits.
pub fn acknowledge_keyset_export(current: &mut Preferences, exported: &Preferences) {
    current.keyset = exported.keyset.clone();
    current.keyset.dirty = current.keymap != exported.keymap;
}
