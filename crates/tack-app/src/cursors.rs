//! Seventeen fixed pixel cursors; tiny static hotspots, loaded once and cached.
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
                        .and_then(|rgba| CustomCursor::from_rgba(rgba, 16, 16, *x, *y).ok())
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
