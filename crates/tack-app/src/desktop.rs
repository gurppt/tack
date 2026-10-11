//! Event-only window chrome policy; no timer or external-window tracking.
#[derive(Default)]
pub struct Chrome {
    pub visible: bool,
    pub pressed: Option<usize>,
    pub right_start: Option<[f64; 2]>,
    pub right_dragged: bool,
    pub applied: Option<(bool, bool)>,
}
impl Chrome {
    pub fn hover(&mut self, point: [f64; 2], scale: f64, compact: bool) -> bool {
        let next =
            compact && point[1] >= 0. && point[1] < if self.visible { 24. } else { 4. } * scale;
        let changed = self.visible != next;
        self.visible = next;
        changed
    }
    pub fn moved(&self, point: [f64; 2], scale: f64) -> bool {
        self.right_start
            .is_some_and(|start| (point[0] - start[0]).hypot(point[1] - start[1]) >= 8. * scale)
    }
    pub fn button(&self, point: [f64; 2], width: f64, scale: f64) -> Option<usize> {
        if !self.visible || point[1] < 0. || point[1] >= 24. * scale {
            return None;
        }
        let x = point[0] / scale;
        if (4. ..32.).contains(&x) {
            Some(0)
        } else {
            let start = width / scale - 76.;
            if x >= start && x < start + 72. {
                Some(1 + ((x - start) / 24.) as usize)
            } else {
                None
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reveal_hide_threshold_and_buttons() {
        let mut chrome = Chrome::default();
        assert!(!chrome.hover([50., 10.], 2., true));
        assert!(chrome.hover([50., 4.], 2., true));
        assert!(!chrome.hover([50., 40.], 2., true));
        assert_eq!(chrome.button([20., 20.], 800., 2.), Some(0));
        assert_eq!(chrome.button([680., 20.], 800., 2.), Some(1));
        chrome.right_start = Some([40., 50.]);
        assert!(!chrome.moved([50., 50.], 2.));
        assert!(chrome.moved([56., 50.], 2.));
        assert!(chrome.hover([50., 50.], 2., true));
        assert!(chrome.hover([50., 4.], 2., true));
        assert!(chrome.hover([50., 4.], 2., false));
    }
}
