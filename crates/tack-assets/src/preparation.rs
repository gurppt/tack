//! Benchmark-only accounting for the immutable board's 128-pixel cache set.
//! This is historical validated progress, not a persistent-cache inventory.
use crate::{AssetKey, Board, DecodeRequest, JobProfile, Loader};
use serde::Serialize;
use std::{
    collections::{HashMap, HashSet},
    time::Instant,
};
use tack_core::Lod;

#[derive(Serialize)]
pub struct Progress {
    pub elapsed_ms: f64,
    pub ready: usize,
    pub errors: usize,
}

pub struct OverviewPreparation {
    requests: Vec<DecodeRequest>,
    ids: HashSet<u32>,
    ready: HashSet<u32>,
    errors: HashSet<u32>,
    observed: usize,
    started: Instant,
    progress: Vec<Progress>,
    pending_peak: usize,
    cpu_peak: usize,
}

impl OverviewPreparation {
    /// Clock includes manifest loading and cache initialization when the caller
    /// passes its process-start Instant. No filesystem access happens here.
    pub fn new(board: &Board, started: Instant) -> Self {
        let mut requests: Vec<_> = board
            .objects
            .iter()
            .map(|o| DecodeRequest {
                key: AssetKey {
                    id: o.id,
                    lod: Lod::Thumbnail,
                },
                path: o.path.clone(),
                source_sha256: o.source_sha256.clone(),
            })
            .collect();
        requests.sort_by_key(|r| r.key.id);
        Self {
            requests,
            ids: board.objects.iter().map(|o| o.id).collect(),
            ready: HashSet::new(),
            errors: HashSet::new(),
            observed: 0,
            started,
            progress: Vec::new(),
            pending_peak: 0,
            cpu_peak: 0,
        }
    }

    /// Ready survives CPU eviction and stale delivery after a successful write.
    /// Cancellation without persistence remains retryable. A broken cache is a
    /// terminal preparation error even when a usable RAM image was delivered.
    pub fn observe(&mut self, loader: &Loader) {
        let before = (self.ready.len(), self.errors.len());
        for profile in &loader.profiles()[self.observed..] {
            if profile.edge != 128 || !self.ids.contains(&profile.asset_id) {
                continue;
            }
            if profile.cache_entry_ready {
                self.ready.insert(profile.asset_id);
                self.errors.remove(&profile.asset_id);
            } else if !self.ready.contains(&profile.asset_id)
                && (profile.failed || profile.cancelled_after.is_none())
            {
                self.errors.insert(profile.asset_id);
            }
        }
        self.observed = loader.profiles().len();
        let stats = loader.stats();
        self.pending_peak = self.pending_peak.max(stats.pending_peak);
        self.cpu_peak = self.cpu_peak.max(stats.cpu_bytes);
        if before != (self.ready.len(), self.errors.len()) {
            self.progress.push(Progress {
                elapsed_ms: self.started.elapsed().as_secs_f64() * 1000.0,
                ready: self.ready.len(),
                errors: self.errors.len(),
            });
        }
    }

    pub fn ready(&self) -> usize {
        self.ready.len()
    }
    pub fn total(&self) -> usize {
        self.requests.len()
    }
    pub fn settled(&self) -> bool {
        self.ready.len() + self.errors.len() == self.total()
    }

    /// Remaining stable ID order; bounded by the immutable board, not a queue.
    pub fn remaining(&self, output: &mut Vec<DecodeRequest>) {
        output.clear();
        output.extend(
            self.requests
                .iter()
                .filter(|r| !self.ready.contains(&r.key.id) && !self.errors.contains(&r.key.id))
                .cloned(),
        );
    }

    pub fn report(&self, loader: &Loader, workers: usize) -> serde_json::Value {
        let profiles: Vec<&JobProfile> =
            loader.profiles().iter().filter(|p| p.edge == 128).collect();
        let elapsed_ms = self.started.elapsed().as_secs_f64() * 1000.0;
        let thresholds: HashMap<String, Option<f64>> = [1usize, 10, 25, 50, 75, 90, 100]
            .into_iter()
            .map(|percent| {
                let needed = if percent == 1 {
                    1
                } else {
                    (self.total() * percent).div_ceil(100)
                };
                (
                    if percent == 1 {
                        "first".into()
                    } else {
                        percent.to_string()
                    },
                    self.progress
                        .iter()
                        .find(|p| p.ready >= needed)
                        .map(|p| p.elapsed_ms),
                )
            })
            .collect();
        let active_ms: f64 = profiles.iter().map(|p| p.active_ms).sum();
        serde_json::json!({
            "total": self.total(), "ready": self.ready(), "errors": self.errors.len(),
            "settled": self.settled(), "elapsed_ms": elapsed_ms, "threshold_ms": thresholds,
            "images_per_second": self.ready() as f64 / (elapsed_ms / 1000.0),
            "source_bytes_read": profiles.iter().map(|p| p.stage_bytes[0]).sum::<u64>(),
            "source_read_attempts": profiles.iter().filter(|p| p.stage_ms[0] > 0.0).count(),
            "derived_bytes_written": profiles.iter().filter(|p| p.cache_entry_ready && !p.disk_hit).map(|p| p.stage_bytes[5]).sum::<u64>(),
            "derived_write_attempt_bytes": profiles.iter().map(|p| p.stage_bytes[5]).sum::<u64>(),
            "cache_bytes_read": profiles.iter().map(|p| p.stage_bytes[6]).sum::<u64>(),
            "cache_hits": profiles.iter().filter(|p| p.disk_hit).count(),
            "cancelled": profiles.iter().filter(|p| p.cancelled_after.is_some()).count(),
            "worker_active_ms": active_ms, "worker_utilization_fraction": active_ms / (elapsed_ms * workers as f64),
            "pending_peak": self.pending_peak, "cpu_payload_peak_bytes": self.cpu_peak,
            "progress": self.progress,
            "note": "Validated historical progress; no frame-path filesystem inventory. 100% ready differs from all settled. Persistence and cancellation may overlap. Derived bytes written means successfully committed PNG volume; attempt bytes are scheduled write sizes, not measured physical I/O. Active time covers observed 128 jobs only; no allocator scratch measurement.",
        })
    }
}
