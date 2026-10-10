//! Bounded local strokes, deterministic finish/merge and a target-only vector eraser.
use crate::{actions::Tool, annotation_geometry::point, image_input::ImageInput};
use tack_assets::AssetError;
use tack_core::*;

pub struct EraserDraft {
    pub id: ObjectId,
    pub annotation: Option<Annotation>,
    transform: Transform,
    generation: u64,
    last: [f64; 2],
    radius: f64,
}
/// Convert world strokes to one axis-aligned local box; styles retain appearance.
fn local_strokes(
    strokes: Vec<(Vec<[f64; 2]>, Option<AnnotationStyle>)>,
) -> Result<(ScribbleObject, Transform), AssetError> {
    if strokes.is_empty()
        || strokes.len() > MAX_SCRIBBLE_STROKES
        || strokes.iter().map(|s| s.0.len()).sum::<usize>() > MAX_STROKE_POINTS
    {
        return Err("Scribble stroke/point limit".into());
    }
    let first = *strokes[0].0.first().ok_or("empty Scribble stroke")?;
    let mut lo = first;
    let mut hi = first;
    for p in strokes.iter().flat_map(|s| &s.0) {
        for axis in 0..2 {
            lo[axis] = lo[axis].min(p[axis]);
            hi[axis] = hi[axis].max(p[axis]);
        }
    }
    let t = Transform::new(
        std::array::from_fn(|i| (lo[i] + hi[i]) / 2.),
        std::array::from_fn(|i| (hi[i] - lo[i]).max(1e-6)),
        0.,
        [false; 2],
    )?;
    let strokes = strokes
        .into_iter()
        .map(|(points, style)| {
            ScribbleStroke::new(
                points
                    .into_iter()
                    .map(|p| {
                        std::array::from_fn(|i| {
                            ((p[i] - t.center()[i]) / t.size()[i] + 0.5).clamp(0., 1.)
                        })
                    })
                    .collect(),
                style,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok((ScribbleObject::from_strokes(strokes)?, t))
}
impl ImageInput {
    pub(crate) fn complete_scribble_stroke(&mut self, generation: u64) -> Result<(), AssetError> {
        let Some(c) = &self.annotation.creation else {
            return Ok(());
        };
        if c.tool != Tool::Scribble {
            return Ok(());
        }
        if c.generation != generation {
            return Err("Board changed during active Scribble stroke".into());
        }
        if c.points.len() < 2 {
            self.annotation.creation = None;
            return Ok(());
        }
        let points = simplify_stroke(&c.points, c.tolerance)?;
        let mut c = self.annotation.creation.take().ok_or("Scribble stroke")?;
        c.points = points;
        self.annotation.scribble.push(*c);
        Ok(())
    }
    /// Independent creation drafts survive accepted remote edits; only a
    /// touched existing eraser target invalidates that captured gesture.
    pub fn rebase_scribble_drafts(&mut self, generation: u64, touches: impl Fn(ObjectId) -> bool) {
        for c in &mut self.annotation.scribble {
            c.generation = generation;
        }
        if self
            .annotation
            .eraser
            .as_ref()
            .is_some_and(|e| touches(e.id))
        {
            self.annotation.eraser = None;
            self.status = "Scribble changed remotely; eraser cancelled".into();
        } else if let Some(e) = &mut self.annotation.eraser {
            e.generation = generation;
        }
    }
    pub(crate) fn finish_scribble(
        &mut self,
        editor: &mut DocumentEditor,
    ) -> Result<(), AssetError> {
        if self.annotation.scribble.is_empty() {
            return Ok(());
        }
        if self
            .annotation
            .scribble
            .iter()
            .any(|s| s.generation != editor.generation())
        {
            return Err("Board changed during Scribble session; Escape discards the draft".into());
        }
        let style = self.annotation.scribble[0].style;
        let strokes = self
            .annotation
            .scribble
            .iter()
            .map(|s| (s.points.clone(), (s.style != style).then_some(s.style)))
            .collect();
        let (scribble, transform) = local_strokes(strokes)?;
        let id = tack_storage::new_object_id()?;
        editor.execute(Command::AddObject {
            object: DocumentObject::annotation(
                id,
                Annotation::new(AnnotationKind::Scribble(scribble), style),
                transform,
            )?,
            index: editor.document().object_order().len(),
        })?;
        self.annotation.scribble.clear();
        self.images.selection.select(Some(id), false);
        Ok(())
    }
    pub(crate) fn merge_scribbles(
        &mut self,
        editor: &mut DocumentEditor,
    ) -> Result<(), AssetError> {
        let ids = editor
            .document()
            .object_order()
            .iter()
            .copied()
            .filter(|id| self.images.selection.contains(*id))
            .collect::<Vec<_>>();
        if ids.len() < 2 {
            return Err("Select at least two Scribbles to merge".into());
        }
        let doc = editor.document();
        let parent = doc.frame_parent(ids[0]);
        let mut strokes = Vec::new();
        let mut points = 0;
        let mut style = None;
        for id in &ids {
            if self.images.blocked.contains(id) || doc.frame_parent(*id) != parent {
                return Err("Merge requires available Scribbles with the same Frame parent".into());
            }
            let object = doc.object(*id).ok_or("missing Scribble")?;
            let ObjectKind::Annotation(a) = object.kind() else {
                return Err("Merge accepts Scribbles only".into());
            };
            let AnnotationKind::Scribble(s) = a.kind() else {
                return Err("Merge accepts Scribbles only".into());
            };
            style.get_or_insert(a.style());
            points += s.point_count();
            if strokes.len() + s.strokes().len() > MAX_SCRIBBLE_STROKES
                || points > MAX_STROKE_POINTS
            {
                return Err("Merge exceeds Scribble limit".into());
            }
            for stroke in s.strokes() {
                strokes.push((
                    stroke
                        .points()
                        .iter()
                        .map(|p| point(object.transform(), *p))
                        .collect(),
                    Some(stroke.style().unwrap_or(a.style())),
                ));
            }
        }
        // A single object cannot preserve alternating painter order with overlapping
        // unselected objects. Refuse that case before changing the document.
        let first = doc
            .object_order()
            .iter()
            .position(|id| self.images.selection.contains(*id))
            .ok_or("selection")?;
        let top = doc
            .object_order()
            .iter()
            .rposition(|id| self.images.selection.contains(*id))
            .ok_or("selection")?;
        let mut bounds = doc.object(ids[0]).ok_or("Scribble")?.bounds();
        for id in &ids[1..] {
            let b = doc.object(*id).ok_or("Scribble")?.bounds();
            let right = (bounds.x + bounds.width).max(b.x + b.width);
            let bottom = (bounds.y + bounds.height).max(b.y + b.height);
            bounds.x = bounds.x.min(b.x);
            bounds.y = bounds.y.min(b.y);
            bounds.width = right - bounds.x;
            bounds.height = bottom - bounds.y;
        }
        if doc.object_order()[first..top].iter().any(|id| {
            !self.images.selection.contains(*id)
                && doc.object(*id).is_some_and(|o| {
                    !matches!(o.kind(), ObjectKind::Frame(_)) && o.bounds().intersects(bounds)
                })
        }) {
            return Err(
                "Merge would change overlap order; place the Scribbles together first".into(),
            );
        }
        let (scribble, transform) = local_strokes(strokes)?;
        let id = tack_storage::new_object_id()?;
        // Place at the topmost selected position, preserving unrelated z-order.
        let index = top + 1 - ids.len();
        let mut edits = ids
            .iter()
            .copied()
            .map(Command::RemoveObject)
            .collect::<Vec<_>>();
        edits.push(Command::AddObject {
            object: DocumentObject::annotation(
                id,
                Annotation::new(AnnotationKind::Scribble(scribble), style.ok_or("style")?),
                transform,
            )?,
            index,
        });
        if let Some(parent) = parent {
            edits.push(Command::SetFrameLinks(vec![(id, Some(parent))]));
        }
        editor.execute(Command::Batch(edits))?;
        self.images.selection.select(Some(id), false);
        Ok(())
    }
    pub(crate) fn begin_eraser(
        &mut self,
        world: [f64; 2],
        editor: &DocumentEditor,
        camera: &Camera,
    ) -> Result<bool, AssetError> {
        if self.active_tool() != Tool::Eraser {
            return Ok(false);
        }
        let doc = editor.document();
        let hit = self.images.selection.ids().find(|_| self.images.selection.len() == 1)
            .filter(|id| doc.object(*id).is_some_and(|o| matches!(o.kind(), ObjectKind::Annotation(a) if matches!(a.kind(), AnnotationKind::Scribble(_)))))
            .or_else(|| self.images.hit_with_tolerance(doc, world, 6. / camera.zoom()));
        if let Some(id) = hit.filter(|id| !self.images.blocked.contains(id))
            && let Some(o) = doc.object(id)
            && let ObjectKind::Annotation(a) = o.kind()
            && matches!(a.kind(), AnnotationKind::Scribble(_))
        {
            self.images.selection.select(Some(id), false);
            self.annotation.eraser = Some(Box::new(EraserDraft {
                id,
                annotation: Some(a.as_ref().clone()),
                transform: o.transform(),
                generation: editor.generation(),
                last: world,
                radius: a.style().width() * 0.75 + 5. * self.gizmo.scale / camera.zoom(),
            }));
            self.update_eraser(world, editor)?;
        } else {
            self.status = "Eraser affects a target Scribble only".into();
        }
        Ok(true)
    }
    pub(crate) fn update_eraser(
        &mut self,
        next: [f64; 2],
        editor: &DocumentEditor,
    ) -> Result<(), AssetError> {
        let Some(draft) = &mut self.annotation.eraser else {
            return Ok(());
        };
        if draft.generation != editor.generation() {
            self.annotation.eraser = None;
            return Err("Board changed; eraser cancelled".into());
        }
        if let Some(a) = &draft.annotation {
            let replacement = erase(a, draft.transform, draft.last, next, draft.radius)?;
            draft.annotation = replacement;
        }
        draft.last = next;
        Ok(())
    }
    pub(crate) fn finish_eraser(&mut self, editor: &mut DocumentEditor) -> Result<(), AssetError> {
        let Some(draft) = self.annotation.eraser.take() else {
            return Ok(());
        };
        if draft.generation != editor.generation() {
            return Err("Board changed; eraser cancelled".into());
        }
        let command = match draft.annotation {
            Some(annotation) => Command::SetAnnotation {
                object: draft.id,
                annotation,
            },
            None => Command::RemoveObject(draft.id),
        };
        editor.execute(command)?;
        Ok(())
    }
}
/// Intersect a source segment with the swept circular eraser (a capsule).
/// Distance to a segment is convex: bounded searches locate its erased interval.
fn erased_interval(
    a: [f64; 2],
    b: [f64; 2],
    from: [f64; 2],
    to: [f64; 2],
    radius: f64,
) -> Option<[f64; 2]> {
    if (0..2).any(|i| {
        a[i].min(b[i]) > from[i].max(to[i]) + radius || a[i].max(b[i]) < from[i].min(to[i]) - radius
    }) {
        return None;
    }
    let p = |t: f64| std::array::from_fn(|i| a[i] + (b[i] - a[i]) * t);
    let inside = |t: f64| segment_distance(p(t), from, to) <= radius;
    let (mut lo, mut hi) = (0., 1.);
    for _ in 0..28 {
        let x = lo + (hi - lo) / 3.;
        let y = hi - (hi - lo) / 3.;
        if segment_distance(p(x), from, to) < segment_distance(p(y), from, to) {
            hi = y;
        } else {
            lo = x;
        }
    }
    let middle = (lo + hi) / 2.;
    if !inside(middle) {
        return None;
    }
    let (mut lo, mut hi) = (0., middle);
    if !inside(0.) {
        for _ in 0..32 {
            let m = (lo + hi) / 2.;
            if inside(m) {
                hi = m;
            } else {
                lo = m;
            }
        }
    } else {
        hi = 0.;
    }
    let start = hi;
    let (mut lo, mut hi) = (middle, 1.);
    if !inside(1.) {
        for _ in 0..32 {
            let m = (lo + hi) / 2.;
            if inside(m) {
                lo = m;
            } else {
                hi = m;
            }
        }
    } else {
        lo = 1.;
    }
    Some([start, lo])
}
pub fn erase(
    a: &Annotation,
    t: Transform,
    from: [f64; 2],
    to: [f64; 2],
    radius: f64,
) -> Result<Option<Annotation>, AssetError> {
    if !from.into_iter().chain(to).all(f64::is_finite) || !radius.is_finite() || radius <= 0. {
        return Err("invalid eraser geometry".into());
    }
    let AnnotationKind::Scribble(s) = a.kind() else {
        return Err("Eraser accepts Scribbles only".into());
    };
    let mut strokes = Vec::new();
    let mut changed = false;
    for stroke in s.strokes() {
        let mut piece = Vec::new();
        for segment in stroke.points().windows(2) {
            let [a, b] = [point(t, segment[0]), point(t, segment[1])];
            if piece.is_empty() {
                piece.push(segment[0]);
            }
            if let Some([enter, exit]) = erased_interval(a, b, from, to, radius) {
                changed = true;
                let interpolate = |u: f64| {
                    std::array::from_fn(|i| segment[0][i] + (segment[1][i] - segment[0][i]) * u)
                };
                if enter > 1e-8 {
                    piece.push(interpolate(enter));
                }
                if piece.len() >= 2 {
                    strokes.push(ScribbleStroke::new(
                        std::mem::take(&mut piece),
                        stroke.style(),
                    )?);
                } else {
                    piece.clear();
                }
                if exit < 1. - 1e-8 {
                    piece.push(interpolate(exit));
                    piece.push(segment[1]);
                }
            } else {
                piece.push(segment[1]);
            }
            if strokes.len() > MAX_SCRIBBLE_STROKES {
                return Err("Eraser split limit reached; finish this gesture".into());
            }
        }
        if piece.len() >= 2 {
            strokes.push(ScribbleStroke::new(piece, stroke.style())?);
        }
    }
    if !changed {
        return Ok(Some(a.clone()));
    }
    if strokes.is_empty() {
        return Ok(None);
    }
    Ok(Some(Annotation::new(
        AnnotationKind::Scribble(ScribbleObject::from_strokes(strokes)?),
        a.style(),
    )))
}
