//! Bounded regional products and optional worker-owned persistent raw reuse.
use super::*;
use crate::{huge_image::Tile, tile_disk::TileDisk};
use sha2::{Digest, Sha256};

pub(super) struct Persistent {
    root: Option<PathBuf>,
    budget: usize,
    attempted: bool,
    disk: Option<TileDisk>,
}
impl Persistent {
    pub(super) fn new(root: Option<PathBuf>, budget: usize) -> Self {
        Self {
            root,
            budget,
            attempted: false,
            disk: None,
        }
    }
    fn get(&mut self, key: [u8; 80]) -> Option<Decoded> {
        if !self.attempted {
            self.attempted = true;
            self.disk = self
                .root
                .as_ref()
                .and_then(|root| TileDisk::open(root, self.budget).ok());
        }
        match self.disk.as_mut()?.get(key) {
            Ok(image) => image,
            Err(_) => {
                self.disk = None;
                None
            }
        }
    }
    fn put(&mut self, key: [u8; 80], image: &Decoded) -> bool {
        match self.disk.as_mut().map(|disk| disk.put(key, image)) {
            Some(Ok(written)) => written,
            Some(Err(_)) => {
                self.disk = None;
                false
            }
            None => false,
        }
    }
    fn used(&self) -> usize {
        self.disk.as_ref().map_or(0, TileDisk::used_bytes)
    }
}

/// Greedily join a contiguous band/rectangle. At most fifteen admitted keys,
/// no off-screen ring, no extra demand and no source access on the main thread.
pub(super) fn gather(first: Job, jpeg: bool, queue: &mut Vec<Job>) -> Vec<Job> {
    let mut jobs = vec![first];
    if !jpeg || Tile::from_tag(jobs[0].edge).is_none() {
        return jobs;
    }
    let references: Vec<_> = std::iter::once(&jobs[0]).chain(queue.iter()).collect();
    let available: Vec<_> = (0..references.len()).collect();
    let best = largest_region(&references, &available);
    // Remove backwards to keep queue indices stable; publication is keyed.
    for index in best.into_iter().filter(|index| *index != 0).rev() {
        let mut next = queue.remove(index - 1);
        next.cancel = jobs[0].cancel.clone();
        jobs.push(next);
    }
    jobs
}

/// One complete rectangle of currently missing tiles, or a bounded single
/// fallback. The same admission serves scheduling and cache-hole refinement.
fn largest_region(jobs: &[&Job], available: &[usize]) -> Vec<usize> {
    let first = available[0];
    let Some(origin) = Tile::from_tag(jobs[first].edge) else {
        return vec![first];
    };
    let mut best = vec![first];
    for candidate in available {
        let Some(end) = Tile::from_tag(jobs[*candidate].edge) else {
            continue;
        };
        if origin.mip != end.mip {
            continue;
        }
        let low = [origin.x.min(end.x), origin.y.min(end.y)];
        let high = [origin.x.max(end.x), origin.y.max(end.y)];
        let selected: Vec<_> = available
            .iter()
            .copied()
            .filter(|index| {
                let job = jobs[*index];
                let Some(tile) = Tile::from_tag(job.edge) else {
                    return false;
                };
                job.source.id() == jobs[first].source.id()
                    && job.source.revision() == jobs[first].source.revision()
                    && job.asset.pixel_size() == jobs[first].asset.pixel_size()
                    && job.lod == jobs[first].lod
                    && tile.mip == origin.mip
                    && (low[0]..=high[0]).contains(&tile.x)
                    && (low[1]..=high[1]).contains(&tile.y)
            })
            .collect();
        if selected.len() <= best.len() {
            continue;
        }
        let edges: Vec<_> = selected.iter().map(|index| jobs[*index].edge).collect();
        if crate::jpeg_scanlines::admissible_tiles(jobs[first].asset.pixel_size(), &edges) {
            best = selected;
        }
    }
    best
}

pub(super) fn load_batch(
    batch: WorkBatch,
    base: &Path,
    dir: &Path,
    disk: &Mutex<crate::decode::DiskCache>,
    persistent: &Mutex<Persistent>,
) -> Vec<Outcome> {
    let tiled = Tile::from_tag(batch.jobs[0].edge).is_some();
    if !tiled {
        return batch
            .jobs
            .into_iter()
            .map(|job| load(job, base, dir, disk))
            .collect();
    }
    // Preserve the existing per-open route when no persistent quota is enabled.
    let enabled = persistent.lock().is_ok_and(|cache| cache.root.is_some());
    if !enabled && batch.jobs.len() == 1 {
        let Some(job) = batch.jobs.into_iter().next() else {
            return Vec::new();
        };
        let mut result = load(job, base, dir, disk);
        result.region_jobs = usize::from(!result.repair_cache_hit && result.result.is_ok());
        return vec![result];
    }
    let mut outcomes: Vec<_> = batch.jobs.iter().map(empty_outcome).collect();
    let result = supply(&batch, base, persistent, &mut outcomes);
    if let Err(error) = result {
        for outcome in &mut outcomes {
            outcome.result = Err(error.to_string().into());
        }
    }
    let cancelled = batch.jobs[0]
        .cancel
        .as_ref()
        .is_some_and(|flag| flag.load(Ordering::Relaxed));
    for outcome in &mut outcomes {
        outcome.cancelled = cancelled;
    }
    outcomes
}

fn source_path(job: &Job, base: &Path) -> Result<Option<PathBuf>, AssetError> {
    if let Some(path) = &job.shared_path {
        return Ok(Some(path.clone()));
    }
    match job.source.location() {
        SourceLocation::Embedded => Ok(None),
        SourceLocation::Linked(path) => {
            let native = path.to_native().ok_or("foreign tile source path")?;
            Ok(Some(if path.is_absolute() {
                native
            } else {
                base.join(native)
            }))
        }
    }
}

fn source_identity(job: &Job, path: Option<&Path>) -> Result<([u8; 20], u32), AssetError> {
    let mut fingerprint = [0; 20];
    if let Some(path) = path {
        let actual = representation::source_fingerprint(path)?;
        if job.shared_path.is_none() && job.source.fingerprint().is_some_and(|old| old != actual) {
            return Err("tile source changed; explicit revision required".into());
        }
        fingerprint[..8].copy_from_slice(&actual.size.to_le_bytes());
        fingerprint[8..16].copy_from_slice(&actual.modified_seconds.to_le_bytes());
        fingerprint[16..20].copy_from_slice(&actual.modified_nanos.to_le_bytes());
        return Ok((fingerprint, 0));
    }
    let range = match &job.original {
        Some(Payload::Stored { range, .. }) => *range,
        Some(Payload::File(_)) => return Err("embedded tile original is not pinned".into()),
        None => {
            job.board
                .originals
                .get(&job.source.id())
                .ok_or("missing original")?
                .range
        }
    };
    fingerprint[..8].copy_from_slice(&range.len.to_le_bytes());
    Ok((fingerprint, range.crc32))
}

/// Relative links in two copies of the same document may name different
/// originals with identical stat fingerprints. Scope the key to the resolved
/// physical path as well as document identity; no source-content preload.
fn cache_namespace(job: &Job, path: Option<&Path>) -> Result<[u8; 16], AssetError> {
    let mut hash = Sha256::new();
    hash.update(job.board.document.id().value().to_le_bytes());
    if let Some(path) = path {
        hash.update(path.canonicalize()?.as_os_str().as_encoded_bytes());
    }
    let digest = hash.finalize();
    let mut namespace = [0; 16];
    namespace.copy_from_slice(&digest[..16]);
    Ok(namespace)
}

fn cache_key(job: &Job, identity: ([u8; 20], u32), jpeg: bool, namespace: [u8; 16]) -> [u8; 80] {
    let mut key = [0; 80];
    key[..16].copy_from_slice(&namespace);
    key[16..32].copy_from_slice(&job.source.id().value().to_le_bytes());
    key[32..40].copy_from_slice(&job.source.revision().to_le_bytes());
    key[40..60].copy_from_slice(&identity.0);
    key[60..64].copy_from_slice(&identity.1.to_le_bytes());
    key[64..68].copy_from_slice(&job.asset.pixel_size()[0].to_le_bytes());
    key[68..72].copy_from_slice(&job.asset.pixel_size()[1].to_le_bytes());
    key[72..76].copy_from_slice(&job.edge.to_le_bytes());
    key[76..80].copy_from_slice(&(if jpeg { 5u32 } else { 3u32 }).to_le_bytes());
    key
}

fn supply(
    batch: &WorkBatch,
    base: &Path,
    persistent: &Mutex<Persistent>,
    outcomes: &mut [Outcome],
) -> Result<(), AssetError> {
    let first = &batch.jobs[0];
    let path = source_path(first, base)?;
    // Authority is checked before derived reuse, including the current actual
    // linked fingerprint when the document has no recorded fingerprint.
    let identity = source_identity(first, path.as_deref())?;
    let namespace = cache_namespace(first, path.as_deref())?;
    let keys: Vec<_> = batch
        .jobs
        .iter()
        .map(|job| cache_key(job, identity, batch.jpeg, namespace))
        .collect();
    let mut missing = Vec::new();
    for (index, outcome) in outcomes.iter_mut().enumerate() {
        outcome.state = if path.is_some() {
            SourceState::Available
        } else {
            SourceState::Embedded
        };
        outcome.streamed_jpeg = batch.jpeg;
        outcome.streamed_png = !batch.jpeg;
        let hit = persistent
            .lock()
            .ok()
            .and_then(|mut cache| cache.get(keys[index]));
        let expected = Tile::from_tag(batch.jobs[index].edge)
            .ok_or("invalid tile address")?
            .dimensions(first.asset.pixel_size())?
            .map(|n| n + u32::from(batch.jpeg) * 2);
        if let Some(image) = hit.filter(|image| [image.width, image.height] == expected) {
            outcome.tile_cache_hit = true;
            outcome.tile_cache_read_bytes = image.rgba.len() as u64;
            outcome.result = Ok(image);
        } else {
            missing.push(index);
        }
    }
    // Cached holes are excluded: derive only contiguous newly exposed bands.
    while !missing.is_empty() {
        let references: Vec<_> = batch.jobs.iter().collect();
        let region = if batch.jpeg {
            largest_region(&references, &missing)
        } else {
            vec![missing[0]]
        };
        missing.retain(|index| !region.contains(index));
        decode_region(batch, path.as_deref(), &region, outcomes)?;
        // A linked source changed during decode (or during hot disk reads) must
        // never acquire a current identity or publish its pixels.
        if source_identity(first, path.as_deref())? != identity
            || cache_namespace(first, path.as_deref())? != namespace
        {
            return Err("tile source changed during derivation".into());
        }
        for index in region {
            if let Ok(image) = &outcomes[index].result {
                let written = persistent
                    .lock()
                    .is_ok_and(|mut cache| cache.put(keys[index], image));
                if written {
                    outcomes[index].tile_cache_write_bytes = image.rgba.len() as u64;
                }
            }
        }
    }
    if source_identity(first, path.as_deref())? != identity
        || cache_namespace(first, path.as_deref())? != namespace
    {
        return Err("tile source changed during cache reuse".into());
    }
    let used = persistent.lock().map_or(0, |cache| cache.used());
    for outcome in outcomes {
        outcome.tile_disk_bytes = used;
    }
    Ok(())
}

fn decode_region(
    batch: &WorkBatch,
    path: Option<&Path>,
    indices: &[usize],
    outcomes: &mut [Outcome],
) -> Result<(), AssetError> {
    let first = &batch.jobs[0];
    let edges: Vec<_> = indices.iter().map(|i| batch.jobs[*i].edge).collect();
    let leader = indices[0];
    outcomes[leader].region_jobs += 1;
    let pixels = if let Some(path) = path {
        representation::derive_linked_tiles_cancel(
            path,
            &mut outcomes[leader].source_bytes,
            &edges,
            first.asset.pixel_size(),
            first.cancel.as_deref(),
        )?
    } else {
        let inner = match &first.original {
            Some(Payload::Stored { file, range }) => RangeReader::new(Arc::clone(file), *range),
            Some(Payload::File(_)) => return Err("embedded original is not pinned".into()),
            None => first.board.original_reader(first.source.id())?,
        };
        let mut reader = CountRead { inner, count: 0 };
        let result = representation::derive_stream_tiles_cancel(
            &mut reader,
            &edges,
            first.asset.pixel_size(),
            first.cancel.as_deref(),
        );
        outcomes[leader].container_bytes += reader.count;
        result?
    };
    for (&index, pixels) in indices.iter().zip(pixels) {
        let expected = Tile::from_tag(batch.jobs[index].edge)
            .ok_or("invalid tile address")?
            .dimensions(first.asset.pixel_size())?
            .map(|n| n + u32::from(batch.jpeg) * 2);
        if [pixels.width(), pixels.height()] != expected {
            return Err("regional pixels disagree with declared source dimensions".into());
        }
        outcomes[index].result = Ok(Decoded {
            width: pixels.width(),
            height: pixels.height(),
            rgba: pixels.into_raw(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests;
