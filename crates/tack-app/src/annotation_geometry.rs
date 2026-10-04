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
    let head = (width * 4. + 8.).clamp(6., 64.).min(length * 0.35);
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
        AnnotationKind::Ellipse => {
            let inside = (lp[0] / half[0]).hypot(lp[1] / half[1]) <= 1.;
            (a.style().fill().is_some() && inside) || ellipse_distance(lp, half) <= width
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
        AnnotationKind::Scribble(s) => s
            .points()
            .windows(2)
            .any(|q| segment_distance(p, point(t, q[0]), point(t, q[1])) <= width),
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
        AnnotationKind::Scribble(s) => s
            .points()
            .windows(2)
            .any(|q| segment_rect(point(t, q[0]), point(t, q[1]), r, radius)),
        AnnotationKind::Rect => {
            if a.style().fill().is_some() {
                return crate::image_geometry::intersects(t, r);
            }
            let q = crate::image_geometry::corners(t);
            (0..4).any(|i| segment_rect(q[i], q[(i + 1) % 4], r, radius))
        }
        AnnotationKind::Ellipse => {
            let p = |i: usize| {
                let angle = i as f64 * std::f64::consts::TAU / 64.;
                world(
                    t,
                    [
                        angle.cos() * t.size()[0] / 2.,
                        angle.sin() * t.size()[1] / 2.,
                    ],
                )
            };
            (0..64).any(|i| segment_rect(p(i), p(i + 1), r, radius))
                || (a.style().fill().is_some()
                    && (hit(a, t, [r.x, r.y], 0.)
                        || (t.center()[0] >= r.x
                            && t.center()[0] <= r.x + r.width
                            && t.center()[1] >= r.y
                            && t.center()[1] <= r.y + r.height)))
        }
    }
}

/// Closest boundary point from the ellipse Lagrange multiplier, bounded bisection.
/// Major-axis special case avoids the interior singular root; distance is unsigned.
pub fn ellipse_distance(mut p: [f64; 2], mut radii: [f64; 2]) -> f64 {
    if radii[0] < radii[1] {
        p.swap(0, 1);
        radii.swap(0, 1);
    }
    let scale = radii[0];
    let b = radii[1] / scale;
    let x = p[0].abs() / scale;
    let y = p[1].abs() / scale;
    if b > 0.999999 {
        return (x.hypot(y) - 1.).abs() * scale;
    }
    if y <= 1e-7 {
        let q = (x / (1. - b * b)).min(1.);
        return (x - q).hypot(y - b * (1. - q * q).max(0.).sqrt()) * scale;
    }
    let inside = x * x + y * y / (b * b) <= 1.;
    let mut lo = if inside { b * (y - b) } else { 0. };
    let mut hi = if inside { 0. } else { x + b * y };
    for _ in 0..48 {
        let mid = (lo + hi) / 2.;
        let f = (x / (mid + 1.)).powi(2) + (b * y / (mid + b * b)).powi(2);
        if f > 1. {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let q = (lo + hi) / 2.;
    (x - x / (q + 1.)).hypot(y - b * b * y / (q + b * b)) * scale
}
