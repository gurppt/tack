//! Optional trusted-LAN authority; no renderer or GUI dependency.
mod assets;
mod authority;
mod persistence;
mod service;
pub use assets::{AssetStore, Upload};
pub use authority::{Authority, Change};
pub use service::{ServerConfig, serve};

pub type Result<T> = std::result::Result<T, String>;
pub const MAX_CLIENTS: usize = 16;
pub const MAX_BOARDS: usize = 16;
pub const HISTORY_PER_CLIENT: usize = 32;
pub const RECEIPTS_PER_CLIENT: usize = 128;
pub const MAX_HISTORY_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_AUTHORITY_BYTES: usize = 128 * 1024 * 1024;
