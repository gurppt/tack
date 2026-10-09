use super::*;
impl LocalUi {
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
            self.focus = None;
            self.release = false;
            self.modifiers = Modifiers::NONE;
        }
        if let WindowEvent::CursorMoved { position, .. } = event {
            self.cursor = [position.x, position.y];
            return None;
        }
        if self.panel == Panel::About {
            return match event {
                WindowEvent::MouseInput {
                    state: ElementState::Pressed,
                    button: winit::event::MouseButton::Left,
                    ..
                } if self.about_layout.is_some_and(|l| l.close_hit(self.cursor)) => {
                    Some(UiResult::Dismiss)
                }
                WindowEvent::KeyboardInput { event, .. }
                    if event.state == ElementState::Pressed
                        && matches!(
                            event.physical_key,
                            PhysicalKey::Code(KeyCode::Escape | KeyCode::Enter)
                        ) =>
                {
                    Some(UiResult::Dismiss)
                }
                _ => None,
            };
        }
        if let Some(result) = self.daily_event(event) {
            return result;
        }
        let p = self.cursor.map(|v| v / self.layout[0]);
        let hit = self.hits.iter().copied().find(|h| h.contains(p));
        if self.capture {
            if matches!(
                event,
                WindowEvent::MouseInput {
                    state: ElementState::Pressed,
                    button: winit::event::MouseButton::Left,
                    ..
                }
            ) && hit.is_some_and(|h| h.command == Command::Cancel)
            {
                return self.command(Command::Cancel, keymap, profile);
            }
            let control = match event {
                WindowEvent::KeyboardInput { event, .. }
                    if event.state == ElementState::Pressed && !event.repeat =>
                {
                    match event.physical_key {
                        PhysicalKey::Code(KeyCode::Escape) => {
                            self.capture = false;
                            self.message.clear();
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
                        _ => crate::input::logical_key(event).map(PhysicalControl::LogicalKey),
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
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: winit::event::MouseButton::Left,
                ..
            } => {
                if let Some(hit) = hit {
                    if hit.enabled {
                        return self.command(hit.command, keymap, profile);
                    }
                    return None;
                }
                if self.confirm_reset.is_some() {
                    return None;
                }
                let [_, width, _, top] = self.layout;
                if p[0] >= 20.
                    && p[0] < width + 4.
                    && p[1] >= top
                    && p[1] < top + self.visible as f64 * 22.
                {
                    let selected = self.first + ((p[1] - top) / 22.).floor() as usize;
                    if selected < self.count(keymap, profile) {
                        self.selected = selected;
                        self.focus = None;
                        if !(matches!(self.panel, Panel::Keymap | Panel::Info | Panel::Connecting)
                            || self.panel == Panel::Preferences && matches!(selected, 5 | 6))
                        {
                            return self.activate(keymap, profile);
                        }
                    }
                }
                return None;
            }
            WindowEvent::MouseWheel { delta, .. } if self.confirm_reset.is_none() => {
                let dy = crate::input::wheel_steps(*delta)[1];
                let count = self.count(keymap, profile);
                if count > 0 && dy != 0. {
                    self.selected = if dy > 0. {
                        self.selected.saturating_sub(1)
                    } else {
                        (self.selected + 1).min(count - 1)
                    };
                    self.focus = None;
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
        if self.confirm_reset.is_some() {
            return match event.physical_key {
                PhysicalKey::Code(KeyCode::Escape) => {
                    self.command(Command::Cancel, keymap, profile)
                }
                PhysicalKey::Code(KeyCode::Tab) => {
                    self.focus = Some(if self.focus == Some(Command::ConfirmReset) {
                        Command::Cancel
                    } else {
                        Command::ConfirmReset
                    });
                    None
                }
                PhysicalKey::Code(KeyCode::Enter) => {
                    self.focus.and_then(|c| self.command(c, keymap, profile))
                }
                _ => None,
            };
        }
        let count = self.count(keymap, profile);
        match event.physical_key {
            PhysicalKey::Code(KeyCode::Escape) => {
                if self.panel == Panel::Keymap && !self.search.is_empty() {
                    self.search.clear();
                    self.selected = 0;
                    self.focus = None;
                    self.message.clear();
                    None
                } else {
                    self.back()
                }
            }
            PhysicalKey::Code(KeyCode::Tab) => {
                let enabled: Vec<_> = self
                    .hits
                    .iter()
                    .filter(|h| h.enabled)
                    .map(|h| h.command)
                    .collect();
                let index = self
                    .focus
                    .and_then(|c| enabled.iter().position(|v| *v == c));
                let next = if self.modifiers.contains(Modifiers::SHIFT) {
                    index
                        .and_then(|i| i.checked_sub(1))
                        .unwrap_or(enabled.len().saturating_sub(1))
                } else {
                    index.map_or(0, |i| i + 1)
                };
                self.focus = enabled.get(next).copied();
                None
            }
            PhysicalKey::Code(KeyCode::ArrowDown) => {
                self.selected = (self.selected + 1).min(count.saturating_sub(1));
                self.focus = None;
                None
            }
            PhysicalKey::Code(KeyCode::ArrowUp) => {
                self.selected = self.selected.saturating_sub(1);
                self.focus = None;
                None
            }
            PhysicalKey::Code(KeyCode::ArrowLeft | KeyCode::ArrowRight)
                if self.panel == Panel::Preferences =>
            {
                self.adjust(
                    profile,
                    event.physical_key == PhysicalKey::Code(KeyCode::ArrowRight),
                )
            }
            PhysicalKey::Code(KeyCode::Enter) => {
                if let Some(command) = self.focus.filter(|c| *c != Command::Search) {
                    self.command(command, keymap, profile)
                } else {
                    self.activate(keymap, profile)
                }
            }
            PhysicalKey::Code(KeyCode::KeyF)
                if self.panel == Panel::Keymap && self.modifiers.contains(Modifiers::CONTROL) =>
            {
                self.focus = Some(Command::Search);
                None
            }
            PhysicalKey::Code(KeyCode::Backspace) if self.panel == Panel::Keymap => {
                self.search.pop();
                self.selected = 0;
                None
            }
            PhysicalKey::Code(KeyCode::Delete) if self.panel == Panel::Keymap => {
                self.command(Command::Unassign, keymap, profile)
            }
            PhysicalKey::Code(KeyCode::F6) if self.panel == Panel::Keymap => {
                self.command(Command::Trigger, keymap, profile)
            }
            PhysicalKey::Code(KeyCode::F5) if self.panel == Panel::Keymap => {
                if self.modifiers.contains(Modifiers::CONTROL) {
                    self.command(Command::ResetAll, keymap, profile)
                } else if self.modifiers.contains(Modifiers::SHIFT) {
                    self.confirm_reset = self
                        .actions(keymap)
                        .get(self.selected)
                        .copied()
                        .map(ResetScope::Category);
                    self.focus = None;
                    self.message = "Reset category shortcuts? Confirm or Cancel.".into();
                    None
                } else {
                    self.command(Command::Reset, keymap, profile)
                }
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
                    self.focus = Some(Command::Search);
                }
                None
            }
            _ => None,
        }
    }
}
