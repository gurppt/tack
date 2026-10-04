use crate::{AssetId, Crop, ObjectId, Opacity, SourceId, Transform};
use std::{error::Error, fmt, path::Path};

/// Document override; Default delegates to a local user preference.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ImageFiltering {
    #[default]
    Default,
    Smooth,
    Nearest,
}

pub const MAX_SOURCE_PATH_BYTES: usize = 4096;

/// Metadata only. Embedded bytes are resolved outside core using SourceId.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SourceLocation {
    Embedded,
    Linked(crate::LinkedPath),
}

/// Cheap observed file metadata, not a strong content identity or freshness proof.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceFingerprint {
    pub size: u64,
    pub modified_seconds: i64,
    pub modified_nanos: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Source {
    id: SourceId,
    location: SourceLocation,
    revision: u64,
    fingerprint: Option<SourceFingerprint>,
}
impl Source {
    pub fn embedded(id: SourceId) -> Self {
        Self {
            id,
            location: SourceLocation::Embedded,
            revision: 1,
            fingerprint: None,
        }
    }
    pub fn linked(id: SourceId, path: impl AsRef<Path>) -> Result<Self, ModelError> {
        Self::from_descriptor(
            id,
            SourceLocation::Linked(crate::LinkedPath::native(path.as_ref())?),
            1,
            None,
        )
    }
    /// Validated source replacement also serves the explicit persistence adapter.
    pub fn from_descriptor(
        id: SourceId,
        location: SourceLocation,
        revision: u64,
        fingerprint: Option<SourceFingerprint>,
    ) -> Result<Self, ModelError> {
        if revision == 0 || fingerprint.is_some_and(|f| f.modified_nanos >= 1_000_000_000) {
            return Err(ModelError::InvalidSourceRevision);
        }
        Ok(Self {
            id,
            location,
            revision,
            fingerprint,
        })
    }
    pub fn id(&self) -> SourceId {
        self.id
    }
    pub fn location(&self) -> &SourceLocation {
        &self.location
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn fingerprint(&self) -> Option<SourceFingerprint> {
        self.fingerprint
    }
}

/// Immutable logical image metadata. No decoded representation is document state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImageAsset {
    id: AssetId,
    source: SourceId,
    pixel_size: [u32; 2],
}
impl ImageAsset {
    pub fn new(id: AssetId, source: SourceId, pixel_size: [u32; 2]) -> Result<Self, ModelError> {
        if pixel_size.contains(&0) {
            return Err(ModelError::InvalidPixelSize);
        }
        Ok(Self {
            id,
            source,
            pixel_size,
        })
    }
    pub fn id(self) -> AssetId {
        self.id
    }
    pub fn source_id(self) -> SourceId {
        self.source
    }
    pub fn pixel_size(self) -> [u32; 2] {
        self.pixel_size
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ImageObject {
    pub(crate) asset: AssetId,
    pub(crate) crop: Crop,
    pub(crate) opacity: Opacity,
    pub(crate) filtering: ImageFiltering,
}
impl ImageObject {
    pub fn new(asset: AssetId) -> Self {
        Self {
            asset,
            crop: Crop::FULL,
            opacity: Opacity::OPAQUE,
            filtering: ImageFiltering::Default,
        }
    }
    pub fn asset_id(self) -> AssetId {
        self.asset
    }
    pub fn crop(self) -> Crop {
        self.crop
    }
    pub fn opacity(self) -> Opacity {
        self.opacity
    }
    pub fn filtering(self) -> ImageFiltering {
        self.filtering
    }
}

/// Explicit extensibility point, containing only implemented object kinds.
#[derive(Clone, Debug, PartialEq)]
pub enum ObjectKind {
    Image(ImageObject),
    Frame(String),
    Annotation(Box<crate::Annotation>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct DocumentObject {
    pub(crate) id: ObjectId,
    pub(crate) transform: Transform,
    pub(crate) kind: ObjectKind,
}
impl DocumentObject {
    pub fn annotation(
        id: ObjectId,
        annotation: crate::Annotation,
        transform: Transform,
    ) -> Result<Self, crate::GeometryError> {
        annotation.bounds(transform)?;
        Ok(Self {
            id,
            transform,
            kind: ObjectKind::Annotation(Box::new(annotation)),
        })
    }
    pub fn bounds(&self) -> crate::WorldRect {
        match self.kind() {
            ObjectKind::Annotation(a) => {
                a.bounds(self.transform).unwrap_or(self.transform.bounds())
            }
            _ => self.transform.bounds(),
        }
    }
    pub fn image(id: ObjectId, asset: AssetId, transform: Transform) -> Self {
        Self {
            id,
            transform,
            kind: ObjectKind::Image(ImageObject::new(asset)),
        }
    }
    pub fn frame(id: ObjectId, name: String, transform: Transform) -> Result<Self, ModelError> {
        validate_frame_name(&name)?;
        if transform.rotation() != 0. || transform.flips() != [false; 2] {
            return Err(ModelError::InvalidFrame);
        }
        Ok(Self {
            id,
            transform,
            kind: ObjectKind::Frame(name),
        })
    }
    pub fn id(&self) -> ObjectId {
        self.id
    }
    pub fn transform(&self) -> Transform {
        self.transform
    }
    pub fn kind(&self) -> &ObjectKind {
        &self.kind
    }
    pub(crate) fn image_mut(&mut self) -> Option<&mut ImageObject> {
        match &mut self.kind {
            ObjectKind::Image(image) => Some(image),
            _ => None,
        }
    }
}

pub const MAX_FRAME_NAME_BYTES: usize = 256;
pub fn validate_frame_name(name: &str) -> Result<(), ModelError> {
    if name.trim().is_empty()
        || name.len() > MAX_FRAME_NAME_BYTES
        || name.chars().any(char::is_control)
    {
        return Err(ModelError::InvalidFrame);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelError {
    InvalidSourcePath,
    InvalidPixelSize,
    InvalidSourceRevision,
    InvalidFrame,
    InvalidAnnotation,
}
impl fmt::Display for ModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid image metadata: {self:?}")
    }
}
impl Error for ModelError {}
