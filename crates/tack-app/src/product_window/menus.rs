//! Thin native routing: every menu command uses ImageInput's semantic dispatcher.
use super::*;
use tack_app::{
    actions::{ActionEvent, ActionPhase},
    context_menu::{self, Context, ContextMenu},
};
use winit::event::{ElementState, MouseButton};
impl App {
    pub(super) fn menu_access_event(&mut self, event: &WindowEvent) -> Result<bool, AssetError> {
        let WindowEvent::KeyboardInput { event, .. } = event else {
            return Ok(false);
        };
        if event.state != ElementState::Pressed
            || self.native_modifiers != tack_app::input::Modifiers::NONE
            || !(tack_app::menu_access::is_key(tack_app::input::PhysicalControl::Key(
                event.physical_key,
            )) || tack_app::input::logical_key(event).is_some_and(|key| {
                tack_app::menu_access::is_key(tack_app::input::PhysicalControl::LogicalKey(key))
            }))
        {
            return Ok(false);
        }
        if !event.repeat
            && !self.load_failed
            && !self.local.saving_as
            && !self.local.recovery_pending
            && !self
                .local
                .ui
                .as_ref()
                .is_some_and(|ui| ui.blocks_menu_access())
        {
            self.local_action(tack_app::actions::Action::ApplicationMenu)?;
        }
        self.dirty = true;
        Ok(true)
    }
    pub(super) fn invalidate_context(&mut self) {
        if self.context.as_ref().is_some_and(|m| {
            self.editor
                .as_ref()
                .is_none_or(|e| e.generation() != m.context.generation)
        }) {
            self.context = None;
            self.dirty = true;
        }
    }
    pub(super) fn menu_context(&self) -> Option<Context> {
        Some(
            Context::selection(
                self.editor.as_ref()?,
                &self.input.images.selection,
                self.input.grid_visible,
                self.input.snap.enabled,
            )
            .with_shared(self.shared.is_some() || self.offline.is_some())
            .with_hosted(self.host.is_some()),
        )
    }
    fn prepare_menu(&mut self) -> Result<(), AssetError> {
        self.release_about();
        if let Some(editor) = &mut self.editor {
            self.input.suspend_for_menu(editor, &mut self.camera)?;
            self.input
                .cursor_moved(self.pointer, editor, &mut self.camera)?;
        }
        if self
            .local
            .ui
            .as_ref()
            .is_some_and(|u| u.panel == tack_app::local_ui::Panel::UpdateChecking)
        {
            self.local.worker.cancel();
            self.local.queued = None;
        }
        self.local.ui = None;
        Ok(())
    }
    pub(super) fn open_application_menu(&mut self) -> Result<(), AssetError> {
        self.prepare_menu()?;
        if let Some(context) = self.menu_context() {
            self.context = Some(Box::new(ContextMenu::application(
                context,
                &self.camera,
                &self.input.keymap,
            )));
            if let Some(menu) = &mut self.context {
                menu.set_modifiers(self.input.modifiers());
            }
        }
        self.dirty = true;
        Ok(())
    }
    pub(super) fn context_event(&mut self, event: &WindowEvent) -> Result<bool, AssetError> {
        if let WindowEvent::CursorMoved { position, .. } = event {
            self.pointer = [position.x, position.y];
        }
        if self.context.as_ref().is_some_and(|m| {
            self.editor
                .as_ref()
                .is_none_or(|e| e.generation() != m.context.generation)
        }) {
            self.context = None;
            self.dirty = true;
            // Do not reinterpret a click on an invalidated popup as a canvas drag.
            return Ok(true);
        }
        if self.load_failed || self.local.saving_as || self.local.ui.is_some() {
            return Ok(false);
        }
        if matches!(
            event,
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Right,
                ..
            }
        ) {
            if self.context.is_none()
                && self.input.keymap.pointer_bound(
                    tack_app::input::PointerButton::Mouse(MouseButton::Right),
                    self.input.modifiers(),
                )
            {
                return Ok(false);
            }
            self.prepare_menu()?;
            if let Some(editor) = &self.editor {
                self.input.context_selection(editor, &self.camera);
                if let Some(context) = self.menu_context() {
                    self.context = Some(Box::new(ContextMenu::new(
                        context,
                        self.pointer,
                        &self.camera,
                        &self.input.keymap,
                    )));
                    if let Some(menu) = &mut self.context {
                        menu.set_modifiers(self.input.modifiers());
                    }
                }
            }
            self.dirty = true;
            return Ok(true);
        }
        let Some(menu) = &mut self.context else {
            return Ok(false);
        };
        if let Some(editor) = &mut self.editor {
            match event {
                WindowEvent::ModifiersChanged(m) => {
                    self.input.physical(
                        tack_app::input::PhysicalEvent::Modifiers(m.state().into()),
                        editor,
                        &mut self.camera,
                    )?;
                }
                WindowEvent::Focused(false) => {
                    self.input.physical(
                        tack_app::input::PhysicalEvent::FocusLost,
                        editor,
                        &mut self.camera,
                    )?;
                }
                _ => {}
            }
        }
        if matches!(event, WindowEvent::CursorMoved { .. })
            && let Some(editor) = &self.editor
        {
            self.input
                .cursor_moved(self.pointer, editor, &mut self.camera)?;
        }
        let result = menu.handle(event, &self.input.keymap, &self.camera);
        match result {
            context_menu::Result::None => {}
            context_menu::Result::Dismiss => self.context = None,
            context_menu::Result::Action(action) => {
                self.context = None;
                if self.menu_context().is_some_and(|c| c.enabled(action))
                    && let Some(editor) = &mut self.editor
                {
                    self.local.manual |= self.input.dispatch(
                        ActionEvent {
                            action,
                            phase: ActionPhase::Invoke,
                        },
                        editor,
                        &mut self.camera,
                    )?;
                }
            }
        }
        self.dirty = true;
        Ok(true)
    }
    pub(super) fn update_cursor(&mut self) {
        use tack_app::cursors::Kind;
        use winit::window::CursorIcon;
        let ui = self.context.is_some()
            || self.local.ui.is_some()
            || self.chrome.toolbar.contains_point(self.pointer)
            || self.chrome.toolbar.grip_hit(self.pointer);
        let icon = if ui {
            Kind::Pointer
        } else {
            let native = self.editor.as_ref().map_or(CursorIcon::Default, |e| {
                self.input.cursor_icon(e, &self.camera)
            });
            match native {
                CursorIcon::Grab => Kind::HandOpen,
                CursorIcon::Grabbing => Kind::HandClosed,
                CursorIcon::Text => Kind::Text,
                CursorIcon::Move => Kind::Move,
                CursorIcon::NwseResize => Kind::ResizeNwse,
                CursorIcon::NeswResize => Kind::ResizeNesw,
                CursorIcon::NsResize => Kind::ResizeNs,
                CursorIcon::EwResize => Kind::ResizeEw,
                CursorIcon::Crosshair
                    if self.editor.as_ref().is_some_and(|e| {
                        self.input.hover(e, &self.camera)
                            == Some(tack_app::image_gizmo::GizmoHit::Rotate)
                    }) =>
                {
                    Kind::Rotate
                }
                CursorIcon::Crosshair if self.input.images.crop_mode => Kind::Crop,
                CursorIcon::Crosshair
                    if self.input.annotation.tools.tool() == tack_app::actions::Tool::Scribble =>
                {
                    Kind::Draw
                }
                CursorIcon::Crosshair => Kind::Crosshair,
                _ => Kind::Pointer,
            }
        };
        if icon != self.cursor_icon {
            if let Some(window) = &self.window {
                self.cursors.set(window, icon);
            }
            self.cursor_icon = icon;
        }
    }
}
