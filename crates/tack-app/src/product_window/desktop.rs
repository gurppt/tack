//! Native desktop actions and bounded event routing. No position loop.
use super::*;
use tack_app::actions::Action;
use winit::{
    event::{ElementState, MouseButton},
    raw_window_handle::{HasDisplayHandle, RawDisplayHandle},
    window::{ResizeDirection, WindowLevel},
};
impl App {
    pub(super) fn wayland(&self) -> bool {
        self.window.as_ref().is_some_and(|w| {
            matches!(
                w.display_handle().map(|h| h.as_raw()),
                Ok(RawDisplayHandle::Wayland(_))
            )
        })
    }
    pub(super) fn desktop_preferences(&mut self) {
        let native = self.local.profile.native_title_bar;
        let top = self.local.profile.always_on_top && !self.wayland();
        if self.chrome.desktop.applied != Some((native, top))
            && let Some(window) = &self.window
        {
            window.set_decorations(native);
            window.set_window_level(if top {
                WindowLevel::AlwaysOnTop
            } else {
                WindowLevel::Normal
            });
            self.chrome.desktop.applied = Some((native, top));
            self.chrome.desktop.visible = false;
        }
    }
    pub(super) fn desktop_action(&mut self, action: Action) -> Result<(), AssetError> {
        match action {
            Action::ToggleAlwaysOnTop => {
                if self.wayland() {
                    return Err(
                        "Always on Top is unavailable on Wayland; use compositor window rules"
                            .into(),
                    );
                }
                self.local.profile.always_on_top = !self.local.profile.always_on_top;
                self.local.profile_pending = true;
            }
            Action::ToggleNativeTitleBar => {
                self.local.profile.native_title_bar = !self.local.profile.native_title_bar;
                self.local.profile_pending = true;
            }
            Action::MinimizeWindow => {
                if let Some(w) = &self.window {
                    w.set_minimized(true);
                }
            }
            Action::MaximizeWindow => {
                if let Some(w) = &self.window {
                    w.set_maximized(!w.is_maximized());
                }
            }
            _ => {}
        }
        self.desktop_preferences();
        self.dirty = true;
        Ok(())
    }
    pub(super) fn desktop_event(
        &mut self,
        e: &ActiveEventLoop,
        event: &WindowEvent,
    ) -> Result<bool, AssetError> {
        let Some(window) = self.window.clone() else {
            return Ok(false);
        };
        let scale = self.camera.ui_scale();
        // Keep the controls accessible when a Wayland compositor supplies no server decorations.
        let compact = !self.local.profile.native_title_bar || self.wayland();
        if matches!(
            event,
            WindowEvent::Focused(false)
                | WindowEvent::Occluded(true)
                | WindowEvent::CursorLeft { .. }
        ) {
            self.chrome.desktop.right_start = None;
            self.chrome.desktop.pressed = None;
            self.dirty |= self.chrome.desktop.visible;
            self.chrome.desktop.visible = false;
        }
        if matches!(event, WindowEvent::CursorMoved { .. }) {
            self.dirty |= self.chrome.desktop.hover(self.pointer, scale, compact);
            if self.chrome.desktop.right_start.is_some() {
                if !self.chrome.desktop.right_dragged
                    && self.chrome.desktop.moved(self.pointer, scale)
                {
                    self.chrome.desktop.right_dragged = true;
                    self.input.cancel();
                    if self.wayland() {
                        window.drag_window()?;
                    } else {
                        self.input.status =
                            "Use left drag on the top-edge chrome to move this window".into();
                    }
                }
                return Ok(true);
            }
        }
        if let WindowEvent::MouseInput { state, button, .. } = event {
            let hit = self.chrome.desktop.button(
                self.pointer,
                f64::from(self.camera.screen_size()[0]),
                scale,
            );
            if *button == MouseButton::Left {
                if *state == ElementState::Released
                    && let Some(pressed) = self.chrome.desktop.pressed.take()
                {
                    if hit == Some(pressed) {
                        match pressed {
                            0 => self.open_application_menu()?,
                            1 => self.desktop_action(Action::MinimizeWindow)?,
                            2 => self.desktop_action(Action::MaximizeWindow)?,
                            _ => self.handle_window_event(
                                e,
                                window.id(),
                                WindowEvent::CloseRequested,
                            ),
                        }
                    }
                    return Ok(true);
                }
                if *state == ElementState::Pressed && compact {
                    if let Some(hit) = hit {
                        self.chrome.desktop.pressed = Some(hit);
                        return Ok(true);
                    }
                    if let Some(edge) = resize_edge(self.pointer, self.camera.screen_size(), scale)
                    {
                        self.input.cancel();
                        window.drag_resize_window(edge)?;
                        return Ok(true);
                    }
                    if self.chrome.desktop.visible && self.pointer[1] < 24. * scale {
                        self.input.cancel();
                        window.drag_window()?;
                        return Ok(true);
                    }
                }
            }
            if *button == MouseButton::Right {
                if *state == ElementState::Released
                    && self.chrome.desktop.right_start.take().is_some()
                {
                    if !self.chrome.desktop.right_dragged {
                        self.open_context_menu()?;
                    }
                    self.chrome.desktop.right_dragged = false;
                    return Ok(true);
                }
                if *state == ElementState::Pressed
                    && self.context.is_none()
                    && self.local.ui.is_none()
                    && !self.input.active()
                    && !self.load_failed
                    && self.mouse_canvas()
                    && self.native_modifiers == tack_app::input::Modifiers::NONE
                    && !self.input.keymap.pointer_bound(
                        tack_app::input::PointerButton::Mouse(MouseButton::Right),
                        self.native_modifiers,
                    )
                    && self.editor.as_ref().is_some_and(|ed| {
                        self.input
                            .empty_canvas_at(ed.document(), &self.camera, self.pointer)
                    })
                {
                    self.chrome.desktop.right_start = Some(self.pointer);
                    self.chrome.desktop.right_dragged = false;
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }
}
fn resize_edge(p: [f64; 2], size: [u32; 2], scale: f64) -> Option<ResizeDirection> {
    let b = 3. * scale;
    let x = if p[0] < b {
        -1
    } else if p[0] >= f64::from(size[0]) - b {
        1
    } else {
        0
    };
    let y = if p[1] < b {
        -1
    } else if p[1] >= f64::from(size[1]) - b {
        1
    } else {
        0
    };
    Some(match (x, y) {
        (-1, -1) => ResizeDirection::NorthWest,
        (0, -1) => ResizeDirection::North,
        (1, -1) => ResizeDirection::NorthEast,
        (-1, 0) => ResizeDirection::West,
        (1, 0) => ResizeDirection::East,
        (-1, 1) => ResizeDirection::SouthWest,
        (0, 1) => ResizeDirection::South,
        (1, 1) => ResizeDirection::SouthEast,
        _ => return None,
    })
}
pub(super) fn draw(
    chrome: &tack_app::desktop::Chrome,
    g: &mut tack_app::image_gizmo::ImageGizmo,
    c: &Camera,
    p: &tack_app::preferences::Preferences,
) {
    if !chrome.visible {
        return;
    }
    let s = c.ui_scale();
    let w = f64::from(c.screen_size()[0]);
    let palette = p.theme.palette();
    g.pixel_rect(c, [0., 0.], [w, 24. * s], palette.menu_bg, None);
    let mut budget = 80;
    for (i, label) in ["T", "-", "[]", "X"].iter().enumerate() {
        let x = if i == 0 {
            4. * s
        } else {
            w - 76. * s + (i - 1) as f64 * 24. * s
        };
        g.ui_text(
            c,
            [x + 4. * s, 4. * s],
            24.,
            label,
            palette.accent_primary,
            &mut budget,
        );
    }
    g.ui_text(
        c,
        [40. * s, 4. * s],
        ((w / s) - 124.).max(0.),
        "Tack",
        palette.text_secondary,
        &mut budget,
    );
}
