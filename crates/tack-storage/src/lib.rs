//! Explicit bounded snapshot storage. All functions are worker/startup operations.
mod annotation_codec;
mod codec;
mod metadata;
pub use metadata::{decode_metadata, encode_metadata};
mod ownership;
mod reader;
mod recovery;
pub use ownership::{BoardLease, lock_sidecar};
mod save;

pub use reader::{BlobRange, OverviewEntry, RangeReader, TackFile};
pub use save::{BlobInput, Payload, SaveStage, save, save_with_hook};
use std::{error::Error, fmt};
use tack_core::{AssetId, DocumentId, GroupId, ObjectId, SourceId};

pub const HEADER_BYTES: usize = 80;
pub const MAX_METADATA_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_RECORDS: usize = 100_000;
pub const MAX_OVERVIEW_BYTES: u64 = 1024 * 1024;
pub const MAX_FILE_BYTES: u64 = 8 * 1024 * 1024 * 1024 * 1024;
pub const DIRECTORY_ENTRY_BYTES: usize = 64;

#[derive(Debug)]
pub enum StorageError {
    Io(std::io::Error),
    PublishedButNotDirectorySynced(std::io::Error),
    Invalid(&'static str),
    Unsupported(&'static str),
    Corrupt(&'static str),
    Entropy(String),
}
impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "storage I/O: {e}"),
            Self::PublishedButNotDirectorySynced(e) => write!(
                f,
                "new file published but parent-directory sync failed: {e}"
            ),
            Self::Entropy(e) => write!(f, "OS entropy: {e}"),
            Self::Invalid(e) => write!(f, "invalid Tack file: {e}"),
            Self::Unsupported(e) => write!(f, "unsupported Tack file: {e}"),
            Self::Corrupt(e) => write!(f, "corrupt Tack data: {e}"),
        }
    }
}
impl Error for StorageError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(e) | Self::PublishedButNotDirectorySynced(e) => Some(e),
            _ => None,
        }
    }
}
impl From<std::io::Error> for StorageError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}
pub type Result<T> = std::result::Result<T, StorageError>;

/// Full 128 bits of OS entropy. Zero is resampled; generation never depends on a path.
fn random_id() -> Result<u128> {
    loop {
        let mut bytes = [0; 16];
        getrandom::fill(&mut bytes).map_err(|e| StorageError::Entropy(e.to_string()))?;
        let value = u128::from_le_bytes(bytes);
        if value != 0 {
            return Ok(value);
        }
    }
}
macro_rules! generator {
    ($f:ident, $t:ident) => {
        pub fn $f() -> Result<$t> {
            $t::new(random_id()?).map_err(|_| StorageError::Invalid("zero generated ID"))
        }
    };
}
generator!(new_document_id, DocumentId);
generator!(new_object_id, ObjectId);
generator!(new_asset_id, AssetId);
generator!(new_source_id, SourceId);
generator!(new_group_id, GroupId);

/// Private owned work directories; callers choose trusted paths. Existing directories
/// are not chmod'ed. Unix permissions are set at creation, before any media is written.
pub fn create_private_directory(path: &std::path::Path, recursive: bool) -> std::io::Result<()> {
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(recursive);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(path)
}
