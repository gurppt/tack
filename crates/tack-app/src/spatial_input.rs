//! Small spatial action adapter, reusing the image selection/gesture owner.
use crate::{actions::Action, image_input::ImageInput, spatial_layout};
use tack_assets::AssetError;
use tack_core::{
    Camera, Command, DocumentEditor, DocumentObject, DocumentQuery, Group, ObjectId, ObjectKind,
    Transform,
};

pub struct FrameNameEdit {
    pub id: ObjectId,
    pub value: String,
}
impl ImageInput {
    pub fn finish_name_edit(
        &mut self,
        editor: &mut DocumentEditor,
        commit: bool,
    ) -> Result<(), AssetError> {
        if commit && let Some(edit) = &self.name_edit {
            tack_core::validate_frame_name(&edit.value)?;
            editor.execute(Command::SetFrameName {
                object: edit.id,
                name: edit.value.clone(),
            })?;
        }
        self.name_edit = None;
        Ok(())
    }
    pub(crate) fn name_text(&mut self, text: &str) {
        if let Some(edit) = &mut self.name_edit {
            if text.chars().any(char::is_control) {
                return;
            }
            let len = if self.name_replace {
                0
            } else {
                edit.value.len()
            };
            if len + text.len() > tack_core::MAX_FRAME_NAME_BYTES {
                return;
            }
            if self.name_replace {
                edit.value.clear();
                self.name_replace = false;
            }
            edit.value.push_str(text);
        }
    }
    pub(crate) fn update_snapped(
        &mut self,
        world: [f64; 2],
        editor: &DocumentEditor,
        camera: &Camera,
    ) -> Result<(), tack_core::GeometryError> {
        self.images.update(world)?;
        if self.snap.disabled || (!self.snap.enabled && !self.snap.grid) {
            self.snap.clear();
            return Ok(());
        }
        let Some(frame) = self.images.frame(editor.document()) else {
            self.snap.clear();
            return Ok(());
        };
        let kind = self.images.gesture_kind();
        let resize = match kind {
            Some(crate::image_interaction::GestureKind::Resize { handle, .. })
                if frame.rotation() == 0. =>
            {
                Some(crate::image_geometry::HANDLE_DIRECTIONS[handle].map(|d| {
                    if d < 0. {
                        0
                    } else if d > 0. {
                        2
                    } else {
                        3
                    }
                }))
            }
            Some(crate::image_interaction::GestureKind::Move) => None,
            _ => {
                self.snap.clear();
                return Ok(());
            }
        };
        let query_start = self.snap.measure.then(std::time::Instant::now);
        let delta = self.snap.resolve(
            editor.document(),
            &self.images.selection,
            frame.bounds(),
            camera.zoom(),
            self.gizmo.scale,
            resize,
        );
        self.snap.last_query_ms = query_start.map(|s| s.elapsed().as_secs_f64() * 1000.);
        let mut corrected = [world[0] + delta[0], world[1] + delta[1]];
        if let Some(crate::image_interaction::GestureKind::Resize { handle, .. }) = kind {
            let d = crate::image_geometry::HANDLE_DIRECTIONS[handle];
            if d[0] != 0. && d[1] != 0. && !self.images.selected_note(editor.document()) {
                let size = frame.size();
                let denom = size[0] * size[0] + size[1] * size[1];
                let response = size.map(|s| s * s / denom);
                // Bound actual corner displacement after preserving the aspect ratio.
                // A small short-axis delta must never cause a large long-axis jump.
                let distance = std::array::from_fn::<_, 2, _>(|axis| {
                    delta[axis].abs() / response[axis].sqrt()
                });
                let tolerance = crate::spatial_snap::SNAP_PIXELS * self.gizmo.scale / camera.zoom();
                for (axis, distance) in distance.iter().enumerate() {
                    if *distance > tolerance {
                        self.snap.guides[axis] = None;
                    }
                }
                corrected = world;
                let axis = if self.snap.guides[0].is_some()
                    && (self.snap.guides[1].is_none() || distance[0] <= distance[1])
                {
                    Some(0)
                } else if self.snap.guides[1].is_some() {
                    Some(1)
                } else {
                    None
                };
                if let Some(axis) = axis {
                    corrected[axis] += delta[axis] / response[axis];
                    self.snap.guides[1 - axis] = None;
                }
            }
        }
        self.images.update(corrected)?;
        Ok(())
    }
    pub(crate) fn spatial_action(
        &mut self,
        action: Action,
        editor: &mut DocumentEditor,
        camera: &mut Camera,
    ) -> Result<(), AssetError> {
        match action {
            Action::ToggleGrid => self.grid_visible = !self.grid_visible,
            Action::ToggleSnapping => {
                self.snap.enabled = !self.snap.enabled;
                self.snap.grid = self.snap.enabled;
            }
            Action::Layout(layout) => {
                editor.execute(spatial_layout::arrange(
                    editor.document(),
                    self.images.selection.ids(),
                    layout,
                )?)?;
            }
            Action::GroupSelection => {
                if self.images.selection.ids().any(|id| {
                    editor
                        .document()
                        .object(id)
                        .is_some_and(|o| matches!(o.kind(), tack_core::ObjectKind::Annotation(_)))
                }) {
                    return Err(
                        "groups support images only; deselect annotations before grouping".into(),
                    );
                }
                let ids: Vec<_> = self
                    .images
                    .selection
                    .ids()
                    .filter(|id| editor.document().object_render_data(*id).is_some())
                    .collect();
                if ids.len() >= 2 {
                    let mut edits: Vec<_> = editor
                        .document()
                        .groups()
                        .filter(|g| {
                            g.members()
                                .iter()
                                .any(|id| self.images.selection.contains(*id))
                        })
                        .map(|g| Command::RemoveGroup(g.id()))
                        .collect();
                    edits.push(Command::AddGroup(Group::new(
                        tack_storage::new_group_id()?,
                        ids,
                    )?));
                    editor.execute(Command::Batch(edits))?;
                }
            }
            Action::UngroupSelection => {
                let edits = editor
                    .document()
                    .groups()
                    .filter(|g| {
                        g.members()
                            .iter()
                            .any(|id| self.images.selection.contains(*id))
                    })
                    .map(|g| Command::RemoveGroup(g.id()))
                    .collect();
                editor.execute(Command::Batch(edits))?;
            }
            Action::CreateFrame => {
                let view = camera.viewport();
                let bounds = spatial_layout::bounds(
                    self.images
                        .selection
                        .ids()
                        .filter_map(|id| editor.document().object(id))
                        .map(|o| o.transform()),
                );
                let (center, size) = if let Some(b) = bounds {
                    (
                        [b.x + b.width / 2., b.y + b.height / 2.],
                        [b.width + 32., b.height + 32.],
                    )
                } else {
                    (
                        [view.x + view.width / 2., view.y + view.height / 2.],
                        [view.width * 0.6, view.height * 0.6],
                    )
                };
                let id = tack_storage::new_object_id()?;
                editor.execute(Command::AddObject {
                    object: DocumentObject::frame(
                        id,
                        "Frame".to_owned(),
                        Transform::new(center, size, 0., [false; 2])?,
                    )?,
                    index: editor.document().object_order().len(),
                })?;
                self.images.selection.select(Some(id), false);
                self.annotation.tools.reset_pointer();
            }
            Action::RenameFrame => {
                if self.images.selection.len() == 1
                    && let Some(id) = self.images.selection.ids().next()
                    && let Some(object) = editor.document().object(id)
                    && let ObjectKind::Frame(name) = object.kind()
                {
                    self.name_replace = true;
                    self.name_edit = Some(FrameNameEdit {
                        id,
                        value: name.clone(),
                    });
                }
            }
            Action::FocusFrame | Action::NextFrame | Action::PreviousFrame => {
                let order = editor.document().object_order();
                let current = order.iter().position(|id| {
                    self.images.selection.contains(*id)
                        && editor
                            .document()
                            .object(*id)
                            .is_some_and(|o| matches!(o.kind(), ObjectKind::Frame(_)))
                });
                let id = if action == Action::FocusFrame {
                    current.map(|i| order[i])
                } else {
                    (0..order.len())
                        .map(|step| {
                            if action == Action::NextFrame {
                                (current.map_or(order.len().saturating_sub(1), |i| i) + step + 1)
                                    % order.len()
                            } else {
                                (current.unwrap_or(0) + order.len() - 1 - step) % order.len()
                            }
                        })
                        .find_map(|i| {
                            editor
                                .document()
                                .object(order[i])
                                .filter(|o| matches!(o.kind(), ObjectKind::Frame(_)))
                                .map(|o| o.id())
                        })
                };
                if let Some(id) = id {
                    self.images.selection.select(Some(id), false);
                    self.images.focus(editor.document(), camera, id)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    pub(crate) fn frame_hit(&self, editor: &DocumentEditor, camera: &Camera) -> Option<ObjectId> {
        if editor.document().frame_count() == 0 {
            return None;
        }
        let p = self.cursor();
        editor
            .document()
            .object_order()
            .iter()
            .rev()
            .find_map(|id| {
                let o = editor.document().object(*id)?;
                if !matches!(o.kind(), ObjectKind::Frame(_)) {
                    return None;
                }
                let t = self.images.preview_transform(editor.document(), *id)?;
                let b = t.bounds();
                let lo = camera.world_to_screen([b.x, b.y]);
                let hi = camera.world_to_screen([b.x + b.width, b.y + b.height]);
                let r = 6. * self.gizmo.scale;
                let border = p[0] >= lo[0] - r
                    && p[0] <= hi[0] + r
                    && p[1] >= lo[1] - r
                    && p[1] <= hi[1] + r
                    && ((p[0] - lo[0]).abs() < r
                        || (p[0] - hi[0]).abs() < r
                        || (p[1] - lo[1]).abs() < r
                        || (p[1] - hi[1]).abs() < r);
                let label = p[0] >= lo[0]
                    && p[0] <= lo[0] + 160. * self.gizmo.scale
                    && p[1] >= lo[1] - 20. * self.gizmo.scale
                    && p[1] <= lo[1];
                (border || label).then_some(*id)
            })
    }
}
