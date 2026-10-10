//! Small row scroll owner: wheel, captured thumb and keyboard reveal only.
use crate::{image_gizmo::ImageGizmo, ui_theme::Palette};
use tack_core::Camera;
#[derive(Clone, Copy, Default)]
pub(crate) struct Scrollbar {
    pub(crate) viewport: [f64; 4],
    thumb: [f64; 4],
    first: usize,
    visible: usize,
    count: usize,
    drag: Option<f64>,
}
fn hit(r: [f64; 4], p: [f64; 2]) -> bool {
    p[0] >= r[0] && p[0] < r[2] && p[1] >= r[1] && p[1] < r[3]
}
pub(crate) fn reveal(first: usize, selected: usize, visible: usize, count: usize) -> usize {
    let first = first.min(count.saturating_sub(visible));
    if selected < first {
        selected
    } else if selected >= first + visible {
        selected.saturating_add(1).saturating_sub(visible)
    } else {
        first
    }
    .min(count.saturating_sub(visible))
}
impl Scrollbar {
    pub(crate) fn draw(
        &mut self,
        g: &mut ImageGizmo,
        c: &Camera,
        palette: Palette,
        viewport: [f64; 4],
        rows: (usize, usize, usize),
    ) {
        let screen = c.screen_size().map(|v| f64::from(v) / c.ui_scale());
        let viewport = [
            viewport[0].clamp(0., screen[0]),
            viewport[1].clamp(0., screen[1]),
            viewport[2].clamp(0., screen[0]),
            viewport[3].clamp(0., screen[1]),
        ];
        self.viewport = viewport;
        (self.first, self.visible, self.count) = rows;
        if self.count <= self.visible || viewport[3] <= viewport[1] || viewport[2] <= viewport[0] {
            self.thumb = [0.; 4];
            self.drag = None;
            return;
        }
        let height = viewport[3] - viewport[1];
        let thumb_height = (height * self.visible as f64 / self.count as f64)
            .round()
            .clamp(8., height);
        let travel = height - thumb_height;
        let y =
            viewport[1] + (travel * self.first as f64 / (self.count - self.visible) as f64).round();
        self.thumb = [viewport[2] - 7., y, viewport[2] - 1., y + thumb_height];
        let s = c.ui_scale();
        g.pixel_rect(
            c,
            [(viewport[2] - 8.) * s, viewport[1] * s],
            [viewport[2] * s, viewport[3] * s],
            palette.menu_border,
            None,
        );
        g.pixel_rect(
            c,
            [self.thumb[0] * s, self.thumb[1] * s],
            [self.thumb[2] * s, self.thumb[3] * s],
            palette.accent_secondary,
            None,
        );
    }
    pub(crate) fn wheel(&self, p: [f64; 2], delta: f64) -> Option<usize> {
        if !hit(self.viewport, p) || delta == 0. || !delta.is_finite() {
            return None;
        }
        let steps = delta.abs().ceil().clamp(1., 3.) as usize;
        Some(
            if delta > 0. {
                self.first.saturating_sub(steps)
            } else {
                self.first.saturating_add(steps)
            }
            .min(self.count.saturating_sub(self.visible)),
        )
    }
    pub(crate) fn press(&mut self, p: [f64; 2]) -> bool {
        if self.count <= self.visible
            || !hit(
                [
                    self.viewport[2] - 8.,
                    self.viewport[1],
                    self.viewport[2],
                    self.viewport[3],
                ],
                p,
            )
        {
            return false;
        }
        self.drag = Some(if hit(self.thumb, p) {
            p[1] - self.thumb[1]
        } else {
            (self.thumb[3] - self.thumb[1]) / 2.
        });
        true
    }
    pub(crate) fn motion(&self, p: [f64; 2]) -> Option<usize> {
        let offset = self.drag?;
        let travel = self.viewport[3] - self.viewport[1] - (self.thumb[3] - self.thumb[1]);
        let ratio = if travel > 0. {
            ((p[1] - offset - self.viewport[1]) / travel).clamp(0., 1.)
        } else {
            0.
        };
        Some((ratio * self.count.saturating_sub(self.visible) as f64).round() as usize)
    }
    pub(crate) fn release(&mut self) -> bool {
        self.drag.take().is_some()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reveal_is_minimal_and_bounded() {
        assert_eq!(reveal(4, 6, 8, 100), 4);
        assert_eq!(reveal(4, 12, 8, 100), 5);
        assert_eq!(reveal(80, 99, 8, 100), 92);
        assert_eq!(reveal(8, 2, 8, 100), 2);
    }
}
