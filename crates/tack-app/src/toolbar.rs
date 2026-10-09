//! Fixed-size action projection. Geometry is stack-resident; no widget tree or timer.
use crate::{
    actions::{Action, Tool},
    preferences::Preferences,
};
use serde::{Deserialize, Serialize};
pub const MAX_ENTRIES: usize = 32;
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
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub actions: Vec<String>,
    pub placement: Placement,
    pub offset: u16,
    pub floating: [u16; 2],
}
impl Default for Config {
    fn default() -> Self {
        Self {
            actions: DEFAULT_ACTIONS.map(Action::id).to_vec(),
            placement: Placement::Top,
            offset: 0,
            floating: [48, 48],
        }
    }
}
impl Config {
    pub fn normalize(&mut self) -> Result<(), tack_assets::AssetError> {
        if self.actions.len() > MAX_ENTRIES
            || self.offset > 8192
            || self.floating.iter().any(|v| *v > 8192)
        {
            return Err("toolbar configuration exceeds bound".into());
        }
        let mut valid = Vec::with_capacity(self.actions.len());
        for id in &self.actions {
            if let Some(a) = Action::from_id(id)
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
        let scale = scale.round().clamp(1., 4.);
        let w = f64::from(size[0]) / scale;
        let h = f64::from(size[1]) / scale - if status { 20. } else { 0. };
        let vertical = matches!(config.placement, Placement::Left | Placement::Right);
        let capacity = ((if vertical { h } else { w } - 12.) / 22.).floor().max(1.) as usize;
        let count = config.actions.len().min(MAX_ENTRIES);
        let across = count.min(capacity);
        let rows = count.div_ceil(across);
        let dims = if vertical {
            [rows as f64 * 22. + 4., across as f64 * 22. + 12.]
        } else {
            [across as f64 * 22. + 12., rows as f64 * 22. + 4.]
        };
        let offset = f64::from(config.offset);
        let origin = match config.placement {
            Placement::Top => [offset, 2.],
            Placement::Bottom => [offset, h - dims[1] - 2.],
            Placement::Left => [2., offset],
            Placement::Right => [w - dims[0] - 2., offset],
            Placement::Floating => config.floating.map(f64::from),
            Placement::Hidden => [0.; 2],
        };
        let origin = [
            origin[0].clamp(0., (w - dims[0]).max(0.)),
            origin[1].clamp(0., (h - dims[1]).max(0.)),
        ];
        self.bounds = [
            origin[0] * scale,
            origin[1] * scale,
            (origin[0] + dims[0]) * scale,
            (origin[1] + dims[1]) * scale,
        ];
        self.grip = if vertical {
            [
                origin[0] * scale,
                origin[1] * scale,
                (origin[0] + dims[0]) * scale,
                (origin[1] + 10.) * scale,
            ]
        } else {
            [
                origin[0] * scale,
                origin[1] * scale,
                (origin[0] + 10.) * scale,
                (origin[1] + dims[1]) * scale,
            ]
        };
        for (i, id) in config.actions.iter().take(count).enumerate() {
            let (x, y) = if vertical {
                (
                    (i / across) as f64 * 22. + 2.,
                    (i % across) as f64 * 22. + 10.,
                )
            } else {
                (
                    (i % across) as f64 * 22. + 10.,
                    (i / across) as f64 * 22. + 2.,
                )
            };
            self.buttons[i] = Button {
                rect: [
                    (origin[0] + x) * scale,
                    (origin[1] + y) * scale,
                    (origin[0] + x + 20.) * scale,
                    (origin[1] + y + 20.) * scale,
                ],
                action: Action::from_id(id),
            };
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
            Placement::Top
        } else {
            Placement::Hidden
        };
    }
}
