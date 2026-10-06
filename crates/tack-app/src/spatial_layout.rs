//! Atomic arrangement of selected images, frames and flat group units.
use std::collections::BTreeSet;
use tack_core::{Command, Document, GeometryError, ObjectId, Transform, WorldRect};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    Grid,
    SnapToGrid,
    Left,
    HorizontalCenter,
    Right,
    Top,
    VerticalCenter,
    Bottom,
    DistributeHorizontal,
    DistributeVertical,
    PackHorizontal,
    PackVertical,
}
struct Unit {
    key: ObjectId,
    ids: Vec<ObjectId>,
    bounds: WorldRect,
}
pub fn bounds(transforms: impl Iterator<Item = Transform>) -> Option<WorldRect> {
    transforms.map(Transform::bounds).reduce(|a, b| WorldRect {
        x: a.x.min(b.x),
        y: a.y.min(b.y),
        width: (a.x + a.width).max(b.x + b.width) - a.x.min(b.x),
        height: (a.y + a.height).max(b.y + b.height) - a.y.min(b.y),
    })
}
pub fn arrange(
    doc: &Document,
    ids: impl Iterator<Item = ObjectId>,
    layout: Layout,
) -> Result<Command, GeometryError> {
    let mut seen: BTreeSet<ObjectId> = BTreeSet::new();
    let mut units = Vec::new();
    for id in ids {
        if seen.contains(&id) {
            continue;
        }
        let members = doc
            .group_for(id)
            .map_or_else(|| vec![id], |g| g.members().to_vec());
        seen.extend(members.iter().copied());
        let Some(b) = bounds(
            members
                .iter()
                .filter_map(|id| doc.object(*id))
                .map(|o| o.transform()),
        ) else {
            continue;
        };
        units.push(Unit {
            key: members[0],
            ids: members,
            bounds: b,
        });
    }
    if units.is_empty() || (units.len() < 2 && layout != Layout::SnapToGrid) {
        return Ok(Command::Batch(Vec::new()));
    }
    let all = bounds(units.iter().filter_map(|u| {
        Transform::new(
            [
                u.bounds.x + u.bounds.width / 2.,
                u.bounds.y + u.bounds.height / 2.,
            ],
            [u.bounds.width, u.bounds.height],
            0.,
            [false; 2],
        )
        .ok()
    }))
    .ok_or(GeometryError)?;
    if matches!(layout, Layout::Grid | Layout::SnapToGrid) {
        units.sort_by(|a, b| {
            a.bounds
                .y
                .total_cmp(&b.bounds.y)
                .then(a.bounds.x.total_cmp(&b.bounds.x))
                .then(a.key.cmp(&b.key))
        });
        let columns = if layout == Layout::Grid {
            grid_columns(&units, all)
        } else {
            1
        };
        let mut cursor = [all.x, all.y];
        let mut row_height: f64 = 0.;
        let mut edits = Vec::with_capacity(seen.len());
        for (i, unit) in units.into_iter().enumerate() {
            let next = if layout == Layout::SnapToGrid {
                // Snap each independent unit's AABB top-left to the base lattice.
                // No DPI-dependent/adaptive spacing or implied auto-layout.
                [unit.bounds.x, unit.bounds.y].map(|v| {
                    (v / crate::spatial_snap::GRID_BASE).round() * crate::spatial_snap::GRID_BASE
                })
            } else {
                if i > 0 && i % columns == 0 {
                    cursor = [all.x, cursor[1] + row_height + GRID_GAP];
                    row_height = 0.;
                }
                let next = cursor;
                cursor[0] += unit.bounds.width + GRID_GAP;
                row_height = row_height.max(unit.bounds.height);
                next
            };
            let delta = [next[0] - unit.bounds.x, next[1] - unit.bounds.y];
            if delta == [0.; 2] {
                continue;
            }
            for id in unit.ids {
                let t = doc.object(id).ok_or(GeometryError)?.transform();
                edits.push(Command::SetTransform {
                    object: id,
                    transform: Transform::new(
                        [t.center()[0] + delta[0], t.center()[1] + delta[1]],
                        t.size(),
                        t.rotation(),
                        t.flips(),
                    )?,
                });
            }
        }
        return Ok(Command::Batch(edits));
    }
    let axis = usize::from(matches!(
        layout,
        Layout::Top
            | Layout::VerticalCenter
            | Layout::Bottom
            | Layout::DistributeVertical
            | Layout::PackVertical
    ));
    let interval = |b: WorldRect| {
        if axis == 0 {
            [b.x, b.width]
        } else {
            [b.y, b.height]
        }
    };
    let [start, span] = interval(all);
    let distribution = matches!(
        layout,
        Layout::DistributeHorizontal | Layout::DistributeVertical
    );
    let pack = matches!(layout, Layout::PackHorizontal | Layout::PackVertical);
    if distribution || pack {
        units.sort_by(|a, b| {
            interval(a.bounds)[0]
                .total_cmp(&interval(b.bounds)[0])
                .then(a.key.cmp(&b.key))
        });
    }
    let gap = if pack {
        16.
    } else {
        (span - units.iter().map(|u| interval(u.bounds)[1]).sum::<f64>()) / (units.len() - 1) as f64
    };
    let gap = gap.max(0.);
    let mut cursor = start;
    let mut edits = Vec::with_capacity(seen.len());
    for unit in units {
        let [old, size] = interval(unit.bounds);
        let next = match layout {
            Layout::Left | Layout::Top => start,
            Layout::Right | Layout::Bottom => start + span - size,
            Layout::HorizontalCenter | Layout::VerticalCenter => start + (span - size) / 2.,
            _ => {
                let next = cursor;
                cursor += size + gap;
                next
            }
        };
        for id in unit.ids {
            let t = doc.object(id).ok_or(GeometryError)?.transform();
            let mut center = t.center();
            center[axis] += next - old;
            if pack {
                center[1 - axis] += if axis == 0 {
                    all.y - unit.bounds.y
                } else {
                    all.x - unit.bounds.x
                };
            }
            edits.push(Command::SetTransform {
                object: id,
                transform: Transform::new(center, t.size(), t.rotation(), t.flips())?,
            });
        }
    }
    Ok(Command::Batch(edits))
}

/// Fixed world-space visual gap; independent of window, zoom and UI scale.
pub const GRID_GAP: f64 = 16.;
fn grid_columns(units: &[Unit], selection: WorldRect) -> usize {
    let n = units.len();
    if n == 2 {
        return 2;
    }
    let target = (selection.width / selection.height).clamp(2. / 3., 1.75);
    let width = units.iter().map(|u| u.bounds.width).sum::<f64>();
    let height = units.iter().map(|u| u.bounds.height).sum::<f64>();
    let area = units
        .iter()
        .map(|u| u.bounds.width * u.bounds.height)
        .sum::<f64>();
    let estimate =
        ((n as f64 * target * height / width).sqrt().round() as usize).clamp(2, n.div_ceil(2));
    let square = (n as f64).sqrt() as usize;
    // Five candidates, each a linear shelf measurement. No solver/pairwise scan.
    let candidates = [
        estimate.saturating_sub(1),
        estimate,
        estimate + 1,
        square,
        square + 1,
    ];
    let mut best = (f64::INFINITY, estimate);
    for columns in candidates.map(|c| c.clamp(2, n.div_ceil(2))) {
        let mut max_width: f64 = 0.;
        let mut total_height = 0.;
        for row in units.chunks(columns) {
            max_width = max_width.max(
                row.iter().map(|u| u.bounds.width).sum::<f64>() + (row.len() - 1) as f64 * GRID_GAP,
            );
            total_height += row.iter().map(|u| u.bounds.height).fold(0., f64::max) + GRID_GAP;
        }
        total_height -= GRID_GAP;
        let score = max_width * total_height / area
            + 0.5 * ((max_width / total_height) / target).ln().abs();
        if score < best.0 || (score == best.0 && columns < best.1) {
            best = (score, columns);
        }
    }
    best.1
}
