use crate::{
    AssetId, Command, CommandError, DocumentId, DocumentObject, ImageAsset, ObjectId, Source,
    SourceId,
};
use std::collections::BTreeMap;

/// Metadata bounds are independent from source byte sizes or working caches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DocumentLimits {
    pub objects: usize,
    pub assets: usize,
    pub sources: usize,
}
impl Default for DocumentLimits {
    fn default() -> Self {
        Self {
            objects: 100_000,
            assets: 100_000,
            sources: 100_000,
        }
    }
}

/// Durable state only. Mutation is through commands; no table/field mutable access.
/// Equality includes exact ordering, identities and metadata, not undo history.
#[derive(Clone, Debug, PartialEq)]
pub struct Document {
    id: DocumentId,
    pub(crate) limits: DocumentLimits,
    pub(crate) objects: BTreeMap<ObjectId, DocumentObject>,
    pub(crate) assets: BTreeMap<AssetId, ImageAsset>,
    pub(crate) sources: BTreeMap<SourceId, Source>,
    pub(crate) order: Vec<ObjectId>,
}
impl Document {
    pub fn new(id: DocumentId, limits: DocumentLimits) -> Self {
        Self {
            id,
            limits,
            objects: BTreeMap::new(),
            assets: BTreeMap::new(),
            sources: BTreeMap::new(),
            order: Vec::new(),
        }
    }
    pub fn id(&self) -> DocumentId {
        self.id
    }
    pub fn limits(&self) -> DocumentLimits {
        self.limits
    }
    pub fn object(&self, id: ObjectId) -> Option<&DocumentObject> {
        self.objects.get(&id)
    }
    pub fn asset(&self, id: AssetId) -> Option<&ImageAsset> {
        self.assets.get(&id)
    }
    pub fn source(&self, id: SourceId) -> Option<&Source> {
        self.sources.get(&id)
    }
    /// Back-to-front order. Slots are ordering positions, never identities.
    pub fn object_order(&self) -> &[ObjectId] {
        &self.order
    }
    pub fn assets(&self) -> impl Iterator<Item = &ImageAsset> {
        self.assets.values()
    }
    pub fn sources(&self) -> impl Iterator<Item = &Source> {
        self.sources.values()
    }
    /// Apply a single atomic deterministic mutation without retaining history.
    /// Editors use their own execute API so undo cannot become inconsistent.
    pub fn apply(&mut self, command: Command) -> Result<bool, CommandError> {
        Ok(self.apply_reversible(command)?.is_some())
    }
}
