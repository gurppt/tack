use std::{error::Error, fmt};
use tack_core::{CommandError, DocumentEditor};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tool {
    #[default]
    Pointer,
    Pan,
    RotateView,
}

/// Canonical semantic commands for this slice. Input and future menus use the
/// same values; no keyboard codes appear in action/tool behavior.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    SelectTool(Tool),
    TemporaryTool(Tool),
    Undo,
    Redo,
    PanView,
    ZoomView,
}
impl Action {
    /// Enumerable action catalog, including currently unassigned actions.
    pub const ALL: [Self; 10] = [
        Self::SelectTool(Tool::Pointer),
        Self::SelectTool(Tool::Pan),
        Self::SelectTool(Tool::RotateView),
        Self::TemporaryTool(Tool::Pointer),
        Self::TemporaryTool(Tool::Pan),
        Self::TemporaryTool(Tool::RotateView),
        Self::Undo,
        Self::Redo,
        Self::PanView,
        Self::ZoomView,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::SelectTool(Tool::Pointer) => "Pointer tool",
            Self::SelectTool(Tool::Pan) => "Pan tool",
            Self::SelectTool(Tool::RotateView) => "Rotate view tool",
            Self::TemporaryTool(Tool::Pointer) => "Temporary pointer tool",
            Self::TemporaryTool(Tool::Pan) => "Temporary pan tool",
            Self::TemporaryTool(Tool::RotateView) => "Temporary rotate view tool",
            Self::Undo => "Undo",
            Self::Redo => "Redo",
            Self::PanView => "Pan view",
            Self::ZoomView => "Zoom view",
        }
    }
}

/// Opaque gesture identity assigned by input, not a physical key or document ID.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HoldToken(pub(crate) u64);

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ActionPhase {
    Invoke,
    Begin(HoldToken),
    End(HoldToken),
    Delta(f64),
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ActionEvent {
    pub action: Action,
    pub phase: ActionPhase,
}

/// Transient tool state, separate from camera and document. Last held temporary
/// tool wins; releasing it reveals the remaining hold or selected base tool.
/// Selecting a base tool during a hold changes what restoration returns to.
#[derive(Default)]
pub struct Interaction {
    base: Tool,
    temporary: Vec<(HoldToken, Tool)>,
}
impl Interaction {
    pub fn tool(&self) -> Tool {
        self.temporary.last().map(|(_, t)| *t).unwrap_or(self.base)
    }
    pub fn apply(&mut self, event: ActionEvent) -> Result<(), ActionError> {
        match event {
            ActionEvent {
                action: Action::SelectTool(tool),
                phase: ActionPhase::Invoke,
            } => self.base = tool,
            ActionEvent {
                action: Action::TemporaryTool(tool),
                phase: ActionPhase::Begin(token),
            } => {
                if self.temporary.iter().any(|(held, _)| *held == token) {
                    return Ok(());
                }
                if self.temporary.len() >= crate::input::MAX_HELD_INPUTS {
                    return Err(ActionError::TooManyHolds);
                }
                self.temporary.push((token, tool));
            }
            ActionEvent {
                action: Action::TemporaryTool(_),
                phase: ActionPhase::End(token),
            } => self.temporary.retain(|(held, _)| *held != token),
            _ => return Err(ActionError::WrongTargetOrPhase),
        }
        Ok(())
    }
}

/// Menu and binding dispatch share this document action boundary.
pub fn dispatch_document_action(
    event: ActionEvent,
    editor: &mut DocumentEditor,
) -> Result<bool, ActionError> {
    match event {
        ActionEvent {
            action: Action::Undo,
            phase: ActionPhase::Invoke,
        } => Ok(editor.undo()?),
        ActionEvent {
            action: Action::Redo,
            phase: ActionPhase::Invoke,
        } => Ok(editor.redo()?),
        _ => Err(ActionError::WrongTargetOrPhase),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionError {
    TooManyHolds,
    WrongTargetOrPhase,
    Document(CommandError),
}
impl From<CommandError> for ActionError {
    fn from(error: CommandError) -> Self {
        Self::Document(error)
    }
}
impl fmt::Display for ActionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "action rejected: {self:?}")
    }
}
impl Error for ActionError {}
