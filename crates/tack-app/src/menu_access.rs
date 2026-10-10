//! Fixed menu escape hatch, independent of editable profile bindings.
use crate::{
    actions::Action,
    bindings::{Binding, ModifierMatch, Trigger},
    input::{LogicalKey, Modifiers, PhysicalControl},
};
use winit::keyboard::{KeyCode, NamedKey, PhysicalKey};

pub const LABEL: &str = "F10: Menu (fixed)";

pub fn reserved(binding: &Binding) -> bool {
    is_key(binding.control) && binding.modifiers.matches(Modifiers::NONE)
}

pub fn canonical(binding: &Binding) -> bool {
    reserved(binding)
        && binding.modifiers == ModifierMatch::Exact(Modifiers::NONE)
        && binding.action == Action::ApplicationMenu
        && binding.trigger == Trigger::Press
}

pub fn is_key(control: PhysicalControl) -> bool {
    matches!(
        control,
        PhysicalControl::Key(PhysicalKey::Code(KeyCode::F10))
            | PhysicalControl::LogicalKey(LogicalKey::Named(NamedKey::F10))
    )
}
