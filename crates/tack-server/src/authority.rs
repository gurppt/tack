use crate::{HISTORY_PER_CLIENT, MAX_CLIENTS, MAX_HISTORY_BYTES, RECEIPTS_PER_CLIENT, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};
use tack_core::Document;
use tack_shared::{CommandDto, DocumentRecord, SourceBinding, WireId};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct HistoryEntry {
    before: u64,
    after: u64,
    command: CommandDto,
    sources: Vec<SourceBinding>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ClientHistory {
    undo: Vec<HistoryEntry>,
    redo: Vec<HistoryEntry>,
    receipts: Vec<Change>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Change {
    pub revision: u64,
    pub client: WireId,
    pub operation: WireId,
    pub command: CommandDto,
    pub sources: Vec<SourceBinding>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct State {
    pub version: u32,
    pub board: WireId,
    pub revision: u64,
    pub source_high_water: u64,
    pub document: DocumentRecord,
    pub sources: Vec<SourceBinding>,
    pub clients: BTreeMap<WireId, ClientHistory>,
    #[serde(default)]
    pub object_clock: Option<crate::object_clock::ObjectClock>,
}
pub struct Authority {
    path: PathBuf,
    pub(crate) state: State,
    document: Document,
    poisoned: bool,
}
impl Authority {
    pub fn publish(root: &Path, document: Document, sources: Vec<SourceBinding>) -> Result<Self> {
        let board = WireId::new(document.id().value()).map_err(|e| e.to_string())?;
        let path = root.join(format!("{board}.board"));
        if path.exists() {
            return Err("board already exists; publish never overwrites".into());
        }
        let document_record =
            DocumentRecord::from_document(&document).map_err(|e| e.to_string())?;
        let canonical = document_record.to_document().map_err(|e| e.to_string())?;
        validate_bindings(&canonical, &sources)?;
        let state = State {
            version: 1,
            board,
            revision: 0,
            source_high_water: canonical.sources().map(|s| s.revision()).max().unwrap_or(0),
            document: document_record,
            sources,
            clients: BTreeMap::new(),
            object_clock: Some(crate::object_clock::ObjectClock::new(&canonical, 0)),
        };
        ensure_joinable(&state)?;
        crate::persistence::publish(&path, &state)?;
        Ok(Self {
            path,
            state,
            document: canonical,
            poisoned: false,
        })
    }
    pub fn open(root: &Path, board: WireId) -> Result<Self> {
        let path = root.join(format!("{board}.board"));
        let mut state = crate::persistence::read(&path)?;
        #[cfg(unix)]
        crate::persistence::sync_directory(root)
            .map_err(|e| format!("authority directory durability unavailable: {e}"))?;
        if state.version != 1 || state.board != board || state.clients.len() > MAX_CLIENTS {
            return Err("invalid authority version, board or clients".into());
        }
        let document = state.document.to_document().map_err(|e| e.to_string())?;
        let clock = state.object_clock.get_or_insert_with(|| {
            crate::object_clock::ObjectClock::new(&document, state.revision)
        });
        clock.validate(&document, state.revision)?;
        if document.id().value() != board.value() {
            return Err("authority identity mismatch".into());
        }
        validate_bindings(&document, &state.sources)?;
        ensure_joinable(&state)?;
        if state.source_high_water < document.sources().map(|s| s.revision()).max().unwrap_or(0) {
            return Err("authority source high water".into());
        }
        for (client, history) in &state.clients {
            if serde_json::to_vec(history)
                .map_err(|e| e.to_string())?
                .len()
                > MAX_HISTORY_BYTES
            {
                return Err("authority history byte bounds".into());
            }
            if history.undo.len() > HISTORY_PER_CLIENT
                || history.redo.len() > HISTORY_PER_CLIENT
                || history.receipts.len() > RECEIPTS_PER_CLIENT
            {
                return Err("authority history bounds".into());
            }
            for entry in history.undo.iter().chain(&history.redo) {
                entry.command.to_command().map_err(|e| e.to_string())?;
                validate_binding_delta(&entry.command, &entry.sources)?;
                for binding in &entry.sources {
                    binding.validate().map_err(|e| e.to_string())?;
                }
                if entry.after > state.revision || entry.before >= entry.after {
                    return Err("authority history revision".into());
                }
            }
            let mut previous = 0;
            let mut operations = std::collections::BTreeSet::new();
            for receipt in &history.receipts {
                receipt.command.to_command().map_err(|e| e.to_string())?;
                validate_binding_delta(&receipt.command, &receipt.sources)?;
                for binding in &receipt.sources {
                    binding.validate().map_err(|e| e.to_string())?;
                }
                if receipt.client != *client
                    || receipt.revision <= previous
                    || !operations.insert(receipt.operation)
                {
                    return Err("authority receipt identity/order".into());
                }
                previous = receipt.revision;
                if receipt.revision > state.revision {
                    return Err("authority receipt revision".into());
                }
            }
        }
        Ok(Self {
            path,
            state,
            document,
            poisoned: false,
        })
    }
    pub fn is_available(&self) -> bool {
        !self.poisoned
    }
    pub fn board(&self) -> WireId {
        self.state.board
    }
    pub fn revision(&self) -> u64 {
        self.state.revision
    }
    pub fn source_high_water(&self) -> u64 {
        self.state.source_high_water
    }
    pub fn document(&self) -> &Document {
        &self.document
    }
    pub fn sources(&self) -> &[SourceBinding] {
        &self.state.sources
    }
    pub fn record(&self) -> &DocumentRecord {
        &self.state.document
    }
    pub fn check_objects(&self, base: u64, objects: &[WireId]) -> Result<()> {
        let scope = tack_shared::CommandScope {
            objects: objects.iter().copied().collect(),
            ..Default::default()
        };
        self.clock()?.check(&scope, base, self.revision())
    }
    pub fn history_command(&self, client: WireId, redo: bool) -> Option<&CommandDto> {
        let history = self.state.clients.get(&client)?;
        (if redo { &history.redo } else { &history.undo })
            .last()
            .map(|entry| &entry.command)
    }
    fn clock(&self) -> Result<&crate::object_clock::ObjectClock> {
        self.state
            .object_clock
            .as_ref()
            .ok_or_else(|| "object conflict clock unavailable".into())
    }
    pub fn receipt(&self, client: WireId, operation: WireId) -> Option<Change> {
        self.state
            .clients
            .get(&client)?
            .receipts
            .iter()
            .find(|r| r.operation == operation)
            .cloned()
    }
    pub fn edit(
        &mut self,
        client: WireId,
        operation: WireId,
        base: u64,
        command: CommandDto,
        sources: Vec<SourceBinding>,
    ) -> Result<Change> {
        if self.poisoned {
            return Err("authority publication uncertain; rejoin required".into());
        }
        if let Some(receipt) = self.receipt(client, operation) {
            return Ok(receipt);
        }
        let scope = command.scope(&self.document);
        self.clock()?.check(&scope, base, self.revision())?;
        validate_binding_delta(&command, &sources)?;
        let next_high_water = source_revision(&command, self.state.source_high_water)?;
        let mut document = self.document.clone();
        let inverse = document
            .apply_with_inverse(command.to_command().map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        let inverse = inverse.ok_or("operation made no change")?;
        let mut state = self.state.clone();
        admit_history(&mut state, client);
        state.source_high_water = next_high_water;
        let history = state.clients.entry(client).or_default();
        history.redo.clear();
        history.undo.push(HistoryEntry {
            before: self.revision(),
            after: self
                .revision()
                .checked_add(1)
                .ok_or("authority revision exhausted")?,
            command: transmissible_inverse(
                &inverse,
                self.state.board,
                client,
                operation,
                self.revision()
                    .checked_add(1)
                    .ok_or("authority revision exhausted")?,
                &self.state.sources,
            )?,
            sources: inverse_bindings(&inverse, &self.state.sources),
        });
        trim(history);
        self.commit(state, document, client, operation, command, sources)
    }
    pub fn history(
        &mut self,
        client: WireId,
        operation: WireId,
        base: u64,
        redo: bool,
    ) -> Result<Change> {
        if self.poisoned {
            return Err("authority publication uncertain; rejoin required".into());
        }
        if let Some(receipt) = self.receipt(client, operation) {
            return Ok(receipt);
        }
        let mut state = self.state.clone();
        admit_history(&mut state, client);
        let history = state.clients.entry(client).or_default();
        let entries = if redo {
            &mut history.redo
        } else {
            &mut history.undo
        };
        let entry = entries.pop().ok_or("history is empty")?;
        let scope = entry.command.scope(&self.document);
        self.clock()?.check(&scope, base, self.revision())?;
        self.clock()?
            .check(&scope, entry.after, self.revision())
            .map_err(|e| format!("undo conflict: {e}"))?;
        let next_revision = self
            .revision()
            .checked_add(1)
            .ok_or("authority revision exhausted")?;
        let mut document = self.document.clone();
        let inverse = document
            .apply_inverse(entry.command.to_command().map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?
            .ok_or("undo made no change")?;
        if let Some(previous) = entries.last_mut()
            && previous.after == entry.before
            && previous.command.scope(&self.document).objects == scope.objects
        {
            previous.after = next_revision;
        }
        let destination = if redo {
            &mut history.undo
        } else {
            &mut history.redo
        };
        destination.push(HistoryEntry {
            before: self.revision(),
            after: next_revision,
            command: transmissible_inverse(
                &inverse,
                self.state.board,
                client,
                operation,
                self.revision()
                    .checked_add(1)
                    .ok_or("authority revision exhausted")?,
                &self.state.sources,
            )?,
            sources: inverse_bindings(&inverse, &self.state.sources),
        });
        trim(history);
        self.commit(
            state,
            document,
            client,
            operation,
            entry.command,
            entry.sources,
        )
    }
    fn commit(
        &mut self,
        mut state: State,
        document: Document,
        client: WireId,
        operation: WireId,
        command: CommandDto,
        sources: Vec<SourceBinding>,
    ) -> Result<Change> {
        let revision = self
            .state
            .revision
            .checked_add(1)
            .ok_or("authority revision exhausted")?;
        let scope = command.scope(&self.document);
        state
            .object_clock
            .as_mut()
            .ok_or("object conflict clock unavailable")?
            .advance(&scope, &document, revision);
        let mut bindings: BTreeMap<_, _> = state
            .sources
            .into_iter()
            .map(|b| ((b.source, b.revision), b))
            .collect();
        for binding in &sources {
            bindings.insert((binding.source, binding.revision), binding.clone());
        }
        state.sources = bindings
            .into_values()
            .filter(|b| {
                tack_core::SourceId::new(b.source.value())
                    .ok()
                    .and_then(|id| document.source(id))
                    .is_some_and(|s| s.revision() == b.revision)
            })
            .collect();
        validate_bindings(&document, &state.sources)?;
        ensure_joinable(&state)?;
        state.document = DocumentRecord::from_document(&document).map_err(|e| e.to_string())?;
        state.revision = revision;
        let change = Change {
            revision,
            client,
            operation,
            command,
            sources,
        };
        tack_shared::encode_message(&tack_shared::Message::Accepted {
            board: state.board,
            revision,
            client,
            operation,
            command: change.command.clone(),
            sources: change.sources.clone(),
        })
        .map_err(|e| e.to_string())?;
        ensure_joinable(&state)?;
        let history = state.clients.entry(client).or_default();
        history.receipts.push(change.clone());
        if history.receipts.len() > RECEIPTS_PER_CLIENT {
            history.receipts.remove(0);
        }
        trim(history);
        if let Err(error) = crate::persistence::publish(&self.path, &state) {
            if error.starts_with("authority published but") {
                self.state = state;
                self.document = document;
                self.poisoned = true;
            }
            return Err(error);
        }
        self.state = state;
        self.document = document;
        Ok(change)
    }
}
fn trim(history: &mut ClientHistory) {
    for entries in [&mut history.undo, &mut history.redo] {
        while entries.len() > HISTORY_PER_CLIENT {
            entries.remove(0);
        }
    }
    while serde_json::to_vec(history).map_or(true, |v| v.len() > MAX_HISTORY_BYTES) {
        if !history.undo.is_empty() {
            history.undo.remove(0);
        } else if !history.redo.is_empty() {
            history.redo.remove(0);
        } else if !history.receipts.is_empty() {
            history.receipts.remove(0);
        } else {
            break;
        }
    }
}
fn validate_bindings(document: &Document, bindings: &[SourceBinding]) -> Result<()> {
    if bindings.len() > tack_storage::MAX_RECORDS {
        return Err("too many source bindings".into());
    }
    let mut seen = std::collections::BTreeSet::new();
    for binding in bindings {
        if binding.revision == 0
            || binding.size > tack_shared::MAX_ASSET_BYTES
            || !seen.insert(binding.source)
        {
            return Err("invalid or duplicate source binding".into());
        }
        let source = document
            .source(tack_core::SourceId::new(binding.source.value()).map_err(|e| e.to_string())?)
            .ok_or("binding references missing source")?;
        if source.revision() != binding.revision {
            return Err("binding source revision mismatch".into());
        }
    }
    Ok(())
}

fn inverse_bindings(
    command: &tack_core::Command,
    bindings: &[SourceBinding],
) -> Vec<SourceBinding> {
    let edits = match command {
        tack_core::Command::Batch(edits) => edits.as_slice(),
        other => std::slice::from_ref(other),
    };
    bindings.iter().filter(|b|edits.iter().any(|e|matches!(e,tack_core::Command::AddSource(s)|tack_core::Command::SetSource(s) if s.id().value()==b.source.value() && s.revision()==b.revision))).cloned().collect()
}
fn source_revision(command: &CommandDto, high: u64) -> Result<u64> {
    match command {
        CommandDto::SetSource { source } => {
            if source.revision <= high {
                Err("source revision must increase above persisted high water".into())
            } else {
                Ok(source.revision)
            }
        }
        CommandDto::AddSource { source } => Ok(high.max(source.revision)),
        CommandDto::Batch { edits } => edits
            .iter()
            .try_fold(high, |water, e| source_revision(e, water)),
        _ => Ok(high),
    }
}

#[cfg(all(test, unix))]
mod publication_tests {
    use super::*;
    #[test]
    fn postrename_sync_failure_poison_is_recoverable_without_revision_reuse() -> Result<()> {
        let board = tack_storage::new_document_id().map_err(|e| e.to_string())?;
        let root = std::env::temp_dir().join(format!("tack-authority-sync-{}", board.value()));
        std::fs::create_dir(&root).map_err(|e| e.to_string())?;
        let doc = Document::new(board, tack_core::DocumentLimits::default());
        let mut authority = Authority::publish(&root, doc, vec![])?;
        let object = tack_core::DocumentObject::frame(
            tack_core::ObjectId::new(2).map_err(|e| e.to_string())?,
            "frame".into(),
            tack_core::Transform::new([0., 0.], [10., 10.], 0., [false, false])
                .map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        let command = CommandDto::from_command(&tack_core::Command::AddObject { object, index: 0 })
            .map_err(|e| e.to_string())?;
        let client = WireId::new(3).map_err(|e| e.to_string())?;
        let operation = WireId::new(4).map_err(|e| e.to_string())?;
        crate::persistence::FAIL_SYNC.with(|v| v.set(true));
        let result = authority.edit(client, operation, 0, command.clone(), vec![]);
        crate::persistence::FAIL_SYNC.with(|v| v.set(false));
        assert!(result.is_err());
        assert!(!authority.is_available());
        assert_eq!(authority.revision(), 1);
        assert!(
            authority
                .edit(client, operation, 0, command, vec![])
                .is_err()
        );
        let reopened = Authority::open(&root, authority.board())?;
        assert_eq!(reopened.revision(), 1);
        assert!(reopened.receipt(client, operation).is_some());
        std::fs::remove_dir_all(root).map_err(|e| e.to_string())?;
        Ok(())
    }
}

fn admit_history(state: &mut State, client: WireId) {
    if !state.clients.contains_key(&client) && state.clients.len() >= MAX_CLIENTS {
        let oldest = state
            .clients
            .iter()
            .min_by_key(|(id, h)| (h.receipts.last().map_or(0, |r| r.revision), **id))
            .map(|(id, _)| *id);
        if let Some(oldest) = oldest {
            state.clients.remove(&oldest);
        }
    }
}
fn validate_binding_delta(command: &CommandDto, bindings: &[SourceBinding]) -> Result<()> {
    let edits = match command {
        CommandDto::Batch { edits } => edits.as_slice(),
        other => std::slice::from_ref(other),
    };
    if bindings.iter().any(|b|!edits.iter().any(|e|matches!(e,CommandDto::AddSource{source}|CommandDto::SetSource{source} if source.id==b.source && source.revision==b.revision))) {return Err("source binding requires matching source replacement command".into());}
    Ok(())
}

fn ensure_joinable(state: &State) -> Result<()> {
    // Metadata is ASCII hex. 240 bytes conservatively covers every binding's
    // JSON keys, punctuation, fixed IDs/hash and maximum decimal lengths.
    let maximum = state
        .document
        .metadata
        .len()
        .saturating_add(state.sources.len().saturating_mul(240))
        .saturating_add(512);
    if maximum > tack_shared::MAX_FRAME_BYTES {
        return Err("authoritative snapshot exceeds reconnect frame budget".into());
    }
    Ok(())
}

// Admit generated history only if a later undo/redo can be broadcast within
// the same wire budget as the original edit. This runs before publication.
fn transmissible_inverse(
    inverse: &tack_core::Command,
    board: WireId,
    client: WireId,
    operation: WireId,
    revision: u64,
    bindings: &[SourceBinding],
) -> Result<CommandDto> {
    let command = CommandDto::from_command(inverse).map_err(|e| e.to_string())?;
    tack_shared::encode_message(&tack_shared::Message::Accepted {
        board,
        revision,
        client,
        operation,
        command: command.clone(),
        sources: inverse_bindings(inverse, bindings),
    })
    .map_err(|e| format!("generated undo/redo exceeds protocol budget: {e}"))?;
    Ok(command)
}
