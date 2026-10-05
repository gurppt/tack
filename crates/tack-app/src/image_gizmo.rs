//! Logical UI preferences, shared drawing/hit geometry; never document state.
use crate::{image_geometry as geometry, image_interaction::ImageInteraction};
use tack_core::{Camera, Document, Transform};
use tack_render::{MAX_OVERLAY_QUADS, OverlayQuad};
#[derive(Clone, Copy)]
pub struct GizmoStyle {
    pub handle_size: f64,
    pub hit_radius: f64,
    pub line_width: f64,
    pub rotation_offset: f64,
    pub selection: [f32; 4],
    pub crop: [f32; 4],
    pub active: [f32; 4],
}
impl Default for GizmoStyle {
    fn default() -> Self {
        Self {
            handle_size: 7.,
            hit_radius: 9.,
            line_width: 1.,
            rotation_offset: 26.,
            selection: [0.25, 0.65, 0.83, 1.],
            crop: [0.93, 0.68, 0.3, 1.],
            active: [0.75, 0.9, 1., 1.],
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GizmoHit {
    Resize(usize),
    Rotate,
}
pub struct ImageGizmo {
    pub style: GizmoStyle,
    pub scale: f64,
    hit_scale: f64,
    pub quads: Vec<OverlayQuad>,
}
impl Default for ImageGizmo {
    fn default() -> Self {
        Self {
            style: GizmoStyle::default(),
            scale: 1.,
            hit_scale: 1.,
            quads: Vec::with_capacity(128),
        }
    }
}
impl ImageGizmo {
    pub fn set_scale(&mut self, scale: f64) {
        if scale.is_finite() && scale > 0. {
            self.scale = scale.round().clamp(1., 8.);
            self.hit_scale = scale;
        }
    }
    pub fn handle(&self, frame: Transform, camera: &Camera, index: usize) -> [f64; 2] {
        if index == 8 {
            geometry::world(
                frame,
                [
                    0.,
                    -frame.size()[1] / 2. - self.style.rotation_offset * self.scale / camera.zoom(),
                ],
            )
        } else {
            let d = geometry::HANDLE_DIRECTIONS[index];
            geometry::world(
                frame,
                [d[0] * frame.size()[0] / 2., d[1] * frame.size()[1] / 2.],
            )
        }
    }
    pub fn hit(
        &self,
        frame: Transform,
        camera: &Camera,
        pointer: [f64; 2],
        crop: bool,
        multiple: bool,
    ) -> Option<GizmoHit> {
        let radius = self.style.hit_radius * self.hit_scale;
        let mut best = None;
        let mut distance = radius;
        for i in 0..if crop { 8 } else { 9 } {
            if multiple && i < 8 && i % 2 == 1 {
                continue;
            }
            let p = camera.world_to_screen(self.handle(frame, camera, i));
            let next = (p[0] - pointer[0]).hypot(p[1] - pointer[1]);
            if next <= distance {
                distance = next;
                best = Some(if i == 8 {
                    GizmoHit::Rotate
                } else {
                    GizmoHit::Resize(i)
                });
            }
        }
        best
    }
    pub(crate) fn line(&mut self, a: [f64; 2], b: [f64; 2], width: f64, color: [f32; 4]) {
        if self.quads.len() >= MAX_OVERLAY_QUADS {
            return;
        }
        let delta = [b[0] - a[0], b[1] - a[1]];
        let length = delta[0].hypot(delta[1]);
        if length <= 0. || !length.is_finite() {
            return;
        }
        let n = [
            -delta[1] * width / length / 2.,
            delta[0] * width / length / 2.,
        ];
        self.quads.push(OverlayQuad {
            bitmap: None,
            points: [
                [a[0] - n[0], a[1] - n[1]],
                [a[0] + n[0], a[1] + n[1]],
                [b[0] - n[0], b[1] - n[1]],
                [b[0] + n[0], b[1] + n[1]],
            ],
            color,
        });
    }
    pub fn outline(&mut self, t: Transform, camera: &Camera, color: [f32; 4]) {
        let points = geometry::corners(t);
        for i in 0..4 {
            self.line(
                points[i],
                points[(i + 1) % 4],
                self.style.line_width * self.scale / camera.zoom(),
                color,
            );
        }
    }
    pub fn build(
        &mut self,
        images: &ImageInteraction,
        doc: &Document,
        camera: &Camera,
        hover: Option<GizmoHit>,
    ) {
        self.quads.clear();
        let color = if images.crop_mode {
            self.style.crop
        } else {
            self.style.selection
        };
        let Some(frame) = images.frame(doc) else {
            return;
        };
        // A bounded decoration budget. Group outline always remains visible.
        for id in images.selection.ids().take(20) {
            if let Some(t) = images.preview_transform(doc, id) {
                self.outline(t, camera, color);
            }
        }
        if images.selection.len() > 1 {
            self.outline(frame, camera, color);
        }
        let crop = images.crop_mode && images.selection.len() == 1;
        let has_frame = doc.frame_count() > 0
            && images.selection.ids().any(|id| {
                doc.object(id)
                    .is_some_and(|o| matches!(o.kind(), tack_core::ObjectKind::Frame(_)))
            });
        if !crop && !has_frame {
            self.line(
                self.handle(frame, camera, 1),
                self.handle(frame, camera, 8),
                self.style.line_width * self.scale / camera.zoom(),
                color,
            );
        }
        for i in 0..if crop || has_frame { 8 } else { 9 } {
            if images.selection.len() > 1 && i < 8 && i % 2 == 1 {
                continue;
            }
            let p = self.handle(frame, camera, i);
            let r = self.style.handle_size * self.scale / camera.zoom() / 2.;
            if self.quads.len() < MAX_OVERLAY_QUADS {
                let active = hover
                    == Some(if i == 8 {
                        GizmoHit::Rotate
                    } else {
                        GizmoHit::Resize(i)
                    })
                    || images.active();
                self.quads.push(OverlayQuad {
                    bitmap: None,
                    points: [
                        [p[0] - r, p[1] - r],
                        [p[0] - r, p[1] + r],
                        [p[0] + r, p[1] - r],
                        [p[0] + r, p[1] + r],
                    ],
                    color: if active { self.style.active } else { color },
                });
            }
        }
    }
}
