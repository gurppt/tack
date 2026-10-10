use crate::{
    actions::Action,
    input::{LogicalKey, Modifiers, PhysicalControl},
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
    pub(crate) fn overlaps(self, other: Self) -> bool {
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

/// Dedicated keypad controls cannot alias ordinary layout keys. Physical
/// resolution takes precedence only for the keypad key that is actually pressed.
pub(crate) fn controls_overlap(a: PhysicalControl, b: PhysicalControl) -> bool {
    use winit::keyboard::{KeyCode, PhysicalKey};
    let keypad = |c| {
        matches!(
            c,
            PhysicalControl::Key(PhysicalKey::Code(
                KeyCode::NumpadAdd
                    | KeyCode::NumpadSubtract
                    | KeyCode::Numpad0
                    | KeyCode::Numpad1
                    | KeyCode::Numpad2
                    | KeyCode::Numpad3
                    | KeyCode::Numpad4
                    | KeyCode::Numpad5
                    | KeyCode::Numpad6
                    | KeyCode::Numpad7
                    | KeyCode::Numpad8
                    | KeyCode::Numpad9
            ))
        )
    };
    a == b
        || (matches!(
            (a, b),
            (PhysicalControl::Key(_), PhysicalControl::LogicalKey(_))
                | (PhysicalControl::LogicalKey(_), PhysicalControl::Key(_))
        ) && !keypad(a)
            && !keypad(b))
}
/// Bounded user preferences. An empty map assigns nothing. Configuration changes
/// are explicit; callers can replace it while held temporary tools still retain
/// their original release action/token in InputState.
#[derive(Clone, Debug, Default)]
pub struct Keymap {
    bindings: Vec<Binding>,
}
impl Keymap {
    pub fn bind(&mut self, mut binding: Binding) -> Result<(), BindingError> {
        if crate::menu_access::reserved(&binding) && !crate::menu_access::canonical(&binding) {
            return Err(BindingError::ReservedMenu);
        }
        if let PhysicalControl::LogicalKey(LogicalKey::Character(c)) = binding.control {
            if c.is_control() {
                return Err(BindingError::InvalidControl);
            }
            binding.control =
                PhysicalControl::LogicalKey(LogicalKey::Character(c.to_ascii_lowercase()));
        }
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
            let mixed = matches!(
                (b.control, binding.control),
                (PhysicalControl::Key(_), PhysicalControl::LogicalKey(_))
                    | (PhysicalControl::LogicalKey(_), PhysicalControl::Key(_))
            );
            controls_overlap(b.control, binding.control)
                && (mixed
                    || b.trigger == binding.trigger
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
    /// Resolve one keyboard domain only. Mixed physical/logical bindings with
    /// overlapping modifiers/triggers are refused by bind, avoiding double actions.
    pub fn keyboard_control(
        &self,
        physical: winit::keyboard::PhysicalKey,
        logical: Option<LogicalKey>,
        modifiers: Modifiers,
    ) -> PhysicalControl {
        let physical = PhysicalControl::Key(physical);
        if self
            .bindings
            .iter()
            .any(|b| b.control == physical && b.modifiers.matches(modifiers))
        {
            physical
        } else {
            logical.map(PhysicalControl::LogicalKey).unwrap_or(physical)
        }
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
    /// Explicit pointer bindings take priority over native fallback gestures.
    pub fn pointer_bound(&self, button: crate::input::PointerButton, modifiers: Modifiers) -> bool {
        self.bindings.iter().any(|b| {
            b.control == PhysicalControl::Pointer(button) && b.modifiers.matches(modifiers)
        })
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
    ReservedMenu,
    InvalidTrigger,
    InvalidControl,
    Conflict { existing: usize },
    Full,
}
impl fmt::Display for BindingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if *self == Self::ReservedMenu {
            write!(f, "F10 is reserved for the menu and cannot be changed")
        } else {
            write!(f, "binding rejected: {self:?}")
        }
    }
}
impl Error for BindingError {}
