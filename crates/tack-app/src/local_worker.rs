//! One explicit local operation with one bounded result slot; no idle thread/timer.
pub use crate::clipboard::Clipboard;
use crate::{
    actions::Action,
    local_import::{ImportRequest, ImportUpdate},
    local_relink::RelinkReady,
    native_files::{self, Picker},
    preferences::{self, Preferences},
};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
    },
    thread::JoinHandle,
};
use tack_assets::AssetError;
use tack_core::{ObjectId, Source};
pub enum LocalUpdate {
    About {
        ticket: u64,
        result: Result<tack_assets::Decoded, String>,
    },
    Picked(Action, Result<Vec<PathBuf>, String>),
    Imported(ImportUpdate),
    Relinked(Result<RelinkReady, String>),
    Restored(Result<tack_storage::TackFile, String>),
    Discarded(Result<(), String>),
    Clipboard {
        ticket: Option<(ObjectId, String)>,
        result: Result<Clipboard, String>,
        received: mpsc::SyncSender<()>,
    },
    Profile(Result<u32, String>),
    Keymap(Action, Result<Option<Preferences>, String>),
    Done(Result<(), String>),
}
pub enum Operation {
    About {
        ticket: u64,
    },
    Pick(Action, Picker, PathBuf),
    Import(ImportRequest),
    Relink(Source, PathBuf),
    Clipboard {
        work: PathBuf,
        ticket: Option<(ObjectId, String)>,
    },
    Profile {
        root: PathBuf,
        profile: Preferences,
        base: Option<u32>,
        changed: bool,
        board: Option<PathBuf>,
    },
    Keymap {
        action: Action,
        path: PathBuf,
        profile: Preferences,
    },
    Launch {
        path: Option<PathBuf>,
        new: bool,
    },
    RestoreRecovery {
        lease: Arc<tack_storage::BoardLease>,
        id: tack_core::DocumentId,
    },
    RetireSeed {
        lease: Arc<tack_storage::BoardLease>,
        id: tack_core::DocumentId,
    },
    DiscardRecovery {
        lease: Arc<tack_storage::BoardLease>,
        id: tack_core::DocumentId,
    },
}
struct Active {
    cancel: Arc<AtomicBool>,
    receiver: Receiver<LocalUpdate>,
    handle: JoinHandle<()>,
    importing: bool,
    mutating: bool,
}
#[derive(Default)]
pub struct LocalWorker {
    active: Option<Active>,
    pub operations: u64,
}
impl LocalWorker {
    pub fn active(&self) -> bool {
        self.active.is_some()
    }
    pub fn importing(&self) -> bool {
        self.active.as_ref().is_some_and(|a| a.importing)
    }
    pub fn mutating(&self) -> bool {
        self.active.as_ref().is_some_and(|a| a.mutating)
    }
    pub fn cancel(&self) {
        if let Some(a) = &self.active {
            a.cancel.store(true, Ordering::Relaxed);
        }
    }
    pub fn start(
        &mut self,
        operation: Operation,
        notify: impl Fn() + Send + 'static,
    ) -> Result<(), AssetError> {
        if self.active() {
            return Err("another local operation is active; Escape cancels it".into());
        }
        let importing = matches!(operation, Operation::Import(_));
        let mutating = matches!(
            operation,
            Operation::Import(_) | Operation::Relink(..) | Operation::RestoreRecovery { .. }
        );
        let (tx, receiver) = mpsc::sync_channel(1);
        let cancel = Arc::new(AtomicBool::new(false));
        let stop = Arc::clone(&cancel);
        let handle = std::thread::Builder::new()
            .name("tack-local-operation".into())
            .spawn(move || {
                let emit = |update| {
                    let sent = tx.send(update).is_ok();
                    if sent {
                        notify();
                    }
                    sent
                };
                let result = (|| -> Result<(), AssetError> {
                    match operation {
                        Operation::About { ticket } => {
                            if !stop.load(Ordering::Relaxed) {
                                let result = crate::about::load_image().map_err(|e| e.to_string());
                                if !stop.load(Ordering::Relaxed) {
                                    emit(LocalUpdate::About { ticket, result });
                                }
                            }
                        }
                        Operation::Pick(action, picker, work) => {
                            emit(LocalUpdate::Picked(
                                action,
                                native_files::pick(picker, &work, &stop).map_err(|e| e.to_string()),
                            ));
                        }
                        Operation::Import(request) => {
                            let temporary = request.temporary.clone();
                            let result = request.run(&stop, |u| emit(LocalUpdate::Imported(u)));
                            if let Some(path) = temporary {
                                let _ = std::fs::remove_file(path);
                            }
                            result?;
                        }
                        Operation::Relink(source, path) => {
                            emit(LocalUpdate::Relinked(
                                RelinkReady::read(source, &path).map_err(|e| e.to_string()),
                            ));
                        }
                        Operation::Clipboard { work, ticket } => {
                            let result = crate::clipboard::read(&work, &stop, ticket.is_some());
                            let staged = match &result {
                                Ok(Clipboard::Image(path)) => Some(path.clone()),
                                _ => None,
                            };
                            let (received, ack) = mpsc::sync_channel(1);
                            let published = emit(LocalUpdate::Clipboard {
                                ticket,
                                result: result.map_err(|e| e.to_string()),
                                received,
                            });
                            while published && !stop.load(Ordering::Relaxed) {
                                match ack.recv_timeout(std::time::Duration::from_millis(20)) {
                                    Err(mpsc::RecvTimeoutError::Timeout) => continue,
                                    _ => break,
                                }
                            }
                            if (!published || stop.load(Ordering::Relaxed))
                                && let Some(path) = staged
                            {
                                let _ = std::fs::remove_file(path);
                            }
                        }
                        Operation::Profile {
                            root,
                            profile,
                            base,
                            changed,
                            board,
                        } => {
                            emit(LocalUpdate::Profile(
                                preferences::save_profile(
                                    &root,
                                    &profile,
                                    base,
                                    changed,
                                    board.as_deref(),
                                )
                                .map_err(|e| e.to_string()),
                            ));
                        }
                        Operation::Keymap {
                            action,
                            path,
                            profile,
                        } => {
                            let result = if action == Action::ImportKeymap {
                                preferences::read(&path).map(Some)
                            } else {
                                preferences::write(&path, &profile).map(|_| None)
                            };
                            emit(LocalUpdate::Keymap(
                                action,
                                result.map_err(|e| e.to_string()),
                            ));
                        }
                        Operation::Launch { path, new } => {
                            let mut command = std::process::Command::new(std::env::current_exe()?);
                            if let Some(path) = path {
                                command.arg(if new { "new" } else { "open" }).arg(path);
                            }
                            command.stdin(std::process::Stdio::null()).spawn()?;
                        }
                        Operation::RestoreRecovery { lease, id } => {
                            let result = (|| -> Result<tack_storage::TackFile, AssetError> {
                                let (board, _) =
                                    lease.recovery(id)?.ok_or("recovery is no longer current")?;
                                board.verify_originals()?;
                                Ok(board)
                            })();
                            emit(LocalUpdate::Restored(result.map_err(|e| e.to_string())));
                        }
                        Operation::RetireSeed { lease, id } => {
                            lease.retire_empty_seed(id)?;
                        }
                        Operation::DiscardRecovery { lease, id } => {
                            emit(LocalUpdate::Discarded(
                                lease.discard_recovery(id).map_err(|e| e.to_string()),
                            ));
                        }
                    }
                    Ok(())
                })();
                emit(LocalUpdate::Done(result.map_err(|e| e.to_string())));
            })?;
        self.operations += 1;
        self.active = Some(Active {
            cancel,
            receiver,
            handle,
            importing,
            mutating,
        });
        Ok(())
    }
    pub fn poll(&mut self) -> Vec<LocalUpdate> {
        let mut updates = Vec::new();
        if let Some(active) = &self.active {
            for _ in 0..4 {
                match active.receiver.try_recv() {
                    Ok(update) => {
                        if let LocalUpdate::Clipboard { received, .. } = &update {
                            let _ = received.try_send(());
                            if active.cancel.load(Ordering::Relaxed) {
                                continue;
                            }
                        }
                        let done = matches!(update, LocalUpdate::Done(_));
                        updates.push(update);
                        if done {
                            break;
                        }
                    }
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        updates.push(LocalUpdate::Done(Err(
                            "local worker stopped before completion".into(),
                        )));
                        break;
                    }
                }
            }
        }
        if updates.iter().any(|u| matches!(u, LocalUpdate::Done(_))) {
            self.active.take();
        }
        updates
    }
}
impl Drop for LocalWorker {
    fn drop(&mut self) {
        if let Some(active) = self.active.take() {
            active.cancel.store(true, Ordering::Relaxed);
            drop(active.receiver);
            let _ = active.handle.join();
        }
    }
}
