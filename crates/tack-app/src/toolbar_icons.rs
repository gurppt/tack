//! Editable PNGs read once at startup, then packed into one 128x32 RGBA atlas.
use crate::actions::{Action, Tool};
use std::{
    io::Read,
    path::{Path, PathBuf},
};
use tack_assets::{AssetError, Decoded};
pub const NAMES: [&str; 18] = [
    "pointer",
    "pan",
    "text",
    "rectangle",
    "line",
    "arrow",
    "scribble",
    "frame",
    "duplicate",
    "undo",
    "redo",
    "share",
    "join",
    "save",
    "grid",
    "placeholder",
    "annotation_lock_on",
    "annotation_lock_off",
];
pub fn index(a: Action) -> usize {
    match a {
        Action::SelectTool(Tool::Pointer) => 0,
        Action::SelectTool(Tool::Pan) => 1,
        Action::SelectTool(Tool::Text) => 2,
        Action::SelectTool(Tool::Rectangle) => 3,
        Action::SelectTool(Tool::Line) => 4,
        Action::SelectTool(Tool::Arrow) => 5,
        Action::SelectTool(Tool::Scribble) => 6,
        Action::CreateFrame => 7,
        Action::DuplicateSelection => 8,
        Action::Undo => 9,
        Action::Redo => 10,
        Action::ShareBoard => 11,
        Action::JoinSharedBoard => 12,
        Action::Save => 13,
        Action::ToggleGrid => 14,
        Action::ToggleAnnotationSelectionLock => 17,
        _ => 15,
    }
}
/// Only authored symbols; the fallback placeholder is not an action preview.
pub fn actual_index(a: Action) -> Option<usize> {
    let i = index(a);
    (i != 15).then_some(i)
}
pub fn root() -> PathBuf {
    if let Some(p) = std::env::var_os("TACK_ICON_DIR") {
        return p.into();
    }
    if let Ok(p) = std::env::current_exe()
        && let Some(parent) = p.parent()
    {
        let installed = parent.join("gfx/icons");
        if installed.is_dir() {
            return installed;
        }
    }
    let development = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../gfx/icons");
    if development.is_dir() {
        return development;
    }
    PathBuf::from("gfx/icons")
}
pub fn load(path: &Path) -> Decoded {
    let mut rgba = vec![0; 128 * 64 * 4];
    for (i, name) in NAMES.iter().enumerate() {
        let pixels = match read(&path.join(format!("{name}.png"))) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("[tack/icons] {name}: {e}; placeholder");
                placeholder()
            }
        };
        for y in 0..16 {
            let dst = ((i / 8 * 16 + y) * 128 + i % 8 * 16) * 4;
            rgba[dst..dst + 64].copy_from_slice(&pixels[y * 64..y * 64 + 64]);
        }
    }
    Decoded {
        width: 128,
        height: 64,
        rgba,
    }
}
pub fn read(path: &Path) -> Result<Vec<u8>, AssetError> {
    let file = std::fs::File::open(path)?;
    if !file.metadata()?.is_file() || file.metadata()?.len() > 16 * 1024 {
        return Err("icon must be a PNG <=16 KiB".into());
    }
    let mut bytes = Vec::new();
    file.take(16 * 1024 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > 16 * 1024 {
        return Err("icon size bound".into());
    }
    let image = tack_assets::decode_ui_png(&bytes)?;
    if image.width != 16 || image.height != 16 {
        return Err("icon must be exactly 16x16".into());
    }
    if image.rgba.chunks_exact(4).any(|p| !matches!(p[3], 0 | 255)) {
        return Err("icon requires hard alpha (0 or 255)".into());
    }
    Ok(image.rgba)
}
fn placeholder() -> Vec<u8> {
    let mut p = vec![0; 1024];
    for y in 3..13 {
        for x in 3..13 {
            if x == 3 || x == 12 || y == 3 || y == 12 || x == y {
                p[(y * 16 + x) * 4..(y * 16 + x) * 4 + 4].copy_from_slice(&[255, 255, 255, 255]);
            }
        }
    }
    p
}

/// Two artworks, one semantic action. All existing cell state chrome is retained.
pub fn state_index(action: Action, annotation_locked: bool) -> usize {
    if action == Action::ToggleAnnotationSelectionLock {
        if annotation_locked { 16 } else { 17 }
    } else {
        index(action)
    }
}
