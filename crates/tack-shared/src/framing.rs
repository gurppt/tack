use crate::{
    Error, MAX_CHUNK_BYTES, MAX_FRAME_BYTES, MAX_OPERATION_BYTES, Message, PROTOCOL_MAJOR, Result,
    decode_hex,
};
use std::io::{Read, Write};
const HEADER_BYTES: usize = 10;
const MAGIC: &[u8; 4] = b"TLAN";

/// Encode JSON payload only; useful for bounded persistence and independent tests.
pub fn encode_message(message: &Message) -> Result<Vec<u8>> {
    validate_message(message)?;
    let bytes = serde_json::to_vec(message)?;
    validate_payload_size(message, bytes.len())?;
    Ok(bytes)
}
pub fn decode_message(bytes: &[u8]) -> Result<Message> {
    if bytes.len() > MAX_FRAME_BYTES {
        return Err(Error::Invalid("frame size budget"));
    }
    let message: Message = serde_json::from_slice(bytes)?;
    validate_payload_size(&message, bytes.len())?;
    validate_message(&message)?;
    Ok(message)
}
fn validate_payload_size(message: &Message, size: usize) -> Result<()> {
    let maximum = match message {
        Message::Snapshot { .. } | Message::Publish { .. } => MAX_FRAME_BYTES,
        _ => MAX_OPERATION_BYTES,
    };
    if size > maximum {
        return Err(Error::Invalid("message size budget"));
    }
    Ok(())
}
pub fn write_message(writer: &mut impl Write, message: &Message) -> Result<usize> {
    let payload = encode_message(message)?;
    let mut header = [0u8; HEADER_BYTES];
    header[..4].copy_from_slice(MAGIC);
    header[4..6].copy_from_slice(&PROTOCOL_MAJOR.to_be_bytes());
    let size = u32::try_from(payload.len()).map_err(|_| Error::Invalid("frame length overflow"))?;
    header[6..10].copy_from_slice(&size.to_be_bytes());
    writer.write_all(&header)?;
    writer.write_all(&payload)?;
    writer.flush()?;
    Ok(HEADER_BYTES + payload.len())
}
pub fn read_message(reader: &mut impl Read) -> Result<Message> {
    let mut header = [0u8; HEADER_BYTES];
    reader.read_exact(&mut header)?;
    if &header[..4] != MAGIC {
        return Err(Error::Invalid("frame magic"));
    }
    let major = u16::from_be_bytes([header[4], header[5]]);
    if major != PROTOCOL_MAJOR {
        return Err(Error::Version(major));
    }
    let size = u32::from_be_bytes([header[6], header[7], header[8], header[9]]) as usize;
    if size == 0 || size > MAX_FRAME_BYTES {
        return Err(Error::Invalid("frame size budget"));
    }
    let mut bytes = vec![0u8; size];
    reader.read_exact(&mut bytes)?;
    decode_message(&bytes)
}
fn validate_bindings(bindings: &[crate::SourceBinding]) -> Result<()> {
    if bindings.len() > tack_storage::MAX_RECORDS {
        return Err(Error::Invalid("source binding count"));
    }
    let mut seen = std::collections::BTreeSet::new();
    for binding in bindings {
        binding.validate()?;
        if !seen.insert(binding.source) {
            return Err(Error::Invalid("duplicate source binding"));
        }
    }
    Ok(())
}
fn validate_message(message: &Message) -> Result<()> {
    use Message::*;
    match message {
        LeaseAcquire { objects, .. } => validate_lease_objects(objects, crate::MAX_LEASE_TARGETS)?,
        LeaseSnapshot { leases } => {
            let objects: Vec<_> = leases.iter().map(|lease| lease.object).collect();
            if !leases.is_empty() {
                validate_lease_objects(&objects, crate::MAX_BOARD_LEASES)?;
            }
            if leases
                .iter()
                .any(|lease| lease.ttl_ms == 0 || lease.ttl_ms > crate::LEASE_TTL_MS)
            {
                return Err(Error::Invalid("lease lifetime"));
            }
        }
        LeaseChanged {
            objects,
            ttl_ms,
            client,
            operation,
        } => {
            validate_lease_objects(objects, crate::MAX_BOARD_LEASES)?;
            if *ttl_ms > crate::LEASE_TTL_MS
                || client.is_some() != operation.is_some()
                || client.is_some() != (*ttl_ms > 0)
            {
                return Err(Error::Invalid("lease state"));
            }
        }
        LeaseDenied { reason, .. }
            if reason.len() > 512 || reason.chars().any(char::is_control) =>
        {
            return Err(Error::Invalid("lease reason"));
        }
        Snapshot {
            board,
            document,
            sources,
            clients,
            source_high_water,
            ..
        } => {
            if *clients > 64 {
                return Err(Error::Invalid("client count"));
            }
            let live_max = validate_document(*board, document, sources)?;
            if *source_high_water < live_max {
                return Err(Error::Invalid("source revision high water"));
            }
        }
        Publish {
            board,
            document,
            sources,
            ..
        } => {
            validate_document(*board, document, sources)?;
        }
        Edit {
            command, sources, ..
        }
        | Accepted {
            command, sources, ..
        } => {
            command.to_command()?;
            validate_bindings(sources)?;
        }
        Refused { reason, .. } if reason.len() > 512 || reason.chars().any(char::is_control) => {
            return Err(Error::Invalid("refusal reason budget/encoding"));
        }
        AssetBegin { size, .. } | AssetReady { size, .. } | AssetStatus { size, .. }
            if *size > crate::MAX_ASSET_BYTES =>
        {
            return Err(Error::Invalid("asset size budget"));
        }
        AssetChunk { offset, bytes, .. } => {
            let chunk = decode_hex(bytes, MAX_CHUNK_BYTES)?;
            if chunk.is_empty()
                || offset
                    .checked_add(chunk.len() as u64)
                    .is_none_or(|end| end > crate::MAX_ASSET_BYTES)
            {
                return Err(Error::Invalid("asset chunk range"));
            }
        }
        AssetData {
            offset,
            size,
            bytes,
            ..
        } => {
            let chunk = decode_hex(bytes, MAX_CHUNK_BYTES)?;
            if *size > crate::MAX_ASSET_BYTES
                || *offset > *size
                || offset
                    .checked_add(chunk.len() as u64)
                    .is_none_or(|end| end > *size)
            {
                return Err(Error::Invalid("asset data range"));
            }
        }
        AssetGet { offset, length, .. }
            if *length == 0
                || *length as usize > MAX_CHUNK_BYTES
                || offset
                    .checked_add(u64::from(*length))
                    .is_none_or(|end| end > crate::MAX_ASSET_BYTES) =>
        {
            return Err(Error::Invalid("asset request range"));
        }
        AssetProgress { offset, .. } if *offset > crate::MAX_ASSET_BYTES => {
            return Err(Error::Invalid("asset progress range"));
        }
        _ => {}
    }
    Ok(())
}
fn validate_lease_objects(objects: &[crate::WireId], maximum: usize) -> Result<()> {
    if objects.is_empty()
        || objects.len() > maximum
        || objects
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != objects.len()
    {
        return Err(Error::Invalid("lease object count or duplicate"));
    }
    Ok(())
}
fn validate_document(
    board: crate::WireId,
    record: &crate::DocumentRecord,
    sources: &[crate::SourceBinding],
) -> Result<u64> {
    let document = record.to_document()?;
    if document.id().value() != board.value() {
        return Err(Error::Invalid("board/document identity"));
    }
    validate_bindings(sources)?;
    for binding in sources {
        let source =
            tack_core::SourceId::new(binding.source.value()).map_err(crate::domain_error)?;
        if document
            .source(source)
            .is_none_or(|source| source.revision() != binding.revision)
        {
            return Err(Error::Invalid("source binding mismatch"));
        }
    }
    Ok(document
        .sources()
        .map(|source| source.revision())
        .max()
        .unwrap_or(0))
}
