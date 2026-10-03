use crate::{AssetError, JobProfile, decode, profile::Stage};
use std::{
    collections::HashSet,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc::{self, Receiver, SyncSender, TryRecvError},
    },
    thread,
    time::Instant,
};
use tack_core::{ByteCache, Lod};

pub const MAX_PENDING_PER_WORKER: usize = 8;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
/// Benchmark display-cache key; not a stable product AssetId.
pub struct AssetKey {
    pub id: u32,
    pub lod: Lod,
}

pub struct Decoded {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[derive(Clone)]
pub struct DecodeRequest {
    pub key: AssetKey,
    pub path: PathBuf,
    pub source_sha256: String,
}

pub(crate) struct Job {
    pub request: DecodeRequest,
    pub cancelled: Arc<AtomicBool>,
    pub queued: Instant,
    pub epoch: Instant,
}

pub(crate) struct Outcome {
    pub key: AssetKey,
    pub result: Result<Option<Arc<Decoded>>, AssetError>,
    pub disk_hit: bool,
    pub decode_ms: f64,
    pub profile: JobProfile,
}

struct Worker {
    sender: Option<SyncSender<Job>>,
    receiver: Receiver<Outcome>,
    active: Vec<ActiveJob>,
    retention_peak: Arc<AtomicUsize>,
    thread: Option<thread::JoinHandle<()>>,
}

struct ActiveJob {
    key: AssetKey,
    cancelled: Arc<AtomicBool>,
    background: bool,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct LoaderStats {
    pub pending: usize,
    pub pending_peak: usize,
    pub stale: u64,
    pub errors: u64,
    pub completed: u64,
    pub disk_hits: u64,
    pub decode_ms: f64,
    pub cpu_bytes: usize,
    pub cpu_evictions: u64,
    pub cpu_thumbnail_bytes: usize,
    pub rejected: u64,
    pub worker_retention_peak_bytes: usize,
}

/// Event thread owns cache and scheduling; workers own all I/O and decoding.
/// At most eight cheap jobs/results or one large job per worker. Each large
/// buffer retains a worker-owned Arc so eviction never frees its pages on a frame.
pub struct Loader {
    workers: Vec<Worker>,
    cache: ByteCache<AssetKey, Arc<Decoded>>,
    thumbnails: ByteCache<AssetKey, Arc<Decoded>>,
    desired: HashSet<AssetKey>,
    failed: HashSet<AssetKey>,
    stats: LoaderStats,
    profiles: Vec<JobProfile>,
    profile_dropped: usize,
    started: Instant,
}

impl Loader {
    pub fn new(
        cache_dir: PathBuf,
        cpu_budget_bytes: usize,
        disk_budget_bytes: usize,
        worker_count: usize,
    ) -> Result<Self, AssetError> {
        let started = Instant::now();
        if ![1, 2, 4].contains(&worker_count) {
            return Err("worker count must be 1, 2 or 4".into());
        }
        // Four stable disk shards give cache identity and quota independent of
        // worker count. Powers-of-two worker ownership never overlaps a shard.
        // Initial quota cleanup is an explicit startup boundary, not frame I/O.
        let mut shards: Vec<_> = (0..4)
            .map(|index| {
                decode::DiskCache::new(
                    cache_dir.join(format!("worker-{index}")),
                    disk_budget_bytes / 4,
                )
            })
            .collect();
        for shard in &mut shards {
            shard.prepare();
        }
        let mut workers = Vec::new();
        for index in 0..worker_count {
            let mut disks = shards.clone();
            let retention_peak = Arc::new(AtomicUsize::new(0));
            let worker_peak = Arc::clone(&retention_peak);
            let (job_tx, job_rx) = mpsc::sync_channel::<Job>(MAX_PENDING_PER_WORKER);
            let (result_tx, result_rx) = mpsc::sync_channel(MAX_PENDING_PER_WORKER);
            let thread = thread::Builder::new()
                .name(format!("tack-decode-{index}"))
                .spawn(move || {
                    let mut retained: Vec<Arc<Decoded>> = Vec::new();
                    while let Ok(job) = job_rx.recv() {
                        // Keep the last ownership reference on workers so eviction
                        // of a large display buffer cannot unmap pages on a frame.
                        let reclaim = Instant::now();
                        retained.retain(|image| Arc::strong_count(image) > 1);
                        let reclaim_ms = reclaim.elapsed().as_secs_f64() * 1000.0;
                        let shard = job.request.key.id as usize % 4;
                        let mut outcome = decode::run(&job, &mut disks[shard]);
                        outcome.profile.stage_ms[Stage::Retire as usize] = reclaim_ms;
                        outcome.profile.active_ms += reclaim_ms;
                        if let Ok(Some(image)) = &outcome.result {
                            retained.push(Arc::clone(image));
                        }
                        worker_peak.fetch_max(
                            retained.iter().map(|image| image.rgba.len()).sum(),
                            Ordering::Relaxed,
                        );
                        if result_tx.send(outcome).is_err() {
                            break;
                        }
                    }
                })?;
            workers.push(Worker {
                sender: Some(job_tx),
                receiver: result_rx,
                active: Vec::new(),
                retention_peak,
                thread: Some(thread),
            });
        }
        let thumbnail_budget = if cpu_budget_bytes >= 16 * 1024 * 1024 {
            (cpu_budget_bytes / 2).min(cpu_budget_bytes - 12 * 1024 * 1024)
        } else {
            cpu_budget_bytes
        };
        Ok(Self {
            workers,
            cache: ByteCache::new(cpu_budget_bytes - thumbnail_budget),
            thumbnails: ByteCache::new(thumbnail_budget),
            desired: HashSet::new(),
            failed: HashSet::new(),
            stats: LoaderStats::default(),
            profiles: Vec::new(),
            profile_dropped: 0,
            started,
        })
    }

    /// Requests are ordered by screen priority. Missing GPU data drives this list.
    /// The caller bounds it to the immutable board's at-most-10,000 visible objects.
    pub fn request(&mut self, requests: &[DecodeRequest]) {
        self.request_with_background(requests, &[]);
    }

    /// Benchmark preparation shares the same pool. Foreground admission comes
    /// first; at most one background job per worker prevents a background FIFO
    /// backlog. It counts against the unchanged eight-job total, never adds a pool.
    pub fn request_with_background(
        &mut self,
        foreground: &[DecodeRequest],
        background: &[DecodeRequest],
    ) {
        self.desired.clear();
        self.desired
            .extend(foreground.iter().chain(background).map(|r| r.key));
        self.failed.retain(|key| self.desired.contains(key));
        for worker in &mut self.workers {
            for active in &worker.active {
                if !self.desired.contains(&active.key) {
                    active.cancelled.store(true, Ordering::Relaxed);
                }
            }
        }
        for (is_background, requests) in [(false, foreground), (true, background)] {
            for request in requests {
                if self.has_cached(request.key)
                    || self.failed.contains(&request.key)
                    || self
                        .workers
                        .iter()
                        .any(|w| w.active.iter().any(|active| active.key == request.key))
                {
                    continue;
                }
                // Stable sharding also stabilizes persistent cache identity across sessions.
                let index = request.key.id as usize % self.workers.len();
                let worker = &mut self.workers[index];
                let thumbnail = request.key.lod == Lod::Thumbnail;
                if worker.active.len() >= MAX_PENDING_PER_WORKER
                    || (is_background && !thumbnail)
                    || (is_background && worker.active.iter().any(|active| active.background))
                    || (!thumbnail && !worker.active.is_empty())
                    || worker
                        .active
                        .iter()
                        .any(|active| active.key.lod != Lod::Thumbnail)
                {
                    continue;
                }
                let Some(sender) = &worker.sender else {
                    continue;
                };
                let cancelled = Arc::new(AtomicBool::new(false));
                let job = Job {
                    request: request.clone(),
                    cancelled: Arc::clone(&cancelled),
                    queued: Instant::now(),
                    epoch: self.started,
                };
                if sender.try_send(job).is_ok() {
                    worker.active.push(ActiveJob {
                        key: request.key,
                        cancelled,
                        background: is_background,
                    });
                }
            }
        }
        self.stats.pending_peak = self
            .stats
            .pending_peak
            .max(self.workers.iter().map(|w| w.active.len()).sum());
    }

    /// Non-blocking; at most eight outcomes per worker per call.
    pub fn poll(&mut self) -> bool {
        let mut changed = false;
        for worker in &mut self.workers {
            for _ in 0..MAX_PENDING_PER_WORKER {
                match worker.receiver.try_recv() {
                    Ok(mut outcome) => {
                        worker.active.retain(|active| active.key != outcome.key);
                        self.stats.decode_ms += outcome.decode_ms;
                        if !self.desired.contains(&outcome.key) {
                            self.stats.stale += 1;
                            outcome.profile.cancelled_after.get_or_insert("result");
                            if self.profiles.len() < 20000 {
                                self.profiles.push(outcome.profile);
                            } else {
                                self.profile_dropped += 1;
                            }
                            continue;
                        }
                        match outcome.result {
                            Ok(Some(image)) => {
                                let bytes = image.rgba.len();
                                let cache = if outcome.key.lod == Lod::Thumbnail {
                                    &mut self.thumbnails
                                } else {
                                    &mut self.cache
                                };
                                let inserted = outcome.profile.measure(Stage::CpuInsert, || {
                                    cache.insert(outcome.key, image, bytes)
                                });
                                changed |= inserted;
                                if !inserted {
                                    self.failed.insert(outcome.key);
                                    self.stats.rejected += 1;
                                }
                                outcome.profile.bytes(Stage::CpuInsert, bytes);
                                self.stats.completed += u64::from(inserted);
                                self.stats.disk_hits += u64::from(outcome.disk_hit);
                            }
                            Ok(None) => {
                                self.stats.stale += 1;
                            }
                            Err(error) => {
                                tracing::warn!(asset_id = outcome.key.id, %error, "display image unavailable");
                                self.failed.insert(outcome.key);
                                self.stats.errors += 1;
                            }
                        }
                        if self.profiles.len() < 20000 {
                            self.profiles.push(outcome.profile);
                        } else {
                            self.profile_dropped += 1;
                        }
                    }
                    Err(TryRecvError::Disconnected) => {
                        for active in worker.active.drain(..) {
                            self.failed.insert(active.key);
                            self.stats.errors += 1;
                        }
                        worker.sender = None;
                    }
                    Err(TryRecvError::Empty) => break,
                }
            }
        }
        changed
    }

    pub fn get(&mut self, key: AssetKey) -> Option<&Decoded> {
        if key.lod == Lod::Thumbnail {
            self.thumbnails.get(key).map(Arc::as_ref)
        } else {
            self.cache.get(key).map(Arc::as_ref)
        }
    }

    pub fn has_cached(&self, key: AssetKey) -> bool {
        if key.lod == Lod::Thumbnail {
            self.thumbnails.contains(key)
        } else {
            self.cache.contains(key)
        }
    }

    pub fn stats(&self) -> LoaderStats {
        LoaderStats {
            pending: self.workers.iter().map(|w| w.active.len()).sum(),
            cpu_bytes: self.cache.used_bytes() + self.thumbnails.used_bytes(),
            cpu_evictions: self.cache.evictions() + self.thumbnails.evictions(),
            cpu_thumbnail_bytes: self.thumbnails.used_bytes(),
            worker_retention_peak_bytes: self
                .workers
                .iter()
                .map(|w| w.retention_peak.load(Ordering::Relaxed))
                .sum(),
            ..self.stats
        }
    }

    pub fn profiles(&self) -> &[JobProfile] {
        &self.profiles
    }

    pub fn profile_dropped(&self) -> usize {
        self.profile_dropped
    }
}

impl Drop for Loader {
    fn drop(&mut self) {
        // Joining would wait for an uninterruptible codec. Signal shutdown and let
        // threads finish naturally; process teardown is outside frame timing.
        for worker in &mut self.workers {
            for active in &worker.active {
                active.cancelled.store(true, Ordering::Relaxed);
            }
            worker.sender.take();
            worker.thread.take();
        }
    }
}
