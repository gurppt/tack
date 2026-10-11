//! Persistent whole-image LOD keys must survive revision reuse after reopening.
//! Hash only bounded descriptors here: no original read or render-thread I/O.
use super::*;
use sha2::{Digest, Sha256};
pub(super) fn namespace(job: &Job, path: Option<&Path>) -> String {
    let mut hash = Sha256::new();
    hash.update(b"tack-whole-lod-v2");
    hash.update(job.board.document.id().value().to_le_bytes());
    for size in job.asset.pixel_size() {
        hash.update(size.to_le_bytes());
    }
    if let Some(path) = path {
        hash.update(b"linked");
        hash.update(path.as_os_str().as_encoded_bytes());
        // A missing/changed link may retain the recorded last-known preview,
        // but newly admitted source descriptors must never find that old key.
        let fingerprint = if job.shared_path.is_some() {
            representation::source_fingerprint(path).ok()
        } else {
            job.source
                .fingerprint()
                .or_else(|| representation::source_fingerprint(path).ok())
        };
        if let Some(f) = fingerprint {
            hash.update(f.size.to_le_bytes());
            hash.update(f.modified_seconds.to_le_bytes());
            hash.update(f.modified_nanos.to_le_bytes());
        }
    } else {
        hash.update(b"embedded");
        let range = match &job.original {
            Some(Payload::Stored { range, .. }) => Some(*range),
            _ => job.board.originals.get(&job.source.id()).map(|o| o.range),
        };
        if let Some(range) = range {
            hash.update(range.len.to_le_bytes());
            hash.update(range.crc32.to_le_bytes());
        }
    }
    format!("{:x}", hash.finalize())
}
