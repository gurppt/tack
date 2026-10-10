#![allow(clippy::unwrap_used)]
use tack_app::{
    actions::*,
    bindings::*,
    image_input::ImageInput,
    input::*,
    spatial_layout::{self, Layout},
};
use tack_core::*;
use winit::{
    event::ElementState,
    keyboard::{KeyCode, PhysicalKey},
};
type R<T = ()> = Result<T, Box<dyn std::error::Error + Send + Sync>>;
fn fixture(n: usize) -> R<DocumentEditor> {
    let mut d = Document::new(DocumentId::new(1)?, DocumentLimits::default());
    d.apply(Command::AddSource(Source::linked(
        SourceId::new(1)?,
        "missing.png",
    )?))?;
    d.apply(Command::AddAsset(ImageAsset::new(
        AssetId::new(1)?,
        SourceId::new(1)?,
        [100, 100],
    )?))?;
    for i in 0..n {
        d.apply(Command::AddObject {
            object: DocumentObject::image(
                ObjectId::new(i as u128 + 1)?,
                AssetId::new(1)?,
                Transform::new(
                    [i as f64 * 23. - 80., (i % 3) as f64 * 37. - 90.],
                    if i % 2 == 0 { [100., 60.] } else { [50., 130.] },
                    if i % 3 == 0 { 0.4 } else { 0. },
                    [i % 2 == 0, false],
                )?,
            ),
            index: i,
        })?;
    }
    Ok(DocumentEditor::new(d, 200))
}
fn invoke(input: &mut ImageInput, e: &mut DocumentEditor, c: &mut Camera, a: Action) -> R {
    input.dispatch(
        ActionEvent {
            action: a,
            phase: ActionPhase::Invoke,
        },
        e,
        c,
    )?;
    Ok(())
}
fn edge(
    input: &mut ImageInput,
    e: &mut DocumentEditor,
    c: &mut Camera,
    physical: KeyCode,
    logical: char,
    state: ElementState,
    repeat: bool,
) -> R {
    input.physical(
        PhysicalEvent::Keyboard {
            physical: PhysicalKey::Code(physical),
            logical: Some(LogicalKey::Character(logical)),
            state,
            repeat,
        },
        e,
        c,
    )?;
    Ok(())
}
fn chord(
    input: &mut ImageInput,
    e: &mut DocumentEditor,
    c: &mut Camera,
    physical: KeyCode,
    logical: char,
    mods: Modifiers,
) -> R {
    input.physical(PhysicalEvent::Modifiers(mods), e, c)?;
    edge(input, e, c, physical, logical, ElementState::Pressed, false)?;
    // Modifier release before key release, with a changed logical letter: physical
    // identity must still end the original hold and suppress duplicate presses.
    input.physical(PhysicalEvent::Modifiers(Modifiers::NONE), e, c)?;
    edge(input, e, c, physical, 'x', ElementState::Released, false)
}
#[test]
fn azerty_keyboard_reaches_owner_history_and_wrong_us_position_does_not() -> R {
    let mut e = fixture(1)?;
    let before = e.document().clone();
    let mut input = ImageInput::new()?;
    let mut c = Camera::new([800, 600]);
    e.execute(Command::SetTransform {
        object: ObjectId::new(1)?,
        transform: Transform::new([200., 100.], [100., 60.], 0.4, [true, false])?,
    })?;
    let moved = e.document().clone();
    chord(
        &mut input,
        &mut e,
        &mut c,
        KeyCode::KeyZ,
        'w',
        Modifiers::CONTROL,
    )?;
    assert_eq!(e.document(), &moved);
    chord(
        &mut input,
        &mut e,
        &mut c,
        KeyCode::KeyW,
        'z',
        Modifiers::CONTROL,
    )?;
    assert_eq!(e.document(), &before);
    assert_eq!(e.redo_len(), 1);
    chord(
        &mut input,
        &mut e,
        &mut c,
        KeyCode::KeyW,
        'z',
        Modifiers::CONTROL.union(Modifiers::SHIFT),
    )?;
    assert_eq!(e.document(), &moved);
    chord(
        &mut input,
        &mut e,
        &mut c,
        KeyCode::KeyW,
        'z',
        Modifiers::CONTROL,
    )?;
    chord(
        &mut input,
        &mut e,
        &mut c,
        KeyCode::KeyY,
        'y',
        Modifiers::CONTROL,
    )?;
    assert_eq!(e.document(), &moved);
    input.images.selection.clear();
    chord(
        &mut input,
        &mut e,
        &mut c,
        KeyCode::KeyQ,
        'a',
        Modifiers::CONTROL,
    )?;
    assert_eq!(input.images.selection.len(), 1);
    assert_eq!(
        tack_app::context_menu::shortcut(&input.keymap, Action::Undo),
        "Ctrl+Z"
    );
    input.physical(PhysicalEvent::Modifiers(Modifiers::CONTROL), &mut e, &mut c)?;
    edge(
        &mut input,
        &mut e,
        &mut c,
        KeyCode::KeyW,
        'z',
        ElementState::Pressed,
        false,
    )?;
    edge(
        &mut input,
        &mut e,
        &mut c,
        KeyCode::KeyW,
        'z',
        ElementState::Pressed,
        true,
    )?;
    assert_eq!(e.document(), &before);
    input.physical(PhysicalEvent::FocusLost, &mut e, &mut c)?;
    edge(
        &mut input,
        &mut e,
        &mut c,
        KeyCode::KeyW,
        'z',
        ElementState::Pressed,
        false,
    )?;
    assert_eq!(e.document(), &before);
    Ok(())
}
#[test]
fn remap_conflicts_and_version_one_migration_are_explicit() -> R {
    let mut profile = tack_app::preferences::Preferences::defaults()?;
    profile.version = 1;
    for b in &mut profile.keymap {
        if let PhysicalControl::LogicalKey(k) = b.control {
            let code = match k {
                LogicalKey::Character('z') => Some(KeyCode::KeyZ),
                LogicalKey::Character('a') => Some(KeyCode::KeyA),
                _ => None,
            };
            if let Some(code) = code {
                b.control = PhysicalControl::Key(PhysicalKey::Code(code));
            }
        }
    }
    let mut map = profile.keymap()?;
    assert_eq!(
        tack_app::context_menu::shortcut(&map, Action::Undo),
        "Ctrl+Z"
    );
    let binding = Binding {
        control: PhysicalControl::LogicalKey(LogicalKey::Character('z')),
        modifiers: ModifierMatch::Exact(Modifiers::CONTROL),
        trigger: Trigger::Press,
        action: Action::Save,
    };
    assert!(map.bind(binding).is_err());
    assert!(
        map.bind(Binding {
            control: PhysicalControl::Key(PhysicalKey::Code(KeyCode::KeyW)),
            ..binding
        })
        .is_err()
    );
    map.unassign(Action::Undo);
    map.bind(Binding {
        control: PhysicalControl::LogicalKey(LogicalKey::Character('u')),
        action: Action::Undo,
        ..binding
    })?;
    let mut e = fixture(1)?;
    let before = e.document().clone();
    e.execute(Command::SetTransform {
        object: ObjectId::new(1)?,
        transform: Transform::new([90., 80.], [100., 60.], 0.4, [true, false])?,
    })?;
    let mut input = ImageInput::new()?;
    input.keymap = map;
    let mut c = Camera::new([800, 600]);
    chord(
        &mut input,
        &mut e,
        &mut c,
        KeyCode::KeyU,
        'u',
        Modifiers::CONTROL,
    )?;
    assert_eq!(e.document(), &before);
    Ok(())
}
#[test]
fn mixed_grid_groups_determinism_no_overlap_atomic_history_and_save() -> R {
    for n in [2, 3, 4, 5, 10, 100] {
        let mut e = fixture(n)?;
        if n >= 4 {
            e.execute(Command::AddGroup(Group::new(
                GroupId::new(1)?,
                vec![ObjectId::new(1)?, ObjectId::new(2)?],
            )?))?;
        }
        let before = e.document().clone();
        let ids: Vec<_> = before.object_order().to_vec();
        let a = spatial_layout::arrange(&before, ids.iter().copied(), Layout::Grid)?;
        let b = spatial_layout::arrange(&before, ids.iter().rev().copied(), Layout::Grid)?;
        assert_eq!(a, b);
        let old_history = e.undo_len();
        e.execute(a)?;
        assert_eq!(e.undo_len(), old_history + 1);
        let after = e.document().clone();
        for o in after.objects() {
            let old = before.object(o.id()).unwrap().transform();
            let t = o.transform();
            assert_eq!(t.size(), old.size());
            assert_eq!(t.rotation(), old.rotation());
            assert_eq!(t.flips(), old.flips());
        }
        let mut units = Vec::new();
        for id in &ids {
            if *id == ObjectId::new(2)? && n >= 4 {
                continue;
            }
            let members = after
                .group_for(*id)
                .map_or_else(|| vec![*id], |g| g.members().to_vec());
            units.push(
                spatial_layout::bounds(
                    members
                        .iter()
                        .map(|id| after.object(*id).unwrap().transform()),
                )
                .unwrap(),
            );
        }
        for (i, a) in units.iter().enumerate() {
            for b in &units[i + 1..] {
                assert!(
                    a.x + a.width <= b.x + 1e-9
                        || b.x + b.width <= a.x + 1e-9
                        || a.y + a.height <= b.y + 1e-9
                        || b.y + b.height <= a.y + 1e-9
                );
            }
        }
        if units.len() >= 3 {
            assert!(units.iter().any(|u| (u.x - units[0].x).abs() > 1.));
            assert!(units.iter().any(|u| (u.y - units[0].y).abs() > 1.));
        }
        if n >= 4 {
            let delta = |id| {
                let a = after.object(id).unwrap().transform().center();
                let b = before.object(id).unwrap().transform().center();
                [a[0] - b[0], a[1] - b[1]]
            };
            assert_eq!(delta(ObjectId::new(1)?), delta(ObjectId::new(2)?));
        }
        e.undo()?;
        assert_eq!(e.document(), &before);
        e.redo()?;
        assert_eq!(e.document(), &after);
        let path = std::env::temp_dir().join(format!(
            "tack-grid-{:032x}.tack",
            tack_storage::new_document_id()?.value()
        ));
        tack_storage::save(&path, &after, vec![])?;
        assert_eq!(&tack_storage::TackFile::open(&path)?.document, &after);
        std::fs::remove_file(path)?;
    }
    Ok(())
}
#[test]
fn snap_both_axes_negative_rotated_groups_single_unit_and_exact_history() -> R {
    let mut e = fixture(5)?;
    e.execute(Command::AddGroup(Group::new(
        GroupId::new(1)?,
        vec![ObjectId::new(1)?, ObjectId::new(2)?],
    )?))?;
    let before = e.document().clone();
    let ids = before.object_order().to_vec();
    let old = e.undo_len();
    e.execute(spatial_layout::arrange(
        &before,
        ids.into_iter(),
        Layout::SnapToGrid,
    )?)?;
    assert_eq!(e.undo_len(), old + 1);
    for id in [1, 3, 4, 5] {
        let id = ObjectId::new(id)?;
        let members = e
            .document()
            .group_for(id)
            .map_or_else(|| vec![id], |g| g.members().to_vec());
        let b = spatial_layout::bounds(
            members
                .iter()
                .map(|id| e.document().object(*id).unwrap().transform()),
        )
        .unwrap();
        for p in [b.x, b.y] {
            assert!((p / 64. - (p / 64.).round()).abs() < 1e-10);
        }
    }
    let after = e.document().clone();
    e.undo()?;
    assert_eq!(e.document(), &before);
    e.redo()?;
    assert_eq!(e.document(), &after);
    assert_eq!(
        spatial_layout::arrange(&after, [ObjectId::new(3)?].into_iter(), Layout::SnapToGrid)?,
        Command::Batch(vec![])
    );
    Ok(())
}
#[test]
fn creation_completion_cancel_focus_and_freehand_policy() -> R {
    use tack_app::input::{PhysicalControl, PointerButton};
    use winit::event::MouseButton;
    for tool in [Tool::Rectangle, Tool::Line, Tool::Arrow, Tool::Scribble] {
        let mut e = fixture(0)?;
        let mut input = ImageInput::new()?;
        let mut c = Camera::new([800, 600]);
        invoke(&mut input, &mut e, &mut c, Action::SelectTool(tool))?;
        input.cursor_moved([100., 100.], &e, &mut c)?;
        let button = |state| PhysicalEvent::Button {
            control: PhysicalControl::Pointer(PointerButton::Mouse(MouseButton::Left)),
            state,
            repeat: false,
        };
        input.physical(button(ElementState::Pressed), &mut e, &mut c)?;
        input.cursor_moved([180., 160.], &e, &mut c)?;
        input.physical(button(ElementState::Released), &mut e, &mut c)?;
        assert_eq!(e.undo_len(), 1);
        assert_eq!(
            input.annotation.tools.tool(),
            if tool == Tool::Scribble {
                tool
            } else {
                Tool::Pointer
            }
        );
        e.undo()?;
        assert_eq!(e.document().annotation_count(), 0);
        invoke(&mut input, &mut e, &mut c, Action::SelectTool(tool))?;
        input.physical(button(ElementState::Pressed), &mut e, &mut c)?;
        invoke(&mut input, &mut e, &mut c, Action::CancelInteraction)?;
        input.physical(button(ElementState::Released), &mut e, &mut c)?;
        assert_eq!(input.annotation.tools.tool(), Tool::Pointer);
        assert_eq!(e.undo_len(), 0);
        invoke(&mut input, &mut e, &mut c, Action::SelectTool(tool))?;
        input.physical(button(ElementState::Pressed), &mut e, &mut c)?;
        input.physical(PhysicalEvent::FocusLost, &mut e, &mut c)?;
        assert!(input.annotation.creation.is_none());
        assert_eq!(
            input.annotation.tools.tool(),
            if tool == Tool::Scribble {
                tool
            } else {
                Tool::Pointer
            }
        );
    }
    Ok(())
}

#[test]
fn physical_alias_release_never_ends_another_keys_captured_hold() -> R {
    use winit::keyboard::NamedKey;
    let mut map = Keymap::default();
    map.bind(Binding {
        control: PhysicalControl::LogicalKey(LogicalKey::Named(NamedKey::Enter)),
        modifiers: ModifierMatch::Exact(Modifiers::NONE),
        trigger: Trigger::Hold,
        action: Action::TemporaryTool(Tool::Pan),
    })?;
    let mut state = InputState::default();
    let mut emitted = Vec::new();
    let key = |physical, state| PhysicalEvent::Keyboard {
        physical: PhysicalKey::Code(physical),
        logical: Some(LogicalKey::Named(NamedKey::Enter)),
        state,
        repeat: false,
    };
    state.handle(key(KeyCode::Enter, ElementState::Pressed), &map, |e| {
        emitted.push(e)
    })?;
    state.handle(
        key(KeyCode::NumpadEnter, ElementState::Pressed),
        &map,
        |e| emitted.push(e),
    )?;
    state.handle(
        key(KeyCode::NumpadEnter, ElementState::Released),
        &map,
        |e| emitted.push(e),
    )?;
    assert_eq!(state.held_len(), 1);
    assert_eq!(emitted.len(), 1);
    state.handle(key(KeyCode::Enter, ElementState::Pressed), &map, |e| {
        emitted.push(e)
    })?;
    assert_eq!(emitted.len(), 1);
    state.handle(key(KeyCode::Enter, ElementState::Released), &map, |e| {
        emitted.push(e)
    })?;
    assert_eq!(state.held_len(), 0);
    assert_eq!(emitted.len(), 2);
    assert!(matches!(emitted[1].phase, ActionPhase::End(_)));
    Ok(())
}

#[test]
fn note_commit_cancel_and_temporary_completion_share_tool_owner() -> R {
    use winit::event::MouseButton;
    let mut e = fixture(0)?;
    let mut input = ImageInput::new()?;
    let mut c = Camera::new([800, 600]);
    let pointer = |state| PhysicalEvent::Button {
        control: PhysicalControl::Pointer(PointerButton::Mouse(MouseButton::Left)),
        state,
        repeat: false,
    };
    invoke(&mut input, &mut e, &mut c, Action::SelectTool(Tool::Text))?;
    input.cursor_moved([40., 40.], &e, &mut c)?;
    input.physical(pointer(ElementState::Pressed), &mut e, &mut c)?;
    input.physical(pointer(ElementState::Released), &mut e, &mut c)?;
    assert_eq!(e.undo_len(), 0);
    input.annotation.edit.as_mut().ok_or("draft")?.value = "note".into();
    input.commit_drafts(&mut e)?;
    assert_eq!(input.annotation.tools.tool(), Tool::Pointer);
    assert_eq!(e.undo_len(), 1);
    e.undo()?;
    assert_eq!(e.document().annotation_count(), 0);
    input.keymap.bind(Binding {
        control: PhysicalControl::LogicalKey(LogicalKey::Character('q')),
        modifiers: ModifierMatch::Exact(Modifiers::NONE),
        trigger: Trigger::Hold,
        action: Action::TemporaryTool(Tool::Text),
    })?;
    edge(
        &mut input,
        &mut e,
        &mut c,
        KeyCode::KeyQ,
        'q',
        ElementState::Pressed,
        false,
    )?;
    assert_eq!(input.annotation.tools.tool(), Tool::Text);
    input.physical(pointer(ElementState::Pressed), &mut e, &mut c)?;
    input.physical(pointer(ElementState::Released), &mut e, &mut c)?;
    input.annotation.edit.as_mut().ok_or("draft")?.value = "temp".into();
    input.commit_drafts(&mut e)?;
    assert_eq!(input.annotation.tools.tool(), Tool::Pointer);
    edge(
        &mut input,
        &mut e,
        &mut c,
        KeyCode::KeyQ,
        'a',
        ElementState::Released,
        false,
    )?;
    assert_eq!(input.annotation.tools.tool(), Tool::Pointer);
    assert_eq!(e.undo_len(), 1);
    invoke(&mut input, &mut e, &mut c, Action::SelectTool(Tool::Text))?;
    input.physical(pointer(ElementState::Pressed), &mut e, &mut c)?;
    input.physical(pointer(ElementState::Released), &mut e, &mut c)?;
    invoke(&mut input, &mut e, &mut c, Action::CancelInteraction)?;
    assert!(input.annotation.edit.is_none());
    assert_eq!(e.undo_len(), 1);
    assert_eq!(input.annotation.tools.tool(), Tool::Pointer);
    Ok(())
}

#[test]
fn nested_temporary_tools_never_outlive_modal_note_state() -> R {
    use winit::event::{MouseButton, WindowEvent};
    for mode in 0..3 {
        let mut e = fixture(0)?;
        let mut input = ImageInput::new()?;
        let mut c = Camera::new([800, 600]);
        invoke(&mut input, &mut e, &mut c, Action::SelectTool(Tool::Pan))?;
        for (ch, tool) in [('b', Tool::Pan), ('q', Tool::Text)] {
            input.keymap.unassign(Action::AddCameraBookmark);
            input.keymap.bind(Binding {
                control: PhysicalControl::LogicalKey(LogicalKey::Character(ch)),
                modifiers: ModifierMatch::Exact(Modifiers::NONE),
                trigger: Trigger::Hold,
                action: Action::TemporaryTool(tool),
            })?;
        }
        edge(
            &mut input,
            &mut e,
            &mut c,
            KeyCode::KeyB,
            'b',
            ElementState::Pressed,
            false,
        )?;
        edge(
            &mut input,
            &mut e,
            &mut c,
            KeyCode::KeyQ,
            'q',
            ElementState::Pressed,
            false,
        )?;
        input.cursor_moved([30., 30.], &e, &mut c)?;
        for state in [ElementState::Pressed, ElementState::Released] {
            input.physical(
                PhysicalEvent::Button {
                    control: PhysicalControl::Pointer(PointerButton::Mouse(MouseButton::Left)),
                    state,
                    repeat: false,
                },
                &mut e,
                &mut c,
            )?;
        }
        assert!(input.annotation.edit.is_some());
        assert_eq!(input.annotation.tools.tool(), Tool::Pointer);
        if mode == 0 {
            input.annotation.edit.as_mut().ok_or("draft")?.value = "nested".into();
            input.commit_drafts(&mut e)?;
            assert_eq!(e.undo_len(), 1);
        } else {
            if mode == 2 {
                input.handle(&WindowEvent::Focused(false), &mut e, &mut c)?;
                assert!(input.annotation.edit.is_some());
            }
            invoke(&mut input, &mut e, &mut c, Action::CancelInteraction)?;
            assert_eq!(e.undo_len(), 0);
        }
        for code in [KeyCode::KeyB, KeyCode::KeyQ] {
            edge(
                &mut input,
                &mut e,
                &mut c,
                code,
                'x',
                ElementState::Released,
                false,
            )?;
        }
        assert_eq!(input.annotation.tools.tool(), Tool::Pointer);
    }
    Ok(())
}
#[test]
fn unsupported_legacy_custom_key_is_reported_without_rewriting_preferences() -> R {
    let mut p = tack_app::preferences::Preferences::defaults()?;
    p.version = 1;
    p.keymap[0].control = PhysicalControl::Key(PhysicalKey::Code(KeyCode::IntlYen));
    let file = std::env::temp_dir().join(format!(
        "tack-legacy-{:032x}.json",
        tack_storage::new_document_id()?.value()
    ));
    let bytes = serde_json::to_vec(&p)?;
    std::fs::write(&file, &bytes)?;
    let err = tack_app::preferences::read(&file)
        .err()
        .ok_or("legacy rejection")?;
    assert!(err.to_string().contains("unsupported version-1 key"));
    assert_eq!(std::fs::read(&file)?, bytes);
    std::fs::remove_file(file)?;
    Ok(())
}
