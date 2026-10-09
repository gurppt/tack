use crate::{BookmarkId, Camera, GeometryError};

pub const MAX_CAMERA_BOOKMARKS: usize = 64;
pub const MAX_BOOKMARK_NAME_BYTES: usize = 128;

/// An invisible durable view, independent of canvas objects and frames.
#[derive(Clone, Debug, PartialEq)]
pub struct CameraBookmark {
    id: BookmarkId,
    name: String,
    center: [f64; 2],
    zoom: f64,
}
impl CameraBookmark {
    pub fn new(
        id: BookmarkId,
        name: String,
        center: [f64; 2],
        zoom: f64,
    ) -> Result<Self, GeometryError> {
        if name.trim().is_empty()
            || name.len() > MAX_BOOKMARK_NAME_BYTES
            || name.chars().any(char::is_control)
            || !center.iter().all(|v| v.is_finite() && v.abs() <= 1e8)
            || !zoom.is_finite()
            || !(0.001..=64.).contains(&zoom)
        {
            return Err(GeometryError);
        }
        Ok(Self {
            id,
            name,
            center,
            zoom,
        })
    }
    pub fn id(&self) -> BookmarkId {
        self.id
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn center(&self) -> [f64; 2] {
        self.center
    }
    pub fn zoom(&self) -> f64 {
        self.zoom
    }
    pub fn jump(&self, camera: &mut Camera) -> Result<(), GeometryError> {
        camera.set_view(self.center, self.zoom)
    }
    pub fn retained_bytes(&self) -> usize {
        std::mem::size_of::<Self>() + self.name.capacity()
    }
}
