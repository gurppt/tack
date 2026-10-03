//! Pure metadata geometry. All pointer positions here are world coordinates.
use tack_core::{Crop, GeometryError, ImageRenderData, Transform, WorldRect};

pub const HANDLE_DIRECTIONS: [[f64; 2]; 8] = [
    [-1., -1.],
    [0., -1.],
    [1., -1.],
    [1., 0.],
    [1., 1.],
    [0., 1.],
    [-1., 1.],
    [-1., 0.],
];
pub fn rotate(p: [f64; 2], angle: f64) -> [f64; 2] {
    let (s, c) = angle.sin_cos();
    [c * p[0] - s * p[1], s * p[0] + c * p[1]]
}
pub fn local(t: Transform, world: [f64; 2]) -> [f64; 2] {
    rotate(
        [world[0] - t.center()[0], world[1] - t.center()[1]],
        -t.rotation(),
    )
}
pub fn world(t: Transform, local: [f64; 2]) -> [f64; 2] {
    let p = rotate(local, t.rotation());
    [p[0] + t.center()[0], p[1] + t.center()[1]]
}
pub fn hit(t: Transform, point: [f64; 2]) -> bool {
    if !point.iter().all(|v| v.is_finite()) {
        return false;
    }
    let p = local(t, point);
    p[0].abs() <= t.size()[0] / 2. && p[1].abs() <= t.size()[1] / 2.
}
pub fn corners(t: Transform) -> [[f64; 2]; 4] {
    [[-1., -1.], [1., -1.], [1., 1.], [-1., 1.]]
        .map(|d| world(t, [d[0] * t.size()[0] / 2., d[1] * t.size()[1] / 2.]))
}
/// SAT against a world-axis marquee, including rotated rectangles and edge contact.
pub fn intersects(t: Transform, r: WorldRect) -> bool {
    let a = corners(t);
    let b = [
        [r.x, r.y],
        [r.x + r.width, r.y],
        [r.x + r.width, r.y + r.height],
        [r.x, r.y + r.height],
    ];
    let axes = [
        [1., 0.],
        [0., 1.],
        rotate([1., 0.], t.rotation()),
        rotate([0., 1.], t.rotation()),
    ];
    axes.into_iter().all(|axis| {
        let interval = |points: [[f64; 2]; 4]| {
            points
                .into_iter()
                .map(|p| p[0] * axis[0] + p[1] * axis[1])
                .fold([f64::INFINITY, f64::NEG_INFINITY], |r, v| {
                    [r[0].min(v), r[1].max(v)]
                })
        };
        let x = interval(a);
        let y = interval(b);
        x[0] <= y[1] && y[0] <= x[1]
    })
}
pub fn frame(records: impl Iterator<Item = ImageRenderData>) -> Option<Transform> {
    let mut first = None;
    let mut bounds = [
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    ];
    let mut count = 0;
    for r in records {
        first = Some(r.transform);
        let b = r.transform.bounds();
        bounds = [
            bounds[0].min(b.x),
            bounds[1].min(b.y),
            bounds[2].max(b.x + b.width),
            bounds[3].max(b.y + b.height),
        ];
        count += 1;
    }
    if count == 1 {
        return first;
    }
    Transform::new(
        [(bounds[0] + bounds[2]) / 2., (bounds[1] + bounds[3]) / 2.],
        [bounds[2] - bounds[0], bounds[3] - bounds[1]],
        0.,
        [false; 2],
    )
    .ok()
}
/// Drag delta accounts for where inside a screen-space hit target the pointer began.
pub fn resize(
    t: Transform,
    delta: [f64; 2],
    direction: [f64; 2],
    center: bool,
    uniform: bool,
) -> Result<Transform, GeometryError> {
    let delta = rotate(delta, -t.rotation());
    let old = t.size();
    let multiplier = if center { 2. } else { 1. };
    let mut size = old;
    let corner = direction[0] != 0. && direction[1] != 0.;
    if corner || uniform {
        let vector = [direction[0] * old[0], direction[1] * old[1]];
        let divisor = vector[0] * vector[0] + vector[1] * vector[1];
        let scale =
            (1. + multiplier * (delta[0] * vector[0] + delta[1] * vector[1]) / divisor).max(1e-6);
        size = old.map(|s| s * scale);
    } else {
        for i in 0..2 {
            if direction[i] != 0. {
                size[i] = (old[i] + multiplier * delta[i] * direction[i]).max(old[i] * 1e-6);
            }
        }
    }
    let offset = if center {
        [0.; 2]
    } else {
        rotate(
            [
                direction[0] * (size[0] - old[0]) / 2.,
                direction[1] * (size[1] - old[1]) / 2.,
            ],
            t.rotation(),
        )
    };
    Transform::new(
        [t.center()[0] + offset[0], t.center()[1] + offset[1]],
        size,
        t.rotation(),
        t.flips(),
    )
}
/// Crop within the original source. Retained pixels keep their exact world mapping.
pub fn crop(
    data: ImageRenderData,
    delta: [f64; 2],
    direction: [f64; 2],
) -> Result<ImageRenderData, GeometryError> {
    let t = data.transform;
    let delta = rotate(delta, -t.rotation());
    let uv = data.crop.uv_rect();
    let mut lo = [0.; 2];
    let mut hi = [1.; 2];
    for i in 0..2 {
        let (min, max) = if t.flips()[i] {
            [
                -(1. - uv[i] - uv[i + 2]) / uv[i + 2],
                1. + uv[i] / uv[i + 2],
            ]
        } else {
            [-uv[i] / uv[i + 2], (1. - uv[i]) / uv[i + 2]]
        }
        .into();
        let change = delta[i] / t.size()[i];
        if direction[i] < 0. {
            lo[i] = change.clamp(min, 1. - 1e-6);
        }
        if direction[i] > 0. {
            hi[i] = (1. + change).clamp(1e-6, max);
        }
    }
    let mut next_uv = [0.; 4];
    for i in 0..2 {
        let start = if t.flips()[i] { 1. - hi[i] } else { lo[i] };
        next_uv[i] = (uv[i] + start * uv[i + 2]).clamp(0., 1. - 1e-12);
        next_uv[i + 2] = ((hi[i] - lo[i]) * uv[i + 2]).min(1. - next_uv[i]);
    }
    let shift = rotate(
        [
            (lo[0] + hi[0] - 1.) * t.size()[0] / 2.,
            (lo[1] + hi[1] - 1.) * t.size()[1] / 2.,
        ],
        t.rotation(),
    );
    Ok(ImageRenderData {
        transform: Transform::new(
            [t.center()[0] + shift[0], t.center()[1] + shift[1]],
            [t.size()[0] * (hi[0] - lo[0]), t.size()[1] * (hi[1] - lo[1])],
            t.rotation(),
            t.flips(),
        )?,
        crop: Crop::new(next_uv[0], next_uv[1], next_uv[2], next_uv[3])?,
        ..data
    })
}
pub fn angle(a: f64) -> f64 {
    (a + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI
}
