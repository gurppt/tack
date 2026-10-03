//! Bounded product overview supply. Storage/stat/decode stay on workers.
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
};
use tack_core::{AssetId, ByteCache, SourceId, SourceLocation};
use tack_storage::TackFile;
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
#[derive(Clone, Copy, Default, Debug)]
pub struct ProductAssetStats {
    pub pending: usize,
    pub peak_pending: usize,
    pub completed: usize,
    pub reused: usize,
    pub regenerated: usize,
    pub repair_cache_hits: usize,
    pub repair_cache_bytes: u64,
    pub errors: usize,
    pub container_bytes: u64,
    pub source_bytes: u64,
    pub derived_bytes: u64,
    pub cpu_bytes: usize,
}
struct Outcome {
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
}
struct Worker {
    sender: SyncSender<AssetId>,
    receiver: Receiver<Outcome>,
    handle: thread::JoinHandle<()>,
}
pub struct ProductAssets {
    board: Arc<TackFile>,
    workers: Vec<Worker>,
    pending: HashMap<AssetId, usize>,
    failed: HashSet<AssetId>,
    cache: ByteCache<AssetId, Arc<Decoded>>,
    pub states: BTreeMap<SourceId, SourceState>,
    pub prepared: BTreeMap<AssetId, PreparedOverview>,
    stats: ProductAssetStats,
}
impl ProductAssets {
    pub fn new(
        board: Arc<TackFile>,
        board_path: &Path,
        repair_dir: PathBuf,
    ) -> Result<Self, AssetError> {
        Self::with_budgets(
            board,
            board_path,
            repair_dir,
            64 * 1024 * 1024,
            512 * 1024 * 1024,
        )
    }
    /// Composition/test budgets, bounded by the production maxima. Disk maintenance is worker-only.
    pub fn with_budgets(
        board: Arc<TackFile>,
        board_path: &Path,
        repair_dir: PathBuf,
        cpu_bytes: usize,
        disk_bytes: usize,
    ) -> Result<Self, AssetError> {
        if cpu_bytes == 0
            || cpu_bytes > 64 * 1024 * 1024
            || !(1024 * 1024..=512 * 1024 * 1024).contains(&disk_bytes)
        {
            return Err("invalid product cache budgets".into());
        }
        let disk = Arc::new(Mutex::new(crate::decode::DiskCache::new(
            repair_dir.clone(),
            disk_bytes,
        )));
        // Startup/worker-composition boundary, never called per frame.

        let base = board_path.parent().unwrap_or(Path::new(".")).to_path_buf();
        let mut workers = Vec::new();
        for _ in 0..2 {
            let (tx, rx) = mpsc::sync_channel::<AssetId>(8);
            let (done_tx, done_rx) = mpsc::sync_channel(8);
            let b = Arc::clone(&board);
            let base = base.clone();
            let dir = repair_dir.clone();
            let disk = Arc::clone(&disk);
            let handle = thread::Builder::new()
                .name("tack-product-overview".into())
                .spawn(move || {
                    while let Ok(id) = rx.recv() {
                        let outcome = load(&b, &base, &dir, &disk, id);
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
        Ok(Self {
            board,
            workers,
            pending: HashMap::new(),
            failed: HashSet::new(),
            cache: ByteCache::new(cpu_bytes),
            states: BTreeMap::new(),
            prepared: BTreeMap::new(),
            stats: ProductAssetStats::default(),
        })
    }
    pub fn request(&mut self, id: AssetId) -> bool {
        if self.cache.contains(id)
            || self.pending.contains_key(&id)
            || self.failed.contains(&id)
            || self.pending.len() >= 16
            || self.board.document.asset(id).is_none()
        {
            return false;
        }
        let worker = self
            .workers
            .iter()
            .enumerate()
            .filter(|(i, _)| {
                self.pending
                    .values()
                    .filter(|assigned| **assigned == *i)
                    .count()
                    < 8
            })
            .find(|(_, w)| w.sender.try_send(id).is_ok());
        let Some((index, _)) = worker else {
            return false;
        };
        self.pending.insert(id, index);
        self.stats.pending = self.pending.len();
        self.stats.peak_pending = self.stats.peak_pending.max(self.stats.pending);
        true
    }
    pub fn poll(&mut self) -> bool {
        let mut changed = false;
        for w in &self.workers {
            while let Ok(o) = w.receiver.try_recv() {
                changed = true;
                self.pending.remove(&o.asset);
                self.stats.completed += 1;
                self.stats.container_bytes += o.container_bytes;
                self.stats.source_bytes += o.source_bytes;
                if let Some(source) = o.source {
                    self.states.insert(source, o.state);
                }
                self.stats.repair_cache_bytes += o.repair_cache_bytes;
                if o.repair_cache_hit {
                    self.stats.repair_cache_hits += 1;
                }
                if let Some(p) = o.prepared {
                    self.stats.regenerated += usize::from(!o.repair_cache_hit);
                    self.stats.derived_bytes += o.derived_bytes;
                    self.prepared.insert(o.asset, p);
                } else if o.result.is_ok() {
                    self.stats.reused += 1;
                }
                match o.result {
                    Ok(image) => {
                        let bytes = image.rgba.len();
                        self.cache.insert(o.asset, Arc::new(image), bytes);
                    }
                    Err(_) => {
                        self.stats.errors += 1;
                        self.failed.insert(o.asset);
                    }
                }
            }
        }
        self.stats.pending = self.pending.len();
        self.stats.cpu_bytes = self.cache.used_bytes();
        changed
    }
    pub fn get(&mut self, id: AssetId) -> Option<&Arc<Decoded>> {
        self.cache.get(id)
    }
    pub fn stats(&self) -> ProductAssetStats {
        self.stats
    }
    pub fn failed(&self, id: AssetId) -> bool {
        self.failed.contains(&id)
    }
}
fn load(
    board: &TackFile,
    base: &Path,
    dir: &Path,
    disk: &Mutex<crate::decode::DiskCache>,
    id: AssetId,
) -> Outcome {
    let source = board.document.asset(id).map(|a| a.source_id());
    let mut outcome = Outcome {
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
    };
    let result = (|| -> Result<Decoded, AssetError> {
        let source = source.ok_or("missing asset metadata")?;
        let s = board
            .document
            .source(source)
            .ok_or("missing source metadata")?;
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
        if let Some(e) = board.overviews.get(&id) {
            outcome.container_bytes += e.range.len;
            if let Ok(image) = board.overview_bytes(id).and_then(|bytes| {
                representation::decode_png(&bytes, Some([e.width, e.height]))
                    .map_err(|_| tack_storage::StorageError::Corrupt("overview encoding"))
            }) {
                return Ok(image);
            }
        }
        let cache_path = |generator| {
            dir.join(format!(
                "{:032x}-{}-{}-128.png",
                id.value(),
                s.revision(),
                generator
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
                        .take(tack_storage::MAX_OVERVIEW_BYTES + 1)
                        .read_to_end(&mut bytes)?;
                    outcome.repair_cache_bytes += bytes.len() as u64;
                    if let Ok(image) = representation::decode_png(&bytes, None) {
                        outcome.prepared = Some(PreparedOverview {
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
                representation::derive_linked(&path, &mut outcome.source_bytes)?;
            if representation::source_fingerprint(&path)? != before {
                return Err("source changed during derivation".into());
            }
            (pixels, generator)
        } else if outcome.state == SourceState::Embedded {
            let mut reader = CountRead {
                inner: board.original_reader(source)?,
                count: 0,
            };
            let result = representation::derive_stream(&mut reader);
            outcome.container_bytes += reader.count;
            let pixels = result?;
            (pixels, 2)
        } else {
            return Err("no available source or stored last-known preview".into());
        };
        let bytes = representation::encode(&pixels)?;
        let file = cache_path(generator);
        // Both workers share maintenance/write ownership. Repeated CPU evictions reuse
        // one stable derived file; disk bytes never grow with the number of requests.
        let _guard = disk.lock().map_err(|_| "repair cache lock poisoned")?;
        tack_storage::create_private_directory(dir, true)?;
        _guard.trim_for(bytes.len())?;
        let mut options = File::options();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut output = options.open(&file)?;
        if let Err(e) = output.write_all(&bytes) {
            drop(output);
            let _ = std::fs::remove_file(&file);
            return Err(e.into());
        }
        drop(output);
        outcome.derived_bytes = bytes.len() as u64;
        outcome.prepared = Some(PreparedOverview {
            asset: id,
            revision: s.revision(),
            size: [pixels.width(), pixels.height()],
            generator,
            path: file,
        });
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
