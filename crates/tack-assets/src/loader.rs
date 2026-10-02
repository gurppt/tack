use crate::{AssetError, decode};
use std::{
    collections::HashSet,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender, TryRecvError},
    },
    thread,
};
use tack_core::{ByteCache, Lod};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
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
}

pub(crate) struct Outcome {
    pub key: AssetKey,
    pub result: Result<Option<Decoded>, AssetError>,
    pub disk_hit: bool,
    pub decode_ms: f64,
}

struct Worker {
    sender: Option<SyncSender<Job>>,
    receiver: Receiver<Outcome>,
    active: Option<(AssetKey, Arc<AtomicBool>)>,
    thread: Option<thread::JoinHandle<()>>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct LoaderStats {
    pub pending: usize,
    pub stale: u64,
    pub errors: u64,
    pub completed: u64,
    pub disk_hits: u64,
    pub decode_ms: f64,
    pub cpu_bytes: usize,
    pub cpu_evictions: u64,
}

/// Event thread owns cache and scheduling; workers own all I/O and decoding.
/// At most one job and one result per worker, never a camera-motion backlog.
pub struct Loader {
    workers: Vec<Worker>,
    cache: ByteCache<AssetKey, Decoded>,
    desired: HashSet<AssetKey>,
    failed: HashSet<AssetKey>,
    stats: LoaderStats,
}

impl Loader {
    pub fn new(
        cache_dir: PathBuf,
        cpu_budget_bytes: usize,
        disk_budget_bytes: usize,
        worker_count: usize,
    ) -> Result<Self, AssetError> {
        if !(1..=4).contains(&worker_count) {
            return Err("worker count must be 1..=4".into());
        }
        let mut workers = Vec::new();
        for index in 0..worker_count {
            let dir = cache_dir.join(format!("worker-{index}"));
            let quota = disk_budget_bytes / worker_count;
            let (job_tx, job_rx) = mpsc::sync_channel::<Job>(1);
            let (result_tx, result_rx) = mpsc::sync_channel(1);
            let thread = thread::Builder::new()
                .name(format!("tack-decode-{index}"))
                .spawn(move || {
                    let mut disk = decode::DiskCache::new(dir, quota);
                    while let Ok(job) = job_rx.recv() {
                        let outcome = decode::run(&job, &mut disk);
                        if result_tx.send(outcome).is_err() {
                            break;
                        }
                    }
                })?;
            workers.push(Worker {
                sender: Some(job_tx),
                receiver: result_rx,
                active: None,
                thread: Some(thread),
            });
        }
        Ok(Self {
            workers,
            cache: ByteCache::new(cpu_budget_bytes),
            desired: HashSet::new(),
            failed: HashSet::new(),
            stats: LoaderStats::default(),
        })
    }

    /// Requests are ordered by screen priority. Missing GPU data drives this list.
    /// The caller bounds it to the immutable board's at-most-10,000 visible objects.
    pub fn request(&mut self, requests: &[DecodeRequest]) {
        self.desired.clear();
        self.desired.extend(requests.iter().map(|r| r.key));
        self.failed.retain(|key| self.desired.contains(key));
        for worker in &mut self.workers {
            if let Some((key, cancelled)) = &worker.active
                && !self.desired.contains(key)
            {
                cancelled.store(true, Ordering::Relaxed);
            }
        }
        for request in requests {
            if self.cache.contains(request.key)
                || self.failed.contains(&request.key)
                || self
                    .workers
                    .iter()
                    .any(|w| w.active.as_ref().is_some_and(|(k, _)| *k == request.key))
            {
                continue;
            }
            // Stable sharding also stabilizes persistent cache identity across sessions.
            let index = request.key.id as usize % self.workers.len();
            let worker = &mut self.workers[index];
            if worker.active.is_some() {
                continue;
            }
            let Some(sender) = &worker.sender else {
                continue;
            };
            let cancelled = Arc::new(AtomicBool::new(false));
            let job = Job {
                request: request.clone(),
                cancelled: Arc::clone(&cancelled),
            };
            if sender.try_send(job).is_ok() {
                worker.active = Some((request.key, cancelled));
            }
        }
    }

    /// Non-blocking; maximum results handled per call equals worker count.
    pub fn poll(&mut self) -> bool {
        let mut changed = false;
        for worker in &mut self.workers {
            match worker.receiver.try_recv() {
                Ok(outcome) => {
                    worker.active = None;
                    self.stats.decode_ms += outcome.decode_ms;
                    if !self.desired.contains(&outcome.key) {
                        self.stats.stale += 1;
                        continue;
                    }
                    match outcome.result {
                        Ok(Some(image)) => {
                            let bytes = image.rgba.len();
                            changed |= self.cache.insert(outcome.key, image, bytes);
                            self.stats.completed += 1;
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
                }
                Err(TryRecvError::Disconnected) => {
                    if let Some((key, _)) = worker.active.take() {
                        self.failed.insert(key);
                        self.stats.errors += 1;
                    }
                    worker.sender = None;
                }
                Err(TryRecvError::Empty) => {}
            }
        }
        changed
    }

    pub fn get(&mut self, key: AssetKey) -> Option<&Decoded> {
        self.cache.get(key)
    }

    pub fn has_cached(&self, key: AssetKey) -> bool {
        self.cache.contains(key)
    }

    pub fn stats(&self) -> LoaderStats {
        LoaderStats {
            pending: self.workers.iter().filter(|w| w.active.is_some()).count(),
            cpu_bytes: self.cache.used_bytes(),
            cpu_evictions: self.cache.evictions(),
            ..self.stats
        }
    }
}

impl Drop for Loader {
    fn drop(&mut self) {
        // Joining would wait for an uninterruptible codec. Signal shutdown and let
        // threads finish naturally; process teardown is outside frame timing.
        for worker in &mut self.workers {
            if let Some((_, cancelled)) = &worker.active {
                cancelled.store(true, Ordering::Relaxed);
            }
            worker.sender.take();
            worker.thread.take();
        }
    }
}
