//! Transient creation and note editing. No durable mutation until completion.
use crate::actions::Tool;
use tack_core::*;
pub const NOTE_DEFAULT_SIZE: f64 = 24.;
pub struct Creation {
    pub tool: Tool,
    pub start: [f64; 2],
    pub end: [f64; 2],
    pub style: AnnotationStyle,
    pub points: Vec<[f64; 2]>,
    pub capped: bool,
    pub generation: u64,
    pub tolerance: f64,
    pub point_limit: usize,
}
impl Creation {
    pub fn new(
        tool: Tool,
        start: [f64; 2],
        style: AnnotationStyle,
        generation: u64,
        tolerance: f64,
    ) -> Self {
        Self {
            tool,
            start,
            end: start,
            style,
            points: if tool == Tool::Scribble {
                vec![start]
            } else {
                Vec::new()
            },
            capped: false,
            generation,
            tolerance,
            point_limit: MAX_STROKE_POINTS,
        }
    }
    pub fn update(&mut self, p: [f64; 2]) {
        if !p
            .into_iter()
            .all(|v| v.is_finite() && v.abs() < 1e9 - 1024.)
        {
            return;
        }
        self.end = p;
        if self.tool == Tool::Scribble
            && self
                .points
                .last()
                .is_none_or(|last| (p[0] - last[0]).hypot(p[1] - last[1]) >= self.tolerance * 0.5)
        {
            if self.points.len() < self.point_limit {
                self.points.push(p);
            } else {
                self.capped = true;
                if let Some(last) = self.points.last_mut() {
                    *last = p;
                }
            }
        }
    }
    pub fn transform(&self) -> Result<Transform, GeometryError> {
        let points = if self.tool == Tool::Scribble {
            self.points.as_slice()
        } else {
            &[]
        };
        let mut lo = self.start;
        let mut hi = self.start;
        for p in points.iter().copied().chain([self.end]) {
            for i in 0..2 {
                lo[i] = lo[i].min(p[i]);
                hi[i] = hi[i].max(p[i]);
            }
        }
        Transform::new(
            [(lo[0] + hi[0]) / 2., (lo[1] + hi[1]) / 2.],
            [(hi[0] - lo[0]).max(1e-6), (hi[1] - lo[1]).max(1e-6)],
            0.,
            [false; 2],
        )
    }
    pub fn kind(&self, t: Transform) -> Result<AnnotationKind, ModelError> {
        let normalize = |p: [f64; 2]| {
            std::array::from_fn(|i| ((p[i] - t.center()[i]) / t.size()[i] + 0.5).clamp(0., 1.))
        };
        Ok(match self.tool {
            Tool::Rectangle => AnnotationKind::Rect,
            Tool::Line => AnnotationKind::Line(LineObject::new([
                normalize(self.start),
                normalize(self.end),
            ])?),
            Tool::Arrow => AnnotationKind::Arrow(LineObject::new([
                normalize(self.start),
                normalize(self.end),
            ])?),
            Tool::Scribble => AnnotationKind::Scribble(ScribbleObject::new(
                simplify_stroke(&self.points, self.tolerance)?
                    .into_iter()
                    .map(normalize)
                    .collect(),
            )?),
            _ => return Err(ModelError::InvalidAnnotation),
        })
    }
}
#[derive(Clone)]
pub struct NoteEdit {
    pub id: ObjectId,
    pub value: String,
    pub size: f64,
    pub alignment: TextAlignment,
    pub transform: Transform,
    pub style: AnnotationStyle,
    pub generation: u64,
    pub is_new: bool,
    pub replace: bool,
    pub composing: bool,
}
impl NoteEdit {
    pub(crate) fn minimum_size(&self, width: f64) -> [f64; 2] {
        let padding = self.size * 0.2;
        let word = self
            .value
            .split_whitespace()
            .map(|s| {
                s.chars()
                    .map(|c| crate::note_layout::advance(c, self.size))
                    .sum::<f64>()
            })
            .fold(self.size, f64::max)
            .max(
                self.value
                    .chars()
                    .filter(|c| *c != '\n')
                    .map(|c| crate::note_layout::advance(c, self.size))
                    .fold(0., f64::max),
            );
        let width = width.max(word + padding * 2.);
        let rows = crate::note_layout::NoteLines::new(&self.value, width - padding * 2., self.size)
            .count()
            .max(1)
            + usize::from(self.value.ends_with('\n'));
        [
            word + padding * 2.,
            (rows.saturating_sub(1) as f64 * 1.3 + 1.) * self.size + padding * 2.,
        ]
    }
    pub(crate) fn constrain_box(
        &self,
        proposed: [f64; 2],
        direction: [f64; 2],
        initial: [f64; 2],
    ) -> [f64; 2] {
        let minimum = self.minimum_size(proposed[0]);
        let mut size = std::array::from_fn(|i| proposed[i].max(minimum[i]));
        if direction[1] == 0. {
            size[1] = initial[1];
            if self.minimum_size(size[0])[1] > size[1] {
                let mut lo = size[0];
                let mut hi = initial[0].max(lo);
                for _ in 0..16 {
                    let mid = (lo + hi) * 0.5;
                    if self.minimum_size(mid)[1] > size[1] {
                        lo = mid;
                    } else {
                        hi = mid;
                    }
                }
                size[0] = hi;
            }
        }
        size
    }
    pub(crate) fn ensure_text_fits(&mut self) -> Result<(), tack_core::GeometryError> {
        let minimum = self.minimum_size(self.transform.size()[0]);
        let size = std::array::from_fn(|i| self.transform.size()[i].max(minimum[i]));
        self.transform = Transform::new(
            self.transform.center(),
            size,
            self.transform.rotation(),
            self.transform.flips(),
        )?;
        Ok(())
    }
    pub fn insert(&mut self, text: &str) {
        if text
            .chars()
            .any(|c| c.is_control() && c != '\n' && c != '\t')
        {
            return;
        }
        if (if self.replace { 0 } else { self.value.len() }) + text.len() > MAX_TEXT_BYTES {
            return;
        }
        if self.replace {
            self.value.clear();
            self.replace = false;
        }
        self.value.push_str(text);
    }
    pub fn backspace(&mut self) {
        if self.replace {
            self.value.clear();
            self.replace = false;
        } else {
            self.value.pop();
        }
    }
    pub fn finish(
        self,
        editor: &mut DocumentEditor,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        if editor.generation() != self.generation {
            return Err("document changed during text edit".into());
        }
        let minimum = self.minimum_size(self.transform.size()[0]);
        let size = std::array::from_fn(|i| self.transform.size()[i].max(minimum[i]));
        let transform = Transform::new(
            self.transform.center(),
            size,
            self.transform.rotation(),
            self.transform.flips(),
        )?;
        let text = TextObject::new(self.value, self.size, self.alignment)?;
        Ok(if self.is_new {
            editor.execute(Command::AddObject {
                object: DocumentObject::annotation(
                    self.id,
                    Annotation::new(AnnotationKind::Text(text), self.style),
                    transform,
                )?,
                index: editor.document().object_order().len(),
            })?
        } else {
            editor.execute(Command::Batch(vec![
                Command::SetText {
                    object: self.id,
                    text,
                },
                Command::SetTransform {
                    object: self.id,
                    transform,
                },
            ]))?
        })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StyleAction {
    Color,
    Fill,
    Wider,
    Narrower,
    LargerText,
    SmallerText,
    OpacityUp,
    OpacityDown,
    AlignText,
}
#[derive(Default)]
pub struct AnnotationInput {
    pub tools: crate::actions::Interaction,
    pub creation: Option<Box<Creation>>,
    pub edit: Option<Box<NoteEdit>>,
    pub scribble: Vec<Creation>,
    pub eraser: Option<Box<crate::scribble_edit::EraserDraft>>,
    pub style: AnnotationStyle,
    pub box_resize: Option<(Transform, [f64; 2], usize)>,
}
#[cfg(test)]
mod box_tests {
    use super::*;
    #[test]
    fn horizontal_edit_resize_preserves_height_and_keeps_multiline_text_inside()
    -> Result<(), tack_assets::AssetError> {
        let mut edit = NoteEdit {
            id: tack_core::ObjectId::new(1)?,
            value: "Several words here\nSecond line\n\tThird".into(),
            size: 16.,
            alignment: TextAlignment::Left,
            transform: Transform::new([0.; 2], [220., 80.], 0., [false; 2])?,
            style: AnnotationStyle::default(),
            generation: 0,
            is_new: false,
            replace: false,
            composing: false,
        };
        edit.ensure_text_fits()?;
        let initial = edit.transform.size();
        let size = edit.constrain_box([5., initial[1]], [1., 0.], initial);
        assert_eq!(size[1], initial[1]);
        assert!(edit.minimum_size(size[0])[1] <= size[1]);
        let corner = edit.constrain_box([1., 1.], [1., 1.], initial);
        let minimum = edit.minimum_size(corner[0]);
        assert!(corner[0] >= minimum[0] && corner[1] >= minimum[1]);
        assert_eq!(edit.size, 16.);
        Ok(())
    }
}
