//! One owned storage worker. No queue, source reimport or event-thread file I/O.
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{Arc, mpsc},
    thread::JoinHandle,
};
use tack_assets::{AssetError, PreparedOverview};
use tack_core::{Document, DocumentEditor};
use tack_storage::{BlobInput, BoardLease, Payload, TackFile};
pub type Originals = BTreeMap<(tack_core::SourceId, u64), Payload>;
/// Preserve stable originals needed by undo before releasing an opened generation.
/// This transfers Arc/range metadata only; payload I/O stays on the save worker.
pub fn retain_originals(
    previous: &TackFile,
    publication: &TackFile,
    editor: &DocumentEditor,
    originals: &mut Originals,
) {
    for key @ (id, revision) in editor.required_originals() {
        if let Some(entry) = publication
            .originals
            .get(&id)
            .filter(|e| e.revision == revision)
        {
            originals.insert(key, publication.payload(entry.range));
        } else if let Some(entry) = previous
            .originals
            .get(&id)
            .filter(|e| e.revision == revision)
        {
            originals
                .entry(key)
                .or_insert_with(|| previous.payload(entry.range));
        }
    }
}

pub struct SaveCompletion {
    pub generation: u64,
    pub result: Result<(), AssetError>,
    pub recovery: bool,
    pub bytes: u64,
    pub elapsed_ms: f64,
    publication: Option<Arc<TackFile>>,
    new_owner: Option<(Arc<BoardLease>, bool)>,
}
#[derive(Default)]
pub struct ImageSave {
    active: Option<(mpsc::Receiver<SaveCompletion>, JoinHandle<()>)>,
    pub last_error: Option<String>,
    pub completed: usize,
    pub recoveries: usize,
    pub recovery_generation: Option<u64>,
    pub recovery_bytes: u64,
    pub last_worker_ms: f64,
    pub publication: Option<Arc<TackFile>>,
    pub new_owner: Option<(Arc<BoardLease>, bool)>,
}
impl ImageSave {
    pub fn active(&self) -> bool {
        self.active.is_some()
    }
    pub fn start(
        &mut self,
        path: PathBuf,
        board: Arc<TackFile>,
        editor: &DocumentEditor,
        prepared: &BTreeMap<tack_core::AssetId, PreparedOverview>,
    ) -> Result<bool, AssetError> {
        if self.active() {
            return Ok(false);
        }
        let document = editor.document().clone();
        let generation = editor.generation();
        let prepared = prepared.clone();
        let (tx, rx) = mpsc::sync_channel(1);
        let worker = std::thread::Builder::new()
            .name("tack-document-save".into())
            .spawn(move || {
                let result = save_snapshot(path, &board, &document, &prepared);
                let _ = tx.send(SaveCompletion {
                    generation,
                    result,
                    recovery: false,
                    bytes: 0,
                    elapsed_ms: 0.,
                    publication: None,
                    new_owner: None,
                });
            })?;
        self.last_error = None;
        self.active = Some((rx, worker));
        Ok(true)
    }
    pub fn start_owned(
        &mut self,
        lease: Arc<BoardLease>,
        board: Arc<TackFile>,
        editor: &DocumentEditor,
        prepared: &BTreeMap<tack_core::AssetId, PreparedOverview>,
        originals: &Originals,
        recovery: bool,
    ) -> Result<bool, AssetError> {
        if self.active() {
            return Ok(false);
        }
        let document = editor.document().clone();
        let generation = editor.generation();
        let prepared = prepared.clone();
        let originals = originals.clone();
        let (tx, rx) = mpsc::sync_channel(1);
        let worker = std::thread::Builder::new()
            .name("tack-document-save".into())
            .spawn(move || {
                let started = std::time::Instant::now();
                let mut bytes = 0;
                let mut publication = None;
                let result = (|| -> Result<(), AssetError> {
                    let inputs = snapshot_inputs(&board, &document, &prepared, &originals)?;
                    if recovery {
                        bytes = lease.save_recovery(&document, generation, inputs)?;
                    } else {
                        lease.save(&document, inputs)?;
                        publication = Some(Arc::new(lease.open()?));
                    }
                    Ok(())
                })();
                let _ = tx.send(SaveCompletion {
                    generation,
                    result,
                    recovery,
                    bytes,
                    elapsed_ms: started.elapsed().as_secs_f64() * 1000.,
                    publication,
                    new_owner: None,
                });
            })?;
        self.last_error = None;
        self.active = Some((rx, worker));
        Ok(true)
    }
    pub fn start_as(
        &mut self,
        path: PathBuf,
        source_base: PathBuf,
        board: Arc<TackFile>,
        editor: &DocumentEditor,
        prepared: &BTreeMap<tack_core::AssetId, PreparedOverview>,
        originals: &Originals,
    ) -> Result<bool, AssetError> {
        if self.active() {
            return Ok(false);
        }
        let path = crate::file_names::board(path);
        let mut document = editor.document().clone();
        let generation = editor.generation();
        let revision_floor = editor.next_source_revision();
        let prepared = prepared.clone();
        let originals = originals.clone();
        let (tx, rx) = mpsc::sync_channel(1);
        let worker = std::thread::Builder::new()
            .name("tack-document-save".into())
            .spawn(move || {
                let started = std::time::Instant::now();
                let mut publication = None;
                let mut new_owner = None;
                let result = (|| -> Result<(), AssetError> {
                    match std::fs::symlink_metadata(&path) {
                        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                        Err(e) => return Err(e.into()),
                        Ok(_) => return Err(
                            "Save As requires an unused filename; use Save for the current board"
                                .into(),
                        ),
                    }
                    let owner = Arc::new(BoardLease::acquire_new(&path)?);
                    let mut inputs = snapshot_inputs(&board, &document, &prepared, &originals)?;
                    let changed = rebase_for_save_as(
                        &mut document,
                        &source_base,
                        owner.path(),
                        revision_floor,
                    )?;
                    if changed {
                        for input in &mut inputs {
                            if !input.original {
                                let id = tack_core::AssetId::new(input.entry.id)?;
                                let source = document
                                    .asset(id)
                                    .and_then(|a| document.source(a.source_id()))
                                    .ok_or("overview source reference")?;
                                input.entry.revision = source.revision();
                            }
                        }
                    }
                    owner.save(&document, inputs)?;
                    publication = Some(Arc::new(owner.open()?));
                    new_owner = Some((owner, changed));
                    Ok(())
                })();
                let _ = tx.send(SaveCompletion {
                    generation,
                    result,
                    recovery: false,
                    bytes: 0,
                    elapsed_ms: started.elapsed().as_secs_f64() * 1000.,
                    publication,
                    new_owner,
                });
            })?;
        self.last_error = None;
        self.active = Some((rx, worker));
        Ok(true)
    }
    fn acknowledge(&mut self, completion: SaveCompletion, editor: &mut DocumentEditor) {
        self.last_worker_ms = completion.elapsed_ms;
        match completion.result {
            Ok(()) => {
                if completion.recovery {
                    self.recoveries += 1;
                    self.recovery_generation = Some(completion.generation);
                    self.recovery_bytes += completion.bytes;
                } else {
                    editor.mark_saved_generation(completion.generation);
                    self.completed += 1;
                    self.publication = completion.publication;
                    self.new_owner = completion.new_owner;
                }
                self.last_error = None;
            }
            Err(error) => self.last_error = Some(error.to_string()),
        }
    }
    /// Completion polling never waits or joins on the event/render thread.
    pub fn poll(&mut self, editor: &mut DocumentEditor) -> bool {
        let Some((rx, _)) = &self.active else {
            return false;
        };
        match rx.try_recv() {
            Ok(completion) => {
                self.active.take();
                self.acknowledge(completion, editor);
                true
            }
            Err(mpsc::TryRecvError::Empty) => false,
            Err(mpsc::TryRecvError::Disconnected) => {
                self.active.take();
                self.last_error = Some("save worker stopped before acknowledgement".into());
                true
            }
        }
    }
    /// Shutdown only: join before disposing the owned derived cache directory.
    pub fn finish(&mut self, editor: &mut DocumentEditor) {
        if let Some((rx, worker)) = self.active.take() {
            match rx.recv() {
                Ok(completion) => self.acknowledge(completion, editor),
                Err(_) => self.last_error = Some("save worker stopped".into()),
            }
            let _ = worker.join();
        }
    }
}
pub fn save_snapshot(
    path: PathBuf,
    board: &TackFile,
    document: &Document,
    prepared: &BTreeMap<tack_core::AssetId, PreparedOverview>,
) -> Result<(), AssetError> {
    // Worker-only preflight; refuse replacing symlinks/directories at an opened path.
    let metadata = std::fs::symlink_metadata(&path)?;
    if !metadata.file_type().is_file() {
        return Err("Save target must remain a regular file".into());
    }
    let inputs = snapshot_inputs(board, document, prepared, &Originals::new())?;
    tack_storage::save(path, document, inputs)?;
    Ok(())
}
/// Worker-only streaming payload collection; previews are disposable and revision-specific.
pub fn snapshot_inputs(
    board: &TackFile,
    document: &Document,
    prepared: &BTreeMap<tack_core::AssetId, PreparedOverview>,
    originals: &Originals,
) -> Result<Vec<BlobInput>, AssetError> {
    let mut inputs = Vec::with_capacity(board.originals.len() + board.overviews.len());
    for source in document.sources() {
        if !matches!(source.location(), tack_core::SourceLocation::Embedded) {
            continue;
        }
        let payload = if let Some(payload) = originals.get(&(source.id(), source.revision())) {
            payload.clone()
        } else if let Some(e) = board
            .originals
            .get(&source.id())
            .filter(|e| e.revision == source.revision())
        {
            board.payload(e.range)
        } else {
            return Err("embedded original is not available for saving".into());
        };
        inputs.push(BlobInput::original(source.id(), source.revision(), payload));
    }
    let mut pinned = 0;
    for asset in document.assets() {
        // Prefer a CRC-valid stable persisted overview. Prepared paths may be evicted
        // by cache quota; those are disposable and never carry original data.
        let revision = document
            .source(asset.source_id())
            .ok_or("missing source")?
            .revision();
        if let Some(e) = board.overviews.get(&asset.id())
            && e.revision == revision
            && board
                .document
                .asset(asset.id())
                .is_some_and(|a| a.source_id() == asset.source_id())
            && board.overview_bytes(asset.id()).is_ok()
        {
            inputs.push(BlobInput::overview(
                asset.id(),
                e.revision,
                [e.width, e.height],
                e.generator,
                board.payload(e.range),
            ));
            continue;
        }
        if pinned < 128
            && let Some(p) = prepared.get(&asset.id())
            && p.revision == revision
            && let Ok(payload) = Payload::pin_overview(&p.path)
        {
            inputs.push(BlobInput::overview(
                p.asset,
                p.revision,
                p.size,
                p.generator,
                payload,
            ));
            pinned += 1;
        }
    }
    Ok(inputs)
}

impl Drop for ImageSave {
    fn drop(&mut self) {
        if let Some((rx, worker)) = self.active.take() {
            drop(rx);
            let _ = worker.join();
        }
    }
}

/// A relocated relative descriptor becomes absolute. The caller resets history only
/// when this normalization occurred, so an old relative inverse cannot change meaning.
fn rebase_for_save_as(
    document: &mut Document,
    base: &std::path::Path,
    target: &std::path::Path,
    revision_floor: Option<u64>,
) -> Result<bool, AssetError> {
    let base = std::path::absolute(base)?;
    if base.parent() == target.parent() {
        return Ok(false);
    }
    let mut revision = revision_floor;
    let sources: Vec<_> = document
        .sources()
        .filter(
            |s| matches!(s.location(), tack_core::SourceLocation::Linked(p) if !p.is_absolute()),
        )
        .cloned()
        .collect();
    for source in &sources {
        let tack_core::SourceLocation::Linked(path) = source.location() else {
            continue;
        };
        let path = path
            .to_native()
            .ok_or("foreign relative source cannot be moved; relink it first")?;
        let path = base.parent().ok_or("source base parent")?.join(path);
        let issued = revision.ok_or("source revision exhausted")?;
        revision = issued.checked_add(1);
        document.apply(tack_core::Command::SetSource(
            tack_core::Source::from_descriptor(
                source.id(),
                tack_core::SourceLocation::Linked(tack_core::LinkedPath::native(&path)?),
                issued,
                source.fingerprint(),
            )?,
        ))?;
    }
    Ok(!sources.is_empty())
}
