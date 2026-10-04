//! Iterative deterministic Ramer–Douglas–Peucker, bounded by stroke admission.
use crate::{MAX_STROKE_POINTS, ModelError};
pub fn segment_distance(point: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let v = [b[0] - a[0], b[1] - a[1]];
    let length = v[0] * v[0] + v[1] * v[1];
    let t = if length == 0. {
        0.
    } else {
        (((point[0] - a[0]) * v[0] + (point[1] - a[1]) * v[1]) / length).clamp(0., 1.)
    };
    (point[0] - a[0] - t * v[0]).hypot(point[1] - a[1] - t * v[1])
}
pub fn simplify_stroke(points: &[[f64; 2]], tolerance: f64) -> Result<Vec<[f64; 2]>, ModelError> {
    if !(2..=MAX_STROKE_POINTS).contains(&points.len())
        || !tolerance.is_finite()
        || tolerance < 0.
        || !points
            .iter()
            .flatten()
            .all(|v| v.is_finite() && v.abs() <= 1e9)
    {
        return Err(ModelError::InvalidAnnotation);
    }
    let mut keep = vec![false; points.len()];
    keep[0] = true;
    keep[points.len() - 1] = true;
    let mut stack = vec![(0, points.len() - 1)];
    while let Some((a, b)) = stack.pop() {
        let mut farthest = None;
        let mut distance = tolerance;
        for i in a + 1..b {
            let d = segment_distance(points[i], points[a], points[b]);
            if d > distance {
                distance = d;
                farthest = Some(i);
            }
        }
        if let Some(i) = farthest {
            keep[i] = true;
            stack.push((i, b));
            stack.push((a, i));
        }
    }
    Ok(points
        .iter()
        .zip(keep)
        .filter_map(|(p, k)| k.then_some(*p))
        .collect())
}
