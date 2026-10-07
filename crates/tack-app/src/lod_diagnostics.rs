//! Opt-in single-source receipts. No owner/state exists in ordinary execution.
use serde_json::{Value, json};
use tack_assets::ProductAssets;
use tack_core::{Camera, Document, Lod, SourceId};
use tack_render::{DrawProductImage, Gpu, ProductKey};
#[derive(Default)]
pub struct LodDiagnostics {
    source: Option<SourceId>,
    camera: Option<([f64; 2], f64)>,
    epoch: u64,
}
impl LodDiagnostics {
    #[allow(clippy::too_many_arguments)]
    pub fn record(
        &mut self,
        camera: &Camera,
        draws: &[DrawProductImage],
        document: Option<&Document>,
        assets: Option<&ProductAssets>,
        gpu: &Gpu,
        overview_edge: u32,
    ) -> Option<Value> {
        let (document, assets) = (document?, assets?);
        let view = (
            camera.screen_to_world(camera.screen_size().map(|v| f64::from(v) / 2.)),
            camera.zoom(),
        );
        if self.camera != Some(view) {
            self.epoch += 1;
            self.camera = Some(view);
        }
        let first = draws.first()?;
        let source = *self
            .source
            .get_or_insert(document.asset(first.data.asset_id)?.source_id());
        let needs: Vec<_> = draws
            .iter()
            .filter_map(|draw| {
                let asset = document.asset(draw.data.asset_id)?;
                let uv = draw.data.crop.uv_rect();
                Some(crate::supply_plan::Need {
                    asset: asset.id(),
                    source: asset.source_id(),
                    projected: crate::supply_plan::projected_edge(
                        draw.data.transform,
                        camera.zoom(),
                    ),
                    source_edge: asset.pixel_size().into_iter().max().unwrap_or(0),
                    crop: [uv[2], uv[3]],
                })
            })
            .collect();
        let plan = crate::supply_plan::plan(
            &needs,
            if assets.limits().max_lod == Lod::Medium {
                16
            } else {
                128
            } * 1024
                * 1024,
            assets.limits().max_lod,
        );
        let draw = draws.iter().find(|d| {
            document
                .asset(d.data.asset_id)
                .is_some_and(|a| a.source_id() == source)
        })?;
        let s = document.source(source)?;
        let desired = plan.lods[&source];
        let tiers: Vec<_> = Lod::ALL.into_iter().filter_map(|lod| {
            let edge = if lod == Lod::Thumbnail {overview_edge} else {lod.edge()};
            let state = assets.rep_trace(draw.data.asset_id, s.revision(), lod, edge)?;
            let resident = gpu.contains_product(ProductKey {asset:(lod==Lod::Thumbnail).then_some(draw.data.asset_id), source, revision:s.revision(), lod, edge});
            let reason = if resident {"gpu-resident"} else if state.cpu_resident {"cpu-resident/upload-budget"}
                else if state.active_codec_worker.is_some() {"codec-active"} else if state.queued {"queued"}
                else if state.failed {"source-or-codec-failed"} else if !state.wanted {"not-current-demand"} else {"admissible/queue-cap-or-source-fairness"};
            Some(json!({"tier":format!("{lod:?}"), "edge":edge, "gpu_resident":resident,
                "cpu_cache_hit":state.cpu_resident,"pending":state.queued||state.active_codec_worker.is_some(),
                "active_codec_worker":state.active_codec_worker,"request_generation":state.request_generation,
                "publication":state.publication, "suppression_reason":reason}))
        }).collect();
        let size = draw.data.transform.size().map(|v| v * camera.zoom());
        Some(
            json!({"source":format!("{:032x}",source.value()),"asset":format!("{:032x}",draw.data.asset_id.value()),
            "revision":s.revision(),"camera_epoch":self.epoch,"projected_size":size,"projected_edge":size.into_iter().fold(0.,f64::max),
            "desired":format!("{desired:?}"),"displayed":draw.key.map(|k|format!("{:?}",k.lod)),
            "selection_reason":"smallest-adequate-resident-then-highest-fallback/post-upload",
            "ownership":"source/revision/tier, no camera-epoch ownership", "tiers":tiers}),
        )
    }
}
