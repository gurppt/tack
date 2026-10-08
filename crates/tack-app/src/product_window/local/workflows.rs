//! Local workflows on the existing worker and editor authority.
use super::*;
impl App {
    pub(in super::super) fn apply_preferences(&mut self) -> Result<(), AssetError> {
        self.input.keymap = self.local.profile.keymap()?;
        let palette = self.local.profile.theme.palette();
        self.input.gizmo.palette = palette;
        self.input.gizmo.style.selection = palette.accent_primary;
        self.input.gizmo.style.active = palette.accent_secondary;
        self.input.gizmo.style.crop = palette.accent_attention;
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
    pub(in super::super) fn import_paths(
        &mut self,
        paths: Vec<PathBuf>,
        force_embedded: bool,
    ) -> Result<(), AssetError> {
        if self.local.importing {
            return Err(
                "An image import is active; wait or cancel it before importing again".into(),
            );
        }
        let temporary = (force_embedded
            && paths.len() == 1
            && paths[0].parent() == Some(self.work.join("helpers").as_path()))
        .then(|| paths[0].clone());
        let request = ImportRequest {
            temporary,
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
        self.local.import_rejected = 0;
        self.local.importing = true;
        self.local.import_status = "Import pending · Escape cancels remaining files".into();
        Ok(())
    }
    pub(in super::super) fn local_action(&mut self, action: Action) -> Result<(), AssetError> {
        if self.shared.is_some() && matches!(action, Action::Save | Action::SaveAs) {
            return Err("Shared edits are saved by the server. Local snapshot export is reserved for the documented streaming API.".into());
        }
        if self.load_failed {
            return Ok(());
        }
        use Action::*;
        if action != Paste
            && let Some(editor) = &mut self.editor
        {
            self.input.commit_drafts(editor)?;
        }
        let keymap_panel = matches!(action, ImportKeymap | ExportKeymap)
            .then(|| self.local.ui.take())
            .flatten();
        self.local.ui = None;
        self.release_about();
        match action {
            ApplicationMenu => self.open_application_menu()?,
            About => {
                self.local.about_ticket = self
                    .local
                    .about_ticket
                    .checked_add(1)
                    .ok_or("About request counter exhausted")?;
                self.panel(Panel::About);
                self.operation(Operation::About {
                    ticket: self.local.about_ticket,
                })?;
            }
            Preferences => self.panel(Panel::Preferences),
            KeymapEditor => self.panel(Panel::Keymap),
            RecentBoards => self.panel(Panel::Recent),
            NewBoard => self.operation(Operation::Launch {
                path: None,
                new: true,
            })?,
            SaveOriginalAs => {
                if self.shared.is_some() {
                    return Err("Shared Save Original As is not available yet; use the documented local snapshot API".into());
                }
                if self.input.images.selection.len() != 1 {
                    return Err("Save Original As requires one selected image".into());
                }
                let id = self
                    .input
                    .images
                    .selection
                    .ids()
                    .next()
                    .ok_or("selected image")?;
                let request = tack_app::source_export::OriginalExport::new(
                    self.editor
                        .as_ref()
                        .ok_or("document unavailable")?
                        .document(),
                    id,
                    &self.options.path,
                    self.board.as_ref().ok_or("board unavailable")?,
                    &self.local.originals,
                )?;
                self.operation(Operation::ExportOriginal {
                    request,
                    work: self.work.join("helpers"),
                })?;
            }
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
                if matches!(action, ImportKeymap | ExportKeymap) {
                    if let Some(ui) = keymap_panel {
                        self.local.ui = Some(ui);
                    } else {
                        self.panel(Panel::Keymap);
                    }
                }
                let options = tack_app::native_files::PickOptions {
                    directory: matches!(action, OpenBoard | SaveAs)
                        .then(|| {
                            self.local
                                .profile
                                .last_board_directory
                                .as_ref()
                                .and_then(|p| p.path().ok())
                        })
                        .flatten(),
                    suggested_name: if action == SaveAs && !self.local.untitled {
                        self.options
                            .path
                            .file_name()
                            .map(|name| name.to_os_string())
                    } else {
                        None
                    },
                };
                self.operation(Operation::Pick(
                    action,
                    kind,
                    self.work.join("helpers"),
                    options,
                ))?;
            }
            _ => {}
        }
        self.dirty = true;
        Ok(())
    }
    pub(in super::super) fn local_ui_event(
        &mut self,
        event: &WindowEvent,
    ) -> Result<bool, AssetError> {
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
                    self.release_about();
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
                    self.open_board(path)?;
                }
            }
        }
        Ok(true)
    }
    pub(in super::super) fn can_reuse_fresh(&self, opening: bool) -> bool {
        fresh::FreshOpenState {
            launched_untitled: self.options.new && self.options.untitled && self.local.untitled,
            seed_id: self.local.seed.as_ref().map(|(_, id)| *id),
            recovery: self.local.recovery_pending || self.local.recovery.deadline().is_some(),
            pending: self.load_failed
                || self.save.active()
                || self.local.saving_as
                || self.local.manual
                || self.local.save_as.is_some()
                || self.local.importing
                || (!opening && self.local.worker.mutating())
                || self.local.queued.is_some()
                || !self.local.drops.is_empty()
                || self.local.drop_deadline.is_some()
                || self.source_active
                || self.input.active()
                || self.input.name_edit.is_some()
                || self.input.annotation.edit.is_some()
                || !self.local.originals.is_empty()
                || self.local.spool.is_some()
                || self.local.close_after_save
                || self.local.close_after_discard
                || self.local.close_ready,
        }
        .can_replace(self.editor.as_ref())
    }
    pub(in super::super) fn open_board(&mut self, path: PathBuf) -> Result<(), AssetError> {
        if self.can_reuse_fresh(false) {
            self.operation(Operation::OpenBoard(path))
        } else {
            self.operation(Operation::Launch {
                path: Some(path),
                new: false,
            })
        }
    }
    pub(in super::super) fn picked(
        &mut self,
        action: Action,
        paths: Vec<PathBuf>,
    ) -> Result<(), AssetError> {
        let path = paths.first().ok_or("picker returned no path")?.clone();
        match action {
            Action::OpenBoard => self.open_board(path),
            Action::ImportImages => self.import_paths(paths, false),
            Action::SaveAs => {
                self.local.save_as = Some(tack_app::file_names::board(path));
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
}
