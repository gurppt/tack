use crate::{
    actions::Action,
    input::{Modifiers, PhysicalControl},
};
use std::{error::Error, fmt};

pub const MAX_BINDINGS: usize = 256;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trigger {
    Press,
    Release,
    Hold,
    Wheel,
}

/// Exact is appropriate for keyboard shortcuts; Contains/Any preserve the
/// prototype's modifier-insensitive middle pan and Alt+left behavior.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModifierMatch {
    Exact(Modifiers),
    Contains(Modifiers),
    Any,
}
impl ModifierMatch {
    pub fn matches(self, actual: Modifiers) -> bool {
        match self {
            Self::Exact(wanted) => wanted == actual,
            Self::Contains(wanted) => actual.contains(wanted),
            Self::Any => true,
        }
    }
    fn overlaps(self, other: Self) -> bool {
        (0..16).any(|mask| {
            self.matches(Modifiers::from_mask(mask)) && other.matches(Modifiers::from_mask(mask))
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Binding {
    pub control: PhysicalControl,
    pub modifiers: ModifierMatch,
    pub trigger: Trigger,
    pub action: Action,
}

/// Bounded user preferences. An empty map assigns nothing. Configuration changes
/// are explicit; callers can replace it while held temporary tools still retain
/// their original release action/token in InputState.
#[derive(Clone, Debug, Default)]
pub struct Keymap {
    bindings: Vec<Binding>,
}
impl Keymap {
    pub fn bind(&mut self, binding: Binding) -> Result<(), BindingError> {
        let wheel = matches!(binding.control, PhysicalControl::Wheel(_));
        let valid = match binding.trigger {
            Trigger::Wheel => wheel && binding.action == Action::ZoomView,
            Trigger::Hold => {
                !wheel && (binding.action == Action::PanView || binding.action.captured_hold())
            }
            Trigger::Press | Trigger::Release => {
                !wheel
                    && !binding.action.captured_hold()
                    && !matches!(binding.action, Action::PanView | Action::ZoomView)
            }
        };
        if !valid {
            return Err(BindingError::InvalidTrigger);
        }
        if let Some(existing) = self.bindings.iter().position(|b| {
            b.control == binding.control
                && (b.trigger == binding.trigger
                    || matches!(
                        (b.trigger, binding.trigger),
                        (Trigger::Hold, Trigger::Press) | (Trigger::Press, Trigger::Hold)
                    ))
                && b.modifiers.overlaps(binding.modifiers)
        }) {
            return Err(BindingError::Conflict { existing });
        }
        if self.bindings.len() == MAX_BINDINGS {
            return Err(BindingError::Full);
        }
        self.bindings.push(binding);
        Ok(())
    }
    pub fn unassign(&mut self, action: Action) {
        self.bindings.retain(|binding| binding.action != action);
    }
    pub fn bindings(&self) -> &[Binding] {
        &self.bindings
    }
    pub(crate) fn has_control(&self, control: PhysicalControl) -> bool {
        self.bindings
            .iter()
            .any(|binding| binding.control == control)
    }
    pub fn for_action(&self, action: Action) -> impl Iterator<Item = &Binding> {
        self.bindings.iter().filter(move |b| b.action == action)
    }
    pub(crate) fn matching(
        &self,
        control: PhysicalControl,
        modifiers: Modifiers,
        trigger: Trigger,
    ) -> impl Iterator<Item = Binding> {
        self.bindings.iter().copied().filter(move |b| {
            b.control == control && b.trigger == trigger && b.modifiers.matches(modifiers)
        })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BindingError {
    InvalidTrigger,
    Conflict { existing: usize },
    Full,
}
impl fmt::Display for BindingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "binding rejected: {self:?}")
    }
}
impl Error for BindingError {}
