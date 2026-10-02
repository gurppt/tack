use std::sync::{
    Arc,
    atomic::{AtomicU8, Ordering},
};

const SLOTS: usize = 3;
const MAX_SAMPLES: usize = 7200;

#[derive(Clone, Copy)]
pub struct GpuSample {
    pub submission: usize,
    pub pass_ms: f64,
}

struct Slot {
    resolved: wgpu::Buffer,
    readback: wgpu::Buffer,
    ready: Arc<AtomicU8>,
    pending: bool,
    submission: usize,
}

/// Three fixed readback slots; a busy slot drops a measurement, never stalls a frame.
pub(crate) struct Timings {
    queries: wgpu::QuerySet,
    slots: [Slot; SLOTS],
    period_ns: f64,
    samples: Vec<GpuSample>,
    pub dropped: usize,
}

impl Timings {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        Self {
            queries: device.create_query_set(&wgpu::QuerySetDescriptor {
                label: Some("canvas timestamps"),
                ty: wgpu::QueryType::Timestamp,
                count: (SLOTS * 2) as u32,
            }),
            slots: std::array::from_fn(|_| Slot {
                resolved: device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("timestamp resolve"),
                    size: 256,
                    usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                    mapped_at_creation: false,
                }),
                readback: device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("timestamp readback"),
                    size: 16,
                    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                    mapped_at_creation: false,
                }),
                ready: Arc::new(AtomicU8::new(0)),
                pending: false,
                submission: 0,
            }),
            period_ns: f64::from(queue.get_timestamp_period()),
            samples: Vec::with_capacity(MAX_SAMPLES),
            dropped: 0,
        }
    }

    pub fn poll(&mut self) {
        for slot in &mut self.slots {
            let ready = slot.ready.load(Ordering::Acquire);
            if !slot.pending || ready == 0 {
                continue;
            }
            if ready == 1 {
                let bytes = slot.readback.slice(..).get_mapped_range();
                let ticks: &[u64] = bytemuck::cast_slice(&bytes);
                if ticks[1] >= ticks[0] && self.samples.len() < MAX_SAMPLES {
                    self.samples.push(GpuSample {
                        submission: slot.submission,
                        pass_ms: (ticks[1] - ticks[0]) as f64 * self.period_ns / 1_000_000.0,
                    });
                } else {
                    self.dropped += 1;
                }
                drop(bytes);
                slot.readback.unmap();
            } else {
                self.dropped += 1;
                tracing::warn!("GPU timestamp readback failed");
            }
            slot.pending = false;
            slot.ready.store(0, Ordering::Release);
        }
    }

    pub fn reserve(&mut self, submission: usize) -> Option<usize> {
        let index = self.slots.iter().position(|slot| !slot.pending);
        if let Some(index) = index {
            self.slots[index].pending = true;
            self.slots[index].submission = submission;
        } else {
            self.dropped += 1;
        }
        index
    }

    pub fn writes(&self, index: usize) -> wgpu::RenderPassTimestampWrites<'_> {
        wgpu::RenderPassTimestampWrites {
            query_set: &self.queries,
            beginning_of_pass_write_index: Some((index * 2) as u32),
            end_of_pass_write_index: Some((index * 2 + 1) as u32),
        }
    }

    pub fn resolve(&self, encoder: &mut wgpu::CommandEncoder, index: usize) {
        let slot = &self.slots[index];
        let first = (index * 2) as u32;
        encoder.resolve_query_set(&self.queries, first..first + 2, &slot.resolved, 0);
        encoder.copy_buffer_to_buffer(&slot.resolved, 0, &slot.readback, 0, 16);
    }

    pub fn map(&self, index: usize) {
        let slot = &self.slots[index];
        let ready = Arc::clone(&slot.ready);
        slot.readback
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                ready.store(if result.is_ok() { 1 } else { 2 }, Ordering::Release);
            });
    }

    pub fn samples(&self) -> &[GpuSample] {
        &self.samples
    }

    pub fn pending(&self) -> bool {
        self.slots.iter().any(|slot| slot.pending)
    }
}
