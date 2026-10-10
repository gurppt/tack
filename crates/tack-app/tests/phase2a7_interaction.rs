use tack_app::{
    actions::{Action, ActionEvent, ActionPhase, Tool},
    annotation_tool::{NoteEdit, StyleAction},
    bindings::{Binding, Keymap, ModifierMatch, Trigger},
    image_input::ImageInput,
    input::{Modifiers, PhysicalControl, PhysicalEvent, PointerButton},
    toolbar::{Config, Placement, Toolbar},
};
use tack_core::*;
use winit::{
    event::{ElementState, MouseButton},
    keyboard::{KeyCode, PhysicalKey},
};
type R = Result<(), Box<dyn std::error::Error + Send + Sync>>;
fn invoke(i: &mut ImageInput, e: &mut DocumentEditor, c: &mut Camera, a: Action) -> R {
    i.dispatch(
        ActionEvent {
            action: a,
            phase: ActionPhase::Invoke,
        },
        e,
        c,
    )?;
    Ok(())
}
fn shared_roundtrip(
    i: &mut ImageInput,
    local: &mut DocumentEditor,
    c: &mut Camera,
    action: Action,
) -> R {
    let mut remote = DocumentEditor::new(local.document().clone(), 200);
    let mut shared = DocumentEditor::shared(local.document().clone());
    shared.set_shared_writable(true);
    invoke(i, &mut shared, c, action)?;
    assert_eq!(shared.pending_backend_requests(), 1);
    let Some(BackendRequest::Edit(command)) = shared.take_backend_request() else {
        return Err("expected one shared semantic edit".into());
    };
    let wire = tack_shared::CommandDto::from_command(&command)?;
    let wire: tack_shared::CommandDto = serde_json::from_slice(&serde_json::to_vec(&wire)?)?;
    remote.execute(wire.to_command()?)?;
    invoke(i, local, c, action)?;
    assert_eq!(remote.document(), local.document());
    Ok(())
}
fn editor() -> Result<DocumentEditor, Box<dyn std::error::Error + Send + Sync>> {
    Ok(DocumentEditor::new(
        Document::new(DocumentId::new(1)?, DocumentLimits::default()),
        200,
    ))
}
fn middle(state: ElementState) -> PhysicalEvent {
    PhysicalEvent::Button {
        control: PhysicalControl::Pointer(PointerButton::Mouse(MouseButton::Middle)),
        state,
        repeat: false,
    }
}
#[test]
fn hold_pointer_starts_pan_and_drawing_restores_base_with_one_undo() -> R {
    for tool in [
        Tool::Pan,
        Tool::Rectangle,
        Tool::Line,
        Tool::Arrow,
        Tool::Scribble,
    ] {
        let mut e = editor()?;
        let mut i = ImageInput::new()?;
        let mut c = Camera::new([800, 600]);
        invoke(&mut i, &mut e, &mut c, Action::SelectTool(Tool::Scribble))?;
        i.keymap.unassign(Action::PanView);
        i.keymap.bind(Binding {
            action: Action::TemporaryTool(tool),
            control: PhysicalControl::Pointer(PointerButton::Mouse(MouseButton::Middle)),
            modifiers: ModifierMatch::Any,
            trigger: Trigger::Hold,
        })?;
        let before = c.screen_to_world([400., 300.]);
        i.cursor_moved([100., 100.], &e, &mut c)?;
        i.physical(middle(ElementState::Pressed), &mut e, &mut c)?;
        i.cursor_moved([200., 180.], &e, &mut c)?;
        i.physical(middle(ElementState::Released), &mut e, &mut c)?;
        assert_eq!(i.active_tool(), Tool::Scribble);
        if tool == Tool::Pan {
            assert_ne!(before, c.screen_to_world([400., 300.]));
            assert_eq!(e.undo_len(), 0);
        } else {
            assert_eq!(e.undo_len(), 1);
            assert_eq!(e.document().annotation_count(), 1);
            e.undo()?;
            assert_eq!(e.document().annotation_count(), 0);
            e.redo()?;
        }
        i.physical(middle(ElementState::Pressed), &mut e, &mut c)?;
        let before = e.document().clone();
        i.physical(PhysicalEvent::FocusLost, &mut e, &mut c)?;
        assert_eq!(e.document(), &before);
        assert_eq!(i.active_tool(), Tool::Scribble);
        assert!(!i.active());
    }
    Ok(())
}
#[test]
fn rectangle_fill_palette_context_stroke_and_history_are_exact() -> R {
    let mut e = editor()?;
    let mut i = ImageInput::new()?;
    let mut c = Camera::new([800, 600]);
    let id = ObjectId::new(10)?;
    e.execute(Command::AddObject {
        object: DocumentObject::annotation(
            id,
            Annotation::new(AnnotationKind::Rect, AnnotationStyle::default()),
            Transform::new([0., 0.], [100., 80.], 0., [false; 2])?,
        )?,
        index: 0,
    })?;
    i.images.selection.select(Some(id), false);
    for alpha in [Some(255), Some(191), Some(128), Some(64), None] {
        let before = e.document().clone();
        shared_roundtrip(
            &mut i,
            &mut e,
            &mut c,
            Action::AnnotationStyle(StyleAction::Fill),
        )?;
        let ObjectKind::Annotation(a) = e.document().object(id).ok_or("rect")?.kind() else {
            return Err("rect".into());
        };
        assert_eq!(a.style().fill().map(|c| c.0[3]), alpha);
        e.undo()?;
        assert_eq!(e.document(), &before);
        e.redo()?;
    }
    shared_roundtrip(
        &mut i,
        &mut e,
        &mut c,
        Action::AnnotationStyle(StyleAction::Fill),
    )?;
    shared_roundtrip(&mut i, &mut e, &mut c, Action::ContextDecrease)?;
    assert_eq!(i.status, "Fill 75%");
    shared_roundtrip(
        &mut i,
        &mut e,
        &mut c,
        Action::AnnotationStyle(StyleAction::Color),
    )?;
    let ObjectKind::Annotation(a) = e.document().object(id).ok_or("rect")?.kind() else {
        return Err("rect".into());
    };
    let fill = a.style().fill().ok_or("fill")?.0;
    assert_eq!(fill[3], 191);
    assert_eq!(fill[..3], a.style().stroke().0[..3]);
    let width = a.style().width();
    shared_roundtrip(&mut i, &mut e, &mut c, Action::ContextIncrease)?;
    let ObjectKind::Annotation(a) = e.document().object(id).ok_or("rect")?.kind() else {
        return Err("rect".into());
    };
    assert!(a.style().width() > width);
    Ok(())
}
#[test]
fn note_paste_edit_preserves_paper_and_integer_size() -> R {
    let mut e = editor()?;
    let mut i = ImageInput::new()?;
    let mut c = Camera::new([800, 600]);
    i.paste_text_note("A\nB".into(), &mut e, &c)?;
    let id = i.images.selection.ids().next().ok_or("note")?;
    let before = e.document().object(id).ok_or("note")?.clone();
    let ObjectKind::Annotation(a) = before.kind() else {
        return Err("note".into());
    };
    assert!(a.style().fill().is_some());
    invoke(&mut i, &mut e, &mut c, Action::RenameFrame)?;
    let mut edit: Box<NoteEdit> = i.annotation.edit.take().ok_or("draft")?;
    edit.insert("C");
    edit.finish(&mut e)?;
    let ObjectKind::Annotation(after) = e.document().object(id).ok_or("note")?.kind() else {
        return Err("note".into());
    };
    assert_eq!(a.style(), after.style());
    invoke(&mut i, &mut e, &mut c, Action::ContextIncrease)?;
    let ObjectKind::Annotation(after) = e.document().object(id).ok_or("note")?.kind() else {
        return Err("note".into());
    };
    let AnnotationKind::Text(t) = after.kind() else {
        return Err("text".into());
    };
    assert_eq!(t.font_size(), 32.);
    e.undo()?;
    e.redo()?;
    Ok(())
}
#[test]
fn aspect_reset_preserves_crop_center_rotation_flip_area_and_exact_undo() -> R {
    let mut e = editor()?;
    let mut i = ImageInput::new()?;
    let mut c = Camera::new([800, 600]);
    e.execute(Command::AddSource(Source::linked(
        SourceId::new(1)?,
        "missing.png",
    )?))?;
    e.execute(Command::AddAsset(ImageAsset::new(
        AssetId::new(1)?,
        SourceId::new(1)?,
        [600, 300],
    )?))?;
    let id = ObjectId::new(1)?;
    let transform = Transform::new([50., -70.], [100., 100.], 0.7, [true, false])?;
    e.execute(Command::AddObject {
        object: DocumentObject::image(id, AssetId::new(1)?, transform),
        index: 0,
    })?;
    e.execute(Command::SetCrop {
        object: id,
        crop: Crop::new(0.1, 0.2, 0.5, 0.6)?,
    })?;
    i.images.selection.select(Some(id), false);
    let before = e.document().clone();
    let n = e.undo_len();
    shared_roundtrip(&mut i, &mut e, &mut c, Action::ResetAspectRatio)?;
    let data = e.document().object_render_data(id).ok_or("image")?;
    let t = data.transform;
    assert!((t.size()[0] / t.size()[1] - 5. / 3.).abs() < 1e-10);
    assert!((t.size()[0] * t.size()[1] - 10000.).abs() < 1e-8);
    assert_eq!(t.center(), transform.center());
    assert_eq!(t.rotation(), transform.rotation());
    assert_eq!(t.flips(), transform.flips());
    assert_eq!(data.crop, Crop::new(0.1, 0.2, 0.5, 0.6)?);
    assert_eq!(e.undo_len(), n + 1);
    e.undo()?;
    assert_eq!(e.document(), &before);
    e.redo()?;
    for size in [[1000., 40.], [40., 1000.]] {
        e.execute(Command::SetTransform {
            object: id,
            transform: Transform::new(
                transform.center(),
                size,
                transform.rotation(),
                transform.flips(),
            )?,
        })?;
        shared_roundtrip(&mut i, &mut e, &mut c, Action::ResetAspectRatio)?;
        let restored = e.document().object(id).ok_or("image")?.transform();
        assert!((restored.size()[0] / restored.size()[1] - 5. / 3.).abs() < 1e-10);
        assert!((restored.size()[0] * restored.size()[1] - 40000.).abs() < 1e-8);
    }
    let root = std::env::temp_dir().join(format!(
        "tack-aspect-{:032x}",
        tack_storage::new_document_id()?.value()
    ));
    std::fs::create_dir(&root)?;
    let path = root.join("aspect.tack");
    tack_storage::save(&path, e.document(), vec![])?;
    assert_eq!(&tack_storage::TackFile::open(&path)?.document, e.document());
    std::fs::remove_dir_all(root)?;
    Ok(())
}
#[test]
fn toolbar_anchor_and_independent_integer_scale_survive_resize() -> R {
    let mut cfg = Config::default();
    let mut bar = Toolbar::default();
    for scale in 1..=3 {
        cfg.scale = scale;
        for p in [
            Placement::Top,
            Placement::Bottom,
            Placement::Left,
            Placement::Right,
        ] {
            cfg.placement = p;
            for anchor in [0, 2500, 5000, 7500, 10000] {
                cfg.edge_position = Some(anchor);
                for size in [[800, 600], [1024, 768], [1600, 900]] {
                    bar.layout(&cfg, size, 2., true);
                    let axis = usize::from(matches!(p, Placement::Left | Placement::Right));
                    let b = bar.bounds;
                    let travel = f64::from(size[axis])
                        - if axis == 1 { 40. } else { 0. }
                        - (b[axis + 2] - b[axis]);
                    assert!(
                        (b[axis] - travel * f64::from(anchor) / 10000.).abs() <= f64::from(scale)
                    );
                    for cell in &bar.buttons[..bar.count] {
                        assert_eq!(cell.rect[2] - cell.rect[0], 16. * f64::from(scale));
                        assert_eq!(cell.rect[3] - cell.rect[1], 16. * f64::from(scale));
                    }
                }
            }
        }
    }
    assert_eq!(
        serde_json::from_slice::<Config>(&serde_json::to_vec(&cfg)?)?,
        cfg
    );
    Ok(())
}
#[test]
fn numpad_adjust_is_physical_with_numlock_independent_from_main_row() -> R {
    let mut i = ImageInput::new()?;
    let mut e = editor()?;
    let mut c = Camera::new([800, 600]);
    assert!(
        i.keymap
            .for_action(Action::ContextIncrease)
            .any(|b| b.control == PhysicalControl::Key(PhysicalKey::Code(KeyCode::NumpadAdd)))
    );
    for logical in [Some(tack_app::input::LogicalKey::Character('+')), None] {
        i.physical(
            PhysicalEvent::Keyboard {
                physical: PhysicalKey::Code(KeyCode::NumpadAdd),
                logical,
                state: ElementState::Pressed,
                repeat: false,
            },
            &mut e,
            &mut c,
        )?;
        assert_eq!(i.status, "No contextual adjustment");
        i.physical(
            PhysicalEvent::Keyboard {
                physical: PhysicalKey::Code(KeyCode::NumpadAdd),
                logical,
                state: ElementState::Released,
                repeat: false,
            },
            &mut e,
            &mut c,
        )?;
    }
    assert_eq!(e.undo_len(), 0);
    let mut map = Keymap::default();
    map.bind(Binding {
        action: Action::ContextIncrease,
        control: PhysicalControl::Key(PhysicalKey::Code(KeyCode::NumpadAdd)),
        modifiers: ModifierMatch::Exact(Modifiers::NONE),
        trigger: Trigger::Press,
    })?;
    map.bind(Binding {
        action: Action::Undo,
        control: PhysicalControl::LogicalKey(tack_app::input::LogicalKey::Character('+')),
        modifiers: ModifierMatch::Exact(Modifiers::NONE),
        trigger: Trigger::Press,
    })?;
    Ok(())
}

#[test]
fn hold_focus_loss_restores_an_unstarted_one_shot_base_and_right_bindings_take_priority() -> R {
    let mut e = editor()?;
    let mut i = ImageInput::new()?;
    let mut c = Camera::new([800, 600]);
    invoke(&mut i, &mut e, &mut c, Action::SelectTool(Tool::Rectangle))?;
    i.keymap.unassign(Action::PanView);
    i.keymap.bind(Binding {
        action: Action::TemporaryTool(Tool::Pan),
        control: PhysicalControl::Pointer(PointerButton::Mouse(MouseButton::Middle)),
        trigger: Trigger::Hold,
        modifiers: ModifierMatch::Any,
    })?;
    i.physical(middle(ElementState::Pressed), &mut e, &mut c)?;
    i.physical(PhysicalEvent::FocusLost, &mut e, &mut c)?;
    assert_eq!(i.active_tool(), Tool::Rectangle);
    i.keymap.bind(Binding {
        action: Action::JumpCameraSlot(12),
        control: PhysicalControl::Pointer(PointerButton::Mouse(MouseButton::Right)),
        trigger: Trigger::Release,
        modifiers: ModifierMatch::Exact(Modifiers::CONTROL),
    })?;
    assert!(
        i.keymap
            .pointer_bound(PointerButton::Mouse(MouseButton::Right), Modifiers::CONTROL)
    );
    assert!(
        !i.keymap
            .pointer_bound(PointerButton::Mouse(MouseButton::Right), Modifiers::NONE)
    );
    Ok(())
}
