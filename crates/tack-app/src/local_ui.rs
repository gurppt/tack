//! Lazy compact bitmap panels over the existing canvas; no retained widget tree.
use crate::{
    actions::Action,
    bindings::{Binding, Keymap, ModifierMatch, Trigger},
    image_gizmo::ImageGizmo,
    input::{Modifiers, PhysicalControl, PointerButton},
    preferences::{BindingRecord, Preferences},
};
use tack_core::Camera;
use winit::{
    event::{ElementState, WindowEvent},
    keyboard::{KeyCode, PhysicalKey},
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Panel {
    Menu,
    Preferences,
    Keymap,
    Recent,
    Close,
    Recovery,
    Error,
}
pub enum UiResult {
    Action(Action),
    PreferencesChanged,
    CloseSave,
    CloseDiscard,
    Restore,
    DiscardRecovery,
    Open(std::path::PathBuf),
    Dismiss,
}
pub struct LocalUi {
    pub panel: Panel,
    selected: usize,
    search: String,
    capture: bool,
    release: bool,
    modifiers: Modifiers,
    pub message: String,
    cursor: [f64; 2],
    layout: [f64; 4],
    first: usize,
    visible: usize,
}
impl LocalUi {
    pub fn new(panel: Panel) -> Self {
        Self {
            panel,
            selected: 0,
            search: String::new(),
            capture: false,
            release: false,
            modifiers: Modifiers::NONE,
            message: String::new(),
            cursor: [0.; 2],
            layout: [1., 600., 405., 60.],
            first: 0,
            visible: 12,
        }
    }
    fn actions(&self) -> Vec<Action> {
        Action::ALL
            .into_iter()
            .filter(|a| {
                if self.panel == Panel::Menu {
                    a.is_local() && *a != Action::ApplicationMenu
                } else {
                    let needle = self.search.to_lowercase();
                    a.label().to_lowercase().contains(&needle)
                        || a.id().to_lowercase().contains(&needle)
                        || a.category().to_lowercase().contains(&needle)
                }
            })
            .collect()
    }
    fn count(&self, profile: &Preferences) -> usize {
        match self.panel {
            Panel::Menu | Panel::Keymap => self.actions().len(),
            Panel::Preferences => 10,
            Panel::Recent => profile.recent.len(),
            Panel::Close => 3,
            Panel::Recovery => 2,
            Panel::Error => 1,
        }
    }
    fn capture_binding(
        &mut self,
        control: PhysicalControl,
        keymap: &mut Keymap,
        profile: &mut Preferences,
    ) -> Option<UiResult> {
        let action = *self.actions().get(self.selected)?;
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
        match keymap.bind(binding) {
            Ok(()) => {
                self.capture = false;
                self.message = "Binding added".into();
                profile.keymap = keymap
                    .bindings()
                    .iter()
                    .map(BindingRecord::from_binding)
                    .collect();
                Some(UiResult::PreferencesChanged)
            }
            Err(error) => {
                self.message = error.to_string();
                None
            }
        }
    }
    pub fn handle(
        &mut self,
        event: &WindowEvent,
        keymap: &mut Keymap,
        profile: &mut Preferences,
    ) -> Option<UiResult> {
        if let WindowEvent::ModifiersChanged(m) = event {
            self.modifiers = m.state().into();
        }
        if let WindowEvent::Focused(false) = event {
            self.capture = false;
            self.modifiers = Modifiers::NONE;
        }
        if self.capture {
            let control = match event {
                WindowEvent::KeyboardInput { event, .. }
                    if event.state == ElementState::Pressed && !event.repeat =>
                {
                    match event.physical_key {
                        PhysicalKey::Code(KeyCode::Escape) => {
                            self.capture = false;
                            return None;
                        }
                        PhysicalKey::Code(
                            KeyCode::ControlLeft
                            | KeyCode::ControlRight
                            | KeyCode::ShiftLeft
                            | KeyCode::ShiftRight
                            | KeyCode::AltLeft
                            | KeyCode::AltRight
                            | KeyCode::SuperLeft
                            | KeyCode::SuperRight,
                        ) => None,
                        key => Some(PhysicalControl::Key(key)),
                    }
                }
                WindowEvent::MouseInput {
                    state: ElementState::Pressed,
                    button,
                    ..
                } => Some(PhysicalControl::Pointer(PointerButton::Mouse(*button))),
                WindowEvent::MouseWheel { delta, .. } => {
                    let steps = crate::input::wheel_steps(*delta);
                    Some(PhysicalControl::Wheel(if steps[0].abs() > steps[1].abs() {
                        crate::input::WheelAxis::Horizontal
                    } else {
                        crate::input::WheelAxis::Vertical
                    }))
                }
                _ => None,
            };
            return control.and_then(|c| self.capture_binding(c, keymap, profile));
        }
        match event {
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = [position.x, position.y];
                return None;
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: winit::event::MouseButton::Left,
                ..
            } => {
                let [scale, width, _, top] = self.layout;
                let x = self.cursor[0] / scale;
                let y = self.cursor[1] / scale;
                if x >= 20. && x < width - 8. && y >= top && y < top + self.visible as f64 * 22. {
                    let selected = self.first + ((y - top) / 22.).floor() as usize;
                    if selected < self.count(profile) {
                        self.selected = selected;
                        return self.activate(keymap, profile);
                    }
                }
                return None;
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let dy = crate::input::wheel_steps(*delta)[1];
                let count = self.count(profile);
                if count > 0 && dy != 0. {
                    self.selected = if dy > 0. {
                        self.selected.saturating_sub(1)
                    } else {
                        (self.selected + 1).min(count - 1)
                    };
                }
                return None;
            }
            _ => {}
        }
        let WindowEvent::KeyboardInput { event, .. } = event else {
            return None;
        };
        if event.state != ElementState::Pressed {
            return None;
        }
        let count = self.count(profile);
        match event.physical_key {
            PhysicalKey::Code(KeyCode::Escape) => Some(UiResult::Dismiss),
            PhysicalKey::Code(KeyCode::ArrowDown) => {
                if count > 0 {
                    self.selected = (self.selected + 1) % count;
                }
                None
            }
            PhysicalKey::Code(KeyCode::ArrowUp) => {
                if count > 0 {
                    self.selected = (self.selected + count - 1) % count;
                }
                None
            }
            PhysicalKey::Code(KeyCode::Enter) => self.activate(keymap, profile),
            PhysicalKey::Code(KeyCode::Backspace) if self.panel == Panel::Keymap => {
                self.search.pop();
                self.selected = 0;
                None
            }
            PhysicalKey::Code(KeyCode::Delete) if self.panel == Panel::Keymap => {
                if let Some(action) = self.actions().get(self.selected) {
                    keymap.unassign(*action);
                    profile.keymap = keymap
                        .bindings()
                        .iter()
                        .map(BindingRecord::from_binding)
                        .collect();
                    return Some(UiResult::PreferencesChanged);
                }
                None
            }
            PhysicalKey::Code(KeyCode::F6) if self.panel == Panel::Keymap => {
                self.release = !self.release;
                None
            }
            PhysicalKey::Code(KeyCode::F5) if self.panel == Panel::Keymap => {
                let selected = self.actions().get(self.selected).copied();
                let defaults = crate::image_input::product_keymap().ok()?;
                let mut candidate = keymap.clone();
                for action in Action::ALL {
                    if self.modifiers.contains(Modifiers::CONTROL)
                        || selected.is_some_and(|s| {
                            if self.modifiers.contains(Modifiers::SHIFT) {
                                s.category() == action.category()
                            } else {
                                s == action
                            }
                        })
                    {
                        candidate.unassign(action);
                    }
                }
                for binding in defaults.bindings() {
                    let reset = self.modifiers.contains(Modifiers::CONTROL)
                        || selected.is_some_and(|s| {
                            if self.modifiers.contains(Modifiers::SHIFT) {
                                s.category() == binding.action.category()
                            } else {
                                s == binding.action
                            }
                        });
                    if reset && let Err(error) = candidate.bind(*binding) {
                        self.message = error.to_string();
                        return None;
                    }
                }
                *keymap = candidate;
                profile.keymap = keymap
                    .bindings()
                    .iter()
                    .map(BindingRecord::from_binding)
                    .collect();
                Some(UiResult::PreferencesChanged)
            }
            _ if self.panel == Panel::Keymap
                && !self.modifiers.contains(Modifiers::CONTROL)
                && !self.modifiers.contains(Modifiers::ALT) =>
            {
                if let Some(text) = &event.text
                    && self.search.len() + text.len() <= 128
                    && !text.chars().any(char::is_control)
                {
                    self.search.push_str(text);
                    self.selected = 0;
                }
                None
            }
            _ => None,
        }
    }
    fn activate(&mut self, _keymap: &mut Keymap, profile: &mut Preferences) -> Option<UiResult> {
        match self.panel {
            Panel::Menu => self
                .actions()
                .get(self.selected)
                .copied()
                .map(UiResult::Action),
            Panel::Keymap => {
                self.capture = true;
                self.message.clear();
                None
            }
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
            Panel::Error => Some(UiResult::Dismiss),
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
                    3 => profile.ui_scale = (profile.ui_scale + 1) % 5,
                    4 => {
                        profile.handle_size = if profile.handle_size >= 21 {
                            3
                        } else {
                            profile.handle_size + 2
                        }
                    }
                    5 => {
                        profile.hit_radius = if profile.hit_radius >= 31 {
                            5
                        } else {
                            profile.hit_radius + 2
                        }
                    }
                    6 => return Some(UiResult::Action(Action::KeymapEditor)),
                    7 => return Some(UiResult::Action(Action::ImportKeymap)),
                    8 => return Some(UiResult::Action(Action::ExportKeymap)),
                    _ => return Some(UiResult::Dismiss),
                }
                Some(UiResult::PreferencesChanged)
            }
        }
    }
    pub fn draw(
        &mut self,
        gizmo: &mut ImageGizmo,
        camera: &Camera,
        keymap: &Keymap,
        profile: &Preferences,
    ) {
        let scale = camera.ui_scale();
        let mut budget = 900;
        let screen = camera.screen_size();
        let width = (f64::from(screen[0]) / scale - 24.).clamp(1., 600.);
        let height = (f64::from(screen[1]) / scale - 24.).clamp(1., 405.);
        let top = if height < 100. {
            34.
        } else if height < 240. {
            46.
        } else {
            60.
        };
        let footer = if height >= 180. {
            66.
        } else if height >= 100. {
            24.
        } else {
            0.
        };
        self.layout = [scale, width, height, top];
        self.visible = ((height + 8. - top - footer) / 22.).floor().clamp(1., 12.) as usize;
        gizmo.pixel_rect(
            camera,
            [12. * scale, 12. * scale],
            [(12. + width) * scale, (12. + height) * scale],
            [0.018, 0.023, 0.028, 1.],
            None,
        );
        let heading = match self.panel {
            Panel::Menu => "Tack - local files",
            Panel::Preferences => "Preferences - Enter changes a value",
            Panel::Keymap => "Keymap - type to search",
            Panel::Recent => "Recent boards - open in another window",
            Panel::Close => "Unsaved work - save before closing?",
            Panel::Recovery => "Newer recovery available - normal save is unchanged",
            Panel::Error => "Tack - local operation error",
        };
        gizmo.ui_text(
            camera,
            [24. * scale, if height < 100. { 14. } else { 22. } * scale],
            width - 24.,
            heading,
            [0.9, 0.9, 0.88, 1.],
            &mut budget,
        );
        let rows: Vec<String> = match self.panel {
            Panel::Menu => self.actions().iter().map(|a| a.label().into()).collect(),
            Panel::Keymap => self
                .actions()
                .iter()
                .map(|a| {
                    format!(
                        "{} ({}) {}",
                        a.label(),
                        keymap.for_action(*a).count(),
                        keymap
                            .for_action(*a)
                            .map(|b| format!("{:?} {:?} {:?}", b.control, b.modifiers, b.trigger))
                            .collect::<Vec<_>>()
                            .join("; ")
                    )
                })
                .collect(),
            Panel::Preferences => vec![
                format!("Grid default: {}", profile.grid),
                format!("Image sampling default: {}", profile.sampling),
                format!(
                    "Import: {}",
                    if profile.embedded_import {
                        "Embedded"
                    } else {
                        "Linked"
                    }
                ),
                format!("UI scale: {} (0 = system)", profile.ui_scale),
                format!("Handle size: {}", profile.handle_size),
                format!("Hit radius: {}", profile.hit_radius),
                "Edit keymap".into(),
                "Import keymap".into(),
                "Export preferences/keymap".into(),
                "Close".into(),
            ],
            Panel::Recent => profile
                .recent
                .iter()
                .map(|p| {
                    p.path()
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|e| e.to_string())
                })
                .collect(),
            Panel::Close => vec![
                "Save and close".into(),
                "Discard edits and recovery".into(),
                "Cancel".into(),
            ],
            Panel::Recovery => vec![
                "Restore recovery as unsaved work".into(),
                "Discard recovery and keep normal save".into(),
            ],
            Panel::Error => vec!["Close this message".into()],
        };
        let first = self
            .selected
            .saturating_sub(self.visible / 2)
            .min(rows.len().saturating_sub(self.visible));
        self.first = first;
        for (index, row) in rows.iter().enumerate().skip(first).take(self.visible) {
            let y = top + (index - first) as f64 * 22.;
            if index == self.selected {
                gizmo.pixel_rect(
                    camera,
                    [20. * scale, y * scale],
                    [(width + 4.) * scale, (y + 20.) * scale],
                    [0.10, 0.16, 0.20, 1.],
                    None,
                );
            }
            gizmo.ui_text(
                camera,
                [24. * scale, (y + 2.) * scale],
                width - 24.,
                row,
                [0.78, 0.82, 0.83, 1.],
                &mut budget,
            );
        }
        let hint = if self.capture {
            "Press a key, mouse button or wheel. Escape cancels capture."
        } else if self.panel == Panel::Keymap {
            "Enter add; Delete unassign; F5 reset; Shift+F5 category; Ctrl+F5 all"
        } else {
            "Up/Down choose; Enter confirm; Escape close"
        };
        if height >= 180. {
            gizmo.ui_text(
                camera,
                [24. * scale, (height - 66.) * scale],
                width - 24.,
                hint,
                [0.58, 0.64, 0.68, 1.],
                &mut budget,
            );
        }
        if self.panel == Panel::Keymap && height >= 210. {
            gizmo.ui_text(
                camera,
                [24. * scale, (height - 48.) * scale],
                width - 24.,
                &format!(
                    "Search: {} | F6 binding trigger: {}",
                    self.search,
                    if self.release { "Release" } else { "Press" }
                ),
                [0.72, 0.74, 0.70, 1.],
                &mut budget,
            );
        }
        if height >= 100. {
            gizmo.ui_text(
                camera,
                [24. * scale, (height - 8.) * scale],
                width - 24.,
                &self.message,
                [0.96, 0.69, 0.32, 1.],
                &mut budget,
            );
        }
    }
}
