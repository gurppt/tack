//! Bounded, source-free note/shape metadata. Geometry remains the shared Transform.
use crate::{ModelError, Opacity};

pub const MAX_TEXT_BYTES: usize = 16 * 1024;
pub const MAX_STROKE_POINTS: usize = 4096;

/// Byte channels intrinsically bound color values; opacity is separately validated.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Color(pub [u8; 4]);
impl Color {
    pub fn rgba(self, opacity: Opacity) -> [f32; 4] {
        let mut c = self.0.map(|v| f32::from(v) / 255.);
        c[3] *= opacity.value() as f32;
        c
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnnotationStyle {
    stroke: Color,
    fill: Option<Color>,
    width: f64,
    opacity: Opacity,
}
impl Default for AnnotationStyle {
    fn default() -> Self {
        Self {
            stroke: Color([255, 198, 82, 255]),
            fill: None,
            width: 3.,
            opacity: Opacity::OPAQUE,
        }
    }
}
impl AnnotationStyle {
    pub fn new(
        stroke: Color,
        fill: Option<Color>,
        width: f64,
        opacity: Opacity,
    ) -> Result<Self, ModelError> {
        if !width.is_finite() || !(0.1..=256.).contains(&width) {
            return Err(ModelError::InvalidAnnotation);
        }
        Ok(Self {
            stroke,
            fill,
            width,
            opacity,
        })
    }
    pub fn stroke(self) -> Color {
        self.stroke
    }
    pub fn fill(self) -> Option<Color> {
        self.fill
    }
    pub fn width(self) -> f64 {
        self.width
    }
    pub fn opacity(self) -> Opacity {
        self.opacity
    }
    pub(crate) fn set_opacity(&mut self, value: Opacity) {
        self.opacity = value;
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextAlignment {
    #[default]
    Left,
    Center,
    Right,
}
#[derive(Clone, Debug, PartialEq)]
pub struct TextObject {
    value: String,
    font_size: f64,
    alignment: TextAlignment,
}
impl TextObject {
    pub fn new(
        value: String,
        font_size: f64,
        alignment: TextAlignment,
    ) -> Result<Self, ModelError> {
        if value.len() > MAX_TEXT_BYTES
            || value
                .chars()
                .any(|c| c.is_control() && c != '\n' && c != '\t')
            || !font_size.is_finite()
            || !(4. ..=256.).contains(&font_size)
        {
            return Err(ModelError::InvalidAnnotation);
        }
        Ok(Self {
            value,
            font_size,
            alignment,
        })
    }
    pub fn value(&self) -> &str {
        &self.value
    }
    pub fn font_size(&self) -> f64 {
        self.font_size
    }
    pub fn alignment(&self) -> TextAlignment {
        self.alignment
    }
    pub fn retained_bytes(&self) -> usize {
        self.value.capacity()
    }
}
/// Coordinates are normalized within the object's box. No duplicated world transform.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineObject {
    points: [[f64; 2]; 2],
}
fn valid_point(p: [f64; 2]) -> bool {
    p.into_iter()
        .all(|v| v.is_finite() && (0. ..=1.).contains(&v))
}
impl LineObject {
    pub fn new(points: [[f64; 2]; 2]) -> Result<Self, ModelError> {
        if !points.into_iter().all(valid_point) || points[0] == points[1] {
            return Err(ModelError::InvalidAnnotation);
        }
        Ok(Self { points })
    }
    pub fn points(self) -> [[f64; 2]; 2] {
        self.points
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct ScribbleObject {
    points: Vec<[f64; 2]>,
}
impl ScribbleObject {
    pub fn new(mut points: Vec<[f64; 2]>) -> Result<Self, ModelError> {
        if !(2..=MAX_STROKE_POINTS).contains(&points.len())
            || !points.iter().copied().all(valid_point)
        {
            return Err(ModelError::InvalidAnnotation);
        }
        points.shrink_to_fit();
        Ok(Self { points })
    }
    pub fn points(&self) -> &[[f64; 2]] {
        &self.points
    }
    pub fn retained_bytes(&self) -> usize {
        self.points.capacity() * std::mem::size_of::<[f64; 2]>()
    }
}
/// Only six implemented annotation kinds; no paths, layers, connectors or widget state.
#[derive(Clone, Debug, PartialEq)]
pub enum AnnotationKind {
    Text(TextObject),
    Rect,
    Ellipse,
    Line(LineObject),
    Arrow(LineObject),
    Scribble(ScribbleObject),
}
#[derive(Clone, Debug, PartialEq)]
pub struct Annotation {
    pub(crate) kind: AnnotationKind,
    pub(crate) style: AnnotationStyle,
}
impl Annotation {
    pub fn bounds(
        &self,
        transform: crate::Transform,
    ) -> Result<crate::WorldRect, crate::GeometryError> {
        self.bounds_with_style(transform, self.style)
    }
    pub(crate) fn bounds_with_style(
        &self,
        transform: crate::Transform,
        style: AnnotationStyle,
    ) -> Result<crate::WorldRect, crate::GeometryError> {
        let b = transform.bounds();
        let margin = if matches!(self.kind, AnnotationKind::Arrow(_)) {
            style.width().max(128.) / 2.
        } else {
            style.width() / 2.
        };
        crate::WorldRect::new(
            b.x - margin,
            b.y - margin,
            b.width + 2. * margin,
            b.height + 2. * margin,
        )
    }
    pub fn new(kind: AnnotationKind, style: AnnotationStyle) -> Self {
        Self { kind, style }
    }
    pub fn kind(&self) -> &AnnotationKind {
        &self.kind
    }
    pub fn style(&self) -> AnnotationStyle {
        self.style
    }
    pub fn retained_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            + match &self.kind {
                AnnotationKind::Text(t) => t.retained_bytes(),
                AnnotationKind::Scribble(s) => s.retained_bytes(),
                _ => 0,
            }
    }
}
