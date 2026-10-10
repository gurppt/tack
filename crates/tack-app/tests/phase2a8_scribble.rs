use tack_app::{
    actions::*, annotation_geometry, annotation_scene::AnnotationScene, image_input::ImageInput,
    image_interaction::GestureKind, input::*, scribble_edit,
};
use tack_core::*;
use winit::event::{ElementState, MouseButton};
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
fn button(i: &mut ImageInput, e: &mut DocumentEditor, c: &mut Camera, pressed: bool) -> R {
    i.physical(
        PhysicalEvent::Button {
            control: PhysicalControl::Pointer(PointerButton::Mouse(MouseButton::Left)),
            state: if pressed {
                ElementState::Pressed
            } else {
                ElementState::Released
            },
            repeat: false,
        },
        e,
        c,
    )?;
    Ok(())
}
fn stroke(i: &mut ImageInput, e: &mut DocumentEditor, c: &mut Camera, y: f64) -> R {
    i.cursor_moved(c.world_to_screen([-100., y]), e, c)?;
    button(i, e, c, true)?;
    i.cursor_moved(c.world_to_screen([100., y]), e, c)?;
    button(i, e, c, false)?;
    Ok(())
}
fn scribble(
    o: &DocumentObject,
) -> Result<&ScribbleObject, Box<dyn std::error::Error + Send + Sync>> {
    if let ObjectKind::Annotation(a) = o.kind()
        && let AnnotationKind::Scribble(s) = a.kind()
    {
        Ok(s)
    } else {
        Err("not Scribble".into())
    }
}
#[test]
fn several_strokes_finish_one_object_without_bridges_and_roundtrip_transform_history() -> R {
    let mut e = DocumentEditor::new(
        Document::new(DocumentId::new(1)?, DocumentLimits::default()),
        100,
    );
    let mut i = ImageInput::new()?;
    let mut c = Camera::new([800, 600]);
    invoke(&mut i, &mut e, &mut c, Action::SelectTool(Tool::Scribble))?;
    stroke(&mut i, &mut e, &mut c, -60.)?;
    stroke(&mut i, &mut e, &mut c, 60.)?;
    assert_eq!(e.document().annotation_count(), 0);
    assert_eq!(e.undo_len(), 0);
    assert_eq!(i.annotation.scribble.len(), 2);
    invoke(&mut i, &mut e, &mut c, Action::FinishScribble)?;
    assert_eq!(e.undo_len(), 1);
    assert_eq!(e.document().annotation_count(), 1);
    let id = i.images.selection.ids().next().ok_or("selection")?;
    let o = e.document().object(id).ok_or("object")?;
    assert_eq!(scribble(o)?.strokes().len(), 2);
    let ObjectKind::Annotation(a) = o.kind() else {
        return Err("annotation".into());
    };
    assert!(!annotation_geometry::hit(a, o.transform(), [0., 0.], 2.));
    assert!(annotation_geometry::hit(a, o.transform(), [0., 60.], 2.));
    let mut scene = AnnotationScene::default();
    scene.build(e.document(), &i.images, &i.annotation, &c, &[]);
    assert_eq!(scene.primitives.len(), 2);
    let initial = e.document().clone();
    for kind in [GestureKind::Move, GestureKind::Scale, GestureKind::Rotate] {
        i.images.begin(kind, [100., 60.], &e)?;
        i.images.update([130., 90.])?;
        i.images.commit(&mut e)?;
        e.undo()?;
        assert_eq!(e.document(), &initial);
    }
    invoke(&mut i, &mut e, &mut c, Action::FlipHorizontal)?;
    invoke(&mut i, &mut e, &mut c, Action::FlipVertical)?;
    let flipped = e.document().clone();
    let wire = tack_shared::DocumentRecord::from_document(&flipped)?;
    assert_eq!(wire.to_document()?, flipped);
    let (schema, counts, bytes) = tack_storage::encode_metadata(&flipped)?;
    assert_eq!(schema, 6);
    assert_eq!(
        tack_storage::decode_metadata(schema, counts, &bytes)?,
        flipped
    );
    // A higher kind encoded with old schema must be refused, not truncated.
    assert!(tack_storage::decode_metadata(3, counts, &bytes).is_err());
    e.undo()?;
    e.undo()?;
    assert_eq!(e.document(), &initial);
    invoke(&mut i, &mut e, &mut c, Action::SelectTool(Tool::Scribble))?;
    stroke(&mut i, &mut e, &mut c, 100.)?;
    invoke(&mut i, &mut e, &mut c, Action::CancelInteraction)?;
    assert_eq!(e.document(), &initial);
    assert!(i.annotation.scribble.is_empty());
    Ok(())
}
#[test]
fn swept_eraser_splits_target_only_with_one_undo_and_merge_preserves_styles() -> R {
    let mut d = Document::new(DocumentId::new(1)?, DocumentLimits::default());
    for (n, y, color, width) in [
        (1, 0., Color([255, 0, 0, 255]), 2.),
        (2, 80., Color([0, 255, 0, 255]), 8.),
    ] {
        d.apply(Command::AddObject {
            object: DocumentObject::annotation(
                ObjectId::new(n)?,
                Annotation::new(
                    AnnotationKind::Scribble(ScribbleObject::new(vec![[0., 0.5], [1., 0.5]])?),
                    AnnotationStyle::new(color, None, width, Opacity::new(0.5)?)?,
                ),
                Transform::new([0., y], [200., 10.], 0., [false; 2])?,
            )?,
            index: d.object_order().len(),
        })?;
    }
    let mut e = DocumentEditor::new(d, 100);
    let mut i = ImageInput::new()?;
    let mut c = Camera::new([800, 600]);
    let before = e.document().clone();
    i.images.selection.select(Some(ObjectId::new(1)?), false);
    invoke(&mut i, &mut e, &mut c, Action::SelectTool(Tool::Eraser))?;
    i.cursor_moved(c.world_to_screen([0., -30.]), &e, &mut c)?;
    button(&mut i, &mut e, &mut c, true)?;
    i.cursor_moved(c.world_to_screen([0., 30.]), &e, &mut c)?;
    assert_eq!(e.document(), &before);
    button(&mut i, &mut e, &mut c, false)?;
    assert_eq!(e.undo_len(), 1);
    let o = e.document().object(ObjectId::new(1)?).ok_or("target")?;
    assert_eq!(scribble(o)?.strokes().len(), 2);
    let ObjectKind::Annotation(a) = o.kind() else {
        return Err("annotation".into());
    };
    assert!(!annotation_geometry::hit(a, o.transform(), [0., 0.], 0.));
    assert_eq!(
        e.document().object(ObjectId::new(2)?),
        before.object(ObjectId::new(2)?)
    );
    e.undo()?;
    assert_eq!(e.document(), &before);
    e.redo()?;
    e.undo()?;
    i.images.selection.select(Some(ObjectId::new(1)?), false);
    i.images.selection.select(Some(ObjectId::new(2)?), true);
    invoke(&mut i, &mut e, &mut c, Action::MergeScribbles)?;
    assert_eq!(e.document().annotation_count(), 1);
    let id = i.images.selection.ids().next().ok_or("merged")?;
    let o = e.document().object(id).ok_or("merged")?;
    let styles = scribble(o)?
        .strokes()
        .iter()
        .map(|s| s.style().ok_or("style"))
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(
        styles.iter().map(|s| s.width()).collect::<Vec<_>>(),
        vec![2., 8.]
    );
    assert_eq!(
        styles.iter().map(|s| s.stroke()).collect::<Vec<_>>(),
        vec![Color([255, 0, 0, 255]), Color([0, 255, 0, 255])]
    );
    let merged = e.document().clone();
    invoke(
        &mut i,
        &mut e,
        &mut c,
        Action::AnnotationStyle(tack_app::annotation_tool::StyleAction::Color),
    )?;
    let o = e.document().object(id).ok_or("color")?.clone();
    assert!(scribble(&o)?.strokes().iter().all(|s| {
        s.style()
            .is_some_and(|s| s.stroke() == Color([240, 92, 104, 255]))
    }));
    e.undo()?;
    assert_eq!(e.document(), &merged);
    let (schema, counts, bytes) = tack_storage::encode_metadata(&merged)?;
    assert_eq!(
        tack_storage::decode_metadata(schema, counts, &bytes)?,
        merged
    );
    assert_eq!(
        tack_shared::DocumentRecord::from_document(&merged)?.to_document()?,
        merged
    );
    let ObjectKind::Annotation(a) = o.kind() else {
        return Err("annotation".into());
    };
    assert!(
        scribble_edit::erase(a, o.transform(), [-1000., -1000.], [1000., 1000.], 2000.)?.is_none()
    );
    e.undo()?;
    assert_eq!(e.document(), &before);
    Ok(())
}

#[test]
fn navigation_menu_finish_and_disjoint_remote_changes_preserve_draft() -> R {
    let mut e = DocumentEditor::new(
        Document::new(DocumentId::new(1)?, DocumentLimits::default()),
        100,
    );
    let mut i = ImageInput::new()?;
    let mut c = Camera::new([800, 600]);
    invoke(&mut i, &mut e, &mut c, Action::SelectTool(Tool::Scribble))?;
    stroke(&mut i, &mut e, &mut c, 0.)?;
    i.dispatch(
        ActionEvent {
            action: Action::ZoomView,
            phase: ActionPhase::Delta(1.),
        },
        &mut e,
        &mut c,
    )?;
    assert_eq!(i.annotation.scribble.len(), 1);
    assert_eq!(e.undo_len(), 0);
    // A disjoint authority edit rebases independent creation drafts.
    e.execute(Command::AddObject {
        object: DocumentObject::frame(
            ObjectId::new(10)?,
            "Remote".into(),
            Transform::new([300., 0.], [100., 100.], 0., [false; 2])?,
        )?,
        index: 0,
    })?;
    let remote = ObjectId::new(10)?;
    i.rebase_scribble_drafts(e.generation(), |id| id == remote);
    stroke(&mut i, &mut e, &mut c, 80.)?;
    // Opening a menu while the third stroke is still held finishes it.
    i.cursor_moved(c.world_to_screen([-80., 120.]), &e, &mut c)?;
    button(&mut i, &mut e, &mut c, true)?;
    i.cursor_moved(c.world_to_screen([80., 120.]), &e, &mut c)?;
    i.suspend_for_menu(&mut e, &mut c)?;
    assert_eq!(e.document().annotation_count(), 1);
    let id = i.images.selection.ids().next().ok_or("Scribble")?;
    assert_eq!(
        scribble(e.document().object(id).ok_or("object")?)?
            .strokes()
            .len(),
        3
    );
    // Eraser target survives disjoint edits, but a touching authority scope cancels it.
    invoke(&mut i, &mut e, &mut c, Action::SelectTool(Tool::Eraser))?;
    i.cursor_moved(c.world_to_screen([0., 0.]), &e, &mut c)?;
    button(&mut i, &mut e, &mut c, true)?;
    i.rebase_scribble_drafts(e.generation(), |target| target != id);
    assert!(i.annotation.eraser.is_some());
    i.rebase_scribble_drafts(e.generation(), |target| target == id);
    assert!(i.annotation.eraser.is_none());
    button(&mut i, &mut e, &mut c, false)?;
    Ok(())
}
#[test]
fn merge_uses_painter_order_even_when_ids_run_backwards() -> R {
    let mut d = Document::new(DocumentId::new(1)?, DocumentLimits::default());
    let t = Transform::new([0., 0.], [100., 100.], 0., [false; 2])?;
    for (id, color) in [(10, Color([255, 0, 0, 255])), (1, Color([0, 0, 255, 255]))] {
        d.apply(Command::AddObject {
            object: DocumentObject::annotation(
                ObjectId::new(id)?,
                Annotation::new(
                    AnnotationKind::Scribble(ScribbleObject::new(vec![[0., 0.5], [1., 0.5]])?),
                    AnnotationStyle::new(color, None, 3., Opacity::OPAQUE)?,
                ),
                t,
            )?,
            index: d.object_order().len(),
        })?;
    }
    let before = d.clone();
    let mut e = DocumentEditor::new(d, 100);
    let mut i = ImageInput::new()?;
    let mut c = Camera::new([800, 600]);
    i.images.selection.select_all(e.document());
    invoke(&mut i, &mut e, &mut c, Action::MergeScribbles)?;
    let o = e.document().objects().next().ok_or("merged")?;
    assert_eq!(
        scribble(o)?
            .strokes()
            .iter()
            .filter_map(|s| s.style())
            .map(|s| s.stroke())
            .collect::<Vec<_>>(),
        vec![Color([255, 0, 0, 255]), Color([0, 0, 255, 255])]
    );
    e.undo()?;
    assert_eq!(e.document(), &before);
    Ok(())
}

#[test]
fn merge_refuses_intervening_overlap_without_mutation() -> R {
    let mut d = Document::new(DocumentId::new(1)?, DocumentLimits::default());
    let t = Transform::new([0., 0.], [100., 100.], 0., [false; 2])?;
    for id in [10, 11, 12] {
        let kind = if id == 11 {
            AnnotationKind::Rect
        } else {
            AnnotationKind::Scribble(ScribbleObject::new(vec![[0., 0.5], [1., 0.5]])?)
        };
        d.apply(Command::AddObject {
            object: DocumentObject::annotation(
                ObjectId::new(id)?,
                Annotation::new(kind, AnnotationStyle::default()),
                t,
            )?,
            index: d.object_order().len(),
        })?;
    }
    let before = d.clone();
    let mut e = DocumentEditor::new(d, 100);
    let mut i = ImageInput::new()?;
    let mut c = Camera::new([800, 600]);
    i.images.selection.select(Some(ObjectId::new(10)?), false);
    i.images.selection.select(Some(ObjectId::new(12)?), true);
    assert!(invoke(&mut i, &mut e, &mut c, Action::MergeScribbles).is_err());
    assert_eq!(e.document(), &before);
    assert_eq!(e.undo_len(), 0);
    e.execute(Command::SetTransform {
        object: ObjectId::new(11)?,
        transform: Transform::new([500., 500.], t.size(), t.rotation(), t.flips())?,
    })?;
    invoke(&mut i, &mut e, &mut c, Action::MergeScribbles)?;
    assert_eq!(e.document().annotation_count(), 2);
    Ok(())
}
