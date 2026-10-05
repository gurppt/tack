//! Bounded, replaceable local display supply. All storage/decode stays on workers.
use crate::{
    AssetError, Decoded,
    representation::{self, CountRead},
};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs::File,
    io::Write,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver, SyncSender},
    },
    thread,
    time::Instant,
};
use tack_core::{AssetId, ByteCache, Document, ImageAsset, Lod, Source, SourceId, SourceLocation};
use tack_storage::{Payload, RangeReader, TackFile};
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct Key {
    asset: Option<AssetId>,
    source: SourceId,
    revision: u64,
    lod: Lod,
    edge: u32,
}
#[derive(Clone)]
struct Job {
    board: Arc<TackFile>,
    asset: ImageAsset,
    source: Source,
    original: Option<Payload>,
    lod: Lod,
    edge: u32,
}
impl Job {
    fn key(&self) -> Key {
        Key {
            asset: (self.lod == Lod::Thumbnail).then_some(self.asset.id()),
            source: self.source.id(),
            revision: self.source.revision(),
            lod: self.lod,
            edge: self.edge,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceState {
    Embedded,
    Available,
    Missing,
    Changed,
    Foreign,
    Unavailable,
}
#[derive(Clone, Debug)]
pub struct PreparedOverview {
    pub asset: AssetId,
    pub revision: u64,
    pub size: [u32; 2],
    pub generator: u32,
    pub path: PathBuf,
}
/// Input is sorted by the caller: large visible, small visible, near, background.
#[derive(Clone, Copy, Debug)]
pub struct ProductDemand {
    pub asset: AssetId,
    pub lod: Lod,
    pub edge: u32,
    pub priority: u8,
    pub resident: bool,
}
#[derive(Clone, Copy, Debug)]
pub struct SupplyLimits {
    pub cpu_bytes: usize,
    pub disk_bytes: usize,
    pub workers: usize,
    pub requests: usize,
    pub max_lod: Lod,
}
impl Default for SupplyLimits {
    fn default() -> Self {
        Self {
            cpu_bytes: 64 * 1024 * 1024,
            disk_bytes: 512 * 1024 * 1024,
            workers: 2,
            requests: 16,
            max_lod: Lod::Detail,
        }
    }
}
impl SupplyLimits {
    pub fn potato() -> Self {
        Self {
            cpu_bytes: 8 * 1024 * 1024,
            disk_bytes: 8 * 1024 * 1024,
            workers: 1,
            requests: 4,
            max_lod: Lod::Medium,
        }
    }
    pub fn validate(self) -> Result<Self, AssetError> {
        if self.cpu_bytes == 0
            || self.cpu_bytes > 64 * 1024 * 1024
            || !(1024 * 1024..=512 * 1024 * 1024).contains(&self.disk_bytes)
            || !(1..=2).contains(&self.workers)
            || !(1..=16).contains(&self.requests)
        {
            return Err("invalid product supply limits".into());
        }
        Ok(self)
    }
}
#[derive(Clone, Copy, Default, Debug)]
pub struct ProductAssetStats {
    pub pending: usize,
    pub peak_pending: usize,
    pub queued: usize,
    pub peak_queued: usize,
    pub completed: usize,
    pub reused: usize,
    pub regenerated: usize,
    pub repair_cache_hits: usize,
    pub repair_cache_bytes: u64,
    pub errors: usize,
    pub discarded: usize,
    pub container_bytes: u64,
    pub source_bytes: u64,
    pub derived_bytes: u64,
    pub cpu_bytes: usize,
    pub cpu_peak: usize,
    pub evictions: u64,
    pub decode_ms: f64,
    pub decode_count: usize,
    pub reprioritized: usize,
}
struct Outcome {
    key: Key,
    asset: AssetId,
    source: Option<SourceId>,
    state: SourceState,
    result: Result<Decoded, AssetError>,
    prepared: Option<PreparedOverview>,
    container_bytes: u64,
    source_bytes: u64,
    derived_bytes: u64,
    repair_cache_bytes: u64,
    repair_cache_hit: bool,
    decode_ms: f64,
}
struct Worker {
    sender: SyncSender<Job>,
    receiver: Receiver<Outcome>,
    handle: thread::JoinHandle<()>,
}
pub struct ProductAssets {
    board: Arc<TackFile>,
    workers: Vec<Worker>,
    pending: HashMap<Key, usize>,
    failed: HashSet<Key>,
    cache: ByteCache<Key, Arc<Decoded>>,
    details: ByteCache<Key, Arc<Decoded>>,
    queue: Vec<Job>,
    wanted: HashSet<Key>,
    view_mode: bool,
    limits: SupplyLimits,
    pub states: BTreeMap<SourceId, SourceState>,
    pub prepared: BTreeMap<AssetId, PreparedOverview>,
    stats: ProductAssetStats,
    current: HashMap<AssetId, u64>,
    sources: HashMap<AssetId, SourceId>,
}
impl ProductAssets {
    pub fn new(
        board: Arc<TackFile>,
        board_path: &Path,
        repair_dir: PathBuf,
    ) -> Result<Self, AssetError> {
        Self::with_limits(board, board_path, repair_dir, SupplyLimits::default())
    }
    pub fn with_budgets(
        board: Arc<TackFile>,
        board_path: &Path,
        repair_dir: PathBuf,
        cpu_bytes: usize,
        disk_bytes: usize,
    ) -> Result<Self, AssetError> {
        Self::with_limits(
            board,
            board_path,
            repair_dir,
            SupplyLimits {
                cpu_bytes,
                disk_bytes,
                ..Default::default()
            },
        )
    }
    pub fn with_limits(
        board: Arc<TackFile>,
        board_path: &Path,
        repair_dir: PathBuf,
        limits: SupplyLimits,
    ) -> Result<Self, AssetError> {
        let limits = limits.validate()?;
        let disk = Arc::new(Mutex::new(crate::decode::DiskCache::new(
            repair_dir.clone(),
            limits.disk_bytes,
        )));
        let base = board_path.parent().unwrap_or(Path::new(".")).to_path_buf();
        let mut workers = Vec::new();
        for _ in 0..limits.workers {
            // One active job per worker, no FIFO backlog hidden behind a codec.
            let (tx, rx) = mpsc::sync_channel::<Job>(1);
            let (done_tx, done_rx) = mpsc::sync_channel(1);
            let base = base.clone();
            let dir = repair_dir.clone();
            let disk = Arc::clone(&disk);
            let handle = thread::Builder::new()
                .name("tack-product-display".into())
                .spawn(move || {
                    let _ = tack_storage::create_private_directory(&dir, true);
                    if let Ok(mut cache) = disk.lock() {
                        cache.prepare();
                    }
                    while let Ok(job) = rx.recv() {
                        let start = Instant::now();
                        let mut outcome = load(job, &base, &dir, &disk);
                        outcome.decode_ms = start.elapsed().as_secs_f64() * 1000.;
                        if done_tx.send(outcome).is_err() {
                            break;
                        }
                    }
                })?;
            workers.push(Worker {
                sender: tx,
                receiver: done_rx,
                handle,
            });
        }
        let overview = if limits.cpu_bytes < 1024 * 1024 {
            limits.cpu_bytes
        } else {
            limits.cpu_bytes / 4
        };
        Ok(Self {
            board,
            workers,
            pending: HashMap::new(),
            failed: HashSet::new(),
            cache: ByteCache::new(overview),
            details: ByteCache::new(limits.cpu_bytes - overview),
            queue: Vec::new(),
            wanted: HashSet::new(),
            view_mode: false,
            limits,
            states: BTreeMap::new(),
            prepared: BTreeMap::new(),
            stats: Default::default(),
            current: HashMap::new(),
            sources: HashMap::new(),
        })
    }
    pub fn limits(&self) -> SupplyLimits {
        self.limits
    }
    pub fn sync_document(&mut self, doc: &Document) {
        self.current.retain(|id, rev| {
            if let Some(s) = doc.asset(*id).and_then(|a| doc.source(a.source_id())) {
                if *rev != s.revision() {
                    self.states.remove(&s.id());
                }
                *rev = s.revision();
                self.sources.insert(*id, s.id());
                true
            } else {
                self.sources.remove(id);
                false
            }
        });
        self.failed.retain(|k| {
            doc.source(k.source)
                .is_some_and(|s| s.revision() == k.revision)
        });
        self.states.retain(|id, _| doc.source(*id).is_some());
        self.prepared
            .retain(|id, p| self.current.get(id) == Some(&p.revision));
    }
    pub fn set_board(&mut self, board: Arc<TackFile>) {
        self.board = board;
    }
    pub fn request(&mut self, id: AssetId) -> bool {
        let b = Arc::clone(&self.board);
        self.request_current(id, &b.document, None)
    }
    fn job(
        &mut self,
        id: AssetId,
        lod: Lod,
        edge: u32,
        doc: &Document,
        original: Option<&Payload>,
    ) -> Option<Job> {
        let asset = *doc.asset(id)?;
        let source = doc.source(asset.source_id())?.clone();
        self.observe_current(id, source.revision(), source.id());
        Some(Job {
            board: Arc::clone(&self.board),
            asset,
            source,
            original: original.cloned(),
            lod,
            edge,
        })
    }
    fn cached(&self, k: Key) -> bool {
        if k.lod == Lod::Thumbnail {
            self.cache.contains(k)
        } else {
            self.details.contains(k)
        }
    }
    pub fn request_current(
        &mut self,
        id: AssetId,
        doc: &Document,
        original: Option<&Payload>,
    ) -> bool {
        let Some(job) = self.job(id, Lod::Thumbnail, 128, doc, original) else {
            return false;
        };
        let k = job.key();
        if self.cached(k)
            || self.pending.contains_key(&k)
            || self.queue.iter().any(|j| j.key() == k)
            || self.failed.contains(&k)
            || self.pending.len() + self.queue.len() >= self.limits.requests
        {
            return false;
        }
        self.queue.push(job);
        self.dispatch();
        true
    }
    /// Replace only the bounded main-thread queue, never wait for a worker. Running
    /// codecs can finish; publication is gated by *current* demand and revision.
    pub fn replace_view(
        &mut self,
        demands: &[ProductDemand],
        doc: &Document,
        originals: &BTreeMap<(SourceId, u64), Payload>,
    ) {
        self.view_mode = true;
        self.wanted.clear();
        let previous: HashSet<_> = self.queue.iter().map(Job::key).collect();
        self.queue.clear();
        for d in demands {
            let Some(a) = doc.asset(d.asset) else {
                continue;
            };
            let Some(s) = doc.source(a.source_id()) else {
                continue;
            };
            let lod = d.lod.min(self.limits.max_lod);
            let edge = if lod == Lod::Thumbnail {
                d.edge.clamp(8, 128).next_power_of_two()
            } else {
                lod.edge()
            };
            let k = Key {
                asset: (lod == Lod::Thumbnail).then_some(d.asset),
                source: s.id(),
                revision: s.revision(),
                lod,
                edge,
            };
            self.wanted.insert(k);
            self.observe_current(d.asset, s.revision(), s.id());
            if self.cached(k)
                || self.pending.contains_key(&k)
                || self.failed.contains(&k)
                || d.resident
                || self.queue.len() + self.pending.len() >= self.limits.requests
                || self.queue.iter().any(|j| j.key() == k)
                // At most two queued representations per source leave admission
                // space for other visible sources, with one ready successor.
                || self.queue.iter().filter(|j| j.source.id() == k.source).count() >= 2
            {
                continue;
            }
            if let Some(job) = self.job(
                d.asset,
                lod,
                edge,
                doc,
                originals.get(&(s.id(), s.revision())),
            ) {
                self.queue.push(job);
            }
        }
        self.stats.reprioritized += previous.iter().filter(|k| !self.wanted.contains(k)).count();
        self.dispatch();
    }
    pub fn schedule(&mut self) {
        self.dispatch();
    }
    fn dispatch(&mut self) {
        for i in 0..self.workers.len() {
            if self.pending.values().any(|w| *w == i) {
                continue;
            }
            // Reserve worker zero for overviews when two workers are available.
            // Detail cannot block immediately useful previews; with one worker,
            // the caller's sorted, overview-first demand is used.
            let pos = self.queue.iter().position(|j| {
                (i != 0 || self.workers.len() == 1 || j.lod == Lod::Thumbnail)
                    && !self.pending.keys().any(|k| k.source == j.source.id())
            });
            if let Some(pos) = pos {
                let job = self.queue.remove(pos);
                let key = job.key();
                if self.workers[i].sender.try_send(job).is_ok() {
                    self.pending.insert(key, i);
                }
            }
        }
        self.update_stats();
    }
    fn update_stats(&mut self) {
        self.stats.pending = self.pending.len() + self.queue.len();
        self.stats.peak_pending = self.stats.peak_pending.max(self.stats.pending);
        self.stats.queued = self.queue.len();
        self.stats.peak_queued = self.stats.peak_queued.max(self.queue.len());
        self.stats.cpu_bytes = self.cache.used_bytes() + self.details.used_bytes();
        self.stats.cpu_peak = self.stats.cpu_peak.max(self.stats.cpu_bytes);
        self.stats.evictions = self.cache.evictions() + self.details.evictions();
    }
    pub fn poll(&mut self) -> bool {
        let mut outcomes = Vec::new();
        for w in &self.workers {
            while let Ok(o) = w.receiver.try_recv() {
                outcomes.push(o);
            }
        }
        let changed = !outcomes.is_empty();
        for o in outcomes {
            self.pending.remove(&o.key);
            self.stats.completed += 1;
            self.stats.decode_count += 1;
            self.stats.decode_ms += o.decode_ms;
            self.stats.container_bytes += o.container_bytes;
            self.stats.source_bytes += o.source_bytes;
            self.stats.derived_bytes += o.derived_bytes;
            self.stats.repair_cache_bytes += o.repair_cache_bytes;
            if self.current.get(&o.asset) != Some(&o.key.revision)
                || self.sources.get(&o.asset) != Some(&o.key.source)
                || (self.view_mode && !self.wanted.contains(&o.key))
            {
                self.stats.discarded += 1;
                continue;
            }
            if let Some(source) = o.source {
                self.states.insert(source, o.state);
            }
            if o.repair_cache_hit {
                self.stats.repair_cache_hits += 1;
            }
            if let Some(p) = o.prepared {
                self.stats.regenerated += usize::from(!o.repair_cache_hit);
                self.prepared.insert(o.asset, p);
            } else if o.result.is_ok() {
                self.stats.reused += 1;
            }
            match o.result {
                Ok(image) => {
                    let bytes = image.rgba.len();
                    let cache = if o.key.lod == Lod::Thumbnail {
                        &mut self.cache
                    } else {
                        &mut self.details
                    };
                    if !cache.insert(o.key, Arc::new(image), bytes) {
                        self.failed.insert(o.key);
                    }
                }
                Err(_) => {
                    self.stats.errors += 1;
                    self.failed.insert(o.key);
                }
            }
        }
        // The native caller replaces view demand before dispatching more jobs;
        // CLI callers retain their admitted finite preparation queue.
        if !self.view_mode {
            self.dispatch();
        } else {
            self.update_stats();
        }
        changed
    }
    pub fn get(&mut self, id: AssetId) -> Option<&Arc<Decoded>> {
        let rev = *self.current.get(&id)?;
        self.get_current(id, rev)
    }
    pub fn observe_current(&mut self, id: AssetId, revision: u64, source: SourceId) {
        self.sources.insert(id, source);
        if self
            .current
            .insert(id, revision)
            .is_some_and(|old| old != revision)
        {
            self.states.remove(&source);
            self.prepared.remove(&id);
        }
    }
    pub fn get_current(&mut self, id: AssetId, revision: u64) -> Option<&Arc<Decoded>> {
        self.get_lod(id, revision, Lod::Thumbnail)
    }
    pub fn get_lod(&mut self, id: AssetId, revision: u64, lod: Lod) -> Option<&Arc<Decoded>> {
        self.get_rep(id, revision, lod, lod.edge())
    }
    pub fn get_rep(
        &mut self,
        id: AssetId,
        revision: u64,
        lod: Lod,
        edge: u32,
    ) -> Option<&Arc<Decoded>> {
        let key = Key {
            asset: (lod == Lod::Thumbnail).then_some(id),
            source: *self.sources.get(&id)?,
            revision,
            lod,
            edge,
        };
        if lod == Lod::Thumbnail {
            self.cache.get(key)
        } else {
            self.details.get(key)
        }
    }
    pub fn suspend(&mut self) {
        self.view_mode = true;
        self.wanted.clear();
        self.queue.clear();
        self.poll();
        self.update_stats();
    }
    pub fn stats(&self) -> ProductAssetStats {
        self.stats
    }
    pub fn failed_rep(&self, id: AssetId, lod: Lod, edge: u32) -> bool {
        self.current
            .get(&id)
            .zip(self.sources.get(&id))
            .is_some_and(|(r, s)| {
                self.failed.contains(&Key {
                    asset: (lod == Lod::Thumbnail).then_some(id),
                    source: *s,
                    revision: *r,
                    lod,
                    edge,
                })
            })
    }
    pub fn failed_lod(&self, id: AssetId, lod: Lod) -> bool {
        self.failed_rep(id, lod, lod.edge())
    }
    pub fn failed(&self, id: AssetId) -> bool {
        self.failed_lod(id, Lod::Thumbnail)
    }
}
fn load(job: Job, base: &Path, dir: &Path, disk: &Mutex<crate::decode::DiskCache>) -> Outcome {
    let board = &job.board;
    let id = job.asset.id();
    let source = Some(job.source.id());
    let mut outcome = Outcome {
        key: Key {
            asset: (job.lod == Lod::Thumbnail).then_some(id),
            source: job.source.id(),
            revision: job.source.revision(),
            lod: job.lod,
            edge: job.edge,
        },
        asset: id,
        source,
        state: SourceState::Missing,
        result: Err("missing source".into()),
        prepared: None,
        container_bytes: 0,
        source_bytes: 0,
        derived_bytes: 0,
        repair_cache_bytes: 0,
        repair_cache_hit: false,
        decode_ms: 0.,
    };
    let result = (|| -> Result<Decoded, AssetError> {
        let source = source.ok_or("missing asset metadata")?;
        let s = &job.source;
        let path = match s.location() {
            SourceLocation::Embedded => {
                outcome.state = SourceState::Embedded;
                None
            }
            SourceLocation::Linked(p) => match p.to_native() {
                Some(path) => {
                    let path = if p.is_absolute() {
                        path
                    } else {
                        base.join(path)
                    };
                    outcome.state = match representation::source_fingerprint(&path) {
                        Ok(f) if s.fingerprint().is_none_or(|old| old == f) => {
                            SourceState::Available
                        }
                        Ok(_) => SourceState::Changed,
                        Err(e)
                            if e.downcast_ref::<std::io::Error>()
                                .is_some_and(|e| e.kind() == std::io::ErrorKind::NotFound) =>
                        {
                            SourceState::Missing
                        }
                        Err(_) => SourceState::Unavailable,
                    };
                    Some(path)
                }
                None => {
                    outcome.state = SourceState::Foreign;
                    None
                }
            },
        };
        if job.lod == Lod::Thumbnail
            && let Some(e) = board.overviews.get(&id)
            && e.revision == s.revision()
            && board
                .document
                .asset(id)
                .is_some_and(|a| a.source_id() == source)
        {
            outcome.container_bytes += e.range.len;
            if let Ok(image) = board.overview_bytes(id).and_then(|bytes| {
                representation::decode_png(&bytes, Some([e.width, e.height]))
                    .map_err(|_| tack_storage::StorageError::Corrupt("overview encoding"))
            }) {
                return resize(image, job.edge);
            }
        }
        let cache_path = |generator| {
            dir.join(format!(
                "{:032x}-{}-{}-{}.png",
                if job.lod == Lod::Thumbnail {
                    id.value()
                } else {
                    s.id().value()
                },
                s.revision(),
                generator,
                job.edge
            ))
        };
        {
            let _guard = disk.lock().map_err(|_| "repair cache lock poisoned")?;
            for generator in [1, 2] {
                let file = cache_path(generator);
                if let Ok(input) = File::open(&file) {
                    use std::io::Read;
                    let mut bytes = Vec::new();
                    input
                        .take(u64::from(job.edge).pow(2) * 5 + 1)
                        .read_to_end(&mut bytes)?;
                    outcome.repair_cache_bytes += bytes.len() as u64;
                    if let Ok(image) = representation::decode_png(&bytes, None)
                        && image.width.max(image.height) <= job.edge
                    {
                        outcome.prepared = (job.lod == Lod::Thumbnail && job.edge == 128)
                            .then_some(PreparedOverview {
                                asset: id,
                                revision: s.revision(),
                                size: [image.width, image.height],
                                generator,
                                path: file.clone(),
                            });
                        outcome.repair_cache_hit = true;
                        return Ok(image);
                    }
                    std::fs::remove_file(&file)?;
                }
            }
        }
        let (pixels, generator) = if let Some(path) = path {
            if outcome.state != SourceState::Available {
                return Err("source missing or changed; explicit relink/revision required".into());
            }
            let before = representation::source_fingerprint(&path)?;
            let (pixels, generator) =
                representation::derive_linked_edge(&path, &mut outcome.source_bytes, job.edge)?;
            if representation::source_fingerprint(&path)? != before {
                return Err("source changed during derivation".into());
            }
            (pixels, generator)
        } else if outcome.state == SourceState::Embedded {
            let mut reader = CountRead {
                inner: match &job.original {
                    Some(Payload::Stored { file, range }) => {
                        RangeReader::new(Arc::clone(file), *range)
                    }
                    Some(Payload::File(_)) => {
                        return Err("embedded import requires a pinned original".into());
                    }
                    None => board.original_reader(source)?,
                },
                count: 0,
            };
            let result = representation::derive_stream_edge(&mut reader, job.edge);
            outcome.container_bytes += reader.count;
            let pixels = result?;
            (pixels, 2)
        } else {
            return Err("no available source or stored last-known preview".into());
        };
        let file = cache_path(generator);
        let stored = (|| -> Result<bool, AssetError> {
            let bytes = representation::encode(&pixels)?;
            if bytes.len() > 20 * 1024 * 1024 {
                return Ok(false);
            }
            let guard = disk.lock().map_err(|_| "repair cache lock poisoned")?;
            if bytes.len() > guard.budget_bytes() {
                return Ok(false);
            }
            tack_storage::create_private_directory(dir, true)?;
            guard.trim_for(bytes.len())?;
            let temporary = file.with_extension("partial");
            let mut options = File::options();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut output = options.open(&temporary)?;
            if let Err(e) = output.write_all(&bytes) {
                drop(output);
                let _ = std::fs::remove_file(&temporary);
                return Err(e.into());
            }
            drop(output);
            std::fs::rename(&temporary, &file)?;
            outcome.derived_bytes = bytes.len() as u64;
            Ok(true)
        })();
        if matches!(stored, Ok(true)) && job.lod == Lod::Thumbnail && job.edge == 128 {
            outcome.prepared = Some(PreparedOverview {
                asset: id,
                revision: s.revision(),
                size: [pixels.width(), pixels.height()],
                generator,
                path: file,
            });
        }
        Ok(Decoded {
            width: pixels.width(),
            height: pixels.height(),
            rgba: pixels.into_raw(),
        })
    })();
    if let Err(e) = &result {
        tracing::warn!(asset=?id,%e,"product overview unavailable");
    }
    outcome.result = result;
    outcome
}

// Shutdown only: owners must drop outside native event/render callbacks. Closing
// receivers prevents blocked result publication; at most the active codec finishes.
impl Drop for ProductAssets {
    fn drop(&mut self) {
        for w in std::mem::take(&mut self.workers) {
            drop(w.sender);
            drop(w.receiver);
            let _ = w.handle.join();
        }
    }
}

fn resize(image: Decoded, edge: u32) -> Result<Decoded, AssetError> {
    if image.width.max(image.height) <= edge {
        return Ok(image);
    }
    let pixels = image::RgbaImage::from_raw(image.width, image.height, image.rgba)
        .ok_or("display output dimensions")?;
    let pixels = image::DynamicImage::ImageRgba8(pixels)
        .thumbnail(edge, edge)
        .into_rgba8();
    Ok(Decoded {
        width: pixels.width(),
        height: pixels.height(),
        rgba: pixels.into_raw(),
    })
}
