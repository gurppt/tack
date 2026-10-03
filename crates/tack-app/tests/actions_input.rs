use tack_app::{actions::*, bindings::*, input::*};
use tack_core::*;
use winit::{
    event::{ElementState, MouseButton},
    keyboard::{KeyCode, PhysicalKey},
};
type TestResult = Result<(), Box<dyn std::error::Error>>;
fn key(code: KeyCode) -> PhysicalControl {
    PhysicalControl::Key(PhysicalKey::Code(code))
}
fn button(control: PhysicalControl, state: ElementState) -> PhysicalEvent {
    PhysicalEvent::Button {
        control,
        state,
        repeat: false,
    }
}
fn bind(
    map: &mut Keymap,
    control: PhysicalControl,
    trigger: Trigger,
    action: Action,
) -> Result<(), BindingError> {
    map.bind(Binding {
        control,
        modifiers: ModifierMatch::Exact(Modifiers::NONE),
        trigger,
        action,
    })
}

#[test]
fn multiple_bindings_unassignment_modifiers_and_conflicts() -> TestResult {
    let mut map = Keymap::default();
    assert!(map.for_action(Action::Undo).next().is_none());
    bind(&mut map, key(KeyCode::KeyU), Trigger::Press, Action::Undo)?;
    bind(
        &mut map,
        PhysicalControl::Pointer(PointerButton::Mouse(MouseButton::Other(12))),
        Trigger::Press,
        Action::Undo,
    )?;
    assert_eq!(map.for_action(Action::Undo).count(), 2);
    let mut state = InputState::default();
    let mut emitted = Vec::new();
    state.handle(
        button(key(KeyCode::KeyU), ElementState::Pressed),
        &map,
        |e| emitted.push(e),
    )?;
    state.handle(
        button(key(KeyCode::KeyU), ElementState::Released),
        &map,
        |e| emitted.push(e),
    )?;
    state.handle(PhysicalEvent::Modifiers(Modifiers::CONTROL), &map, |_| {})?;
    state.handle(
        button(key(KeyCode::KeyU), ElementState::Pressed),
        &map,
        |e| emitted.push(e),
    )?;
    assert_eq!(
        emitted,
        vec![ActionEvent {
            action: Action::Undo,
            phase: ActionPhase::Invoke
        }]
    );
    state.handle(PhysicalEvent::Modifiers(Modifiers::NONE), &map, |_| {})?;
    state.handle(
        button(
            PhysicalControl::Pointer(PointerButton::Mouse(MouseButton::Other(12))),
            ElementState::Pressed,
        ),
        &map,
        |e| emitted.push(e),
    )?;
    assert_eq!(emitted.len(), 2);
    assert!(emitted.iter().all(|e| *e
        == ActionEvent {
            action: Action::Undo,
            phase: ActionPhase::Invoke
        }));
    assert!(matches!(
        map.bind(Binding {
            control: key(KeyCode::KeyU),
            modifiers: ModifierMatch::Any,
            trigger: Trigger::Press,
            action: Action::Redo
        }),
        Err(BindingError::Conflict { .. })
    ));
    map.bind(Binding {
        control: key(KeyCode::KeyU),
        modifiers: ModifierMatch::Exact(Modifiers::CONTROL),
        trigger: Trigger::Press,
        action: Action::Redo,
    })?;
    map.unassign(Action::Undo);
    assert_eq!(map.for_action(Action::Undo).count(), 0);
    assert_eq!(map.for_action(Action::Redo).count(), 1);
    assert!(bind(&mut map, key(KeyCode::KeyA), Trigger::Hold, Action::Undo).is_err());
    Ok(())
}

#[test]
fn temporary_rotate_overlap_repeat_release_and_focus_restore_without_document_edits() -> TestResult
{
    let mut map = Keymap::default();
    // Demonstration bindings, deliberately not a shipped/default key preset.
    bind(
        &mut map,
        key(KeyCode::KeyR),
        Trigger::Hold,
        Action::TemporaryTool(Tool::RotateView),
    )?;
    bind(
        &mut map,
        key(KeyCode::Space),
        Trigger::Hold,
        Action::TemporaryTool(Tool::Pan),
    )?;
    let mut state = InputState::default();
    let mut interaction = Interaction::default();
    let mut events = Vec::new();
    interaction.apply(ActionEvent {
        action: Action::SelectTool(Tool::Pointer),
        phase: ActionPhase::Invoke,
    })?;
    state.handle(
        button(key(KeyCode::KeyR), ElementState::Pressed),
        &map,
        |e| events.push(e),
    )?;
    for event in events.drain(..) {
        interaction.apply(event)?;
    }
    assert_eq!(interaction.tool(), Tool::RotateView);
    state.handle(
        button(key(KeyCode::KeyR), ElementState::Released),
        &map,
        |e| events.push(e),
    )?;
    for event in events.drain(..) {
        interaction.apply(event)?;
    }
    assert_eq!(interaction.tool(), Tool::Pointer);
    state.handle(
        button(key(KeyCode::KeyR), ElementState::Pressed),
        &map,
        |e| events.push(e),
    )?;
    for event in events.drain(..) {
        interaction.apply(event)?;
    }
    state.handle(
        PhysicalEvent::Button {
            control: key(KeyCode::KeyR),
            state: ElementState::Pressed,
            repeat: true,
        },
        &map,
        |e| events.push(e),
    )?;
    assert!(events.is_empty());
    state.handle(
        button(key(KeyCode::Space), ElementState::Pressed),
        &map,
        |e| events.push(e),
    )?;
    for event in events.drain(..) {
        interaction.apply(event)?;
    }
    assert_eq!(interaction.tool(), Tool::Pan);
    // Release R out of stack order, after modifiers and keymap were changed.
    state.handle(PhysicalEvent::Modifiers(Modifiers::ALT), &map, |_| {})?;
    map.unassign(Action::TemporaryTool(Tool::RotateView));
    assert!(state.is_action_held(&map, Action::TemporaryTool(Tool::RotateView)));
    state.handle(
        button(key(KeyCode::KeyR), ElementState::Released),
        &map,
        |e| events.push(e),
    )?;
    for event in events.drain(..) {
        interaction.apply(event)?;
    }
    assert_eq!(interaction.tool(), Tool::Pan);
    assert!(!state.is_action_held(&map, Action::TemporaryTool(Tool::RotateView)));
    interaction.apply(ActionEvent {
        action: Action::SelectTool(Tool::RotateView),
        phase: ActionPhase::Invoke,
    })?;
    state.handle(PhysicalEvent::FocusLost, &map, |e| events.push(e))?;
    for event in events.drain(..) {
        interaction.apply(event)?;
    }
    assert_eq!(interaction.tool(), Tool::RotateView);
    assert_eq!(state.held_len(), 0);
    // Fresh Space hold/release restores the selected base exactly.
    state.handle(
        button(key(KeyCode::Space), ElementState::Pressed),
        &map,
        |e| events.push(e),
    )?;
    for event in events.drain(..) {
        interaction.apply(event)?;
    }
    state.handle(
        button(key(KeyCode::Space), ElementState::Released),
        &map,
        |e| events.push(e),
    )?;
    for event in events.drain(..) {
        interaction.apply(event)?;
    }
    assert_eq!(interaction.tool(), Tool::RotateView);
    Ok(())
}

#[test]
fn explicit_release_wheel_axes_and_invalid_input() -> TestResult {
    let mut map = Keymap::default();
    let stylus = PhysicalControl::Pointer(PointerButton::Stylus(9));
    bind(
        &mut map,
        stylus,
        Trigger::Release,
        Action::SelectTool(Tool::Pointer),
    )?;
    bind(
        &mut map,
        PhysicalControl::Wheel(WheelAxis::Horizontal),
        Trigger::Wheel,
        Action::ZoomView,
    )?;
    let mut state = InputState::default();
    let mut events = Vec::new();
    state.handle(button(stylus, ElementState::Pressed), &map, |e| {
        events.push(e)
    })?;
    assert!(events.is_empty());
    state.handle(button(stylus, ElementState::Released), &map, |e| {
        events.push(e)
    })?;
    state.handle(
        PhysicalEvent::Wheel {
            axis: WheelAxis::Horizontal,
            steps: 0.5,
        },
        &map,
        |e| events.push(e),
    )?;
    assert_eq!(
        events,
        vec![
            ActionEvent {
                action: Action::SelectTool(Tool::Pointer),
                phase: ActionPhase::Invoke
            },
            ActionEvent {
                action: Action::ZoomView,
                phase: ActionPhase::Delta(0.5)
            }
        ]
    );
    assert_eq!(
        state.handle(
            PhysicalEvent::Wheel {
                axis: WheelAxis::Vertical,
                steps: f64::NAN
            },
            &map,
            |_| {}
        ),
        Err(InputError::InvalidWheel)
    );
    assert_eq!(
        state.handle(
            button(
                PhysicalControl::Wheel(WheelAxis::Horizontal),
                ElementState::Pressed
            ),
            &map,
            |_| {}
        ),
        Err(InputError::InvalidButton)
    );
    Ok(())
}

#[test]
fn input_and_keymap_growth_are_bounded() -> TestResult {
    let mut map = Keymap::default();
    let mut state = InputState::default();
    for number in 0..MAX_HELD_INPUTS {
        bind(
            &mut map,
            PhysicalControl::Pointer(PointerButton::Stylus(number as u16)),
            Trigger::Press,
            Action::Undo,
        )?;
        state.handle(
            button(
                PhysicalControl::Pointer(PointerButton::Stylus(number as u16)),
                ElementState::Pressed,
            ),
            &map,
            |_| {},
        )?;
    }
    bind(
        &mut map,
        PhysicalControl::Pointer(PointerButton::Stylus(500)),
        Trigger::Press,
        Action::Undo,
    )?;
    assert_eq!(
        state.handle(
            button(
                PhysicalControl::Pointer(PointerButton::Stylus(500)),
                ElementState::Pressed
            ),
            &map,
            |_| {}
        ),
        Err(InputError::TooManyHeldInputs)
    );
    assert_eq!(state.held_len(), MAX_HELD_INPUTS);
    state.handle(PhysicalEvent::FocusLost, &map, |_| {})?;
    assert_eq!(state.held_len(), 0);
    let mut map = Keymap::default();
    for number in 0..MAX_BINDINGS {
        bind(
            &mut map,
            PhysicalControl::Pointer(PointerButton::Stylus(number as u16)),
            Trigger::Press,
            Action::Undo,
        )?;
    }
    assert_eq!(
        bind(
            &mut map,
            PhysicalControl::Pointer(PointerButton::Stylus(500)),
            Trigger::Press,
            Action::Undo
        ),
        Err(BindingError::Full)
    );
    Ok(())
}

#[test]
fn menu_and_binding_dispatch_the_same_document_action() -> TestResult {
    let mut document = Document::new(DocumentId::new(1)?, DocumentLimits::default());
    document.apply(Command::AddSource(Source::embedded(SourceId::new(1)?)))?;
    let mut direct = DocumentEditor::new(document.clone(), 4);
    let mut via_binding = DocumentEditor::new(document, 4);
    for editor in [&mut direct, &mut via_binding] {
        editor.execute(Command::RemoveSource(SourceId::new(1)?))?;
    }
    dispatch_document_action(
        ActionEvent {
            action: Action::Undo,
            phase: ActionPhase::Invoke,
        },
        &mut direct,
    )?;
    let mut map = Keymap::default();
    bind(&mut map, key(KeyCode::KeyU), Trigger::Press, Action::Undo)?;
    let mut state = InputState::default();
    let mut event = None;
    state.handle(
        button(key(KeyCode::KeyU), ElementState::Pressed),
        &map,
        |e| event = Some(e),
    )?;
    dispatch_document_action(event.ok_or("unresolved action")?, &mut via_binding)?;
    assert_eq!(direct.document(), via_binding.document());
    dispatch_document_action(
        ActionEvent {
            action: Action::Redo,
            phase: ActionPhase::Invoke,
        },
        &mut via_binding,
    )?;
    assert!(via_binding.document().source(SourceId::new(1)?).is_none());
    assert!(
        dispatch_document_action(
            ActionEvent {
                action: Action::ZoomView,
                phase: ActionPhase::Invoke
            },
            &mut direct
        )
        .is_err()
    );
    Ok(())
}
