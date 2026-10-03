//! One owned storage worker. No queue, source reimport or event-thread file I/O.
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{Arc, mpsc},
    thread::JoinHandle,
};
use tack_assets::{AssetError, PreparedOverview};
use tack_core::{Document, DocumentEditor};
use tack_storage::{BlobInput, Payload, TackFile};

pub struct SaveCompletion {
    pub generation: u64,
    pub result: Result<(), AssetError>,
}
#[derive(Default)]
pub struct ImageSave {
    active: Option<(mpsc::Receiver<SaveCompletion>, JoinHandle<()>)>,
    pub last_error: Option<String>,
    pub completed: usize,
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
                let _ = tx.send(SaveCompletion { generation, result });
            })?;
        self.last_error = None;
        self.active = Some((rx, worker));
        Ok(true)
    }
    fn acknowledge(&mut self, completion: SaveCompletion, editor: &mut DocumentEditor) {
        match completion.result {
            Ok(()) => {
                editor.mark_saved_generation(completion.generation);
                self.completed += 1;
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
    let mut inputs = Vec::with_capacity(board.originals.len() + board.overviews.len());
    for (id, e) in &board.originals {
        inputs.push(BlobInput::original(*id, e.revision, board.payload(e.range)));
    }
    let mut pinned = 0;
    for asset in document.assets() {
        // Prefer a CRC-valid stable persisted overview. Prepared paths may be evicted
        // by cache quota; those are disposable and never carry original data.
        if let Some(e) = board.overviews.get(&asset.id())
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
    tack_storage::save(path, document, inputs)?;
    Ok(())
}

impl Drop for ImageSave {
    fn drop(&mut self) {
        if let Some((rx, worker)) = self.active.take() {
            drop(rx);
            let _ = worker.join();
        }
    }
}
