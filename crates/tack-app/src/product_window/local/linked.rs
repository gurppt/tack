//! Revalidate links only on open/focus; use the finite local worker.
use super::*;
impl App {
    pub(super) fn probe_linked_sources(&mut self) {
        if !self.local.linked_probe_pending
            || self.local.worker.active()
            || self.local.queued.is_some()
            || self.save.active()
            || self.input.active()
            || self.input.name_edit.is_some()
            || self.local.recovery_pending
            || self.local.ui.is_some()
            || self.local.close_after_save
            || self.local.close_board
        {
            return;
        }
        self.local.linked_probe_pending = false;
        if self.shared.is_some() || self.offline.is_some() {
            return;
        }
        let Some(editor) = &self.editor else {
            self.local.linked_probe_pending = true;
            return;
        };
        let sources: Vec<_> = editor
            .document()
            .sources()
            .filter(|s| matches!(s.location(), tack_core::SourceLocation::Linked(_)))
            .cloned()
            .collect();
        if sources.is_empty() {
            return;
        }
        let request = tack_app::linked_refresh::Request {
            document: editor.document().id(),
            sources,
            board: self.options.path.clone(),
            work: self.work.join("linked-refresh"),
        };
        if let Err(error) = self.operation(Operation::LinkedRefresh(request)) {
            self.input.status = error.to_string();
        }
    }
}
