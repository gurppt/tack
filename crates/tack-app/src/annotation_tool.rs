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
            if self.points.len() < MAX_STROKE_POINTS {
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
        let text = TextObject::new(self.value, self.size, self.alignment)?;
        Ok(if self.is_new {
            editor.execute(Command::AddObject {
                object: DocumentObject::annotation(
                    self.id,
                    Annotation::new(AnnotationKind::Text(text), self.style),
                    self.transform,
                )?,
                index: editor.document().object_order().len(),
            })?
        } else {
            editor.execute(Command::SetText {
                object: self.id,
                text,
            })?
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
    pub style: AnnotationStyle,
}
