//! Logical UI preferences, shared drawing/hit geometry; never document state.
use crate::{image_geometry as geometry, image_interaction::ImageInteraction};
use tack_core::{Camera, Document, DocumentQuery, Transform};
use tack_render::{MAX_OVERLAY_QUADS, OverlayQuad, SelectionRect};
#[derive(Clone, Copy)]
pub struct GizmoStyle {
    pub handle_size: f64,
    pub hit_radius: f64,
    pub line_width: f64,
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
    pub palette: crate::ui_theme::Palette,
    pub scale: f64,
    pub frame_title_scale: u8,
    pub missing_sources: std::collections::BTreeSet<tack_core::SourceId>,
    hit_scale: f64,
    pub quads: Vec<OverlayQuad>,
    pub selection: Vec<SelectionRect>,
}
impl Default for ImageGizmo {
    fn default() -> Self {
        Self {
            style: GizmoStyle::default(),
            palette: crate::ui_theme::Theme::default().palette(),
            scale: 1.,
            frame_title_scale: 1,
            missing_sources: Default::default(),
            hit_scale: 1.,
            quads: Vec::with_capacity(128),
            selection: Vec::new(),
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
    pub fn handle(&self, frame: Transform, _camera: &Camera, index: usize) -> [f64; 2] {
        let d = geometry::HANDLE_DIRECTIONS
            .get(index)
            .copied()
            .unwrap_or([0.; 2]);
        geometry::world(
            frame,
            [d[0] * frame.size()[0] / 2., d[1] * frame.size()[1] / 2.],
        )
    }

    pub fn hit(
        &self,
        frame: Transform,
        camera: &Camera,
        pointer: [f64; 2],
        crop: bool,
        multiple: bool,
    ) -> Option<GizmoHit> {
        let radius = self.style.hit_radius * self.hit_scale / camera.zoom();
        let p = geometry::local(frame, camera.screen_to_world(pointer));
        let half = frame.size().map(|v| v / 2.);
        // The near band wins, including arbitrary points along each full edge.
        let near = [(p[0].abs() - half[0]).abs(), (p[1].abs() - half[1]).abs()];
        let in_band = p[0].abs() <= half[0] + radius && p[1].abs() <= half[1] + radius;
        if in_band && near.into_iter().any(|v| v <= radius) {
            let horizontal = near[0] <= radius;
            let vertical = near[1] <= radius;
            let index = match (horizontal, vertical, p[0] >= 0., p[1] >= 0.) {
                (true, true, false, false) => 0,
                (true, true, true, false) => 2,
                (true, true, true, true) => 4,
                (true, true, false, true) => 6,
                (true, false, true, _) => 3,
                (true, false, false, _) => 7,
                (false, true, _, false) => 1,
                _ => 5,
            };
            return Some(GizmoHit::Resize(index));
        }
        let _ = multiple;
        if !crop
            && p[0].abs() > half[0]
            && p[1].abs() > half[1]
            && (p[0].abs() - half[0]).hypot(p[1].abs() - half[1]) <= radius * 3.
        {
            return Some(GizmoHit::Rotate);
        }
        None
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
            dashed: false,
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
        self.selection.clear();
        let color = if images.crop_mode {
            self.style.crop
        } else {
            self.style.selection
        };
        let Some(frame) = images.frame(doc) else {
            self.selection = Vec::new();
            return;
        };
        let viewport = camera.viewport();
        for id in images.selection.ids() {
            if self.selection.len() >= tack_render::MAX_SELECTION_RECTS {
                break;
            }
            if doc.object_render_data(id).is_some()
                && let Some(transform) = images.preview_transform(doc, id)
                && transform.bounds().intersects(viewport)
            {
                self.selection.push(SelectionRect {
                    transform,
                    color: if images.crop_mode {
                        color
                    } else {
                        doc.object_render_data(id)
                            .and_then(|d| doc.asset(d.asset_id))
                            .and_then(|a| doc.source(a.source_id()))
                            .map_or(color, |s| {
                                crate::source_info::color(
                                    s.location(),
                                    self.missing_sources
                                        .contains(&s.id())
                                        .then_some(tack_assets::SourceState::Missing),
                                )
                            })
                    },
                    width: 1.,
                });
            }
        }
        // One coherent handle set, plus a light outline for each visible selected image.
        // Transform chrome is handles plus a rotation stem only. Foreign
        // manipulation indication is supplied separately by shared leases.

        for i in 0..8 {
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
                    dashed: false,
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
