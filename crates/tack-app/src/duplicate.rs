//! Metadata duplication: originals/assets stay shared, all edits use one history.
use std::collections::{BTreeMap, BTreeSet};
use tack_assets::AssetError;
use tack_core::{Command, Document, Group, ObjectId};

pub fn selection(
    doc: &Document,
    selected: impl Iterator<Item = ObjectId>,
    offset: [f64; 2],
) -> Result<(Command, Vec<ObjectId>), AssetError> {
    let selected: BTreeSet<_> = selected.collect();
    if selected.iter().any(|id| doc.object(*id).is_none()) {
        return Err("duplicate selection contains a missing object".into());
    }
    if doc.object_order().len().saturating_add(selected.len()) > doc.limits().objects {
        return Err("duplicate exceeds board object limit".into());
    }
    let mut ids = BTreeMap::new();
    let mut edits = Vec::with_capacity(selected.len());
    let mut copies = Vec::with_capacity(selected.len());
    for old in doc.object_order().iter().filter(|id| selected.contains(id)) {
        let object = doc.object(*old).ok_or("missing duplicate object")?;
        let id = tack_storage::new_object_id()?;
        let copy = object.duplicate(id, offset)?;
        ids.insert(*old, id);
        edits.push(Command::AddObject {
            object: copy,
            index: doc.object_order().len() + copies.len(),
        });
        copies.push(id);
    }
    let links: Vec<_> = ids
        .iter()
        .filter_map(|(old, new)| {
            doc.frame_parent(*old)
                .map(|parent| (*new, Some(ids.get(&parent).copied().unwrap_or(parent))))
        })
        .collect();
    if !links.is_empty() {
        edits.push(Command::SetFrameLinks(links));
    }
    for group in doc.groups() {
        let members: Vec<_> = group
            .members()
            .iter()
            .filter_map(|id| ids.get(id).copied())
            .collect();
        if members.len() >= 2 {
            edits.push(Command::AddGroup(Group::new(
                tack_storage::new_group_id()?,
                members,
            )?));
        }
    }
    Ok((Command::Batch(edits), copies))
}
