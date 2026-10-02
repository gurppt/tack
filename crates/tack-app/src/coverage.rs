//! Visibility/LOD demand episodes; unresolved episodes remain explicitly censored.
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use tack_core::Lod;

#[derive(Serialize)]
pub struct Episode {
    pub asset_id: u32,
    pub requested_edge: u32,
    pub entered_ms: f64,
    pub first_image_delay_ms: Option<f64>,
    pub requested_delay_ms: Option<f64>,
    pub left_ms: Option<f64>,
}

#[derive(Default)]
pub struct Coverage {
    active: HashMap<u32, usize>,
    seen: HashSet<(u32, u32)>,
    pub episodes: Vec<Episode>,
    pub dropped: usize,
}

impl Coverage {
    pub fn observe(&mut self, visible: &[(u32, Lod, Option<Lod>)], elapsed_ms: f64) {
        self.seen.clear();
        self.seen
            .extend(visible.iter().map(|(id, wanted, _)| (*id, wanted.edge())));
        self.active.retain(|id, index| {
            let keep = self
                .seen
                .contains(&(*id, self.episodes[*index].requested_edge));
            if !keep {
                self.episodes[*index].left_ms = Some(elapsed_ms);
            }
            keep
        });
        for (id, wanted, shown) in visible {
            let index = if let Some(index) = self.active.get(id) {
                *index
            } else {
                if self.episodes.len() >= 20000 {
                    self.dropped += 1;
                    continue;
                }
                let index = self.episodes.len();
                self.episodes.push(Episode {
                    asset_id: *id,
                    requested_edge: wanted.edge(),
                    entered_ms: elapsed_ms,
                    first_image_delay_ms: None,
                    requested_delay_ms: None,
                    left_ms: None,
                });
                self.active.insert(*id, index);
                index
            };
            let episode = &mut self.episodes[index];
            if shown.is_some() {
                episode
                    .first_image_delay_ms
                    .get_or_insert(elapsed_ms - episode.entered_ms);
            }
            if shown.is_some_and(|lod| lod >= *wanted) {
                episode
                    .requested_delay_ms
                    .get_or_insert(elapsed_ms - episode.entered_ms);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unresolved_detail_remains_censored_when_demand_changes() {
        let mut coverage = Coverage::default();
        coverage.observe(&[(7, Lod::Detail, None)], 10.0);
        coverage.observe(&[(7, Lod::Detail, Some(Lod::Thumbnail))], 30.0);
        coverage.observe(&[(7, Lod::Medium, Some(Lod::Medium))], 50.0);
        coverage.observe(&[], 80.0);
        assert_eq!(coverage.episodes.len(), 2);
        let detail = &coverage.episodes[0];
        assert_eq!(detail.first_image_delay_ms, Some(20.0));
        assert_eq!(detail.requested_delay_ms, None);
        assert_eq!(detail.left_ms, Some(50.0));
        let medium = &coverage.episodes[1];
        assert_eq!(medium.requested_delay_ms, Some(0.0));
        assert_eq!(medium.left_ms, Some(80.0));
    }
}
