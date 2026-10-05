//! Local workflows compose the existing editor, storage worker, input and canvas.
use super::*;
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
    fn operation(&mut self, operation: Operation) -> Result<(), AssetError> {
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
    pub(super) fn apply_preferences(&mut self) -> Result<(), AssetError> {
        self.input.keymap = self.local.profile.keymap()?;
        self.input.gizmo.style.handle_size = f64::from(self.local.profile.handle_size);
        self.input.gizmo.style.hit_radius = f64::from(self.local.profile.hit_radius);
        let scale = if self.local.profile.ui_scale > 0 {
            f64::from(self.local.profile.ui_scale)
        } else {
            self.window.as_ref().map_or(1., |w| w.scale_factor())
        };
        self.camera.set_ui_scale(scale);
        self.input.gizmo.set_scale(scale);
        self.dirty = true;
        Ok(())
    }
    fn import_paths(
        &mut self,
        paths: Vec<PathBuf>,
        force_embedded: bool,
    ) -> Result<(), AssetError> {
        let request = ImportRequest {
            paths,
            embedded: force_embedded || self.local.profile.embedded_import,
            position: self.camera.screen_to_world(self.input.cursor()),
            sampling: if self.local.profile.sampling == "Nearest" {
                tack_core::ImageFiltering::Nearest
            } else {
                tack_core::ImageFiltering::Smooth
            },
            work: self.work.join("imports"),
            spool: self.local.spool.clone(),
        };
        self.operation(Operation::Import(request))?;
        self.local.importing = true;
        self.local.import_status = "Import pending · Escape cancels remaining files".into();
        Ok(())
    }
    pub(super) fn local_action(&mut self, action: Action) -> Result<(), AssetError> {
        if self.load_failed {
            return Ok(());
        }
        use Action::*;
        if action != Paste
            && let Some(editor) = &mut self.editor
        {
            self.input.commit_drafts(editor)?;
        }
        self.local.ui = None;
        match action {
            ApplicationMenu => self.panel(Panel::Menu),
            Preferences => self.panel(Panel::Preferences),
            KeymapEditor => self.panel(Panel::Keymap),
            RecentBoards => self.panel(Panel::Recent),
            NewBoard => self.operation(Operation::Launch {
                path: None,
                new: true,
            })?,
            Paste => {
                let ticket = self
                    .input
                    .annotation
                    .edit
                    .as_ref()
                    .map(|e| (e.id, e.value.clone()));
                self.operation(Operation::Clipboard {
                    work: self.work.join("helpers"),
                    ticket,
                })?;
            }
            OpenBoard | ImportImages | SaveAs | RelinkSource | ImportKeymap | ExportKeymap => {
                let kind = match action {
                    OpenBoard => Picker::Open,
                    ImportImages => Picker::Import,
                    SaveAs => Picker::Save,
                    RelinkSource => Picker::Relink,
                    ImportKeymap => Picker::ImportKeymap,
                    _ => Picker::ExportKeymap,
                };
                self.operation(Operation::Pick(action, kind, self.work.join("helpers")))?;
            }
            _ => {}
        }
        self.dirty = true;
        Ok(())
    }
    pub(super) fn local_ui_event(&mut self, event: &WindowEvent) -> Result<bool, AssetError> {
        let Some(ui) = &mut self.local.ui else {
            return Ok(false);
        };
        let old_grid = self.local.profile.grid;
        let result = ui.handle(event, &mut self.input.keymap, &mut self.local.profile);
        self.dirty = true;
        if let Some(result) = result {
            match result {
                UiResult::Action(action) => self.local_action(action)?,
                UiResult::PreferencesChanged => {
                    if old_grid != self.local.profile.grid {
                        self.input.grid_visible = self.local.profile.grid;
                    }
                    self.local.profile_pending = true;
                    self.local.profile_changed = true;
                    self.apply_preferences()?;
                }
                UiResult::Dismiss => {
                    if self.load_failed {
                        self.local.close_ready = true;
                        return Ok(true);
                    }
                    self.local.ui = if self.local.recovery_pending {
                        Some(Box::new(LocalUi::new(Panel::Recovery)))
                    } else {
                        None
                    };
                    self.local.close_after_save = false;
                    self.local.close_after_discard = false;
                }
                UiResult::CloseSave => {
                    self.local.ui = None;
                    self.local.close_after_save = true;
                    if self.local.untitled {
                        self.local_action(Action::SaveAs)?;
                    } else {
                        self.local.manual = true;
                    }
                }
                UiResult::CloseDiscard | UiResult::DiscardRecovery => {
                    let lease = self
                        .local
                        .lease
                        .as_ref()
                        .cloned()
                        .ok_or("board ownership unavailable")?;
                    let id = self
                        .editor
                        .as_ref()
                        .ok_or("document unavailable")?
                        .document()
                        .id();
                    self.local.close_after_discard = matches!(result, UiResult::CloseDiscard);
                    self.operation(Operation::DiscardRecovery { lease, id })?;
                }
                UiResult::Restore => {
                    let lease = self
                        .local
                        .lease
                        .as_ref()
                        .cloned()
                        .ok_or("board ownership unavailable")?;
                    let id = self
                        .editor
                        .as_ref()
                        .ok_or("document unavailable")?
                        .document()
                        .id();
                    self.operation(Operation::RestoreRecovery { lease, id })?;
                }
                UiResult::Open(path) => {
                    self.local.ui = None;
                    self.operation(Operation::Launch {
                        path: Some(path),
                        new: false,
                    })?;
                }
            }
        }
        Ok(true)
    }
    fn picked(&mut self, action: Action, paths: Vec<PathBuf>) -> Result<(), AssetError> {
        let path = paths.first().ok_or("picker returned no path")?.clone();
        match action {
            Action::OpenBoard => self.operation(Operation::Launch {
                path: Some(path),
                new: false,
            }),
            Action::ImportImages => self.import_paths(paths, false),
            Action::SaveAs => {
                self.local.save_as = Some(path);
                Ok(())
            }
            Action::RelinkSource => {
                if self.input.images.selection.len() != 1 {
                    return Err("Relink requires one selected image".into());
                }
                let id = self
                    .input
                    .images
                    .selection
                    .ids()
                    .next()
                    .ok_or("selected image")?;
                let doc = self
                    .editor
                    .as_ref()
                    .ok_or("document unavailable")?
                    .document();
                let source = doc
                    .object_render_data(id)
                    .and_then(|d| doc.asset(d.asset_id))
                    .and_then(|a| doc.source(a.source_id()))
                    .ok_or("Relink requires an image source")?
                    .clone();
                self.operation(Operation::Relink(source, path))
            }
            Action::ImportKeymap | Action::ExportKeymap => self.operation(Operation::Keymap {
                action,
                path,
                profile: self.local.profile.clone(),
            }),
            _ => Ok(()),
        }
    }
    fn local_update(&mut self, update: LocalUpdate) -> Result<(), AssetError> {
        match update {
            LocalUpdate::Picked(action, result) => {
                if result.is_err() && action == Action::SaveAs {
                    // Cancellation must keep an unsaved Untitled board open.
                    self.local.close_after_save = false;
                }
                self.picked(action, result.map_err(AssetError::from)?)?
            }
            LocalUpdate::Imported(ImportUpdate::Admitted(image)) => {
                let editor = self.editor.as_mut().ok_or("document unavailable")?;
                tack_app::local_import::admit(editor, &image)?;
                if let Some(original) = image.original {
                    if let tack_storage::Payload::Stored { file, .. } = &original {
                        self.local.spool = Some(Arc::clone(file));
                    }
                    self.local
                        .originals
                        .insert((image.source.id(), image.source.revision()), original);
                }
            }
            LocalUpdate::Imported(ImportUpdate::Progress {
                completed,
                total,
                bytes,
            }) => {
                self.local.import_status = format!(
                    "Import {completed}/{total} · {} KiB copied · Escape cancels",
                    bytes / 1024
                )
            }
            LocalUpdate::Imported(ImportUpdate::Failed { path, message }) => {
                self.interaction_error = Some(format!("Import {}: {message}", path.display()))
            }
            LocalUpdate::Imported(ImportUpdate::Finished {
                cancelled,
                admitted,
                spool,
                ..
            }) => {
                if spool.is_some() {
                    self.local.spool = spool;
                }
                self.local.importing = false;
                self.local.import_status = if cancelled {
                    format!("Import cancelled; {admitted} admitted items retained")
                } else {
                    String::new()
                };
            }
            LocalUpdate::Relinked(result) => {
                result
                    .map_err(AssetError::from)?
                    .apply(self.editor.as_mut().ok_or("document unavailable")?)?;
            }
            LocalUpdate::Restored(result) => {
                let board = Arc::new(result.map_err(AssetError::from)?);
                self.editor = Some(DocumentEditor::recovered(board.document.clone(), 200));
                self.local.cache_generation = None;
                if let Some(assets) = &mut self.assets {
                    assets.set_board(Arc::clone(&board));
                }
                self.board = Some(board);
                self.local.recovery_pending = false;
                self.local.ui = None;
                self.local.recovery.restored(1);
            }
            LocalUpdate::Clipboard { ticket, result } => match result.map_err(AssetError::from)? {
                Clipboard::Image(path) => self.import_paths(vec![path], true)?,
                Clipboard::Text(text) => {
                    if let Some((id, value)) = ticket {
                        let edit = self
                            .input
                            .annotation
                            .edit
                            .as_mut()
                            .filter(|e| e.id == id && e.value == value)
                            .ok_or("note changed before clipboard arrived; paste again")?;
                        let size = if edit.replace { 0 } else { edit.value.len() };
                        if size + text.len() > tack_core::MAX_TEXT_BYTES
                            || text
                                .chars()
                                .any(|c| c.is_control() && c != '\n' && c != '\t')
                        {
                            return Err("clipboard note text is invalid or exceeds 16 KiB".into());
                        }
                        edit.insert(&text);
                    } else {
                        self.import_paths(tack_app::native_files::reference_paths(&text)?, false)?;
                    }
                }
            },
            LocalUpdate::Profile(result) => {
                self.local.profile_base = Some(result.map_err(AssetError::from)?);
            }
            LocalUpdate::Keymap(_, result) => {
                if let Some(profile) = result.map_err(AssetError::from)? {
                    self.local.profile.keymap = profile.keymap;
                    self.local.profile_pending = true;
                    self.local.profile_changed = true;
                    self.apply_preferences()?;
                    self.panel(Panel::Keymap);
                }
            }
            LocalUpdate::Discarded(result) => {
                result.map_err(AssetError::from)?;
                self.local.recovery_pending = false;
                self.local.ui = None;
                if self.local.close_after_discard {
                    self.local.close_ready = true;
                }
            }
            LocalUpdate::Done(result) => {
                result.map_err(AssetError::from)?;
            }
        }
        self.dirty = true;
        Ok(())
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
                self.operation(Operation::Launch {
                    path: paths.first().cloned(),
                    new: false,
                })
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
                    board: Some(self.options.path.clone()),
                };
                if let Err(error) = self.operation(op) {
                    self.local_error(error.to_string());
                }
            }
        }
    }
    pub(super) fn poll_storage(&mut self) {
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
                    self.editor = Some(DocumentEditor::new(board.document.clone(), 200));
                    self.local.cache_generation = None;
                }
                self.local.profile.remember(&self.options.path).ok();
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
