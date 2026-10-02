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
}

impl JobProfile {
    pub(crate) fn measure<T>(&mut self, stage: Stage, operation: impl FnOnce() -> T) -> T {
        let start = Instant::now();
        let result = operation();
        self.stage_ms[stage as usize] += start.elapsed().as_secs_f64() * 1000.0;
        result
    }

    pub(crate) fn bytes(&mut self, stage: Stage, bytes: usize) {
        self.stage_bytes[stage as usize] += bytes as u64;
    }
}
