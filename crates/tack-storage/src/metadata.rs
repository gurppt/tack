//! Explicit metadata adapter shared by snapshots and the versioned LAN envelope.
use crate::{Result, StorageError, codec};
use tack_core::Document;

pub fn encode_metadata(document: &Document) -> Result<(u32, [u32; 3], Vec<u8>)> {
    let counts = [
        document.sources().count(),
        document.assets().count(),
        document.object_order().len(),
    ];
    let mut encoded_counts = [0; 3];
    for (index, count) in counts.into_iter().enumerate() {
        encoded_counts[index] =
            u32::try_from(count).map_err(|_| StorageError::Invalid("metadata record count"))?;
    }
    Ok((
        codec::spatial_schema(document),
        encoded_counts,
        codec::encode_document(document)?,
    ))
}
pub fn decode_metadata(schema: u32, counts: [u32; 3], bytes: &[u8]) -> Result<Document> {
    if !(1..=4).contains(&schema) {
        return Err(StorageError::Unsupported("metadata schema"));
    }
    if bytes.len() > crate::MAX_METADATA_BYTES
        || counts.iter().any(|&n| n as usize > crate::MAX_RECORDS)
    {
        return Err(StorageError::Invalid("metadata budget/count"));
    }
    let counts = counts.map(|n| n as usize);
    // Each source/asset/object consumes at least one complete fixed record.
    let minimum = 16usize
        .saturating_add(counts[0].saturating_mul(31))
        .saturating_add(counts[1].saturating_mul(42))
        .saturating_add(counts[2].saturating_mul(20));
    if bytes.len() < minimum {
        return Err(StorageError::Invalid("truncated metadata counts"));
    }
    let mut decoder = codec::Decoder::new(bytes);
    let document = codec::decode_document(&mut decoder, counts, schema)?;
    decoder.finish()?;
    Ok(document)
}
