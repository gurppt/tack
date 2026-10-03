use crate::{
    AssetId, Crop, Document, DocumentObject, ImageAsset, ImageFiltering, ObjectId, ObjectKind,
    Opacity, Source, SourceId, Transform,
};
use std::{error::Error, fmt};

/// Metadata-only deterministic commands. No renderer/UI/storage payloads.
#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    /// Flat image edits only; one atomic operation and one inverse.
    Batch(Vec<Command>),
    AddSource(Source),
    RemoveSource(SourceId),
    /// Explicit revision/binding replacement; inverse retains the previous source.
    SetSource(Source),
    AddAsset(ImageAsset),
    RemoveAsset(AssetId),
    AddObject {
        object: DocumentObject,
        index: usize,
    },
    RemoveObject(ObjectId),
    SetTransform {
        object: ObjectId,
        transform: Transform,
    },
    SetCrop {
        object: ObjectId,
        crop: Crop,
    },
    SetOpacity {
        object: ObjectId,
        opacity: Opacity,
    },
    SetImageFiltering {
        object: ObjectId,
        filtering: ImageFiltering,
    },
    /// Final back-to-front index after removing the object's old position.
    SetZOrder {
        object: ObjectId,
        index: usize,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandError {
    DuplicateObject(ObjectId),
    DuplicateAsset(AssetId),
    DuplicateSource(SourceId),
    MissingObject(ObjectId),
    MissingAsset(AssetId),
    MissingSource(SourceId),
    AssetInUse(AssetId),
    SourceInUse(SourceId),
    SourceRevisionMustIncrease(SourceId),
    InvalidOrder { index: usize, len: usize },
    LimitReached(&'static str),
}
impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "document command rejected: {self:?}")
    }
}
impl Error for CommandError {}

fn replace<T: Copy + PartialEq>(field: &mut T, value: T) -> Option<T> {
    if *field == value {
        None
    } else {
        Some(std::mem::replace(field, value))
    }
}

impl Command {
    /// Conservative retained allocation accounting; no image payloads are commands.
    pub fn retained_bytes(&self) -> usize {
        let extra = match self {
            Self::Batch(edits) => edits.capacity() * std::mem::size_of::<Command>(),
            Self::AddSource(_) | Self::SetSource(_) => crate::MAX_SOURCE_PATH_BYTES,
            _ => 0,
        };
        std::mem::size_of::<Self>() + extra
    }
    fn image_edit(&self) -> bool {
        matches!(
            self,
            Self::AddObject { .. }
                | Self::RemoveObject(_)
                | Self::SetTransform { .. }
                | Self::SetCrop { .. }
                | Self::SetOpacity { .. }
                | Self::SetImageFiltering { .. }
                | Self::SetZOrder { .. }
        )
    }
}

impl Document {
    pub(crate) fn validate_source_revision(
        &self,
        command: &Command,
        high_water: u64,
    ) -> Result<(), CommandError> {
        if let Command::SetSource(source) = command {
            let previous = self
                .sources
                .get(&source.id())
                .ok_or(CommandError::MissingSource(source.id()))?;
            if previous != source && source.revision() <= high_water {
                return Err(CommandError::SourceRevisionMustIncrease(source.id()));
            }
        }
        Ok(())
    }

    /// Returns the inverse only when state changed. Validate before mutation.
    pub(crate) fn apply_reversible(
        &mut self,
        command: Command,
    ) -> Result<Option<Command>, CommandError> {
        use Command::*;
        let inverse = match command {
            Batch(edits) => {
                if edits.len() > 200_000 || edits.iter().any(|e| !e.image_edit()) {
                    return Err(CommandError::LimitReached("flat image batch"));
                }
                let mut inverses = Vec::with_capacity(edits.len());
                for edit in edits {
                    match self.apply_reversible(edit) {
                        Ok(Some(inverse)) => inverses.push(inverse),
                        Ok(None) => {}
                        Err(error) => {
                            // Reverse edits restore both records and ordered indices.
                            for inverse in inverses.into_iter().rev() {
                                self.apply_reversible(inverse)?;
                            }
                            return Err(error);
                        }
                    }
                }
                if inverses.is_empty() {
                    None
                } else {
                    inverses.reverse();
                    inverses.shrink_to_fit();
                    Some(Batch(inverses))
                }
            }
            AddSource(source) => {
                let id = source.id();
                if self.sources.contains_key(&id) {
                    return Err(CommandError::DuplicateSource(id));
                }
                if self.sources.len() >= self.limits.sources {
                    return Err(CommandError::LimitReached("sources"));
                }
                self.sources.insert(id, source);
                Some(RemoveSource(id))
            }
            SetSource(source) => {
                let id = source.id();
                let previous = self
                    .sources
                    .get(&id)
                    .ok_or(CommandError::MissingSource(id))?;
                if previous == &source {
                    None
                } else {
                    let previous = previous.clone();
                    self.sources.insert(id, source);
                    Some(SetSource(previous))
                }
            }
            RemoveSource(id) => {
                if !self.sources.contains_key(&id) {
                    return Err(CommandError::MissingSource(id));
                }
                if self.assets.values().any(|a| a.source_id() == id) {
                    return Err(CommandError::SourceInUse(id));
                }
                self.sources.remove(&id).map(AddSource)
            }
            AddAsset(asset) => {
                let id = asset.id();
                if self.assets.contains_key(&id) {
                    return Err(CommandError::DuplicateAsset(id));
                }
                if !self.sources.contains_key(&asset.source_id()) {
                    return Err(CommandError::MissingSource(asset.source_id()));
                }
                if self.assets.len() >= self.limits.assets {
                    return Err(CommandError::LimitReached("assets"));
                }
                self.assets.insert(id, asset);
                Some(RemoveAsset(id))
            }
            RemoveAsset(id) => {
                if !self.assets.contains_key(&id) {
                    return Err(CommandError::MissingAsset(id));
                }
                if self.objects.values().any(|o| match o.kind {
                    ObjectKind::Image(image) => image.asset_id() == id,
                }) {
                    return Err(CommandError::AssetInUse(id));
                }
                self.assets.remove(&id).map(AddAsset)
            }
            AddObject { object, index } => {
                let id = object.id();
                if self.objects.contains_key(&id) {
                    return Err(CommandError::DuplicateObject(id));
                }
                let ObjectKind::Image(image) = object.kind;
                if !self.assets.contains_key(&image.asset_id()) {
                    return Err(CommandError::MissingAsset(image.asset_id()));
                }
                if index > self.order.len() {
                    return Err(CommandError::InvalidOrder {
                        index,
                        len: self.order.len(),
                    });
                }
                if self.objects.len() >= self.limits.objects {
                    return Err(CommandError::LimitReached("objects"));
                }
                self.order.insert(index, id);
                self.objects.insert(id, object);
                Some(RemoveObject(id))
            }
            RemoveObject(id) => {
                let index = self.position(id)?;
                let object = self
                    .objects
                    .remove(&id)
                    .ok_or(CommandError::MissingObject(id))?;
                self.order.remove(index);
                Some(AddObject { object, index })
            }
            SetTransform { object, transform } => {
                let target = self
                    .objects
                    .get_mut(&object)
                    .ok_or(CommandError::MissingObject(object))?;
                replace(&mut target.transform, transform)
                    .map(|transform| SetTransform { object, transform })
            }
            SetCrop { object, crop } => {
                let target = self
                    .objects
                    .get_mut(&object)
                    .ok_or(CommandError::MissingObject(object))?
                    .image_mut();
                replace(&mut target.crop, crop).map(|crop| SetCrop { object, crop })
            }
            SetOpacity { object, opacity } => {
                let target = self
                    .objects
                    .get_mut(&object)
                    .ok_or(CommandError::MissingObject(object))?
                    .image_mut();
                replace(&mut target.opacity, opacity).map(|opacity| SetOpacity { object, opacity })
            }
            SetImageFiltering { object, filtering } => {
                let target = self
                    .objects
                    .get_mut(&object)
                    .ok_or(CommandError::MissingObject(object))?
                    .image_mut();
                replace(&mut target.filtering, filtering)
                    .map(|filtering| SetImageFiltering { object, filtering })
            }
            SetZOrder { object, index } => {
                let previous = self.position(object)?;
                if index >= self.order.len() {
                    return Err(CommandError::InvalidOrder {
                        index,
                        len: self.order.len(),
                    });
                }
                if index == previous {
                    None
                } else {
                    self.order.remove(previous);
                    self.order.insert(index, object);
                    Some(SetZOrder {
                        object,
                        index: previous,
                    })
                }
            }
        };
        Ok(inverse)
    }
    fn position(&self, id: ObjectId) -> Result<usize, CommandError> {
        self.order
            .iter()
            .position(|object| *object == id)
            .ok_or(CommandError::MissingObject(id))
    }
}
