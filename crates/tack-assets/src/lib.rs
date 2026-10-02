//! Bounded asynchronous display-image pipeline.
mod board;
mod decode;
mod loader;

pub use board::{Board, ImageObject};
pub use loader::{AssetKey, DecodeRequest, Decoded, Loader, LoaderStats};

pub type AssetError = Box<dyn std::error::Error + Send + Sync>;
