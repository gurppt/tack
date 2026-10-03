//! Resident document metadata, deterministic commands and streaming policy.
//! No UI, storage, window, GPU, network or decoder dependencies.

mod camera;
mod commands;
mod document;
mod history;
mod ids;
mod model;
mod query;
mod residency;
mod transform;

pub use camera::{Camera, GeometryError, WorldRect};
pub use commands::{Command, CommandError};
pub use document::{Document, DocumentLimits};
pub use history::DocumentEditor;
pub use ids::{AssetId, DocumentId, InvalidId, ObjectId, SourceId};
pub use model::{
    DocumentObject, ImageAsset, ImageFiltering, ImageObject, MAX_SOURCE_PATH_BYTES, ModelError,
    ObjectKind, Source, SourceLocation,
};
pub use query::{DocumentQuery, ImageRenderData};
pub use residency::{ByteCache, Lod};
pub use transform::{Crop, Opacity, Transform};
