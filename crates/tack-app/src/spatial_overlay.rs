//! Bounded pixel primitives in the existing canvas overlay; no widget objects.
use crate::{image_gizmo::ImageGizmo, image_input::ImageInput, pixel_font};
use tack_core::{Camera, DocumentEditor, ObjectKind};
use tack_render::{MAX_OVERLAY_QUADS, OverlayQuad};
impl ImageGizmo {
    pub fn pixel_rect(
        &mut self,
        camera: &Camera,
        lo: [f64; 2],
        hi: [f64; 2],
        color: [f32; 4],
        bitmap: Option<[u32; 8]>,
    ) {
        if self.quads.len() >= MAX_OVERLAY_QUADS {
            return;
        }
        self.quads.push(OverlayQuad {
            points: [lo, [lo[0], hi[1]], [hi[0], lo[1]], hi].map(|p| {
                camera
                    .screen_to_world(p.map(|v| (v / camera.ui_scale()).round() * camera.ui_scale()))
            }),
            color,
            bitmap,
        });
    }
    pub fn ui_text(
        &mut self,
        camera: &Camera,
        at: [f64; 2],
        width: f64,
        text: &str,
        color: [f32; 4],
        budget: &mut usize,
    ) {
        let scale = camera.ui_scale();
        let mut x = (at[0] / scale).round() * scale;
        let y = (at[1] / scale).round() * scale;
        let right = x + width * scale;
        for c in text.chars() {
            let (bits, advance) = pixel_font::glyph(c);
            if *budget == 0
                || x + advance as f64 * scale > right
                || self.quads.len() >= MAX_OVERLAY_QUADS
            {
                break;
            }
            self.pixel_rect(
                camera,
                [x, y],
                [x + 16. * scale, y + 16. * scale],
                color,
                Some(bits),
            );
            x += advance as f64 * scale;
            *budget -= 1;
        }
    }
    pub(crate) fn label(&mut self, camera: &Camera, at: [f64; 2], text: &str, budget: &mut usize) {
        let mut x = (at[0] / self.scale).round() * self.scale;
        let right = x + 160. * self.scale;
        let y = (at[1] / self.scale).round() * self.scale;
        self.pixel_rect(
            camera,
            [x - 2. * self.scale, y],
            [right, y + 18. * self.scale],
            self.palette.menu_bg,
            None,
        );
        for c in text.chars() {
            let (bits, width) = pixel_font::glyph(c);
            if *budget == 0
                || x + width as f64 * self.scale > right
                || self.quads.len() >= MAX_OVERLAY_QUADS
            {
                break;
            }
            self.pixel_rect(
                camera,
                [x, y],
                [x + 16. * self.scale, y + 16. * self.scale],
                self.palette.text_secondary,
                Some(bits),
            );
            x += width as f64 * self.scale;
            *budget -= 1;
        }
    }
}
impl ImageInput {
    pub(crate) fn spatial_overlay(&mut self, editor: &DocumentEditor, camera: &Camera) {
        let doc = editor.document();
        let viewport = camera.viewport();
        let mut labels = 512;
        // Selection was built first and retains priority over crowded frame labels.
        for o in doc
            .object_order()
            .iter()
            .filter_map(|id| doc.object(*id))
            .take(if doc.frame_count() == 0 {
                0
            } else {
                usize::MAX
            })
        {
            let ObjectKind::Frame(name) = o.kind() else {
                continue;
            };
            let Some(t) = self.images.preview_transform(doc, o.id()) else {
                continue;
            };
            if !t.bounds().intersects(viewport) {
                continue;
            }
            if self.gizmo.quads.len() + 4 >= MAX_OVERLAY_QUADS {
                break;
            }
            self.gizmo.outline(
                t,
                camera,
                if self.images.selection.contains(o.id()) {
                    self.gizmo.style.selection
                } else {
                    self.gizmo.palette.text_secondary
                },
            );
            let b = t.bounds();
            let p = camera.world_to_screen([b.x, b.y]);
            if p[0] >= -160. * self.gizmo.scale
                && p[0] < viewport.width * camera.zoom()
                && p[1] >= 0.
                && p[1] < viewport.height * camera.zoom()
                && labels > 0
            {
                let text = self
                    .name_edit
                    .as_ref()
                    .filter(|e| e.id == o.id())
                    .map_or(name.as_str(), |e| e.value.as_str());
                self.gizmo.label(
                    camera,
                    [p[0], p[1] - 18. * self.gizmo.scale],
                    text,
                    &mut labels,
                );
            }
        }
        self.gizmo.quads.truncate(MAX_OVERLAY_QUADS - 2);
        for axis in 0..2 {
            if let Some(s) = self.snap.guides[axis] {
                let (a, b) = if axis == 0 {
                    (
                        [s.target, viewport.y],
                        [s.target, viewport.y + viewport.height],
                    )
                } else {
                    (
                        [viewport.x, s.target],
                        [viewport.x + viewport.width, s.target],
                    )
                };
                self.gizmo.line(
                    a,
                    b,
                    self.gizmo.scale / camera.zoom(),
                    self.gizmo.palette.guide,
                );
            }
        }
    }
}
