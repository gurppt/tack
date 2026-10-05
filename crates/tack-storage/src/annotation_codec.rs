//! Schema 3 annotation payloads. Slice/count checks precede owned allocations.
use crate::{Result, StorageError, codec::Decoder};
use tack_core::*;
const MAX_PAYLOAD: usize = MAX_STROKE_POINTS * 16 + 128;
fn invalid(_: impl std::fmt::Display) -> StorageError {
    StorageError::Invalid("annotation data")
}
pub(crate) fn estimate(a: &Annotation) -> usize {
    128 + match a.kind() {
        AnnotationKind::Text(t) => t.value().len(),
        AnnotationKind::Scribble(s) => s.points().len() * 16,
        _ => 32,
    }
}
pub(crate) fn encode(out: &mut Vec<u8>, o: &DocumentObject, a: &Annotation) {
    let kind: u16 = match a.kind() {
        AnnotationKind::Text(_) => 3,
        AnnotationKind::Rect => 4,
        AnnotationKind::Line(_) => 6,
        AnnotationKind::Arrow(_) => 7,
        AnnotationKind::Scribble(_) => 8,
    };
    out.extend(1u16.to_le_bytes());
    out.extend(kind.to_le_bytes());
    out.extend(o.id().value().to_le_bytes());
    let length_pos = out.len();
    out.extend(0u32.to_le_bytes());
    let t = o.transform();
    for v in t.center().into_iter().chain(t.size()).chain([t.rotation()]) {
        out.extend(v.to_le_bytes());
    }
    for f in t.flips() {
        out.push(u8::from(f));
    }
    let s = a.style();
    out.extend(s.stroke().0);
    out.push(u8::from(s.fill().is_some()));
    out.extend(s.fill().unwrap_or(Color([0; 4])).0);
    out.extend(s.width().to_le_bytes());
    out.extend(s.opacity().value().to_le_bytes());
    match a.kind() {
        AnnotationKind::Text(t) => {
            out.extend(t.font_size().to_le_bytes());
            out.push(match t.alignment() {
                TextAlignment::Left => 0,
                TextAlignment::Center => 1,
                TextAlignment::Right => 2,
            });
            out.extend((t.value().len() as u32).to_le_bytes());
            out.extend(t.value().as_bytes());
        }
        AnnotationKind::Line(l) | AnnotationKind::Arrow(l) => {
            for v in l.points().into_iter().flatten() {
                out.extend(v.to_le_bytes());
            }
        }
        AnnotationKind::Scribble(s) => {
            out.extend((s.points().len() as u32).to_le_bytes());
            for v in s.points().iter().flatten() {
                out.extend(v.to_le_bytes());
            }
        }
        _ => {}
    }
    let len = (out.len() - length_pos - 4) as u32;
    out[length_pos..length_pos + 4].copy_from_slice(&len.to_le_bytes());
}
pub(crate) fn decode(d: &mut Decoder<'_>, id: ObjectId, kind: u16) -> Result<DocumentObject> {
    let len = d.u32()? as usize;
    if len > MAX_PAYLOAD {
        return Err(StorageError::Invalid("annotation payload length"));
    }
    let mut d = Decoder::new(d.take(len)?);
    let t = Transform::new(
        [d.f64()?, d.f64()?],
        [d.f64()?, d.f64()?],
        d.f64()?,
        [d.boolean()?, d.boolean()?],
    )
    .map_err(invalid)?;
    let stroke = Color(d.take(4)?.try_into().map_err(invalid)?);
    let filled = d.boolean()?;
    let fill = Color(d.take(4)?.try_into().map_err(invalid)?);
    if !filled && fill.0 != [0; 4] {
        return Err(StorageError::Unsupported("unused fill bytes"));
    }
    let style = AnnotationStyle::new(
        stroke,
        filled.then_some(fill),
        d.f64()?,
        Opacity::new(d.f64()?).map_err(invalid)?,
    )
    .map_err(invalid)?;
    let kind = match kind {
        3 => {
            let size = d.f64()?;
            let align = match d.byte()? {
                0 => TextAlignment::Left,
                1 => TextAlignment::Center,
                2 => TextAlignment::Right,
                _ => return Err(StorageError::Unsupported("text alignment")),
            };
            let len = d.u32()? as usize;
            if len > MAX_TEXT_BYTES {
                return Err(StorageError::Invalid("text length"));
            }
            let text = std::str::from_utf8(d.take(len)?)
                .map_err(invalid)?
                .to_owned();
            AnnotationKind::Text(TextObject::new(text, size, align).map_err(invalid)?)
        }
        4 => AnnotationKind::Rect,
        6 | 7 => {
            let l =
                LineObject::new([[d.f64()?, d.f64()?], [d.f64()?, d.f64()?]]).map_err(invalid)?;
            if kind == 6 {
                AnnotationKind::Line(l)
            } else {
                AnnotationKind::Arrow(l)
            }
        }
        8 => {
            let count = d.u32()? as usize;
            if !(2..=MAX_STROKE_POINTS).contains(&count) {
                return Err(StorageError::Invalid("stroke point count"));
            }
            let bytes = d.take(count * 16)?;
            let mut r = Decoder::new(bytes);
            let mut points = Vec::with_capacity(count);
            for _ in 0..count {
                points.push([r.f64()?, r.f64()?]);
            }
            AnnotationKind::Scribble(ScribbleObject::new(points).map_err(invalid)?)
        }
        _ => return Err(StorageError::Unsupported("annotation kind")),
    };
    d.finish()?;
    DocumentObject::annotation(id, Annotation::new(kind, style), t).map_err(invalid)
}
