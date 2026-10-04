//! Atomic arrangement of selected images, frames and flat group units.
use std::collections::BTreeSet;
use tack_core::{Command, Document, GeometryError, ObjectId, Transform, WorldRect};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
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
    if units.len() < 2 {
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
