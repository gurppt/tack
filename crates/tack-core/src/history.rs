use crate::{Command, CommandError, Document};
use std::collections::VecDeque;

/// Exclusive document/history ownership. Retained inverses include bounded
/// annotation strings/points and source descriptors, never decoded image bytes.
/// Oldest undo entries are dropped at capacity; undo+redo share that capacity.
pub struct DocumentEditor {
    document: Document,
    capacity: usize,
    undo: VecDeque<Command>,
    redo: Vec<Command>,
    dirty: bool,
    generation: u64,
    source_revision_high_water: u64,
}
impl DocumentEditor {
    /// Zero capacity disables recording. Capacity is chosen explicitly by owner.
    pub fn new(document: Document, capacity: usize) -> Self {
        let source_revision_high_water =
            document.sources().map(|s| s.revision()).max().unwrap_or(0);
        Self {
            document,
            capacity,
            undo: VecDeque::new(),
            redo: Vec::new(),
            dirty: false,
            generation: 0,
            source_revision_high_water,
        }
    }
    /// Startup restore is dirty authority, not an ordinary command or a saved generation.
    pub fn recovered(document: Document, capacity: usize) -> Self {
        let mut editor = Self::new(document, capacity);
        editor.dirty = true;
        editor.generation = 1;
        editor
    }
    pub fn document(&self) -> &Document {
        &self.document
    }
    /// Original handles are needed by live authority or an embedded source redo/inverse.
    pub fn required_originals(&self) -> Vec<(crate::SourceId, u64)> {
        let mut result: Vec<_> = self
            .document
            .sources()
            .filter(|s| matches!(s.location(), crate::SourceLocation::Embedded))
            .map(|s| (s.id(), s.revision()))
            .collect();
        for command in self.undo.iter().chain(&self.redo) {
            let edits = match command {
                Command::Batch(edits) => edits.as_slice(),
                other => std::slice::from_ref(other),
            };
            for edit in edits {
                if let Command::AddSource(source) | Command::SetSource(source) = edit
                    && matches!(source.location(), crate::SourceLocation::Embedded)
                {
                    result.push((source.id(), source.revision()));
                }
            }
        }
        result
    }
    pub fn undo_len(&self) -> usize {
        self.undo.len()
    }
    pub fn redo_len(&self) -> usize {
        self.redo.len()
    }
    /// Conservative: undoing back to a saved state may remain dirty, never falsely clean.
    /// Revisions issued in this editing session are never recycled by undo/divergence.
    pub fn next_source_revision(&self) -> Option<u64> {
        self.source_revision_high_water.checked_add(1)
    }
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }
    /// Call only after the exact current document has been saved successfully.
    pub fn mark_saved(&mut self) {
        self.dirty = false;
    }
    pub fn generation(&self) -> u64 {
        self.generation
    }
    /// A save snapshot acknowledges only its own exact edit generation.
    pub fn mark_saved_generation(&mut self, generation: u64) -> bool {
        if self.generation == generation {
            self.mark_saved();
            true
        } else {
            false
        }
    }
    pub fn history_bytes(&self) -> usize {
        self.undo
            .iter()
            .chain(&self.redo)
            .map(Command::retained_bytes)
            .sum()
    }
    fn trim_history(&mut self) {
        while self.undo.len() + self.redo.len() > self.capacity
            || self.history_bytes() > 32 * 1024 * 1024
        {
            if self.undo.pop_front().is_none() {
                if self.redo.is_empty() {
                    break;
                }
                self.redo.remove(0);
            }
        }
    }
    pub fn capacity(&self) -> usize {
        self.capacity
    }
    pub fn execute(&mut self, command: Command) -> Result<bool, CommandError> {
        let generation = self
            .generation
            .checked_add(1)
            .ok_or(CommandError::LimitReached("edit generation"))?;
        if command.retained_bytes() > 32 * 1024 * 1024 {
            return Err(CommandError::LimitReached("history bytes"));
        }
        self.document
            .validate_source_revision(&command, self.source_revision_high_water)?;
        let next_high_water = self
            .source_revision_high_water
            .max(command.source_revision());
        let Some(inverse) = self.document.apply_reversible(command)? else {
            return Ok(false);
        };
        if self.capacity > 0 && inverse.retained_bytes() > 32 * 1024 * 1024 {
            self.document.apply_reversible(inverse)?;
            return Err(CommandError::LimitReached("history inverse bytes"));
        }
        self.generation = generation;
        self.source_revision_high_water = next_high_water;
        self.dirty = true;
        self.redo.clear();
        if self.capacity > 0 {
            if self.undo.len() == self.capacity {
                self.undo.pop_front();
            }
            self.undo.push_back(inverse);
            self.trim_history();
        }
        Ok(true)
    }
    pub fn undo(&mut self) -> Result<bool, CommandError> {
        let Some(command) = self.undo.back().cloned() else {
            return Ok(false);
        };
        let generation = self
            .generation
            .checked_add(1)
            .ok_or(CommandError::LimitReached("edit generation"))?;
        let inverse = self.document.apply_reversible(command)?;
        self.generation = generation;
        self.dirty = true;
        self.undo.pop_back();
        if let Some(inverse) = inverse {
            self.redo.push(inverse);
        }
        self.trim_history();
        Ok(true)
    }
    pub fn redo(&mut self) -> Result<bool, CommandError> {
        let Some(command) = self.redo.last().cloned() else {
            return Ok(false);
        };
        let generation = self
            .generation
            .checked_add(1)
            .ok_or(CommandError::LimitReached("edit generation"))?;
        let inverse = self.document.apply_reversible(command)?;
        self.generation = generation;
        self.dirty = true;
        self.redo.pop();
        if let Some(inverse) = inverse {
            self.undo.push_back(inverse);
        }
        self.trim_history();
        Ok(true)
    }
    /// Discard history explicitly without changing document state.
    pub fn clear_history(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }
    /// Consume the editor, discarding its local history.
    pub fn into_document(self) -> Document {
        self.document
    }
}
