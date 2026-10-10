//! Shape-aware selection; only resident metadata, never source or pixels.
use crate::image_geometry::{local, world};
use tack_core::{Annotation, AnnotationKind, Transform, WorldRect, segment_distance};

pub fn point(t: Transform, normalized: [f64; 2]) -> [f64; 2] {
    world(
        t,
        std::array::from_fn(|i| {
            (normalized[i] - 0.5) * t.size()[i] * if t.flips()[i] { -1. } else { 1. }
        }),
    )
}
pub fn arrow_head(a: [f64; 2], b: [f64; 2], width: f64) -> [[f64; 2]; 3] {
    let length = (b[0] - a[0]).hypot(b[1] - a[1]);
    let head = (width * 4. + 8.).clamp(8., 96.).min(length * 0.8);
    let v = if length > 0. {
        [(b[0] - a[0]) / length, (b[1] - a[1]) / length]
    } else {
        [1., 0.]
    };
    [
        b,
        [
            b[0] - v[0] * head - v[1] * head * 0.5,
            b[1] - v[1] * head + v[0] * head * 0.5,
        ],
        [
            b[0] - v[0] * head + v[1] * head * 0.5,
            b[1] - v[1] * head - v[0] * head * 0.5,
        ],
    ]
}
fn triangle_hit(p: [f64; 2], q: [[f64; 2]; 3]) -> bool {
    let cross =
        |a: [f64; 2], b: [f64; 2]| (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0]);
    let d = [cross(q[0], q[1]), cross(q[1], q[2]), cross(q[2], q[0])];
    d.iter().all(|v| *v >= 0.) || d.iter().all(|v| *v <= 0.)
}
pub fn hit(a: &Annotation, t: Transform, p: [f64; 2], tolerance: f64) -> bool {
    if !p.into_iter().all(f64::is_finite) || !tolerance.is_finite() || tolerance < 0. {
        return false;
    }
    let width = a.style().width() * 0.5 + tolerance;
    let lp = local(t, p).map(f64::abs);
    let half = t.size().map(|s| s * 0.5);
    match a.kind() {
        AnnotationKind::Text(_) => lp[0] <= half[0] + tolerance && lp[1] <= half[1] + tolerance,
        AnnotationKind::Rect => {
            let outer = lp[0] <= half[0] + width && lp[1] <= half[1] + width;
            outer
                && (a.style().fill().is_some()
                    || lp[0] >= half[0] - width
                    || lp[1] >= half[1] - width)
        }
        AnnotationKind::Line(l) | AnnotationKind::Arrow(l) => {
            let [start, end] = l.points().map(|p| point(t, p));
            segment_distance(p, start, end) <= width
                || (matches!(a.kind(), AnnotationKind::Arrow(_)) && {
                    let q = arrow_head(start, end, a.style().width());
                    triangle_hit(p, q)
                        || (0..3).any(|i| segment_distance(p, q[i], q[(i + 1) % 3]) <= tolerance)
                })
        }
        AnnotationKind::Scribble(s) => s.strokes().iter().any(|stroke| {
            stroke.points().windows(2).any(|q| {
                segment_distance(p, point(t, q[0]), point(t, q[1]))
                    <= stroke.style().unwrap_or(a.style()).width() * 0.5 + tolerance
            })
        }),
    }
}
fn segment_rect(a: [f64; 2], b: [f64; 2], r: WorldRect, radius: f64) -> bool {
    let lo = [r.x - radius, r.y - radius];
    let hi = [r.x + r.width + radius, r.y + r.height + radius];
    let mut min: f64 = 0.;
    let mut max: f64 = 1.;
    for axis in 0..2 {
        let delta = b[axis] - a[axis];
        if delta.abs() < 1e-12 {
            if a[axis] < lo[axis] || a[axis] > hi[axis] {
                return false;
            }
        } else {
            let p = (lo[axis] - a[axis]) / delta;
            let q = (hi[axis] - a[axis]) / delta;
            min = min.max(p.min(q));
            max = max.min(p.max(q));
        }
    }
    min <= max
}
pub fn intersects(a: &Annotation, t: Transform, r: WorldRect) -> bool {
    if !a.bounds(t).is_ok_and(|b| b.intersects(r)) {
        return false;
    }
    let radius = a.style().width() / 2.;
    match a.kind() {
        AnnotationKind::Text(_) => crate::image_geometry::intersects(t, r),
        AnnotationKind::Line(l) | AnnotationKind::Arrow(l) => {
            let [start, end] = l.points().map(|p| point(t, p));
            segment_rect(start, end, r, radius)
                || (matches!(a.kind(), AnnotationKind::Arrow(_)) && {
                    let q = arrow_head(start, end, a.style().width());
                    (0..3).any(|i| segment_rect(q[i], q[(i + 1) % 3], r, 0.))
                        || triangle_hit([r.x, r.y], q)
                })
        }
        AnnotationKind::Scribble(s) => s.strokes().iter().any(|stroke| {
            stroke.points().windows(2).any(|q| {
                segment_rect(
                    point(t, q[0]),
                    point(t, q[1]),
                    r,
                    stroke.style().unwrap_or(a.style()).width() / 2.,
                )
            })
        }),
        AnnotationKind::Rect => {
            if a.style().fill().is_some() {
                return crate::image_geometry::intersects(t, r);
            }
            let q = crate::image_geometry::corners(t);
            (0..4).any(|i| segment_rect(q[i], q[(i + 1) % 4], r, radius))
        }
    }
}

/// Plain note resizing changes only its wrapping field, around the opposite edge.
pub fn resize_note_box(
    t: Transform,
    delta: [f64; 2],
    direction: [f64; 2],
    center: bool,
) -> Result<Transform, tack_core::GeometryError> {
    let delta = crate::image_geometry::rotate(delta, -t.rotation());
    let old = t.size();
    let multiplier = if center { 2. } else { 1. };
    let size = std::array::from_fn(|i| {
        if direction[i] == 0. {
            old[i]
        } else {
            (old[i] + multiplier * delta[i] * direction[i]).max(old[i] * 1e-6)
        }
    });
    note_transform(t, size, if center { [0.; 2] } else { direction })
}
/// Global note scale keeps the opposite corner fixed, including rotated notes.
pub fn scale_note(
    t: Transform,
    scale: f64,
    direction: [f64; 2],
) -> Result<Transform, tack_core::GeometryError> {
    note_transform(t, t.size().map(|v| v * scale), direction)
}
fn note_transform(
    t: Transform,
    size: [f64; 2],
    direction: [f64; 2],
) -> Result<Transform, tack_core::GeometryError> {
    let offset = crate::image_geometry::rotate(
        std::array::from_fn(|i| direction[i] * (size[i] - t.size()[i]) / 2.),
        t.rotation(),
    );
    Transform::new(
        std::array::from_fn(|i| t.center()[i] + offset[i]),
        size,
        t.rotation(),
        t.flips(),
    )
}
