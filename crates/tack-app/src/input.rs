//! winit normalization and bounded physical state. Tools see only ActionEvent.
use crate::{
    actions::{Action, ActionEvent, ActionPhase, HoldToken},
    bindings::{Keymap, Trigger},
};
use std::{error::Error, fmt};
use winit::{
    event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent},
    keyboard::{ModifiersState, PhysicalKey},
};

pub const MAX_HELD_INPUTS: usize = 32;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointerButton {
    Mouse(MouseButton),
    Stylus(u16),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WheelAxis {
    Horizontal,
    Vertical,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PhysicalControl {
    Key(PhysicalKey),
    Pointer(PointerButton),
    Wheel(WheelAxis),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Modifiers(u8);
impl Modifiers {
    pub const NONE: Self = Self(0);
    pub const ALT: Self = Self(1);
    pub const CONTROL: Self = Self(2);
    pub const SHIFT: Self = Self(4);
    pub const SUPER: Self = Self(8);
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
    pub fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
    pub(crate) fn from_mask(mask: u8) -> Self {
        Self(mask & 15)
    }
}
impl From<ModifiersState> for Modifiers {
    fn from(value: ModifiersState) -> Self {
        Self(
            u8::from(value.alt_key())
                | (u8::from(value.control_key()) * 2)
                | (u8::from(value.shift_key()) * 4)
                | (u8::from(value.super_key()) * 8),
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PhysicalEvent {
    Button {
        control: PhysicalControl,
        state: ElementState,
        repeat: bool,
    },
    Wheel {
        axis: WheelAxis,
        steps: f64,
    },
    Modifiers(Modifiers),
    FocusLost,
}

/// At most two events (wheel axes); no event queue or per-event allocation.
pub fn normalize(event: &WindowEvent) -> [Option<PhysicalEvent>; 2] {
    let first = match event {
        WindowEvent::KeyboardInput { event, .. } => Some(PhysicalEvent::Button {
            control: PhysicalControl::Key(event.physical_key),
            state: event.state,
            repeat: event.repeat,
        }),
        WindowEvent::MouseInput { state, button, .. } => Some(PhysicalEvent::Button {
            control: PhysicalControl::Pointer(PointerButton::Mouse(*button)),
            state: *state,
            repeat: false,
        }),
        WindowEvent::ModifiersChanged(modifiers) => {
            Some(PhysicalEvent::Modifiers(modifiers.state().into()))
        }
        WindowEvent::Focused(false) => Some(PhysicalEvent::FocusLost),
        WindowEvent::MouseWheel { delta, .. } => {
            let [x, y] = wheel_steps(*delta);
            return [
                Some(PhysicalEvent::Wheel {
                    axis: WheelAxis::Horizontal,
                    steps: x,
                }),
                Some(PhysicalEvent::Wheel {
                    axis: WheelAxis::Vertical,
                    steps: y,
                }),
            ];
        }
        _ => None,
    };
    [first, None]
}
pub fn wheel_steps(delta: MouseScrollDelta) -> [f64; 2] {
    match delta {
        MouseScrollDelta::LineDelta(x, y) => [f64::from(x), f64::from(y)],
        MouseScrollDelta::PixelDelta(p) => [p.x / 100.0, p.y / 100.0],
    }
}

#[derive(Clone, Copy)]
struct HeldInput {
    control: PhysicalControl,
    temporary: Option<(Action, HoldToken)>,
}
/// Captured temporary gestures are released even if modifiers/keymap change.
/// Continuous pan holds instead follow current modifiers, including late Alt.
#[derive(Default)]
pub struct InputState {
    modifiers: Modifiers,
    held: Vec<HeldInput>,
    next_token: u64,
}
impl InputState {
    pub fn held_len(&self) -> usize {
        self.held.len()
    }
    pub fn is_action_held(&self, keymap: &Keymap, action: Action) -> bool {
        if matches!(action, Action::TemporaryTool(_)) {
            return self.held.iter().any(|held| {
                held.temporary
                    .is_some_and(|(captured, _)| captured == action)
            });
        }
        self.held.iter().any(|held| {
            keymap
                .matching(held.control, self.modifiers, Trigger::Hold)
                .any(|b| b.action == action)
        })
    }
    /// Dispatch immediately through the callback; at most MAX_HELD_INPUTS on
    /// focus loss or two matches per button edge. No retained event payloads.
    pub fn handle(
        &mut self,
        event: PhysicalEvent,
        keymap: &Keymap,
        mut emit: impl FnMut(ActionEvent),
    ) -> Result<(), InputError> {
        match event {
            PhysicalEvent::Modifiers(modifiers) => self.modifiers = modifiers,
            PhysicalEvent::FocusLost => {
                for held in self.held.drain(..) {
                    if let Some((action, token)) = held.temporary {
                        emit(ActionEvent {
                            action,
                            phase: ActionPhase::End(token),
                        });
                    }
                }
                self.modifiers = Modifiers::NONE;
            }
            PhysicalEvent::Wheel { axis, steps } => {
                if !steps.is_finite() {
                    return Err(InputError::InvalidWheel);
                }
                if steps != 0.0 {
                    for binding in keymap.matching(
                        PhysicalControl::Wheel(axis),
                        self.modifiers,
                        Trigger::Wheel,
                    ) {
                        emit(ActionEvent {
                            action: binding.action,
                            phase: ActionPhase::Delta(steps),
                        });
                    }
                }
            }
            PhysicalEvent::Button {
                control,
                state: ElementState::Pressed,
                repeat,
            } => {
                if matches!(control, PhysicalControl::Wheel(_)) {
                    return Err(InputError::InvalidButton);
                }
                if repeat || self.held.iter().any(|h| h.control == control) {
                    return Ok(());
                }
                // Unassigned controls need no retained state. New assignments
                // take effect on their next press, never inventing a gesture.
                if !keymap.has_control(control) {
                    return Ok(());
                }
                if self.held.len() >= MAX_HELD_INPUTS {
                    return Err(InputError::TooManyHeldInputs);
                }
                let mut temporary = None;
                for binding in keymap.matching(control, self.modifiers, Trigger::Hold) {
                    if matches!(binding.action, Action::TemporaryTool(_)) {
                        self.next_token = self
                            .next_token
                            .checked_add(1)
                            .ok_or(InputError::TokenExhausted)?;
                        temporary = Some((binding.action, HoldToken(self.next_token)));
                    }
                }
                self.held.push(HeldInput { control, temporary });
                if let Some((action, token)) = temporary {
                    emit(ActionEvent {
                        action,
                        phase: ActionPhase::Begin(token),
                    });
                }
                for binding in keymap.matching(control, self.modifiers, Trigger::Press) {
                    emit(ActionEvent {
                        action: binding.action,
                        phase: ActionPhase::Invoke,
                    });
                }
            }
            PhysicalEvent::Button {
                control,
                state: ElementState::Released,
                ..
            } => {
                if matches!(control, PhysicalControl::Wheel(_)) {
                    return Err(InputError::InvalidButton);
                }
                if let Some(index) = self.held.iter().position(|h| h.control == control) {
                    let held = self.held.remove(index);
                    if let Some((action, token)) = held.temporary {
                        emit(ActionEvent {
                            action,
                            phase: ActionPhase::End(token),
                        });
                    }
                    for binding in keymap.matching(control, self.modifiers, Trigger::Release) {
                        emit(ActionEvent {
                            action: binding.action,
                            phase: ActionPhase::Invoke,
                        });
                    }
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputError {
    TooManyHeldInputs,
    InvalidWheel,
    InvalidButton,
    TokenExhausted,
}
impl fmt::Display for InputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "input rejected: {self:?}")
    }
}
impl Error for InputError {}
