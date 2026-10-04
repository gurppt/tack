use crate::{Command, CommandError, Document, GroupId, ObjectId, ObjectKind};

/// Flat membership, no parent relation or duplicated transform.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Group {
    id: GroupId,
    members: Vec<ObjectId>,
}
impl Group {
    pub fn new(id: GroupId, mut members: Vec<ObjectId>) -> Result<Self, CommandError> {
        if members.len() < 2 || members.len() > 100_000 {
            return Err(CommandError::InvalidGroup);
        }
        members.sort_unstable();
        if members.windows(2).any(|w| w[0] == w[1]) {
            return Err(CommandError::InvalidGroup);
        }
        Ok(Self { id, members })
    }
    pub fn id(&self) -> GroupId {
        self.id
    }
    pub fn members(&self) -> &[ObjectId] {
        &self.members
    }
    pub(crate) fn retained_bytes(&self) -> usize {
        self.members.capacity() * std::mem::size_of::<ObjectId>()
    }
}
impl Document {
    pub(crate) fn add_group(&mut self, group: Group) -> Result<Option<Command>, CommandError> {
        if self.groups.contains_key(&group.id) || self.groups.len() >= self.limits.objects {
            return Err(CommandError::InvalidGroup);
        }
        for id in &group.members {
            if !self
                .objects
                .get(id)
                .is_some_and(|o| matches!(o.kind(), ObjectKind::Image(_)))
                || self.memberships.contains_key(id)
            {
                return Err(CommandError::InvalidGroup);
            }
        }
        for id in &group.members {
            self.memberships.insert(*id, group.id);
        }
        let id = group.id;
        self.groups.insert(id, group);
        Ok(Some(Command::RemoveGroup(id)))
    }
    pub(crate) fn remove_group(&mut self, id: GroupId) -> Result<Option<Command>, CommandError> {
        let group = self.groups.remove(&id).ok_or(CommandError::InvalidGroup)?;
        for id in group.members() {
            self.memberships.remove(id);
        }
        Ok(Some(Command::AddGroup(group)))
    }
}
