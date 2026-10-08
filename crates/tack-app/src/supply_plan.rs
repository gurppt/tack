//! Stable visible-set admission. Quality degrades before residency can churn.
use std::collections::HashMap;
use tack_core::{AssetId, ImageFiltering, Lod, SourceId, Transform};
/// The longest displayed edge drives both refinement and preview urgency.
/// Portrait and landscape sources must have the same scheduling semantics.
pub fn projected_edge(transform: Transform, zoom: f64) -> f64 {
    transform.size().into_iter().fold(0., f64::max) * zoom
}
pub fn preview_priority(transform: Transform, zoom: f64) -> u8 {
    u8::from(projected_edge(transform, zoom) <= 128.)
}
#[derive(Clone, Copy)]
pub struct Need {
    pub asset: AssetId,
    pub source: SourceId,
    pub projected: f64,
    pub source_edge: u32,
    pub source_size: [u32; 2],
    pub filtering: ImageFiltering,
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
    let mut sources = HashMap::<SourceId, (f64, [u32; 2], bool)>::new();
    for n in needs {
        let projected = n.projected / n.crop[0].min(n.crop[1]).max(1e-9);
        let need = projected.min(f64::from(n.source_edge));
        // A filtered preview permanently discards pixel-art detail. Small native
        // references are cheap enough to retain their original pixels even below
        // the preview threshold; the preview remains a loading fallback.
        let native = n.filtering == ImageFiltering::Nearest && n.source_edge <= 512;
        sources
            .entry(n.source)
            .and_modify(|p| {
                p.0 = p.0.max(need);
                p.1 = std::array::from_fn(|axis| p.1[axis].max(n.source_size[axis]));
                p.2 |= native;
            })
            .or_insert((need, n.source_size, native));
    }
    let mut sources: Vec<_> = sources.into_iter().collect();
    sources.sort_by(|a, b| b.1.0.total_cmp(&a.1.0).then(a.0.cmp(&b.0)));
    let mut remaining = gpu_bytes / 2;
    let mut lods = HashMap::new();
    for (source, (projected, size, native)) in sources {
        let wanted = Lod::for_projected_edge(projected)
            .max(if native { Lod::Medium } else { Lod::Thumbnail })
            .min(max_lod);
        let admitted = [Lod::Detail, Lod::Medium]
            .into_iter()
            .filter(|l| *l <= wanted)
            .find(|l| {
                let cost = size
                    .into_iter()
                    .map(|axis| axis.min(l.edge()) as usize)
                    .product::<usize>()
                    * 4;
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
/// Choose the finest valid post-upload resident. Newly adequate lower tiers
/// must never replace useful resident detail merely because zoom crossed a
/// threshold. Residency is disposable; request/admission stays demand bounded.
pub fn displayed_lod(_desired: Lod, resident: impl Fn(Lod) -> bool) -> Option<Lod> {
    Lod::ALL.into_iter().rev().find(|lod| resident(*lod))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    #[test]
    fn portrait_and_landscape_overtake_small_previews_identically() {
        let images = [[100., 80.], [100., 2000.], [2000., 100.]]
            .map(|size| Transform::new([0.; 2], size, 0., [false; 2]).unwrap());
        let mut order = [0, 1, 2];
        order.sort_by_key(|i| preview_priority(images[*i], 1.));
        assert_eq!(order, [1, 2, 0]);
        assert_eq!(
            Lod::for_projected_edge(projected_edge(images[1], 1.)),
            Lod::Detail
        );
        assert_eq!(
            Lod::for_projected_edge(projected_edge(images[2], 1.)),
            Lod::Detail
        );
        assert_eq!(preview_priority(images[1], 0.01), 1);
    }
    fn needs(n: usize) -> Vec<Need> {
        (0..n)
            .map(|i| Need {
                asset: AssetId::new(i as u128 + 1).unwrap(),
                source: SourceId::new(i as u128 + 1).unwrap(),
                projected: 2000.,
                source_edge: 4096,
                source_size: [4096; 2],
                filtering: ImageFiltering::Smooth,
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
    fn shared_source_cost_covers_all_aliases_and_actual_resident_pixels() {
        let mut n = needs(2);
        n[1].source = n[0].source;
        n[0].source_edge = 96;
        n[0].source_size = [96; 2];
        for aliases in [&n[..], &[n[1], n[0]][..]] {
            let p = plan(aliases, 128 * 1024 * 1024, Lod::Detail);
            assert_eq!(p.detail_reserved, 2048 * 2048 * 4);
        }
        // The large alias can leave the view while its larger shared resident
        // remains. Cost uses actual cached dimensions, independently of demand.
        n[0].source_size = [512, 341];
        n[0].filtering = ImageFiltering::Nearest;
        let p = plan(&n[..1], 128 * 1024 * 1024, Lod::Detail);
        assert_eq!(p.detail_reserved, 512 * 341 * 4);
        assert_eq!(p.lods[&n[0].source], Lod::Medium);
    }
    #[test]
    fn small_source_never_requests_useless_full_original_quality() {
        let mut n = needs(1);
        n[0].source_edge = 96;
        n[0].source_size = [96; 2];
        assert_eq!(
            plan(&n, 128 * 1024 * 1024, Lod::Detail).lods[&n[0].source],
            Lod::Thumbnail
        );
    }
}

#[cfg(test)]
mod convergence_tests {
    use super::*;
    #[test]
    fn resident_detail_bridges_unloaded_medium_on_monotone_dezoom() {
        let resident = |lod| lod != Lod::Medium;
        for edge in [2048., 513., 512., 500., 129., 128., 32.] {
            let desired = Lod::for_projected_edge(edge);
            let displayed = displayed_lod(desired, resident);
            assert!(
                displayed.is_some_and(|lod| lod >= desired),
                "under-resolved at {edge}px: desired {desired:?}, displayed {displayed:?}"
            );
        }
    }
    #[test]
    fn finest_resident_wins_and_missing_detail_keeps_fallback() {
        assert_eq!(displayed_lod(Lod::Medium, |_| true), Some(Lod::Detail));
        assert_eq!(
            displayed_lod(Lod::Detail, |l| l != Lod::Detail),
            Some(Lod::Medium)
        );
        assert_eq!(displayed_lod(Lod::Thumbnail, |_| false), None);
    }
    #[test]
    fn single_source_admission_has_no_quality_holes_in_either_direction()
    -> Result<(), tack_assets::AssetError> {
        let mut n = Need {
            asset: AssetId::new(1)?,
            source: SourceId::new(2)?,
            projected: 0.,
            source_edge: 4096,
            source_size: [4096; 2],
            filtering: ImageFiltering::Smooth,
            crop: [1.; 2],
        };
        let mut previous = Lod::Thumbnail;
        for edge in 1..=4096 {
            n.projected = f64::from(edge);
            let current = plan(&[n], 128 * 1024 * 1024, Lod::Detail).lods[&n.source];
            assert!(current >= previous);
            previous = current;
        }
        for edge in (1..=4096).rev() {
            n.projected = f64::from(edge);
            let current = plan(&[n], 128 * 1024 * 1024, Lod::Detail).lods[&n.source];
            assert!(current <= previous);
            previous = current;
        }
        Ok(())
    }
}

#[cfg(test)]
mod pixel_art_tests {
    use super::*;
    #[test]
    fn native_pixel_art_admission_is_history_independent_and_cheap()
    -> Result<(), tack_assets::AssetError> {
        let mut needs = (1..=1000)
            .map(|id| {
                Ok(Need {
                    asset: AssetId::new(id)?,
                    source: SourceId::new(id)?,
                    projected: 16.,
                    source_edge: 96,
                    source_size: [96, 64],
                    filtering: ImageFiltering::Nearest,
                    crop: [1.; 2],
                })
            })
            .collect::<Result<Vec<_>, tack_assets::AssetError>>()?;
        let budget = 128 * 1024 * 1024;
        for edge in [127., 128., 129., 513., 512., 80., 16.] {
            needs[0].projected = edge;
            let p = plan(&needs, budget, Lod::Detail);
            assert!(p.lods.values().all(|tier| *tier == Lod::Medium));
            assert_eq!(p.detail_reserved, 1000 * 96 * 64 * 4);
        }
        Ok(())
    }
    #[test]
    fn smallest_adequate_label_reproduces_the_quality_valley() {
        // Existing 1K policy selects a filtered preview below 129px even while
        // valid native pixels remain resident. Nearest cannot undo that filter.
        let old = |desired| Lod::ALL.into_iter().find(|lod| *lod >= desired);
        assert_eq!(old(Lod::Thumbnail), Some(Lod::Thumbnail));
        for edge in [80., 128., 129., 512., 513., 512., 128., 80.] {
            assert_eq!(
                displayed_lod(Lod::for_projected_edge(edge), |_| true),
                Some(Lod::Detail)
            );
        }
    }
}
