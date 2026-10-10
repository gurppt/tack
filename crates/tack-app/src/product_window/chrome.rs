//! Tack-owned primitive chrome, independent of the OS titlebar.
use super::*;
use tack_app::{
    actions::{Action, ActionEvent, ActionPhase},
    toolbar::{Placement, Toolbar},
};
use winit::event::{ElementState, MouseButton};
#[derive(Default)]
pub(super) struct Chrome {
    pub toolbar: Toolbar,
    pub atlas: Option<tack_assets::Decoded>,
    pub(super) status: String,
    pub(super) last_hover: Option<Action>,
    pub(super) last_state: &'static str,
}
impl App {
    pub(super) fn chrome_layout(&mut self) {
        self.chrome.toolbar.layout(
            &self.local.profile.toolbar,
            self.camera.screen_size(),
            self.camera.ui_scale(),
            self.local.profile.status_bar || self.shared.is_some() || self.offline.is_some(),
        );
        self.dirty = true;
    }
    pub(super) fn refresh_chrome(&mut self) {
        let hovered = self
            .context
            .as_ref()
            .and_then(|m| m.hovered_action())
            .or(self.chrome.toolbar.hover);
        if self.local.feedback_deadline.is_some() {
            self.chrome.status = "COPIED".into();
            return;
        }
        let state = if let Some(s) = &self.shared {
            match s.state {
                tack_shared::client::ConnectionState::Connected => "Shared board online",
                tack_shared::client::ConnectionState::Connecting
                | tack_shared::client::ConnectionState::Reconnecting => {
                    "Connecting to shared board"
                }
                _ => "Shared board offline",
            }
        } else if self.host.is_some() {
            "Sharing from this computer"
        } else if self.offline.is_some() {
            "Shared copy - read only offline"
        } else if self.editor.as_ref().is_some_and(|e| e.is_dirty()) {
            "Unsaved"
        } else {
            "Saved"
        };
        if hovered != self.chrome.last_hover || state != self.chrome.last_state {
            self.chrome.last_hover = hovered;
            self.chrome.last_state = state;
            self.chrome.status = if let Some(a) = hovered {
                format!(
                    "{}  {}  [{}]",
                    if self.shared.is_some() || self.offline.is_some() {
                        state
                    } else {
                        ""
                    },
                    a.label(),
                    tack_app::context_menu::shortcut(&self.input.keymap, a)
                )
            } else {
                state.into()
            };
        }
    }
    pub(super) fn chrome_event(&mut self, event: &WindowEvent) -> Result<bool, AssetError> {
        if self.local.ui.is_some() || self.context.is_some() {
            self.chrome.toolbar.hover = None;
            return Ok(false);
        }
        if self.input.active() {
            self.chrome.toolbar.hover = None;
            return Ok(false);
        }
        if let WindowEvent::CursorMoved { position, .. } = event {
            self.pointer = [position.x, position.y];
            if let Some(delta) = self.chrome.toolbar.drag {
                let scale = self.camera.ui_scale().round().clamp(1., 4.);
                match self.local.profile.toolbar.placement {
                    Placement::Top | Placement::Bottom => {
                        self.local.profile.toolbar.offset = ((self.pointer[0] - delta[0]) / scale)
                            .round()
                            .clamp(0., 8192.)
                            as u16;
                        self.local.profile.toolbar.offset_set = true;
                    }
                    Placement::Left | Placement::Right => {
                        self.local.profile.toolbar.offset = ((self.pointer[1] - delta[1]) / scale)
                            .round()
                            .clamp(0., 8192.)
                            as u16;
                        self.local.profile.toolbar.offset_set = true;
                    }
                    _ => {
                        self.local.profile.toolbar.floating = [
                            ((self.pointer[0] - delta[0]) / scale)
                                .round()
                                .clamp(0., 8192.) as u16,
                            ((self.pointer[1] - delta[1]) / scale)
                                .round()
                                .clamp(0., 8192.) as u16,
                        ]
                    }
                }
                self.chrome_layout();
                return Ok(true);
            }
            let hover = self.chrome.toolbar.hit(self.pointer);
            if hover != self.chrome.toolbar.hover {
                self.chrome.toolbar.hover = hover;
                self.dirty = true;
            }
        }
        if matches!(event, WindowEvent::Focused(false)) {
            self.chrome.toolbar.hover = None;
            self.chrome.toolbar.drag = None;
            self.dirty = true;
        }
        if matches!(
            event,
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Left,
                ..
            }
        ) && self.chrome.toolbar.drag.take().is_some()
        {
            self.local.profile_pending = true;
            self.local.profile_changed = true;
            return Ok(true);
        }
        if matches!(
            event,
            WindowEvent::MouseInput {
                state: ElementState::Released,
                ..
            }
        ) {
            return Ok(false);
        }
        let within = self.chrome.toolbar.contains(self.pointer);
        if within
            && matches!(
                event,
                WindowEvent::MouseInput {
                    state: ElementState::Pressed,
                    button: MouseButton::Left,
                    ..
                }
            )
        {
            if self.chrome.toolbar.grip_hit(self.pointer) {
                let b = self.chrome.toolbar.bounds;
                self.chrome.toolbar.drag = Some([self.pointer[0] - b[0], self.pointer[1] - b[1]]);
            } else if let Some(a) = self.chrome.toolbar.hit(self.pointer) {
                let enabled = self.menu_context().is_some_and(|c| c.enabled(a));
                if enabled && let Some(e) = &mut self.editor {
                    self.input.suspend_for_menu(e, &mut self.camera)?;
                    self.local.manual |= self.input.dispatch(
                        ActionEvent {
                            action: a,
                            phase: ActionPhase::Invoke,
                        },
                        e,
                        &mut self.camera,
                    )?;
                }
            }
            self.dirty = true;
            return Ok(true);
        }
        Ok(within
            && matches!(
                event,
                WindowEvent::MouseInput { .. } | WindowEvent::MouseWheel { .. }
            ))
    }
}
pub(super) fn draw(
    chrome: &Chrome,
    g: &mut tack_app::image_gizmo::ImageGizmo,
    c: &Camera,
    p: &tack_app::preferences::Preferences,
    tool: tack_app::actions::Tool,
    gpu: &mut Gpu,
    state: (bool, Option<tack_shared::client::ConnectionState>),
) -> Result<(), AssetError> {
    let (popup, shared) = state;
    let mut pixel_camera = *c;
    pixel_camera.set_ui_scale(c.ui_scale().round().clamp(1., 4.));
    let c = &pixel_camera;
    let palette = p.theme.palette();
    let scale = c.ui_scale().round().clamp(1., 4.);
    let mut icons = [tack_render::UiIcon::default(); 32];
    let mut count = 0;
    if !popup {
        let b = chrome.toolbar.bounds;
        if chrome.toolbar.count > 0 {
            g.pixel_rect(c, [b[0], b[1]], [b[2], b[3]], palette.menu_border, None);
            g.pixel_rect(
                c,
                [b[0] + scale, b[1] + scale],
                [b[2] - scale, b[3] - scale],
                palette.menu_bg,
                None,
            );
        }
        for button in &chrome.toolbar.buttons[..chrome.toolbar.count] {
            if let Some(a) = button.action {
                let r = button.rect;
                if a == Action::SelectTool(tool) || chrome.toolbar.hover == Some(a) {
                    g.pixel_rect(
                        c,
                        [r[0], r[1]],
                        [r[2], r[3]],
                        if a == Action::SelectTool(tool) {
                            palette.accent_secondary
                        } else {
                            palette.selection
                        },
                        None,
                    );
                }
                icons[count] = tack_render::UiIcon {
                    rect: [
                        r[0] + 2. * scale,
                        r[1] + 2. * scale,
                        16. * scale,
                        16. * scale,
                    ],
                    index: tack_app::toolbar_icons::index(a),
                };
                count += 1;
            }
        }
    }
    gpu.set_ui_icons(&icons[..count])?;
    let [w, h] = c.screen_size().map(f64::from);
    if p.status_bar || shared.is_some() {
        g.pixel_rect(c, [0., h - 20. * scale], [w, h], palette.menu_bg, None);
        let mut budget = 180;
        g.ui_text(
            c,
            [18. * scale, h - 17. * scale],
            w / scale - 22.,
            &chrome.status,
            palette.text_secondary,
            &mut budget,
        );
    }
    if let Some(state) = shared {
        let color = match state {
            tack_shared::client::ConnectionState::Connected => [0., 0.8, 0.08, 1.],
            tack_shared::client::ConnectionState::Connecting
            | tack_shared::client::ConnectionState::Reconnecting => palette.accent_attention,
            _ => [0.85, 0.02, 0.02, 1.],
        };
        let y = h - 14. * scale;
        g.pixel_rect(
            c,
            [5. * scale, y],
            [12. * scale, y + 7. * scale],
            color,
            None,
        );
    }
    Ok(())
}
