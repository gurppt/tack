//! Static build-derived bitmap identity: no image decoder, worker or SVG runtime.
use crate::image_gizmo::ImageGizmo;
use tack_core::Camera;
include!(concat!(env!("OUT_DIR"), "/icon_pixels.rs"));
pub fn window() -> Option<winit::window::Icon> {
    winit::window::Icon::from_rgba(
        include_bytes!(concat!(env!("OUT_DIR"), "/icon.rgba")).to_vec(),
        32,
        32,
    )
    .ok()
}
pub fn draw(gizmo: &mut ImageGizmo, camera: &Camera, origin: [f64; 2], scale: f64) {
    for (i, p) in PIXELS.iter().enumerate() {
        if p[3] == 0 {
            continue;
        }
        let [x, y] = [
            origin[0] + (i % 16) as f64 * scale,
            origin[1] + (i / 16) as f64 * scale,
        ];
        gizmo.pixel_rect(
            camera,
            [x, y],
            [x + scale, y + scale],
            p.map(|c| f32::from(c) / 255.),
            None,
        );
    }
}
