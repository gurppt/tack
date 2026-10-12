//! Presentation admission only. Document, history and persistence remain complete.
use std::{
    cmp::{Ordering, Reverse},
    collections::BinaryHeap,
};
use tack_core::ObjectId;
use tack_render::{DrawProductImage, MAX_IMAGE_QUADS};

struct Ranked {
    rank: (bool, usize),
    draw: DrawProductImage,
}
impl PartialEq for Ranked {
    fn eq(&self, other: &Self) -> bool {
        self.rank == other.rank
    }
}
impl Eq for Ranked {}
impl PartialOrd for Ranked {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Ranked {
    fn cmp(&self, other: &Self) -> Ordering {
        self.rank.cmp(&other.rank)
    }
}

#[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limit {
    pub visible: usize,
    pub omitted: usize,
    pub selected_omitted: usize,
}
impl Limit {
    pub fn warning(self, tile_limited: bool) -> Option<String> {
        if self.omitted > 0 {
            Some(format!(
                "Display limit: {} of {} images shown; {} omitted ({} selected). Board intact.",
                self.visible - self.omitted,
                self.visible,
                self.omitted,
                self.selected_omitted
            ))
        } else if tile_limited {
            Some("Display limit: tile detail reduced. Board intact.".into())
        } else {
            None
        }
    }
}

/// Input must follow document z-order. Normal frames reuse the caller's Vec;
/// only saturation allocates a bounded heap. Selected visible images win first,
/// then frontmost images. Admission never changes the surviving relative order.
pub struct Images<'a> {
    draws: &'a mut Vec<DrawProductImage>,
    heap: Option<BinaryHeap<Reverse<Ranked>>>,
    visible: usize,
    selected: usize,
}
impl<'a> Images<'a> {
    pub fn new(draws: &'a mut Vec<DrawProductImage>) -> Self {
        draws.clear();
        Self {
            draws,
            heap: None,
            visible: 0,
            selected: 0,
        }
    }
    pub fn push(&mut self, draw: DrawProductImage, selected: impl Fn(ObjectId) -> bool) {
        let is_selected = selected(draw.data.object_id);
        self.selected += usize::from(is_selected);
        let rank = (is_selected, self.visible);
        self.visible += 1;
        if self.heap.is_none() && self.draws.len() < MAX_IMAGE_QUADS {
            self.draws.push(draw);
            return;
        }
        let heap = self.heap.get_or_insert_with(|| {
            let mut heap = BinaryHeap::with_capacity(MAX_IMAGE_QUADS);
            heap.extend(self.draws.drain(..).enumerate().map(|(i, draw)| {
                Reverse(Ranked {
                    rank: (selected(draw.data.object_id), i),
                    draw,
                })
            }));
            heap
        });
        if let Some(mut lowest) = heap.peek_mut()
            && rank > lowest.0.rank
        {
            *lowest = Reverse(Ranked { rank, draw });
        }
    }
    pub fn finish(mut self) -> Limit {
        let mut selected_omitted = 0;
        if let Some(heap) = self.heap.take() {
            let mut admitted = heap.into_vec();
            admitted.sort_unstable_by_key(|r| r.0.rank.1);
            selected_omitted = self.selected - admitted.iter().filter(|r| r.0.rank.0).count();
            self.draws.extend(admitted.into_iter().map(|r| r.0.draw));
        }
        Limit {
            visible: self.visible,
            omitted: self.visible - self.draws.len(),
            selected_omitted,
        }
    }
}

/// Reserve one fallback quad for every remaining logical image. A tile group is
/// atomic: partial coverage is never substituted for an overview.
pub fn tile_group_fits(written: usize, group: usize, remaining: usize) -> bool {
    written
        .checked_add(group)
        .and_then(|v| v.checked_add(remaining))
        .is_some_and(|v| v <= MAX_IMAGE_QUADS)
}
