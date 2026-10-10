//! Editable PNGs read once at startup, then packed into one bounded 128x80 RGBA atlas.
use crate::{
    actions::{Action, Tool},
    annotation_tool::StyleAction,
    selection_commands::Order,
    spatial_layout::Layout,
};
use std::{
    io::Read,
    path::{Path, PathBuf},
};
use tack_assets::{AssetError, Decoded};
pub const NAMES: [&str; 37] = [
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
    "link",
    "eraser",
    "align_left",
    "align_right",
    "align_top",
    "align_bottom",
    "arrange_in_grid",
    "distribute_horizontal",
    "distribute_vertical",
    "flip_horizontal",
    "flip_vertical",
    "bring_forward",
    "send_backward",
    "cycle_color",
    "cycle_image_sampling",
    "rotate_view_tool",
    "toggle_snap",
    "unlink",
    "mulot_icone",
];
pub const ATLAS_WIDTH: usize = 128;
pub const ATLAS_HEIGHT: usize = NAMES.len().div_ceil(8) * 16;
pub fn index(a: Action) -> usize {
    match a {
        Action::SelectTool(Tool::Pointer) | Action::TemporaryTool(Tool::Pointer) => 0,
        Action::SelectTool(Tool::Pan) | Action::TemporaryTool(Tool::Pan) => 1,
        Action::SelectTool(Tool::Text) | Action::TemporaryTool(Tool::Text) => 2,
        Action::SelectTool(Tool::Rectangle) | Action::TemporaryTool(Tool::Rectangle) => 3,
        Action::SelectTool(Tool::Line) | Action::TemporaryTool(Tool::Line) => 4,
        Action::SelectTool(Tool::Arrow) | Action::TemporaryTool(Tool::Arrow) => 5,
        Action::SelectTool(Tool::Scribble) | Action::TemporaryTool(Tool::Scribble) => 6,
        Action::CreateFrame => 7,
        Action::LinkToFrame => 18,
        Action::SelectTool(Tool::Eraser) | Action::TemporaryTool(Tool::Eraser) => 19,
        Action::DuplicateSelection => 8,
        Action::Undo => 9,
        Action::Redo => 10,
        Action::ShareBoard => 11,
        Action::JoinSharedBoard => 12,
        Action::Save => 13,
        Action::ToggleGrid => 14,
        Action::ToggleAnnotationSelectionLock => 17,
        Action::Layout(Layout::Left) => 20,
        Action::Layout(Layout::Right) => 21,
        Action::Layout(Layout::Top) => 22,
        Action::Layout(Layout::Bottom) => 23,
        Action::Layout(Layout::Grid) => 24,
        Action::Layout(Layout::DistributeHorizontal) => 25,
        Action::Layout(Layout::DistributeVertical) => 26,
        Action::FlipHorizontal => 27,
        Action::FlipVertical => 28,
        Action::Order(Order::Forward) => 29,
        Action::Order(Order::Backward) => 30,
        Action::AnnotationStyle(StyleAction::Color) => 31,
        Action::CycleFiltering => 32,
        Action::SelectTool(Tool::RotateView) | Action::TemporaryTool(Tool::RotateView) => 33,
        Action::ToggleSnapping => 34,
        Action::UnlinkFromFrame => 35,
        Action::SelectTool(Tool::Mouse) | Action::TemporaryTool(Tool::Mouse) => 36,
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
    let mut rgba = vec![0; ATLAS_WIDTH * ATLAS_HEIGHT * 4];
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
        width: ATLAS_WIDTH as u32,
        height: ATLAS_HEIGHT as u32,
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
