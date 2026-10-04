//! Product shortcut defaults; semantic tools remain independent of key codes.
use crate::{
    actions::Action,
    bindings::{Binding, BindingError, Keymap, ModifierMatch, Trigger},
    input::{Modifiers, PhysicalControl, PointerButton, WheelAxis},
};
use winit::{
    event::MouseButton,
    keyboard::{KeyCode, PhysicalKey},
};
pub fn product_keymap() -> Result<Keymap, BindingError> {
    let mut map = Keymap::default();
    use crate::spatial_layout::Layout;
    let left = PhysicalControl::Pointer(PointerButton::Mouse(MouseButton::Left));
    let ctrl = Modifiers::CONTROL;
    let shift = Modifiers::SHIFT;
    let alt = Modifiers::ALT;
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
            control: PhysicalControl::Key(PhysicalKey::Code(code)),
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
        (KeyCode::KeyG, ctrl, Action::GroupSelection),
        (KeyCode::KeyG, ctrl.union(shift), Action::UngroupSelection),
        (KeyCode::KeyF, ctrl.union(shift), Action::CreateFrame),
        (KeyCode::F2, Modifiers::NONE, Action::RenameFrame),
        (KeyCode::Space, Modifiers::NONE, Action::FocusFrame),
        (KeyCode::PageDown, Modifiers::NONE, Action::NextFrame),
        (KeyCode::PageUp, Modifiers::NONE, Action::PreviousFrame),
        (KeyCode::ArrowLeft, ctrl, Action::Layout(Layout::Left)),
        (KeyCode::ArrowRight, ctrl, Action::Layout(Layout::Right)),
        (KeyCode::ArrowUp, ctrl, Action::Layout(Layout::Top)),
        (KeyCode::ArrowDown, ctrl, Action::Layout(Layout::Bottom)),
        (
            KeyCode::ArrowLeft,
            ctrl.union(shift),
            Action::Layout(Layout::HorizontalCenter),
        ),
        (
            KeyCode::ArrowUp,
            ctrl.union(shift),
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
            control: PhysicalControl::Key(PhysicalKey::Code(code)),
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
            control: PhysicalControl::Key(PhysicalKey::Code(code)),
            modifiers: ModifierMatch::Exact(ctrl.union(shift)),
            trigger: Trigger::Press,
            action: Action::Layout(layout),
        })?;
    }
    map.bind(Binding {
        control: PhysicalControl::Key(PhysicalKey::Code(KeyCode::KeyX)),
        modifiers: ModifierMatch::Any,
        trigger: Trigger::Hold,
        action: Action::SnapDisable,
    })?;
    Ok(map)
}
