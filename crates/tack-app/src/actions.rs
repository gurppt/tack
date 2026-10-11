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
    Eraser,
    Mouse,
}
impl Tool {
    pub fn is_one_shot(self) -> bool {
        matches!(
            self,
            Self::Text | Self::Rectangle | Self::Line | Self::Arrow
        )
    }
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
            Self::Eraser => "Scribble eraser",
            Self::Mouse => "Mulot",
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
    ResetCrop,
    ActualSize,
    ToggleGrayscale,
    ToggleLockSelection,
    SelectLockedObjects,
    RefreshSource,
    ToggleSourceStorage,
    ContextCoarseIncrease,
    ContextCoarseDecrease,
    ContextIncrease,
    ContextDecrease,
    Save,
    ToggleGrid,
    ToggleSnapping,
    SnapDisable,
    Layout(crate::spatial_layout::Layout),
    GroupSelection,
    UngroupSelection,
    FinishScribble,
    MergeScribbles,
    LinkToFrame,
    UnlinkFromFrame,
    SelectLinkedObjects,
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
    About,
    CheckForUpdates,
    NewBoard,
    CloseBoard,
    OpenBoard,
    ImportImages,
    SaveAs,
    SaveToLocal,
    Paste,
    RelinkSource,
    SaveOriginalAs,
    ToggleAlwaysOnTop,
    ToggleNativeTitleBar,
    MinimizeWindow,
    MaximizeWindow,
    ExportSelectedSources,
    ExportAllSources,
    ExportBoardSnapshot,

    Preferences,
    KeymapEditor,
    RecentBoards,
    ImportKeymap,
    ExportKeymap,
    SaveKeymap,
    ExportPreferences,
    ExportProfile,
    ImportProfile,
    NextToolbarSet,
    PreviousToolbarSet,
    DuplicateSelection,
    AddCameraBookmark,
    JumpCameraSlot(u8),
    CameraBookmarks,
    SourceInfo,
    JoinSharedBoard,
    CopySharedBoardAddress,
    ShareBoard,
    StopSharing,
    ToggleAnnotationSelectionLock,
    ToggleToolbar,
    EditToolbar,
    ToggleStatusBar,
}
impl Action {
    pub fn mutates_selection(self) -> bool {
        use Action::*;
        matches!(
            self,
            ResetCrop
                | ActualSize
                | ToggleGrayscale
                | DeleteSelection
                | FlipHorizontal
                | FlipVertical
                | Order(_)
                | Opacity(_)
                | Filtering(_)
                | CycleFiltering
                | Layout(_)
                | GroupSelection
                | UngroupSelection
                | AnnotationStyle(_)
                | ContextIncrease
                | ContextDecrease
                | ContextCoarseIncrease
                | ContextCoarseDecrease
                | RenameFrame
                | LinkToFrame
                | UnlinkFromFrame
                | MergeScribbles
                | RefreshSource
                | ToggleSourceStorage
                | RelinkSource
        )
    }
    pub fn context(self) -> ActionContext {
        use Action::*;
        match self {
            ResetCrop | ActualSize | ToggleGrayscale | CropMode | Filtering(_) | CycleFiltering
            | OpenSource | RevealSource | CopySourcePath | RefreshSource | ToggleSourceStorage
            | RelinkSource | SourceInfo | SaveOriginalAs => ActionContext::ImageSelection,
            AnnotationStyle(
                crate::annotation_tool::StyleAction::LargerText
                | crate::annotation_tool::StyleAction::SmallerText
                | crate::annotation_tool::StyleAction::AlignText,
            ) => ActionContext::TextSelection,
            AnnotationStyle(
                crate::annotation_tool::StyleAction::Wider
                | crate::annotation_tool::StyleAction::Narrower,
            )
            | MergeScribbles => ActionContext::DrawingSelection,
            AnnotationStyle(_) => ActionContext::Global,
            ImagePointer
            | ToggleSelection
            | RotateImage
            | ScaleImage
            | AdjustOpacity
            | CenterPointer
            | SelectAll
            | DeleteSelection
            | FlipHorizontal
            | FlipVertical
            | Order(_)
            | Opacity(_)
            | ContextIncrease
            | ContextDecrease
            | ContextCoarseIncrease
            | ContextCoarseDecrease
            | Layout(_)
            | GroupSelection
            | UngroupSelection
            | CreateFrame
            | RenameFrame
            | LinkToFrame
            | UnlinkFromFrame
            | SelectLinkedObjects
            | ToggleLockSelection
            | SelectLockedObjects
            | DuplicateSelection => ActionContext::Canvas,
            _ => ActionContext::Global,
        }
    }
    /// Enumerable action catalog, including currently unassigned actions.
    pub const ALL: [Self; 151] = [
        Self::SelectTool(Tool::Mouse),
        Self::TemporaryTool(Tool::Mouse),
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
        Self::ResetCrop,
        Self::ActualSize,
        Self::ToggleGrayscale,
        Self::ToggleLockSelection,
        Self::SelectLockedObjects,
        Self::RefreshSource,
        Self::ToggleSourceStorage,
        Self::ContextCoarseIncrease,
        Self::ContextCoarseDecrease,
        Self::ContextIncrease,
        Self::ContextDecrease,
        Self::Save,
        Self::ToggleGrid,
        Self::ToggleSnapping,
        Self::SnapDisable,
        Self::Layout(crate::spatial_layout::Layout::Grid),
        Self::Layout(crate::spatial_layout::Layout::SnapToGrid),
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
        Self::FinishScribble,
        Self::MergeScribbles,
        Self::SelectTool(Tool::Eraser),
        Self::TemporaryTool(Tool::Eraser),
        Self::LinkToFrame,
        Self::UnlinkFromFrame,
        Self::SelectLinkedObjects,
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
        Self::About,
        Self::CheckForUpdates,
        Self::NewBoard,
        Self::CloseBoard,
        Self::OpenBoard,
        Self::ImportImages,
        Self::SaveAs,
        Self::SaveToLocal,
        Self::Paste,
        Self::RelinkSource,
        Self::SaveOriginalAs,
        Self::ToggleAlwaysOnTop,
        Self::ToggleNativeTitleBar,
        Self::MinimizeWindow,
        Self::MaximizeWindow,
        Self::ExportSelectedSources,
        Self::ExportAllSources,
        Self::ExportBoardSnapshot,
        Self::Preferences,
        Self::KeymapEditor,
        Self::RecentBoards,
        Self::ImportKeymap,
        Self::ExportKeymap,
        Self::SaveKeymap,
        Self::ExportPreferences,
        Self::ExportProfile,
        Self::ImportProfile,
        Self::NextToolbarSet,
        Self::PreviousToolbarSet,
        Self::DuplicateSelection,
        Self::AddCameraBookmark,
        Self::JumpCameraSlot(0),
        Self::JumpCameraSlot(1),
        Self::JumpCameraSlot(2),
        Self::JumpCameraSlot(3),
        Self::JumpCameraSlot(4),
        Self::JumpCameraSlot(5),
        Self::JumpCameraSlot(6),
        Self::JumpCameraSlot(7),
        Self::JumpCameraSlot(8),
        Self::JumpCameraSlot(9),
        Self::CameraBookmarks,
        Self::SourceInfo,
        Self::JoinSharedBoard,
        Self::CopySharedBoardAddress,
        Self::ShareBoard,
        Self::StopSharing,
        Self::ToggleAnnotationSelectionLock,
        Self::ToggleToolbar,
        Self::EditToolbar,
        Self::ToggleStatusBar,
    ];
    /// Stable readable semantic names; never a positional catalog index.
    pub fn id(self) -> String {
        format!("{self:?}")
    }
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|a| a.id() == id).or_else(|| {
            let slot: u8 = id
                .strip_prefix("JumpCameraSlot(")?
                .strip_suffix(')')?
                .parse()
                .ok()?;
            (usize::from(slot) < crate::camera_slots::MAX_VIEWS)
                .then_some(Self::JumpCameraSlot(slot))
        })
    }
    pub fn is_local(self) -> bool {
        matches!(
            self,
            Self::ApplicationMenu
                | Self::About
                | Self::CheckForUpdates
                | Self::NewBoard
                | Self::CloseBoard
                | Self::OpenBoard
                | Self::ImportImages
                | Self::SaveAs
                | Self::SaveToLocal
                | Self::Paste
                | Self::RefreshSource
                | Self::ToggleSourceStorage
                | Self::RelinkSource
                | Self::SaveOriginalAs
                | Self::ToggleAlwaysOnTop
                | Self::ToggleNativeTitleBar
                | Self::MinimizeWindow
                | Self::MaximizeWindow
                | Self::ExportSelectedSources
                | Self::ExportAllSources
                | Self::ExportBoardSnapshot
                | Self::Preferences
                | Self::KeymapEditor
                | Self::RecentBoards
                | Self::ImportKeymap
                | Self::SaveKeymap
                | Self::ExportKeymap
                | Self::ExportProfile
                | Self::ImportProfile
                | Self::NextToolbarSet
                | Self::PreviousToolbarSet
                | Self::ExportPreferences
                | Self::DuplicateSelection
                | Self::AddCameraBookmark
                | Self::JumpCameraSlot(_)
                | Self::CameraBookmarks
                | Self::SourceInfo
                | Self::JoinSharedBoard
                | Self::CopySharedBoardAddress
                | Self::ShareBoard
                | Self::StopSharing
                | Self::ToggleToolbar
                | Self::EditToolbar
                | Self::ToggleStatusBar
        )
    }
    pub fn available(self) -> bool {
        (!matches!(
            self,
            Self::ShareBoard
                | Self::StopSharing
                | Self::JoinSharedBoard
                | Self::CopySharedBoardAddress
                | Self::SaveToLocal
        ) || cfg!(feature = "network"))
            && (self != Self::CheckForUpdates || cfg!(feature = "updater"))
    }
    pub fn category(self) -> &'static str {
        use Action::*;
        match self {
            SelectTool(_) | TemporaryTool(_) | ToggleAnnotationSelectionLock => "TOOLS",
            PanView | ZoomView | CenterPointer | AddCameraBookmark | JumpCameraSlot(_)
            | CameraBookmarks | FocusFrame | NextFrame | PreviousFrame | ToggleGrid
            | ToggleSnapping | SnapDisable => "NAVIGATION",
            RefreshSource | ToggleSourceStorage | ImportImages | CropMode | ResetCrop
            | ActualSize | ToggleGrayscale | Filtering(_) | CycleFiltering | SourceInfo
            | OpenSource | RevealSource | CopySourcePath | SaveOriginalAs | RelinkSource
            | RotateImage | ScaleImage | AdjustOpacity | FlipHorizontal | FlipVertical => "IMAGES",
            AnnotationStyle(
                crate::annotation_tool::StyleAction::LargerText
                | crate::annotation_tool::StyleAction::SmallerText
                | crate::annotation_tool::StyleAction::AlignText,
            ) => "TEXT",
            AnnotationStyle(_)
            | FinishScribble
            | MergeScribbles
            | ContextIncrease
            | ContextDecrease
            | ContextCoarseIncrease
            | ContextCoarseDecrease => "DRAWING",
            CreateFrame | RenameFrame | LinkToFrame | UnlinkFromFrame | SelectLinkedObjects => {
                "FRAMES"
            }
            JoinSharedBoard | CopySharedBoardAddress | ShareBoard | StopSharing | SaveToLocal => {
                "COLLABORATION"
            }
            _ => "GENERAL",
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
            Self::ResetCrop => "Reset Crop",
            Self::ActualSize => "Actual Size / 1:1",
            Self::ToggleGrayscale => "Toggle Grayscale",
            Self::ToggleLockSelection => "Toggle Lock Selection",
            Self::SelectLockedObjects => "Select Locked Objects",
            Self::RefreshSource => "Refresh Source",
            Self::ToggleSourceStorage => "Toggle Linked / Embedded",
            Self::ContextCoarseIncrease => "Context coarse increase",
            Self::ContextCoarseDecrease => "Context coarse decrease",
            Self::ContextIncrease => "Context increase",
            Self::ContextDecrease => "Context decrease",
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
                crate::spatial_layout::Layout::Grid => "Arrange in Grid",
                crate::spatial_layout::Layout::SnapToGrid => "Snap selection to grid",
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
            Self::FinishScribble => "Finish Scribble",
            Self::MergeScribbles => "Merge Scribbles",
            Self::LinkToFrame => "Link to Frame",
            Self::UnlinkFromFrame => "Unlink from Frame",
            Self::SelectLinkedObjects => "Select Linked Objects",
            Self::CreateFrame => "Create frame",
            Self::RenameFrame => "Rename frame",
            Self::FocusFrame => "Fit Frame in View",
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
            Self::About => "About Tack",
            Self::CheckForUpdates => "Update...",
            Self::CloseBoard => "Close Board",
            Self::NewBoard => "New board",
            Self::OpenBoard => "Open board",
            Self::ImportImages => "Import images",
            Self::SaveToLocal => "Save to Local...",
            Self::SaveAs => "Save As",
            Self::Paste => "Paste",
            Self::RelinkSource => "Relink selected source",
            Self::SaveOriginalAs => "Save Original As...",
            Self::ToggleAlwaysOnTop => "Always on Top",
            Self::ToggleNativeTitleBar => "Native title bar",
            Self::MinimizeWindow => "Minimize window",
            Self::MaximizeWindow => "Maximize / Restore window",
            Self::ExportSelectedSources => "Export Selected Sources...",
            Self::ExportAllSources => "Export All Sources...",
            Self::ExportBoardSnapshot => "Export Board Snapshot...",

            Self::Preferences => "Preferences",
            Self::KeymapEditor => "Edit keymap",
            Self::RecentBoards => "Recent boards",
            Self::ImportKeymap => "Import keymap",
            Self::ExportProfile => "Export Profile...",
            Self::ImportProfile => "Import Profile...",
            Self::NextToolbarSet => "Next Toolbar Set",
            Self::PreviousToolbarSet => "Previous Toolbar Set",
            Self::ExportPreferences => "Export Preferences...",
            Self::SaveKeymap => "Save keyset",
            Self::ExportKeymap => "Export keymap",
            Self::DuplicateSelection => "Duplicate selection",
            Self::AddCameraBookmark => "Assign view shortcut...",
            Self::JumpCameraSlot(slot) => match slot {
                0 => "Jump to view 0",
                1 => "Jump to view 1",
                2 => "Jump to view 2",
                3 => "Jump to view 3",
                4 => "Jump to view 4",
                5 => "Jump to view 5",
                6 => "Jump to view 6",
                7 => "Jump to view 7",
                8 => "Jump to view 8",
                9 => "Jump to view 9",
                _ => "Jump to view",
            },
            Self::CameraBookmarks => "Camera bookmarks...",
            Self::SourceInfo => "Image information...",
            Self::JoinSharedBoard => "Join shared board...",
            Self::CopySharedBoardAddress => "Copy Invite",
            Self::ShareBoard => "Share Board...",
            Self::StopSharing => "Stop Sharing",
            Self::ToggleAnnotationSelectionLock => "Lock annotation selection",
            Self::ToggleToolbar => "Show / hide toolbar",
            Self::EditToolbar => "Edit Toolbar...",
            Self::ToggleStatusBar => "Show / hide status bar",
        }
    }
}

/// A deliberately small classification, independent of shortcut controls.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionContext {
    Global,
    Canvas,
    ImageSelection,
    DrawingSelection,
    TextSelection,
    TextEditing,
    TimedAsset,
}
impl ActionContext {
    pub fn overlaps(self, other: Self) -> bool {
        self == other
            || matches!(self, Self::Global | Self::Canvas)
            || matches!(other, Self::Global | Self::Canvas)
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
    pub fn complete_creation(&mut self, tool: Tool) {
        if tool.is_one_shot()
            && self.base == tool
            && !self.temporary.iter().any(|(_, held)| *held == tool)
        {
            self.reset_pointer();
        }
    }
    pub fn release_for_modal(&mut self) {
        self.temporary.clear();
        if self.base == Tool::Text {
            self.base = Tool::Pointer;
        }
    }
    pub fn reset_pointer(&mut self) {
        self.base = Tool::Pointer;
        self.temporary.clear();
    }
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
