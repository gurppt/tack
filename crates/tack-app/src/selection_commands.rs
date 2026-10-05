//! UI adapters for existing metadata commands; one atomic history entry.
use std::collections::{BTreeMap, BTreeSet};
use tack_core::{Command, Document, ObjectId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Order {
    Forward,
    Front,
    Backward,
    Back,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Alpha {
    Full,
    ThreeQuarters,
    Half,
    Quarter,
}
impl Alpha {
    pub fn value(self) -> f64 {
        match self {
            Self::Full => 1.,
            Self::ThreeQuarters => 0.75,
            Self::Half => 0.5,
            Self::Quarter => 0.25,
        }
    }
}
pub fn order(doc: &Document, ids: impl Iterator<Item = ObjectId>, direction: Order) -> Command {
    let selected: BTreeSet<_> = ids.collect();
    let original = doc.object_order();
    let mut target = original.to_vec();
    match direction {
        Order::Forward => {
            for i in (0..target.len().saturating_sub(1)).rev() {
                if selected.contains(&target[i]) && !selected.contains(&target[i + 1]) {
                    target.swap(i, i + 1);
                }
            }
        }
        Order::Backward => {
            for i in 1..target.len() {
                if selected.contains(&target[i]) && !selected.contains(&target[i - 1]) {
                    target.swap(i, i - 1);
                }
            }
        }
        Order::Front => target.sort_by_key(|id| selected.contains(id)),
        Order::Back => target.sort_by_key(|id| !selected.contains(id)),
    }
    if target == original {
        return Command::Batch(Vec::new());
    }
    let positions: BTreeMap<_, _> = target.iter().enumerate().map(|(i, id)| (*id, i)).collect();
    let mut moving: Vec<_> = original
        .iter()
        .copied()
        .filter(|id| selected.contains(id))
        .collect();
    if matches!(direction, Order::Forward | Order::Front) {
        moving.reverse();
    }
    Command::Batch(
        moving
            .into_iter()
            .map(|object| Command::SetZOrder {
                object,
                index: positions[&object],
            })
            .collect(),
    )
}
