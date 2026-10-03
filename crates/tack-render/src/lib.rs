//! Native GPU renderer; document source and storage are outside this boundary.
mod gpu;
mod timing;

pub use gpu::{
    DrawImage, DrawProductImage, Gpu, MAX_UPLOADS, ProductKey, RenderStats, product_quad,
};
pub use timing::GpuSample;
