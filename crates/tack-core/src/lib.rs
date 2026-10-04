//! Resident document metadata, deterministic commands and streaming policy.
//! No UI, storage, window, GPU, network or decoder dependencies.

mod annotations;
mod camera;
mod commands;
mod document;
mod history;
mod ids;
mod model;
mod query;
mod residency;
mod stroke;
mod transform;
pub use annotations::{
    Annotation, AnnotationKind, AnnotationStyle, Color, LineObject, MAX_STROKE_POINTS,
    MAX_TEXT_BYTES, ScribbleObject, TextAlignment, TextObject,
};
pub use stroke::{segment_distance, simplify_stroke};

pub use camera::{Camera, GeometryError, WorldRect};
pub use commands::{Command, CommandError};
pub use document::{Document, DocumentLimits};
pub use history::DocumentEditor;
pub use ids::{AssetId, DocumentId, GroupId, InvalidId, ObjectId, SourceId};
pub use model::{
    DocumentObject, ImageAsset, ImageFiltering, ImageObject, MAX_SOURCE_PATH_BYTES, ModelError,
    ObjectKind, Source, SourceFingerprint, SourceLocation,
};
pub use query::{DocumentQuery, ImageRenderData};
pub use residency::{ByteCache, Lod};
pub use transform::{Crop, Opacity, Transform};

mod source_path;
pub use source_path::{LinkedPath, PathPlatform};
mod groups;
pub use groups::Group;
pub use model::{MAX_FRAME_NAME_BYTES, validate_frame_name};
