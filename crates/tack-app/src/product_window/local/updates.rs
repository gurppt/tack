//! Local updates on the existing worker and editor authority.
use super::*;
impl App {
    pub(in super::super) fn local_update(&mut self, update: LocalUpdate) -> Result<(), AssetError> {
        match update {
            LocalUpdate::Opened(result) => {
                let opened = result.map_err(AssetError::from)?;
                if self.can_reuse_fresh(true) {
                    self.install_loaded(
                        LoadedBoard {
                            board: opened.board,
                            lease: opened.lease,
                            path: opened.path,
                            metadata_ms: opened.metadata_ms,
                            recovery: opened.recovery,
                            warning: opened.warning,
                        },
                        true,
                    )?;
                } else {
                    let path = opened.path;
                    drop(opened.lease);
                    self.operation(Operation::Launch {
                        path: Some(path),
                        new: false,
                    })?;
                }
            }
            LocalUpdate::About { ticket, result } => {
                if ticket == self.local.about_ticket
                    && let Some(ui) = self.local.ui.as_mut().filter(|ui| ui.panel == Panel::About)
                {
                    match result.and_then(|image| {
                        self.gpu
                            .as_mut()
                            .ok_or("About renderer unavailable")?
                            .set_ui_image(&image)
                            .map_err(|e| e.to_string())
                    }) {
                        Ok(()) => {
                            ui.about_image = true;
                            ui.message.clear();
                        }
                        Err(error) => {
                            ui.about_image = false;
                            ui.message = error;
                        }
                    }
                }
            }
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
                let _ = (path, message);
                self.local.import_rejected += 1;
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
                    format!(
                        "Import cancelled; {admitted} admitted items retained, {} rejected",
                        self.local.import_rejected
                    )
                } else if self.local.import_rejected > 0 {
                    format!(
                        "Import: {admitted} admitted, {} rejected (unsupported, remote or unreadable files)",
                        self.local.import_rejected
                    )
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
                self.visibility.invalidate();
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
            LocalUpdate::Clipboard { ticket, result, .. } => match result
                .map_err(AssetError::from)?
            {
                Clipboard::Image(path) => self.import_paths(vec![path], true)?,
                Clipboard::Files { paths, rejected } => {
                    self.import_paths(paths, false)?;
                    self.local.import_rejected = rejected;
                }
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
                        self.input.paste_text_note(
                            text,
                            self.editor.as_mut().ok_or("document unavailable")?,
                            &self.camera,
                        )?;
                    }
                }
            },
            LocalUpdate::Profile(result) => {
                self.local.profile_base = Some(result.map_err(AssetError::from)?);
            }
            LocalUpdate::Keymap(action, result) => {
                if let Some(profile) = result.map_err(AssetError::from)? {
                    self.local.profile.keymap = profile.keymap;
                    self.local.profile_pending = true;
                    self.local.profile_changed = true;
                    self.apply_preferences()?;
                    if self.local.ui.is_none() {
                        self.panel(Panel::Keymap);
                    }
                }
                if let Some(ui) = &mut self.local.ui {
                    ui.message = if action == Action::ImportKeymap {
                        "Keymap imported"
                    } else {
                        "Keymap exported"
                    }
                    .into();
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
}
