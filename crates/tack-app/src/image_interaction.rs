//! Transient selection and gesture preview; durable edits go through DocumentEditor.
use crate::image_geometry as geometry;
use std::collections::BTreeSet;
#[path = "image_gesture.rs"]
mod gesture;
use gesture::Gesture;
use tack_core::{
    AnnotationKind, AnnotationStyle, Camera, Command, CommandError, Document, DocumentEditor,
    DocumentQuery, GeometryError, ImageFiltering, ImageRenderData, ObjectId, ObjectKind, Opacity,
    Transform, WorldRect,
};

#[derive(Default)]
pub struct SelectionState {
    ids: BTreeSet<ObjectId>,
}
impl SelectionState {
    pub fn ids(&self) -> impl Iterator<Item = ObjectId> + '_ {
        self.ids.iter().copied()
    }
    pub fn len(&self) -> usize {
        self.ids.len()
    }
    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }
    pub fn contains(&self, id: ObjectId) -> bool {
        self.ids.contains(&id)
    }
    pub fn clear(&mut self) {
        self.ids.clear();
    }
    pub fn select_all(&mut self, doc: &Document) {
        self.ids = doc.object_order().iter().copied().collect();
    }
    pub fn select(&mut self, id: Option<ObjectId>, toggle: bool) {
        if toggle {
            if let Some(id) = id
                && !self.ids.remove(&id)
            {
                self.ids.insert(id);
            }
        } else {
            self.ids.clear();
            if let Some(id) = id {
                self.ids.insert(id);
            }
        }
    }
    pub fn select_object(&mut self, doc: &Document, id: Option<ObjectId>, toggle: bool) {
        if !toggle {
            self.clear();
        }
        if let Some(id) = id {
            if let Some(group) = doc.group_for(id) {
                let remove = toggle && group.members().iter().all(|id| self.contains(*id));
                for id in group.members() {
                    if remove {
                        self.ids.remove(id);
                    } else {
                        self.ids.insert(*id);
                    }
                }
            } else {
                self.select(Some(id), toggle);
            }
        }
    }
    pub fn expand_groups(&mut self, doc: &Document) {
        for group in doc.groups() {
            if group.members().iter().any(|id| self.contains(*id)) {
                self.ids.extend(group.members());
            }
        }
    }
    pub fn prune(&mut self, doc: &Document) {
        self.ids.retain(|id| doc.object(*id).is_some());
    }
    pub fn marquee(&mut self, doc: &Document, r: WorldRect, additive: bool) {
        if !additive {
            self.clear();
        }
        for id in doc.object_order() {
            if doc.object(*id).is_some_and(|o| match o.kind() {
                tack_core::ObjectKind::Image(_) => geometry::intersects(o.transform(), r),
                tack_core::ObjectKind::Annotation(a) => {
                    crate::annotation_geometry::intersects(a, o.transform(), r)
                }
                tack_core::ObjectKind::Frame(_) => {
                    let b = o.transform().bounds();
                    r.x <= b.x
                        && r.y <= b.y
                        && r.x + r.width >= b.x + b.width
                        && r.y + r.height >= b.y + b.height
                }
            }) {
                self.ids.insert(*id);
            }
        }
        self.expand_groups(doc);
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GestureKind {
    Move,
    Resize { handle: usize, center: bool },
    Scale,
    NoteScale { handle: usize },
    Rotate,
    Crop { handle: usize },
    Opacity,
}
#[derive(Default)]
pub struct ImageInteraction {
    pub selection: SelectionState,
    gesture: Option<Gesture>,
    pub crop_mode: bool,
}
impl ImageInteraction {
    pub fn active(&self) -> bool {
        self.gesture.is_some()
    }
    pub fn cancel(&mut self) {
        self.gesture = None;
    }
    pub fn preview(&self, data: ImageRenderData) -> ImageRenderData {
        self.gesture
            .as_ref()
            .and_then(|g| {
                g.lookup.get(&data.object_id).map(|i| {
                    let p = g.preview[*i];
                    ImageRenderData {
                        transform: p.transform,
                        crop: p.crop,
                        opacity: p.opacity,
                        ..data
                    }
                })
            })
            .unwrap_or(data)
    }
    pub fn frame(&self, doc: &Document) -> Option<Transform> {
        geometry::frame_transforms(
            self.selection
                .ids()
                .filter_map(|id| self.preview_transform(doc, id)),
        )
    }
    pub fn preview_opacity(&self, id: ObjectId) -> Option<Opacity> {
        self.gesture
            .as_ref()
            .and_then(|g| g.lookup.get(&id).map(|i| g.preview[*i].opacity))
    }
    pub fn preview_note_size(&self, id: ObjectId) -> Option<f64> {
        self.gesture
            .as_ref()
            .and_then(|g| g.lookup.get(&id).and_then(|i| g.preview[*i].note_size))
    }
    pub fn preview_style(&self, id: ObjectId) -> Option<AnnotationStyle> {
        self.gesture
            .as_ref()
            .and_then(|g| g.lookup.get(&id).and_then(|i| g.preview[*i].style))
    }
    pub fn selected_note(&self, doc: &Document) -> bool {
        self.selection.len() == 1 && self.selection.ids().any(|id| {
            doc.object(id).is_some_and(|o| matches!(o.kind(), ObjectKind::Annotation(a) if matches!(a.kind(), AnnotationKind::Text(_))))
        })
    }
    pub fn preview_transform(&self, doc: &Document, id: ObjectId) -> Option<Transform> {
        self.gesture
            .as_ref()
            .and_then(|g| g.lookup.get(&id).map(|i| g.preview[*i].transform))
            .or_else(|| doc.object(id).map(|o| o.transform()))
    }
    pub fn gesture_kind(&self) -> Option<GestureKind> {
        self.gesture.as_ref().map(|g| g.kind)
    }
    pub fn hit(&self, doc: &Document, point: [f64; 2]) -> Option<ObjectId> {
        self.hit_with_tolerance(doc, point, 0.)
    }
    pub fn hit_with_tolerance(
        &self,
        doc: &Document,
        point: [f64; 2],
        tolerance: f64,
    ) -> Option<ObjectId> {
        doc.object_order()
            .iter()
            .rev()
            .find(|id| {
                doc.object(**id).is_some_and(|o| {
                    let t = self.preview_transform(doc, **id).unwrap_or(o.transform());
                    match o.kind() {
                        tack_core::ObjectKind::Image(_) => geometry::hit(t, point),
                        tack_core::ObjectKind::Annotation(a) => {
                            crate::annotation_geometry::hit(a, t, point, tolerance)
                        }
                        _ => false,
                    }
                })
            })
            .copied()
    }
    pub fn delete(&mut self, editor: &mut DocumentEditor) -> Result<bool, CommandError> {
        self.cancel();
        self.selection.expand_groups(editor.document());
        let mut edits: Vec<_> = editor
            .document()
            .groups()
            .filter(|g| g.members().iter().any(|id| self.selection.contains(*id)))
            .map(|g| Command::RemoveGroup(g.id()))
            .collect();
        edits.extend(self.selection.ids().map(Command::RemoveObject));
        let changed = editor.execute(Command::Batch(edits))?;
        self.selection.clear();
        Ok(changed)
    }
    pub fn flip(
        &mut self,
        editor: &mut DocumentEditor,
        axis: usize,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        self.cancel();
        if axis > 1 {
            return Err(Box::new(GeometryError));
        }
        let edits = self
            .selection
            .ids()
            .filter_map(|id| editor.document().object(id))
            .filter(|o| matches!(o.kind(), tack_core::ObjectKind::Image(_)))
            .map(|o| {
                let t = o.transform();
                let mut flips = t.flips();
                flips[axis] = !flips[axis];
                Ok(Command::SetTransform {
                    object: o.id(),
                    transform: Transform::new(t.center(), t.size(), t.rotation(), flips)?,
                })
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;
        Ok(editor.execute(Command::Batch(edits))?)
    }
    pub fn filtering(
        &mut self,
        editor: &mut DocumentEditor,
        value: Option<ImageFiltering>,
    ) -> Result<bool, CommandError> {
        self.cancel();
        let edits = self
            .selection
            .ids()
            .filter_map(|id| editor.document().object_render_data(id))
            .map(|d| Command::SetImageFiltering {
                object: d.object_id,
                filtering: value.unwrap_or(match d.filtering {
                    ImageFiltering::Default => ImageFiltering::Smooth,
                    ImageFiltering::Smooth => ImageFiltering::Nearest,
                    ImageFiltering::Nearest => ImageFiltering::Default,
                }),
            })
            .collect();
        editor.execute(Command::Batch(edits))
    }
    pub fn focus(
        &self,
        doc: &Document,
        camera: &mut Camera,
        id: ObjectId,
    ) -> Result<(), GeometryError> {
        let t = doc.object(id).ok_or(GeometryError)?.transform();
        let b = t.bounds();
        let view = camera.viewport();
        camera.set_view(
            t.center().map(|v| v.clamp(-1e8, 1e8)),
            (view.width * camera.zoom() / b.width).min(view.height * camera.zoom() / b.height)
                * 0.9,
        )
    }
}
