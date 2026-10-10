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
#[cfg(test)]
mod polish_tests;
mod scroll;
mod settings;
#[cfg(test)]
mod tests;
mod toolbar;
mod toolbar_events;
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
    UpdateApplying,
    UpdateChecking,
    UpdateOffer,
    UpdateReady,
    About,
    Menu,
    Preferences,
    Theme,
    Keymap,
    Recent,
    Close,
    Recovery,
    Error,
    ViewCapture,
}
pub enum UiResult {
    UpdateAdvance,
    AssignCameraSlot(u8),
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
    ConfirmCapture,
    SaveKeyset,
    ToolbarToggle,
    ToolbarRemove,
    ToolbarUp,
    ToolbarDown,
    ToolbarPlacement,
    ToolbarScale,
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
    pub view_capture_slot: Option<u8>,
    modal: Option<crate::modal_shell::ModalShell>,
    pub about_image: bool,
    pub about_layout: Option<crate::about::Layout>,
    pub panel: Panel,
    parents: Vec<(Panel, usize)>,
    selected: usize,
    toolbar_order: bool,
    toolbar_selected: [usize; 2],
    toolbar_first: [usize; 2],
    pub feedback: crate::feedback::Feedback,
    pub caret: crate::feedback::Caret,
    icons: [tack_render::UiIcon; 32],
    icon_count: usize,
    search: String,
    capture: bool,
    shortcut_click: Option<(usize, std::time::Instant)>,
    behavior: crate::shortcut_capture::Behavior,
    staged: Option<crate::shortcut_capture::Capture>,
    key_column: usize,
    modifiers: Modifiers,
    pub message: String,
    cursor: [f64; 2],
    panel_offset: [f64; 2],
    layout: [f64; 4],
    first: usize,
    visible: usize,
    scroll: [crate::ui_scroll::Scrollbar; 2],
    reveal_row: bool,
    hits: Vec<Hit>,
    focus: Option<Command>,
    confirm_reset: Option<ResetScope>,
}
impl LocalUi {
    pub fn text_editing(&self) -> bool {
        (matches!(
            self.panel,
            Panel::BookmarkName | Panel::Join | Panel::Server
        ) && self.selected == 0
            && self.focus.is_none())
            || (self.panel == Panel::Keymap
                && !self.capture
                && self.confirm_reset.is_none()
                && (self.focus.is_none() || self.focus == Some(Command::Search)))
    }
    pub fn icons(&self) -> &[tack_render::UiIcon] {
        &self.icons[..self.icon_count]
    }
    pub fn set_pointer(&mut self, pointer: [f64; 2]) {
        self.cursor = pointer;
    }
    pub fn new(panel: Panel) -> Self {
        Self {
            daily: None,
            view_capture_slot: None,
            modal: None,
            about_image: false,
            about_layout: None,
            panel,
            parents: Vec::new(),
            selected: 0,
            toolbar_order: false,
            toolbar_selected: [0; 2],
            toolbar_first: [0; 2],
            feedback: Default::default(),
            caret: Default::default(),
            icons: [tack_render::UiIcon::default(); 32],
            icon_count: 0,
            search: String::new(),
            capture: false,
            shortcut_click: None,
            behavior: Default::default(),
            staged: None,
            key_column: 0,
            modifiers: Modifiers::NONE,
            message: String::new(),
            cursor: [0.; 2],
            panel_offset: [0.; 2],
            layout: [1., 600., 405., 60.],
            first: 0,
            visible: 12,
            scroll: Default::default(),
            reveal_row: true,
            hits: Vec::new(),
            focus: None,
            confirm_reset: None,
        }
    }
    pub fn blocks_menu_access(&self) -> bool {
        matches!(
            self.panel,
            Panel::Close | Panel::Recovery | Panel::Connecting | Panel::UpdateApplying
        ) || self.confirm_reset.is_some()
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
        self.behavior = Default::default();
        self.staged = None;
        self.focus = None;
        self.confirm_reset = None;
        self.modifiers = Modifiers::NONE;
        self.search.clear();
        self.message.clear();
        self.hits.clear();
        self.first = 0;
    }
    pub fn view_capture(slot: u8) -> Self {
        let mut ui = Self::new(Panel::ViewCapture);
        ui.view_capture_slot = Some(slot);
        ui.capture = true;
        ui.message = "Press a shortcut; Enter confirms; Escape cancels".into();
        ui
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
        if let Some(slot) = self.view_capture_slot {
            return vec![Action::JumpCameraSlot(slot)];
        }
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
            | Panel::Connecting
            | Panel::UpdateApplying
            | Panel::UpdateChecking
            | Panel::UpdateOffer
            | Panel::UpdateReady => self.daily_rows().len(),
            Panel::Menu | Panel::Keymap => self.actions(keymap).len(),
            Panel::Preferences => 10,
            Panel::Theme => 3,
            Panel::Recent => profile.recent.len(),
            Panel::Close => 3,
            Panel::Recovery => 2,
            Panel::Error | Panel::About | Panel::ViewCapture => 1,
        }
    }
    fn persist(&mut self, keymap: &Keymap, profile: &mut Preferences) -> Option<UiResult> {
        profile.keyset.dirty = true;
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
        let binding = self.behavior.binding(
            action,
            Binding {
                action,
                control,
                trigger: Trigger::Press,
                modifiers: ModifierMatch::Exact(self.modifiers),
            },
        );
        self.staged = None;
        match crate::shortcut_capture::Capture::stage(keymap, action, binding) {
            Ok(staged) => {
                self.message = staged.message();
                self.staged = Some(staged);
            }
            Err(e) => self.message = e.to_string(),
        }
        let _ = profile;
        None
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
            Command::SaveKeyset => Some(UiResult::Action(Action::SaveKeymap)),
            Command::ConfirmCapture => {
                if self.view_capture_slot.is_some()
                    && profile.local_views.len() >= crate::camera_slots::MAX_VIEWS
                {
                    self.message = "Too many local views (maximum 64)".into();
                    return None;
                }
                let staged = self.staged.take()?;
                self.message = staged.message();
                *keymap = staged.candidate;
                self.capture = false;

                let result = self.persist(keymap, profile);
                if let Some(slot) = self.view_capture_slot {
                    return Some(UiResult::AssignCameraSlot(slot));
                }
                result
            }
            Command::ToolbarToggle
            | Command::ToolbarRemove
            | Command::ToolbarUp
            | Command::ToolbarDown
            | Command::ToolbarScale
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
                    self.behavior = keymap
                        .bindings()
                        .iter()
                        .find(|b| shortcut_matches(*action, b.action))
                        .map_or(Default::default(), |b| {
                            crate::shortcut_capture::Behavior::from_trigger(b.trigger)
                        });
                    self.staged = None;
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
                self.message = if action == Action::ApplicationMenu {
                    "Additional menu shortcuts cleared; F10 remains fixed".into()
                } else {
                    format!("Unassigned {}", action.label())
                };
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
                self.staged = None;
                self.confirm_reset = None;
                self.message.clear();
                None
            }
            Command::Trigger => {
                let action = *self.actions(keymap).get(self.selected)?;
                let bindings: Vec<_> = keymap
                    .bindings()
                    .iter()
                    .filter(|b| shortcut_matches(action, b.action))
                    .copied()
                    .collect();
                let current = bindings.first().map_or(self.behavior, |b| {
                    crate::shortcut_capture::Behavior::from_trigger(b.trigger)
                });
                if bindings
                    .iter()
                    .any(|b| matches!(b.control, PhysicalControl::Wheel(_)))
                {
                    self.message = "Wheel shortcuts have Normal behavior only".into();
                    return None;
                }
                let mut next = current.next();
                let mut candidate = keymap.clone();
                candidate.unassign(action);
                if let Action::SelectTool(t) = action {
                    candidate.unassign(Action::TemporaryTool(t));
                }
                // Skip illegal behavior without silently binding a different action.
                let prototype = bindings.first().copied().unwrap_or(Binding {
                    action,
                    control: PhysicalControl::LogicalKey(crate::input::LogicalKey::Character('q')),
                    trigger: Trigger::Press,
                    modifiers: ModifierMatch::Exact(Modifiers::NONE),
                });
                for _ in 0..3 {
                    let mut check = Keymap::default();
                    if check.bind(next.binding(action, prototype)).is_ok() {
                        break;
                    }
                    next = next.next();
                }
                if next == current {
                    self.message =
                        format!("{} supports only {:?} behavior", action.label(), current);
                    return None;
                }
                for binding in bindings {
                    if candidate.bind(next.binding(action, binding)).is_err() {
                        self.message = "Cannot change shortcut behavior".into();
                        return None;
                    }
                }
                self.behavior = next;
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
            4 => &mut profile.handle_size,
            5 => &mut profile.hit_radius,
            _ => return None,
        };
        let (min, max) = if self.selected == 4 { (3, 21) } else { (5, 32) };
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
            Panel::UpdateApplying => None,
            Panel::UpdateOffer | Panel::UpdateReady => self.daily_activate(),
            Panel::UpdateChecking | Panel::Info | Panel::Connecting => Some(UiResult::Dismiss),
            Panel::ViewCapture => None,
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
            Panel::Theme => {
                profile.theme = crate::ui_theme::Theme::ALL[self.selected];
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
                        let selected = crate::ui_theme::Theme::ALL
                            .iter()
                            .position(|t| *t == profile.theme)
                            .unwrap_or(1);
                        self.enter(Panel::Theme, selected);
                        return None;
                    }
                    4 | 5 => return self.adjust(profile, true),
                    6 => profile.frame_title_scale = profile.frame_title_scale % 3 + 1,
                    7 => {
                        profile.update_channel = match profile.update_channel {
                            tack_update::Channel::Dev => tack_update::Channel::Stable,
                            tack_update::Channel::Stable => tack_update::Channel::Dev,
                        };
                    }
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
        .chain(
            keymap
                .bindings()
                .iter()
                .map(|b| b.action)
                .filter(|a| !Action::ALL.contains(a)),
        )
        .filter(|a| a.available() && !matches!(a, Action::TemporaryTool(_)))
        .filter(|a| {
            matches(a.label())
                || matches(&a.id())
                || matches(a.category())
                || keymap
                    .bindings()
                    .iter()
                    .filter(|b| shortcut_matches(*a, b.action))
                    .any(|b| matches(&crate::context_menu::binding_label(b)))
        })
        .fold(Vec::new(), |mut actions, action| {
            if !actions.contains(&action) {
                actions.push(action);
            }
            actions
        })
}

fn shortcut_matches(action: Action, binding: Action) -> bool {
    action == binding
        || matches!((action, binding), (Action::SelectTool(a), Action::TemporaryTool(b)) if a == b)
}
