//! Fixed-size action projection. Geometry is stack-resident; no widget tree or timer.
use crate::{
    actions::{Action, Tool},
    preferences::Preferences,
};
use serde::{Deserialize, Serialize};
pub const MAX_ENTRIES: usize = 32;
/// Persisted layout token, deliberately outside the semantic Action catalog.
pub const SEPARATOR: &str = "Separator";
pub const DEFAULT_ACTIONS: [Action; 12] = [
    Action::SelectTool(Tool::Pointer),
    Action::SelectTool(Tool::Pan),
    Action::SelectTool(Tool::Text),
    Action::SelectTool(Tool::Rectangle),
    Action::SelectTool(Tool::Line),
    Action::SelectTool(Tool::Arrow),
    Action::SelectTool(Tool::Scribble),
    Action::CreateFrame,
    Action::DuplicateSelection,
    Action::Undo,
    Action::Redo,
    Action::ShareBoard,
];
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Item {
    Action(Action),
    Separator,
}
impl Item {
    pub fn from_id(id: &str) -> Option<Self> {
        if id == SEPARATOR {
            Some(Self::Separator)
        } else {
            Action::from_id(id).map(Self::Action)
        }
    }
    pub fn id(self) -> String {
        match self {
            Self::Action(a) => a.id(),
            Self::Separator => SEPARATOR.into(),
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Action(a) => a.label(),
            Self::Separator => "Separator",
        }
    }
    pub fn icon(self) -> Option<usize> {
        if let Self::Action(a) = self {
            crate::toolbar_icons::actual_index(a)
        } else {
            None
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Placement {
    #[default]
    Top,
    Left,
    Right,
    Bottom,
    Floating,
    Hidden,
}
impl Placement {
    pub const ALL: [Self; 6] = [
        Self::Top,
        Self::Left,
        Self::Right,
        Self::Bottom,
        Self::Floating,
        Self::Hidden,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Top => "Top",
            Self::Left => "Left",
            Self::Right => "Right",
            Self::Bottom => "Bottom",
            Self::Floating => "Floating",
            Self::Hidden => "Hidden",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub actions: Vec<String>,
    pub placement: Placement,
    pub offset: u16,
    #[serde(default)]
    pub edge_position: Option<u16>,
    #[serde(default = "toolbar_scale_default")]
    pub scale: u8,
    #[serde(default)]
    pub offset_set: bool,
    #[serde(default)]
    pub last_visible: Placement,
    pub floating: [u16; 2],
}
fn toolbar_scale_default() -> u8 {
    1
}
impl Default for Config {
    fn default() -> Self {
        Self {
            actions: DEFAULT_ACTIONS.map(Action::id).to_vec(),
            placement: Placement::Top,
            offset: 0,
            edge_position: None,
            scale: 1,
            offset_set: false,
            last_visible: Placement::Top,
            floating: [48, 48],
        }
    }
}
impl Config {
    pub fn normalize(&mut self) -> Result<(), tack_assets::AssetError> {
        if self.actions.len() > MAX_ENTRIES
            || self.edge_position.is_some_and(|p| p > 10_000)
            || !(1..=3).contains(&self.scale)
            || self.offset > 8192
            || self.floating.iter().any(|v| *v > 8192)
        {
            return Err("toolbar configuration exceeds bound".into());
        }
        let mut valid = Vec::with_capacity(self.actions.len());
        for id in &self.actions {
            if id == SEPARATOR {
                if valid.last().is_none_or(|v| v != SEPARATOR) {
                    valid.push(id.clone());
                }
            } else if let Some(a) = Action::from_id(id)
                && eligible(a)
                && !valid.contains(id)
            {
                valid.push(id.clone());
            }
        }
        self.actions = valid;
        Ok(())
    }
}
pub fn eligible(a: Action) -> bool {
    !a.captured_hold()
        && !matches!(
            a,
            Action::PanView
                | Action::ZoomView
                | Action::ImagePointer
                | Action::ToggleSelection
                | Action::CenterPointer
                | Action::CancelInteraction
                | Action::ApplicationMenu
                | Action::StopSharing
        )
}
#[derive(Clone, Copy, Debug, Default)]
pub struct Button {
    pub rect: [f64; 4],
    pub action: Option<Action>,
}
fn contains(r: [f64; 4], p: [f64; 2]) -> bool {
    p[0] >= r[0] && p[0] < r[2] && p[1] >= r[1] && p[1] < r[3]
}
#[derive(Default)]
pub struct Toolbar {
    pub buttons: [Button; MAX_ENTRIES],
    pub count: usize,
    pub bounds: [f64; 4],
    pub grip: [f64; 4],
    pub hover: Option<Action>,
    pub pressed: Option<Action>,
    pub drag: Option<[f64; 2]>,
}
impl Toolbar {
    pub fn layout(&mut self, config: &Config, size: [u32; 2], scale: f64, status: bool) {
        self.count = 0;
        self.bounds = [0.; 4];
        self.grip = [0.; 4];
        if config.placement == Placement::Hidden || config.actions.is_empty() {
            return;
        }
        let status_height = if status {
            20. * scale.round().clamp(1., 4.)
        } else {
            0.
        };
        let scale = f64::from(config.scale.clamp(1, 3));
        let w = f64::from(size[0]) / scale;
        let h = (f64::from(size[1]) - status_height) / scale;
        let vertical = matches!(config.placement, Placement::Left | Placement::Right);
        // A 16px grip is a real layout cell, not outer padding. Every action
        // retains its complete 16px hit area. Separators occupy only one pixel.
        let limit = if vertical { h } else { w }.max(16.);
        let count = config.actions.len().min(MAX_ENTRIES);
        let mut main = 16.;
        let mut cross = 0.;
        let mut extent: f64 = 16.;
        for (i, id) in config.actions.iter().take(count).enumerate() {
            let length = if id == SEPARATOR { 1. } else { 16. };
            if main + length > limit {
                main = 0.;
                cross += 16.;
            }
            self.buttons[i] = Button {
                rect: if vertical {
                    [cross, main, cross + 16., main + length]
                } else {
                    [main, cross, main + length, cross + 16.]
                },
                action: Action::from_id(id),
            };
            main += length;
            extent = extent.max(main);
        }
        let dims = if vertical {
            [cross + 16., extent]
        } else {
            [extent, cross + 16.]
        };
        let travel = if vertical {
            (h - dims[1]).max(0.)
        } else {
            (w - dims[0]).max(0.)
        };
        let offset = if let Some(p) = config.edge_position {
            travel * f64::from(p) / 10_000.
        } else if config.offset_set || config.offset > 0 {
            f64::from(config.offset)
        } else if vertical {
            (h - dims[1]) / 2.
        } else {
            (w - dims[0]) / 2.
        };
        let origin = match config.placement {
            Placement::Top => [offset, 0.],
            Placement::Bottom => [offset, h - dims[1]],
            Placement::Left => [0., offset],
            Placement::Right => [w - dims[0], offset],
            Placement::Floating => config.floating.map(f64::from),
            Placement::Hidden => [0.; 2],
        };
        let origin = [
            origin[0].clamp(0., (w - dims[0]).max(0.)).round(),
            origin[1].clamp(0., (h - dims[1]).max(0.)).round(),
        ];
        self.bounds = [
            origin[0] * scale,
            origin[1] * scale,
            (origin[0] + dims[0]) * scale,
            (origin[1] + dims[1]) * scale,
        ];
        self.grip = [
            origin[0] * scale,
            origin[1] * scale,
            (origin[0] + 16.) * scale,
            (origin[1] + 16.) * scale,
        ];
        for button in &mut self.buttons[..count] {
            button.rect = [
                (origin[0] + button.rect[0]) * scale,
                (origin[1] + button.rect[1]) * scale,
                (origin[0] + button.rect[2]) * scale,
                (origin[1] + button.rect[3]) * scale,
            ];
        }
        self.count = count;
    }
    pub fn hit(&self, p: [f64; 2]) -> Option<Action> {
        self.buttons[..self.count]
            .iter()
            .find(|b| contains(b.rect, p))
            .and_then(|b| b.action)
    }
    pub fn contains(&self, p: [f64; 2]) -> bool {
        self.count > 0 && contains(self.bounds, p)
    }
    pub fn grip_hit(&self, p: [f64; 2]) -> bool {
        self.count > 0 && contains(self.grip, p)
    }
    pub fn toggle(profile: &mut Preferences) {
        profile.toolbar.placement = if profile.toolbar.placement == Placement::Hidden {
            if profile.toolbar.last_visible == Placement::Hidden {
                Placement::Top
            } else {
                profile.toolbar.last_visible
            }
        } else {
            profile.toolbar.last_visible = profile.toolbar.placement;
            Placement::Hidden
        };
    }
}
