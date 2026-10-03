//! Thin view-only adapter for the existing benchmark window. No final key preset.
use crate::{
    actions::{Action, ActionEvent, ActionPhase},
    bindings::{Binding, BindingError, Keymap, ModifierMatch, Trigger},
    input::{
        InputError, InputState, Modifiers, PhysicalControl, PhysicalEvent, PointerButton, WheelAxis,
    },
};
use tack_core::{Camera, GeometryError};
use winit::event::{MouseButton, WindowEvent};

fn pointer(button: MouseButton) -> PhysicalControl {
    PhysicalControl::Pointer(PointerButton::Mouse(button))
}

pub struct NavigationInput {
    cursor: [f64; 2],
    state: InputState,
    keymap: Keymap,
}
impl NavigationInput {
    /// Retain only the already-characterized prototype navigation mappings.
    pub fn prototype() -> Result<Self, BindingError> {
        let mut keymap = Keymap::default();
        for binding in [
            Binding {
                control: pointer(MouseButton::Middle),
                modifiers: ModifierMatch::Any,
                trigger: Trigger::Hold,
                action: Action::PanView,
            },
            Binding {
                control: pointer(MouseButton::Left),
                modifiers: ModifierMatch::Contains(Modifiers::ALT),
                trigger: Trigger::Hold,
                action: Action::PanView,
            },
            Binding {
                control: PhysicalControl::Wheel(WheelAxis::Vertical),
                modifiers: ModifierMatch::Any,
                trigger: Trigger::Wheel,
                action: Action::ZoomView,
            },
        ] {
            keymap.bind(binding)?;
        }
        Ok(Self {
            cursor: [0.0; 2],
            state: InputState::default(),
            keymap,
        })
    }
    pub fn physical(&mut self, event: PhysicalEvent) -> Result<Option<ActionEvent>, InputError> {
        let mut action = None;
        self.state
            .handle(event, &self.keymap, |event| action = Some(event))?;
        Ok(action)
    }
    pub fn cursor_moved(&mut self, next: [f64; 2]) -> Option<[f64; 2]> {
        if !next.into_iter().all(f64::is_finite) {
            return None;
        }
        let delta = [next[0] - self.cursor[0], next[1] - self.cursor[1]];
        self.cursor = next;
        self.state
            .is_action_held(&self.keymap, Action::PanView)
            .then_some(delta)
    }
    pub fn zoom(&self, camera: &mut Camera, event: ActionEvent) -> Result<bool, GeometryError> {
        if let ActionEvent {
            action: Action::ZoomView,
            phase: ActionPhase::Delta(steps),
        } = event
        {
            camera.zoom_at(self.cursor, (steps.clamp(-20.0, 20.0) * 0.15).exp())?;
            return Ok(true);
        }
        Ok(false)
    }
    pub fn handle(&mut self, event: &WindowEvent, camera: &mut Camera, interactive: bool) -> bool {
        let mut changed = false;
        for physical in crate::input::normalize(event).into_iter().flatten() {
            // Invalid/excess physical input is dropped, never a reason to close
            // a document/window. Existing holds still release or lose focus.
            if let Ok(Some(action)) = self.physical(physical)
                && interactive
            {
                changed |= self.zoom(camera, action).unwrap_or(false);
            }
        }
        if let WindowEvent::CursorMoved { position, .. } = event
            && let Some(delta) = self.cursor_moved([position.x, position.y])
            && interactive
        {
            changed |= camera.pan(delta).is_ok();
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tack_core::Camera;
    use winit::event::{ElementState, MouseScrollDelta};
    use winit::keyboard::ModifiersState;
    #[test]
    fn late_alt_release_and_other_modifiers_preserve_navigation_contract()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut input = NavigationInput::prototype()?;
        input.physical(PhysicalEvent::Button {
            control: pointer(MouseButton::Left),
            state: ElementState::Pressed,
            repeat: false,
        })?;
        assert_eq!(input.cursor_moved([10.0, 5.0]), None);
        input.physical(PhysicalEvent::Modifiers(
            (ModifiersState::ALT | ModifiersState::CONTROL).into(),
        ))?;
        assert_eq!(input.cursor_moved([13.0, 8.0]), Some([3.0, 3.0]));
        input.physical(PhysicalEvent::Modifiers(Modifiers::NONE))?;
        assert_eq!(input.cursor_moved([20.0, 10.0]), None);
        input.physical(PhysicalEvent::Button {
            control: pointer(MouseButton::Middle),
            state: ElementState::Pressed,
            repeat: false,
        })?;
        input.physical(PhysicalEvent::Modifiers(
            (ModifiersState::SHIFT | ModifiersState::SUPER).into(),
        ))?;
        assert_eq!(input.cursor_moved([22.0, 12.0]), Some([2.0, 2.0]));
        input.physical(PhysicalEvent::FocusLost)?;
        assert_eq!(input.cursor_moved([25.0, 15.0]), None);
        input.physical(PhysicalEvent::Button {
            control: pointer(MouseButton::Left),
            state: ElementState::Pressed,
            repeat: false,
        })?;
        assert_eq!(input.cursor_moved([30.0, 20.0]), None);
        Ok(())
    }
    #[test]
    fn wheel_units_clamping_and_cursor_anchor() -> Result<(), Box<dyn std::error::Error>> {
        let line = crate::input::wheel_steps(MouseScrollDelta::LineDelta(99.0, 2.0))[1];
        let pixel = crate::input::wheel_steps(MouseScrollDelta::PixelDelta(
            winit::dpi::PhysicalPosition::new(99.0, 200.0),
        ))[1];
        assert_eq!(line, pixel);
        let mut camera = Camera::new([1280, 720]);
        let cursor = [123.0, 456.0];
        let mut input = NavigationInput::prototype()?;
        input.cursor_moved(cursor);
        let before = camera.screen_to_world(cursor);
        for steps in [line, 200.0, -200.0] {
            let event = input
                .physical(PhysicalEvent::Wheel {
                    axis: WheelAxis::Vertical,
                    steps,
                })?
                .ok_or("unresolved wheel")?;
            assert!(input.zoom(&mut camera, event)?);
            let after = camera.screen_to_world(cursor);
            assert!((before[0] - after[0]).abs() < 1e-7);
            assert!((before[1] - after[1]).abs() < 1e-7);
        }
        Ok(())
    }

    #[test]
    fn native_event_normalization_and_scripted_isolation()
    -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        use winit::event::{DeviceId, TouchPhase};
        let mut camera = Camera::new([1280, 720]);
        let initial = camera.viewport();
        let mut input = NavigationInput::prototype()?;
        let mouse = WindowEvent::MouseInput {
            device_id: DeviceId::dummy(),
            state: ElementState::Pressed,
            button: MouseButton::Middle,
        };
        let moved = WindowEvent::CursorMoved {
            device_id: DeviceId::dummy(),
            position: winit::dpi::PhysicalPosition::new(100.0, 100.0),
        };
        let wheel = WindowEvent::MouseWheel {
            device_id: DeviceId::dummy(),
            delta: MouseScrollDelta::LineDelta(0.0, 2.0),
            phase: TouchPhase::Moved,
        };
        assert!(!input.handle(&mouse, &mut camera, false));
        assert!(!input.handle(&moved, &mut camera, false));
        assert!(!input.handle(&wheel, &mut camera, false));
        assert_eq!(camera.viewport(), initial);
        assert!(input.handle(&wheel, &mut camera, true));
        assert_ne!(camera.viewport(), initial);
        input.handle(&WindowEvent::Focused(false), &mut camera, true);
        let before = camera.viewport();
        assert!(!input.handle(&moved, &mut camera, true));
        assert_eq!(camera.viewport(), before);
        Ok(())
    }

    #[test]
    fn excessive_unassigned_and_malformed_native_input_are_non_fatal()
    -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        use winit::event::{DeviceId, TouchPhase};
        let mut camera = Camera::new([1280, 720]);
        let initial = camera.viewport();
        let mut input = NavigationInput::prototype()?;
        for number in 0..1000 {
            let event = WindowEvent::MouseInput {
                device_id: DeviceId::dummy(),
                state: ElementState::Pressed,
                button: MouseButton::Other(number),
            };
            assert!(!input.handle(&event, &mut camera, number % 2 == 0));
        }
        assert_eq!(input.state.held_len(), 0);
        let invalid = WindowEvent::MouseWheel {
            device_id: DeviceId::dummy(),
            delta: MouseScrollDelta::PixelDelta(winit::dpi::PhysicalPosition::new(0.0, f64::NAN)),
            phase: TouchPhase::Moved,
        };
        assert!(!input.handle(&invalid, &mut camera, true));
        assert_eq!(camera.viewport(), initial);
        assert_eq!(input.cursor_moved([f64::NAN, 0.0]), None);
        input.physical(PhysicalEvent::Button {
            control: pointer(MouseButton::Middle),
            state: ElementState::Pressed,
            repeat: false,
        })?;
        assert_eq!(input.cursor_moved([10.0, 20.0]), Some([10.0, 20.0]));
        input.handle(&WindowEvent::Focused(false), &mut camera, true);
        assert_eq!(input.cursor_moved([30.0, 40.0]), None);
        Ok(())
    }
}
