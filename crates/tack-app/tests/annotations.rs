use std::{fs, path::Path};
use tack_app::{
    actions::{Action, ActionEvent, ActionPhase, Tool},
    annotation_geometry as geometry,
    annotation_tool::{Creation, NoteEdit},
    image_input::ImageInput,
    image_interaction::{GestureKind, ImageInteraction},
    source_actions::{SourceOperation, SourceRequest},
};
use tack_core::*;
use winit::event::ElementState::{Pressed, Released};
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
fn transform() -> Result<Transform> {
    Ok(Transform::new([0., 0.], [100., 80.], 0., [false; 2])?)
}
fn editor() -> Result<DocumentEditor> {
    Ok(DocumentEditor::new(
        Document::new(DocumentId::new(1)?, DocumentLimits::default()),
        200,
    ))
}
fn event(action: Action, phase: ActionPhase) -> ActionEvent {
    ActionEvent { action, phase }
}
#[test]
fn creation_capture_cancel_pan_priority_and_one_exact_inverse() -> Result {
    let mut e = editor()?;
    let mut input = ImageInput::new()?;
    let mut camera = Camera::new([1280, 720]);
    for tool in [
        Tool::Rectangle,
        Tool::Ellipse,
        Tool::Line,
        Tool::Arrow,
        Tool::Scribble,
    ] {
        input.dispatch(
            event(Action::SelectTool(tool), ActionPhase::Invoke),
            &mut e,
            &mut camera,
        )?;
        input.cursor_moved([300., 250.], &e, &mut camera)?;
        input.physical(button(Pressed), &mut e, &mut camera)?;
        for i in 1..=1000 {
            input.cursor_moved(
                [300. + i as f64 * 0.1, 250. + (i as f64 * 0.02).sin() * 20.],
                &e,
                &mut camera,
            )?;
        }
        assert_eq!(e.undo_len(), 0);
        assert_eq!(e.document().annotation_count(), 0);
        input.dispatch(
            event(Action::CancelInteraction, ActionPhase::Invoke),
            &mut e,
            &mut camera,
        )?;
        assert!(input.annotation.creation.is_none());
        input.physical(button(Released), &mut e, &mut camera)?;
        assert_eq!(e.undo_len(), 0);
        input.cursor_moved([300., 250.], &e, &mut camera)?;
        input.physical(button(Pressed), &mut e, &mut camera)?;
        input.cursor_moved([400., 300.], &e, &mut camera)?;
        input.physical(button(Released), &mut e, &mut camera)?;
        assert_eq!(e.undo_len(), 1);
        assert_eq!(e.document().annotation_count(), 1);
        e.undo()?;
        assert_eq!(e.document().annotation_count(), 0);
        input.physical(
            tack_app::input::PhysicalEvent::Button {
                control: tack_app::input::PhysicalControl::Pointer(
                    tack_app::input::PointerButton::Mouse(winit::event::MouseButton::Middle),
                ),
                state: Pressed,
                repeat: false,
            },
            &mut e,
            &mut camera,
        )?;
        assert!(input.annotation.creation.is_none());
        input.cancel();
        input.physical(
            tack_app::input::PhysicalEvent::FocusLost,
            &mut e,
            &mut camera,
        )?;
    }
    let mut stroke = Creation::new(Tool::Scribble, [0., 0.], AnnotationStyle::default(), 0, 0.1);
    for i in 1..10000 {
        stroke.update([i as f64, 1.]);
    }
    assert_eq!(stroke.points.len(), MAX_STROKE_POINTS);
    assert!(stroke.capped);
    assert_eq!(stroke.points.last(), Some(&[9999., 1.]));
    Ok(())
}
#[test]
fn editable_utf8_notes_and_mixed_transforms_are_atomic() -> Result {
    let mut e = editor()?;
    let id = ObjectId::new(1)?;
    let mut note = NoteEdit {
        id,
        value: "été 猫".into(),
        size: 24.,
        alignment: TextAlignment::Left,
        transform: transform()?,
        style: AnnotationStyle::default(),
        generation: e.generation(),
        is_new: true,
        replace: false,
        composing: false,
    };
    note.backspace();
    assert_eq!(note.value, "été ");
    note.insert("猫\n");
    note.insert("\0bad");
    assert_eq!(e.undo_len(), 0);
    note.finish(&mut e)?;
    assert_eq!(e.undo_len(), 1);
    let before = e.document().clone();
    let mut input = ImageInput::new()?;
    let mut camera = Camera::new([1280, 720]);
    input.images.selection.select(Some(id), false);
    input.dispatch(
        event(Action::RenameFrame, ActionPhase::Invoke),
        &mut e,
        &mut camera,
    )?;
    input
        .annotation
        .edit
        .as_mut()
        .ok_or("note edit")?
        .insert("replaced λ");
    input.cancel();
    assert_eq!(e.document(), &before);
    input.dispatch(
        event(Action::RenameFrame, ActionPhase::Invoke),
        &mut e,
        &mut camera,
    )?;
    let mut edit = input.annotation.edit.take().ok_or("note edit")?;
    edit.insert("replaced λ");
    edit.finish(&mut e)?;
    assert_eq!(e.undo_len(), 2);
    e.undo()?;
    assert_eq!(e.document(), &before);
    e.execute(Command::AddSource(Source::linked(
        SourceId::new(1)?,
        "missing.png",
    )?))?;
    e.execute(Command::AddAsset(ImageAsset::new(
        AssetId::new(1)?,
        SourceId::new(1)?,
        [100, 80],
    )?))?;
    e.execute(Command::AddObject {
        object: DocumentObject::image(ObjectId::new(2)?, AssetId::new(1)?, transform()?),
        index: 1,
    })?;
    for kind in [
        GestureKind::Move,
        GestureKind::Resize {
            handle: 4,
            center: false,
        },
        GestureKind::Rotate,
        GestureKind::Opacity,
    ] {
        let before = e.document().clone();
        let history = e.undo_len();
        let mut images = ImageInteraction::default();
        images.selection.select_all(e.document());
        images.begin(kind, [40., 20.], &e)?;
        images.update([-60., 45.])?;
        assert_eq!(e.document(), &before);
        images.commit(&mut e)?;
        assert_eq!(e.undo_len(), history + 1);
        e.undo()?;
        assert_eq!(e.document(), &before);
    }
    Ok(())
}
#[test]
fn hollow_shape_line_arrow_hits_respect_visible_geometry() -> Result {
    let t = transform()?;
    let style = AnnotationStyle::default();
    let rect = Annotation::new(AnnotationKind::Rect, style);
    assert!(!geometry::hit(&rect, t, [0., 0.], 0.));
    assert!(geometry::hit(&rect, t, [50., 0.], 0.));
    assert!(!geometry::intersects(
        &rect,
        t,
        WorldRect::new(-5., -5., 10., 10.)?
    ));
    let line = Annotation::new(
        AnnotationKind::Line(LineObject::new([[0., 0.], [1., 1.]])?),
        style,
    );
    assert!(geometry::hit(&line, t, [0., 0.], 0.));
    assert!(!geometry::hit(&line, t, [40., -30.], 0.));
    assert!(!geometry::intersects(
        &line,
        t,
        WorldRect::new(35., -35., 5., 5.)?
    ));
    let arrow = Annotation::new(
        AnnotationKind::Arrow(LineObject::new([[0., 0.5], [1., 0.5]])?),
        style,
    );
    let points = geometry::arrow_head([-50., 0.], [50., 0.], style.width());
    let p = [
        (points[0][0] + points[1][0] + points[2][0]) / 3.,
        (points[0][1] + points[1][1] + points[2][1]) / 3.,
    ];
    assert!(geometry::hit(&arrow, t, p, 0.));
    Ok(())
}
fn source_doc(location: SourceLocation) -> Result<Document> {
    let mut d = Document::new(DocumentId::new(1)?, DocumentLimits::default());
    d.apply(Command::AddSource(Source::from_descriptor(
        SourceId::new(1)?,
        location,
        1,
        None,
    )?))?;
    d.apply(Command::AddAsset(ImageAsset::new(
        AssetId::new(1)?,
        SourceId::new(1)?,
        [1, 1],
    )?))?;
    d.apply(Command::AddObject {
        object: DocumentObject::image(ObjectId::new(1)?, AssetId::new(1)?, transform()?),
        index: 0,
    })?;
    Ok(d)
}
#[test]
fn linked_source_invocation_keeps_untrusted_path_as_one_native_argument() -> Result {
    let root = std::env::temp_dir().join(format!(
        "tack-source-test-{:032x}",
        tack_storage::new_document_id()?.value()
    ));
    fs::create_dir(&root)?;
    let path = root.join("-evil $(touch injected);.png");
    fs::write(&path, b"\x89PNG\r\n\x1a\n")?;
    let d = source_doc(SourceLocation::Linked(LinkedPath::native(&path)?))?;
    for op in [
        SourceOperation::Open,
        SourceOperation::Reveal,
        SourceOperation::CopyPath,
    ] {
        let request = SourceRequest::new(
            &d,
            ObjectId::new(1)?,
            Path::new("nonexistent-board.tack"),
            op,
        )?;
        let invocation = request.invocation()?;
        if op == SourceOperation::CopyPath {
            assert!(invocation.stdin_bytes().is_some());
            assert!(
                !invocation
                    .args()
                    .iter()
                    .any(|s| s.to_string_lossy().contains("evil"))
            );
        } else {
            assert_eq!(
                invocation.args().last(),
                Some(&if op == SourceOperation::Open {
                    path.canonicalize()?.into_os_string()
                } else {
                    root.canonicalize()?.into_os_string()
                })
            );
        }
        assert!(!invocation.executable().to_string_lossy().contains("evil"));
    }
    let board = root.join("board.tack");
    fs::write(&board, [])?;
    let relative = source_doc(SourceLocation::Linked(LinkedPath::native(Path::new(
        "-evil $(touch injected);.png",
    ))?))?;
    assert_eq!(
        SourceRequest::new(&relative, ObjectId::new(1)?, &board, SourceOperation::Open)?
            .resolve()?,
        path.canonicalize()?
    );
    let embedded = source_doc(SourceLocation::Embedded)?;
    assert!(
        SourceRequest::new(&embedded, ObjectId::new(1)?, &board, SourceOperation::Open)?
            .resolve()
            .is_err()
    );
    let foreign = if cfg!(windows) {
        LinkedPath::encoded(PathPlatform::Unix, true, b"/tmp/source.png")?
    } else {
        LinkedPath::encoded(
            PathPlatform::Windows,
            true,
            &"C:\\source.png"
                .encode_utf16()
                .flat_map(u16::to_le_bytes)
                .collect::<Vec<_>>(),
        )?
    };
    assert!(
        SourceRequest::new(
            &source_doc(SourceLocation::Linked(foreign))?,
            ObjectId::new(1)?,
            &board,
            SourceOperation::Open
        )?
        .resolve()
        .is_err()
    );
    fs::write(&path, b"[Desktop Entry]\nExec=touch injected")?;
    assert!(
        SourceRequest::new(&d, ObjectId::new(1)?, &board, SourceOperation::Open)?
            .resolve()
            .is_err()
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::write(&path, b"\x89PNG\r\n\x1a\n")?;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700))?;
        assert!(
            SourceRequest::new(&d, ObjectId::new(1)?, &board, SourceOperation::Open)?
                .resolve()
                .is_err()
        );
        let link = root.join("link.png");
        std::os::unix::fs::symlink(&path, &link)?;
        let d = source_doc(SourceLocation::Linked(LinkedPath::native(&link)?))?;
        assert!(
            SourceRequest::new(&d, ObjectId::new(1)?, &board, SourceOperation::Open)?
                .resolve()
                .is_err()
        );
    }
    fs::remove_file(&path)?;
    assert!(
        SourceRequest::new(&d, ObjectId::new(1)?, &board, SourceOperation::Open)?
            .resolve()
            .is_err()
    );
    assert!(!root.join("injected").exists());
    fs::remove_dir_all(root)?;
    Ok(())
}

fn button(state: winit::event::ElementState) -> tack_app::input::PhysicalEvent {
    tack_app::input::PhysicalEvent::Button {
        control: tack_app::input::PhysicalControl::Pointer(tack_app::input::PointerButton::Mouse(
            winit::event::MouseButton::Left,
        )),
        state,
        repeat: false,
    }
}
#[test]
fn opacity_preview_and_saturated_scene_keep_transients_visible() -> Result {
    use tack_app::{annotation_scene::AnnotationScene, annotation_tool::AnnotationInput};
    let mut e = editor()?;
    let t = transform()?;
    let id = ObjectId::new(1)?;
    e.execute(Command::AddObject {
        object: DocumentObject::annotation(
            id,
            Annotation::new(AnnotationKind::Rect, AnnotationStyle::default()),
            t,
        )?,
        index: 0,
    })?;
    let camera = Camera::new([1280, 720]);
    let before = e.document().clone();
    let mut images = ImageInteraction::default();
    images.selection.select(Some(id), false);
    images.begin(GestureKind::Opacity, [0., 0.], &e)?;
    images.update([-50., 0.])?;
    let mut scene = AnnotationScene::default();
    scene.build(
        e.document(),
        &images,
        &AnnotationInput::default(),
        &camera,
        &[],
    );
    assert!((scene.primitives[0].opacity - 0.5).abs() < 1e-6);
    assert_eq!(e.document(), &before);
    images.cancel();
    scene.build(
        e.document(),
        &images,
        &AnnotationInput::default(),
        &camera,
        &[],
    );
    assert_eq!(scene.primitives[0].opacity, 1.);
    let points: Vec<_> = (0..4096)
        .map(|i| [i as f64 / 4095., (i % 2) as f64])
        .collect();
    for i in 2..=10 {
        e.execute(Command::AddObject {
            object: DocumentObject::annotation(
                ObjectId::new(i)?,
                Annotation::new(
                    AnnotationKind::Scribble(ScribbleObject::new(points.clone())?),
                    AnnotationStyle::default(),
                ),
                t,
            )?,
            index: e.document().object_order().len(),
        })?;
    }
    let mut input = AnnotationInput::default();
    let mut c = Creation::new(
        Tool::Scribble,
        [0., 0.],
        AnnotationStyle::default(),
        e.generation(),
        0.1,
    );
    c.update([10., 10.]);
    input.creation = Some(Box::new(c));
    scene.build(
        e.document(),
        &ImageInteraction::default(),
        &input,
        &camera,
        &[],
    );
    assert!(scene.omitted > 0);
    assert!(scene.primitives.len() <= tack_render::MAX_ANNOTATION_PRIMITIVES);
    assert_eq!(
        scene.order.last(),
        Some(&tack_render::CanvasDraw::Annotations { start: 0, end: 1 })
    );
    input.creation = None;
    input.edit = Some(Box::new(NoteEdit {
        id: ObjectId::new(100)?,
        value: "NEW note".into(),
        size: 24.,
        alignment: TextAlignment::Left,
        transform: t,
        style: AnnotationStyle::default(),
        generation: e.generation(),
        is_new: true,
        replace: false,
        composing: false,
    }));
    scene.build(
        e.document(),
        &ImageInteraction::default(),
        &input,
        &camera,
        &[],
    );
    assert!(scene.glyphs > 0);
    assert!(matches!(
        scene.order.last(),
        Some(tack_render::CanvasDraw::Annotations { start: 0, .. })
    ));
    Ok(())
}
#[cfg(unix)]
#[test]
fn relative_source_uses_same_lexical_board_parent_as_display_pipeline() -> Result {
    let root = std::env::temp_dir().join(format!(
        "tack-source-symlink-{:032x}",
        tack_storage::new_document_id()?.value()
    ));
    fs::create_dir(&root)?;
    let real = root.join("real");
    let links = root.join("links");
    fs::create_dir(&real)?;
    fs::create_dir(&links)?;
    fs::write(real.join("board.tack"), [])?;
    fs::write(real.join("image.png"), b"\x89PNG\r\n\x1a\nreal")?;
    fs::write(links.join("image.png"), b"\x89PNG\r\n\x1a\nlinks")?;
    let board = links.join("board.tack");
    std::os::unix::fs::symlink(real.join("board.tack"), &board)?;
    let d = source_doc(SourceLocation::Linked(LinkedPath::native(Path::new(
        "image.png",
    ))?))?;
    let request = SourceRequest::new(&d, ObjectId::new(1)?, &board, SourceOperation::Open)?;
    assert_eq!(request.resolve()?, links.join("image.png").canonicalize()?);
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn thick_high_aspect_ellipse_hits_closest_contour_not_radial_approximation() -> Result {
    let style = AnnotationStyle::new(Color([255; 4]), None, 30., Opacity::OPAQUE)?;
    let a = Annotation::new(AnnotationKind::Ellipse, style);
    let t = Transform::new([0.; 2], [200., 20.], 0., [false; 2])?;
    assert!(geometry::hit(&a, t, [20., 0.], 0.));
    assert!((geometry::ellipse_distance([20., 0.], [100., 10.]) - 9.795897).abs() < 1e-5);
    assert!(geometry::hit(&a, t, [0., 0.], 0.));
    assert!(!geometry::hit(&a, t, [0., 26.], 0.));
    Ok(())
}
