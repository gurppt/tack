//! A bounded ordered bounds memo for measured large-board scans, not a spatial index.
use tack_core::{Document, DocumentId, DocumentQuery, ObjectId, WorldRect};
#[derive(Default)]
pub struct Visibility {
    identity: Option<(DocumentId, u64)>,
    rows: Vec<(ObjectId, WorldRect)>,
}
impl Visibility {
    pub fn refresh(&mut self, document: &Document, generation: u64) {
        let identity = (document.id(), generation);
        if self.identity == Some(identity) {
            return;
        }
        self.identity = Some(identity);
        self.rows.clear();
        // Below this measured threshold the allocation-free document scan is cheaper.
        if document.object_order().len() <= 4096 {
            self.rows.shrink_to_fit();
            return;
        }
        // The document already has a hard object cap; never enlarge that authority.
        self.rows
            .extend(document.object_order().iter().filter_map(|id| {
                document
                    .object_render_data(*id)
                    .map(|d| (*id, d.transform.bounds()))
            }));
    }
    pub fn invalidate(&mut self) {
        self.identity = None;
    }
    pub fn cached(&self) -> bool {
        !self.rows.is_empty()
    }
    pub fn candidates<'a>(
        &'a self,
        view: WorldRect,
        selected: impl Fn(ObjectId) -> bool + 'a,
    ) -> impl Iterator<Item = ObjectId> + 'a {
        self.rows
            .iter()
            .filter(move |(id, b)| b.intersects(view) || selected(*id))
            .map(|(id, _)| *id)
    }
    pub fn bytes(&self) -> usize {
        self.rows.capacity() * std::mem::size_of::<(ObjectId, WorldRect)>()
    }
}
#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use tack_core::*;
    #[test]
    fn memo_preserves_order_selection_entry_and_generation_invalidation() {
        let mut d = Document::new(DocumentId::new(1).unwrap(), DocumentLimits::default());
        d.apply(Command::AddSource(
            Source::linked(SourceId::new(1).unwrap(), "missing.png").unwrap(),
        ))
        .unwrap();
        d.apply(Command::AddAsset(
            ImageAsset::new(
                AssetId::new(1).unwrap(),
                SourceId::new(1).unwrap(),
                [10, 10],
            )
            .unwrap(),
        ))
        .unwrap();
        for i in 0..5000 {
            d.apply(Command::AddObject {
                object: DocumentObject::image(
                    ObjectId::new(i + 1).unwrap(),
                    AssetId::new(1).unwrap(),
                    Transform::new([i as f64 * 100., 0.], [10., 10.], 0., [false; 2]).unwrap(),
                ),
                index: i as usize,
            })
            .unwrap();
        }
        let mut v = Visibility::default();
        v.refresh(&d, 0);
        let view = WorldRect::new(-10., -10., 200., 20.).unwrap();
        let expected: Vec<_> = d.objects_in_view(view).map(|x| x.object_id).collect();
        assert_eq!(v.candidates(view, |_| false).collect::<Vec<_>>(), expected);
        let moved = ObjectId::new(5000).unwrap();
        assert!(v.candidates(view, |id| id == moved).any(|id| id == moved));
        d.apply(Command::SetTransform {
            object: moved,
            transform: Transform::new([0., 0.], [10., 10.], 0., [false; 2]).unwrap(),
        })
        .unwrap();
        v.refresh(&d, 1);
        assert_eq!(v.candidates(view, |_| false).last(), Some(moved));
        assert!(v.bytes() < 5000 * 64 * 2);
    }
}
