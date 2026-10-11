//! Eighteen fixed pixel cursors; tiny static hotspots, loaded once and cached.
use winit::{
    event_loop::ActiveEventLoop,
    window::{CursorIcon, CustomCursor},
};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(usize)]
pub enum Kind {
    #[default]
    Pointer,
    HandOpen,
    HandClosed,
    Crosshair,
    Draw,
    Eraser,
    Text,
    Move,
    ResizeNwse,
    ResizeNesw,
    ResizeNs,
    ResizeEw,
    Rotate,
    Crop,
    LinkOpen,
    LinkClosed,
    Forbidden,
    Mouse,
}
pub const TABLE: [(&str, u16, u16); 18] = [
    ("cursor_pointer", 1, 1),
    ("cursor_hand_open", 7, 7),
    ("cursor_hand_closed", 7, 7),
    ("cursor_crosshair", 7, 7),
    ("cursor_draw", 1, 14),
    ("cursor_eraser", 4, 11),
    ("cursor_text", 7, 7),
    ("cursor_move", 7, 7),
    ("cursor_resize_nwse", 7, 7),
    ("cursor_resize_nesw", 7, 7),
    ("cursor_resize_ns", 7, 7),
    ("cursor_resize_ew", 7, 7),
    ("cursor_rotate", 7, 7),
    ("cursor_crop", 7, 7),
    ("cursor_link_open", 7, 7),
    ("cursor_link_closed", 7, 7),
    ("cursor_forbidden", 7, 7),
    ("minimulot", 7, 7),
];
#[derive(Default)]
pub struct Cache {
    handles: Vec<Option<CustomCursor>>,
}
impl Cache {
    pub fn load(event_loop: &ActiveEventLoop) -> Self {
        let installed = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.join("gfx/cursors")));
        let directory = installed.filter(|p| p.is_dir()).unwrap_or_else(|| {
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../gfx/cursors")
        });
        Self {
            handles: TABLE
                .iter()
                .map(|(name, x, y)| {
                    crate::toolbar_icons::read(&directory.join(format!("{name}.png")))
                        .ok()
                        .and_then(|rgba| {
                            let scale = if *name == "minimulot" { 2 } else { 1 };
                            CustomCursor::from_rgba(
                                scaled_rgba(rgba, scale),
                                16 * scale,
                                16 * scale,
                                *x * scale,
                                *y * scale,
                            )
                            .ok()
                        })
                        .map(|source| event_loop.create_custom_cursor(source))
                })
                .collect(),
        }
    }
    pub fn set(&self, window: &winit::window::Window, kind: Kind) {
        if let Some(Some(handle)) = self.handles.get(kind as usize) {
            window.set_cursor(handle.clone());
        } else {
            window.set_cursor(kind.fallback());
        }
    }
}
impl Kind {
    pub fn fallback(self) -> CursorIcon {
        match self {
            Self::Pointer => CursorIcon::Default,
            Self::HandOpen => CursorIcon::Grab,
            Self::HandClosed => CursorIcon::Grabbing,
            Self::Text => CursorIcon::Text,
            Self::Move => CursorIcon::Move,
            Self::ResizeNwse => CursorIcon::NwseResize,
            Self::ResizeNesw => CursorIcon::NeswResize,
            Self::ResizeNs => CursorIcon::NsResize,
            Self::ResizeEw => CursorIcon::EwResize,
            Self::Forbidden => CursorIcon::NotAllowed,
            _ => CursorIcon::Crosshair,
        }
    }
}

// Startup-only nearest replication: preserve PNG bytes and every hard pixel.
fn scaled_rgba(rgba: Vec<u8>, scale: u16) -> Vec<u8> {
    if scale == 1 {
        return rgba;
    }
    let scale = usize::from(scale);
    let edge = 16 * scale;
    let mut out = vec![0; edge * edge * 4];
    for y in 0..edge {
        for x in 0..edge {
            let source = ((y / scale) * 16 + x / scale) * 4;
            let target = (y * edge + x) * 4;
            out[target..target + 4].copy_from_slice(&rgba[source..source + 4]);
        }
    }
    out
}
