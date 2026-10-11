//! One event-driven local admission pass, never a watcher or recurring poll.
use crate::source_transition;
use std::{
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
};
use tack_assets::{AssetError, source_fingerprint};
use tack_core::*;
pub struct Request {
    pub document: DocumentId,
    pub sources: Vec<Source>,
    pub board: PathBuf,
    pub work: PathBuf,
}
pub struct Ready {
    document: DocumentId,
    changes: Vec<source_transition::Ready>,
}
impl Request {
    pub fn run(self, cancel: &AtomicBool) -> Ready {
        let mut changes = Vec::new();
        for source in self.sources {
            if cancel.load(Ordering::Relaxed) {
                break;
            }
            let SourceLocation::Linked(link) = source.location() else {
                continue;
            };
            let Some(path) = link.to_native() else {
                continue;
            };
            let path = if path.is_absolute() {
                path
            } else {
                self.board
                    .parent()
                    .unwrap_or(std::path::Path::new("."))
                    .join(path)
            };
            let Ok(actual) = source_fingerprint(&path) else {
                continue;
            };
            if source.fingerprint() == Some(actual) {
                continue;
            }
            // No picker or encoded byte copy for Linked. A file changed again,
            // missing or invalid during admission retains its last-known view.
            let request = source_transition::Request {
                document: self.document,
                previous: source,
                board: self.board.clone(),
                work: self.work.clone(),
                spool: None,
                toggle: false,
            };
            if let Ok(Some(ready)) = request.run(cancel) {
                changes.push(ready);
            }
        }
        Ready {
            document: self.document,
            changes,
        }
    }
}
impl Ready {
    pub fn apply(self, editor: &mut DocumentEditor) -> Result<usize, AssetError> {
        if self.document != editor.document().id() {
            return Ok(0);
        }
        let mut edits = Vec::new();
        let mut count = 0usize;
        for ready in self.changes {
            if editor.document().source(ready.previous.id()) != Some(&ready.previous) {
                continue; // A toggle/Undo/relink since capture remains authoritative.
            }
            let revision = editor
                .next_source_revision()
                .and_then(|r| r.checked_add(count as u64))
                .ok_or("Source revision exhausted")?;
            edits.extend(ready.edits(revision, editor.document())?);
            count += 1;
        }
        if count > 0 {
            editor.execute(Command::Batch(edits))?;
        }
        Ok(count)
    }
}
