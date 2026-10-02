//! Bounded asynchronous display-image pipeline.
mod board;
mod decode;
mod loader;
mod profile;

pub use board::{Board, ImageObject};
pub use loader::{AssetKey, DecodeRequest, Decoded, Loader, LoaderStats, MAX_PENDING_PER_WORKER};
pub use profile::{JobProfile, STAGE_NAMES};

pub type AssetError = Box<dyn std::error::Error + Send + Sync>;
