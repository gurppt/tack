//! Bounded asynchronous display-image pipeline.
mod board;
mod decode;
mod jpeg_header;
mod jpeg_scanlines;
mod loader;
mod native_thumbnail;
mod preparation;
mod product;
mod profile;
mod representation;
mod ui_image;
pub use product::{
    PreparedOverview, ProductAssetStats, ProductAssets, ProductDemand, RepTrace, SourceState,
    SupplyLimits,
};
pub use representation::{image_metadata, source_fingerprint};
pub use ui_image::decode_ui_png;

pub use board::{BenchmarkBoard, BenchmarkImage};
pub use loader::{AssetKey, DecodeRequest, Decoded, Loader, LoaderStats, MAX_PENDING_PER_WORKER};
pub use native_thumbnail::NativeThumbnail;
pub use preparation::OverviewPreparation;
pub use profile::{JobProfile, STAGE_NAMES};

pub type AssetError = Box<dyn std::error::Error + Send + Sync>;

pub const THUMBNAIL_DECODER_ID: &str = "turbojpeg 1.5.1 / libjpeg-turbo 3.2.0 DCT";
pub mod huge_image;
