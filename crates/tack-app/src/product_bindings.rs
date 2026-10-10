//! Product shortcut defaults; semantic tools remain independent of key codes.
use crate::{
    actions::Action,
    bindings::{Binding, BindingError, Keymap, ModifierMatch, Trigger},
    input::{Modifiers, PhysicalControl, PointerButton, WheelAxis},
};
use winit::{event::MouseButton, keyboard::KeyCode};
pub fn product_keymap() -> Result<Keymap, BindingError> {
    let mut map = Keymap::default();
    map.bind(Binding {
        control: PhysicalControl::LogicalKey(crate::input::LogicalKey::Character('b')),
        modifiers: ModifierMatch::Exact(Modifiers::NONE),
        trigger: Trigger::Press,
        action: Action::AddCameraBookmark,
    })?;
    for slot in 0..10u8 {
        map.bind(Binding {
            control: PhysicalControl::LogicalKey(crate::input::LogicalKey::Character(char::from(
                b'0' + slot,
            ))),
            modifiers: ModifierMatch::Exact(Modifiers::NONE),
            trigger: Trigger::Press,
            action: Action::JumpCameraSlot(slot),
        })?;
    }
    use crate::spatial_layout::Layout;
    let left = PhysicalControl::Pointer(PointerButton::Mouse(MouseButton::Left));
    let ctrl = Modifiers::CONTROL;
    let shift = Modifiers::SHIFT;
    let alt = Modifiers::ALT;
    use crate::{actions::Tool, annotation_tool::StyleAction};
    for (code, mods, action) in [
        (
            KeyCode::F8,
            Modifiers::NONE,
            Action::ToggleAnnotationSelectionLock,
        ),
        (KeyCode::F9, Modifiers::NONE, Action::ToggleToolbar),
        (
            KeyCode::ArrowUp,
            ctrl,
            Action::Order(crate::selection_commands::Order::Forward),
        ),
        (
            KeyCode::ArrowDown,
            ctrl,
            Action::Order(crate::selection_commands::Order::Backward),
        ),
        (
            KeyCode::ArrowUp,
            ctrl.union(shift),
            Action::Order(crate::selection_commands::Order::Front),
        ),
        (
            KeyCode::ArrowDown,
            ctrl.union(shift),
            Action::Order(crate::selection_commands::Order::Back),
        ),
        (KeyCode::Enter, Modifiers::NONE, Action::FinishScribble),
        (
            KeyCode::KeyE,
            Modifiers::NONE,
            Action::SelectTool(Tool::Eraser),
        ),
        (KeyCode::F10, Modifiers::NONE, Action::ApplicationMenu),
        (KeyCode::KeyN, ctrl, Action::NewBoard),
        (KeyCode::KeyO, ctrl, Action::OpenBoard),
        (KeyCode::KeyI, ctrl, Action::ImportImages),
        (KeyCode::KeyS, ctrl.union(shift), Action::SaveAs),
        (KeyCode::KeyV, ctrl, Action::Paste),
        (KeyCode::KeyD, ctrl, Action::DuplicateSelection),
        (KeyCode::KeyR, ctrl.union(shift), Action::RelinkSource),
        (KeyCode::Comma, ctrl, Action::Preferences),
        (
            KeyCode::KeyV,
            Modifiers::NONE,
            Action::SelectTool(Tool::Pointer),
        ),
        (
            KeyCode::KeyT,
            Modifiers::NONE,
            Action::SelectTool(Tool::Text),
        ),
        (
            KeyCode::KeyR,
            Modifiers::NONE,
            Action::SelectTool(Tool::Rectangle),
        ),
        (
            KeyCode::KeyL,
            Modifiers::NONE,
            Action::SelectTool(Tool::Line),
        ),
        (
            KeyCode::KeyA,
            Modifiers::NONE,
            Action::SelectTool(Tool::Arrow),
        ),
        (
            KeyCode::KeyP,
            Modifiers::NONE,
            Action::SelectTool(Tool::Scribble),
        ),
        (
            KeyCode::KeyC,
            Modifiers::NONE,
            Action::AnnotationStyle(StyleAction::Color),
        ),
        (
            KeyCode::KeyF,
            Modifiers::NONE,
            Action::AnnotationStyle(StyleAction::Fill),
        ),
        (
            KeyCode::Period,
            ctrl.union(shift),
            Action::AnnotationStyle(StyleAction::LargerText),
        ),
        (
            KeyCode::Comma,
            ctrl.union(shift),
            Action::AnnotationStyle(StyleAction::SmallerText),
        ),
        (
            KeyCode::BracketRight,
            shift,
            Action::AnnotationStyle(StyleAction::OpacityUp),
        ),
        (
            KeyCode::BracketLeft,
            shift,
            Action::AnnotationStyle(StyleAction::OpacityDown),
        ),
        (
            KeyCode::KeyE,
            ctrl.union(shift),
            Action::AnnotationStyle(StyleAction::AlignText),
        ),
        (KeyCode::KeyO, ctrl.union(shift), Action::OpenSource),
        (KeyCode::KeyO, ctrl.union(alt), Action::RevealSource),
        (KeyCode::KeyC, ctrl.union(shift), Action::CopySourcePath),
    ] {
        map.bind(Binding {
            control: PhysicalControl::LogicalKey(
                crate::input::LogicalKey::from_legacy(code).ok_or(BindingError::InvalidControl)?,
            ),
            modifiers: ModifierMatch::Exact(mods),
            trigger: Trigger::Press,
            action,
        })?;
    }
    for (mods, action) in [
        (Modifiers::NONE, Action::ImagePointer),
        (shift, Action::ToggleSelection),
        (ctrl, Action::RotateImage),
        (ctrl.union(shift), Action::RotateImage),
        (ctrl.union(alt), Action::ScaleImage),
        (ctrl.union(alt).union(shift), Action::AdjustOpacity),
        (alt, Action::CenterPointer),
        (alt.union(shift), Action::PanView),
    ] {
        map.bind(Binding {
            control: left,
            modifiers: ModifierMatch::Exact(mods),
            trigger: Trigger::Hold,
            action,
        })?;
    }
    map.bind(Binding {
        control: PhysicalControl::Pointer(PointerButton::Mouse(MouseButton::Middle)),
        modifiers: ModifierMatch::Any,
        trigger: Trigger::Hold,
        action: Action::PanView,
    })?;
    map.bind(Binding {
        control: PhysicalControl::Wheel(WheelAxis::Vertical),
        modifiers: ModifierMatch::Any,
        trigger: Trigger::Wheel,
        action: Action::ZoomView,
    })?;
    for (code, mods, action) in [
        (KeyCode::Escape, Modifiers::NONE, Action::CancelInteraction),
        (KeyCode::Delete, Modifiers::NONE, Action::DeleteSelection),
        (KeyCode::KeyA, ctrl, Action::SelectAll),
        (KeyCode::KeyZ, ctrl, Action::Undo),
        (KeyCode::KeyZ, ctrl.union(shift), Action::Redo),
        (KeyCode::KeyY, ctrl, Action::Redo),
        (KeyCode::KeyS, ctrl, Action::Save),
        (
            KeyCode::KeyC,
            ctrl.union(alt).union(shift),
            Action::CropMode,
        ),
        (KeyCode::KeyH, alt.union(shift), Action::FlipHorizontal),
        (KeyCode::KeyV, alt.union(shift), Action::FlipVertical),
        (KeyCode::KeyT, alt, Action::CycleFiltering),
    ] {
        map.bind(Binding {
            control: PhysicalControl::LogicalKey(
                crate::input::LogicalKey::from_legacy(code).ok_or(BindingError::InvalidControl)?,
            ),
            modifiers: if action == Action::CancelInteraction {
                ModifierMatch::Any
            } else {
                ModifierMatch::Exact(mods)
            },
            trigger: Trigger::Press,
            action,
        })?;
    }
    for (code, mods, action) in [
        (KeyCode::KeyG, Modifiers::NONE, Action::ToggleGrid),
        (KeyCode::KeyG, shift, Action::ToggleSnapping),
        (KeyCode::KeyL, ctrl, Action::LinkToFrame),
        (KeyCode::KeyL, ctrl.union(shift), Action::UnlinkFromFrame),
        (KeyCode::KeyG, ctrl, Action::GroupSelection),
        (KeyCode::KeyG, ctrl.union(shift), Action::UngroupSelection),
        (KeyCode::KeyF, ctrl.union(shift), Action::CreateFrame),
        (KeyCode::F2, Modifiers::NONE, Action::RenameFrame),
        (KeyCode::Space, Modifiers::NONE, Action::FocusFrame),
        (KeyCode::PageDown, Modifiers::NONE, Action::NextFrame),
        (KeyCode::PageUp, Modifiers::NONE, Action::PreviousFrame),
        (KeyCode::ArrowLeft, ctrl, Action::Layout(Layout::Left)),
        (KeyCode::ArrowRight, ctrl, Action::Layout(Layout::Right)),
        (
            KeyCode::ArrowUp,
            ctrl.union(alt),
            Action::Layout(Layout::Top),
        ),
        (
            KeyCode::ArrowDown,
            ctrl.union(alt),
            Action::Layout(Layout::Bottom),
        ),
        (
            KeyCode::ArrowLeft,
            ctrl.union(shift),
            Action::Layout(Layout::HorizontalCenter),
        ),
        (
            KeyCode::ArrowUp,
            alt.union(shift),
            Action::Layout(Layout::VerticalCenter),
        ),
        (
            KeyCode::ArrowUp,
            ctrl.union(alt).union(shift),
            Action::Layout(Layout::DistributeHorizontal),
        ),
        (
            KeyCode::ArrowDown,
            ctrl.union(alt).union(shift),
            Action::Layout(Layout::DistributeVertical),
        ),
        (KeyCode::KeyP, ctrl, Action::Layout(Layout::PackHorizontal)),
        (
            KeyCode::KeyP,
            ctrl.union(shift),
            Action::Layout(Layout::PackVertical),
        ),
    ] {
        map.bind(Binding {
            control: PhysicalControl::LogicalKey(
                crate::input::LogicalKey::from_legacy(code).ok_or(BindingError::InvalidControl)?,
            ),
            modifiers: ModifierMatch::Exact(mods),
            trigger: Trigger::Press,
            action,
        })?;
    }
    for (code, layout) in [
        (KeyCode::KeyD, Layout::DistributeHorizontal),
        (KeyCode::KeyV, Layout::DistributeVertical),
    ] {
        map.bind(Binding {
            control: PhysicalControl::LogicalKey(
                crate::input::LogicalKey::from_legacy(code).ok_or(BindingError::InvalidControl)?,
            ),
            modifiers: ModifierMatch::Exact(ctrl.union(shift)),
            trigger: Trigger::Press,
            action: Action::Layout(layout),
        })?;
    }
    map.bind(Binding {
        control: PhysicalControl::LogicalKey(crate::input::LogicalKey::Character('x')),
        modifiers: ModifierMatch::Any,
        trigger: Trigger::Hold,
        action: Action::SnapDisable,
    })?;
    for (code, action) in [
        (KeyCode::NumpadAdd, Action::ContextIncrease),
        (KeyCode::NumpadSubtract, Action::ContextDecrease),
    ] {
        map.bind(Binding {
            control: PhysicalControl::Key(winit::keyboard::PhysicalKey::Code(code)),
            modifiers: ModifierMatch::Exact(Modifiers::NONE),
            trigger: Trigger::Press,
            action,
        })?;
    }
    Ok(map)
}
