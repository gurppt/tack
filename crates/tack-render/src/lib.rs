//! Native GPU renderer; document source and storage are outside this boundary.
mod gpu;
mod timing;

pub use gpu::{
    DrawImage, DrawProductImage, Gpu, MAX_UPLOADS, ProductKey, RenderStats, product_quad,
};
pub use timing::GpuSample;

mod overlay;
mod pixel_grid;
pub use overlay::{MAX_OVERLAY_QUADS, OverlayQuad};
mod grid;
pub use grid::GridView;
mod annotations;
pub use annotations::{
    AnnotationDraws, AnnotationPrimitive, CanvasDraw, MAX_ANNOTATION_PRIMITIVES,
};
