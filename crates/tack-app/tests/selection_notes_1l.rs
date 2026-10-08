use tack_app::{
    actions::Tool,
    annotation_scene::AnnotationScene,
    annotation_tool::{AnnotationInput, Creation},
    image_geometry as geometry,
    image_gizmo::ImageGizmo,
    image_input::ImageInput,
    image_interaction::{GestureKind, ImageInteraction},
    input::{Modifiers, PhysicalControl, PhysicalEvent, PointerButton},
    note_layout::NoteLines,
};
use tack_core::*;
use winit::event::{ElementState, MouseButton};
type R<T = ()> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

fn fixture(count: usize) -> R<DocumentEditor> {
    let mut doc = Document::new(DocumentId::new(1)?, DocumentLimits::default());
    doc.apply(Command::AddSource(Source::linked(
        SourceId::new(1)?,
        "missing.png",
    )?))?;
    doc.apply(Command::AddAsset(ImageAsset::new(
        AssetId::new(1)?,
        SourceId::new(1)?,
        [6000, 3500],
    )?))?;
    for i in 0..count {
        doc.apply(Command::AddObject {
            object: DocumentObject::image(
                ObjectId::new(i as u128 + 1)?,
                AssetId::new(1)?,
                Transform::new(
                    [(i % 40) as f64 * 16., (i / 40) as f64 * 16.],
                    [10., 8.],
                    if i % 2 == 0 { 0.3 } else { 0. },
                    [false; 2],
                )?,
            ),
            index: doc.object_order().len(),
        })?;
    }
    Ok(DocumentEditor::new(doc, 200))
}
fn add_note(e: &mut DocumentEditor, id: u128, transform: Transform) -> R<ObjectId> {
    let id = ObjectId::new(id)?;
    e.execute(Command::AddObject {
        object: DocumentObject::annotation(
            id,
            Annotation::new(
                AnnotationKind::Text(TextObject::new(
                    "plain bounded wrapping note words ".repeat(10),
                    24.,
                    TextAlignment::Left,
                )?),
                AnnotationStyle::default(),
            ),
            transform,
        )?,
        index: e.document().object_order().len(),
    })?;
    Ok(id)
}
fn note_fixture() -> R<(DocumentEditor, ObjectId)> {
    let mut e = fixture(0)?;
    let id = add_note(
        &mut e,
        1,
        Transform::new([0., 0.], [320., 160.], 0.4, [true, false])?,
    )?;
    // Creation isn't part of the resize transaction under test.
    Ok((DocumentEditor::new(e.document().clone(), 200), id))
}
fn note(e: &DocumentEditor, id: ObjectId) -> R<(&TextObject, AnnotationStyle)> {
    let ObjectKind::Annotation(a) = e.document().object(id).ok_or("note")?.kind() else {
        return Err("annotation".into());
    };
    let AnnotationKind::Text(t) = a.kind() else {
        return Err("text".into());
    };
    Ok((t, a.style()))
}
fn assert_visible(g: &mut ImageGizmo, images: &ImageInteraction, e: &DocumentEditor, c: &Camera) {
    g.build(images, e.document(), c, None);
    let expected: Vec<_> = images
        .selection
        .ids()
        .filter_map(|id| images.preview_transform(e.document(), id))
        .filter(|t| t.bounds().intersects(c.viewport()))
        .collect();
    assert_eq!(g.selection.len(), expected.len());
    for (rect, t) in g.selection.iter().zip(expected) {
        assert_eq!(rect.transform, t);
    }
    assert!(
        g.quads.len() < 32,
        "member borders do not consume the handle/menu budget"
    );
}
#[test]
fn marquee_highlights_every_member_above_overlay_budget_and_clears_on_deselect() -> R {
    let e = fixture(3000)?;
    let before = e.document().clone();
    let mut c = Camera::new([1280, 720]);
    c.set_view([312., 600.], 0.5)?;
    let mut images = ImageInteraction::default();
    images.selection.marquee(
        e.document(),
        WorldRect::new(-10., -10., 650., 1250.)?,
        false,
    );
    assert_eq!(images.selection.len(), 3000);
    let mut g = ImageGizmo::default();
    assert_visible(&mut g, &images, &e, &c);
    assert_eq!(g.selection.len(), 3000);
    c.set_view([100., 100.], 2.)?;
    assert_visible(&mut g, &images, &e, &c);
    assert!(g.selection.len() < 3000);
    assert_eq!(e.document(), &before);
    images.selection.clear();
    g.build(&images, e.document(), &c, None);
    assert!(g.selection.is_empty() && g.quads.is_empty());
    assert_eq!(g.selection.capacity(), 0);
    Ok(())
}
#[test]
fn additive_marquee_group_rotated_annotation_and_immediate_transforms_match_highlights() -> R {
    let mut e = fixture(3)?;
    e.execute(Command::AddGroup(Group::new(
        GroupId::new(1)?,
        vec![ObjectId::new(1)?, ObjectId::new(3)?],
    )?))?;
    let id = add_note(
        &mut e,
        4,
        Transform::new([100., 60.], [40., 30.], 0.5, [false; 2])?,
    )?;
    let mut images = ImageInteraction::default();
    images
        .selection
        .marquee(e.document(), WorldRect::new(-2., -2., 4., 4.)?, false);
    assert_eq!(images.selection.len(), 2);
    images
        .selection
        .marquee(e.document(), WorldRect::new(14., -2., 4., 4.)?, true);
    images
        .selection
        .marquee(e.document(), WorldRect::new(98., 58., 4., 4.)?, true);
    assert_eq!(images.selection.len(), 4);
    assert!(images.selection.contains(id));
    let c = Camera::new([800, 600]);
    let mut g = ImageGizmo::default();
    for kind in [
        GestureKind::Move,
        GestureKind::Resize {
            handle: 4,
            center: false,
        },
        GestureKind::Rotate,
    ] {
        let before = e.document().clone();
        assert_visible(&mut g, &images, &e, &c);
        images.begin(kind, [140., 100.], &e)?;
        images.update([170., 130.])?;
        assert_visible(&mut g, &images, &e, &c);
        images.commit(&mut e)?;
        assert_visible(&mut g, &images, &e, &c);
        e.undo()?;
        assert_eq!(e.document(), &before);
    }
    images.selection.select(None, false);
    assert_visible(&mut g, &images, &e, &c);
    assert!(g.selection.is_empty());
    Ok(())
}
#[test]
fn normal_note_corner_changes_wrapping_only_and_has_one_inverse() -> R {
    let (mut e, id) = note_fixture()?;
    let before = e.document().clone();
    let t = e.document().object(id).ok_or("note")?.transform();
    let mut images = ImageInteraction::default();
    images.selection.select(Some(id), false);
    images.begin(
        GestureKind::Resize {
            handle: 4,
            center: false,
        },
        [0.; 2],
        &e,
    )?;
    images.update(geometry::rotate([-160., 60.], t.rotation()))?;
    let preview = images
        .preview_transform(e.document(), id)
        .ok_or("preview")?;
    assert_eq!(images.preview_note_size(id), Some(24.));
    assert!((preview.size()[0] - 160.).abs() < 1e-9);
    assert!((preview.size()[1] - 220.).abs() < 1e-9);
    let a = geometry::world(t, t.size().map(|s| -s / 2.));
    let b = geometry::world(preview, preview.size().map(|s| -s / 2.));
    assert!((a[0] - b[0]).hypot(a[1] - b[1]) < 1e-9);
    assert_eq!(e.document(), &before);
    images.commit(&mut e)?;
    assert_eq!(e.undo_len(), 1);
    let (text, style) = note(&e, id)?;
    assert_eq!(text.font_size(), 24.);
    assert_eq!(style, AnnotationStyle::default());
    assert!(
        NoteLines::new(text.value(), 150., 24.).count()
            > NoteLines::new(text.value(), 310., 24.).count()
    );
    let after = e.document().clone();
    e.undo()?;
    assert_eq!(e.document(), &before);
    e.redo()?;
    assert_eq!(e.document(), &after);
    Ok(())
}
#[test]
fn shift_note_scale_preview_uses_initial_values_and_commits_transform_text_style_once() -> R {
    let (mut e, id) = note_fixture()?;
    let before = e.document().clone();
    let t = e.document().object(id).ok_or("note")?.transform();
    let mut images = ImageInteraction::default();
    images.selection.select(Some(id), false);
    images.begin(GestureKind::NoteScale { handle: 4 }, [0.; 2], &e)?;
    let delta = geometry::rotate([160., 80.], t.rotation());
    images.update(delta)?;
    let preview = images
        .preview_transform(e.document(), id)
        .ok_or("preview")?;
    assert_eq!(images.preview_note_size(id), Some(36.));
    assert_eq!(images.preview_style(id).ok_or("style")?.width(), 4.5);
    for i in 0..300 {
        images.update(geometry::rotate(
            [i as f64 * 0.5, i as f64 * 0.25],
            t.rotation(),
        ))?;
    }
    images.update(delta)?;
    assert_eq!(images.preview_transform(e.document(), id), Some(preview));
    assert_eq!(images.preview_note_size(id), Some(36.));
    let mut scene = AnnotationScene::default();
    scene.build(
        e.document(),
        &images,
        &AnnotationInput::default(),
        &Camera::new([1280, 720]),
        &[],
    );
    assert!(
        scene
            .primitives
            .iter()
            .any(|p| p.kind == 6 && p.size == [36., 36.])
    );
    assert_eq!(e.document(), &before);
    images.commit(&mut e)?;
    assert_eq!(e.undo_len(), 1);
    let (text, style) = note(&e, id)?;
    assert_eq!(text.font_size(), 36.);
    assert_eq!(style.width(), 4.5);
    let after = e.document().clone();
    e.undo()?;
    assert_eq!(e.document(), &before);
    e.redo()?;
    assert_eq!(e.document(), &after);
    for delta in [1e6, -1e6] {
        images.begin(GestureKind::NoteScale { handle: 4 }, [0.; 2], &e)?;
        images.update(geometry::rotate([delta, delta], t.rotation()))?;
        let size = images.preview_note_size(id).ok_or("size")?;
        let style = images.preview_style(id).ok_or("style")?;
        assert!((4. ..=256.).contains(&size));
        assert!((0.1..=256.).contains(&style.width()));
        images.cancel();
    }
    Ok(())
}
#[test]
fn physical_shift_corner_scales_note_and_normal_corner_leaves_font_size() -> R {
    for shift in [false, true] {
        let (mut e, id) = note_fixture()?;
        let mut input = ImageInput::new()?;
        input.images.selection.select(Some(id), false);
        let mut c = Camera::new([1280, 720]);
        let t = e.document().object(id).ok_or("note")?.transform();
        let p = c.world_to_screen(input.gizmo.handle(t, &c, 4));
        input.cursor_moved(p, &e, &mut c)?;
        if shift {
            input.physical(PhysicalEvent::Modifiers(Modifiers::SHIFT), &mut e, &mut c)?;
        }
        let button = |state| PhysicalEvent::Button {
            control: PhysicalControl::Pointer(PointerButton::Mouse(MouseButton::Left)),
            state,
            repeat: false,
        };
        input.physical(button(ElementState::Pressed), &mut e, &mut c)?;
        let delta = geometry::rotate([160., 80.], t.rotation());
        input.cursor_moved([p[0] + delta[0], p[1] + delta[1]], &e, &mut c)?;
        input.physical(button(ElementState::Released), &mut e, &mut c)?;
        assert_eq!(e.undo_len(), 1);
        assert_eq!(note(&e, id)?.0.font_size(), if shift { 36. } else { 24. });
    }
    Ok(())
}
#[test]
fn arrow_default_shaft_and_head_are_stronger_with_bounded_user_width() -> R {
    let old = AnnotationStyle::default();
    let arrow = Creation::new(Tool::Arrow, [0.; 2], old, 0, 1.);
    assert_eq!(arrow.style.width(), 4.);
    let head = tack_app::annotation_geometry::arrow_head([0., 0.], [100., 0.], arrow.style.width());
    assert_eq!(head[1][0], 76.);
    let edited = AnnotationStyle::new(old.stroke(), old.fill(), 10., old.opacity())?;
    assert_eq!(
        Creation::new(Tool::Arrow, [0.; 2], edited, 0, 1.).style,
        edited
    );
    assert_eq!(Creation::new(Tool::Line, [0.; 2], old, 0, 1.).style, old);
    Ok(())
}

#[test]
fn tiled_image_draws_keep_all_quads_and_annotation_document_order() -> R {
    let mut e = fixture(1)?;
    add_note(
        &mut e,
        2,
        Transform::new([30., 30.], [80., 40.], 0., [false; 2])?,
    )?;
    e.execute(Command::AddObject {
        object: DocumentObject::image(
            ObjectId::new(3)?,
            AssetId::new(1)?,
            Transform::new([100., 0.], [40., 40.], 0., [false; 2])?,
        ),
        index: e.document().object_order().len(),
    })?;
    let packet = |id| -> R<tack_render::DrawProductImage> {
        Ok(tack_render::DrawProductImage {
            data: e
                .document()
                .object_render_data(ObjectId::new(id)?)
                .ok_or("image data")?,
            key: None,
        })
    };
    let draws = [packet(1)?, packet(1)?, packet(1)?, packet(3)?, packet(3)?];
    let mut scene = AnnotationScene::default();
    scene.build(
        e.document(),
        &ImageInteraction::default(),
        &AnnotationInput::default(),
        &Camera::new([800, 600]),
        &draws,
    );
    assert_eq!(
        &scene.order[..3],
        &[
            tack_render::CanvasDraw::Image(0),
            tack_render::CanvasDraw::Image(1),
            tack_render::CanvasDraw::Image(2)
        ]
    );
    assert!(matches!(
        scene.order[3],
        tack_render::CanvasDraw::Annotations { .. }
    ));
    assert_eq!(
        &scene.order[4..],
        &[
            tack_render::CanvasDraw::Image(3),
            tack_render::CanvasDraw::Image(4)
        ]
    );
    Ok(())
}

#[test]
fn scaled_note_roundtrip_and_later_text_size_edits_remain_explicit_and_deterministic() -> R {
    use tack_app::{
        actions::{Action, ActionEvent, ActionPhase},
        annotation_tool::StyleAction,
    };
    let (mut e, id) = note_fixture()?;
    let mut images = ImageInteraction::default();
    images.selection.select(Some(id), false);
    let t = e.document().object(id).ok_or("note")?.transform();
    images.begin(GestureKind::NoteScale { handle: 4 }, [0.; 2], &e)?;
    images.update(geometry::rotate([160., 80.], t.rotation()))?;
    images.commit(&mut e)?;
    let root = std::env::temp_dir().join(format!(
        "tack-scaled-note-{:032x}",
        tack_storage::new_document_id()?.value()
    ));
    std::fs::create_dir(&root)?;
    let path = root.join("scaled.tack");
    tack_storage::save(&path, e.document(), vec![])?;
    let reopened = tack_storage::TackFile::open(&path)?.document;
    std::fs::remove_dir_all(&root)?;
    assert_eq!(&reopened, e.document());
    let mut reopened = DocumentEditor::new(reopened, 200);
    let mut input = ImageInput::new()?;
    input.images.selection.select(Some(id), false);
    let mut camera = Camera::new([800, 600]);
    for _ in 0..10 {
        input.dispatch(
            ActionEvent {
                action: Action::AnnotationStyle(StyleAction::LargerText),
                phase: ActionPhase::Invoke,
            },
            &mut reopened,
            &mut camera,
        )?;
        assert_eq!(note(&reopened, id)?.0.font_size(), 45.);
        input.dispatch(
            ActionEvent {
                action: Action::AnnotationStyle(StyleAction::SmallerText),
                phase: ActionPhase::Invoke,
            },
            &mut reopened,
            &mut camera,
        )?;
        assert_eq!(note(&reopened, id)?.0.font_size(), 36.);
    }
    assert_eq!(note(&reopened, id)?.1.width(), 4.5);
    assert_eq!(
        reopened.document().object(id).ok_or("note")?.transform(),
        e.document().object(id).ok_or("note")?.transform()
    );
    Ok(())
}
