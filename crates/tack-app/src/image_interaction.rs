//! Transient selection and gesture preview; durable edits go through DocumentEditor.
use crate::image_geometry as geometry;
use std::collections::{BTreeMap, BTreeSet};
use tack_core::{
    Camera, Command, CommandError, Document, DocumentEditor, DocumentQuery, GeometryError,
    ImageFiltering, ImageRenderData, ObjectId, Opacity, Transform, WorldRect,
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
    pub fn prune(&mut self, doc: &Document) {
        self.ids.retain(|id| doc.object(*id).is_some());
    }
    pub fn marquee(&mut self, doc: &Document, r: WorldRect, additive: bool) {
        if !additive {
            self.clear();
        }
        for id in doc.object_order() {
            if doc
                .object(*id)
                .is_some_and(|o| geometry::intersects(o.transform(), r))
            {
                self.ids.insert(*id);
            }
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GestureKind {
    Move,
    Resize { handle: usize, center: bool },
    Scale,
    Rotate,
    Crop { handle: usize },
    Opacity,
}
struct Gesture {
    kind: GestureKind,
    start: [f64; 2],
    frame: Transform,
    initial: Vec<ImageRenderData>,
    preview: Vec<ImageRenderData>,
    lookup: BTreeMap<ObjectId, usize>,
    generation: u64,
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
            .and_then(|g| g.lookup.get(&data.object_id).map(|i| g.preview[*i]))
            .unwrap_or(data)
    }
    pub fn frame(&self, doc: &Document) -> Option<Transform> {
        geometry::frame(
            self.selection
                .ids()
                .filter_map(|id| doc.object_render_data(id))
                .map(|d| self.preview(d)),
        )
    }
    pub fn hit(&self, doc: &Document, point: [f64; 2]) -> Option<ObjectId> {
        doc.object_order()
            .iter()
            .rev()
            .find(|id| {
                doc.object_render_data(**id)
                    .is_some_and(|d| geometry::hit(self.preview(d).transform, point))
            })
            .copied()
    }
    pub fn begin(
        &mut self,
        kind: GestureKind,
        pointer: [f64; 2],
        editor: &DocumentEditor,
    ) -> Result<bool, GeometryError> {
        if !pointer.iter().all(|p| p.is_finite()) {
            return Err(GeometryError);
        }
        if matches!(kind, GestureKind::Crop { .. }) && self.selection.len() != 1 {
            return Ok(false);
        }
        if matches!(kind, GestureKind::Resize{handle,..}|GestureKind::Crop{handle} if handle >= 8) {
            return Err(GeometryError);
        }
        let initial: Vec<_> = self
            .selection
            .ids()
            .filter_map(|id| editor.document().object_render_data(id))
            .collect();
        let Some(frame) = geometry::frame(initial.iter().copied()) else {
            return Ok(false);
        };
        let lookup = initial
            .iter()
            .enumerate()
            .map(|(i, d)| (d.object_id, i))
            .collect();
        self.gesture = Some(Gesture {
            kind,
            start: pointer,
            frame,
            preview: initial.clone(),
            initial,
            lookup,
            generation: editor.generation(),
        });
        Ok(true)
    }
    /// Reuses fixed preview storage. Validate a whole update before publishing it.
    pub fn update(&mut self, pointer: [f64; 2]) -> Result<bool, GeometryError> {
        if !pointer.iter().all(|p| p.is_finite()) {
            return Err(GeometryError);
        }
        let Some(g) = &mut self.gesture else {
            return Ok(false);
        };
        let delta = [pointer[0] - g.start[0], pointer[1] - g.start[1]];
        let resized = match g.kind {
            GestureKind::Resize { handle, center } => Some(geometry::resize(
                g.frame,
                delta,
                geometry::HANDLE_DIRECTIONS[handle],
                center,
                g.initial.len() > 1,
            )?),
            GestureKind::Scale => {
                let scale = (delta[0] / g.frame.size()[0]).exp().clamp(1e-6, 1e6);
                Some(Transform::new(
                    g.frame.center(),
                    g.frame.size().map(|s| s * scale),
                    g.frame.rotation(),
                    g.frame.flips(),
                )?)
            }
            _ => None,
        };
        let start_radius =
            (g.start[0] - g.frame.center()[0]).hypot(g.start[1] - g.frame.center()[1]);
        let angle = if start_radius < g.frame.size()[0].min(g.frame.size()[1]) * 0.05 {
            delta[0] / g.frame.size()[0] * std::f64::consts::PI
        } else {
            (pointer[1] - g.frame.center()[1]).atan2(pointer[0] - g.frame.center()[0])
                - (g.start[1] - g.frame.center()[1]).atan2(g.start[0] - g.frame.center()[0])
        };
        let calculate = |data: ImageRenderData| -> Result<ImageRenderData, GeometryError> {
            let t = data.transform;
            let transform = match g.kind {
                GestureKind::Move => Transform::new(
                    [t.center()[0] + delta[0], t.center()[1] + delta[1]],
                    t.size(),
                    t.rotation(),
                    t.flips(),
                )?,
                GestureKind::Resize { .. } | GestureKind::Scale => {
                    let next = resized.ok_or(GeometryError)?;
                    if g.initial.len() == 1 {
                        next
                    } else {
                        let scale = next.size()[0] / g.frame.size()[0];
                        let offset = [
                            t.center()[0] - g.frame.center()[0],
                            t.center()[1] - g.frame.center()[1],
                        ];
                        Transform::new(
                            [
                                next.center()[0] + offset[0] * scale,
                                next.center()[1] + offset[1] * scale,
                            ],
                            t.size().map(|s| s * scale),
                            t.rotation(),
                            t.flips(),
                        )?
                    }
                }
                GestureKind::Rotate => {
                    let offset = geometry::rotate(
                        [
                            t.center()[0] - g.frame.center()[0],
                            t.center()[1] - g.frame.center()[1],
                        ],
                        angle,
                    );
                    Transform::new(
                        [
                            g.frame.center()[0] + offset[0],
                            g.frame.center()[1] + offset[1],
                        ],
                        t.size(),
                        geometry::angle(t.rotation() + angle),
                        t.flips(),
                    )?
                }
                GestureKind::Crop { handle } => {
                    return geometry::crop(data, delta, geometry::HANDLE_DIRECTIONS[handle]);
                }
                GestureKind::Opacity => {
                    return Ok(ImageRenderData {
                        opacity: Opacity::new(
                            (data.opacity.value() + delta[0] / g.frame.size()[0]).clamp(0., 1.),
                        )?,
                        ..data
                    });
                }
            };
            Ok(ImageRenderData { transform, ..data })
        };
        for &data in &g.initial {
            calculate(data)?;
        }
        for (&data, next) in g.initial.iter().zip(&mut g.preview) {
            *next = calculate(data)?;
        }
        Ok(true)
    }
    pub fn commit(&mut self, editor: &mut DocumentEditor) -> Result<bool, CommandError> {
        let Some(g) = self.gesture.take() else {
            return Ok(false);
        };
        if editor.generation() != g.generation {
            return Err(CommandError::LimitReached(
                "document changed during gesture",
            ));
        }
        let mut edits = Vec::with_capacity(g.preview.len() * 2);
        for (before, after) in g.initial.iter().zip(g.preview) {
            if before.transform != after.transform {
                edits.push(Command::SetTransform {
                    object: after.object_id,
                    transform: after.transform,
                });
            }
            if before.crop != after.crop {
                edits.push(Command::SetCrop {
                    object: after.object_id,
                    crop: after.crop,
                });
            }
            if before.opacity != after.opacity {
                edits.push(Command::SetOpacity {
                    object: after.object_id,
                    opacity: after.opacity,
                });
            }
        }
        editor.execute(Command::Batch(edits))
    }
    pub fn delete(&mut self, editor: &mut DocumentEditor) -> Result<bool, CommandError> {
        self.cancel();
        let edits = self.selection.ids().map(Command::RemoveObject).collect();
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
        let data = doc.object_render_data(id).ok_or(GeometryError)?;
        let b = data.transform.bounds();
        let view = camera.viewport();
        camera.set_view(
            data.transform.center().map(|v| v.clamp(-1e8, 1e8)),
            (view.width * camera.zoom() / b.width).min(view.height * camera.zoom() / b.height)
                * 0.9,
        )
    }
}
