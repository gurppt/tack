//! Resident metadata scan. No candidate vector, index, I/O or source access.
use crate::image_interaction::SelectionState;
use tack_core::{Document, ObjectId, WorldRect};

pub const SNAP_PIXELS: f64 = 6.;
pub const GRID_BASE: f64 = 64.;
pub fn grid_spacing(zoom: f64, dpi: f64) -> f64 {
    GRID_BASE * (24. * dpi / (GRID_BASE * zoom)).log2().ceil().exp2()
}
#[derive(Clone, Copy, Debug)]
pub struct AxisSnap {
    pub target: f64,
    pub anchor: usize,
    object: Option<ObjectId>,
}
#[derive(Default)]
pub struct SnapState {
    pub enabled: bool,
    pub temporary: bool,
    pub grid: bool,
    pub disabled: bool,
    pub measure: bool,
    pub last_query_ms: Option<f64>,
    pub guides: [Option<AxisSnap>; 2],
}
fn anchors(r: WorldRect, axis: usize) -> [f64; 3] {
    let (start, size) = if axis == 0 {
        (r.x, r.width)
    } else {
        (r.y, r.height)
    };
    [start, start + size / 2., start + size]
}
impl SnapState {
    pub fn clear(&mut self) {
        self.guides = [None; 2];
    }
    pub fn resolve(
        &mut self,
        doc: &Document,
        selection: &SelectionState,
        bounds: WorldRect,
        zoom: f64,
        dpi: f64,
        resize: Option<[usize; 2]>,
    ) -> [f64; 2] {
        if self.disabled || (!self.enabled && !self.grid && !self.temporary) {
            self.clear();
            return [0.; 2];
        }
        let tolerance = SNAP_PIXELS * dpi / zoom;
        let mut best = [None; 2];
        let mut distances = [f64::INFINITY; 2];
        for axis in 0..2 {
            let a = anchors(bounds, axis);
            if let Some(old) = self.guides[axis]
                && resize.is_none_or(|r| r[axis] == old.anchor)
                && (old.target - a[old.anchor]).abs() <= tolerance * 1.5
            {
                best[axis] = Some(old);
                distances[axis] = -1.;
            }
        }
        let mut consider = |axis: usize, target: f64, object: Option<ObjectId>| {
            for (anchor, position) in anchors(bounds, axis).into_iter().enumerate() {
                if resize.is_some_and(|r| r[axis] != anchor) {
                    continue;
                }
                let distance = (target - position).abs();
                if distance <= tolerance && distance < distances[axis] {
                    distances[axis] = distance;
                    best[axis] = Some(AxisSnap {
                        target,
                        anchor,
                        object,
                    });
                }
            }
        };
        // Stable ID order. On an exact distance tie the first object/anchor wins.
        if self.enabled || self.temporary {
            for object in doc.objects() {
                if selection.contains(object.id()) {
                    continue;
                }
                let b = object.transform().bounds();
                for axis in 0..2 {
                    for target in anchors(b, axis) {
                        consider(axis, target, Some(object.id()));
                    }
                }
            }
        }
        if self.grid || self.temporary {
            // Same adaptive world lattice as visible dots, independent of visibility.
            let spacing = grid_spacing(zoom, dpi);
            for axis in 0..2 {
                for position in anchors(bounds, axis) {
                    consider(axis, (position / spacing).round() * spacing, None);
                }
            }
        }
        self.guides = best;
        std::array::from_fn(|axis| {
            best[axis].map_or(0., |s| s.target - anchors(bounds, axis)[s.anchor])
        })
    }
    pub fn guide_object(&self, axis: usize) -> Option<ObjectId> {
        self.guides.get(axis).and_then(|s| s.and_then(|s| s.object))
    }
}
