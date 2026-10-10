use crate::{
    AssetId, Crop, Document, DocumentObject, ImageAsset, ImageFiltering, ObjectId, ObjectKind,
    Opacity, Source, SourceId, Transform,
};
use std::{error::Error, fmt};

/// Metadata-only deterministic commands. No renderer/UI/storage payloads.
#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    /// Flat bounded object edits; one atomic operation and one inverse.
    Batch(Vec<Command>),
    SetCameraBookmarks(Vec<crate::CameraBookmark>),
    /// Atomically change flat child-to-Frame relations; groups must stay uniform.
    SetFrameLinks(Vec<(ObjectId, Option<ObjectId>)>),
    AddSource(Source),
    RemoveSource(SourceId),
    /// Explicit revision/binding replacement; inverse retains the previous source.
    SetSource(Source),
    AddAsset(ImageAsset),
    RemoveAsset(AssetId),
    SetAsset(ImageAsset),
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
    AddGroup(crate::Group),
    RemoveGroup(crate::GroupId),
    SetFrameColor {
        object: ObjectId,
        color: crate::Color,
    },
    SetFrameName {
        object: ObjectId,
        name: String,
    },
    SetAnnotation {
        object: ObjectId,
        annotation: crate::Annotation,
    },
    SetAnnotationStyle {
        object: ObjectId,
        style: crate::AnnotationStyle,
    },
    SetText {
        object: ObjectId,
        text: crate::TextObject,
    },
    SetZOrder {
        object: ObjectId,
        index: usize,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandError {
    InvalidGroup,
    InvalidFrameLink,
    ObjectInGroup(ObjectId),
    WrongObjectKind(ObjectId),
    InvalidFrame,
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
            Self::Batch(edits) => {
                edits.capacity() * std::mem::size_of::<Command>()
                    + edits
                        .iter()
                        .map(|e| {
                            e.retained_bytes()
                                .saturating_sub(std::mem::size_of::<Command>())
                        })
                        .sum::<usize>()
            }
            Self::SetCameraBookmarks(views) => {
                views.capacity() * std::mem::size_of::<crate::CameraBookmark>()
                    + views
                        .iter()
                        .map(|b| b.retained_bytes() - std::mem::size_of::<crate::CameraBookmark>())
                        .sum::<usize>()
            }
            Self::AddGroup(g) => g.retained_bytes(),
            Self::SetAnnotation { annotation, .. } => annotation.retained_bytes(),
            Self::SetFrameLinks(links) => {
                links.capacity() * std::mem::size_of::<(ObjectId, Option<ObjectId>)>()
            }
            Self::SetFrameName { name, .. } => name.capacity(),
            Self::SetText { text, .. } => text.retained_bytes(),
            Self::AddObject { object, .. } => match object.kind() {
                ObjectKind::Frame(name) => name.capacity(),
                ObjectKind::Annotation(a) => a.retained_bytes(),
                _ => 0,
            },
            Self::AddSource(_) | Self::SetSource(_) => crate::MAX_SOURCE_PATH_BYTES,
            _ => 0,
        };
        std::mem::size_of::<Self>() + extra
    }
    pub(crate) fn source_revision(&self) -> u64 {
        match self {
            Self::SetSource(s) | Self::AddSource(s) => s.revision(),
            Self::Batch(edits) => edits
                .iter()
                .filter_map(|e| match e {
                    Self::SetSource(s) | Self::AddSource(s) => Some(s.revision()),
                    _ => None,
                })
                .max()
                .unwrap_or(0),
            _ => 0,
        }
    }
    fn flat_edit(&self) -> bool {
        matches!(
            self,
            Self::SetCameraBookmarks(_)
                | Self::SetFrameLinks(_)
                | Self::AddSource(_)
                | Self::RemoveSource(_)
                | Self::SetSource(_)
                | Self::AddAsset(_)
                | Self::RemoveAsset(_)
                | Self::SetAsset(_)
                | Self::AddObject { .. }
                | Self::RemoveObject(_)
                | Self::SetTransform { .. }
                | Self::SetCrop { .. }
                | Self::SetOpacity { .. }
                | Self::SetImageFiltering { .. }
                | Self::SetZOrder { .. }
                | Self::AddGroup(_)
                | Self::RemoveGroup(_)
                | Self::SetFrameName { .. }
                | Self::SetFrameColor { .. }
                | Self::SetText { .. }
                | Self::SetAnnotation { .. }
                | Self::SetAnnotationStyle { .. }
        )
    }
}

impl Document {
    pub(crate) fn validate_source_revision(
        &self,
        command: &Command,
        high_water: u64,
    ) -> Result<(), CommandError> {
        if let Command::Batch(edits) = command {
            let mut water = high_water;
            for edit in edits {
                if !edit.flat_edit() {
                    return Err(CommandError::LimitReached("flat metadata batch"));
                }
                self.validate_source_revision(edit, water)?;
                water = water.max(edit.source_revision());
            }
        }
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
            SetFrameLinks(changes) => return self.set_frame_links(changes),
            SetCameraBookmarks(views) => {
                let ids: std::collections::BTreeSet<_> = views.iter().map(|b| b.id()).collect();
                if views.len() > crate::MAX_CAMERA_BOOKMARKS || ids.len() != views.len() {
                    return Err(CommandError::LimitReached("valid unique camera bookmarks"));
                }
                if self.bookmarks == views {
                    None
                } else {
                    Some(SetCameraBookmarks(std::mem::replace(
                        &mut self.bookmarks,
                        views,
                    )))
                }
            }
            Batch(edits) => {
                if edits.len() > 200_000 || edits.iter().any(|e| !e.flat_edit()) {
                    return Err(CommandError::LimitReached("flat metadata batch"));
                }
                let mut inverses = Vec::with_capacity(edits.len());
                let mut inverse_count = 0usize;
                for edit in edits {
                    match self.apply_reversible(edit) {
                        Ok(Some(inverse)) => {
                            inverse_count += match &inverse {
                                Batch(edits) => edits.len(),
                                _ => 1,
                            };
                            if inverse_count > 200_000 {
                                self.apply_reversible(inverse)?;
                                for inverse in inverses.into_iter().rev() {
                                    self.apply_reversible(inverse)?;
                                }
                                return Err(CommandError::LimitReached("generated inverse batch"));
                            }
                            inverses.push(inverse);
                        }
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
                    // Generated inverses may be compound (Frame delete/translation).
                    // Flatten in undo order; caller-provided nested batches stay forbidden.
                    let inverses: Vec<_> = inverses
                        .into_iter()
                        .rev()
                        .flat_map(|inverse| match inverse {
                            Batch(edits) => edits,
                            other => vec![other],
                        })
                        .collect();
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
            SetAsset(asset) => {
                let id = asset.id();
                let previous = self.assets.get(&id).ok_or(CommandError::MissingAsset(id))?;
                if !self.sources.contains_key(&asset.source_id()) {
                    return Err(CommandError::MissingSource(asset.source_id()));
                }
                if previous == &asset {
                    None
                } else {
                    let previous = *previous;
                    self.assets.insert(id, asset);
                    Some(SetAsset(previous))
                }
            }
            RemoveAsset(id) => {
                if !self.assets.contains_key(&id) {
                    return Err(CommandError::MissingAsset(id));
                }
                if self.objects.values().any(|o| match o.kind {
                    ObjectKind::Image(image) => image.asset_id() == id,
                    _ => false,
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
                if let ObjectKind::Image(image) = &object.kind
                    && !self.assets.contains_key(&image.asset_id())
                {
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
                if matches!(object.kind(), ObjectKind::Frame(_)) {
                    self.frame_count += 1;
                }
                if matches!(object.kind(), ObjectKind::Annotation(_)) {
                    self.annotation_count += 1;
                }
                self.order.insert(index, id);
                self.objects.insert(id, object);
                Some(RemoveObject(id))
            }
            RemoveObject(id) => {
                if self.memberships.contains_key(&id) {
                    return Err(CommandError::ObjectInGroup(id));
                }
                let mut links: Vec<_> = self
                    .linked_children(id)
                    .map(|child| (child, None))
                    .collect();
                if self.frame_parent(id).is_some() {
                    links.push((id, None));
                }
                let restore_links = self.set_frame_links(links)?;
                let index = self.position(id)?;
                let object = self
                    .objects
                    .remove(&id)
                    .ok_or(CommandError::MissingObject(id))?;
                self.order.remove(index);
                if matches!(object.kind(), ObjectKind::Frame(_)) {
                    self.frame_count -= 1;
                }
                if matches!(object.kind(), ObjectKind::Annotation(_)) {
                    self.annotation_count -= 1;
                }
                let restore = AddObject { object, index };
                Some(match restore_links {
                    Some(links) => Batch(vec![restore, links]),
                    None => restore,
                })
            }
            SetTransform { object, transform } => {
                return self.set_linked_transform(object, transform);
            }
            SetCrop { object, crop } => {
                let target = self
                    .objects
                    .get_mut(&object)
                    .ok_or(CommandError::MissingObject(object))?
                    .image_mut()
                    .ok_or(CommandError::WrongObjectKind(object))?;
                replace(&mut target.crop, crop).map(|crop| SetCrop { object, crop })
            }
            SetOpacity { object, opacity } => {
                if let Some(target) = self.object(object)
                    && let ObjectKind::Annotation(a) = target.kind()
                    && let crate::AnnotationKind::Scribble(s) = a.kind()
                    && s.strokes().iter().any(|s| s.style().is_some())
                {
                    let old = a.style();
                    let style =
                        crate::AnnotationStyle::new(old.stroke(), old.fill(), old.width(), opacity)
                            .map_err(|_| CommandError::LimitReached("annotation style"))?;
                    return self.apply_reversible(SetAnnotationStyle { object, style });
                }
                let target = self
                    .objects
                    .get_mut(&object)
                    .ok_or(CommandError::MissingObject(object))?;
                match &mut target.kind {
                    ObjectKind::Image(image) => replace(&mut image.opacity, opacity)
                        .map(|opacity| SetOpacity { object, opacity }),
                    ObjectKind::Annotation(a) => {
                        let old = a.style.opacity();
                        if old == opacity {
                            None
                        } else {
                            a.style.set_opacity(opacity);
                            Some(SetOpacity {
                                object,
                                opacity: old,
                            })
                        }
                    }
                    _ => return Err(CommandError::WrongObjectKind(object)),
                }
            }
            SetImageFiltering { object, filtering } => {
                let target = self
                    .objects
                    .get_mut(&object)
                    .ok_or(CommandError::MissingObject(object))?
                    .image_mut()
                    .ok_or(CommandError::WrongObjectKind(object))?;
                replace(&mut target.filtering, filtering)
                    .map(|filtering| SetImageFiltering { object, filtering })
            }
            AddGroup(group) => return self.add_group(group),
            RemoveGroup(id) => return self.remove_group(id),
            SetAnnotation { object, annotation } => {
                let target = self
                    .objects
                    .get_mut(&object)
                    .ok_or(CommandError::MissingObject(object))?;
                let ObjectKind::Annotation(a) = &mut target.kind else {
                    return Err(CommandError::WrongObjectKind(object));
                };
                annotation
                    .bounds(target.transform)
                    .map_err(|_| CommandError::LimitReached("annotation bounds"))?;
                if a.as_ref() == &annotation {
                    None
                } else {
                    Some(SetAnnotation {
                        object,
                        annotation: std::mem::replace(a.as_mut(), annotation),
                    })
                }
            }
            SetAnnotationStyle { object, style } => {
                let target = self
                    .objects
                    .get_mut(&object)
                    .ok_or(CommandError::MissingObject(object))?;
                let ObjectKind::Annotation(a) = &mut target.kind else {
                    return Err(CommandError::WrongObjectKind(object));
                };
                a.bounds_with_style(target.transform, style)
                    .map_err(|_| CommandError::LimitReached("annotation bounds"))?;
                if let crate::AnnotationKind::Scribble(s) = &a.kind
                    && s.strokes().iter().any(|s| s.style().is_some())
                {
                    let next = crate::Annotation::new(
                        crate::AnnotationKind::Scribble(
                            s.restyled(a.style, style)
                                .map_err(|_| CommandError::LimitReached("scribble style"))?,
                        ),
                        style,
                    );
                    next.bounds(target.transform)
                        .map_err(|_| CommandError::LimitReached("annotation bounds"))?;
                    if a.as_ref() == &next {
                        None
                    } else {
                        Some(SetAnnotation {
                            object,
                            annotation: std::mem::replace(a.as_mut(), next),
                        })
                    }
                } else {
                    replace(&mut a.style, style).map(|style| SetAnnotationStyle { object, style })
                }
            }
            SetText { object, text } => {
                let target = self
                    .objects
                    .get_mut(&object)
                    .ok_or(CommandError::MissingObject(object))?;
                let ObjectKind::Annotation(a) = &mut target.kind else {
                    return Err(CommandError::WrongObjectKind(object));
                };
                let crate::AnnotationKind::Text(previous) = &mut a.kind else {
                    return Err(CommandError::WrongObjectKind(object));
                };
                if *previous == text {
                    None
                } else {
                    Some(SetText {
                        object,
                        text: std::mem::replace(previous, text),
                    })
                }
            }
            SetFrameColor { object, color } => {
                let target = self
                    .objects
                    .get_mut(&object)
                    .ok_or(CommandError::MissingObject(object))?;
                if !matches!(target.kind, ObjectKind::Frame(_)) {
                    return Err(CommandError::WrongObjectKind(object));
                }
                let previous = target.frame_color();
                if previous == color {
                    None
                } else {
                    target.frame_color = (color != crate::DEFAULT_FRAME_COLOR).then_some(color);
                    Some(SetFrameColor {
                        object,
                        color: previous,
                    })
                }
            }
            SetFrameName { object, name } => {
                crate::validate_frame_name(&name).map_err(|_| CommandError::InvalidFrame)?;
                let target = self
                    .objects
                    .get_mut(&object)
                    .ok_or(CommandError::MissingObject(object))?;
                let ObjectKind::Frame(previous) = &mut target.kind else {
                    return Err(CommandError::WrongObjectKind(object));
                };
                if *previous == name {
                    None
                } else {
                    Some(SetFrameName {
                        object,
                        name: std::mem::replace(previous, name),
                    })
                }
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
