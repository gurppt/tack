//! Bounded asynchronous display-image pipeline.
mod board;
mod decode;
mod loader;
mod native_thumbnail;
mod profile;

pub use board::{Board, ImageObject};
pub use loader::{AssetKey, DecodeRequest, Decoded, Loader, LoaderStats, MAX_PENDING_PER_WORKER};
pub use native_thumbnail::NativeThumbnail;
pub use profile::{JobProfile, STAGE_NAMES};

pub type AssetError = Box<dyn std::error::Error + Send + Sync>;

pub const THUMBNAIL_DECODER_ID: &str = "turbojpeg 1.5.1 / libjpeg-turbo 3.2.0 DCT";
