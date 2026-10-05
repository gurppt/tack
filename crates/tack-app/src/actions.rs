use std::{error::Error, fmt};
use tack_core::{CommandError, DocumentEditor};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tool {
    #[default]
    Pointer,
    Pan,
    RotateView,
    Text,
    Rectangle,
    Line,
    Arrow,
    Scribble,
}
impl Tool {
    pub fn is_annotation(self) -> bool {
        matches!(
            self,
            Self::Text | Self::Rectangle | Self::Line | Self::Arrow | Self::Scribble
        )
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Pointer => "Pointer",
            Self::Pan => "Pan",
            Self::RotateView => "Rotate view",
            Self::Text => "Text",
            Self::Rectangle => "Rectangle",
            Self::Line => "Line",
            Self::Arrow => "Arrow",
            Self::Scribble => "Scribble",
        }
    }
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
    ImagePointer,
    ToggleSelection,
    RotateImage,
    ScaleImage,
    AdjustOpacity,
    CenterPointer,
    CancelInteraction,
    CropMode,
    SelectAll,
    DeleteSelection,
    FlipHorizontal,
    FlipVertical,
    Order(crate::selection_commands::Order),
    Opacity(crate::selection_commands::Alpha),
    Filtering(tack_core::ImageFiltering),
    CycleFiltering,
    Save,
    ToggleGrid,
    ToggleSnapping,
    SnapDisable,
    Layout(crate::spatial_layout::Layout),
    GroupSelection,
    UngroupSelection,
    CreateFrame,
    RenameFrame,
    FocusFrame,
    NextFrame,
    PreviousFrame,
    AnnotationStyle(crate::annotation_tool::StyleAction),
    OpenSource,
    RevealSource,
    CopySourcePath,
    ApplicationMenu,
    NewBoard,
    OpenBoard,
    ImportImages,
    SaveAs,
    Paste,
    RelinkSource,
    Preferences,
    KeymapEditor,
    RecentBoards,
    ImportKeymap,
    ExportKeymap,
}
impl Action {
    /// Enumerable action catalog, including currently unassigned actions.
    pub const ALL: [Self; 89] = [
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
        Self::ImagePointer,
        Self::ToggleSelection,
        Self::RotateImage,
        Self::ScaleImage,
        Self::AdjustOpacity,
        Self::CenterPointer,
        Self::CancelInteraction,
        Self::CropMode,
        Self::SelectAll,
        Self::DeleteSelection,
        Self::FlipHorizontal,
        Self::FlipVertical,
        Self::Order(crate::selection_commands::Order::Forward),
        Self::Order(crate::selection_commands::Order::Front),
        Self::Order(crate::selection_commands::Order::Backward),
        Self::Order(crate::selection_commands::Order::Back),
        Self::Opacity(crate::selection_commands::Alpha::Full),
        Self::Opacity(crate::selection_commands::Alpha::ThreeQuarters),
        Self::Opacity(crate::selection_commands::Alpha::Half),
        Self::Opacity(crate::selection_commands::Alpha::Quarter),
        Self::Filtering(tack_core::ImageFiltering::Default),
        Self::Filtering(tack_core::ImageFiltering::Smooth),
        Self::Filtering(tack_core::ImageFiltering::Nearest),
        Self::CycleFiltering,
        Self::Save,
        Self::ToggleGrid,
        Self::ToggleSnapping,
        Self::SnapDisable,
        Self::Layout(crate::spatial_layout::Layout::Left),
        Self::Layout(crate::spatial_layout::Layout::HorizontalCenter),
        Self::Layout(crate::spatial_layout::Layout::Right),
        Self::Layout(crate::spatial_layout::Layout::Top),
        Self::Layout(crate::spatial_layout::Layout::VerticalCenter),
        Self::Layout(crate::spatial_layout::Layout::Bottom),
        Self::Layout(crate::spatial_layout::Layout::DistributeHorizontal),
        Self::Layout(crate::spatial_layout::Layout::DistributeVertical),
        Self::Layout(crate::spatial_layout::Layout::PackHorizontal),
        Self::Layout(crate::spatial_layout::Layout::PackVertical),
        Self::GroupSelection,
        Self::UngroupSelection,
        Self::CreateFrame,
        Self::RenameFrame,
        Self::FocusFrame,
        Self::NextFrame,
        Self::PreviousFrame,
        Self::SelectTool(Tool::Text),
        Self::SelectTool(Tool::Rectangle),
        Self::SelectTool(Tool::Line),
        Self::SelectTool(Tool::Arrow),
        Self::SelectTool(Tool::Scribble),
        Self::TemporaryTool(Tool::Text),
        Self::TemporaryTool(Tool::Rectangle),
        Self::TemporaryTool(Tool::Line),
        Self::TemporaryTool(Tool::Arrow),
        Self::TemporaryTool(Tool::Scribble),
        Self::AnnotationStyle(crate::annotation_tool::StyleAction::Color),
        Self::AnnotationStyle(crate::annotation_tool::StyleAction::Fill),
        Self::AnnotationStyle(crate::annotation_tool::StyleAction::Wider),
        Self::AnnotationStyle(crate::annotation_tool::StyleAction::Narrower),
        Self::AnnotationStyle(crate::annotation_tool::StyleAction::LargerText),
        Self::AnnotationStyle(crate::annotation_tool::StyleAction::SmallerText),
        Self::AnnotationStyle(crate::annotation_tool::StyleAction::OpacityUp),
        Self::AnnotationStyle(crate::annotation_tool::StyleAction::OpacityDown),
        Self::AnnotationStyle(crate::annotation_tool::StyleAction::AlignText),
        Self::OpenSource,
        Self::RevealSource,
        Self::CopySourcePath,
        Self::ApplicationMenu,
        Self::NewBoard,
        Self::OpenBoard,
        Self::ImportImages,
        Self::SaveAs,
        Self::Paste,
        Self::RelinkSource,
        Self::Preferences,
        Self::KeymapEditor,
        Self::RecentBoards,
        Self::ImportKeymap,
        Self::ExportKeymap,
    ];
    /// Stable readable semantic names; never a positional catalog index.
    pub fn id(self) -> String {
        format!("{self:?}")
    }
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|a| a.id() == id)
    }
    pub fn is_local(self) -> bool {
        matches!(
            self,
            Self::ApplicationMenu
                | Self::NewBoard
                | Self::OpenBoard
                | Self::ImportImages
                | Self::SaveAs
                | Self::Paste
                | Self::RelinkSource
                | Self::Preferences
                | Self::KeymapEditor
                | Self::RecentBoards
                | Self::ImportKeymap
                | Self::ExportKeymap
        )
    }
    pub fn category(self) -> &'static str {
        match self {
            Self::SelectTool(_) | Self::TemporaryTool(_) | Self::AnnotationStyle(_) => {
                "Tools and annotations"
            }
            Self::PanView | Self::ZoomView | Self::CenterPointer => "Navigation",
            Self::OpenSource | Self::RevealSource | Self::CopySourcePath => "Sources",
            _ if self.is_local() || self == Self::Save => "Local files and preferences",
            Self::Layout(_)
            | Self::GroupSelection
            | Self::UngroupSelection
            | Self::CreateFrame
            | Self::RenameFrame
            | Self::FocusFrame
            | Self::NextFrame
            | Self::PreviousFrame
            | Self::ToggleGrid
            | Self::ToggleSnapping
            | Self::SnapDisable => "Spatial organization",
            _ => "Selection and editing",
        }
    }
    pub fn captured_hold(self) -> bool {
        matches!(
            self,
            Self::SnapDisable
                | Self::TemporaryTool(_)
                | Self::ImagePointer
                | Self::ToggleSelection
                | Self::RotateImage
                | Self::ScaleImage
                | Self::AdjustOpacity
                | Self::CenterPointer
        )
    }
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
            Self::ImagePointer => "Select and manipulate image",
            Self::ToggleSelection => "Toggle image selection",
            Self::RotateImage => "Rotate image",
            Self::ScaleImage => "Scale image",
            Self::AdjustOpacity => "Adjust image opacity",
            Self::CenterPointer => "Pan or resize handle about center",
            Self::CancelInteraction => "Cancel interaction",
            Self::CropMode => "Crop gizmo",
            Self::SelectAll => "Select all",
            Self::DeleteSelection => "Delete selection",
            Self::FlipHorizontal => "Flip horizontal",
            Self::FlipVertical => "Flip vertical",
            Self::Order(order) => match order {
                crate::selection_commands::Order::Forward => "Bring forward",
                crate::selection_commands::Order::Front => "Bring to front",
                crate::selection_commands::Order::Backward => "Send backward",
                crate::selection_commands::Order::Back => "Send to back",
            },
            Self::Opacity(alpha) => match alpha {
                crate::selection_commands::Alpha::Full => "Opacity 100%",
                crate::selection_commands::Alpha::ThreeQuarters => "Opacity 75%",
                crate::selection_commands::Alpha::Half => "Opacity 50%",
                crate::selection_commands::Alpha::Quarter => "Opacity 25%",
            },
            Self::Filtering(tack_core::ImageFiltering::Default) => "Default sampling",
            Self::Filtering(tack_core::ImageFiltering::Smooth) => "Smooth sampling",
            Self::Filtering(tack_core::ImageFiltering::Nearest) => "Nearest sampling",
            Self::CycleFiltering => "Cycle image sampling",
            Self::Save => "Save",
            Self::ToggleGrid => "Toggle dotted grid",
            Self::ToggleSnapping => "Toggle snapping",
            Self::SnapDisable => "Temporarily disable snapping",
            Self::Layout(layout) => match layout {
                crate::spatial_layout::Layout::Left => "Align left",
                crate::spatial_layout::Layout::HorizontalCenter => "Align horizontal center",
                crate::spatial_layout::Layout::Right => "Align right",
                crate::spatial_layout::Layout::Top => "Align top",
                crate::spatial_layout::Layout::VerticalCenter => "Align vertical center",
                crate::spatial_layout::Layout::Bottom => "Align bottom",
                crate::spatial_layout::Layout::DistributeHorizontal => "Distribute horizontally",
                crate::spatial_layout::Layout::DistributeVertical => "Distribute vertically",
                crate::spatial_layout::Layout::PackHorizontal => "Pack horizontally",
                crate::spatial_layout::Layout::PackVertical => "Pack vertically",
            },
            Self::GroupSelection => "Group selection",
            Self::UngroupSelection => "Ungroup selection",
            Self::CreateFrame => "Create frame",
            Self::RenameFrame => "Rename frame",
            Self::FocusFrame => "Focus selected frame",
            Self::NextFrame => "Focus next frame",
            Self::PreviousFrame => "Focus previous frame",
            Self::SelectTool(tool) => tool.label(),
            Self::TemporaryTool(tool) => tool.label(),
            Self::AnnotationStyle(style) => match style {
                crate::annotation_tool::StyleAction::Color => "Cycle annotation color",
                crate::annotation_tool::StyleAction::Fill => "Toggle annotation fill",
                crate::annotation_tool::StyleAction::Wider => "Increase stroke width",
                crate::annotation_tool::StyleAction::Narrower => "Decrease stroke width",
                crate::annotation_tool::StyleAction::LargerText => "Increase note size",
                crate::annotation_tool::StyleAction::SmallerText => "Decrease note size",
                crate::annotation_tool::StyleAction::OpacityUp => "Increase annotation opacity",
                crate::annotation_tool::StyleAction::OpacityDown => "Decrease annotation opacity",
                crate::annotation_tool::StyleAction::AlignText => "Cycle note alignment",
            },
            Self::OpenSource => "Open linked image source",
            Self::RevealSource => "Reveal linked image source",
            Self::CopySourcePath => "Copy linked source path",
            Self::ApplicationMenu => "Local menu",
            Self::NewBoard => "New board",
            Self::OpenBoard => "Open board",
            Self::ImportImages => "Import images",
            Self::SaveAs => "Save As",
            Self::Paste => "Paste",
            Self::RelinkSource => "Relink selected source",
            Self::Preferences => "Preferences",
            Self::KeymapEditor => "Edit keymap",
            Self::RecentBoards => "Recent boards",
            Self::ImportKeymap => "Import keymap",
            Self::ExportKeymap => "Export keymap",
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
    Cancel(HoldToken),
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
                phase: ActionPhase::End(token) | ActionPhase::Cancel(token),
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
