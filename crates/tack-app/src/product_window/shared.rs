//! Optional authoritative backend; local windows never construct a LAN client.
use super::*;
mod leases;
pub(super) use leases::LeaseState;
use std::collections::{HashMap, HashSet};
use tack_core::{BackendRequest, SourceId};
use tack_shared::client::{ClientConfig, ClientEvent, ConnectionState, SharedClient};
use tack_shared::{CommandDto, SourceBinding, WireId};

use tack_app::capabilities::UiConnection;
pub(super) struct SharedState {
    pub leases: LeaseState,
    pub duplicate_selection: Vec<tack_core::ObjectId>,
    pub client: SharedClient,
    pub address: String,
    pub board: WireId,
    client_id: WireId,
    pub state: ConnectionState,
    pub snapshot_seen: bool,
    pub transitioning: bool,
    pub revision: u64,
    pub bindings: HashMap<SourceId, SourceBinding>,
    requested: HashSet<(SourceId, u64)>,
    evicted: HashSet<(SourceId, u64)>,
    in_flight: Option<WireId>,
    pub accepted: u64,
    pub refused: u64,
    pub probes: Vec<serde_json::Value>,
}
impl SharedState {
    pub fn ui_connection(&self) -> UiConnection {
        match self.state {
            ConnectionState::Connected => UiConnection::Online,
            ConnectionState::Connecting | ConnectionState::Reconnecting => UiConnection::Connecting,
            _ => UiConnection::Offline,
        }
    }
    pub fn has_in_flight(&self) -> bool {
        self.in_flight.is_some()
    }
    pub fn start(
        address: String,
        board: WireId,
        cache_dir: PathBuf,
        proxy: winit::event_loop::EventLoopProxy<Event>,
    ) -> Result<Self, AssetError> {
        if !cfg!(feature = "network") {
            return Err("Network capability omitted from this build".into());
        }
        let client_id = WireId::new(tack_storage::new_document_id()?.value())?;
        let client = SharedClient::start(
            ClientConfig {
                address: address.clone(),
                board,
                client: client_id,
                cache_dir,
            },
            move || {
                let _ = proxy.send_event(Event::SharedReady);
            },
        )?;
        Ok(Self {
            leases: LeaseState::default(),
            duplicate_selection: Vec::new(),
            client,
            address,
            board,
            client_id,
            state: ConnectionState::Connecting,
            snapshot_seen: false,
            transitioning: false,
            revision: 0,
            bindings: HashMap::new(),
            requested: HashSet::new(),
            evicted: HashSet::new(),
            in_flight: None,
            accepted: 0,
            refused: 0,
            probes: Vec::new(),
        })
    }
    pub fn probe_deadline(&self, started: Instant) -> Option<Instant> {
        [10, 15]
            .get(self.probes.len())
            .map(|seconds| started + Duration::from_secs(*seconds))
    }
    pub fn probe(&mut self, started: Instant) {
        if self
            .probe_deadline(started)
            .is_some_and(|deadline| Instant::now() >= deadline)
        {
            let stats = self.client.stats();
            self.probes
                .push(json!({"elapsed_seconds":started.elapsed().as_secs_f64(),
                "bytes_sent":stats.bytes_sent,"bytes_received":stats.bytes_received,
                "messages_sent":stats.messages_sent,"messages_received":stats.messages_received,
                "revision":self.revision,"state":self.state.label()}));
        }
    }
}
impl App {
    pub(super) fn poll_shared(&mut self) {
        let Some(shared) = &mut self.shared else {
            return;
        };
        let events = shared.client.drain();
        for event in events {
            if self
                .shared
                .as_ref()
                .is_some_and(|s| s.transitioning && !s.snapshot_seen)
                && matches!(
                    &event,
                    ClientEvent::Error(_)
                        | ClientEvent::State(
                            ConnectionState::Disconnected
                                | ConnectionState::ServerUnavailable
                                | ConnectionState::Refused
                                | ConnectionState::BoardUnavailable
                        )
                )
            {
                self.abort_shared_transition();
                self.local_error("Sharing connection failed; original local board retained");
                break;
            }
            if let Err(error) = self.shared_event(event) {
                if self.shared.as_ref().is_some_and(|s| s.transitioning) {
                    self.abort_shared_transition();
                    self.local_error(format!("Sharing failed; original retained: {error}"));
                    break;
                }
                self.interaction_error = Some(error.to_string());
                if let Some(editor) = &mut self.editor {
                    editor.set_shared_writable(false);
                }
                if let Some(shared) = &mut self.shared {
                    shared.state = ConnectionState::Disconnected;
                    let _ = shared.client.reconnect();
                }
                self.dirty = true;
            }
        }
        if let Some(shared) = &mut self.shared
            && !shared.client.connected()
            && shared.state == ConnectionState::Connected
        {
            shared.state = ConnectionState::Disconnected;
            if let Some(editor) = &mut self.editor {
                editor.set_shared_writable(false);
            }
            self.dirty = true;
        }
    }
    fn shared_event(&mut self, event: ClientEvent) -> Result<(), AssetError> {
        let mut show_host = false;
        let mut show_offline = false;
        let shared = self.shared.as_mut().ok_or("shared backend unavailable")?;
        match event {
            ClientEvent::LeaseSnapshot { leases } => shared.leases.snapshot(leases),
            ClientEvent::LeaseChanged {
                operation,
                client,
                objects,
                ttl_ms,
            } => shared.leases.changed(operation, client, objects, ttl_ms),
            ClientEvent::LeaseDenied { operation, reason } => {
                if shared.leases.owns(operation) {
                    self.input.cancel();
                    shared.leases.release(&shared.client);
                }
                self.interaction_error = Some(reason);
            }
            ClientEvent::State(state) => {
                show_offline = shared.snapshot_seen
                    && shared.state == ConnectionState::Connected
                    && state != ConnectionState::Connected;
                shared.state = state;
                if state != ConnectionState::Connected {
                    shared.leases.clear();
                    self.input.gesture_waiting = false;
                    self.input.images.cancel();
                }
                if let Some(editor) = &mut self.editor {
                    editor.set_shared_writable(shared.state == ConnectionState::Connected);
                }
            }
            ClientEvent::Snapshot {
                revision,
                source_high_water,
                document,
                path,
                board,
                sources,
                ..
            } => {
                shared.leases.clear();
                show_host = !shared.snapshot_seen && (self.host.is_some() || shared.transitioning);
                let bindings = sources
                    .into_iter()
                    .map(|binding| SourceId::new(binding.source.value()).map(|id| (id, binding)))
                    .collect::<Result<_, _>>()?;
                let first = self.editor.is_none();
                self.input.cancel();
                self.input.name_edit = None;
                self.context = None;
                let mut editor = DocumentEditor::shared(document);
                editor.set_shared_revision(revision);
                editor.set_shared_source_revision_floor(source_high_water);
                editor.set_shared_writable(true);
                self.input.images.selection.prune(editor.document());
                if first && let Some(object) = editor.document().objects().next() {
                    let center = object.transform().center().map(|v| v.clamp(-1e8, 1e8));
                    self.camera_clamped = center != object.transform().center();
                    self.camera.set_view(
                        center,
                        (600. / object.transform().size()[0]).clamp(0.000001, 1000.),
                    )?;
                }
                let limits = if self.options.potato {
                    SupplyLimits::potato()
                } else {
                    SupplyLimits::default()
                };
                if let Some(assets) = &mut self.assets {
                    assets.set_board(Arc::clone(&board));
                    assets.sync_document(editor.document());
                } else {
                    self.assets = Some(ProductAssets::with_limits(
                        Arc::clone(&board),
                        &path,
                        self.work.join("display"),
                        limits,
                    )?);
                }
                for source in editor.document().sources() {
                    if let Some(assets) = &mut self.assets {
                        assets.defer_shared_source(source.id(), source.revision());
                    }
                }
                if shared.transitioning {
                    self.local.lease = None;
                    self.local.untitled = false;
                    self.local.retire_seed = self.local.seed.is_some();
                    self.options.untitled = false;
                    self.options.new = false;
                    self.local.recovery = tack_app::recovery_schedule::RecoverySchedule::default();
                    shared.transitioning = false;
                }
                self.board = Some(board);
                self.editor = Some(editor);
                self.options.path = path;
                shared.snapshot_seen = true;
                shared.revision = revision;
                shared.state = ConnectionState::Connected;
                shared.bindings = bindings;
                shared.requested.clear();
                shared.evicted.clear();
                shared.in_flight = None;
                self.local.originals.clear();
                self.visibility.invalidate();
                self.load_failed = false;
                self.interaction_error = None;
            }
            ClientEvent::Accepted {
                revision,
                command,
                sources,
                client,
                operation,
            } => {
                if shared.in_flight == Some(operation) && client == shared.client_id {
                    shared.in_flight = None;
                    shared.leases.finish(operation, &shared.client);
                }
                if revision <= shared.revision {
                    return Ok(());
                }
                if revision != shared.revision.saturating_add(1) {
                    if let Some(editor) = &mut self.editor {
                        editor.set_shared_writable(false);
                    }
                    shared.client.reconnect()?;
                    return Err("shared revision gap; reconciling snapshot".into());
                }
                let editor = self.editor.as_mut().ok_or("shared snapshot pending")?;
                let scope = command.scope(editor.document());
                if client != shared.client_id {
                    editor.discard_backend_requests(|request, document| match request {
                        BackendRequest::Edit(edit) => CommandDto::from_command(edit)
                            .is_ok_and(|c| !c.scope(document).objects.is_disjoint(&scope.objects)),
                        _ => false,
                    });
                    let touches = |id: tack_core::ObjectId| {
                        scope
                            .objects
                            .iter()
                            .any(|target| target.value() == id.value())
                    };
                    if self.input.images.gesture_targets().any(touches) {
                        self.input.images.cancel();
                    }
                    if self
                        .input
                        .annotation
                        .edit
                        .as_ref()
                        .is_some_and(|e| !e.is_new && touches(e.id))
                    {
                        self.input.annotation.edit = None;
                    }
                    if self.input.name_edit.as_ref().is_some_and(|e| touches(e.id)) {
                        self.input.name_edit = None;
                    }
                }
                editor.accept_authoritative(command.to_command()?)?;
                editor.set_shared_revision(revision);
                if client == shared.client_id
                    && !shared.duplicate_selection.is_empty()
                    && shared
                        .duplicate_selection
                        .iter()
                        .all(|id| editor.document().object(*id).is_some())
                {
                    self.input.images.selection.clear();
                    for id in shared.duplicate_selection.drain(..) {
                        self.input.images.selection.select(Some(id), true);
                    }
                }
                self.input
                    .rebase_scribble_drafts(editor.generation(), |id| {
                        scope
                            .objects
                            .iter()
                            .any(|target| target.value() == id.value())
                    });
                // New annotation drafts and unrelated text are local state.
                if let Some(c) = &mut self.input.annotation.creation {
                    c.generation = editor.generation();
                }
                if let Some(e) = &mut self.input.annotation.edit {
                    e.generation = editor.generation();
                }
                if let Some(menu) = &mut self.context {
                    menu.context.generation = editor.generation();
                }
                self.input.images.selection.prune(editor.document());
                if let Some(assets) = &mut self.assets {
                    assets.sync_document(editor.document());
                }
                shared.revision = revision;
                shared.accepted += 1;
                for binding in sources {
                    let id = SourceId::new(binding.source.value())?;
                    if let Some(assets) = &mut self.assets {
                        assets.defer_shared_source(id, binding.revision);
                    }
                    shared.bindings.insert(id, binding);
                }
                shared.bindings.retain(|id, binding| {
                    editor
                        .document()
                        .source(*id)
                        .is_some_and(|source| source.revision() == binding.revision)
                });
                shared.requested.retain(|(id, revision)| {
                    editor
                        .document()
                        .source(*id)
                        .is_some_and(|source| source.revision() == *revision)
                });
                shared.evicted.retain(|(id, revision)| {
                    editor
                        .document()
                        .source(*id)
                        .is_some_and(|source| source.revision() == *revision)
                });
                self.visibility.invalidate();
            }
            ClientEvent::Refused {
                reason,
                revision,
                operation,
            } => {
                if shared.in_flight == operation {
                    shared.in_flight = None;
                }
                if let Some(operation) = operation {
                    shared.leases.finish(operation, &shared.client);
                }
                shared.refused += 1;
                shared.duplicate_selection.clear();

                self.interaction_error =
                    Some(format!("Edit refused at revision {revision}: {reason}"));
            }
            ClientEvent::AssetReady { binding, path } => {
                let id = SourceId::new(binding.source.value())?;
                if shared.bindings.get(&id) == Some(&binding)
                    && let Some(assets) = &mut self.assets
                {
                    assets.set_shared_source(id, binding.revision, path);
                }
            }
            ClientEvent::AssetFailed { reason, .. } => {
                self.interaction_error = Some(format!("Shared source unavailable: {reason}"));
            }
            ClientEvent::Error(reason) => {
                self.interaction_error = Some(reason);
            }
            ClientEvent::AssetEvicted { hashes } => {
                // Keep current visible supply stable even when originals leave
                // the disposable cache. Re-entry may fetch once again, never a
                // render-driven eviction/download loop for >512 MiB visible sources.
                for binding in shared.bindings.values() {
                    if hashes.contains(&binding.hash) {
                        shared
                            .evicted
                            .insert((SourceId::new(binding.source.value())?, binding.revision));
                    }
                }
            }
        }
        if show_offline {
            self.share_panel()?;
        }
        if show_host {
            self.share_panel()?;
        }
        self.notify_join_receipt()?;
        self.dirty = true;
        Ok(())
    }
    pub(super) fn notify_join_receipt(&mut self) -> Result<(), AssetError> {
        let Some(path) = self.options.join_receipt.as_ref() else {
            return Ok(());
        };
        if self.local.worker.active() || self.local.queued.is_some() {
            return Ok(());
        }
        let message = if self
            .shared
            .as_ref()
            .is_some_and(|s| s.snapshot_seen && s.state == ConnectionState::Connected)
        {
            "joined".into()
        } else if let Some(error) = &self.interaction_error {
            format!("failed: {}", error.chars().take(256).collect::<String>())
        } else {
            return Ok(());
        };
        self.operation(tack_app::local_worker::Operation::JoinReceipt {
            path: path.clone(),
            message,
        })?;
        self.options.join_receipt = None;
        Ok(())
    }
    pub(super) fn flush_shared(&mut self) {
        let (Some(shared), Some(editor)) = (&mut self.shared, &mut self.editor) else {
            return;
        };
        if shared.in_flight.is_some() {
            return;
        }
        if let Some((base, request)) = editor.take_backend_request_at_revision() {
            let lease_command = matches!(&request, BackendRequest::Edit(command) if CommandDto::from_command(command).is_ok_and(|c| shared.leases.touches(&c.scope(editor.document()).objects)));
            let result = match request {
                BackendRequest::Edit(command) => {
                    let command = normalize_queued_import(command, editor.document());
                    let commands = match &command {
                        tack_core::Command::Batch(edits) => edits.as_slice(),
                        other => std::slice::from_ref(other),
                    };
                    if commands.iter().any(|edit| {
                        matches!(
                            edit,
                            tack_core::Command::AddSource(_) | tack_core::Command::SetSource(_)
                        )
                    }) {
                        let needed: HashSet<_> = commands
                            .iter()
                            .filter_map(|edit| match edit {
                                tack_core::Command::AddSource(source)
                                | tack_core::Command::SetSource(source) => {
                                    Some((source.id(), source.revision()))
                                }
                                _ => None,
                            })
                            .collect();
                        let originals = self
                            .local
                            .originals
                            .iter()
                            .filter(|(key, _)| needed.contains(key))
                            .map(|((id, rev), payload)| (*id, *rev, payload.clone()))
                            .collect();
                        let submitted = shared.client.prepare_edit(
                            base,
                            command,
                            originals,
                            self.options.path.clone(),
                        );
                        self.local.originals.retain(|key, _| !needed.contains(key));
                        submitted
                    } else {
                        CommandDto::from_command(&command)
                            .and_then(|command| shared.client.edit(base, command))
                    }
                }
                BackendRequest::Undo => shared.client.undo(base),
                BackendRequest::Redo => shared.client.redo(base),
            };
            match result {
                Ok(operation) => {
                    shared.in_flight = Some(operation);
                    if lease_command {
                        shared.leases.submitted(operation);
                    }
                }
                Err(error) => {
                    self.interaction_error = Some(error.to_string());
                    self.dirty = true;
                }
            }
        }
    }
}

/// Consecutive imports of the same local path share source/asset IDs. The first
/// acknowledgement may arrive after both requests were constructed. Preserve
/// command strictness generally, adapting only this existing import batch shape.
fn normalize_queued_import(
    command: tack_core::Command,
    document: &tack_core::Document,
) -> tack_core::Command {
    use tack_core::{Command, ObjectKind};
    let Command::Batch(mut edits) = command else {
        return command;
    };
    if edits
        .iter()
        .any(|edit| matches!(edit, Command::AddSource(_)))
        && edits.iter().any(|edit| {
            matches!(edit, Command::AddObject { object, .. }
            if matches!(object.kind(), ObjectKind::Image(_)))
        })
    {
        edits.retain(|edit| match edit {
            Command::AddSource(source) => document
                .source(source.id())
                .is_none_or(|existing| existing.revision() != source.revision()),
            Command::AddAsset(asset) => document.asset(asset.id()) != Some(asset),
            _ => true,
        });
        for edit in &mut edits {
            if let Command::AddObject { index, .. } = edit {
                *index = document.object_order().len();
            }
        }
    }
    Command::Batch(edits)
}
impl SharedState {
    pub fn request_visible(&mut self, editor: &DocumentEditor, draws: &[DrawProductImage]) {
        let shared = self;
        if !shared.evicted.is_empty() {
            let visible: HashSet<_> = draws
                .iter()
                .filter_map(|draw| {
                    let source = editor
                        .document()
                        .asset(draw.data.asset_id)
                        .and_then(|asset| editor.document().source(asset.source_id()))?;
                    Some((source.id(), source.revision()))
                })
                .collect();
            shared.evicted.retain(|key| {
                if visible.contains(key) {
                    true
                } else {
                    shared.requested.remove(key);
                    false
                }
            });
        }
        for draw in draws {
            let Some(source) = editor
                .document()
                .asset(draw.data.asset_id)
                .and_then(|asset| editor.document().source(asset.source_id()))
            else {
                continue;
            };
            let key = (source.id(), source.revision());
            if shared.requested.contains(&key) {
                continue;
            }
            if let Some(binding) = shared.bindings.get(&source.id())
                && shared.client.request_asset(binding.clone()).is_ok()
            {
                shared.requested.insert(key);
            }
        }
    }
}
