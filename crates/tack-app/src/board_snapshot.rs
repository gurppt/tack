//! Finite offscreen export through the production renderer. Bounds precede all raster allocation.
use crate::{
    annotation_scene::AnnotationScene,
    annotation_tool::AnnotationInput,
    image_interaction::{ImageInteraction, SelectionState},
    image_save::Originals,
    native_files::{self, Picker},
    preferences::Preferences,
    source_export::{OriginalExport, copy_atomic},
};
use std::{
    collections::HashMap,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};
use tack_assets::{AssetError, Decoded};
use tack_core::{Camera, Document, DocumentQuery, Lod, ObjectKind, WorldRect};
use tack_render::{DrawProductImage, Gpu, ProductKey};
use tack_storage::{Payload, TackFile};
mod readback;
pub const MAX_EDGE: u32 = 4096;
pub const MAX_PIXELS: u64 = 8 * 1024 * 1024;
const TEXTURE_BUDGET: usize = 64 * 1024 * 1024;
pub struct Request {
    doc: Document,
    region: WorldRect,
    size: [u32; 2],
    originals: Vec<(tack_core::ObjectId, OriginalExport)>,
    profile: Preferences,
}
pub fn dimensions(region: WorldRect) -> Result<[u32; 2], AssetError> {
    if ![region.x, region.y, region.width, region.height]
        .iter()
        .all(|v| v.is_finite())
        || region.width <= 0.
        || region.height <= 0.
        || region.width.ceil() > MAX_EDGE as f64
        || region.height.ceil() > MAX_EDGE as f64
    {
        return Err("Snapshot exceeds 4096 px per side; select a smaller region".into());
    }
    let size = [region.width.ceil() as u32, region.height.ceil() as u32];
    if u64::from(size[0]) * u64::from(size[1]) > MAX_PIXELS {
        return Err("Snapshot exceeds 8 megapixels; select a smaller region".into());
    }
    Ok(size)
}
impl Request {
    pub fn new(
        doc: &Document,
        selection: &SelectionState,
        board_path: &Path,
        board: &TackFile,
        originals: &Originals,
        profile: &Preferences,
    ) -> Result<Self, AssetError> {
        if doc.object_order().len() > 4096
            || doc.assets().count() > 4096
            || doc.sources().count() > 4096
        {
            return Err("Snapshot supports up to 4096 objects, assets and sources".into());
        }
        let metadata = doc
            .object_order()
            .iter()
            .filter_map(|id| doc.object(*id))
            .filter_map(|o| match o.kind() {
                ObjectKind::Annotation(a) => Some(a.retained_bytes()),
                _ => None,
            })
            .try_fold(0usize, |total, size| total.checked_add(size));
        if metadata.is_none_or(|n| n > 16 * 1024 * 1024) {
            return Err("Snapshot annotation metadata exceeds 16 MiB".into());
        }
        let mut union: Option<WorldRect> = None;
        for id in doc.object_order() {
            if !selection.is_empty() && !selection.contains(*id) {
                continue;
            }
            let Some(o) = doc.object(*id) else {
                continue;
            };
            let mut b = match o.kind() {
                ObjectKind::Annotation(a) => a.bounds(o.transform())?,
                _ => o.transform().bounds(),
            };
            if let ObjectKind::Frame(name) = o.kind() {
                let c = Camera::new([1, 1]);
                let label =
                    crate::frame_ui::Label::new(o.transform(), &c, profile.frame_title_scale, name);
                let lo = c.screen_to_world([label.rect[0], label.rect[1]]);
                let hi = c.screen_to_world([label.rect[2], label.rect[3]]);
                let x = b.x.min(lo[0]);
                let y = b.y.min(lo[1]);
                b = WorldRect {
                    x,
                    y,
                    width: (b.x + b.width).max(hi[0]) - x,
                    height: (b.y + b.height).max(hi[1]) - y,
                };
            }
            union = Some(union.map_or(b, |u| {
                let x = u.x.min(b.x);
                let y = u.y.min(b.y);
                WorldRect {
                    x,
                    y,
                    width: (u.x + u.width).max(b.x + b.width) - x,
                    height: (u.y + u.height).max(b.y + b.height) - y,
                }
            }));
        }
        let b = union.ok_or("Board has no content to export")?;
        let region = WorldRect {
            x: b.x - 24.,
            y: b.y - 24.,
            width: b.width + 48.,
            height: b.height + 48.,
        };
        let size = dimensions(region)?;
        let originals = doc
            .object_order()
            .iter()
            .copied()
            .filter(|id| {
                doc.object_render_data(*id)
                    .is_some_and(|d| d.transform.bounds().intersects(region))
            })
            .map(|id| {
                Ok((
                    id,
                    OriginalExport::new(doc, id, board_path, board, originals)?,
                ))
            })
            .collect::<Result<Vec<_>, AssetError>>()?;
        Ok(Self {
            doc: doc.clone(),
            region,
            size,
            originals,
            profile: profile.clone(),
        })
    }
    pub fn pick_and_save(self, work: &Path, cancel: &AtomicBool) -> Result<(), AssetError> {
        let paths = native_files::pick(Picker::ExportSnapshot, work, cancel)?;
        if let Some(path) = paths.first() {
            self.export_to(path, cancel)?;
        }
        Ok(())
    }
    pub fn export_to(&self, path: &Path, cancel: &AtomicBool) -> Result<(), AssetError> {
        let target = crate::file_names::with_suffix(path, "png");
        if target != path && target.try_exists()? {
            return Err("PNG filename already exists; choose the explicit .png destination to confirm replacement".into());
        }
        let pixels = self.render(cancel)?;
        if cancel.load(Ordering::Relaxed) {
            return Err("Snapshot cancelled".into());
        }
        let temporary = target.with_file_name(format!(
            ".tack-snapshot-{:032x}.tmp",
            tack_storage::new_document_id()?.value()
        ));
        let result = (|| {
            tack_assets::snapshot::write_png(&pixels, &temporary)?;
            copy_atomic(&Payload::File(temporary.clone()), &target, cancel, false)
        })();
        let _ = std::fs::remove_file(temporary);
        result
    }
    pub fn render(&self, cancel: &AtomicBool) -> Result<Decoded, AssetError> {
        if cancel.load(Ordering::Relaxed) {
            return Err("Snapshot cancelled".into());
        }
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::from_env_or_default());
        let mut gpu = pollster::block_on(Gpu::new(
            &instance,
            None,
            wgpu::TextureFormat::Rgba8UnormSrgb,
            TEXTURE_BUDGET,
        ))?;
        gpu.constrain_uploads(TEXTURE_BUDGET, 4096);
        let mut camera = Camera::new(self.size);
        camera.set_view(
            [
                self.region.x + self.size[0] as f64 / 2.,
                self.region.y + self.size[1] as f64 / 2.,
            ],
            1.,
        )?;
        let mut draws = Vec::new();
        let mut uploaded = HashMap::new();
        let mut bytes = 0usize;
        gpu.begin_frame()?;
        for (id, original) in &self.originals {
            if cancel.load(Ordering::Relaxed) {
                return Err("Snapshot cancelled".into());
            }
            let mut data = self
                .doc
                .object_render_data(*id)
                .ok_or("Snapshot image unavailable")?;
            if data.filtering == tack_core::ImageFiltering::Default {
                data.filtering = if self.profile.sampling == "Nearest" {
                    tack_core::ImageFiltering::Nearest
                } else {
                    tack_core::ImageFiltering::Smooth
                };
            }
            let key = ProductKey {
                asset: None,
                source: original.source.id(),
                revision: original.source.revision(),
                lod: Lod::Detail,
                edge: 2048,
            };
            if let std::collections::hash_map::Entry::Vacant(entry) = uploaded.entry(key) {
                gpu.begin_frame()?;
                let decoded = tack_assets::snapshot::decode(&original.payload()?, 2048, cancel)?;
                bytes = bytes
                    .checked_add(decoded.rgba.len())
                    .ok_or("Snapshot texture size overflow")?;
                if bytes > TEXTURE_BUDGET {
                    return Err(
                        "Snapshot images exceed 64 MiB display budget; select a smaller region"
                            .into(),
                    );
                }
                if !gpu.upload_product(key, &decoded) {
                    return Err("Snapshot image GPU upload failed".into());
                }
                entry.insert(());
            }
            draws.push(DrawProductImage {
                data,
                key: Some(key),
            });
        }
        let palette = self.profile.theme.palette();
        gpu.set_background(tack_render::Background {
            fill: palette.background,
            grid: palette.grid,
        });
        let mut scene = AnnotationScene::default();
        scene.palette = palette;
        scene.build(
            &self.doc,
            &ImageInteraction::default(),
            &AnnotationInput::default(),
            &camera,
            &draws,
        );
        if scene.omitted > 0 {
            return Err("Snapshot annotation budget exceeded; select a smaller region".into());
        }
        let overlay = frame_overlay(&self.doc, &camera, &self.profile)?;
        let grid = self.profile.grid.then_some(tack_render::GridView {
            spacing: crate::spatial_snap::grid_spacing(1., 1.),
            dpi: 1.,
            lines: self.profile.grid_lines,
        });
        readback::render(&mut gpu, &camera, &draws, &overlay, grid, &scene, cancel)
    }
}
fn frame_overlay(
    doc: &Document,
    camera: &Camera,
    profile: &Preferences,
) -> Result<Vec<tack_render::OverlayQuad>, AssetError> {
    let mut gizmo = crate::image_gizmo::ImageGizmo::default();
    gizmo.palette = profile.theme.palette();
    let mut budget = 4096;
    for id in doc.object_order() {
        if let Some(o) = doc.object(*id)
            && let ObjectKind::Frame(name) = o.kind()
            && o.transform().bounds().intersects(camera.viewport())
        {
            let layout =
                crate::frame_ui::Label::new(o.transform(), camera, profile.frame_title_scale, name);
            gizmo.outline(
                o.transform(),
                camera,
                o.frame_color().rgba(tack_core::Opacity::OPAQUE),
            );
            gizmo.frame_label(camera, &layout, name, o.frame_color(), false, &mut budget);
            if budget == 0 || gizmo.quads.len() >= tack_render::MAX_OVERLAY_QUADS - 4 {
                return Err("Snapshot frame label budget exceeded".into());
            }
        }
    }
    Ok(gizmo.quads)
}
#[cfg(test)]
mod tests;
