use crate::{GeometryError, WorldRect};

/// Durable image box in world units. Positive size is independent of source
/// pixels. Rotation is clockwise radians in y-down world space, around center.
/// Crop selects UVs within this box; it does not implicitly resize the box.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    center: [f64; 2],
    size: [f64; 2],
    rotation: f64,
    flips: [bool; 2],
    bounds: WorldRect,
}

impl Transform {
    pub fn new(
        center: [f64; 2],
        size: [f64; 2],
        rotation: f64,
        flips: [bool; 2],
    ) -> Result<Self, GeometryError> {
        if !center
            .into_iter()
            .chain(size)
            .chain([rotation])
            .all(f64::is_finite)
            || size.iter().any(|s| *s <= 0.0)
        {
            return Err(GeometryError);
        }
        let (sin, cos) = rotation.sin_cos();
        let width = cos.abs() * size[0] + sin.abs() * size[1];
        let height = sin.abs() * size[0] + cos.abs() * size[1];
        let bounds = WorldRect::new(
            center[0] - width / 2.0,
            center[1] - height / 2.0,
            width,
            height,
        )?;
        Ok(Self {
            center,
            size,
            rotation,
            flips,
            bounds,
        })
    }
    pub fn center(self) -> [f64; 2] {
        self.center
    }
    pub fn size(self) -> [f64; 2] {
        self.size
    }
    pub fn rotation(self) -> f64 {
        self.rotation
    }
    pub fn flips(self) -> [bool; 2] {
        self.flips
    }
    /// Conservative rotated bounds computed once when geometry changes.
    pub fn bounds(self) -> WorldRect {
        self.bounds
    }
}

/// Positive normalized source UV rectangle within [0, 1].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Crop([f64; 4]);
impl Crop {
    pub const FULL: Self = Self([0.0, 0.0, 1.0, 1.0]);
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Result<Self, GeometryError> {
        if ![x, y, width, height].into_iter().all(f64::is_finite)
            || x < 0.0
            || y < 0.0
            || width <= 0.0
            || height <= 0.0
            || x + width > 1.0
            || y + height > 1.0
        {
            return Err(GeometryError);
        }
        Ok(Self([x, y, width, height]))
    }
    pub fn uv_rect(self) -> [f64; 4] {
        self.0
    }
}

/// Validated non-destructive alpha, independent of source pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Opacity(f64);
impl Opacity {
    pub const OPAQUE: Self = Self(1.0);
    pub fn new(value: f64) -> Result<Self, GeometryError> {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(GeometryError);
        }
        Ok(Self(value))
    }
    pub fn value(self) -> f64 {
        self.0
    }
}
