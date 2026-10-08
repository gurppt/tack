//! Explicit diagnostics only: bounded by active workers, no I/O or timers.
use super::*;
#[derive(Default)]
pub(super) struct Trace {
    next: u64,
    active: HashMap<Key, u64>,
    last: std::collections::VecDeque<(Key, u64, bool)>,
}
impl Trace {
    pub(super) fn started(&mut self, key: Key) {
        self.next += 1;
        self.active.insert(key, self.next);
    }
    pub(super) fn completed(&mut self, key: Key, accepted: bool) {
        if self.last.len() == 16 {
            self.last.pop_front();
        }
        self.last
            .push_back((key, self.active.remove(&key).unwrap_or(0), accepted));
    }
}
#[derive(Debug)]
pub struct RepTrace {
    pub lod: Lod,
    pub edge: u32,
    pub cpu_resident: bool,
    pub pixel_size: Option<[u32; 2]>,
    pub active_codec_worker: Option<usize>,
    pub queued: bool,
    pub wanted: bool,
    pub failed: bool,
    pub request_generation: Option<u64>,
    pub publication: Option<(u64, bool)>,
}
impl ProductAssets {
    pub fn enable_lod_diagnostics(&mut self) {
        self.trace = Some(Box::default());
    }
    pub fn rep_trace(
        &self,
        asset: AssetId,
        revision: u64,
        lod: Lod,
        edge: u32,
    ) -> Option<RepTrace> {
        let trace = self.trace.as_ref()?;
        let key = Key {
            asset: (lod == Lod::Thumbnail).then_some(asset),
            source: *self.sources.get(&asset)?,
            revision,
            lod,
            edge,
        };
        Some(RepTrace {
            lod,
            edge,
            cpu_resident: self.cached(key),
            pixel_size: (if lod == Lod::Thumbnail {
                &self.cache
            } else {
                &self.details
            })
            .peek(key)
            .map(|image| [image.width, image.height]),
            active_codec_worker: self.pending.get(&key).copied(),
            queued: self.queue.iter().any(|j| j.key() == key),
            wanted: self.wanted.contains(&key),
            failed: self.failed.contains(&key),
            request_generation: trace.active.get(&key).copied(),
            publication: trace
                .last
                .iter()
                .rev()
                .find(|(k, _, _)| *k == key)
                .map(|(_, g, accepted)| (*g, *accepted)),
        })
    }
}
