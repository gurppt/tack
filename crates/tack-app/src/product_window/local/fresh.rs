//! Small document-replacement policy, independent of a native window/display.
use tack_core::{DocumentEditor, DocumentId};
pub(super) struct FreshOpenState {
    pub launched_untitled: bool,
    pub seed_id: Option<DocumentId>,
    pub recovery: bool,
    pub pending: bool,
}
impl FreshOpenState {
    pub fn can_replace(&self, editor: Option<&DocumentEditor>) -> bool {
        self.launched_untitled
            && !self.recovery
            && !self.pending
            && editor.is_some_and(|editor| {
                self.seed_id == Some(editor.document().id())
                    && editor.generation() == 0
                    && !editor.is_dirty()
                    && editor.undo_len() == 0
                    && editor.redo_len() == 0
                    && editor.document().objects().next().is_none()
                    && editor.document().assets().next().is_none()
                    && editor.document().sources().next().is_none()
            })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use tack_core::*;
    type R = Result<(), Box<dyn std::error::Error + Send + Sync>>;
    #[test]
    fn initial_owned_clean_empty_slot_is_reused_and_other_work_is_retained() -> R {
        let id = DocumentId::new(1)?;
        let mut editor = DocumentEditor::new(Document::new(id, DocumentLimits::default()), 200);
        let mut state = FreshOpenState {
            launched_untitled: true,
            seed_id: Some(id),
            recovery: false,
            pending: false,
        };
        assert!(state.can_replace(Some(&editor)));
        assert!(!state.can_replace(None));
        state.pending = true;
        assert!(!state.can_replace(Some(&editor)));
        state.pending = false;
        state.recovery = true;
        assert!(!state.can_replace(Some(&editor)));
        state.recovery = false;
        state.launched_untitled = false;
        assert!(!state.can_replace(Some(&editor)));
        state.launched_untitled = true;
        state.seed_id = Some(DocumentId::new(2)?);
        assert!(!state.can_replace(Some(&editor)));
        state.seed_id = Some(id);
        editor.execute(Command::AddObject {
            object: DocumentObject::frame(
                ObjectId::new(1)?,
                "work".into(),
                Transform::new([0.; 2], [10.; 2], 0., [false; 2])?,
            )?,
            index: 0,
        })?;
        assert!(!state.can_replace(Some(&editor)));
        editor.undo()?;
        assert!(!state.can_replace(Some(&editor)));
        editor.mark_saved();
        assert!(!state.can_replace(Some(&editor)));
        Ok(())
    }
    #[test]
    fn clean_preexisting_source_and_recovered_empty_board_are_owned_work() -> R {
        let id = DocumentId::new(1)?;
        let state = FreshOpenState {
            launched_untitled: true,
            seed_id: Some(id),
            recovery: false,
            pending: false,
        };
        let mut doc = Document::new(id, DocumentLimits::default());
        doc.apply(Command::AddSource(Source::embedded(SourceId::new(1)?)))?;
        let editor = DocumentEditor::new(doc, 200);
        assert!(!state.can_replace(Some(&editor)));
        let recovered =
            DocumentEditor::recovered(Document::new(id, DocumentLimits::default()), 200);
        assert!(!state.can_replace(Some(&recovered)));
        Ok(())
    }
}
