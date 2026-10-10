use crate::{
    codec::{self, Decoder},
    *,
};
use std::{
    collections::BTreeMap,
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
    sync::Arc,
};
use tack_core::{AssetId, Document, SourceId, SourceLocation};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlobRange {
    pub offset: u64,
    pub len: u64,
    pub crc32: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OverviewEntry {
    pub id: u128,
    pub revision: u64,
    pub role: u16,
    pub encoding: u16,
    pub generator: u32,
    pub width: u32,
    pub height: u32,
    pub range: BlobRange,
}
/// Resident metadata + stable open file handle; no preview/original payload preload.
pub struct TackFile {
    pub document: Document,
    pub originals: BTreeMap<SourceId, OverviewEntry>,
    pub overviews: BTreeMap<AssetId, OverviewEntry>,
    pub discarded_overviews: usize,
    pub metadata_bytes_read: u64,
    pub(crate) file: Arc<File>,
}
fn read_exact(file: &Arc<File>, offset: u64, len: usize) -> Result<Vec<u8>> {
    let mut r = RangeReader::new(
        Arc::clone(file),
        BlobRange {
            offset,
            len: len as u64,
            crc32: 0,
        },
    );
    let mut bytes = vec![0; len];
    r.read_exact(&mut bytes)?;
    Ok(bytes)
}
fn range_valid(r: BlobRange, start: u64, end: u64) -> bool {
    r.len > 0 && r.offset >= start && r.offset.checked_add(r.len).is_some_and(|e| e <= end)
}
impl TackFile {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let file = Arc::new(File::open(path)?);
        let len = file.metadata()?.len();
        if !(HEADER_BYTES as u64..=MAX_FILE_BYTES).contains(&len) {
            return Err(StorageError::Invalid("file length"));
        }
        let header = read_exact(&file, 0, HEADER_BYTES)?;
        let mut h = Decoder::new(&header);
        if h.take(8)? != b"TACKSN01" {
            return Err(StorageError::Invalid("magic"));
        }
        let container = h.u32()?;
        let schema = h.u32()?;
        if container != 1 || ![1, 2, 3, 4, 5, 6, 7].contains(&schema) {
            return Err(StorageError::Unsupported("container/schema version"));
        }
        let auth_len = h.u64()?;
        let derived_len = h.u64()?;
        let recorded_len = h.u64()?;
        let auth_crc = h.u32()?;
        let derived_crc = h.u32()?;
        let mut counts = [0usize; 5];
        for c in &mut counts {
            *c = h.u32()? as usize;
            if *c > MAX_RECORDS {
                return Err(StorageError::Invalid("record count"));
            }
        }
        if h.take(12)?.iter().any(|b| *b != 0) {
            return Err(StorageError::Unsupported("required header capabilities"));
        }
        if auth_len > MAX_METADATA_BYTES as u64
            || derived_len != (counts[4] * 64) as u64
            || auth_len < 16
            || (recorded_len != len)
        {
            return Err(StorageError::Invalid("metadata/file length"));
        }
        let payload_start = (HEADER_BYTES as u64)
            .checked_add(auth_len)
            .and_then(|n| n.checked_add(derived_len))
            .filter(|n| *n <= len)
            .ok_or(StorageError::Invalid("metadata range"))?;
        let bytes = read_exact(&file, HEADER_BYTES as u64, auth_len as usize)?;
        if crc32fast::hash(&bytes) != auth_crc {
            return Err(StorageError::Corrupt("authoritative metadata checksum"));
        }
        let mut d = Decoder::new(&bytes);
        let document = codec::decode_document(&mut d, [counts[0], counts[1], counts[2]], schema)?;
        let mut originals = BTreeMap::new();
        let mut intervals = Vec::with_capacity(counts[3] + counts[4]);
        for _ in 0..counts[3] {
            let e = codec::decode_entry(&mut d, 1)?;
            let id = SourceId::new(e.id).map_err(|_| StorageError::Invalid("source ID"))?;
            let source = document
                .source(id)
                .ok_or(StorageError::Invalid("original source reference"))?;
            if !matches!(source.location(), SourceLocation::Embedded)
                || source.revision() != e.revision
                || !range_valid(e.range, payload_start, len)
                || e.role != 0
                || e.encoding != 0
                || e.generator != 0
                || e.width != 0
                || e.height != 0
                || originals.insert(id, e).is_some()
            {
                return Err(StorageError::Invalid("original directory"));
            }
            intervals.push((e.range.offset, e.range.offset + e.range.len));
        }
        d.finish()?;
        if document
            .sources()
            .filter(|s| matches!(s.location(), SourceLocation::Embedded))
            .count()
            != originals.len()
        {
            return Err(StorageError::Invalid("missing embedded original"));
        }
        intervals.sort_unstable();
        if intervals.windows(2).any(|w| w[0].1 > w[1].0) {
            return Err(StorageError::Invalid("overlapping originals"));
        }
        let derived = read_exact(&file, HEADER_BYTES as u64 + auth_len, derived_len as usize)?;
        let mut overviews = BTreeMap::new();
        let mut discarded = 0;
        if crc32fast::hash(&derived) == derived_crc {
            let mut d = Decoder::new(&derived);
            // Each preview directory record is independently disposable. Sorting prevents quadratic overlap checks.
            let mut entries = Vec::with_capacity(counts[4]);
            for _ in 0..counts[4] {
                let slice = d.take(64)?;
                match codec::decode_entry(&mut Decoder::new(slice), 2) {
                    Ok(e) => entries.push(e),
                    Err(_) => discarded += 1,
                }
            }
            entries.sort_unstable_by_key(|e| e.range.offset);
            let mut previous_end = payload_start;
            for e in entries {
                let Ok(id) = AssetId::new(e.id) else {
                    discarded += 1;
                    continue;
                };
                let valid_source = document
                    .asset(id)
                    .and_then(|a| document.source(a.source_id()))
                    .is_some_and(|s| s.revision() == e.revision);
                let orig_overlap = intervals.partition_point(|r| r.1 <= e.range.offset);
                let overlaps = intervals
                    .get(orig_overlap)
                    .is_some_and(|r| r.0 < e.range.offset.saturating_add(e.range.len));
                if !range_valid(e.range, payload_start, len)
                    || e.range.offset < previous_end
                    || overlaps
                    || e.range.len > MAX_OVERVIEW_BYTES
                    || !valid_source
                    || e.role != 1
                    || e.encoding != 1
                    || ![1, 2].contains(&e.generator)
                    || e.width == 0
                    || e.height == 0
                    || e.width > 512
                    || e.height > 512
                    || overviews.contains_key(&id)
                {
                    discarded += 1;
                    continue;
                }
                previous_end = e.range.offset + e.range.len;
                overviews.insert(id, e);
            }
        } else {
            discarded = counts[4];
        }
        Ok(Self {
            document,
            originals,
            overviews,
            discarded_overviews: discarded,
            metadata_bytes_read: payload_start,
            file,
        })
    }
    /// Explicit restore/export validation on a worker, with a fixed streaming buffer.
    pub fn verify_originals(&self) -> Result<()> {
        let mut buffer = [0; 128 * 1024];
        for (id, entry) in &self.originals {
            let mut reader = self.original_reader(*id)?;
            let mut crc = crc32fast::Hasher::new();
            let mut count = 0;
            loop {
                let n = reader.read(&mut buffer)?;
                if n == 0 {
                    break;
                }
                crc.update(&buffer[..n]);
                count += n as u64;
            }
            if count != entry.range.len || crc.finalize() != entry.range.crc32 {
                return Err(StorageError::Corrupt("recovery original checksum"));
            }
        }
        Ok(())
    }
    pub fn original_reader(&self, id: SourceId) -> Result<RangeReader> {
        let e = self
            .originals
            .get(&id)
            .ok_or(StorageError::Invalid("missing original"))?;
        Ok(RangeReader::new(Arc::clone(&self.file), e.range))
    }
    pub fn overview_bytes(&self, id: AssetId) -> Result<Vec<u8>> {
        let e = self
            .overviews
            .get(&id)
            .ok_or(StorageError::Invalid("missing overview"))?;
        let bytes = read_exact(&self.file, e.range.offset, e.range.len as usize)?;
        if crc32fast::hash(&bytes) != e.range.crc32 {
            return Err(StorageError::Corrupt("overview checksum"));
        }
        Ok(bytes)
    }
    pub fn payload(&self, range: BlobRange) -> Payload {
        Payload::Stored {
            file: Arc::clone(&self.file),
            range,
        }
    }
}
/// Independent cursor over an immutable opened generation. Position-based OS reads
/// avoid File::try_clone's shared seek cursor and save-replace/path races.
pub struct RangeReader {
    file: Arc<File>,
    range: BlobRange,
    pos: u64,
    pub bytes_read: u64,
}
impl RangeReader {
    pub fn new(file: Arc<File>, range: BlobRange) -> Self {
        Self {
            file,
            range,
            pos: 0,
            bytes_read: 0,
        }
    }
}
impl Read for RangeReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let size = buf
            .len()
            .min(usize::try_from(self.range.len.saturating_sub(self.pos)).unwrap_or(usize::MAX));
        if size == 0 {
            return Ok(0);
        }
        let offset = self
            .range
            .offset
            .checked_add(self.pos)
            .ok_or_else(|| std::io::Error::other("payload offset overflow"))?;
        #[cfg(unix)]
        let n = {
            use std::os::unix::fs::FileExt;
            self.file.read_at(&mut buf[..size], offset)?
        };
        #[cfg(windows)]
        let n = {
            use std::os::windows::fs::FileExt;
            self.file.seek_read(&mut buf[..size], offset)?
        };
        #[cfg(not(any(unix, windows)))]
        let n = {
            return Err(std::io::Error::other(
                "position-based file reads unsupported",
            ));
        };
        self.pos += n as u64;
        self.bytes_read += n as u64;
        Ok(n)
    }
}
impl Seek for RangeReader {
    fn seek(&mut self, from: SeekFrom) -> std::io::Result<u64> {
        let pos = match from {
            SeekFrom::Start(n) => i128::from(n),
            SeekFrom::Current(n) => i128::from(self.pos) + i128::from(n),
            SeekFrom::End(n) => i128::from(self.range.len) + i128::from(n),
        };
        if pos < 0 || pos > i128::from(self.range.len) {
            return Err(std::io::Error::other("seek outside payload"));
        }
        self.pos = pos as u64;
        Ok(self.pos)
    }
}
