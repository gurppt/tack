//! Keyboard and pointer share the toolbar editor focus.
use super::*;
impl LocalUi {
    pub(super) fn toolbar_event(
        &mut self,
        event: &WindowEvent,
        keymap: &mut Keymap,
        p: &mut Preferences,
    ) -> Option<UiResult> {
        let point = self.cursor.map(|v| v / self.layout[0]);
        let hit = self
            .hits
            .iter()
            .copied()
            .find(|h| h.enabled && h.contains(point));
        match event {
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: winit::event::MouseButton::Left,
                ..
            } => {
                if let Some(hit) = hit {
                    return self.command(hit.command, keymap, p);
                }
                self.toolbar_pointer(point, keymap, p);
            }
            WindowEvent::CursorMoved { .. } => {
                if let Some(hit) = hit {
                    self.focus = Some(hit.command);
                }
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                let PhysicalKey::Code(key) = event.physical_key else {
                    return None;
                };
                return self.toolbar_key(key, keymap, p);
            }
            _ => {}
        }
        None
    }
    pub(super) fn toolbar_key(
        &mut self,
        key: KeyCode,
        keymap: &mut Keymap,
        p: &mut Preferences,
    ) -> Option<UiResult> {
        if key == KeyCode::Escape {
            return self.back();
        }
        if key == KeyCode::Enter {
            return if let Some(cmd) = self.focus {
                self.command(cmd, keymap, p)
            } else if self.toolbar_order {
                None
            } else {
                self.toolbar_command(Command::ToolbarToggle, p)
            };
        }
        if matches!(key, KeyCode::ArrowUp | KeyCode::ArrowDown)
            && self.modifiers.contains(Modifiers::CONTROL)
        {
            if self.toolbar_order && self.focus.is_none() {
                return self.toolbar_command(
                    if key == KeyCode::ArrowUp {
                        Command::ToolbarUp
                    } else {
                        Command::ToolbarDown
                    },
                    p,
                );
            }
            return None;
        }
        if key == KeyCode::Tab {
            let commands: Vec<_> = self
                .hits
                .iter()
                .filter(|h| h.enabled)
                .map(|h| h.command)
                .collect();
            if let Some(i) = self
                .focus
                .and_then(|f| commands.iter().position(|v| *v == f))
            {
                if self.modifiers.contains(Modifiers::SHIFT) {
                    self.focus = i.checked_sub(1).map(|j| commands[j]);
                    if self.focus.is_none() {
                        self.toolbar_column(true);
                    }
                } else if i + 1 < commands.len() {
                    self.focus = Some(commands[i + 1]);
                } else {
                    self.toolbar_column(false);
                }
            } else if self.modifiers.contains(Modifiers::SHIFT) {
                if self.toolbar_order {
                    self.toolbar_column(false);
                } else {
                    self.focus = commands.last().copied();
                }
            } else if self.toolbar_order {
                self.focus = commands.first().copied();
            } else {
                self.toolbar_column(true);
            }
        } else if let Some(i) = self
            .focus
            .and_then(|f| self.hits.iter().position(|h| h.command == f))
        {
            let next = match key {
                KeyCode::ArrowUp => i.checked_sub(4),
                KeyCode::ArrowDown => Some(i + 4),
                KeyCode::ArrowLeft => i.checked_sub(1),
                KeyCode::ArrowRight => Some(i + 1),
                _ => None,
            };
            if let Some(h) = next.and_then(|j| self.hits.get(j)).filter(|h| h.enabled) {
                self.focus = Some(h.command);
            }
        } else {
            match key {
                KeyCode::ArrowLeft => self.toolbar_column(false),
                KeyCode::ArrowRight => self.toolbar_column(true),
                KeyCode::ArrowUp => self.selected = self.selected.saturating_sub(1),
                KeyCode::ArrowDown => {
                    self.selected = (self.selected + 1).min(self.count(keymap, p).saturating_sub(1))
                }
                _ => {}
            }
        }
        None
    }
    fn toolbar_pointer(&mut self, point: [f64; 2], keymap: &Keymap, p: &Preferences) {
        let [_, width, _, top] = self.layout;
        if point[0] >= 20.
            && point[0] < width + 4.
            && point[1] >= top
            && point[1] < top + self.visible as f64 * 22.
        {
            let order = point[0] >= (width + 24.) / 2.;
            if order != self.toolbar_order {
                self.toolbar_column(order);
            }
            let selected = self.first + ((point[1] - top) / 22.).floor() as usize;
            if selected < self.count(keymap, p) {
                self.selected = selected;
                self.focus = None;
            }
        }
    }
}
