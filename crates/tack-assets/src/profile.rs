//! Bounded worker-stage evidence; this is telemetry, not a frame-time profiler.
use serde::Serialize;
use std::time::Instant;

#[derive(Clone, Copy)]
pub(crate) enum Stage {
    SourceRead,
    Header,
    Decode,
    Resize,
    Encode,
    CacheWrite,
    CacheRead,
    CacheMaintenance,
    QueueWait,
    CpuInsert,
    Retire,
}

pub const STAGE_NAMES: [&str; 11] = [
    "source_read",
    "header",
    "decode",
    "resize",
    "encode",
    "cache_write",
    "cache_read",
    "cache_maintenance",
    "queue_wait",
    "cpu_insert",
    "retire",
];

#[derive(Default, Serialize)]
pub struct JobProfile {
    pub asset_id: u32,
    pub edge: u32,
    pub disk_hit: bool,
    pub active_ms: f64,
    pub stage_ms: [f64; 11],
    pub stage_bytes: [u64; 11],
    pub cancelled_after: Option<&'static str>,
    pub encoded_peak_bytes: usize,
    pub decoded_peak_bytes: usize,
    pub resize_peak_bytes: usize,
    /// Validated old PNG or newly renamed PNG, even if the CPU result is stale.
    pub cache_entry_ready: bool,
    pub failed: bool,
    /// Common clock across workers: milliseconds since Loader construction.
    pub decode_started_ms: Option<f64>,
    pub decode_finished_ms: Option<f64>,
    #[serde(skip)]
    pub(crate) epoch: Option<Instant>,
}

impl JobProfile {
    pub(crate) fn measure<T>(&mut self, stage: Stage, operation: impl FnOnce() -> T) -> T {
        let start = Instant::now();
        let result = operation();
        let elapsed = start.elapsed().as_secs_f64() * 1000.0;
        self.stage_ms[stage as usize] += elapsed;
        if matches!(stage, Stage::Decode)
            && let Some(epoch) = self.epoch
        {
            let started_ms = start.duration_since(epoch).as_secs_f64() * 1000.0;
            self.decode_started_ms = Some(started_ms);
            self.decode_finished_ms = Some(started_ms + elapsed);
        }
        result
    }

    pub(crate) fn bytes(&mut self, stage: Stage, bytes: usize) {
        self.stage_bytes[stage as usize] += bytes as u64;
    }
}
