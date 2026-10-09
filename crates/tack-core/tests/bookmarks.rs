use tack_core::*;

type R<T = ()> = Result<T, Box<dyn std::error::Error>>;

fn bookmark(id: u128, name: &str, center: [f64; 2], zoom: f64) -> R<CameraBookmark> {
    Ok(CameraBookmark::new(
        BookmarkId::new(id)?,
        name.into(),
        center,
        zoom,
    )?)
}

#[test]
fn bookmark_bounds_are_utf8_bytes_and_camera_jump_is_exact() -> R {
    let id = BookmarkId::new(1)?;
    assert!(BookmarkId::new(0).is_err());
    for name in ["", " \t ", "bad\nname", "bad\0name", "bad\u{7f}name"] {
        assert!(CameraBookmark::new(id, name.into(), [0.; 2], 1.).is_err());
    }
    assert!(CameraBookmark::new(id, "é".repeat(64), [1e8, -1e8], 0.001).is_ok());
    assert!(CameraBookmark::new(id, "é".repeat(65), [0.; 2], 1.).is_err());
    assert!(CameraBookmark::new(id, "x".repeat(MAX_BOOKMARK_NAME_BYTES), [0.; 2], 64.).is_ok());
    for center in [
        [f64::NAN, 0.],
        [0., f64::INFINITY],
        [1e8 + 1., 0.],
        [0., -1e8 - 1.],
    ] {
        assert!(CameraBookmark::new(id, "view".into(), center, 1.).is_err());
    }
    for zoom in [f64::NAN, f64::INFINITY, -1., 0., 0.0009, 64.001] {
        assert!(CameraBookmark::new(id, "view".into(), [0.; 2], zoom).is_err());
    }
    let view = bookmark(1, "Detail 猫", [123.125, -987.75], 6.53125)?;
    let mut camera = Camera::new([800, 600]);
    view.jump(&mut camera)?;
    assert_eq!(camera.zoom(), view.zoom());
    assert_eq!(camera.screen_to_world([400., 300.]), view.center());
    Ok(())
}

#[test]
fn bookmark_list_rejects_overflow_duplicate_ids_and_batch_failure_atomically() -> R {
    let doc = Document::new(DocumentId::new(1)?, DocumentLimits::default());
    let mut editor = DocumentEditor::new(doc, 200);
    let views: Vec<_> = (1..=MAX_CAMERA_BOOKMARKS)
        .map(|id| bookmark(id as u128, "view", [id as f64, 0.], 1.))
        .collect::<R<_>>()?;
    assert!(editor.execute(Command::SetCameraBookmarks(views.clone()))?);
    let before = editor.document().clone();
    let generation = editor.generation();
    let undo_len = editor.undo_len();
    assert!(!editor.execute(Command::SetCameraBookmarks(views.clone()))?);
    assert_eq!(editor.generation(), generation);
    assert_eq!(editor.undo_len(), undo_len);
    let mut too_many = views.clone();
    too_many.push(bookmark(100, "extra", [0.; 2], 1.)?);
    let duplicate = vec![views[0].clone(), views[0].clone()];
    for invalid in [too_many, duplicate] {
        assert!(
            editor
                .execute(Command::SetCameraBookmarks(invalid))
                .is_err()
        );
        assert_eq!(editor.document(), &before);
        assert_eq!(editor.undo_len(), undo_len);
        assert_eq!(editor.generation(), generation);
    }
    assert!(
        editor
            .execute(Command::Batch(vec![
                Command::SetCameraBookmarks(Vec::new()),
                Command::RemoveObject(ObjectId::new(999)?),
            ]))
            .is_err()
    );
    assert_eq!(editor.document(), &before);
    assert_eq!(editor.generation(), generation);
    editor.undo()?;
    assert!(editor.document().bookmarks().is_empty());
    editor.redo()?;
    assert_eq!(editor.document(), &before);
    Ok(())
}

#[test]
fn rename_reorder_delete_bookmarks_do_not_touch_frame_or_object_order() -> R {
    let mut doc = Document::new(DocumentId::new(1)?, DocumentLimits::default());
    let frame = DocumentObject::frame(
        ObjectId::new(5)?,
        "Frame".into(),
        Transform::new([10., 20.], [100., 80.], 0., [false; 2])?,
    )?;
    doc.apply(Command::AddObject {
        object: frame.clone(),
        index: 0,
    })?;
    let mut editor = DocumentEditor::new(doc, 200);
    let a = bookmark(1, "Overview", [0.; 2], 1.)?;
    let b = bookmark(2, "Detail", [30., 40.], 10.)?;
    editor.execute(Command::SetCameraBookmarks(vec![a.clone(), b.clone()]))?;
    let original = editor.document().clone();
    let renamed = CameraBookmark::new(a.id(), "Renamed".into(), a.center(), a.zoom())?;
    editor.execute(Command::SetCameraBookmarks(vec![b.clone(), renamed]))?;
    let reordered = editor.document().clone();
    editor.execute(Command::SetCameraBookmarks(vec![b]))?;
    assert_eq!(editor.document().object(frame.id()), Some(&frame));
    assert_eq!(editor.document().object_order(), &[frame.id()]);
    assert_eq!(editor.document().frame_count(), 1);
    editor.undo()?;
    assert_eq!(editor.document(), &reordered);
    editor.undo()?;
    assert_eq!(editor.document(), &original);
    Ok(())
}
