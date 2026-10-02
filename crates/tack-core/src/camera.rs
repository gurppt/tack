use std::{error::Error, fmt};

/// Axis-aligned rectangle in world units. Construction rejects invalid geometry.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct GeometryError;

impl fmt::Display for GeometryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("geometry must be finite, positive in size, and within world limits")
    }
}

impl Error for GeometryError {}

impl WorldRect {
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Result<Self, GeometryError> {
        if ![x, y, width, height].iter().all(|v| v.is_finite())
            || width <= 0.0
            || height <= 0.0
            || x.abs() + width > 1e9
            || y.abs() + height > 1e9
        {
            return Err(GeometryError);
        }
        Ok(Self {
            x,
            y,
            width,
            height,
        })
    }

    pub fn intersects(self, other: Self) -> bool {
        self.x < other.x + other.width
            && self.x + self.width > other.x
            && self.y < other.y + other.height
            && self.y + self.height > other.y
    }
}

/// Camera coordinates use f64; only camera-relative vertices become f32.
#[derive(Clone, Copy, Debug)]
pub struct Camera {
    center: [f64; 2],
    zoom: f64,
    screen: [u32; 2],
}

impl Camera {
    pub fn new(screen: [u32; 2]) -> Self {
        Self {
            center: [0.0; 2],
            zoom: 1.0,
            screen: screen.map(|s| s.max(1)),
        }
    }

    pub fn resize(&mut self, screen: [u32; 2]) {
        self.screen = screen.map(|s| s.max(1));
    }

    pub fn set_view(&mut self, center: [f64; 2], zoom: f64) -> Result<(), GeometryError> {
        if !center.iter().all(|v| v.is_finite() && v.abs() <= 1e8)
            || !zoom.is_finite()
            || zoom <= 0.0
        {
            return Err(GeometryError);
        }
        self.center = center;
        self.zoom = zoom.clamp(0.001, 64.0);
        Ok(())
    }

    pub fn zoom(&self) -> f64 {
        self.zoom
    }

    pub fn screen_to_world(&self, screen: [f64; 2]) -> [f64; 2] {
        [
            self.center[0] + (screen[0] - f64::from(self.screen[0]) / 2.0) / self.zoom,
            self.center[1] + (screen[1] - f64::from(self.screen[1]) / 2.0) / self.zoom,
        ]
    }

    pub fn world_to_clip(&self, world: [f64; 2]) -> [f32; 2] {
        [
            ((world[0] - self.center[0]) * self.zoom * 2.0 / f64::from(self.screen[0])) as f32,
            (-(world[1] - self.center[1]) * self.zoom * 2.0 / f64::from(self.screen[1])) as f32,
        ]
    }

    pub fn viewport(&self) -> WorldRect {
        let width = f64::from(self.screen[0]) / self.zoom;
        let height = f64::from(self.screen[1]) / self.zoom;
        WorldRect {
            x: self.center[0] - width / 2.0,
            y: self.center[1] - height / 2.0,
            width,
            height,
        }
    }

    pub fn pan(&mut self, delta_px: [f64; 2]) -> Result<(), GeometryError> {
        self.set_view(
            [
                self.center[0] - delta_px[0] / self.zoom,
                self.center[1] - delta_px[1] / self.zoom,
            ],
            self.zoom,
        )
    }

    /// Keeps the world point under the cursor fixed during zoom.
    pub fn zoom_at(&mut self, cursor: [f64; 2], factor: f64) -> Result<(), GeometryError> {
        if !cursor.iter().all(|v| v.is_finite()) || !factor.is_finite() || factor <= 0.0 {
            return Err(GeometryError);
        }
        let before = self.screen_to_world(cursor);
        let mut next = *self;
        next.set_view(self.center, self.zoom * factor)?;
        let after = next.screen_to_world(cursor);
        next.set_view(
            [
                next.center[0] + before[0] - after[0],
                next.center[1] + before[1] - after[1],
            ],
            next.zoom,
        )?;
        *self = next;
        Ok(())
    }
}
