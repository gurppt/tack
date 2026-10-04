use crate::{
    AssetId, Crop, Document, ImageFiltering, ObjectId, ObjectKind, Opacity, Transform, WorldRect,
};

/// Copyable render metadata. Source location, bytes and cache handles stay outside.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ImageRenderData {
    pub object_id: ObjectId,
    pub asset_id: AssetId,
    pub transform: Transform,
    pub crop: Crop,
    pub opacity: Opacity,
    pub filtering: ImageFiltering,
}

/// Immutable resident geometry boundary. Implementations must not perform I/O,
/// decoding, hashing, locks or worker waits. Visibility may conservatively return
/// rotated bounding-box intersections, in back-to-front order.
pub trait DocumentQuery {
    fn object_render_data(&self, id: ObjectId) -> Option<ImageRenderData>;
    fn objects_in_view(&self, viewport: WorldRect) -> impl Iterator<Item = ImageRenderData>;
}
impl DocumentQuery for Document {
    fn object_render_data(&self, id: ObjectId) -> Option<ImageRenderData> {
        let object = self.object(id)?;
        let ObjectKind::Image(image) = object.kind() else {
            return None;
        };
        Some(ImageRenderData {
            object_id: id,
            asset_id: image.asset_id(),
            transform: object.transform(),
            crop: image.crop(),
            opacity: image.opacity(),
            filtering: image.filtering(),
        })
    }
    /// Allocation-free ordered linear scan with O(log n) ID lookups; no spatial
    /// index in this slice. A future measured index can sit behind this contract.
    fn objects_in_view(&self, viewport: WorldRect) -> impl Iterator<Item = ImageRenderData> {
        self.object_order()
            .iter()
            .filter_map(move |id| self.object_render_data(*id))
            .filter(move |data| data.transform.bounds().intersects(viewport))
    }
}
