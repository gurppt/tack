use crate::{AssetId, Crop, ObjectId, Opacity, SourceId, Transform};
use std::{
    error::Error,
    fmt,
    path::{Path, PathBuf},
};

/// Document override; Default delegates to a local user preference.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ImageFiltering {
    #[default]
    Default,
    Smooth,
    Nearest,
}

pub const MAX_SOURCE_PATH_BYTES: usize = 4096;

/// Locator only. Embedded bytes will be resolved by storage using SourceId;
/// linked paths are opaque platform paths. Constructing this performs no I/O.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SourceLocation {
    Embedded,
    Linked(PathBuf),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Source {
    id: SourceId,
    location: SourceLocation,
}
impl Source {
    pub fn embedded(id: SourceId) -> Self {
        Self {
            id,
            location: SourceLocation::Embedded,
        }
    }
    pub fn linked(id: SourceId, path: impl AsRef<Path>) -> Result<Self, ModelError> {
        let path = path.as_ref();
        let bytes = path.as_os_str().as_encoded_bytes();
        if bytes.is_empty() || bytes.len() > MAX_SOURCE_PATH_BYTES || bytes.contains(&0) {
            return Err(ModelError::InvalidSourcePath);
        }
        // Copy only the bounded descriptor, not a possibly over-capacity input.
        Ok(Self {
            id,
            location: SourceLocation::Linked(path.to_path_buf()),
        })
    }
    pub fn id(&self) -> SourceId {
        self.id
    }
    pub fn location(&self) -> &SourceLocation {
        &self.location
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
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ObjectKind {
    Image(ImageObject),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DocumentObject {
    pub(crate) id: ObjectId,
    pub(crate) transform: Transform,
    pub(crate) kind: ObjectKind,
}
impl DocumentObject {
    pub fn image(id: ObjectId, asset: AssetId, transform: Transform) -> Self {
        Self {
            id,
            transform,
            kind: ObjectKind::Image(ImageObject::new(asset)),
        }
    }
    pub fn id(self) -> ObjectId {
        self.id
    }
    pub fn transform(self) -> Transform {
        self.transform
    }
    pub fn kind(self) -> ObjectKind {
        self.kind
    }
    pub(crate) fn image_mut(&mut self) -> &mut ImageObject {
        match &mut self.kind {
            ObjectKind::Image(image) => image,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelError {
    InvalidSourcePath,
    InvalidPixelSize,
}
impl fmt::Display for ModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid image metadata: {self:?}")
    }
}
impl Error for ModelError {}
