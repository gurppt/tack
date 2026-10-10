//! Optional LAN protocol. Merely linking this crate starts no workers or sockets.
mod commands;
mod domain;
mod framing;
mod identity;
mod protocol;
mod scope;
pub use commands::{AssetDto, CommandDto, ObjectDto, SourceDto, StyleDto, TextDto, TransformDto};
pub use domain::{DocumentRecord, SourceBinding, shared_document};
pub use framing::{decode_message, encode_message, read_message, write_message};
pub use identity::{ContentHash, WireId, decode_hex, encode_hex, hash_reader};
pub use protocol::{
    LEASE_TTL_MS, LeaseRecord, MAX_BOARD_LEASES, MAX_LEASE_TARGETS, Message, RefusalCode,
};
pub use scope::CommandScope;

pub const PROTOCOL_MAJOR: u16 = 3;
pub const MAX_FRAME_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_OPERATION_BYTES: usize = 1024 * 1024;
pub const MAX_CHUNK_BYTES: usize = 64 * 1024;
pub const MAX_ASSET_BYTES: u64 = 8 * 1024 * 1024 * 1024;
pub const MAX_QUEUE_MESSAGES: usize = 64;
pub type Result<T> = std::result::Result<T, Error>;
#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    Json(serde_json::Error),
    Invalid(&'static str),
    Version(u16),
    Domain(String),
    Storage(tack_storage::StorageError),
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "LAN I/O: {e}"),
            Self::Json(e) => write!(f, "LAN JSON: {e}"),
            Self::Invalid(e) => write!(f, "invalid LAN message: {e}"),
            Self::Version(v) => write!(f, "unsupported LAN protocol major {v}"),
            Self::Domain(e) => write!(f, "invalid LAN document: {e}"),
            Self::Storage(e) => write!(f, "{e}"),
        }
    }
}
impl std::error::Error for Error {}
impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}
impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e)
    }
}
impl From<tack_storage::StorageError> for Error {
    fn from(e: tack_storage::StorageError) -> Self {
        Self::Storage(e)
    }
}
pub(crate) fn domain_error(e: impl std::fmt::Display) -> Error {
    Error::Domain(e.to_string())
}

pub mod cache;
pub mod client;
pub mod publish;
pub use client::{ClientConfig, ClientEvent, ConnectionState, SharedClient};
