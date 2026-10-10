use super::*;
impl LocalUi {
    pub(super) fn scroll_event(&mut self, event: &WindowEvent) -> bool {
        if self.capture || self.confirm_reset.is_some() {
            return false;
        }
        if let WindowEvent::CursorMoved { position, .. } = event {
            self.cursor = [position.x, position.y];
        }
        let p = std::array::from_fn(|i| self.cursor[i] / self.layout[0] - self.panel_offset[i]);
        let count = if self.panel == Panel::Toolbar { 2 } else { 1 };
        for index in 0..count {
            let next = match event {
                WindowEvent::MouseWheel { delta, .. } => {
                    self.scroll[index].wheel(p, crate::input::wheel_steps(*delta)[1])
                }
                WindowEvent::MouseInput {
                    state: ElementState::Pressed,
                    button: winit::event::MouseButton::Left,
                    ..
                } if self.scroll[index].press(p) => self.scroll[index].motion(p),
                WindowEvent::CursorMoved { .. } => self.scroll[index].motion(p),
                WindowEvent::MouseInput {
                    state: ElementState::Released,
                    ..
                }
                | WindowEvent::Focused(false) => {
                    if self.scroll[index].release() && !matches!(event, WindowEvent::Focused(false))
                    {
                        return true;
                    }
                    None
                }
                _ => None,
            };
            if let Some(first) = next {
                if self.panel == Panel::Toolbar {
                    self.toolbar_first[index] = first;
                    if usize::from(self.toolbar_order) == index {
                        self.first = first;
                    }
                } else {
                    self.first = first;
                }
                self.reveal_row = false;
                return true;
            }
        }
        false
    }
}
