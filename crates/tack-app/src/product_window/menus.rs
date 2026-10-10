//! Thin native routing: every menu command uses ImageInput's semantic dispatcher.
use super::*;
use tack_app::{
    actions::{ActionEvent, ActionPhase},
    context_menu::{self, Context, ContextMenu},
};
use winit::event::{ElementState, MouseButton};
impl App {
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
        let icon = if self.context.is_some() || self.local.ui.is_some() {
            winit::window::CursorIcon::Default
        } else {
            self.editor
                .as_ref()
                .map_or(winit::window::CursorIcon::Default, |e| {
                    self.input.cursor_icon(e, &self.camera)
                })
        };
        if icon != self.cursor_icon {
            if let Some(window) = &self.window {
                window.set_cursor(icon);
            }
            self.cursor_icon = icon;
        }
    }
}
