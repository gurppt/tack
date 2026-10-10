//! Connection-owned, transient manipulation leases. No background timer.
use super::*;
use std::time::{Duration, Instant};
use tack_shared::{LEASE_TTL_MS, LeaseRecord, MAX_BOARD_LEASES, MAX_LEASE_TARGETS};

struct Lease {
    connection: usize,
    client: WireId,
    operation: WireId,
    until: Instant,
}
#[derive(Default)]
pub(super) struct LeaseTable {
    entries: BTreeMap<(WireId, WireId), Lease>,
}
impl LeaseTable {
    pub fn check(
        &self,
        board: WireId,
        connection: usize,
        objects: &std::collections::BTreeSet<WireId>,
    ) -> Result<()> {
        let now = Instant::now();
        if objects.iter().any(|object| {
            self.entries
                .get(&(board, *object))
                .is_some_and(|lease| lease.connection != connection && lease.until > now)
        }) {
            return Err("Object is being manipulated elsewhere".into());
        }
        Ok(())
    }
    pub fn snapshot(&self, board: WireId) -> Vec<LeaseRecord> {
        let now = Instant::now();
        self.entries
            .iter()
            .filter_map(|((id, object), lease)| {
                let ttl_ms = lease
                    .until
                    .saturating_duration_since(now)
                    .as_millis()
                    .min(u128::from(LEASE_TTL_MS)) as u32;
                (*id == board && ttl_ms > 0).then_some(LeaseRecord {
                    object: *object,
                    client: lease.client,
                    operation: lease.operation,
                    ttl_ms,
                })
            })
            .collect()
    }
}
pub(super) fn broadcast(hub: &mut Hub, board: WireId, message: Message) -> Result<()> {
    let bytes = Arc::new(frame(&message)?);
    let failed: Vec<_> = hub
        .subscribers
        .iter()
        .filter_map(|(id, subscriber)| {
            (subscriber.board == board && !subscriber.out.send(Arc::clone(&bytes))).then_some(*id)
        })
        .collect();
    for id in failed {
        hub.subscribers.remove(&id);
        release_connection(hub, id)?;
    }
    Ok(())
}
pub(super) fn prune(hub: &mut Hub, board: WireId) -> Result<()> {
    let now = Instant::now();
    let expired: Vec<_> = hub
        .leases
        .entries
        .iter()
        .filter_map(|((b, object), lease)| (*b == board && lease.until <= now).then_some(*object))
        .collect();
    for object in &expired {
        hub.leases.entries.remove(&(board, *object));
    }
    cleared(hub, board, expired)
}
fn cleared(hub: &mut Hub, board: WireId, objects: Vec<WireId>) -> Result<()> {
    if objects.is_empty() {
        return Ok(());
    }
    broadcast(
        hub,
        board,
        Message::LeaseChanged {
            operation: None,
            client: None,
            objects,
            ttl_ms: 0,
        },
    )
}
pub(super) fn release_connection(hub: &mut Hub, connection: usize) -> Result<()> {
    let keys: Vec<_> = hub
        .leases
        .entries
        .iter()
        .filter_map(|(key, lease)| (lease.connection == connection).then_some(*key))
        .collect();
    let mut boards: BTreeMap<_, Vec<_>> = BTreeMap::new();
    for (board, object) in keys {
        hub.leases.entries.remove(&(board, object));
        boards.entry(board).or_default().push(object);
    }
    for (board, objects) in boards {
        cleared(hub, board, objects)?;
    }
    Ok(())
}
pub(super) fn release(hub: &mut Hub, connection: usize, operation: WireId) -> Result<()> {
    let keys: Vec<_> = hub
        .leases
        .entries
        .iter()
        .filter_map(|(key, lease)| {
            (lease.connection == connection && lease.operation == operation).then_some(*key)
        })
        .collect();
    let mut boards: BTreeMap<_, Vec<_>> = BTreeMap::new();
    for (board, object) in keys {
        hub.leases.entries.remove(&(board, object));
        boards.entry(board).or_default().push(object);
    }
    for (board, objects) in boards {
        cleared(hub, board, objects)?;
    }
    Ok(())
}
pub(super) fn acquire(
    hub: &mut Hub,
    connection: usize,
    operation: WireId,
    base: u64,
    objects: Vec<WireId>,
    out: &Out,
) -> Result<()> {
    let result = (|| -> Result<(WireId, WireId)> {
        let subscriber = hub.subscribers.get(&connection).ok_or("join required")?;
        let (board, client) = (subscriber.board, subscriber.client);
        let authority = hub.boards.get(&board).ok_or("board unavailable")?;
        if !authority.is_available() {
            return Err("Board unavailable".into());
        }
        authority.check_objects(base, &objects)?;
        if objects.iter().any(|id| {
            tack_core::ObjectId::new(id.value())
                .ok()
                .and_then(|id| authority.document().object(id))
                .is_none()
        }) {
            return Err("Object no longer exists".into());
        }
        hub.leases
            .check(board, connection, &objects.iter().copied().collect())?;
        let new = objects
            .iter()
            .filter(|object| !hub.leases.entries.contains_key(&(board, **object)))
            .count();
        if hub
            .leases
            .entries
            .keys()
            .filter(|(b, _)| *b == board)
            .count()
            + new
            > MAX_BOARD_LEASES
            || hub
                .leases
                .entries
                .values()
                .filter(|lease| lease.connection == connection)
                .count()
                + new
                > MAX_LEASE_TARGETS
        {
            return Err("Manipulation lease capacity reached".into());
        }
        Ok((board, client))
    })();
    let (board, client) = match result {
        Ok(ids) => ids,
        Err(reason) => {
            return reply(
                out,
                Message::LeaseDenied {
                    operation,
                    reason: bounded_reason(&reason),
                },
            );
        }
    };
    let until = Instant::now() + Duration::from_millis(u64::from(LEASE_TTL_MS));
    for object in &objects {
        hub.leases.entries.insert(
            (board, *object),
            Lease {
                connection,
                client,
                operation,
                until,
            },
        );
    }
    broadcast(
        hub,
        board,
        Message::LeaseChanged {
            operation: Some(operation),
            client: Some(client),
            objects,
            ttl_ms: LEASE_TTL_MS,
        },
    )
}
