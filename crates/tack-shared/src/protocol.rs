use crate::{CommandDto, ContentHash, DocumentRecord, SourceBinding, WireId};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct LeaseRecord {
    pub object: WireId,
    pub client: WireId,
    pub operation: WireId,
    pub ttl_ms: u32,
}
pub const MAX_LEASE_TARGETS: usize = 256;
pub const MAX_BOARD_LEASES: usize = 1024;
pub const LEASE_TTL_MS: u32 = 5000;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RefusalCode {
    StaleRevision,
    InvalidOperation,
    Conflict,
    VersionMismatch,
    BoardUnavailable,
    Busy,
    AssetUnavailable,
    InvalidAsset,
    Persistence,
    Protocol,
}

/// All variants have independent stable names. Unknown fields/variants are refused.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Message {
    LeaseAcquire {
        operation: WireId,
        base: u64,
        objects: Vec<WireId>,
    },
    LeaseRelease {
        operation: WireId,
    },
    LeaseSnapshot {
        leases: Vec<LeaseRecord>,
    },
    LeaseChanged {
        operation: Option<WireId>,
        client: Option<WireId>,
        objects: Vec<WireId>,
        ttl_ms: u32,
    },
    LeaseDenied {
        operation: WireId,
        reason: String,
    },
    Hello {
        board: WireId,
        client: WireId,
        revision: u64,
    },
    Snapshot {
        board: WireId,
        revision: u64,
        source_high_water: u64,
        document: DocumentRecord,
        sources: Vec<SourceBinding>,
        clients: u32,
    },
    Publish {
        board: WireId,
        client: WireId,
        document: DocumentRecord,
        sources: Vec<SourceBinding>,
    },
    Edit {
        operation: WireId,
        base: u64,
        command: CommandDto,
        sources: Vec<SourceBinding>,
    },
    Accepted {
        board: WireId,
        revision: u64,
        client: WireId,
        operation: WireId,
        command: CommandDto,
        sources: Vec<SourceBinding>,
    },
    Undo {
        operation: WireId,
        base: u64,
    },
    Redo {
        operation: WireId,
        base: u64,
    },
    Refused {
        operation: Option<WireId>,
        revision: u64,
        code: RefusalCode,
        reason: String,
    },
    AssetBegin {
        hash: ContentHash,
        size: u64,
    },
    AssetChunk {
        hash: ContentHash,
        offset: u64,
        bytes: String,
    },
    AssetCommit {
        hash: ContentHash,
    },
    AssetStatus {
        hash: ContentHash,
        size: u64,
        present: bool,
    },
    AssetProgress {
        hash: ContentHash,
        offset: u64,
    },
    AssetReady {
        hash: ContentHash,
        size: u64,
    },
    AssetGet {
        hash: ContentHash,
        offset: u64,
        length: u32,
    },
    AssetData {
        hash: ContentHash,
        offset: u64,
        size: u64,
        bytes: String,
    },
    AssetCancelled {
        hash: ContentHash,
    },
    AssetCancel {
        hash: ContentHash,
    },
}
