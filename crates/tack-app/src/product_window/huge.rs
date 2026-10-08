//! Bounded JPEG tiles and opt-in PNG tiles. Fallback remains until admitted tiles converge.
use super::*;
use tack_assets::huge_image::{self, TILE_EDGE, Tile};
use tack_core::{Crop, ImageRenderData, Transform};
#[derive(Clone, Copy)]
struct Region {
    index: usize,
    draw: DrawProductImage,
}
pub(super) struct Tiles {
    regions: Vec<Region>,
    pub requested: usize,
    pub ready: usize,
}
pub(super) struct TilePolicy {
    pub reserved: usize,
    pub png: bool,
}
impl Tiles {
    pub fn plan(
        draws: &[DrawProductImage],
        camera: &Camera,
        doc: &tack_core::Document,
        assets: &ProductAssets,
        gpu: &Gpu,
        policy: TilePolicy,
        demands: &mut Vec<ProductDemand>,
    ) -> Self {
        let TilePolicy {
            reserved,
            png: png_opt_in,
        } = policy;
        let budget = if assets.limits().workers == 1 {
            8 * 1024 * 1024usize
        } else {
            64 * 1024 * 1024
        };
        let mut slots = budget
            .saturating_sub(reserved)
            .min(assets.limits().cpu_bytes / 4)
            / (TILE_EDGE as usize + 2).pow(2)
            / 4;
        slots = slots.min(64).min(10000usize.saturating_sub(draws.len()));
        let mut eligible = draws
            .iter()
            .filter(|draw| {
                doc.asset(draw.data.asset_id)
                    .and_then(|a| doc.source(a.source_id()))
                    .is_some_and(|s| eligible_source(assets, s.id(), s.revision(), png_opt_in))
                    && detail_density(draw.data, camera) > f64::from(assets.limits().max_lod.edge())
            })
            .count();
        let mut result = Self {
            regions: Vec::new(),
            requested: 0,
            ready: 0,
        };
        for (index, draw) in draws.iter().enumerate() {
            if slots == 0 {
                break;
            }
            let Some(asset) = doc.asset(draw.data.asset_id) else {
                continue;
            };
            let Some(source) = doc.source(asset.source_id()) else {
                continue;
            };
            let size = asset.pixel_size();
            let density = detail_density(draw.data, camera);
            if !eligible_source(assets, source.id(), source.revision(), png_opt_in)
                || density <= f64::from(assets.limits().max_lod.edge())
            {
                continue;
            }
            let quota = slots / eligible.max(1);
            eligible = eligible.saturating_sub(1);
            if quota == 0 {
                continue;
            }
            let Ok(mut mip) = huge_image::mip_for_density(size, density) else {
                continue;
            };
            let bounds = visible_uv(draw.data, camera);
            let mut selected = Vec::new();
            while mip <= 31 {
                selected = regions(draw.data, size, bounds, mip, quota);
                if !selected.is_empty() {
                    break;
                }
                mip += 1;
            }
            for (tile, mut data) in selected {
                if assets.supports_jpeg_tiles(source.id(), source.revision()) {
                    let Ok(dimensions) = tile.dimensions(size) else {
                        continue;
                    };
                    let uv = data.crop.uv_rect();
                    let padded = dimensions.map(|axis| f64::from(axis + 2));
                    let Ok(crop) = Crop::new(
                        (1. + uv[0] * f64::from(dimensions[0])) / padded[0],
                        (1. + uv[1] * f64::from(dimensions[1])) / padded[1],
                        uv[2] * f64::from(dimensions[0]) / padded[0],
                        uv[3] * f64::from(dimensions[1]) / padded[1],
                    ) else {
                        continue;
                    };
                    data.crop = crop;
                }
                let Ok(edge) = tile.tag() else {
                    continue;
                };
                let lod = assets.limits().max_lod;
                let key = ProductKey {
                    asset: None,
                    source: source.id(),
                    revision: source.revision(),
                    lod,
                    edge,
                };
                demands.push(ProductDemand {
                    asset: asset.id(),
                    lod,
                    edge,
                    priority: 2,
                    resident: gpu.contains_product(key),
                });
                result.regions.push(Region {
                    index,
                    draw: DrawProductImage {
                        data,
                        key: Some(key),
                    },
                });
                slots -= 1;
            }
        }
        result.requested = result.regions.len();
        result
    }
    pub fn upload(&self, assets: &mut ProductAssets, gpu: &mut Gpu) {
        for region in &self.regions {
            let Some(key) = region.draw.key else { continue };
            if !gpu.contains_product(key)
                && let Some(image) =
                    assets.get_rep(region.draw.data.asset_id, key.revision, key.lod, key.edge)
            {
                gpu.upload_product(key, image);
            }
        }
    }
    pub fn display(
        &mut self,
        draws: &mut Vec<DrawProductImage>,
        assets: &ProductAssets,
        gpu: &Gpu,
    ) -> bool {
        if self.regions.is_empty() {
            return false;
        }
        self.ready = self
            .regions
            .iter()
            .filter(|r| r.draw.key.is_some_and(|k| gpu.contains_product(k)))
            .count();
        let mut pending = false;
        let mut output = Vec::with_capacity(draws.len() + self.regions.len());
        let mut first = 0;
        for (index, draw) in draws.iter().enumerate() {
            let end = first
                + self.regions[first..]
                    .iter()
                    .take_while(|r| r.index == index)
                    .count();
            let group = &self.regions[first..end];
            if !group.is_empty()
                && group
                    .iter()
                    .all(|r| r.draw.key.is_some_and(|k| gpu.contains_product(k)))
            {
                output.extend(group.iter().map(|r| r.draw));
            } else {
                output.push(*draw);
                pending |= group.iter().any(|r| {
                    r.draw.key.is_some_and(|k| {
                        !gpu.contains_product(k)
                            && !assets.failed_rep(r.draw.data.asset_id, k.lod, k.edge)
                    })
                });
            }
            first = end;
        }
        *draws = output;
        pending
    }
}
fn eligible_source(
    assets: &ProductAssets,
    source: tack_core::SourceId,
    revision: u64,
    png_opt_in: bool,
) -> bool {
    assets.supports_jpeg_tiles(source, revision)
        || (png_opt_in && assets.supports_tiles(source, revision))
}
fn detail_density(data: ImageRenderData, camera: &Camera) -> f64 {
    let crop = data.crop.uv_rect();
    tack_app::supply_plan::projected_edge(data.transform, camera.zoom())
        / crop[2].min(crop[3]).max(1e-9)
}
fn visible_uv(data: ImageRenderData, camera: &Camera) -> [f64; 4] {
    let view = camera.viewport();
    let crop = data.crop.uv_rect();
    let mut bounds = [1f64, 1., 0., 0.];
    for p in [
        [view.x, view.y],
        [view.x + view.width, view.y],
        [view.x, view.y + view.height],
        [view.x + view.width, view.y + view.height],
    ] {
        let local = tack_app::image_geometry::local(data.transform, p);
        for axis in 0..2 {
            let mut fraction = (local[axis] / data.transform.size()[axis] + 0.5).clamp(0., 1.);
            if data.transform.flips()[axis] {
                fraction = 1. - fraction;
            }
            let uv = crop[axis] + fraction * crop[axis + 2];
            bounds[axis] = bounds[axis].min(uv);
            bounds[axis + 2] = bounds[axis + 2].max(uv);
        }
    }
    bounds
}
fn regions(
    data: ImageRenderData,
    size: [u32; 2],
    bounds: [f64; 4],
    mip: u8,
    limit: usize,
) -> Vec<(Tile, ImageRenderData)> {
    let Ok(ms) = huge_image::mip_dimensions(size, mip) else {
        return Vec::new();
    };
    let scale = 2f64.powi(i32::from(mip));
    let range = |axis: usize| {
        let first = (bounds[axis] * f64::from(size[axis]) / scale / f64::from(TILE_EDGE))
            .floor()
            .max(0.) as u32;
        let last = (bounds[axis + 2] * f64::from(size[axis]) / scale / f64::from(TILE_EDGE))
            .ceil()
            .max(f64::from(first + 1)) as u32;
        first..last.min(ms[axis].div_ceil(TILE_EDGE))
    };
    let xr = range(0);
    let yr = range(1);
    if (xr.len() as u64) * (yr.len() as u64) > limit as u64 {
        return Vec::new();
    }
    let mut output = Vec::new();
    for y in yr {
        for x in xr.clone() {
            let tile = Tile { mip, x, y };
            if let Some(data) = tile_draw(data, size, tile) {
                output.push((tile, data));
            }
        }
    }
    output
}
fn tile_draw(mut data: ImageRenderData, size: [u32; 2], tile: Tile) -> Option<ImageRenderData> {
    let dim = tile.dimensions(size).ok()?;
    let scale = 2f64.powi(i32::from(tile.mip));
    let origin = [
        f64::from(tile.x) * f64::from(TILE_EDGE) * scale,
        f64::from(tile.y) * f64::from(TILE_EDGE) * scale,
    ];
    let span = dim.map(|v| f64::from(v) * scale);
    let crop = data.crop.uv_rect();
    let mut lo = [0.; 2];
    let mut hi = [0.; 2];
    let mut uv = [0.; 4];
    for axis in 0..2 {
        lo[axis] = origin[axis].max(crop[axis] * f64::from(size[axis]));
        hi[axis] = (origin[axis] + span[axis])
            .min((crop[axis] + crop[axis + 2]) * f64::from(size[axis]))
            .min(f64::from(size[axis]));
        if hi[axis] <= lo[axis] {
            return None;
        }
        uv[axis] = (lo[axis] - origin[axis]) / span[axis];
        uv[axis + 2] = (hi[axis] - lo[axis]) / span[axis];
    }
    let old = data.transform;
    let mut local = [0.; 2];
    let mut dimensions = [0.; 2];
    for axis in 0..2 {
        let middle = (lo[axis] + hi[axis]) / 2. / f64::from(size[axis]);
        let fraction = (middle - crop[axis]) / crop[axis + 2];
        local[axis] =
            (fraction - 0.5) * old.size()[axis] * if old.flips()[axis] { -1. } else { 1. };
        dimensions[axis] =
            (hi[axis] - lo[axis]) / f64::from(size[axis]) / crop[axis + 2] * old.size()[axis];
    }
    data.transform = Transform::new(
        tack_app::image_geometry::world(old, local),
        dimensions,
        old.rotation(),
        old.flips(),
    )
    .ok()?;
    data.crop = Crop::new(uv[0], uv[1], uv[2], uv[3]).ok()?;
    Some(data)
}
#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    fn data(flips: [bool; 2], angle: f64, crop: Crop) -> ImageRenderData {
        ImageRenderData {
            object_id: tack_core::ObjectId::new(1).unwrap(),
            asset_id: tack_core::AssetId::new(2).unwrap(),
            transform: Transform::new([0.; 2], [5000., 1000.], angle, flips).unwrap(),
            crop,
            opacity: tack_core::Opacity::OPAQUE,
            filtering: tack_core::ImageFiltering::Smooth,
        }
    }
    #[test]
    fn cropped_huge_source_demands_tiles_below_uncropped_tier_threshold() {
        let mut d = data([false; 2], 0., Crop::new(0.4, 0., 0.005, 1.).unwrap());
        d.transform = Transform::new([0.; 2], [800., 400.], 0., [false; 2]).unwrap();
        let camera = Camera::new([800, 600]);
        assert_eq!(
            tack_app::supply_plan::projected_edge(d.transform, camera.zoom()),
            800.
        );
        assert!(detail_density(d, &camera) > 2048.);
        let tiles = regions(d, [50000, 512], visible_uv(d, &camera), 0, 64);
        assert!(!tiles.is_empty());
        assert!(tiles.iter().all(|(tile, _)| tile.x >= 78 && tile.x <= 79));
    }
    #[test]
    fn visible_tiles_bound_admission_and_preserve_flipped_rotated_crop() {
        let size = [50000, 10000];
        let crop = Crop::new(0.2, 0.1, 0.6, 0.8).unwrap();
        for flips in [[false; 2], [true, false], [false, true], [true; 2]] {
            let d = data(flips, 0.43, crop);
            let camera = Camera::new([800, 600]);
            let uv = visible_uv(d, &camera);
            assert!(uv[0] >= 0.2 && uv[1] >= 0.1 && uv[2] <= 0.8 && uv[3] <= 0.9);
            let mut mip = 0;
            let admitted = loop {
                let r = regions(d, size, uv, mip, 8);
                if !r.is_empty() {
                    break r;
                }
                mip += 1;
                assert!(mip < 31);
            };
            assert!(admitted.len() <= 8);
            for (tile, draw) in admitted {
                let center = tack_app::image_geometry::local(d.transform, draw.transform.center());
                let tile_size = tile.dimensions(size).unwrap();
                for axis in 0..2 {
                    let fraction = 0.5 + center[axis] / d.transform.size()[axis];
                    let fraction = if flips[axis] { 1. - fraction } else { fraction };
                    let source = (crop.uv_rect()[axis] + fraction * crop.uv_rect()[axis + 2])
                        * f64::from(size[axis]);
                    let sample = (f64::from(if axis == 0 { tile.x } else { tile.y }) * 256.
                        + f64::from(tile_size[axis]) / 2.)
                        * 2f64.powi(i32::from(mip));
                    // Interior tiles map exactly; border tiles are clipped to author crop.
                    assert!(
                        (source - sample).abs()
                            <= f64::from(tile_size[axis]) * 2f64.powi(i32::from(mip)) / 2. + 1.
                    );
                }
                assert_eq!(draw.transform.rotation(), d.transform.rotation());
                assert_eq!(draw.transform.flips(), flips);
                assert_eq!(draw.opacity, d.opacity);
            }
        }
    }
    #[test]
    fn partial_last_tile_keeps_source_boundary_without_stretching() {
        let d = data([false; 2], 0., Crop::FULL);
        let tile = Tile {
            mip: 0,
            x: 195,
            y: 39,
        };
        let draw = tile_draw(d, [50000, 10000], tile).unwrap();
        assert_eq!(tile.dimensions([50000, 10000]).unwrap(), [80, 16]);
        assert!((draw.transform.size()[0] - 8.).abs() < 1e-9);
        assert!((draw.transform.size()[1] - 1.6).abs() < 1e-9);
        assert!(
            tile_draw(
                d,
                [50000, 10000],
                Tile {
                    mip: 0,
                    x: 196,
                    y: 39
                }
            )
            .is_none()
        );
    }
}
