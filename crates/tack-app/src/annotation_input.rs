//! Semantic annotation actions, sharing the production input/selection owner.
use crate::{
    actions::{Action, ActionEvent, ActionPhase, Tool},
    annotation_tool::{Creation, NOTE_DEFAULT_SIZE, NoteEdit, StyleAction},
    image_input::ImageInput,
    input::{InputState, PhysicalEvent},
};
use tack_assets::AssetError;
use tack_core::*;
use winit::{
    event::{ElementState, Ime, WindowEvent},
    keyboard::{KeyCode, PhysicalKey},
};
const PALETTE: [Color; 8] = [
    Color([255, 198, 82, 255]),
    Color([240, 92, 104, 255]),
    Color([110, 214, 145, 255]),
    Color([102, 175, 255, 255]),
    Color([198, 141, 240, 255]),
    Color([230, 233, 239, 255]),
    Color([18, 22, 28, 255]),
    Color([252, 153, 85, 255]),
];
fn next_style(s: AnnotationStyle, action: StyleAction) -> Result<AnnotationStyle, ModelError> {
    let mut color = s.stroke();
    let mut fill = s.fill();
    let mut width = s.width();
    let mut opacity = s.opacity();
    match action {
        StyleAction::Color => {
            color =
                PALETTE[(PALETTE.iter().position(|c| *c == color).unwrap_or(0) + 1) % PALETTE.len()]
        }
        StyleAction::Fill => {
            fill = if fill.is_some() {
                None
            } else {
                Some(Color([color.0[0], color.0[1], color.0[2], 64]))
            }
        }
        StyleAction::Wider => width = (width * 1.25).min(256.),
        StyleAction::Narrower => width = (width / 1.25).max(0.1),
        StyleAction::OpacityUp => {
            opacity = Opacity::new((opacity.value() + 0.1).min(1.))
                .map_err(|_| ModelError::InvalidAnnotation)?
        }
        StyleAction::OpacityDown => {
            opacity = Opacity::new((opacity.value() - 0.1).max(0.))
                .map_err(|_| ModelError::InvalidAnnotation)?
        }
        _ => {}
    }
    AnnotationStyle::new(color, fill, width, opacity)
}
impl ImageInput {
    pub(crate) fn annotation_action(
        &mut self,
        event: ActionEvent,
        editor: &mut DocumentEditor,
    ) -> Result<bool, AssetError> {
        match event.action {
            Action::SelectTool(_) | Action::TemporaryTool(_) => {
                self.annotation.tools.apply(event)?;
                if !self.annotation.tools.tool().is_annotation() {
                    self.annotation.creation = None;
                }
                Ok(true)
            }
            Action::OpenSource | Action::RevealSource | Action::CopySourcePath
                if event.phase == ActionPhase::Invoke =>
            {
                self.pending_source = Some(event.action);
                Ok(true)
            }
            Action::AnnotationStyle(action) if event.phase == ActionPhase::Invoke => {
                let mut commands = Vec::new();
                for id in self.images.selection.ids() {
                    let Some(o) = editor.document().object(id) else {
                        continue;
                    };
                    let ObjectKind::Annotation(a) = o.kind() else {
                        continue;
                    };
                    if let AnnotationKind::Text(t) = a.kind()
                        && matches!(
                            action,
                            StyleAction::LargerText
                                | StyleAction::SmallerText
                                | StyleAction::AlignText
                        )
                    {
                        let size = match action {
                            StyleAction::LargerText => (t.font_size() * 1.25).min(256.),
                            StyleAction::SmallerText => (t.font_size() / 1.25).max(4.),
                            _ => t.font_size(),
                        };
                        let alignment = if action == StyleAction::AlignText {
                            match t.alignment() {
                                TextAlignment::Left => TextAlignment::Center,
                                TextAlignment::Center => TextAlignment::Right,
                                TextAlignment::Right => TextAlignment::Left,
                            }
                        } else {
                            t.alignment()
                        };
                        commands.push(Command::SetText {
                            object: id,
                            text: TextObject::new(t.value().to_owned(), size, alignment)?,
                        });
                    } else {
                        commands.push(Command::SetAnnotationStyle {
                            object: id,
                            style: next_style(a.style(), action)?,
                        });
                    }
                }
                editor.execute(Command::Batch(commands))?;
                self.annotation.style = next_style(self.annotation.style, action)?;
                Ok(true)
            }
            Action::RenameFrame
                if event.phase == ActionPhase::Invoke && self.images.selection.len() == 1 =>
            {
                let Some(id) = self.images.selection.ids().next() else {
                    return Ok(false);
                };
                let Some(o) = editor.document().object(id) else {
                    return Ok(false);
                };
                let ObjectKind::Annotation(a) = o.kind() else {
                    return Ok(false);
                };
                let AnnotationKind::Text(t) = a.kind() else {
                    return Ok(false);
                };
                self.annotation.edit = Some(Box::new(NoteEdit {
                    id,
                    value: t.value().to_owned(),
                    size: t.font_size(),
                    alignment: t.alignment(),
                    transform: o.transform(),
                    style: a.style(),
                    generation: editor.generation(),
                    is_new: false,
                    replace: true,
                    composing: false,
                }));
                self.reset_modal_inputs();
                Ok(true)
            }
            _ => Ok(false),
        }
    }
    pub(crate) fn reset_modal_inputs(&mut self) {
        self.state = InputState::default();
        self.snap.disabled = false;
        self.snap.clear();
    }
    pub(crate) fn begin_annotation(
        &mut self,
        p: [f64; 2],
        editor: &DocumentEditor,
        camera: &Camera,
    ) -> bool {
        let tool = self.annotation.tools.tool();
        if !tool.is_annotation() {
            return false;
        }
        self.images.selection.clear();
        self.images.crop_mode = false;
        self.annotation.creation = Some(Box::new(Creation::new(
            tool,
            p,
            self.annotation.style,
            editor.generation(),
            self.gizmo.scale * 0.75 / camera.zoom(),
        )));
        true
    }
    pub(crate) fn finish_annotation(
        &mut self,
        editor: &mut DocumentEditor,
        camera: &Camera,
    ) -> Result<(), AssetError> {
        let Some(c) = self.annotation.creation.take() else {
            return Ok(());
        };
        if c.generation != editor.generation() {
            return Err("document changed during annotation creation".into());
        }
        let distance = (c.end[0] - c.start[0]).hypot(c.end[1] - c.start[1]) * camera.zoom();
        if c.tool != Tool::Text
            && (distance < self.gizmo.scale * 2. && c.tool != Tool::Scribble
                || c.tool == Tool::Scribble && c.points.len() < 2)
        {
            return Ok(());
        }
        let id = tack_storage::new_object_id()?;
        let transform = if c.tool == Tool::Text && distance < self.gizmo.scale * 4. {
            let size = [320., 160.].map(|v| v * self.gizmo.scale / camera.zoom());
            Transform::new(
                [c.start[0] + size[0] / 2., c.start[1] + size[1] / 2.],
                size,
                0.,
                [false; 2],
            )?
        } else {
            c.transform()?
        };
        if c.tool == Tool::Text {
            self.annotation.edit = Some(Box::new(NoteEdit {
                id,
                value: String::new(),
                size: NOTE_DEFAULT_SIZE * self.gizmo.scale / camera.zoom(),
                alignment: TextAlignment::Left,
                transform,
                style: c.style,
                generation: editor.generation(),
                is_new: true,
                replace: false,
                composing: false,
            }));
            // Keep durable font sizes within admission even at extreme camera zoom.
            if let Some(e) = &mut self.annotation.edit {
                e.size = e.size.clamp(4., 256.);
            }
            self.reset_modal_inputs();
        } else {
            let a = Annotation::new(c.kind(transform)?, c.style);
            editor.execute(Command::AddObject {
                object: DocumentObject::annotation(id, a, transform)?,
                index: editor.document().object_order().len(),
            })?;
            self.images.selection.select(Some(id), false);
        }
        Ok(())
    }
    /// Editing consumes text presses; releases synchronize captured controls.
    pub(crate) fn note_event(
        &mut self,
        event: &WindowEvent,
        editor: &mut DocumentEditor,
        camera: &mut Camera,
    ) -> Result<(), AssetError> {
        for physical in crate::input::normalize(event).into_iter().flatten() {
            if matches!(
                physical,
                PhysicalEvent::Modifiers(_)
                    | PhysicalEvent::Button {
                        state: ElementState::Released,
                        ..
                    }
            ) {
                self.state.handle(physical, &self.keymap, |_| {})?;
            }
        }
        match event {
            WindowEvent::Focused(false) | WindowEvent::Occluded(true) => {
                let draft = self.annotation.edit.take();
                self.physical(PhysicalEvent::FocusLost, editor, camera)?;
                self.annotation.edit = draft;
                if let Some(edit) = &mut self.annotation.edit {
                    edit.composing = false;
                }
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                if let Some(edit) = &mut self.annotation.edit {
                    edit.composing = false;
                }
                self.gizmo.set_scale(*scale_factor);
                self.reset_modal_inputs();
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                let ctrl = self
                    .state
                    .modifiers()
                    .contains(crate::input::Modifiers::CONTROL);
                if self
                    .keymap
                    .matching(
                        crate::input::PhysicalControl::Key(event.physical_key),
                        self.state.modifiers(),
                        crate::bindings::Trigger::Press,
                    )
                    .any(|b| b.action == Action::Paste)
                {
                    self.pending_local = Some(Action::Paste);
                    return Ok(());
                }
                match event.physical_key {
                    PhysicalKey::Code(KeyCode::Escape) => self.annotation.edit = None,
                    PhysicalKey::Code(KeyCode::Enter) if ctrl => {
                        if let Some(edit) = self.annotation.edit.clone() {
                            let id = edit.id;
                            edit.finish(editor)?;
                            self.annotation.edit = None;
                            self.images.selection.select(Some(id), false);
                        }
                        self.reset_modal_inputs();
                    }
                    PhysicalKey::Code(KeyCode::Enter) => {
                        if let Some(e) = &mut self.annotation.edit {
                            e.insert("\n");
                        }
                    }
                    PhysicalKey::Code(KeyCode::Backspace) => {
                        if let Some(e) = &mut self.annotation.edit {
                            e.backspace();
                        }
                    }
                    PhysicalKey::Code(KeyCode::KeyA) if ctrl => {
                        if let Some(e) = &mut self.annotation.edit {
                            e.replace = true;
                        }
                    }
                    _ if !ctrl => {
                        if let Some(e) = &mut self.annotation.edit
                            && !e.composing
                            && let Some(text) = &event.text
                        {
                            e.insert(text);
                        }
                    }
                    _ => {}
                }
            }
            WindowEvent::Ime(Ime::Preedit(text, _)) => {
                if let Some(e) = &mut self.annotation.edit {
                    e.composing = !text.is_empty();
                }
            }
            WindowEvent::Ime(Ime::Commit(text)) => {
                if let Some(e) = &mut self.annotation.edit {
                    e.insert(text);
                    e.composing = false;
                }
            }
            _ => {}
        }
        Ok(())
    }
}
