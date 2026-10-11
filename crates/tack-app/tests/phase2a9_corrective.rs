use tack_app::{
    actions::{Action, ActionEvent, ActionPhase},
    image_geometry,
    image_gizmo::{GizmoHit, ImageGizmo},
    image_input::ImageInput,
    image_interaction::{GestureKind, ImageInteraction},
};
use tack_core::*;
type R<T = ()> = Result<T, Box<dyn std::error::Error + Send + Sync>>;
fn fixture() -> R<DocumentEditor> {
    let mut d = Document::new(DocumentId::new(1)?, DocumentLimits::default());
    d.apply(Command::AddSource(Source::embedded(SourceId::new(1)?)))?;
    d.apply(Command::AddAsset(ImageAsset::new(
        AssetId::new(1)?,
        SourceId::new(1)?,
        [800, 600],
    )?))?;
    for id in 1..=2 {
        let id = ObjectId::new(id)?;
        d.apply(Command::AddObject {
            object: DocumentObject::image(
                id,
                AssetId::new(1)?,
                Transform::new(
                    [id.value() as f64 * 150., 20.],
                    [160., 120.],
                    0.4,
                    [true, false],
                )?,
            ),
            index: d.object_order().len(),
        })?;
        d.apply(Command::SetCrop {
            object: id,
            crop: Crop::new(0.1, 0.2, 0.5, 0.5)?,
        })?;
        d.apply(Command::SetOpacity {
            object: id,
            opacity: Opacity::new(0.5)?,
        })?;
        d.apply(Command::SetImageFiltering {
            object: id,
            filtering: ImageFiltering::Nearest,
        })?;
    }
    Ok(DocumentEditor::new(d, 128))
}
fn invoke(input: &mut ImageInput, action: Action, e: &mut DocumentEditor, c: &mut Camera) -> R {
    input.dispatch(
        ActionEvent {
            action,
            phase: ActionPhase::Invoke,
        },
        e,
        c,
    )?;
    Ok(())
}
#[test]
fn every_image_edge_scales_proportionally_around_center_with_one_undo() -> R {
    for handle in 0..8 {
        let mut e = fixture()?;
        let before = e.document().clone();
        let id = ObjectId::new(1)?;
        let original = e.document().object_render_data(id).ok_or("image")?;
        let mut images = ImageInteraction::default();
        images.selection.select(Some(id), false);
        images.begin(
            GestureKind::Resize {
                handle,
                center: false,
            },
            [100., 50.],
            &e,
        )?;
        images.update([155., 82.])?;
        images.commit(&mut e)?;
        let data = e.document().object_render_data(id).ok_or("image")?;
        assert!((data.transform.size()[0] / data.transform.size()[1] - 4. / 3.).abs() < 1e-9);
        assert_eq!(data.transform.center(), original.transform.center());
        assert_eq!(data.transform.rotation(), original.transform.rotation());
        assert_eq!(data.transform.flips(), original.transform.flips());
        assert_eq!(
            (data.crop, data.opacity, data.filtering),
            (original.crop, original.opacity, original.filtering)
        );
        assert_eq!(e.undo_len(), 1);
        e.undo()?;
        assert_eq!(e.document(), &before);
        e.redo()?;
        assert_eq!(e.document().object_render_data(id), Some(data));
    }
    Ok(())
}
#[test]
fn full_edges_and_outer_corners_work_at_small_viewports_and_dpi() -> R {
    let t = Transform::new([0., 0.], [220., 140.], 0.4, [false; 2])?;
    for size in [[800, 600], [1024, 768]] {
        for dpi in [1., 2.] {
            for zoom in [0.5, 1., 2.] {
                let mut c = Camera::new(size);
                c.set_ui_scale(dpi);
                c.set_view([0., 0.], zoom)?;
                let mut g = ImageGizmo::default();
                g.set_scale(dpi);
                let r = 9. * dpi / zoom;
                for (point, handle) in [
                    ([110., 30.], 3),
                    ([-110., -20.], 7),
                    ([20., -70.], 1),
                    ([-30., 70.], 5),
                ] {
                    assert_eq!(
                        g.hit(
                            t,
                            &c,
                            c.world_to_screen(image_geometry::world(t, point)),
                            false,
                            false
                        ),
                        Some(GizmoHit::Resize(handle))
                    );
                }
                let near = image_geometry::world(t, [110. + r * 0.6, 70. + r * 0.6]);
                let far = image_geometry::world(t, [110. + r * 1.5, 70. + r * 1.5]);
                assert_eq!(
                    g.hit(t, &c, c.world_to_screen(near), false, false),
                    Some(GizmoHit::Resize(4))
                );
                assert_eq!(
                    g.hit(t, &c, c.world_to_screen(far), false, false),
                    Some(GizmoHit::Rotate)
                );
            }
        }
    }
    Ok(())
}
#[test]
fn crop_actual_size_grayscale_and_lock_preserve_semantics_history_and_storage() -> R {
    let mut e = fixture()?;
    let mut input = ImageInput::new()?;
    let mut c = Camera::new([800, 600]);
    input.images.selection.select_all(e.document());
    let before = e.document().clone();
    invoke(&mut input, Action::ActualSize, &mut e, &mut c)?;
    for id in input.images.selection.ids() {
        assert_eq!(
            e.document()
                .object_render_data(id)
                .ok_or("image")?
                .transform
                .size(),
            [400., 300.]
        );
    }
    assert_eq!(c.zoom(), 1.);
    e.undo()?;
    assert_eq!(e.document(), &before);
    invoke(&mut input, Action::ResetCrop, &mut e, &mut c)?;
    for id in input.images.selection.ids() {
        let a = e.document().object_render_data(id).ok_or("image")?;
        assert_eq!(a.crop, Crop::FULL);
        assert_eq!(a.transform, before.object(id).ok_or("before")?.transform());
    }
    e.undo()?;
    assert_eq!(e.document(), &before);
    invoke(&mut input, Action::ToggleGrayscale, &mut e, &mut c)?;
    invoke(&mut input, Action::ToggleLockSelection, &mut e, &mut c)?;
    let after = e.document().clone();
    let (schema, counts, bytes) = tack_storage::encode_metadata(&after)?;
    assert_eq!(schema, 8);
    assert_eq!(
        tack_storage::decode_metadata(schema, counts, &bytes)?,
        after
    );
    for o in after.objects() {
        assert!(o.locked());
        assert_eq!(tack_shared::ObjectDto::from_object(o)?.to_object()?, *o);
    }
    invoke(&mut input, Action::ResetCrop, &mut e, &mut c)?;
    assert_eq!(e.document(), &after);
    assert!(input.status.contains("Unlock"));
    assert!(!input.images.begin(GestureKind::Move, [0., 0.], &e)?);
    let mut picks = ImageInteraction::default();
    picks.selection.select_all(e.document());
    assert!(picks.selection.is_empty());
    picks.selection.marquee(
        e.document(),
        WorldRect::new(-1000., -1000., 2000., 2000.)?,
        false,
    );
    assert!(picks.selection.is_empty());
    invoke(&mut input, Action::SelectLockedObjects, &mut e, &mut c)?;
    assert_eq!(input.images.selection.len(), 2);
    invoke(&mut input, Action::ToggleLockSelection, &mut e, &mut c)?;
    assert!(e.document().objects().all(|o| !o.locked()));
    e.undo()?;
    assert_eq!(e.document(), &after);
    e.undo()?;
    e.undo()?;
    assert_eq!(e.document(), &before);
    Ok(())
}
#[test]
fn import_placement_is_viewport_relative_without_source_size_residency() {
    for v in [[800., 600.], [1024., 768.], [8000., 6000.]] {
        for (pixels, fraction) in [([800, 600], 0.5), ([40000, 30000], 0.75)] {
            let actual = tack_app::local_import::placement_size(pixels, v);
            for axis in 0..2 {
                assert!((actual[axis] - v[axis] * fraction).abs() < 1e-9);
            }
        }
    }
}

#[test]
fn text_creation_edits_immediately_and_selection_scaling_scales_font_on_all_handles() -> R {
    use tack_app::{
        actions::Tool,
        input::{PhysicalControl, PhysicalEvent, PointerButton},
    };
    use winit::event::{ElementState, MouseButton};
    let mut e = DocumentEditor::new(
        Document::new(DocumentId::new(9)?, DocumentLimits::default()),
        64,
    );
    let mut i = ImageInput::new()?;
    let mut c = Camera::new([800, 600]);
    invoke(&mut i, Action::SelectTool(Tool::Text), &mut e, &mut c)?;
    i.cursor_moved([300., 200.], &e, &mut c)?;
    for state in [ElementState::Pressed, ElementState::Released] {
        i.physical(
            PhysicalEvent::Button {
                control: PhysicalControl::Pointer(PointerButton::Mouse(MouseButton::Left)),
                state,
                repeat: false,
            },
            &mut e,
            &mut c,
        )?;
    }
    assert!(i.annotation.edit.is_some());
    assert_eq!(e.document().annotation_count(), 0);
    i.annotation
        .edit
        .as_mut()
        .ok_or("draft")?
        .insert("LongWordHere\nSecond line\nThird");
    i.commit_drafts(&mut e)?;
    assert_eq!(e.undo_len(), 1);
    let id = i.images.selection.ids().next().ok_or("text")?;
    let before = e.document().clone();
    for handle in 0..8 {
        let mut images = ImageInteraction::default();
        images.selection.select(Some(id), false);
        images.begin(GestureKind::NoteScale { handle }, [0., 0.], &e)?;
        images.update([12., 9.])?;
        images.commit(&mut e)?;
        let a = e.document().object(id).ok_or("text")?;
        let b = before.object(id).ok_or("before")?;
        let font = |o: &DocumentObject| match o.kind() {
            ObjectKind::Annotation(a) => match a.kind() {
                AnnotationKind::Text(t) => t.font_size(),
                _ => 0.,
            },
            _ => 0.,
        };
        let scale = a.transform().size()[0] / b.transform().size()[0];
        assert!((a.transform().size()[1] / b.transform().size()[1] - scale).abs() < 1e-9);
        assert!((font(a) / font(b) - scale).abs() < 1e-9);
        e.undo()?;
        assert_eq!(e.document(), &before);
    }
    invoke(&mut i, Action::RenameFrame, &mut e, &mut c)?;
    let draft = i.annotation.edit.as_ref().ok_or("edit")?;
    let font = draft.size;
    let initial = draft.transform;
    let edge = c.world_to_screen(image_geometry::world(initial, [initial.size()[0] / 2., 0.]));
    let motion = |p: [f64; 2]| winit::event::WindowEvent::CursorMoved {
        device_id: winit::event::DeviceId::dummy(),
        position: winit::dpi::PhysicalPosition::new(p[0], p[1]),
    };
    i.handle(&motion(edge), &mut e, &mut c)?;
    i.handle(
        &winit::event::WindowEvent::MouseInput {
            device_id: winit::event::DeviceId::dummy(),
            state: ElementState::Pressed,
            button: MouseButton::Left,
        },
        &mut e,
        &mut c,
    )?;
    i.handle(&motion([edge[0] + 80., edge[1]]), &mut e, &mut c)?;
    let draft = i.annotation.edit.as_ref().ok_or("edit")?;
    assert_eq!(draft.size, font);
    assert!(draft.transform.size()[0] > initial.size()[0]);
    assert_eq!(draft.transform.size()[1], initial.size()[1]);
    i.handle(
        &winit::event::WindowEvent::MouseInput {
            device_id: winit::event::DeviceId::dummy(),
            state: ElementState::Released,
            button: MouseButton::Left,
        },
        &mut e,
        &mut c,
    )?;
    i.handle(&motion([10., 10.]), &mut e, &mut c)?;
    i.handle(
        &winit::event::WindowEvent::MouseInput {
            device_id: winit::event::DeviceId::dummy(),
            state: ElementState::Pressed,
            button: MouseButton::Left,
        },
        &mut e,
        &mut c,
    )?;
    assert!(i.annotation.edit.is_none());
    assert_eq!(e.undo_len(), 2);
    Ok(())
}
#[test]
fn exclusive_context_bindings_share_keys_but_same_context_conflicts_and_dispatch_is_filtered() -> R
{
    use tack_app::{
        annotation_tool::StyleAction,
        bindings::{Binding, Keymap, ModifierMatch, Trigger},
        input::{LogicalKey, Modifiers, PhysicalControl, PhysicalEvent},
    };
    use winit::{
        event::ElementState,
        keyboard::{KeyCode, PhysicalKey},
    };
    let mut map = Keymap::default();
    let bind = |action, control| Binding {
        action,
        control,
        modifiers: ModifierMatch::Exact(Modifiers::NONE),
        trigger: Trigger::Press,
    };
    map.bind(bind(
        Action::ToggleGrayscale,
        PhysicalControl::Key(PhysicalKey::Code(KeyCode::KeyG)),
    ))?;
    map.bind(bind(
        Action::AnnotationStyle(StyleAction::LargerText),
        PhysicalControl::LogicalKey(LogicalKey::Character('g')),
    ))?;
    assert!(
        map.bind(bind(
            Action::ResetCrop,
            PhysicalControl::Key(PhysicalKey::Code(KeyCode::KeyG))
        ))
        .is_err()
    );
    let mut e = fixture()?;
    let mut i = ImageInput::new()?;
    let mut c = Camera::new([800, 600]);
    i.keymap = map;
    i.images.selection.select(Some(ObjectId::new(1)?), false);
    let key = |state| PhysicalEvent::Keyboard {
        physical: PhysicalKey::Code(KeyCode::KeyG),
        logical: Some(LogicalKey::Character('g')),
        state,
        repeat: false,
    };
    i.physical(key(ElementState::Pressed), &mut e, &mut c)?;
    assert!(
        e.document()
            .object_render_data(ObjectId::new(1)?)
            .ok_or("image")?
            .grayscale
    );
    i.physical(key(ElementState::Released), &mut e, &mut c)?;
    i.paste_text_note("Text".into(), &mut e, &c)?;
    let id = i.images.selection.ids().next().ok_or("text")?;
    let size = |doc: &Document| match doc.object(id).map(|o| o.kind()) {
        Some(ObjectKind::Annotation(a)) => match a.kind() {
            AnnotationKind::Text(t) => t.font_size(),
            _ => 0.,
        },
        _ => 0.,
    };
    let before = size(e.document());
    i.physical(key(ElementState::Pressed), &mut e, &mut c)?;
    assert!(size(e.document()) > before); // Inactive physical image binding must not hide the logical text binding.
    Ok(())
}
