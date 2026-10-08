//! Captured initial data and atomic preview/commit for shared object gestures.
use super::{GestureKind, ImageInteraction};
use crate::image_geometry as geometry;
use std::collections::BTreeMap;
use tack_core::{
    AnnotationKind, AnnotationStyle, Command, CommandError, Document, DocumentEditor,
    DocumentQuery, GeometryError, ObjectId, ObjectKind, Opacity, TextObject, Transform, WorldRect,
};

#[derive(Clone, Copy)]
pub(super) struct EditData {
    pub(super) object_id: ObjectId,
    pub(super) transform: Transform,
    pub(super) crop: tack_core::Crop,
    pub(super) opacity: Opacity,
    pub(super) margin: f64,
    pub(super) note_size: Option<f64>,
    pub(super) style: Option<AnnotationStyle>,
}
impl EditData {
    fn get(doc: &Document, id: ObjectId) -> Option<Self> {
        let o = doc.object(id)?;
        let image = doc.object_render_data(id);
        Some(Self {
            object_id: id,
            transform: o.transform(),
            crop: image.map_or(tack_core::Crop::FULL, |d| d.crop),
            opacity: image.map_or_else(
                || match o.kind() {
                    tack_core::ObjectKind::Annotation(a) => a.style().opacity(),
                    _ => Opacity::OPAQUE,
                },
                |d| d.opacity,
            ),
            margin: (o.transform().bounds().x - o.bounds().x).max(0.),
            note_size: match o.kind() {
                ObjectKind::Annotation(a) => match a.kind() {
                    AnnotationKind::Text(t) => Some(t.font_size()),
                    _ => None,
                },
                _ => None,
            },
            style: match o.kind() {
                ObjectKind::Annotation(a) => Some(a.style()),
                _ => None,
            },
        })
    }
}
pub(super) struct Gesture {
    pub(super) kind: GestureKind,
    pub(super) start: [f64; 2],
    pub(super) frame: Transform,
    pub(super) initial: Vec<EditData>,
    pub(super) preview: Vec<EditData>,
    pub(super) lookup: BTreeMap<ObjectId, usize>,
    pub(super) generation: u64,
}
impl ImageInteraction {
    pub fn begin(
        &mut self,
        kind: GestureKind,
        pointer: [f64; 2],
        editor: &DocumentEditor,
    ) -> Result<bool, GeometryError> {
        self.selection.expand_groups(editor.document());
        if !pointer.iter().all(|p| p.is_finite()) {
            return Err(GeometryError);
        }
        if matches!(kind, GestureKind::Crop { .. }) && self.selection.len() != 1 {
            return Ok(false);
        }
        if matches!(kind, GestureKind::Resize{handle,..}|GestureKind::Crop{handle}|GestureKind::NoteScale{handle} if handle >= 8)
        {
            return Err(GeometryError);
        }
        if let GestureKind::NoteScale { handle } = kind
            && (handle % 2 != 0 || !self.selected_note(editor.document()))
        {
            return Ok(false);
        }
        let initial: Vec<_> = self
            .selection
            .ids()
            .filter_map(|id| EditData::get(editor.document(), id))
            .collect();
        if !matches!(
            kind,
            GestureKind::Move
                | GestureKind::Resize { .. }
                | GestureKind::Scale
                | GestureKind::NoteScale { .. }
        ) && self.selection.ids().any(|id| {
            editor
                .document()
                .object(id)
                .is_some_and(|o| match o.kind() {
                    tack_core::ObjectKind::Frame(_) => true,
                    tack_core::ObjectKind::Annotation(_) => {
                        matches!(kind, GestureKind::Crop { .. })
                    }
                    _ => false,
                })
        }) {
            return Ok(false);
        }
        let Some(frame) = geometry::frame_transforms(initial.iter().map(|d| d.transform)) else {
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
            GestureKind::Resize { handle, center } => Some(
                if g.initial.len() == 1 && g.initial[0].note_size.is_some() {
                    crate::annotation_geometry::resize_note_box(
                        g.frame,
                        delta,
                        geometry::HANDLE_DIRECTIONS[handle],
                        center,
                    )?
                } else {
                    geometry::resize(
                        g.frame,
                        delta,
                        geometry::HANDLE_DIRECTIONS[handle],
                        center,
                        g.initial.len() > 1,
                    )?
                },
            ),
            GestureKind::NoteScale { handle } => {
                let direction = geometry::HANDLE_DIRECTIONS[handle];
                let proposed = geometry::resize(g.frame, delta, direction, false, true)?;
                let size = g.initial[0].note_size.ok_or(GeometryError)?;
                let style = g.initial[0].style.ok_or(GeometryError)?;
                let min = (4. / size).max(0.1 / style.width());
                let max = (256. / size).min(256. / style.width());
                let scale = (proposed.size()[0] / g.frame.size()[0]).clamp(min, max);
                Some(crate::annotation_geometry::scale_note(
                    g.frame, scale, direction,
                )?)
            }
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
        let calculate = |data: EditData| -> Result<EditData, GeometryError> {
            let t = data.transform;
            let transform = match g.kind {
                GestureKind::Move => Transform::new(
                    [t.center()[0] + delta[0], t.center()[1] + delta[1]],
                    t.size(),
                    t.rotation(),
                    t.flips(),
                )?,
                GestureKind::Resize { .. } | GestureKind::Scale | GestureKind::NoteScale { .. } => {
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
                    let (transform, crop) = geometry::crop_transform(
                        data.transform,
                        data.crop,
                        delta,
                        geometry::HANDLE_DIRECTIONS[handle],
                    )?;
                    return Ok(EditData {
                        transform,
                        crop,
                        ..data
                    });
                }
                GestureKind::Opacity => {
                    return Ok(EditData {
                        opacity: Opacity::new(
                            (data.opacity.value() + delta[0] / g.frame.size()[0]).clamp(0., 1.),
                        )?,
                        ..data
                    });
                }
            };
            if matches!(g.kind, GestureKind::NoteScale { .. }) {
                let scale = transform.size()[0] / t.size()[0];
                let style = data.style.ok_or(GeometryError)?;
                let style = AnnotationStyle::new(
                    style.stroke(),
                    style.fill(),
                    (style.width() * scale).clamp(0.1, 256.),
                    style.opacity(),
                )
                .map_err(|_| GeometryError)?;
                Ok(EditData {
                    transform,
                    note_size: data.note_size.map(|v| (v * scale).clamp(4., 256.)),
                    style: Some(style),
                    margin: style.width() / 2.,
                    ..data
                })
            } else {
                Ok(EditData { transform, ..data })
            }
        };
        for &data in &g.initial {
            let next = calculate(data)?;
            // Annotation styles may extend outside the shared transform box.
            if next.margin > 0. {
                let b = next.transform.bounds();
                WorldRect::new(
                    b.x - next.margin,
                    b.y - next.margin,
                    b.width + 2. * next.margin,
                    b.height + 2. * next.margin,
                )?;
            }
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
            if before.note_size != after.note_size
                && let Some(size) = after.note_size
                && let Some(o) = editor.document().object(after.object_id)
                && let ObjectKind::Annotation(a) = o.kind()
                && let AnnotationKind::Text(text) = a.kind()
            {
                let text = TextObject::new(text.value().to_owned(), size, text.alignment())
                    .map_err(|_| CommandError::LimitReached("invalid scaled note"))?;
                edits.push(Command::SetText {
                    object: after.object_id,
                    text,
                });
            }
            if before.style != after.style
                && let Some(style) = after.style
            {
                edits.push(Command::SetAnnotationStyle {
                    object: after.object_id,
                    style,
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
}
