//! Native GPU renderer; document source and storage are outside this boundary.
mod gpu;
mod timing;

pub use gpu::{DrawImage, Gpu, MAX_UPLOADS, RenderStats};
pub use timing::GpuSample;
