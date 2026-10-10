//! Bounded dotted Frame preview. One quad per visible unit, existing overlay pass.
use crate::image_gizmo::ImageGizmo;
use tack_core::Camera;
use tack_render::{MAX_OVERLAY_QUADS, OverlayQuad};
pub const MAX_LINES: usize = 256;
#[derive(Clone, Copy, Debug, Default)]
pub struct Stats {
    pub units: usize,
    pub lines: usize,
    pub vertices: usize,
    pub simplified: bool,
}
/// Decorative writes can be refused without turning a valid document into a fatal error.
impl ImageGizmo {
    pub fn try_push_decoration(&mut self, quad: OverlayQuad, limit: usize) -> bool {
        if self.quads.len() >= limit.min(MAX_OVERLAY_QUADS) {
            return false;
        }
        self.quads.push(quad);
        true
    }
}
/// Visible-screen clipping bounds both the shader envelope and its dash count.
fn clip(mut a: [f64; 2], b: [f64; 2], screen: [f64; 2]) -> Option<([f64; 2], [f64; 2])> {
    if !a.into_iter().chain(b).all(f64::is_finite) {
        return None;
    }
    let d = [b[0] - a[0], b[1] - a[1]];
    if !d.into_iter().all(f64::is_finite) {
        return None;
    }
    let mut lo: f64 = 0.;
    let mut hi: f64 = 1.;
    for (p, q) in [
        (-d[0], a[0]),
        (d[0], screen[0] - a[0]),
        (-d[1], a[1]),
        (d[1], screen[1] - a[1]),
    ] {
        if p == 0. {
            if q < 0. {
                return None;
            }
        } else {
            let t = q / p;
            if p < 0. {
                lo = lo.max(t);
            } else {
                hi = hi.min(t);
            }
        }
    }
    if lo > hi {
        return None;
    }
    let end = std::array::from_fn(|i| a[i] + hi * d[i]);
    a = std::array::from_fn(|i| a[i] + lo * d[i]);
    Some((a, end))
}
pub fn draw(
    g: &mut ImageGizmo,
    camera: &Camera,
    anchors: &[[f64; 2]],
    cursor: [f64; 2],
    limit: usize,
) -> Stats {
    let available = limit
        .min(MAX_OVERLAY_QUADS)
        .saturating_sub(g.quads.len())
        .min(MAX_LINES);
    let mut stats = Stats {
        units: anchors.len(),
        ..Default::default()
    };
    if available == 0 {
        stats.simplified = !anchors.is_empty();
        return stats;
    }
    let stride = anchors.len().div_ceil(available).max(1);
    stats.simplified = stride > 1;
    let scale = camera.ui_scale();
    for anchor in anchors.iter().step_by(stride) {
        let Some((a, b)) = clip(
            camera.world_to_screen(*anchor),
            cursor,
            camera.screen_size().map(f64::from),
        ) else {
            continue;
        };
        let a = a.map(|v| ((v / scale).round() + 0.5) * scale);
        let b = b.map(|v| ((v / scale).round() + 0.5) * scale);
        let d = [b[0] - a[0], b[1] - a[1]];
        let length = d[0].hypot(d[1]);
        if length < scale {
            continue;
        }
        let n = [-d[1] / length * scale / 2., d[0] / length * scale / 2.];
        let quad = OverlayQuad {
            points: [
                [a[0] - n[0], a[1] - n[1]],
                [a[0] + n[0], a[1] + n[1]],
                [b[0] - n[0], b[1] - n[1]],
                [b[0] + n[0], b[1] + n[1]],
            ]
            .map(|p| camera.screen_to_world(p)),
            color: g.style.selection,
            bitmap: None,
            dashed: true,
        };
        if !g.try_push_decoration(quad, limit) {
            stats.simplified = true;
            break;
        }
        stats.lines += 1;
        if stats.lines == available {
            break;
        }
    }
    stats.vertices = stats.lines * 6;
    stats
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn extreme_zoom_distance_and_exhausted_budget_remain_bounded()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut c = Camera::new([800, 600]);
        for zoom in [0.0001, 1., 1000.] {
            c.set_view([0.; 2], zoom)?;
            let mut g = ImageGizmo::default();
            let stats = draw(&mut g, &c, &[[0.; 2]; 10000], [1e12, 1e12], 3);
            assert!(stats.simplified);
            assert!(stats.lines <= 3);
            assert!(g.quads.len() <= 3);
            assert!(g.quads.iter().all(|q| q.dashed));
            let before = g.quads.len();
            draw(&mut g, &c, &[[0.; 2]], [500., 400.], before);
            assert_eq!(g.quads.len(), before);
        }
        Ok(())
    }
    #[test]
    fn line_is_one_quad_independent_of_dash_count() {
        let c = Camera::new([800, 600]);
        let mut g = ImageGizmo::default();
        let a = c.screen_to_world([100., 100.]);
        let stats = draw(&mut g, &c, &[a], [700., 100.], MAX_OVERLAY_QUADS);
        assert_eq!(stats.lines, 1);
        assert_eq!(stats.vertices, 6);
        assert_eq!(g.quads.len(), 1);
        assert!(g.quads[0].dashed);
    }
}
