//! Explicit publish/export and original transfer helpers. Never called on the UI thread.
use crate::cache::CancelReader;
use crate::{
    CommandDto, DocumentRecord, Error, Message, Result, SourceBinding, WireId, decode_hex,
    encode_hex, hash_reader, read_message, write_message,
};
use std::{
    fs::File,
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tack_core::{Document, Source, SourceId, SourceLocation};
use tack_storage::{BlobInput, Payload, RangeReader, TackFile};

pub struct PreparedSource {
    pub binding: SourceBinding,
    pub payload: Payload,
}
pub trait MessageIo: Read + Write {
    fn message_sent(&self) {}
    fn message_received(&self) {}
}
impl MessageIo for TcpStream {}
pub(crate) fn send(stream: &mut impl MessageIo, message: &Message) -> Result<()> {
    write_message(stream, message)?;
    stream.message_sent();
    Ok(())
}
pub(crate) fn receive(stream: &mut impl MessageIo) -> Result<Message> {
    let message = read_message(stream)?;
    stream.message_received();
    Ok(message)
}
/// Numeric endpoint connection with bounded connect and optional transfer read deadlines.
pub fn connect(address: &str, timed_reads: bool) -> Result<TcpStream> {
    let address: SocketAddr = address
        .parse()
        .map_err(|_| Error::Invalid("server address must be numeric IP:port"))?;
    let stream = TcpStream::connect_timeout(&address, Duration::from_secs(5))?;
    stream.set_nodelay(true)?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    if timed_reads {
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    }
    Ok(stream)
}
pub(crate) fn check_cancel(cancel: &AtomicBool) -> Result<()> {
    if cancel.load(Ordering::Relaxed) {
        Err(Error::Invalid("shared transfer cancelled"))
    } else {
        Ok(())
    }
}
pub(crate) fn payload_reader(payload: &Payload) -> Result<Box<dyn Read>> {
    Ok(match payload {
        Payload::File(path) => Box::new(File::open(path)?),
        Payload::Stored { file, range } => Box::new(RangeReader::new(Arc::clone(file), *range)),
    })
}
fn linked_payload(source: &Source, board_path: &Path) -> Result<Payload> {
    let SourceLocation::Linked(descriptor) = source.location() else {
        return Err(Error::Invalid("source original not provided"));
    };
    let native = descriptor
        .to_native()
        .ok_or(Error::Invalid("foreign linked source path"))?;
    let path = if native.is_absolute() {
        native
    } else {
        std::path::absolute(board_path)?
            .parent()
            .ok_or(Error::Invalid("board directory"))?
            .join(native)
    };
    let file = File::open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        return Err(Error::Invalid("linked source is not a regular file"));
    }
    if source
        .fingerprint()
        .is_some_and(|expected| fingerprint(&metadata).is_none_or(|actual| actual != expected))
    {
        return Err(Error::Invalid(
            "linked source changed; relink before publishing",
        ));
    }
    let len = metadata.len();
    Ok(Payload::Stored {
        file: Arc::new(file),
        range: tack_storage::BlobRange {
            offset: 0,
            len,
            crc32: 0,
        },
    })
}
fn fingerprint(metadata: &std::fs::Metadata) -> Option<tack_core::SourceFingerprint> {
    let modified = metadata.modified().ok()?;
    let (modified_seconds, modified_nanos) = match modified.duration_since(std::time::UNIX_EPOCH) {
        Ok(duration) => (
            i64::try_from(duration.as_secs()).ok()?,
            duration.subsec_nanos(),
        ),
        Err(error) => {
            let duration = error.duration();
            let seconds = i64::try_from(duration.as_secs()).ok()?;
            if duration.subsec_nanos() == 0 {
                (-seconds, 0)
            } else {
                (
                    seconds.checked_neg()?.checked_sub(1)?,
                    1_000_000_000 - duration.subsec_nanos(),
                )
            }
        }
    };
    Some(tack_core::SourceFingerprint {
        size: metadata.len(),
        modified_seconds,
        modified_nanos,
    })
}
struct CrcReader<R> {
    inner: R,
    crc: crc32fast::Hasher,
}
impl<R: Read> Read for CrcReader<R> {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        let count = self.inner.read(bytes)?;
        self.crc.update(&bytes[..count]);
        Ok(count)
    }
}
pub fn prepare_source(
    source: &Source,
    payload: Payload,
    cancel: &AtomicBool,
) -> Result<PreparedSource> {
    let expected = payload.len()?;
    if expected == 0 || expected > crate::MAX_ASSET_BYTES {
        return Err(Error::Invalid("source encoded length admission budget"));
    }
    let observed = match &payload {
        Payload::Stored { file, .. } => fingerprint(&file.metadata()?),
        Payload::File(path) => fingerprint(&std::fs::metadata(path)?),
    };
    let mut reader = CrcReader {
        inner: CancelReader {
            inner: payload_reader(&payload)?,
            cancel,
        },
        crc: crc32fast::Hasher::new(),
    };
    let (hash, size) = hash_reader(&mut reader)?;
    if size != expected || size != payload.len()? {
        return Err(Error::Invalid("source length changed while hashing"));
    }
    let after = match &payload {
        Payload::Stored { file, .. } => fingerprint(&file.metadata()?),
        Payload::File(path) => fingerprint(&std::fs::metadata(path)?),
    };
    // Embedded payloads can be ranges in a concurrently appended import spool;
    // their own CRC validates the range independently of whole-file metadata.
    if matches!(source.location(), SourceLocation::Linked(_))
        && (observed.is_none() || observed != after)
    {
        return Err(Error::Invalid("source changed while hashing"));
    }
    if matches!(source.location(), SourceLocation::Embedded)
        && let Payload::Stored { range, .. } = &payload
        && reader.crc.finalize() != range.crc32
    {
        return Err(Error::Invalid("embedded original checksum mismatch"));
    }
    Ok(PreparedSource {
        binding: SourceBinding {
            source: WireId::new(source.id().value())?,
            revision: source.revision(),
            hash,
            size,
        },
        payload,
    })
}
pub fn prepare_sources(
    document: &Document,
    board_path: &Path,
    board: Option<&TackFile>,
    originals: &[(SourceId, u64, Payload)],
    cancel: &AtomicBool,
) -> Result<Vec<PreparedSource>> {
    let mut result = Vec::new();
    for source in document.sources() {
        check_cancel(cancel)?;
        let payload = originals
            .iter()
            .find(|(id, revision, _)| *id == source.id() && *revision == source.revision())
            .map(|(_, _, p)| p.clone())
            .or_else(|| {
                board.and_then(|board| {
                    board
                        .originals
                        .get(&source.id())
                        .filter(|e| e.revision == source.revision())
                        .map(|e| board.payload(e.range))
                })
            });
        let payload = match payload {
            Some(p) => p,
            None => linked_payload(source, board_path)?,
        };
        result.push(prepare_source(source, payload, cancel)?);
    }
    Ok(result)
}
/// Stop-and-wait 64 KiB chunks. Server validates content before Ready; no full original buffer.
pub fn upload_source(
    stream: &mut impl MessageIo,
    source: &PreparedSource,
    cancel: &AtomicBool,
) -> Result<bool> {
    check_cancel(cancel)?;
    let binding = &source.binding;
    send(
        stream,
        &Message::AssetBegin {
            hash: binding.hash.clone(),
            size: binding.size,
        },
    )?;
    match receive(stream)? {
        Message::AssetStatus {
            hash,
            size,
            present,
        } if hash == binding.hash && size == binding.size => {
            if present {
                return Ok(false);
            }
        }
        Message::Refused { reason, .. } => return Err(Error::Domain(reason)),
        _ => return Err(Error::Invalid("asset begin response")),
    }
    let mut input = payload_reader(&source.payload)?;
    let mut buffer = [0u8; crate::MAX_CHUNK_BYTES];
    let mut offset = 0u64;
    loop {
        check_cancel(cancel)?;
        let count = input.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        let next = offset
            .checked_add(count as u64)
            .ok_or(Error::Invalid("upload offset overflow"))?;
        if next > binding.size {
            return Err(Error::Invalid("source changed during upload"));
        }
        send(
            stream,
            &Message::AssetChunk {
                hash: binding.hash.clone(),
                offset,
                bytes: encode_hex(&buffer[..count]),
            },
        )?;
        match receive(stream)? {
            Message::AssetProgress {
                hash,
                offset: accepted,
            } if hash == binding.hash && accepted == next => {}
            Message::Refused { reason, .. } => return Err(Error::Domain(reason)),
            _ => return Err(Error::Invalid("asset chunk acknowledgement")),
        }
        offset = next;
    }
    if offset != binding.size {
        return Err(Error::Invalid("source truncated during upload"));
    }
    check_cancel(cancel)?;
    send(
        stream,
        &Message::AssetCommit {
            hash: binding.hash.clone(),
        },
    )?;
    match receive(stream)? {
        Message::AssetReady { hash, size } if hash == binding.hash && size == binding.size => {
            Ok(true)
        }
        Message::Refused { reason, .. } => Err(Error::Domain(reason)),
        _ => Err(Error::Invalid("asset commit response")),
    }
}
/// Explicit command-line/local-worker publish. Stable board/object/source IDs are preserved.
pub fn publish_board(
    address: &str,
    input: &Path,
    client: WireId,
    cancel: &AtomicBool,
) -> Result<Message> {
    let board = TackFile::open(input)?;
    let prepared = prepare_sources(&board.document, input, Some(&board), &[], cancel)?;
    let mut stream = connect(address, true)?;
    for source in &prepared {
        upload_source(&mut stream, source, cancel)?;
    }
    let bindings = prepared.iter().map(|s| s.binding.clone()).collect();
    let document = DocumentRecord::from_document(&board.document)?;
    check_cancel(cancel)?;
    write_message(
        &mut stream,
        &Message::Publish {
            board: WireId::new(board.document.id().value())?,
            client,
            document,
            sources: bindings,
        },
    )?;
    match read_message(&mut stream)? {
        snapshot @ Message::Snapshot { .. } => Ok(snapshot),
        Message::Refused { reason, .. } => Err(Error::Domain(reason)),
        _ => Err(Error::Invalid("publish response")),
    }
}
/// Architectural reverse path: embed streamed CAS payloads into a new ordinary local board.
/// Caller supplies pinned payloads, deliberately avoiding eager mirroring of all originals.
pub fn save_local_snapshot(
    path: &Path,
    document: &Document,
    sources: &[(SourceBinding, Payload)],
) -> Result<()> {
    if path.exists() {
        return Err(Error::Invalid("snapshot destination already exists"));
    }
    let (canonical, inputs) = snapshot_inputs(document, sources)?;
    let owner = tack_storage::BoardLease::acquire_new(path)?;
    owner.save(&canonical, inputs)?;
    Ok(())
}
/// Checked streaming snapshot inputs, also used for an owned host's offline checkpoint.
pub fn snapshot_inputs(
    document: &Document,
    sources: &[(SourceBinding, Payload)],
) -> Result<(Document, Vec<BlobInput>)> {
    let canonical = crate::shared_document(document)?;
    let mut inputs = Vec::new();
    for source in canonical.sources() {
        let (binding, payload) = sources
            .iter()
            .find(|(b, _)| {
                b.source.value() == source.id().value() && b.revision == source.revision()
            })
            .ok_or(Error::Invalid("snapshot original unavailable"))?;
        let pinned = match payload {
            Payload::File(path) => {
                let file = File::open(path)?;
                if !file.metadata()?.is_file() {
                    return Err(Error::Invalid("snapshot original is not a regular file"));
                }
                let len = file.metadata()?.len();
                Payload::Stored {
                    file: Arc::new(file),
                    range: tack_storage::BlobRange {
                        offset: 0,
                        len,
                        crc32: 0,
                    },
                }
            }
            other => other.clone(),
        };
        let mut reader = CrcReader {
            inner: payload_reader(&pinned)?,
            crc: crc32fast::Hasher::new(),
        };
        let (hash, size) = hash_reader(&mut reader)?;
        if hash != binding.hash || size != binding.size {
            return Err(Error::Invalid("snapshot original hash mismatch"));
        }
        let crc = reader.crc.finalize();
        let Payload::Stored { file, mut range } = pinned else {
            return Err(Error::Invalid("snapshot original pin"));
        };
        range.crc32 = crc;
        inputs.push(BlobInput::original(
            source.id(),
            source.revision(),
            Payload::Stored { file, range },
        ));
    }
    Ok((canonical, inputs))
}
pub(crate) fn changed_sources(command: &tack_core::Command, output: &mut Vec<Source>) {
    match command {
        tack_core::Command::AddSource(s) | tack_core::Command::SetSource(s) => {
            output.push(s.clone())
        }
        tack_core::Command::Batch(commands) => {
            for c in commands {
                changed_sources(c, output);
            }
        }
        _ => {}
    }
}
pub(crate) fn prepare_command(
    command: &tack_core::Command,
    originals: &[(SourceId, u64, Payload)],
    path: &Path,
    cancel: &AtomicBool,
) -> Result<(CommandDto, Vec<PreparedSource>)> {
    let mut sources = Vec::new();
    changed_sources(command, &mut sources);
    let mut prepared = Vec::new();
    for source in sources {
        let payload = originals
            .iter()
            .find(|(id, rev, _)| *id == source.id() && *rev == source.revision())
            .map(|(_, _, p)| p.clone());
        let payload = match payload {
            Some(p) => p,
            None => linked_payload(&source, path)?,
        };
        prepared.push(prepare_source(&source, payload, cancel)?);
    }
    Ok((CommandDto::from_command(command)?, prepared))
}
/// Stream one validated bounded original range to an explicit worker-owned sink.
pub fn download_chunk(
    stream: &mut impl MessageIo,
    binding: &SourceBinding,
    offset: u64,
    output: &mut impl Write,
) -> Result<usize> {
    let length = binding
        .size
        .saturating_sub(offset)
        .min(crate::MAX_CHUNK_BYTES as u64) as u32;
    send(
        stream,
        &Message::AssetGet {
            hash: binding.hash.clone(),
            offset,
            length,
        },
    )?;
    match receive(stream)? {
        Message::AssetData {
            hash,
            offset: received,
            size,
            bytes,
        } if hash == binding.hash && received == offset && size == binding.size => {
            let data = decode_hex(&bytes, crate::MAX_CHUNK_BYTES)?;
            if data.is_empty() || data.len() != length as usize {
                return Err(Error::Invalid("asset download chunk length"));
            }
            output.write_all(&data)?;
            Ok(data.len())
        }
        Message::Refused { reason, .. } => Err(Error::Domain(reason)),
        _ => Err(Error::Invalid("asset download response")),
    }
}

#[cfg(test)]
mod tests;
