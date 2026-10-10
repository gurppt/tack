//! Shared drawn/title hit geometry and a tiny deterministic Frame label style.
use crate::{note_layout::NoteLines, ui_theme::Color};
use tack_core::{Camera, Opacity, Transform};
pub struct Label {
    pub rect: [f64; 4],
    pub glyph_scale: f64,
    pub line_width: f64,
}
impl Label {
    pub fn new(t: Transform, camera: &Camera, title_scale: u8, text: &str) -> Self {
        let scale = camera.ui_scale().round().clamp(1., 8.);
        let glyph_scale = scale * f64::from(title_scale.clamp(1, 3));
        let b = t.bounds();
        let at = camera.world_to_screen([b.x, b.y]);
        let left = (at[0] / scale).round() * scale;
        let bottom = (at[1] / scale).round() * scale;
        let width = (b.width * camera.zoom() / scale).round() * scale;
        let line_width = (width - 4. * scale).max(0.) / glyph_scale;
        let rows = NoteLines::new(text, line_width, 16.).count().max(1);
        Self {
            rect: [
                left,
                bottom - rows as f64 * 18. * glyph_scale,
                left + width,
                bottom,
            ],
            glyph_scale,
            line_width,
        }
    }
    pub fn contains(&self, p: [f64; 2]) -> bool {
        p[0] >= self.rect[0] && p[0] < self.rect[2] && p[1] >= self.rect[1] && p[1] < self.rect[3]
    }
}
pub fn label_style(color: tack_core::Color) -> (Color, Color) {
    let background = color.rgba(Opacity::OPAQUE);
    let luminance = background[0] * 0.2126 + background[1] * 0.7152 + background[2] * 0.0722;
    let text = if luminance > 0.18 {
        [0.005, 0.007, 0.01, 1.]
    } else {
        [0.96, 0.97, 1., 1.]
    };
    (background, text)
}
