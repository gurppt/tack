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
    pub(crate) groups: BTreeMap<crate::GroupId, crate::Group>,
    pub(crate) bookmarks: Vec<crate::CameraBookmark>,
    pub(crate) frame_count: usize,
    pub(crate) annotation_count: usize,
    pub(crate) frame_links: BTreeMap<ObjectId, ObjectId>,
    pub(crate) frame_children: BTreeMap<ObjectId, std::collections::BTreeSet<ObjectId>>,
    pub(crate) memberships: BTreeMap<ObjectId, crate::GroupId>,
}
impl Document {
    /// Independent document incarnation; object/source identities are retained.
    pub fn fork(&self, id: DocumentId) -> Self {
        let mut copy = self.clone();
        copy.id = id;
        copy
    }

    pub fn new(id: DocumentId, limits: DocumentLimits) -> Self {
        Self {
            id,
            limits,
            objects: BTreeMap::new(),
            assets: BTreeMap::new(),
            sources: BTreeMap::new(),
            order: Vec::new(),
            groups: BTreeMap::new(),
            memberships: BTreeMap::new(),
            frame_links: BTreeMap::new(),
            frame_children: BTreeMap::new(),
            bookmarks: Vec::new(),
            frame_count: 0,
            annotation_count: 0,
        }
    }
    pub fn bookmarks(&self) -> &[crate::CameraBookmark] {
        &self.bookmarks
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
    pub fn objects(&self) -> impl Iterator<Item = &DocumentObject> {
        self.objects.values()
    }
    pub fn frame_count(&self) -> usize {
        self.frame_count
    }
    pub fn annotation_count(&self) -> usize {
        self.annotation_count
    }
    pub fn groups(&self) -> impl Iterator<Item = &crate::Group> {
        self.groups.values()
    }
    pub fn group(&self, id: crate::GroupId) -> Option<&crate::Group> {
        self.groups.get(&id)
    }
    pub fn group_for(&self, id: ObjectId) -> Option<&crate::Group> {
        self.memberships.get(&id).and_then(|id| self.groups.get(id))
    }
    /// Apply a single atomic deterministic mutation without retaining history.
    /// Editors use their own execute API so undo cannot become inconsistent.
    /// Validated authoritative mutation returning its metadata inverse. Callers must
    /// separately enforce revision/conflict policy before replaying an inverse.
    pub fn apply_with_inverse(
        &mut self,
        command: Command,
    ) -> Result<Option<Command>, CommandError> {
        let high_water = if command.source_revision() > 0 {
            self.sources().map(|s| s.revision()).max().unwrap_or(0)
        } else {
            0
        };
        self.validate_source_revision(&command, high_water)?;
        self.apply_reversible(command)
    }
    /// Apply a previously produced inverse after external conflict validation.
    /// Allows restoring an earlier source binding while its authority remains monotonic.
    pub fn apply_inverse(&mut self, inverse: Command) -> Result<Option<Command>, CommandError> {
        self.apply_reversible(inverse)
    }
    pub fn apply(&mut self, command: Command) -> Result<bool, CommandError> {
        Ok(self.apply_with_inverse(command)?.is_some())
    }
}
