//! Flat, translation-only containment. Frame children are resolved through an index.
use crate::{Command, CommandError, Document, ObjectId, ObjectKind, Transform};
use std::collections::{BTreeMap, BTreeSet};

impl Document {
    pub fn frame_parent(&self, child: ObjectId) -> Option<ObjectId> {
        self.frame_links.get(&child).copied()
    }
    pub fn frame_links(&self) -> impl Iterator<Item = (ObjectId, ObjectId)> + '_ {
        self.frame_links
            .iter()
            .map(|(child, parent)| (*child, *parent))
    }
    pub fn linked_children(&self, frame: ObjectId) -> impl Iterator<Item = ObjectId> + '_ {
        self.frame_children
            .get(&frame)
            .into_iter()
            .flat_map(|children| children.iter().copied())
    }
    pub(crate) fn set_frame_links(
        &mut self,
        changes: Vec<(ObjectId, Option<ObjectId>)>,
    ) -> Result<Option<Command>, CommandError> {
        if changes.len() > self.limits.objects {
            return Err(CommandError::LimitReached("Frame link count"));
        }
        let mut targets = BTreeMap::new();
        let mut groups = BTreeSet::new();
        for (child, parent) in &changes {
            if targets.insert(*child, *parent).is_some() {
                return Err(CommandError::InvalidFrameLink);
            }
            let object = self
                .object(*child)
                .ok_or(CommandError::MissingObject(*child))?;
            if matches!(object.kind(), ObjectKind::Frame(_)) {
                return Err(CommandError::InvalidFrameLink);
            }
            if let Some(parent) = parent
                && !self
                    .object(*parent)
                    .is_some_and(|o| matches!(o.kind(), ObjectKind::Frame(_)))
            {
                return Err(CommandError::InvalidFrameLink);
            }
            if let Some(group) = self.group_for(*child) {
                groups.insert(group.id());
            }
        }
        for group in groups.into_iter().filter_map(|id| self.group(id)) {
            let parent = |id: &ObjectId| {
                targets
                    .get(id)
                    .copied()
                    .unwrap_or_else(|| self.frame_parent(*id))
            };
            let first = parent(&group.members()[0]);
            if group.members().iter().any(|id| parent(id) != first) {
                return Err(CommandError::InvalidGroup);
            }
        }
        let mut inverse = Vec::new();
        for (child, parent) in changes {
            let previous = self.frame_parent(child);
            if previous == parent {
                continue;
            }
            inverse.push((child, previous));
            if let Some(previous) = previous {
                self.frame_links.remove(&child);
                if let Some(children) = self.frame_children.get_mut(&previous) {
                    children.remove(&child);
                }
                if self
                    .frame_children
                    .get(&previous)
                    .is_some_and(BTreeSet::is_empty)
                {
                    self.frame_children.remove(&previous);
                }
            }
            if let Some(parent) = parent {
                self.frame_links.insert(child, parent);
                self.frame_children.entry(parent).or_default().insert(child);
            }
        }
        Ok((!inverse.is_empty()).then_some(Command::SetFrameLinks(inverse)))
    }
    pub(crate) fn set_linked_transform(
        &mut self,
        object: ObjectId,
        transform: Transform,
    ) -> Result<Option<Command>, CommandError> {
        let target = self
            .object(object)
            .ok_or(CommandError::MissingObject(object))?;
        let previous = target.transform();
        if let ObjectKind::Annotation(annotation) = target.kind() {
            annotation
                .bounds(transform)
                .map_err(|_| CommandError::LimitReached("annotation bounds"))?;
        }
        let frame = matches!(target.kind(), ObjectKind::Frame(_));
        if frame && (transform.rotation() != 0. || transform.flips() != [false; 2]) {
            return Err(CommandError::InvalidFrame);
        }
        if previous == transform {
            return Ok(None);
        }
        // Resizing a Frame, even about an opposite edge, never moves children.
        let translate = frame && previous.size() == transform.size();
        let delta =
            std::array::from_fn::<_, 2, _>(|i| transform.center()[i] - previous.center()[i]);
        let mut children = Vec::new();
        if translate {
            for child in self.linked_children(object) {
                let current = self
                    .object(child)
                    .ok_or(CommandError::MissingObject(child))?;
                let old = current.transform();
                let next = Transform::new(
                    std::array::from_fn(|i| old.center()[i] + delta[i]),
                    old.size(),
                    old.rotation(),
                    old.flips(),
                )
                .map_err(|_| CommandError::LimitReached("linked transform bounds"))?;
                if let ObjectKind::Annotation(a) = current.kind() {
                    a.bounds(next)
                        .map_err(|_| CommandError::LimitReached("linked annotation bounds"))?;
                }
                children.push((child, old, next));
            }
        }
        // All fallible geometry/reference checks precede the first write.
        self.objects
            .get_mut(&object)
            .ok_or(CommandError::MissingObject(object))?
            .transform = transform;
        let mut inverse = vec![Command::SetTransform {
            object,
            transform: previous,
        }];
        for (child, old, next) in children {
            self.objects
                .get_mut(&child)
                .ok_or(CommandError::MissingObject(child))?
                .transform = next;
            inverse.push(Command::SetTransform {
                object: child,
                transform: old,
            });
        }
        Ok(Some(if inverse.len() == 1 {
            inverse.remove(0)
        } else {
            Command::Batch(inverse)
        }))
    }
}
