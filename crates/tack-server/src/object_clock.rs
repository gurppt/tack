//! Durable per-object stamps and bounded deletion history for scoped CAS.
use crate::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use tack_core::{Document, ObjectId};
use tack_shared::{CommandScope, WireId};

pub const MAX_TOMBSTONES: usize = 4096;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ObjectClock {
    objects: BTreeMap<WireId, u64>,
    removed: BTreeMap<WireId, u64>,
    floor: u64,
    dependencies: u64,
    order: u64,
}
impl ObjectClock {
    pub fn new(document: &Document, revision: u64) -> Self {
        Self {
            objects: document
                .objects()
                .filter_map(|o| WireId::new(o.id().value()).ok().map(|id| (id, revision)))
                .collect(),
            removed: BTreeMap::new(),
            floor: revision,
            dependencies: revision,
            order: revision,
        }
    }
    pub fn validate(&self, document: &Document, revision: u64) -> Result<()> {
        if self.objects.len() != document.object_order().len()
            || self.removed.len() > MAX_TOMBSTONES
            || [self.floor, self.dependencies, self.order]
                .into_iter()
                .any(|v| v > revision)
            || self.objects.iter().any(|(id, v)| {
                *v > revision
                    || ObjectId::new(id.value())
                        .ok()
                        .and_then(|id| document.object(id))
                        .is_none()
            })
            || self
                .removed
                .iter()
                .any(|(id, v)| *v > revision || self.objects.contains_key(id))
        {
            return Err("invalid object conflict clock".into());
        }
        Ok(())
    }
    pub fn check(&self, scope: &CommandScope, base: u64, revision: u64) -> Result<()> {
        if base > revision || base < self.floor {
            return Err("stale revision outside retained conflict window; rejoin".into());
        }
        if scope.objects.iter().any(|id| {
            self.objects
                .get(id)
                .or_else(|| self.removed.get(id))
                .is_some_and(|v| *v > base)
        }) || (scope.reads_dependencies && self.dependencies > base)
            || (scope.reads_order && self.order > base)
        {
            return Err("object conflict: target or dependency changed".into());
        }
        Ok(())
    }
    pub fn advance(&mut self, scope: &CommandScope, document: &Document, revision: u64) {
        for id in &scope.objects {
            if ObjectId::new(id.value())
                .ok()
                .and_then(|id| document.object(id))
                .is_some()
            {
                self.objects.insert(*id, revision);
                self.removed.remove(id);
            } else {
                self.objects.remove(id);
                self.removed.insert(*id, revision);
            }
        }
        if scope.writes_dependencies {
            self.dependencies = revision;
        }
        if scope.writes_order {
            self.order = revision;
        }
        while self.removed.len() > MAX_TOMBSTONES {
            let oldest = self
                .removed
                .iter()
                .min_by_key(|(_, v)| **v)
                .map(|(id, v)| (*id, *v));
            if let Some((id, stamp)) = oldest {
                self.removed.remove(&id);
                self.floor = self.floor.max(stamp.saturating_add(1));
            }
        }
    }
}
