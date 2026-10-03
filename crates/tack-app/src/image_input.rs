//! One production adapter: normalized input → semantic actions → gestures/view.
use crate::{
    actions::{Action, ActionEvent, ActionPhase, HoldToken},
    bindings::{Binding, BindingError, Keymap, ModifierMatch, Trigger},
    image_gizmo::{GizmoHit, ImageGizmo},
    image_interaction::{GestureKind, ImageInteraction},
    input::{InputState, Modifiers, PhysicalControl, PhysicalEvent, PointerButton, WheelAxis},
};
use std::time::{Duration, Instant};
use tack_assets::AssetError;
use tack_core::{Camera, DocumentEditor, Transform, WorldRect};
use winit::{
    event::{MouseButton, WindowEvent},
    keyboard::{KeyCode, PhysicalKey},
};

pub fn product_keymap() -> Result<Keymap, BindingError> {
    let mut map = Keymap::default();
    let left = PhysicalControl::Pointer(PointerButton::Mouse(MouseButton::Left));
    let ctrl = Modifiers::CONTROL;
    let shift = Modifiers::SHIFT;
    let alt = Modifiers::ALT;
    for (mods, action) in [
        (Modifiers::NONE, Action::ImagePointer),
        (shift, Action::ToggleSelection),
        (ctrl, Action::RotateImage),
        (ctrl.union(shift), Action::RotateImage),
        (ctrl.union(alt), Action::ScaleImage),
        (ctrl.union(alt).union(shift), Action::AdjustOpacity),
        (alt, Action::CenterPointer),
        (alt.union(shift), Action::PanView),
    ] {
        map.bind(Binding {
            control: left,
            modifiers: ModifierMatch::Exact(mods),
            trigger: Trigger::Hold,
            action,
        })?;
    }
    map.bind(Binding {
        control: PhysicalControl::Pointer(PointerButton::Mouse(MouseButton::Middle)),
        modifiers: ModifierMatch::Any,
        trigger: Trigger::Hold,
        action: Action::PanView,
    })?;
    map.bind(Binding {
        control: PhysicalControl::Wheel(WheelAxis::Vertical),
        modifiers: ModifierMatch::Any,
        trigger: Trigger::Wheel,
        action: Action::ZoomView,
    })?;
    for (code, mods, action) in [
        (KeyCode::Escape, Modifiers::NONE, Action::CancelInteraction),
        (KeyCode::Delete, Modifiers::NONE, Action::DeleteSelection),
        (KeyCode::KeyA, ctrl, Action::SelectAll),
        (KeyCode::KeyZ, ctrl, Action::Undo),
        (KeyCode::KeyZ, ctrl.union(shift), Action::Redo),
        (KeyCode::KeyY, ctrl, Action::Redo),
        (KeyCode::KeyS, ctrl, Action::Save),
        (
            KeyCode::KeyC,
            ctrl.union(alt).union(shift),
            Action::CropMode,
        ),
        (KeyCode::KeyH, alt.union(shift), Action::FlipHorizontal),
        (KeyCode::KeyV, alt.union(shift), Action::FlipVertical),
        (KeyCode::KeyT, alt, Action::CycleFiltering),
    ] {
        map.bind(Binding {
            control: PhysicalControl::Key(PhysicalKey::Code(code)),
            modifiers: if action == Action::CancelInteraction {
                ModifierMatch::Any
            } else {
                ModifierMatch::Exact(mods)
            },
            trigger: Trigger::Press,
            action,
        })?;
    }
    Ok(map)
}
pub struct ImageInput {
    pub images: ImageInteraction,
    pub gizmo: ImageGizmo,
    pub keymap: Keymap,
    state: InputState,
    cursor: [f64; 2],
    active_token: Option<HoldToken>,
    center_handle: bool,
    marquee: Option<([f64; 2], [f64; 2], bool)>,
    click: Option<(tack_core::ObjectId, [f64; 2], Instant)>,
    last_click: Option<(tack_core::ObjectId, [f64; 2], Instant)>,
}
impl ImageInput {
    pub fn new() -> Result<Self, BindingError> {
        Ok(Self {
            images: ImageInteraction::default(),
            gizmo: ImageGizmo::default(),
            keymap: product_keymap()?,
            state: InputState::default(),
            cursor: [0.; 2],
            active_token: None,
            center_handle: false,
            marquee: None,
            click: None,
            last_click: None,
        })
    }
    pub fn cancel(&mut self) {
        self.images.cancel();
        self.active_token = None;
        self.center_handle = false;
        self.marquee = None;
        self.click = None;
    }
    pub fn cursor(&self) -> [f64; 2] {
        self.cursor
    }
    pub fn active(&self) -> bool {
        self.images.active() || self.marquee.is_some()
    }
    pub fn hover(&self, editor: &DocumentEditor, camera: &Camera) -> Option<GizmoHit> {
        self.images.frame(editor.document()).and_then(|f| {
            self.gizmo.hit(
                f,
                camera,
                self.cursor,
                self.images.crop_mode && self.images.selection.len() == 1,
                self.images.selection.len() > 1,
            )
        })
    }
    pub fn build_overlay(&mut self, editor: &DocumentEditor, camera: &Camera) {
        let hover = self.hover(editor, camera);
        self.gizmo
            .build(&self.images, editor.document(), camera, hover);
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
        let ActionEvent { action, phase } = event;
        if let ActionPhase::Cancel(token) = phase {
            if self.active_token == Some(token) {
                self.cancel();
            }
            return Ok(false);
        }
        if let ActionPhase::End(token) = phase {
            if self.active_token == Some(token) {
                self.active_token = None;
                self.images.commit(editor)?;
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
                        self.images.focus(editor.document(), camera, id)?;
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
            self.cancel();
            self.active_token = Some(token);
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
            let hit = self.images.hit(editor.document(), pointer);
            if action == Action::ToggleSelection {
                self.images.selection.select(hit, true);
            } else if let Some(id) = hit {
                if !self.images.selection.contains(id) {
                    self.images.selection.select(Some(id), false);
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
            if action == Action::Save {
                return Ok(true);
            }
            self.cancel();
            match action {
                Action::CancelInteraction => {}
                Action::Undo => {
                    editor.undo()?;
                }
                Action::Redo => {
                    editor.redo()?;
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
                Action::Filtering(value) => {
                    self.images.filtering(editor, Some(value))?;
                }
                Action::CycleFiltering => {
                    self.images.filtering(editor, None)?;
                }
                Action::CropMode => {
                    self.images.crop_mode =
                        self.images.selection.len() == 1 && !self.images.crop_mode
                }
                Action::Save => return Ok(true),
                _ => {}
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
        let mut save = false;
        for action in emitted.into_iter().flatten() {
            save |= self.dispatch(action, editor, camera)?;
        }
        if focus {
            self.cancel();
            self.last_click = None;
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
            || (self
                .state
                .is_action_held(&self.keymap, Action::CenterPointer)
                && !self.center_handle)
        {
            self.cancel();
            camera.pan(delta)?;
        } else {
            let world = camera.screen_to_world(next);
            if let Some((_, end, _)) = &mut self.marquee {
                *end = world;
            }
            // Bad geometry is ignored; preserve the last valid preview.
            let _ = self.images.update(world);
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
            }
            _ => {}
        }
        Ok(save)
    }
}
