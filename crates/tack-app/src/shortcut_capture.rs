//! Explicit shortcut staging, shared by Keymap and bookmark capture.
use crate::{
    actions::Action,
    bindings::{Binding, BindingError, Keymap, Trigger},
};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Behavior {
    #[default]
    Normal,
    Hold,
    Release,
}
impl Behavior {
    pub(crate) fn from_trigger(t: Trigger) -> Self {
        match t {
            Trigger::Hold => Self::Hold,
            Trigger::Release => Self::Release,
            _ => Self::Normal,
        }
    }
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Normal => "Normal",
            Self::Hold => "Hold",
            Self::Release => "Release",
        }
    }
    pub(crate) fn next(self) -> Self {
        match self {
            Self::Normal => Self::Hold,
            Self::Hold => Self::Release,
            Self::Release => Self::Normal,
        }
    }
    pub(crate) fn binding(self, action: Action, mut b: Binding) -> Binding {
        b.action = match (action, self) {
            (Action::SelectTool(t), Self::Hold) => Action::TemporaryTool(t),
            _ => action,
        };
        b.trigger = if matches!(b.control, crate::input::PhysicalControl::Wheel(_)) {
            Trigger::Wheel
        } else {
            match self {
                Self::Normal => Trigger::Press,
                Self::Hold => Trigger::Hold,
                Self::Release => Trigger::Release,
            }
        };
        b
    }
}
pub(crate) struct Capture {
    pub(crate) candidate: Keymap,
    pub(crate) binding: Binding,
    pub(crate) displaced: Vec<Action>,
}
impl Capture {
    pub(crate) fn stage(
        keymap: &Keymap,
        action: Action,
        binding: Binding,
    ) -> Result<Self, BindingError> {
        let mut candidate = keymap.clone();
        candidate.unassign(action);
        if let Action::SelectTool(t) = action {
            candidate.unassign(Action::TemporaryTool(t));
        }
        let mut check = Keymap::default();
        check.bind(binding)?;
        let mut displaced = Vec::new();
        for old in keymap.bindings() {
            if old.action != action
                && crate::bindings::controls_overlap(old.control, binding.control)
                && old.modifiers.overlaps(binding.modifiers)
                && !displaced.contains(&old.action)
            {
                candidate.unassign(old.action);
                displaced.push(old.action);
            }
        }
        loop {
            match candidate.bind(binding) {
                Ok(()) => {
                    return Ok(Self {
                        candidate,
                        binding,
                        displaced,
                    });
                }
                Err(BindingError::Conflict { existing }) => {
                    let old = candidate.bindings()[existing].action;
                    candidate.unassign(old);
                    displaced.push(old);
                }
                Err(e) => return Err(e),
            }
        }
    }
    pub(crate) fn message(&self) -> String {
        let conflict = self
            .displaced
            .iter()
            .map(|a| a.label())
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "{} -> {}{}. Enter confirms; Escape cancels",
            crate::context_menu::binding_label(&self.binding),
            self.binding.action.label(),
            if conflict.is_empty() {
                String::new()
            } else {
                format!(" replaces {conflict}")
            }
        )
    }
}
