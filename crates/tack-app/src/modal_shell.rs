//! Pixel geometry/policy only: short dialogs center, work panels stay top-left.
use crate::image_gizmo::ImageGizmo;
use tack_core::Camera;
#[derive(Clone, Copy, Debug)]
pub struct ModalShell {
    pub rect: [f64; 4],
    pub outside_dismiss: bool,
}
impl ModalShell {
    pub fn work(camera: &Camera, requested: [f64; 2]) -> Self {
        let screen = camera
            .screen_size()
            .map(|v| (f64::from(v) / camera.ui_scale()).floor());
        Self {
            rect: [
                12.,
                12.,
                requested[0].min((screen[0] - 24.).max(1.)).floor(),
                requested[1].min((screen[1] - 24.).max(1.)).floor(),
            ],
            outside_dismiss: false,
        }
    }
    pub fn short(camera: &Camera, requested: [f64; 2], outside_dismiss: bool) -> Self {
        let screen = camera
            .screen_size()
            .map(|v| (f64::from(v) / camera.ui_scale()).floor());
        let size =
            std::array::from_fn::<_, 2, _>(|i| requested[i].min((screen[i] - 24.).max(1.)).floor());
        Self {
            rect: [
                ((screen[0] - size[0]) / 2.).floor(),
                ((screen[1] - size[1]) / 2.).floor(),
                size[0],
                size[1],
            ],
            outside_dismiss,
        }
    }
    pub fn paint(self, g: &mut ImageGizmo, c: &Camera, p: crate::ui_theme::Palette) {
        let [x, y, w, h] = self.rect;
        let s = c.ui_scale();
        g.pixel_rect(
            c,
            [x * s, y * s],
            [(x + w) * s, (y + h) * s],
            p.menu_border,
            None,
        );
        if w > 2. && h > 2. {
            g.pixel_rect(
                c,
                [(x + 1.) * s, (y + 1.) * s],
                [(x + w - 1.) * s, (y + h - 1.) * s],
                p.menu_bg,
                None,
            );
        }
    }
    pub fn outside(self, p: [f64; 2], scale: f64) -> bool {
        let p = p.map(|v| v / scale);
        let [x, y, w, h] = self.rect;
        self.outside_dismiss && !(p[0] >= x && p[0] < x + w && p[1] >= y && p[1] < y + h)
    }
    pub fn offset(self) -> [f64; 2] {
        [self.rect[0] - 12., self.rect[1] - 12.]
    }
    pub fn translate(self, g: &mut ImageGizmo, start: usize, camera: &Camera) {
        let offset = self.offset().map(|v| v * camera.ui_scale() / camera.zoom());
        for quad in &mut g.quads[start..] {
            for point in &mut quad.points {
                for i in 0..2 {
                    point[i] += offset[i];
                }
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn short_dialog_center_resize_and_destructive_outside_policy() {
        let c = Camera::new([800, 600]);
        let a = ModalShell::short(&c, [400., 180.], true);
        assert_eq!(a.rect, [200., 210., 400., 180.]);
        assert!(a.outside([0., 0.], 1.));
        assert!(!ModalShell::short(&c, [400., 180.], false).outside([0., 0.], 1.));
    }
}
