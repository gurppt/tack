//! winit normalization and bounded physical state. Tools see only ActionEvent.
use crate::{
    actions::{Action, ActionEvent, ActionPhase, HoldToken},
    bindings::{Keymap, Trigger},
};
use std::{error::Error, fmt};
use winit::{
    event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent},
    keyboard::{Key, KeyCode, ModifiersState, NamedKey, PhysicalKey},
};

pub const MAX_HELD_INPUTS: usize = 32;
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PointerButton {
    Mouse(MouseButton),
    Stylus(u16),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum WheelAxis {
    Horizontal,
    Vertical,
}
/// Layout-aware base key, without modifiers; fixed-size and serializable.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LogicalKey {
    Character(char),
    Named(NamedKey),
}
impl LogicalKey {
    pub fn from_key(key: &Key) -> Option<Self> {
        match key {
            Key::Character(text) => {
                let mut chars = text.chars();
                let c = chars.next()?;
                chars
                    .next()
                    .is_none()
                    .then_some(Self::Character(c.to_ascii_lowercase()))
            }
            Key::Named(name) => Some(Self::Named(*name)),
            _ => None,
        }
    }
    /// Version-1 profiles displayed US key names. Preserve those advertised
    /// names as logical bindings when migrating, including custom assignments.
    pub fn from_legacy(code: KeyCode) -> Option<Self> {
        Some(match code {
            KeyCode::KeyA => Self::Character('a'),
            KeyCode::KeyB => Self::Character('b'),
            KeyCode::KeyC => Self::Character('c'),
            KeyCode::KeyD => Self::Character('d'),
            KeyCode::KeyE => Self::Character('e'),
            KeyCode::KeyF => Self::Character('f'),
            KeyCode::KeyG => Self::Character('g'),
            KeyCode::KeyH => Self::Character('h'),
            KeyCode::KeyI => Self::Character('i'),
            KeyCode::KeyJ => Self::Character('j'),
            KeyCode::KeyK => Self::Character('k'),
            KeyCode::KeyL => Self::Character('l'),
            KeyCode::KeyM => Self::Character('m'),
            KeyCode::KeyN => Self::Character('n'),
            KeyCode::KeyO => Self::Character('o'),
            KeyCode::KeyP => Self::Character('p'),
            KeyCode::KeyQ => Self::Character('q'),
            KeyCode::KeyR => Self::Character('r'),
            KeyCode::KeyS => Self::Character('s'),
            KeyCode::KeyT => Self::Character('t'),
            KeyCode::KeyU => Self::Character('u'),
            KeyCode::KeyV => Self::Character('v'),
            KeyCode::KeyW => Self::Character('w'),
            KeyCode::KeyX => Self::Character('x'),
            KeyCode::KeyY => Self::Character('y'),
            KeyCode::KeyZ => Self::Character('z'),
            KeyCode::Digit0 => Self::Character('0'),
            KeyCode::Digit1 => Self::Character('1'),
            KeyCode::Digit2 => Self::Character('2'),
            KeyCode::Digit3 => Self::Character('3'),
            KeyCode::Digit4 => Self::Character('4'),
            KeyCode::Digit5 => Self::Character('5'),
            KeyCode::Digit6 => Self::Character('6'),
            KeyCode::Digit7 => Self::Character('7'),
            KeyCode::Digit8 => Self::Character('8'),
            KeyCode::Digit9 => Self::Character('9'),
            KeyCode::Comma => Self::Character(','),
            KeyCode::Period => Self::Character('.'),
            KeyCode::BracketLeft => Self::Character('['),
            KeyCode::BracketRight => Self::Character(']'),
            KeyCode::Slash => Self::Character('/'),
            KeyCode::Backslash => Self::Character('\\'),
            KeyCode::Minus => Self::Character('-'),
            KeyCode::Equal => Self::Character('='),
            KeyCode::Semicolon => Self::Character(';'),
            KeyCode::Quote => Self::Character('\''),
            KeyCode::Backquote => Self::Character('`'),
            KeyCode::Space => Self::Named(NamedKey::Space),
            KeyCode::Escape => Self::Named(NamedKey::Escape),
            KeyCode::Enter => Self::Named(NamedKey::Enter),
            KeyCode::Tab => Self::Named(NamedKey::Tab),
            KeyCode::Backspace => Self::Named(NamedKey::Backspace),
            KeyCode::Delete => Self::Named(NamedKey::Delete),
            KeyCode::Insert => Self::Named(NamedKey::Insert),
            KeyCode::Home => Self::Named(NamedKey::Home),
            KeyCode::End => Self::Named(NamedKey::End),
            KeyCode::PageUp => Self::Named(NamedKey::PageUp),
            KeyCode::PageDown => Self::Named(NamedKey::PageDown),
            KeyCode::ArrowLeft => Self::Named(NamedKey::ArrowLeft),
            KeyCode::ArrowRight => Self::Named(NamedKey::ArrowRight),
            KeyCode::ArrowUp => Self::Named(NamedKey::ArrowUp),
            KeyCode::ArrowDown => Self::Named(NamedKey::ArrowDown),
            KeyCode::F1 => Self::Named(NamedKey::F1),
            KeyCode::F2 => Self::Named(NamedKey::F2),
            KeyCode::F3 => Self::Named(NamedKey::F3),
            KeyCode::F4 => Self::Named(NamedKey::F4),
            KeyCode::F5 => Self::Named(NamedKey::F5),
            KeyCode::F6 => Self::Named(NamedKey::F6),
            KeyCode::F7 => Self::Named(NamedKey::F7),
            KeyCode::F8 => Self::Named(NamedKey::F8),
            KeyCode::F9 => Self::Named(NamedKey::F9),
            KeyCode::F10 => Self::Named(NamedKey::F10),
            KeyCode::F11 => Self::Named(NamedKey::F11),
            KeyCode::F12 => Self::Named(NamedKey::F12),
            KeyCode::F13 => Self::Named(NamedKey::F13),
            KeyCode::F14 => Self::Named(NamedKey::F14),
            KeyCode::F15 => Self::Named(NamedKey::F15),
            KeyCode::F16 => Self::Named(NamedKey::F16),
            KeyCode::F17 => Self::Named(NamedKey::F17),
            KeyCode::F18 => Self::Named(NamedKey::F18),
            KeyCode::F19 => Self::Named(NamedKey::F19),
            KeyCode::F20 => Self::Named(NamedKey::F20),
            KeyCode::F21 => Self::Named(NamedKey::F21),
            KeyCode::F22 => Self::Named(NamedKey::F22),
            KeyCode::F23 => Self::Named(NamedKey::F23),
            KeyCode::F24 => Self::Named(NamedKey::F24),
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PhysicalControl {
    Key(PhysicalKey),
    LogicalKey(LogicalKey),
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
    pub fn mask(self) -> u8 {
        self.0
    }
    pub fn checked_mask(mask: u8) -> Option<Self> {
        (mask < 16).then_some(Self(mask))
    }
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
    Keyboard {
        physical: PhysicalKey,
        logical: Option<LogicalKey>,
        state: ElementState,
        repeat: bool,
    },
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
        WindowEvent::KeyboardInput { event, .. } => Some(PhysicalEvent::Keyboard {
            physical: event.physical_key,
            logical: logical_key(event),
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
    identity: PhysicalControl,
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
    pub fn modifiers(&self) -> Modifiers {
        self.modifiers
    }
    pub fn held_len(&self) -> usize {
        self.held.len()
    }
    pub fn is_action_held(&self, keymap: &Keymap, action: Action) -> bool {
        if action.captured_hold() && action != Action::CenterPointer {
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
        let (identity, event) = if let PhysicalEvent::Keyboard {
            physical,
            logical,
            state,
            repeat,
        } = event
        {
            let identity = PhysicalControl::Key(physical);
            // An alias key never releases another physical key's captured edge.
            if state == ElementState::Released && !self.held.iter().any(|h| h.identity == identity)
            {
                return Ok(());
            }
            let control = self
                .held
                .iter()
                .find(|h| h.identity == identity)
                .map_or_else(
                    || keymap.keyboard_control(physical, logical, self.modifiers),
                    |h| h.control,
                );
            let assign = (state == ElementState::Pressed
                && !self.held.iter().any(|h| h.control == control))
            .then_some((identity, control));
            (
                assign,
                PhysicalEvent::Button {
                    control,
                    state,
                    repeat,
                },
            )
        } else {
            (None, event)
        };
        match event {
            PhysicalEvent::Keyboard { .. } => return Err(InputError::InvalidButton),
            PhysicalEvent::Modifiers(modifiers) => self.modifiers = modifiers,
            PhysicalEvent::FocusLost => {
                for held in self.held.drain(..) {
                    if let Some((action, token)) = held.temporary {
                        emit(ActionEvent {
                            action,
                            phase: if matches!(action, Action::TemporaryTool(_)) {
                                ActionPhase::End(token)
                            } else {
                                ActionPhase::Cancel(token)
                            },
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
                    if binding.action.captured_hold() || binding.action == Action::PanView {
                        self.next_token = self
                            .next_token
                            .checked_add(1)
                            .ok_or(InputError::TokenExhausted)?;
                        temporary = Some((binding.action, HoldToken(self.next_token)));
                    }
                }
                self.held.push(HeldInput {
                    identity: control,
                    control,
                    temporary,
                });
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
        if let Some((identity, control)) = identity
            && let Some(held) = self.held.iter_mut().find(|h| h.control == control)
        {
            held.identity = identity;
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

/// winit's desktop base-key value respects the active layout while removing
/// Shift/Caps/Control. The logical event value remains available in diagnostics.
pub fn logical_key(event: &winit::event::KeyEvent) -> Option<LogicalKey> {
    #[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
    {
        use winit::platform::modifier_supplement::KeyEventExtModifierSupplement;
        LogicalKey::from_key(&event.key_without_modifiers())
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        LogicalKey::from_key(&event.logical_key)
    }
}
