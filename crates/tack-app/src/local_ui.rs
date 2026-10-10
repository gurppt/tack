//! Temporary bitmap panels: bounded hit regions, one catalog/profile path.
use crate::{
    actions::Action,
    bindings::{Binding, BindingError, Keymap, ModifierMatch, Trigger},
    image_gizmo::ImageGizmo,
    input::{Modifiers, PhysicalControl, PointerButton},
    preferences::{BindingRecord, Preferences},
};
use tack_core::Camera;
use winit::{
    event::{ElementState, WindowEvent},
    keyboard::{KeyCode, PhysicalKey},
};
mod daily;
mod draw;
pub use daily::DailyPanel;
#[cfg(test)]
mod daily_tests;
mod events;
mod settings;
#[cfg(test)]
mod tests;
mod toolbar;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Panel {
    Sharing,
    Server,
    Toolbar,
    Bookmarks,
    BookmarkName,
    Info,
    Join,
    Connecting,
    About,
    Menu,
    Preferences,
    Scale,
    Theme,
    Keymap,
    Recent,
    Close,
    Recovery,
    Error,
}
pub enum UiResult {
    JumpBookmark(tack_core::BookmarkId),
    SaveBookmark(Option<tack_core::BookmarkId>, String),
    RenameBookmark(tack_core::BookmarkId),
    DeleteBookmark(tack_core::BookmarkId),
    Join(String),
    PasteAddress,
    Sharing(crate::sharing::Choice),
    Server(String),
    Action(Action),
    PreferencesChanged,
    CloseSave,
    CloseDiscard,
    Restore,
    DiscardRecovery,
    Open(std::path::PathBuf),
    Dismiss,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Command {
    ToolbarToggle,
    ToolbarRemove,
    ToolbarUp,
    ToolbarDown,
    ToolbarPlacement,
    ToolbarReset,
    RenameBookmark,
    DeleteBookmark,
    ConfirmDaily,
    Search,
    Change,
    Unassign,
    Reset,
    Import,
    Export,
    ResetAll,
    Trigger,
    Close,
    Decrement(usize),
    Increment(usize),
    ConfirmReset,
    Cancel,
}
#[derive(Clone, Copy)]
struct Hit {
    rect: [f64; 4],
    command: Command,
    enabled: bool,
}
impl Hit {
    fn contains(self, p: [f64; 2]) -> bool {
        p[0] >= self.rect[0] && p[0] < self.rect[2] && p[1] >= self.rect[1] && p[1] < self.rect[3]
    }
}
#[derive(Clone, Copy)]
enum ResetScope {
    All,
    Category(Action),
}
pub struct LocalUi {
    daily: Option<Box<DailyPanel>>,
    pub about_image: bool,
    pub about_layout: Option<crate::about::Layout>,
    pub panel: Panel,
    parents: Vec<(Panel, usize)>,
    selected: usize,
    toolbar_order: bool,
    search: String,
    capture: bool,
    shortcut_click: Option<(usize, std::time::Instant)>,
    release: bool,
    modifiers: Modifiers,
    pub message: String,
    cursor: [f64; 2],
    panel_offset: [f64; 2],
    layout: [f64; 4],
    first: usize,
    visible: usize,
    hits: Vec<Hit>,
    focus: Option<Command>,
    confirm_reset: Option<ResetScope>,
}
impl LocalUi {
    pub fn set_pointer(&mut self, pointer: [f64; 2]) {
        self.cursor = pointer;
    }
    pub fn new(panel: Panel) -> Self {
        Self {
            daily: None,
            about_image: false,
            about_layout: None,
            panel,
            parents: Vec::new(),
            selected: 0,
            toolbar_order: false,
            search: String::new(),
            capture: false,
            shortcut_click: None,
            release: false,
            modifiers: Modifiers::NONE,
            message: String::new(),
            cursor: [0.; 2],
            panel_offset: [0.; 2],
            layout: [1., 600., 405., 60.],
            first: 0,
            visible: 12,
            hits: Vec::new(),
            focus: None,
            confirm_reset: None,
        }
    }
    fn enter(&mut self, panel: Panel, selected: usize) {
        if self.parents.len() < 4 {
            self.parents.push((self.panel, self.selected));
            self.panel = panel;
            self.selected = selected;
            self.clear_focus();
        }
    }
    fn clear_focus(&mut self) {
        self.capture = false;
        self.release = false;
        self.focus = None;
        self.confirm_reset = None;
        self.modifiers = Modifiers::NONE;
        self.search.clear();
        self.message.clear();
        self.hits.clear();
        self.first = 0;
    }
    fn back(&mut self) -> Option<UiResult> {
        self.clear_focus();
        if let Some((panel, selected)) = self.parents.pop() {
            self.panel = panel;
            self.selected = selected;
            None
        } else {
            Some(UiResult::Dismiss)
        }
    }
    fn actions(&self, keymap: &Keymap) -> Vec<Action> {
        if self.panel == Panel::Menu {
            Action::ALL
                .into_iter()
                .filter(|a| a.is_local() && *a != Action::ApplicationMenu)
                .collect()
        } else {
            filtered_actions(&self.search, keymap)
        }
    }
    fn count(&self, keymap: &Keymap, profile: &Preferences) -> usize {
        match self.panel {
            Panel::Toolbar => {
                if self.toolbar_order {
                    profile.toolbar.actions.len()
                } else {
                    Self::toolbar_count()
                }
            }
            Panel::Sharing | Panel::Server => self.daily_rows().len(),
            Panel::Bookmarks
            | Panel::BookmarkName
            | Panel::Info
            | Panel::Join
            | Panel::Connecting => self.daily_rows().len(),
            Panel::Menu | Panel::Keymap => self.actions(keymap).len(),
            Panel::Preferences => 10,
            Panel::Scale => 5,
            Panel::Theme => 3,
            Panel::Recent => profile.recent.len(),
            Panel::Close => 3,
            Panel::Recovery => 2,
            Panel::Error | Panel::About => 1,
        }
    }
    fn persist(&mut self, keymap: &Keymap, profile: &mut Preferences) -> Option<UiResult> {
        profile.keymap = keymap
            .bindings()
            .iter()
            .map(BindingRecord::from_binding)
            .collect();
        Some(UiResult::PreferencesChanged)
    }
    fn capture_binding(
        &mut self,
        control: PhysicalControl,
        keymap: &mut Keymap,
        profile: &mut Preferences,
    ) -> Option<UiResult> {
        let action = *self.actions(keymap).get(self.selected)?;
        let trigger = if matches!(control, PhysicalControl::Wheel(_)) {
            Trigger::Wheel
        } else if action.captured_hold() || action == Action::PanView {
            Trigger::Hold
        } else if self.release {
            Trigger::Release
        } else {
            Trigger::Press
        };
        let binding = Binding {
            action,
            control,
            trigger,
            modifiers: ModifierMatch::Exact(self.modifiers),
        };
        let mut candidate = keymap.clone();
        candidate.unassign(action);
        if let Action::SelectTool(tool) = action {
            candidate.unassign(Action::TemporaryTool(tool));
        }
        let binding = if let Action::SelectTool(tool) = action {
            Binding {
                action: if self.release {
                    Action::TemporaryTool(tool)
                } else {
                    action
                },
                trigger: if self.release {
                    Trigger::Hold
                } else {
                    Trigger::Press
                },
                ..binding
            }
        } else {
            Binding {
                trigger: if action.captured_hold() || action == Action::PanView {
                    Trigger::Hold
                } else {
                    Trigger::Press
                },
                ..binding
            }
        };
        let mut displaced = None;
        if let Err(error) = candidate.bind(binding) {
            if let BindingError::Conflict { existing } = error {
                let old = candidate.bindings()[existing].action;
                displaced = Some(old);
                candidate.unassign(old);
                if candidate.bind(binding).is_err() {
                    self.message = "Shortcut cannot be assigned".into();
                    return None;
                }
            } else {
                self.message = error.to_string();
                return None;
            }
        }
        *keymap = candidate;
        self.capture = false;
        self.focus = None;
        self.message = displaced.map_or_else(
            || format!("Changed {}", action.label()),
            |old| {
                format!(
                    "{} assigned to {}; {} is now unbound",
                    crate::context_menu::binding_label(&binding),
                    action.label(),
                    old.label()
                )
            },
        );
        self.persist(keymap, profile)
    }

    fn reset(
        &mut self,
        keymap: &mut Keymap,
        profile: &mut Preferences,
        scope: Option<ResetScope>,
    ) -> Option<UiResult> {
        let selected = self.actions(keymap).get(self.selected).copied();
        let includes = |a: Action| match scope {
            Some(ResetScope::All) => true,
            Some(ResetScope::Category(s)) => s.category() == a.category(),
            None => selected.is_some_and(|s| shortcut_matches(s, a)),
        };
        let defaults = crate::image_input::product_keymap().ok()?;
        let mut candidate = keymap.clone();
        for action in Action::ALL {
            if includes(action) {
                candidate.unassign(action);
            }
        }
        for binding in defaults.bindings().iter().filter(|b| includes(b.action)) {
            if let Err(error) = candidate.bind(*binding) {
                self.message = if let BindingError::Conflict { existing } = error {
                    format!(
                        "Reset refused: {} is used by {}.",
                        crate::context_menu::binding_label(binding),
                        candidate.bindings()[existing].action.label()
                    )
                } else {
                    error.to_string()
                };
                return None;
            }
        }
        *keymap = candidate;
        self.message = "Defaults restored".into();
        self.confirm_reset = None;
        self.persist(keymap, profile)
    }
    fn command(
        &mut self,
        command: Command,
        keymap: &mut Keymap,
        profile: &mut Preferences,
    ) -> Option<UiResult> {
        self.focus = None;
        if self.panel == Panel::Toolbar && command != Command::Close {
            return self.toolbar_command(command, profile);
        }
        match command {
            Command::ToolbarToggle
            | Command::ToolbarRemove
            | Command::ToolbarUp
            | Command::ToolbarDown
            | Command::ToolbarPlacement
            | Command::ToolbarReset => None,
            Command::RenameBookmark => self.daily_command(true),
            Command::DeleteBookmark => self.daily_command(false),
            Command::ConfirmDaily => {
                self.selected = 1;
                self.daily_activate()
            }
            Command::Search => {
                self.focus = Some(Command::Search);
                None
            }
            Command::Change => {
                if let Some(action) = self.actions(keymap).get(self.selected) {
                    self.release = keymap
                        .bindings()
                        .iter()
                        .any(|b| shortcut_matches(*action, b.action) && b.trigger == Trigger::Hold);
                    self.capture = true;
                    self.message.clear();
                }
                None
            }
            Command::Unassign => {
                let action = *self.actions(keymap).get(self.selected)?;
                keymap.unassign(action);
                if let Action::SelectTool(tool) = action {
                    keymap.unassign(Action::TemporaryTool(tool));
                }
                self.message = format!("Unassigned {}", action.label());
                self.persist(keymap, profile)
            }
            Command::Reset => self.reset(keymap, profile, None),
            Command::ResetAll => {
                self.confirm_reset = Some(ResetScope::All);
                self.message = "Reset all shortcuts? Confirm or Cancel.".into();
                None
            }
            Command::ConfirmReset => self.reset(keymap, profile, self.confirm_reset),
            Command::Cancel => {
                self.capture = false;
                self.confirm_reset = None;
                self.message.clear();
                None
            }
            Command::Trigger => {
                let action = *self.actions(keymap).get(self.selected)?;
                let Action::SelectTool(tool) = action else {
                    self.message = "This action has fixed Normal/Hold semantics".into();
                    return None;
                };
                self.release = !keymap
                    .bindings()
                    .iter()
                    .any(|b| shortcut_matches(action, b.action) && b.trigger == Trigger::Hold);
                let bindings: Vec<_> = keymap
                    .bindings()
                    .iter()
                    .filter(|b| shortcut_matches(action, b.action))
                    .copied()
                    .collect();
                let mut candidate = keymap.clone();
                candidate.unassign(action);
                candidate.unassign(Action::TemporaryTool(tool));
                for binding in bindings {
                    if candidate
                        .bind(Binding {
                            action: if self.release {
                                Action::TemporaryTool(tool)
                            } else {
                                action
                            },
                            trigger: if self.release {
                                Trigger::Hold
                            } else {
                                Trigger::Press
                            },
                            ..binding
                        })
                        .is_err()
                    {
                        self.message = "Cannot change shortcut mode".into();
                        return None;
                    }
                }
                *keymap = candidate;
                self.persist(keymap, profile)
            }
            Command::Import => Some(UiResult::Action(Action::ImportKeymap)),
            Command::Export => Some(UiResult::Action(Action::ExportKeymap)),
            Command::Close => self.back(),
            Command::Decrement(row) => {
                self.selected = row;
                self.adjust(profile, false)
            }
            Command::Increment(row) => {
                self.selected = row;
                self.adjust(profile, true)
            }
        }
    }
    fn adjust(&mut self, profile: &mut Preferences, increment: bool) -> Option<UiResult> {
        let value = match self.selected {
            5 => &mut profile.handle_size,
            6 => &mut profile.hit_radius,
            _ => return None,
        };
        let (min, max) = if self.selected == 5 { (3, 21) } else { (5, 32) };
        let next = if increment {
            value.saturating_add(1).min(max)
        } else {
            value.saturating_sub(1).max(min)
        };
        if next == *value {
            return None;
        }
        *value = next;
        Some(UiResult::PreferencesChanged)
    }
    fn activate(&mut self, keymap: &mut Keymap, profile: &mut Preferences) -> Option<UiResult> {
        match self.panel {
            Panel::Toolbar => self.toolbar_command(Command::ToolbarToggle, profile),
            Panel::Sharing
            | Panel::Server
            | Panel::Bookmarks
            | Panel::BookmarkName
            | Panel::Join => self.daily_activate(),
            Panel::Info | Panel::Connecting => Some(UiResult::Dismiss),
            Panel::Menu => self
                .actions(keymap)
                .get(self.selected)
                .copied()
                .map(UiResult::Action),
            Panel::Keymap => self.command(Command::Change, keymap, profile),
            Panel::Recent => profile
                .recent
                .get(self.selected)
                .and_then(|p| match p.path() {
                    Ok(p) => Some(UiResult::Open(p)),
                    Err(e) => {
                        self.message = e.to_string();
                        None
                    }
                }),
            Panel::Close => Some(match self.selected {
                0 => UiResult::CloseSave,
                1 => UiResult::CloseDiscard,
                _ => UiResult::Dismiss,
            }),
            Panel::Recovery => Some(if self.selected == 0 {
                UiResult::Restore
            } else {
                UiResult::DiscardRecovery
            }),
            Panel::Error | Panel::About => Some(UiResult::Dismiss),
            Panel::Scale | Panel::Theme => {
                let scale = self.panel == Panel::Scale;
                if scale {
                    profile.ui_scale = self.selected as u8;
                } else {
                    profile.theme = crate::ui_theme::Theme::ALL[self.selected];
                }
                Some(UiResult::PreferencesChanged)
            }
            Panel::Preferences => {
                match self.selected {
                    0 => profile.grid = !profile.grid,
                    1 => {
                        profile.sampling = if profile.sampling == "Smooth" {
                            "Nearest"
                        } else {
                            "Smooth"
                        }
                        .into()
                    }
                    2 => profile.embedded_import = !profile.embedded_import,
                    3 => {
                        self.enter(Panel::Scale, usize::from(profile.ui_scale.min(4)));
                        return None;
                    }
                    4 => {
                        let selected = crate::ui_theme::Theme::ALL
                            .iter()
                            .position(|t| *t == profile.theme)
                            .unwrap_or(1);
                        self.enter(Panel::Theme, selected);
                        return None;
                    }
                    5 | 6 => return self.adjust(profile, true),
                    7 => profile.frame_title_scale = profile.frame_title_scale % 3 + 1,
                    8 => return Some(UiResult::Action(Action::ExportPreferences)),
                    _ => return self.back(),
                }
                Some(UiResult::PreferencesChanged)
            }
        }
    }
}
/// Bounded event/draw-time scan; no retained action widgets or search index.
pub fn filtered_actions(search: &str, keymap: &Keymap) -> Vec<Action> {
    let needle = search.to_lowercase();
    let matches = |text: &str| {
        if needle.is_empty() {
            true
        } else if text.is_ascii() && needle.is_ascii() {
            text.as_bytes()
                .windows(needle.len())
                .any(|s| s.eq_ignore_ascii_case(needle.as_bytes()))
        } else {
            text.to_lowercase().contains(&needle)
        }
    };
    Action::ALL
        .into_iter()
        .filter(|a| !matches!(a, Action::TemporaryTool(_)))
        .filter(|a| {
            matches(a.label())
                || matches(&a.id())
                || matches(a.category())
                || keymap
                    .for_action(*a)
                    .any(|b| matches(&crate::context_menu::binding_label(b)))
        })
        .collect()
}

fn shortcut_matches(action: Action, binding: Action) -> bool {
    action == binding
        || matches!((action, binding), (Action::SelectTool(a), Action::TemporaryTool(b)) if a == b)
}
