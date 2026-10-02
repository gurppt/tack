//! GPU-independent geometry and streaming policy for the Mission 0 experiment.

mod camera;
mod residency;

pub use camera::{Camera, GeometryError, WorldRect};
pub use residency::{ByteCache, Lod};
