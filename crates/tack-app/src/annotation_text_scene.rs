//! Note glyph quads use smooth atlas sampling; UI labels retain the pixel path.
use crate::{
    annotation_scene::{AnnotationScene, primitive},
    image_geometry,
    note_layout::{NoteLines, advance},
    pixel_font,
};
use tack_core::{AnnotationStyle, TextAlignment, Transform};
impl AnnotationScene {
    pub(crate) fn note(
        &mut self,
        style: AnnotationStyle,
        t: Transform,
        text: &str,
        size: f64,
        alignment: TextAlignment,
    ) -> bool {
        let box_size = t.size();
        let padding = (size * 0.2).min(box_size[0].min(box_size[1]) * 0.1);
        let width = (box_size[0] - padding * 2.).max(1e-6);
        if style.fill().is_some() {
            let points = [[-1., -1.], [-1., 1.], [1., -1.], [1., 1.]].map(|d| {
                image_geometry::world(t, [d[0] * box_size[0] / 2., d[1] * box_size[1] / 2.])
            });
            let mut p = primitive(points, box_size, 7, style);
            p.stroke = style.fill().map_or([0.; 4], |c| c.rgba(style.opacity()));
            p.fill = p.stroke;
            if !self.push(p) {
                return false;
            }
        }
        for (line, (text, line_width)) in NoteLines::new(text, width, size).enumerate() {
            let y = padding + line as f64 * size * 1.3;
            if y >= box_size[1] - padding {
                break;
            }
            let mut x = padding
                + match alignment {
                    TextAlignment::Left => 0.,
                    TextAlignment::Center => (width - line_width).max(0.) / 2.,
                    TextAlignment::Right => (width - line_width).max(0.),
                };
            for c in text.chars() {
                let step = advance(c, size);
                if !c.is_whitespace() {
                    let uv = tack_render::note_glyph_uv(c);
                    let lo = if uv.is_some() {
                        [x - size * 0.125, y - size * 0.25]
                    } else {
                        [x, y]
                    };
                    let extent = if uv.is_some() {
                        [size, size * 1.5]
                    } else {
                        [size, size]
                    };
                    let clipped_lo = [lo[0].max(0.), lo[1].max(0.)];
                    let clipped_hi = [
                        (lo[0] + extent[0]).min(box_size[0]),
                        (lo[1] + extent[1]).min(box_size[1]),
                    ];
                    if clipped_hi[0] > clipped_lo[0] && clipped_hi[1] > clipped_lo[1] {
                        let points = [
                            clipped_lo,
                            [clipped_lo[0], clipped_hi[1]],
                            [clipped_hi[0], clipped_lo[1]],
                            clipped_hi,
                        ]
                        .map(|p| {
                            image_geometry::world(
                                t,
                                std::array::from_fn(|i| {
                                    (p[i] - box_size[i] / 2.) * if t.flips()[i] { -1. } else { 1. }
                                }),
                            )
                        });
                        let mut p = primitive(
                            points,
                            [size, size],
                            if uv.is_some() { 5 } else { 6 },
                            style,
                        );
                        let uv = uv.unwrap_or([0., 0., 1., 1.]).map(f64::from);
                        p.atlas_uv = [
                            uv[0] + (clipped_lo[0] - lo[0]) / extent[0] * uv[2],
                            uv[1] + (clipped_lo[1] - lo[1]) / extent[1] * uv[3],
                            (clipped_hi[0] - clipped_lo[0]) / extent[0] * uv[2],
                            (clipped_hi[1] - clipped_lo[1]) / extent[1] * uv[3],
                        ]
                        .map(|v| v as f32);
                        if p.kind == 6 {
                            p.bitmap = pixel_font::glyph(c).0;
                        }
                        if !self.push(p) {
                            return false;
                        }
                        self.glyphs += 1;
                    }
                }
                x += step;
            }
        }
        true
    }
}
