//! Local workflows compose the existing editor, storage worker, input and canvas.
use super::*;
mod daily;
mod fresh;
mod installer;
mod updates;
mod workflows;
use tack_app::{
    actions::Action,
    image_save::Originals,
    local_import::{ImportRequest, ImportUpdate},
    local_ui::{LocalUi, Panel, UiResult},
    local_worker::{Clipboard, LocalUpdate, LocalWorker, Operation},
    native_files::Picker,
    preferences::{self, Preferences},
    recovery_schedule::RecoverySchedule,
};
pub(super) struct LoadedBoard {
    pub board: TackFile,
    pub lease: Arc<tack_storage::BoardLease>,
    pub metadata_ms: f64,
    pub path: PathBuf,
    pub recovery: bool,
    pub warning: Option<String>,
}
pub(super) struct LocalState {
    pub about_ticket: u64,
    pub lease: Option<Arc<tack_storage::BoardLease>>,
    pub worker: LocalWorker,
    pub queued: Option<Operation>,
    pub ui: Option<Box<LocalUi>>,
    pub profile: Preferences,
    pub root: PathBuf,
    pub profile_base: Option<u32>,
    pub profile_protected: bool,
    pub profile_pending: bool,
    pub profile_changed: bool,
    pub recovery: RecoverySchedule,
    pub recovery_pending: bool,
    pub manual: bool,
    pub save_as: Option<PathBuf>,
    pub saving_as: bool,
    pub originals: Originals,
    pub spool: Option<Arc<std::fs::File>>,
    cache_generation: Option<u64>,
    pub seed: Option<(Arc<tack_storage::BoardLease>, tack_core::DocumentId)>,
    retire_seed: bool,
    pub seed_remembered: bool,
    pub importing: bool,
    pub import_status: String,
    pub import_rejected: usize,
    pub close_after_save: bool,
    pub close_after_discard: bool,
    pub close_ready: bool,
    pub drops: Vec<PathBuf>,
    pub drop_deadline: Option<Instant>,
    pub untitled: bool,
}
impl LocalState {
    pub fn new() -> Result<Self, AssetError> {
        let root = std::path::absolute(preferences::profile_root()?)?;
        let path = root.join("preferences.json");
        let profile_base = preferences::fingerprint(&path).ok().flatten();
        let (profile, protected, warning) = match preferences::read(&path) {
            Ok(profile) => (profile, false, None),
            Err(e)
                if e.downcast_ref::<std::io::Error>()
                    .is_some_and(|e| e.kind() == std::io::ErrorKind::NotFound) =>
            {
                (Preferences::defaults()?, false, None)
            }
            Err(e) => (
                Preferences::defaults()?,
                true,
                Some(format!("Preferences retained unchanged: {e}")),
            ),
        };
        let ui = warning.map(|message| {
            let mut ui = LocalUi::new(Panel::Error);
            ui.message = message;
            Box::new(ui)
        });
        Ok(Self {
            about_ticket: 0,
            lease: None,
            worker: LocalWorker::default(),
            queued: None,
            ui,
            profile,
            root,
            profile_base,
            profile_protected: protected,
            profile_pending: false,
            profile_changed: false,
            recovery: RecoverySchedule::default(),
            recovery_pending: false,
            manual: false,
            save_as: None,
            saving_as: false,
            originals: Originals::new(),
            spool: None,
            cache_generation: None,
            seed: None,
            retire_seed: false,
            seed_remembered: false,
            importing: false,
            import_status: String::new(),
            import_rejected: 0,
            close_after_save: false,
            close_after_discard: false,
            close_ready: false,
            drops: Vec::new(),
            drop_deadline: None,
            untitled: false,
        })
    }
}
impl App {
    pub(super) fn fail_load(&mut self) {
        self.release_about();
        // The empty editor used to draw the error window has no writable board.
        // Keep this separate from recoverable errors in an opened document.
        self.load_failed = true;
        self.input.cancel();
        self.local.worker.cancel();
        self.local.queued = None;
        self.local.lease = None;
        self.local.seed = None;
        self.board = None;
        self.assets = None;
        self.local.originals.clear();
        self.local.spool = None;
        self.local.recovery = RecoverySchedule::default();
        self.local.recovery_pending = false;
        self.local.drops.clear();
        self.local.drop_deadline = None;
        self.local.manual = false;
        self.local.save_as = None;
        self.local.saving_as = false;
        self.local.importing = false;
        self.local.import_status.clear();
        self.local.profile_pending = false;
        self.local.retire_seed = false;
        self.local.close_after_save = false;
        self.local.close_after_discard = false;
    }
    pub(super) fn local_error(&mut self, message: impl Into<String>) {
        self.release_about();
        self.context = None;
        let message = message.into();
        eprintln!(
            "[tack/local] {}",
            message.chars().take(2048).collect::<String>()
        );
        self.interaction_error = Some(message.clone());
        let mut panel = LocalUi::new(Panel::Error);
        panel.message = message;
        self.local.ui = Some(Box::new(panel));
        self.dirty = true;
    }
    pub(super) fn panel(&mut self, panel: Panel) {
        self.context = None;
        self.release_about();
        if let Some(editor) = &mut self.editor {
            let _ = self.input.physical(
                tack_app::input::PhysicalEvent::FocusLost,
                editor,
                &mut self.camera,
            );
        }
        self.local.ui = Some(Box::new(LocalUi::new(panel)));
        self.dirty = true;
    }
    pub(super) fn release_about(&mut self) {
        if let Some(gpu) = &mut self.gpu {
            gpu.clear_ui_image();
        }
        if matches!(self.local.queued, Some(Operation::About { .. })) {
            self.local.queued = None;
        }
    }
    pub(super) fn operation(&mut self, operation: Operation) -> Result<(), AssetError> {
        if self.load_failed {
            return Err("close the failed-open window before opening another board".into());
        }
        if self.local.worker.active() {
            if self.local.queued.is_some() {
                return Err("another local operation is queued; Escape cancels active work".into());
            }
            self.local.queued = Some(operation);
            return Ok(());
        }
        let operation = match operation {
            Operation::Import(mut request) => {
                request.spool = self.local.spool.clone();
                Operation::Import(request)
            }
            other => other,
        };
        let proxy = self.proxy.clone();
        self.local.worker.start(operation, move || {
            let _ = proxy.send_event(Event::LocalReady);
        })
    }
    pub(super) fn poll_local(&mut self) {
        if self.load_failed {
            // Drain canceled workers without applying any late document result.
            let _ = self.local.worker.poll();
            return;
        }
        if self.local.saving_as {
            return;
        } // bounded channel preserves every pending mutation
        self.local.importing = self.local.worker.importing()
            || matches!(self.local.queued, Some(Operation::Import(_)));
        if !(self.local.worker.mutating()
            && (self.input.active() || self.input.name_edit.is_some()))
        {
            for update in self.local.worker.poll() {
                if let Err(error) = self.local_update(update) {
                    self.local_error(error.to_string());
                }
            }
        }
        if let Err(error) = self.notify_join_receipt() {
            self.interaction_error = Some(error.to_string());
        }
        if !self.local.worker.active()
            && let Some(op) = self.local.queued.take()
            && let Err(error) = self.operation(op)
        {
            self.local_error(error.to_string());
        }
        if self
            .local
            .drop_deadline
            .is_some_and(|d| Instant::now() >= d)
        {
            self.local.drop_deadline = None;
            let paths = std::mem::take(&mut self.local.drops);
            let result = if paths.len() == 1 && paths[0].extension().is_some_and(|e| e == "tack") {
                self.open_board(paths[0].clone())
            } else {
                self.import_paths(paths, false)
            };
            if let Err(error) = result {
                self.local_error(error.to_string());
            }
        }
        self.local.importing = self.local.worker.importing()
            || matches!(self.local.queued, Some(Operation::Import(_)));
        if !self.local.importing && self.local.import_status.starts_with("Import pending") {
            self.local.import_status.clear();
        }
        if self.local.retire_seed && !self.local.worker.active() {
            self.local.retire_seed = false;
            if let Some((lease, id)) = self.local.seed.take()
                && let Err(error) = self.operation(Operation::RetireSeed { lease, id })
            {
                self.local_error(error.to_string());
            }
        }
        if self.local.profile_pending && !self.local.worker.active() && !self.local.importing {
            self.local.profile_pending = false;
            if self.local.profile_protected {
                self.interaction_error = Some(
                    "Invalid/future preferences retained; export a new profile instead".into(),
                );
            } else {
                let changed = std::mem::take(&mut self.local.profile_changed);
                let op = Operation::Profile {
                    root: self.local.root.clone(),
                    profile: self.local.profile.clone(),
                    base: self.local.profile_base,
                    changed,
                    board: (!self.local.untitled).then(|| self.options.path.clone()),
                };
                if let Err(error) = self.operation(op) {
                    self.local_error(error.to_string());
                }
            }
        }
    }
    pub(super) fn poll_storage(&mut self) {
        if self.shared.is_some() {
            return;
        }
        if self.load_failed {
            return;
        }
        let Some(editor) = &mut self.editor else {
            return;
        };
        let finished = self.save.poll(editor);
        self.dirty |= finished;
        if finished {
            self.local.saving_as = false;
            if let Some(error) = self.save.last_error.clone() {
                self.local.close_after_save = false;
                self.local_error(format!("Save/recovery failed: {error}"));
            }
        }
        if let Some(board) = self.save.publication.take() {
            if let Some((lease, reset)) = self.save.new_owner.take() {
                self.options.path = lease.path().to_owned();
                self.local.lease = Some(lease);
                self.local.untitled = false;
                self.local.retire_seed = self.local.seed.is_some();
                if reset {
                    self.visibility.invalidate();
                    self.editor = Some(DocumentEditor::new(board.document.clone(), 200));
                    self.local.cache_generation = None;
                }
                self.local.profile.remember(&self.options.path).ok();
                self.local
                    .profile
                    .remember_board_directory(&self.options.path)
                    .ok();
                self.local.profile_pending = true;
            }
            if let (Some(previous), Some(editor)) = (&self.board, &self.editor) {
                tack_app::image_save::retain_originals(
                    previous,
                    &board,
                    editor,
                    &mut self.local.originals,
                );
            }
            if let Some(assets) = &mut self.assets {
                assets.set_board(Arc::clone(&board));
            }
            self.board = Some(board);
        }
        let Some(editor) = &self.editor else {
            return;
        };
        if self.local.untitled && editor.is_dirty() && !self.local.seed_remembered {
            self.local.seed_remembered = true;
            self.local.profile.remember(&self.options.path).ok();
            self.local.profile_pending = true;
        }
        if self.local.cache_generation != Some(editor.generation()) {
            self.local.cache_generation = Some(editor.generation());
            if let Some(assets) = &mut self.assets {
                assets.sync_document(editor.document());
            }
            let needed: std::collections::BTreeSet<_> =
                editor.required_originals().into_iter().collect();
            self.local.originals.retain(|key, _| needed.contains(key));
        }
        self.local
            .recovery
            .observe(editor.generation(), editor.is_dirty(), Instant::now());
        if self.local.close_after_save
            && !self.save.active()
            && !self.local.worker.active()
            && !self.local.importing
        {
            if editor.is_dirty() && self.save.last_error.is_none() && !self.local.untitled {
                self.local.manual = true;
            } else if !editor.is_dirty() {
                self.local.close_ready = true;
            }
        }
        if self.save.active() || self.local.recovery_pending {
            return;
        }
        let (Some(board), Some(assets), Some(lease)) =
            (&self.board, &self.assets, &self.local.lease)
        else {
            return;
        };
        let result = if let Some(path) = self.local.save_as.take() {
            self.local.saving_as = true;
            self.input.cancel();
            self.save.start_as(
                path,
                self.options.path.clone(),
                Arc::clone(board),
                editor,
                &assets.prepared,
                &self.local.originals,
            )
        } else if self.local.manual {
            self.local.manual = false;
            self.save.start_owned(
                Arc::clone(lease),
                Arc::clone(board),
                editor,
                &assets.prepared,
                &self.local.originals,
                false,
            )
        } else if self.local.recovery.ready(Instant::now()) {
            self.local.recovery.started();
            self.save.start_owned(
                Arc::clone(lease),
                Arc::clone(board),
                editor,
                &assets.prepared,
                &self.local.originals,
                true,
            )
        } else {
            return;
        };
        if let Err(error) = result {
            self.local.saving_as = false;
            self.local.close_after_save = false;
            self.save.last_error = Some(error.to_string());
            self.local_error(format!("Save/recovery failed: {error}"));
        }
    }
}
