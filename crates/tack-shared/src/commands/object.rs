use crate::{Error, Result, WireId, domain_error};
use serde::{Deserialize, Serialize};
use tack_core::*;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SourceDto {
    pub id: WireId,
    pub revision: u64,
}
impl SourceDto {
    pub fn from_source(source: &Source) -> Result<Self> {
        Ok(Self {
            id: WireId::new(source.id().value())?,
            revision: source.revision(),
        })
    }
    pub fn to_source(&self) -> Result<Source> {
        Source::from_descriptor(
            SourceId::new(self.id.value()).map_err(domain_error)?,
            SourceLocation::Embedded,
            self.revision,
            None,
        )
        .map_err(domain_error)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AssetDto {
    pub id: WireId,
    pub source: WireId,
    pub pixels: [u32; 2],
}
impl AssetDto {
    pub fn from_asset(asset: ImageAsset) -> Result<Self> {
        Ok(Self {
            id: WireId::new(asset.id().value())?,
            source: WireId::new(asset.source_id().value())?,
            pixels: asset.pixel_size(),
        })
    }
    pub fn to_asset(&self) -> Result<ImageAsset> {
        ImageAsset::new(
            AssetId::new(self.id.value()).map_err(domain_error)?,
            SourceId::new(self.source.value()).map_err(domain_error)?,
            self.pixels,
        )
        .map_err(domain_error)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TransformDto {
    pub center: [f64; 2],
    pub size: [f64; 2],
    pub rotation: f64,
    pub flips: [bool; 2],
}
impl TransformDto {
    pub fn from_transform(t: Transform) -> Self {
        Self {
            center: t.center(),
            size: t.size(),
            rotation: t.rotation(),
            flips: t.flips(),
        }
    }
    pub fn to_transform(&self) -> Result<Transform> {
        Transform::new(self.center, self.size, self.rotation, self.flips).map_err(domain_error)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct StyleDto {
    pub stroke: [u8; 4],
    pub fill: Option<[u8; 4]>,
    pub width: f64,
    pub opacity: f64,
}
impl StyleDto {
    pub fn from_style(s: AnnotationStyle) -> Self {
        Self {
            stroke: s.stroke().0,
            fill: s.fill().map(|c| c.0),
            width: s.width(),
            opacity: s.opacity().value(),
        }
    }
    pub fn to_style(&self) -> Result<AnnotationStyle> {
        AnnotationStyle::new(
            Color(self.stroke),
            self.fill.map(Color),
            self.width,
            Opacity::new(self.opacity).map_err(domain_error)?,
        )
        .map_err(domain_error)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TextDto {
    pub value: String,
    pub font_size: f64,
    pub alignment: u8,
}
impl TextDto {
    pub fn from_text(t: &TextObject) -> Self {
        Self {
            value: t.value().to_owned(),
            font_size: t.font_size(),
            alignment: match t.alignment() {
                TextAlignment::Left => 0,
                TextAlignment::Center => 1,
                TextAlignment::Right => 2,
            },
        }
    }
    pub fn to_text(&self) -> Result<TextObject> {
        TextObject::new(
            self.value.clone(),
            self.font_size,
            match self.alignment {
                0 => TextAlignment::Left,
                1 => TextAlignment::Center,
                2 => TextAlignment::Right,
                _ => return Err(Error::Invalid("text alignment")),
            },
        )
        .map_err(domain_error)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum AnnotationDto {
    Rect,
    Text { text: TextDto },
    Line { points: [[f64; 2]; 2] },
    Arrow { points: [[f64; 2]; 2] },
    Scribble { points: Vec<[f64; 2]> },
}
impl AnnotationDto {
    fn from_kind(kind: &AnnotationKind) -> Self {
        match kind {
            AnnotationKind::Rect => Self::Rect,
            AnnotationKind::Text(text) => Self::Text {
                text: TextDto::from_text(text),
            },
            AnnotationKind::Line(line) => Self::Line {
                points: line.points(),
            },
            AnnotationKind::Arrow(line) => Self::Arrow {
                points: line.points(),
            },
            AnnotationKind::Scribble(stroke) => Self::Scribble {
                points: stroke.points().to_vec(),
            },
        }
    }
    fn to_kind(&self) -> Result<AnnotationKind> {
        Ok(match self {
            Self::Rect => AnnotationKind::Rect,
            Self::Text { text } => AnnotationKind::Text(text.to_text()?),
            Self::Line { points } => {
                AnnotationKind::Line(LineObject::new(*points).map_err(domain_error)?)
            }
            Self::Arrow { points } => {
                AnnotationKind::Arrow(LineObject::new(*points).map_err(domain_error)?)
            }
            Self::Scribble { points } => {
                if points.len() > MAX_STROKE_POINTS {
                    return Err(Error::Invalid("scribble point budget"));
                }
                AnnotationKind::Scribble(ScribbleObject::new(points.clone()).map_err(domain_error)?)
            }
        })
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ObjectDto {
    Image {
        id: WireId,
        asset: WireId,
        transform: TransformDto,
        crop: [f64; 4],
        opacity: f64,
        filtering: u8,
    },
    Frame {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        color: Option<[u8; 4]>,
        id: WireId,
        name: String,
        transform: TransformDto,
    },
    Annotation {
        id: WireId,
        annotation: AnnotationDto,
        style: StyleDto,
        transform: TransformDto,
    },
}
impl ObjectDto {
    pub fn from_object(object: &DocumentObject) -> Result<Self> {
        let id = WireId::new(object.id().value())?;
        let transform = TransformDto::from_transform(object.transform());
        Ok(match object.kind() {
            ObjectKind::Image(image) => Self::Image {
                id,
                asset: WireId::new(image.asset_id().value())?,
                transform,
                crop: image.crop().uv_rect(),
                opacity: image.opacity().value(),
                filtering: encode_filtering(image.filtering()),
            },
            ObjectKind::Frame(name) => Self::Frame {
                color: (object.frame_color() != tack_core::DEFAULT_FRAME_COLOR)
                    .then_some(object.frame_color().0),
                id,
                name: name.clone(),
                transform,
            },
            ObjectKind::Annotation(a) => Self::Annotation {
                id,
                annotation: AnnotationDto::from_kind(a.kind()),
                style: StyleDto::from_style(a.style()),
                transform,
            },
        })
    }
    pub fn to_object(&self) -> Result<DocumentObject> {
        match self {
            Self::Image {
                id,
                asset,
                transform,
                crop,
                opacity,
                filtering,
            } => Ok(DocumentObject::image_with_properties(
                ObjectId::new(id.value()).map_err(domain_error)?,
                AssetId::new(asset.value()).map_err(domain_error)?,
                transform.to_transform()?,
                Crop::new(crop[0], crop[1], crop[2], crop[3]).map_err(domain_error)?,
                Opacity::new(*opacity).map_err(domain_error)?,
                decode_filtering(*filtering)?,
            )),
            Self::Frame {
                color,
                id,
                name,
                transform,
            } => DocumentObject::frame_with_color(
                ObjectId::new(id.value()).map_err(domain_error)?,
                name.clone(),
                transform.to_transform()?,
                color.map_or(tack_core::DEFAULT_FRAME_COLOR, tack_core::Color),
            )
            .map_err(domain_error),
            Self::Annotation {
                id,
                annotation,
                style,
                transform,
            } => DocumentObject::annotation(
                ObjectId::new(id.value()).map_err(domain_error)?,
                Annotation::new(annotation.to_kind()?, style.to_style()?),
                transform.to_transform()?,
            )
            .map_err(domain_error),
        }
    }
}
pub fn encode_filtering(f: ImageFiltering) -> u8 {
    match f {
        ImageFiltering::Default => 0,
        ImageFiltering::Smooth => 1,
        ImageFiltering::Nearest => 2,
    }
}
pub fn decode_filtering(f: u8) -> Result<ImageFiltering> {
    match f {
        0 => Ok(ImageFiltering::Default),
        1 => Ok(ImageFiltering::Smooth),
        2 => Ok(ImageFiltering::Nearest),
        _ => Err(Error::Invalid("image filtering")),
    }
}
