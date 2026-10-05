//! Stable visible-set admission. Quality degrades before residency can churn.
use std::collections::HashMap;
use tack_core::{AssetId, Lod, SourceId};
#[derive(Clone, Copy)]
pub struct Need {
    pub asset: AssetId,
    pub source: SourceId,
    pub projected: f64,
    pub source_edge: u32,
    pub crop: [f64; 2],
}
pub struct Plan {
    pub overview_edge: u32,
    pub lods: HashMap<SourceId, Lod>,
    pub detail_reserved: usize,
}
pub fn plan(needs: &[Need], gpu_bytes: usize, max_lod: Lod) -> Plan {
    let overview_budget = gpu_bytes - gpu_bytes / 2;
    // Reserve sixteen near-view previews; use a conservative square RGBA cost.
    let mut edge = 128;
    while edge > 8 && (needs.len() + 16) * edge * edge * 4 > overview_budget {
        edge /= 2;
    }
    let mut sources = HashMap::<SourceId, (f64, u32)>::new();
    for n in needs {
        let projected = n.projected / n.crop[0].min(n.crop[1]).max(1e-9);
        let need = projected.min(f64::from(n.source_edge));
        sources
            .entry(n.source)
            .and_modify(|p| p.0 = p.0.max(need))
            .or_insert((need, n.source_edge));
    }
    let mut sources: Vec<_> = sources.into_iter().collect();
    sources.sort_by(|a, b| b.1.0.total_cmp(&a.1.0).then(a.0.cmp(&b.0)));
    let mut remaining = gpu_bytes / 2;
    let mut lods = HashMap::new();
    for (source, (projected, _)) in sources {
        let wanted = Lod::for_projected_edge(projected).min(max_lod);
        let admitted = [Lod::Detail, Lod::Medium]
            .into_iter()
            .filter(|l| *l <= wanted)
            .find(|l| {
                let cost = (l.edge() as usize).pow(2) * 4;
                if cost <= remaining {
                    remaining -= cost;
                    true
                } else {
                    false
                }
            })
            .unwrap_or(Lod::Thumbnail);
        lods.insert(source, admitted);
    }
    Plan {
        overview_edge: edge as u32,
        lods,
        detail_reserved: gpu_bytes / 2 - remaining,
    }
}
#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    fn needs(n: usize) -> Vec<Need> {
        (0..n)
            .map(|i| Need {
                asset: AssetId::new(i as u128 + 1).unwrap(),
                source: SourceId::new(i as u128 + 1).unwrap(),
                projected: 2000.,
                source_edge: 4096,
                crop: [1.; 2],
            })
            .collect()
    }
    #[test]
    fn oversubscribed_view_has_stable_bounded_admission_and_small_previews() {
        let n = needs(10000);
        let a = plan(&n, 16 * 1024 * 1024, Lod::Medium);
        let b = plan(&n, 16 * 1024 * 1024, Lod::Medium);
        assert_eq!(a.lods, b.lods);
        assert_eq!(a.overview_edge, 8);
        assert!(a.detail_reserved <= 8 * 1024 * 1024);
        assert!((n.len() + 16) * (a.overview_edge as usize).pow(2) * 4 <= 8 * 1024 * 1024);
    }
    #[test]
    fn crop_need_and_shared_sources_are_counted_once() {
        let mut n = needs(2);
        n[0].projected = 100.;
        n[0].crop = [0.1, 0.1];
        n[1] = n[0];
        let p = plan(&n, 128 * 1024 * 1024, Lod::Detail);
        assert_eq!(p.lods.len(), 1);
        assert_eq!(p.lods[&n[0].source], Lod::Detail);
        assert_eq!(p.detail_reserved, 2048 * 2048 * 4);
    }
    #[test]
    fn small_source_never_requests_useless_full_original_quality() {
        let mut n = needs(1);
        n[0].source_edge = 96;
        assert_eq!(
            plan(&n, 128 * 1024 * 1024, Lod::Detail).lods[&n[0].source],
            Lod::Thumbnail
        );
    }
}
