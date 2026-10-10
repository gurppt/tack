//! One production adapter: normalized input → semantic actions → gestures/view.
use crate::{
    actions::{Action, ActionEvent, ActionPhase, HoldToken},
    bindings::{BindingError, Keymap},
    image_gizmo::{GizmoHit, ImageGizmo},
    image_interaction::{GestureKind, ImageInteraction},
    input::{InputState, PhysicalEvent},
};
use std::time::{Duration, Instant};
use tack_assets::AssetError;
use tack_core::{Camera, DocumentEditor, DocumentQuery, Transform, WorldRect};
use winit::{
    event::WindowEvent,
    keyboard::{KeyCode, PhysicalKey},
};

pub use crate::product_bindings::product_keymap;
pub struct ImageInput {
    pub images: ImageInteraction,
    pub gesture_waiting: bool,
    pub gizmo: ImageGizmo,
    pub keymap: Keymap,
    pub grid_visible: bool,
    pub snap: crate::spatial_snap::SnapState,
    pub caret: crate::feedback::Caret,
    pub edit_focused: bool,
    pub name_edit: Option<crate::spatial_input::FrameNameEdit>,
    pub(crate) name_replace: bool,
    pub annotation: crate::annotation_tool::AnnotationInput,
    pub pending_source: Option<Action>,
    pub pending_local: Option<Action>,
    pub status: String,
    pub(crate) adjust_fill: Option<tack_core::ObjectId>,
    pub(crate) state: InputState,
    input_trace: bool,
    cursor: [f64; 2],
    active_token: Option<HoldToken>,
    center_handle: bool,
    marquee: Option<([f64; 2], [f64; 2], bool)>,
    click: Option<(tack_core::ObjectId, [f64; 2], Instant)>,
    last_click: Option<(tack_core::ObjectId, [f64; 2], Instant)>,
}
impl ImageInput {
    pub fn active_tool(&self) -> crate::actions::Tool {
        self.annotation.tools.tool()
    }
    pub fn new() -> Result<Self, BindingError> {
        Ok(Self {
            images: ImageInteraction::default(),
            gesture_waiting: false,
            gizmo: ImageGizmo::default(),
            keymap: product_keymap()?,
            grid_visible: false,
            snap: crate::spatial_snap::SnapState::default(),
            caret: Default::default(),
            edit_focused: true,
            name_edit: None,
            name_replace: true,
            annotation: Default::default(),
            pending_source: None,
            pending_local: None,
            status: String::new(),
            adjust_fill: None,
            state: InputState::default(),
            input_trace: std::env::var_os("TACK_TRACE_INPUT").is_some(),
            cursor: [0.; 2],
            active_token: None,
            center_handle: false,
            marquee: None,
            click: None,
            last_click: None,
        })
    }
    /// Save/close explicitly commit bounded text drafts; failure retains the draft.
    pub fn commit_drafts(&mut self, editor: &mut DocumentEditor) -> Result<(), AssetError> {
        if let Some(edit) = &self.annotation.edit {
            let id = edit.id;
            edit.as_ref().clone().finish(editor)?;
            let is_new = edit.is_new;
            self.annotation.edit = None;
            if is_new {
                self.annotation
                    .tools
                    .complete_creation(crate::actions::Tool::Text);
            }
            self.images.selection.select(Some(id), false);
        }
        self.finish_name_edit(editor, true)?;
        Ok(())
    }
    pub fn cancel(&mut self) {
        self.gesture_waiting = false;
        if let Some(c) = &self.annotation.creation {
            self.annotation.tools.complete_creation(c.tool);
        }
        if self.annotation.edit.as_ref().is_some_and(|e| e.is_new) {
            self.annotation
                .tools
                .complete_creation(crate::actions::Tool::Text);
        }
        self.annotation.creation = None;
        self.annotation.edit = None;
        self.images.cancel();
        self.snap.clear();
        self.active_token = None;
        self.center_handle = false;
        self.marquee = None;
        self.click = None;
    }
    pub fn cursor(&self) -> [f64; 2] {
        self.cursor
    }
    pub fn modifiers(&self) -> crate::input::Modifiers {
        self.state.modifiers()
    }
    /// A popup ends captured canvas gestures, without forgetting held modifiers.
    pub fn suspend_for_menu(
        &mut self,
        editor: &mut DocumentEditor,
        camera: &mut Camera,
    ) -> Result<(), AssetError> {
        let modifiers = self.modifiers();
        self.commit_drafts(editor)?;
        self.physical(PhysicalEvent::FocusLost, editor, camera)?;
        self.physical(PhysicalEvent::Modifiers(modifiers), editor, camera)?;
        Ok(())
    }
    pub fn cursor_icon(
        &self,
        editor: &DocumentEditor,
        camera: &Camera,
    ) -> winit::window::CursorIcon {
        use winit::window::CursorIcon;
        if self.annotation.edit.is_some() || self.name_edit.is_some() {
            return CursorIcon::Text;
        }
        if self.annotation.tools.tool().is_annotation() {
            return CursorIcon::Crosshair;
        }
        if self.annotation.tools.tool() == crate::actions::Tool::Pan {
            return CursorIcon::Grab;
        }
        match self.hover(editor, camera) {
            Some(GizmoHit::Rotate) => CursorIcon::Crosshair,
            Some(GizmoHit::Resize(_)) if self.images.crop_mode => CursorIcon::Crosshair,
            Some(GizmoHit::Resize(i)) => match i % 4 {
                0 => CursorIcon::NwseResize,
                1 => CursorIcon::NsResize,
                2 => CursorIcon::NeswResize,
                _ => CursorIcon::EwResize,
            },
            None if self.images.active() => CursorIcon::Grabbing,
            None if self.images.selection.ids().any(|id| {
                editor.document().object(id).is_some_and(|o| {
                    crate::image_geometry::hit(o.transform(), camera.screen_to_world(self.cursor))
                })
            }) =>
            {
                CursorIcon::Move
            }
            None => CursorIcon::Default,
        }
    }
    /// Right click targets the same geometry as left click, preserving a selected
    /// group/multiselection and clearing selection on genuinely empty canvas.
    pub fn context_selection(&mut self, editor: &DocumentEditor, camera: &Camera) {
        let hit = self
            .images
            .hit_with_tolerance(
                editor.document(),
                camera.screen_to_world(self.cursor),
                6. * self.gizmo.scale / camera.zoom(),
            )
            .or_else(|| self.frame_hit(editor, camera));
        if !hit.is_some_and(|id| self.images.selection.contains(id)) {
            self.images
                .selection
                .select_object(editor.document(), hit, false);
        }
        self.images.selection.prune(editor.document());
        if self.images.selection.len() != 1 {
            self.images.crop_mode = false;
        }
    }
    pub fn active(&self) -> bool {
        self.images.active()
            || self.marquee.is_some()
            || self.annotation.creation.is_some()
            || self.annotation.edit.is_some()
    }
    pub fn hover(&self, editor: &DocumentEditor, camera: &Camera) -> Option<GizmoHit> {
        let frame_selected = editor.document().frame_count() > 0
            && self.images.selection.ids().any(|id| {
                editor
                    .document()
                    .object(id)
                    .is_some_and(|o| matches!(o.kind(), tack_core::ObjectKind::Frame(_)))
            });
        self.images.frame(editor.document()).and_then(|f| {
            self.gizmo
                .hit(
                    f,
                    camera,
                    self.cursor,
                    self.images.crop_mode && self.images.selection.len() == 1,
                    self.images.selection.len() > 1,
                )
                .filter(|h| !frame_selected || *h != GizmoHit::Rotate)
        })
    }
    pub fn build_overlay(&mut self, editor: &DocumentEditor, camera: &Camera) {
        let hover = self.hover(editor, camera);
        self.gizmo
            .build(&self.images, editor.document(), camera, hover);
        self.spatial_overlay(editor, camera);
        if self.images.selection.annotations_locked {
            self.gizmo.label(
                camera,
                [56. * self.gizmo.scale, 28. * self.gizmo.scale],
                "ANNOTATIONS LOCKED",
                &mut 100,
            );
        }
        if let Some(c) = &self.annotation.creation
            && c.tool != crate::actions::Tool::Scribble
            && let Ok(t) = c.transform()
        {
            self.gizmo.outline(t, camera, self.gizmo.style.selection);
        }
        if self.annotation.tools.tool().is_annotation() || self.annotation.edit.is_some() {
            let label = if self.annotation.edit.is_some() {
                "Enter: done  Shift+Enter: newline"
            } else {
                self.annotation.tools.tool().label()
            };
            self.gizmo.label(
                camera,
                [56. * self.gizmo.scale, 8. * self.gizmo.scale],
                label,
                &mut 32,
            );
        }
        if let Some((start, end, _)) = self.marquee {
            let size = [(end[0] - start[0]).abs(), (end[1] - start[1]).abs()];
            if let Ok(frame) = Transform::new(
                [(start[0] + end[0]) / 2., (start[1] + end[1]) / 2.],
                size,
                0.,
                [false; 2],
            ) {
                self.gizmo
                    .outline(frame, camera, self.gizmo.style.selection);
            }
        }
    }
    /// Native and scripted events share exactly this semantic dispatch.
    /// Returns true only when the owner should start an asynchronous Save.
    pub fn dispatch(
        &mut self,
        event: ActionEvent,
        editor: &mut DocumentEditor,
        camera: &mut Camera,
    ) -> Result<bool, AssetError> {
        if self.input_trace {
            eprintln!(
                "TACK_INPUT dispatch={event:?} generation={} undo={} redo={}",
                editor.generation(),
                editor.undo_len(),
                editor.redo_len()
            );
        }
        let ActionEvent { action, phase } = event;
        if phase == ActionPhase::Invoke {
            self.status.clear();
        }
        if matches!(action, Action::TemporaryTool(_)) {
            self.annotation_action(event, editor)?;
            return Ok(false);
        }
        if action == Action::SnapDisable {
            self.snap.disabled = matches!(phase, ActionPhase::Begin(_));
            self.snap.clear();
            self.cursor_moved(self.cursor, editor, camera)?;
            return Ok(false);
        }
        if let ActionPhase::Cancel(token) = phase {
            if self.active_token == Some(token) {
                self.cancel();
            }
            return Ok(false);
        }
        if let ActionPhase::End(token) = phase {
            if self.active_token == Some(token) {
                self.active_token = None;
                self.finish_annotation(editor, camera)?;
                if self.gesture_waiting {
                    self.images.cancel();
                } else {
                    self.images.commit(editor)?;
                }
                self.snap.clear();
                if let Some((start, end, additive)) = self.marquee.take()
                    && let Ok(rect) = WorldRect::new(
                        start[0].min(end[0]),
                        start[1].min(end[1]),
                        (end[0] - start[0]).abs(),
                        (end[1] - start[1]).abs(),
                    )
                {
                    self.images
                        .selection
                        .marquee(editor.document(), rect, additive);
                }
                if let Some((id, p, time)) = self.click.take() {
                    if self.last_click.is_some_and(|(old, point, last)| {
                        old == id
                            && time.duration_since(last) < Duration::from_millis(400)
                            && (p[0] - point[0]).hypot(p[1] - point[1]) < self.gizmo.scale * 5.
                    }) {
                        if self.frame_title_hit(editor, camera, id) {
                            self.dispatch(
                                ActionEvent {
                                    action: Action::RenameFrame,
                                    phase: ActionPhase::Invoke,
                                },
                                editor,
                                camera,
                            )?;
                        } else if !self.annotation_action(
                            ActionEvent {
                                action: Action::RenameFrame,
                                phase: ActionPhase::Invoke,
                            },
                            editor,
                        )? {
                            self.images.focus(editor.document(), camera, id)?;
                        }
                        self.last_click = None;
                    } else {
                        self.last_click = Some((id, p, time));
                    }
                }
                self.center_handle = false;
            }
            return Ok(false);
        }
        if let ActionPhase::Delta(steps) = phase {
            if action == Action::ZoomView {
                self.cancel();
                camera.zoom_at(self.cursor, (steps.clamp(-20., 20.) * 0.15).exp())?;
            }
            return Ok(false);
        }
        if let ActionPhase::Begin(token) = phase {
            self.adjust_fill = None;
            let pointer = camera.screen_to_world(self.cursor);
            let handle = self.hover(editor, camera);
            if matches!(action, Action::PanView | Action::CenterPointer) {
                self.cancel();
                if action == Action::CenterPointer
                    && !self.state.is_action_held(&self.keymap, Action::PanView)
                    && !self.images.crop_mode
                    && let Some(GizmoHit::Resize(handle)) = handle
                {
                    self.images.begin(
                        GestureKind::Resize {
                            handle,
                            center: true,
                        },
                        pointer,
                        editor,
                    )?;
                    self.active_token = Some(token);
                    self.center_handle = true;
                }
                return Ok(false);
            }
            if !matches!(
                action,
                Action::ImagePointer
                    | Action::ToggleSelection
                    | Action::RotateImage
                    | Action::ScaleImage
                    | Action::AdjustOpacity
            ) {
                return Ok(false);
            }
            if self.state.is_action_held(&self.keymap, Action::PanView) {
                self.cancel();
                return Ok(false);
            }
            if action == Action::ImagePointer
                && self.annotation.tools.tool() == crate::actions::Tool::Pan
            {
                self.cancel();
                return Ok(false);
            }
            self.cancel();
            self.active_token = Some(token);
            if action == Action::ImagePointer && self.begin_annotation(pointer, editor, camera) {
                return Ok(false);
            }
            if matches!(action, Action::ImagePointer | Action::ToggleSelection)
                && self.modifiers().contains(crate::input::Modifiers::SHIFT)
                && self.images.selected_note(editor.document())
                && let Some(GizmoHit::Resize(handle)) = handle
            {
                let kind = if handle % 2 == 0 {
                    GestureKind::NoteScale { handle }
                } else {
                    GestureKind::Resize {
                        handle,
                        center: false,
                    }
                };
                self.images.begin(kind, pointer, editor)?;
                return Ok(false);
            }
            if action == Action::ImagePointer
                && let Some(handle) = handle
            {
                let kind = match handle {
                    GizmoHit::Rotate => GestureKind::Rotate,
                    GizmoHit::Resize(handle) => {
                        if self.images.crop_mode && self.images.selection.len() == 1 {
                            GestureKind::Crop { handle }
                        } else {
                            GestureKind::Resize {
                                handle,
                                center: false,
                            }
                        }
                    }
                };
                self.images.begin(kind, pointer, editor)?;
                return Ok(false);
            }
            let hit = self
                .images
                .hit_with_tolerance(
                    editor.document(),
                    pointer,
                    6. * self.gizmo.scale / camera.zoom(),
                )
                .or_else(|| self.frame_hit(editor, camera));
            if action == Action::ToggleSelection {
                self.images
                    .selection
                    .select_object(editor.document(), hit, true);
            } else if let Some(id) = hit {
                if !self.images.selection.contains(id) {
                    self.images
                        .selection
                        .select_object(editor.document(), Some(id), false);
                }
            } else {
                self.images.selection.clear();
            }
            if self.images.selection.len() != 1 {
                self.images.crop_mode = false;
            }
            if action == Action::ImagePointer
                && let Some(id) = hit
            {
                self.click = Some((id, self.cursor, Instant::now()));
            }
            if hit.is_none() && matches!(action, Action::ImagePointer | Action::ToggleSelection) {
                self.marquee = Some((pointer, pointer, action == Action::ToggleSelection));
            } else if hit.is_some_and(|id| self.images.selection.contains(id)) {
                let kind = match action {
                    Action::RotateImage => GestureKind::Rotate,
                    Action::ScaleImage => GestureKind::Scale,
                    Action::AdjustOpacity => GestureKind::Opacity,
                    _ => GestureKind::Move,
                };
                self.images.begin(kind, pointer, editor)?;
            }
            return Ok(false);
        }
        if phase == ActionPhase::Invoke {
            if action.is_local() {
                self.pending_local = Some(action);
                return Ok(false);
            }
            if action == Action::Save {
                return Ok(true);
            }
            self.cancel();
            if self.annotation_action(event, editor)? {
                self.images.selection.prune(editor.document());
                return Ok(false);
            }
            match action {
                Action::CancelInteraction => self.annotation.tools.reset_pointer(),
                Action::Undo => {
                    editor.undo()?;
                }
                Action::Redo => {
                    editor.redo()?;
                }
                Action::ToggleAnnotationSelectionLock => {
                    self.images.selection.annotations_locked =
                        !self.images.selection.annotations_locked;
                    self.images.selection.prune(editor.document());
                }
                Action::SelectAll => self.images.selection.select_all(editor.document()),
                Action::DeleteSelection => {
                    self.images.delete(editor)?;
                }
                Action::FlipHorizontal => {
                    self.images.flip(editor, 0)?;
                }
                Action::FlipVertical => {
                    self.images.flip(editor, 1)?;
                }
                Action::Order(direction) => {
                    editor.execute(crate::selection_commands::order(
                        editor.document(),
                        self.images.selection.ids(),
                        direction,
                    ))?;
                }
                Action::Opacity(alpha) => {
                    let commands = self
                        .images
                        .selection
                        .ids()
                        .filter(|id| editor.document().object_render_data(*id).is_some())
                        .map(|object| {
                            Ok(tack_core::Command::SetOpacity {
                                object,
                                opacity: tack_core::Opacity::new(alpha.value())?,
                            })
                        })
                        .collect::<Result<Vec<_>, tack_core::GeometryError>>()?;
                    editor.execute(tack_core::Command::Batch(commands))?;
                }
                Action::Filtering(value) => {
                    self.images.filtering(editor, Some(value))?;
                }
                Action::CycleFiltering => {
                    self.images.filtering(editor, None)?;
                }
                Action::CropMode => {
                    self.images.crop_mode =
                        self.images.selection.len() == 1
                            && self.images.selection.ids().next().is_some_and(|id| {
                                editor.document().object_render_data(id).is_some()
                            })
                            && !self.images.crop_mode
                }
                Action::ResetAspectRatio => self.reset_aspect_ratio(editor)?,
                Action::Save => return Ok(true),
                _ => self.spatial_action(action, editor, camera)?,
            }
            if self.name_edit.is_some() {
                self.state = InputState::default();
                self.snap.disabled = false;
            }
            self.images.selection.prune(editor.document());
            if self.images.selection.len() != 1 {
                self.images.crop_mode = false;
            }
        }
        Ok(false)
    }
    pub fn physical(
        &mut self,
        event: PhysicalEvent,
        editor: &mut DocumentEditor,
        camera: &mut Camera,
    ) -> Result<bool, AssetError> {
        let focus = event == PhysicalEvent::FocusLost;
        let mut emitted = [None; crate::input::MAX_HELD_INPUTS];
        let mut count = 0;
        self.state.handle(event, &self.keymap, |e| {
            if count < emitted.len() {
                emitted[count] = Some(e);
                count += 1;
            }
        })?;
        if focus {
            self.cancel();
        }
        let mut save = false;
        for action in emitted.into_iter().flatten() {
            let pointer_begin = matches!(
                event,
                PhysicalEvent::Button {
                    control: crate::input::PhysicalControl::Pointer(_),
                    state: winit::event::ElementState::Pressed,
                    ..
                }
            );
            if matches!(action.action, Action::TemporaryTool(_)) {
                if let ActionPhase::End(token) | ActionPhase::Cancel(token) = action.phase
                    && self.active_token == Some(token)
                {
                    self.dispatch(
                        ActionEvent {
                            action: Action::ImagePointer,
                            phase: action.phase,
                        },
                        editor,
                        camera,
                    )?;
                }
                save |= self.dispatch(action, editor, camera)?;
                if pointer_begin && matches!(action.phase, ActionPhase::Begin(_)) {
                    save |= self.dispatch(
                        ActionEvent {
                            action: Action::ImagePointer,
                            phase: action.phase,
                        },
                        editor,
                        camera,
                    )?;
                }
            } else {
                save |= self.dispatch(action, editor, camera)?;
            }
        }
        if focus {
            self.cancel();
            self.last_click = None;
            self.name_edit = None;
        }
        Ok(save)
    }
    pub fn cursor_moved(
        &mut self,
        next: [f64; 2],
        editor: &DocumentEditor,
        camera: &mut Camera,
    ) -> Result<(), AssetError> {
        if !next.into_iter().all(f64::is_finite) {
            return Ok(());
        }
        let delta = [next[0] - self.cursor[0], next[1] - self.cursor[1]];
        self.cursor = next;
        if self
            .click
            .is_some_and(|(_, p, _)| (p[0] - next[0]).hypot(p[1] - next[1]) > 4. * self.gizmo.scale)
        {
            self.click = None;
        }
        if self.state.is_action_held(&self.keymap, Action::PanView)
            || (self.annotation.tools.tool() == crate::actions::Tool::Pan
                && self
                    .state
                    .is_action_held(&self.keymap, Action::ImagePointer)
                || self.annotation.tools.tool() == crate::actions::Tool::Pan
                    && self.state.pointer_tool_held(crate::actions::Tool::Pan))
            || (self
                .state
                .is_action_held(&self.keymap, Action::CenterPointer)
                && !self.center_handle)
        {
            self.cancel();
            camera.pan(delta)?;
        } else {
            let world = camera.screen_to_world(next);
            if let Some(c) = &mut self.annotation.creation {
                c.update(world);
            }
            if let Some((_, end, _)) = &mut self.marquee {
                *end = world;
            }
            // Bad geometry is ignored; preserve the last valid preview.
            if !self.gesture_waiting {
                let _ = self.update_snapped(world, editor, camera);
            }
        }
        let _ = editor;
        Ok(())
    }
    pub fn handle(
        &mut self,
        event: &WindowEvent,
        editor: &mut DocumentEditor,
        camera: &mut Camera,
    ) -> Result<bool, AssetError> {
        if self.input_trace
            && let WindowEvent::KeyboardInput { event: key, .. } = event
        {
            eprintln!(
                "TACK_INPUT physical={:?} logical={:?} modifiers={:?} normalized={:?} repeat={}",
                key.physical_key,
                key.logical_key,
                self.state.modifiers(),
                crate::input::normalize(event),
                key.repeat
            );
        }
        let save_pressed = matches!(event, WindowEvent::KeyboardInput {event, ..} if event.state == winit::event::ElementState::Pressed && !event.repeat
            && self.keymap.matching(self.keymap.keyboard_control(event.physical_key, crate::input::logical_key(event), self.state.modifiers()), self.state.modifiers(), crate::bindings::Trigger::Press).any(|b| b.action == Action::Save));
        if save_pressed && (self.annotation.edit.is_some() || self.name_edit.is_some()) {
            self.commit_drafts(editor)?;
            return Ok(true);
        }
        if self.annotation.edit.is_some() {
            self.note_event(event, editor, camera)?;
            return Ok(false);
        }
        if self.name_edit.is_some() {
            // Modal text consumes presses, but releases/modifiers must not linger.
            for physical in crate::input::normalize(event).into_iter().flatten() {
                if matches!(
                    physical,
                    PhysicalEvent::Modifiers(_)
                        | PhysicalEvent::Keyboard {
                            state: winit::event::ElementState::Released,
                            ..
                        }
                        | PhysicalEvent::Button {
                            state: winit::event::ElementState::Released,
                            ..
                        }
                ) {
                    self.state.handle(physical, &self.keymap, |_| {})?;
                }
            }
            match event {
                WindowEvent::CursorMoved { position, .. } => {
                    self.cursor = [position.x, position.y];
                }

                WindowEvent::Focused(false) => {
                    let draft = self.name_edit.take();
                    self.physical(PhysicalEvent::FocusLost, editor, camera)?;
                    self.name_edit = draft;
                }
                WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                    self.gizmo.set_scale(*scale_factor);
                    camera.set_ui_scale(*scale_factor);
                    self.snap.clear();
                }
                WindowEvent::KeyboardInput { event, .. }
                    if event.state == winit::event::ElementState::Pressed =>
                {
                    match event.physical_key {
                        PhysicalKey::Code(KeyCode::Escape) => self.name_edit = None,
                        PhysicalKey::Code(KeyCode::Enter) => {
                            self.finish_name_edit(editor, true)?;
                        }
                        PhysicalKey::Code(KeyCode::Backspace) => {
                            if let Some(edit) = &mut self.name_edit {
                                if self.name_replace {
                                    edit.value.clear();
                                } else {
                                    edit.value.pop();
                                }
                            }
                            self.name_replace = false;
                        }
                        _ => {
                            if let Some(text) = &event.text {
                                self.name_text(text);
                            }
                        }
                    }
                }
                WindowEvent::MouseInput {
                    state: winit::event::ElementState::Pressed,
                    button: winit::event::MouseButton::Left,
                    ..
                } if self
                    .name_edit
                    .as_ref()
                    .is_some_and(|edit| !self.frame_title_hit(editor, camera, edit.id)) =>
                {
                    self.finish_name_edit(editor, true)?;
                }
                WindowEvent::Ime(winit::event::Ime::Commit(text)) => self.name_text(text),
                _ => {}
            }
            return Ok(false);
        }
        self.name_replace = true;
        let mut save = false;
        for physical in crate::input::normalize(event).into_iter().flatten() {
            save |= self.physical(physical, editor, camera)?;
        }
        match event {
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor_moved([position.x, position.y], editor, camera)?
            }
            WindowEvent::CursorLeft { .. }
            | WindowEvent::Occluded(true)
            | WindowEvent::Resized(_) => self.cancel(),
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                self.cancel();
                self.gizmo.set_scale(*scale_factor);
                camera.set_ui_scale(*scale_factor);
            }
            _ => {}
        }
        Ok(save)
    }
}
