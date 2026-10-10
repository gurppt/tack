//! Connection-local reservations. Deadlines exist only during active leases.
use super::*;
use std::collections::BTreeMap;
use tack_shared::{LEASE_TTL_MS, LeaseRecord, MAX_BOARD_LEASES};

struct Own {
    operation: WireId,
    base: u64,
    objects: Vec<WireId>,
    granted: bool,
    expires: Instant,
    renew: Instant,
    submitted: Option<WireId>,
}
#[derive(Default)]
pub(crate) struct LeaseState {
    remote: BTreeMap<WireId, (WireId, Instant)>,
    own: Option<Own>,
    retired: Option<WireId>,
    lost: bool,
}
impl LeaseState {
    pub fn touches(&self, objects: &std::collections::BTreeSet<WireId>) -> bool {
        self.own
            .as_ref()
            .is_some_and(|o| o.objects.iter().any(|id| objects.contains(id)))
    }
    pub fn overlay(
        &self,
        gizmo: &mut tack_app::image_gizmo::ImageGizmo,
        document: &tack_core::Document,
        camera: &Camera,
    ) {
        for id in self.remote.keys() {
            if let Some(object) = tack_core::ObjectId::new(id.value())
                .ok()
                .and_then(|id| document.object(id))
            {
                let t = object.transform();
                let margin = 3. * gizmo.scale / camera.zoom();
                if t.bounds().intersects(camera.viewport())
                    && let Ok(outer) = tack_core::Transform::new(
                        t.center(),
                        t.size().map(|s| s + margin * 2.),
                        t.rotation(),
                        t.flips(),
                    )
                {
                    gizmo.outline(outer, camera, [0.94, 0.24, 0.32, 1.]);
                }
            }
        }
    }
    pub fn clear(&mut self) {
        self.remote.clear();
        self.own = None;
        self.retired = None;
        self.lost = false;
    }
    pub fn owns(&self, operation: WireId) -> bool {
        self.own.as_ref().is_some_and(|o| o.operation == operation)
    }
    pub fn snapshot(&mut self, leases: Vec<LeaseRecord>) {
        self.remote.clear();
        for lease in leases {
            self.changed(
                Some(lease.operation),
                Some(lease.client),
                vec![lease.object],
                lease.ttl_ms,
            );
        }
    }
    pub fn changed(
        &mut self,
        operation: Option<WireId>,
        _client: Option<WireId>,
        objects: Vec<WireId>,
        ttl_ms: u32,
    ) {
        let now = Instant::now();
        if operation.is_some() && operation == self.retired {
            return;
        }
        if let Some(own) = &self.own
            && operation != Some(own.operation)
            && objects.iter().any(|id| own.objects.contains(id))
        {
            self.lost = true;
        }
        if let Some(own) = &mut self.own
            && operation == Some(own.operation)
            && objects.len() == own.objects.len()
            && own.objects.iter().all(|id| objects.contains(id))
        {
            own.granted = true;
            own.expires = now + Duration::from_millis(u64::from(ttl_ms));
            own.renew = now + Duration::from_secs(2);
            for object in objects {
                self.remote.remove(&object);
            }
        } else {
            for object in objects {
                if let Some(operation) = operation {
                    if self.remote.len() < MAX_BOARD_LEASES || self.remote.contains_key(&object) {
                        self.remote.insert(
                            object,
                            (operation, now + Duration::from_millis(u64::from(ttl_ms))),
                        );
                    }
                } else {
                    self.remote.remove(&object);
                }
            }
        }
    }
    pub fn release(&mut self, client: &SharedClient) {
        if let Some(own) = self.own.take() {
            self.retired = Some(own.operation);
            let _ = client.release_lease(own.operation);
        }
    }
    pub fn submitted(&mut self, operation: WireId) {
        if let Some(own) = &mut self.own {
            own.submitted = Some(operation);
        }
    }
    pub fn finish(&mut self, operation: WireId, client: &SharedClient) {
        if self
            .own
            .as_ref()
            .is_some_and(|o| o.submitted == Some(operation))
        {
            self.release(client);
        }
    }
    pub fn deadline(&self) -> Option<Instant> {
        self.remote
            .values()
            .map(|(_, until)| *until)
            .chain(self.own.iter().map(|o| o.renew.min(o.expires)))
            .min()
    }
}
impl App {
    pub(crate) fn sync_leases(&mut self) {
        let (Some(shared), Some(editor)) = (&mut self.shared, &mut self.editor) else {
            return;
        };
        let now = Instant::now();
        if std::mem::take(&mut shared.leases.lost) {
            self.input.images.cancel();
            shared.leases.release(&shared.client);
            self.dirty = true;
        }
        let count = shared.leases.remote.len();
        shared.leases.remote.retain(|_, (_, until)| *until > now);
        self.dirty |= count != shared.leases.remote.len();
        self.input.images.blocked = shared
            .leases
            .remote
            .keys()
            .filter_map(|id| tack_core::ObjectId::new(id.value()).ok())
            .collect();
        if let Some(own) = &shared.leases.own {
            if now >= own.expires {
                self.input.images.cancel();
                shared.leases.release(&shared.client);
                self.interaction_error = Some("Object reservation expired; try again".into());
                self.dirty = true;
            } else if !self.input.images.active()
                && editor.pending_backend_requests() == 0
                && shared.in_flight.is_none()
            {
                shared.leases.release(&shared.client);
            } else if now >= own.renew {
                let result =
                    shared
                        .client
                        .acquire_lease(own.operation, own.base, own.objects.clone());
                if let Some(own) = &mut shared.leases.own {
                    own.renew = now + Duration::from_secs(2);
                }
                if let Err(error) = result {
                    self.interaction_error = Some(error.to_string());
                }
            }
        }
        if self.input.images.active() && shared.leases.own.is_none() {
            let result = tack_storage::new_document_id()
                .map_err(tack_shared::Error::from)
                .and_then(|id| WireId::new(id.value()))
                .and_then(|operation| {
                    let objects = self
                        .input
                        .images
                        .gesture_targets()
                        .map(|id| WireId::new(id.value()))
                        .collect::<tack_shared::Result<Vec<_>>>()?;
                    shared
                        .client
                        .acquire_lease(operation, shared.revision, objects.clone())?;
                    shared.leases.own = Some(Own {
                        operation,
                        base: shared.revision,
                        objects,
                        granted: false,
                        expires: now + Duration::from_millis(u64::from(LEASE_TTL_MS)),
                        renew: now + Duration::from_secs(2),
                        submitted: None,
                    });
                    Ok(())
                });
            if let Err(error) = result {
                self.input.images.cancel();
                self.interaction_error = Some(error.to_string());
            }
        }
        self.input.gesture_waiting = shared.leases.own.as_ref().is_some_and(|o| !o.granted);
        self.input.images.reservation_pending = shared
            .leases
            .own
            .as_ref()
            .is_some_and(|o| o.submitted.is_some());
    }
}
