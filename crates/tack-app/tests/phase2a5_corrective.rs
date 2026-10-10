use std::{
    fs,
    sync::{Arc, atomic::AtomicBool},
};
use tack_app::{
    camera_slots::{self, View},
    independent_copy,
    preferences::{self, Preferences},
    toolbar::{Placement, Toolbar},
};
use tack_core::*;
use tack_storage::*;
type R<T = ()> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
struct Root(std::path::PathBuf);
impl Root {
    fn new() -> R<Self> {
        let p = std::env::temp_dir().join(format!("tack-2a5-{:032x}", new_document_id()?.value()));
        fs::create_dir(&p)?;
        Ok(Self(p))
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn multi_window_preferences_merge_recent_and_distinct_settings_but_refuse_same_setting() -> R {
    let root = Root::new()?;
    let base = Preferences::defaults()?;
    let saved = preferences::save_profile_merged(
        &root.0,
        base.clone(),
        &base,
        Some(&root.0.join("a.tack")),
    )?;
    let mut a = base.clone();
    a.toolbar.placement = Placement::Left;
    let saved_a = preferences::save_profile_merged(&root.0, a.clone(), &base, None)?;
    assert_eq!(saved_a.saved.recent, saved.saved.recent);
    let mut b = base.clone();
    b.grid = true;
    let merged = preferences::save_profile_merged(&root.0, b.clone(), &base, None)?;
    assert!(merged.saved.grid);
    assert_eq!(merged.saved.toolbar.placement, Placement::Left);
    b.toolbar.placement = Placement::Bottom;
    assert!(preferences::save_profile_merged(&root.0, b, &base, None).is_err());
    assert_eq!(
        preferences::read(&root.0.join("preferences.json"))?
            .toolbar
            .placement,
        Placement::Left
    );
    Ok(())
}
#[test]
fn camera_slots_remain_local_bounded_merge_independent_boards_and_exports_are_separate() -> R {
    let root = Root::new()?;
    let mut p = Preferences::defaults()?;
    let view = View {
        board: format!("{:032x}", 1),
        slot: 7,
        center: [12., 34.],
        zoom: 0.01,
    };
    camera_slots::assign(&mut p.local_views, view.clone())?;
    let mut other = view.clone();
    other.slot = 8;
    assert_eq!(camera_slots::merge(&[], &p.local_views, &[other])?.len(), 2);
    preferences::export_preferences(&root.0.join("prefs.json"), &p)?;
    let data: serde_json::Value = serde_json::from_slice(&fs::read(root.0.join("prefs.json"))?)?;
    for field in ["keymap", "recent", "local_views"] {
        assert!(data.get(field).is_none());
    }
    preferences::export_keymap(&root.0.join("map.tackey"), &p)?;
    let data: serde_json::Value = serde_json::from_slice(&fs::read(root.0.join("map.tackey"))?)?;
    assert!(data.get("bindings").is_some());
    assert!(data.get("toolbar").is_none());
    assert_eq!(
        preferences::read_keymap(&root.0.join("map.tackey"))?.keymap,
        p.keymap
    );
    preferences::write(&root.0.join("legacy.json"), &p)?;
    assert_eq!(
        preferences::read_keymap(&root.0.join("legacy.json"))?.keymap,
        p.keymap
    );
    for place in Placement::ALL {
        p.toolbar.placement = place;
        for _ in 0..20 {
            Toolbar::toggle(&mut p);
            Toolbar::toggle(&mut p);
        }
        assert_eq!(p.toolbar.placement, place);
    }
    Ok(())
}
#[test]
fn frame_color_schema_roundtrip_undo_and_malformed_authority_are_safe() -> R {
    let mut e = DocumentEditor::new(
        Document::new(DocumentId::new(1)?, DocumentLimits::default()),
        100,
    );
    let id = ObjectId::new(1)?;
    e.execute(Command::AddObject {
        object: DocumentObject::frame(
            id,
            "Wrapped title".into(),
            Transform::new([0.; 2], [100., 50.], 0., [false; 2])?,
        )?,
        index: 0,
    })?;
    let gray = e.document().clone();
    let color = Color([250, 20, 150, 255]);
    e.execute(Command::SetFrameColor { object: id, color })?;
    let doc = e.document().clone();
    let (schema, counts, bytes) = encode_metadata(&doc)?;
    assert_eq!(schema, 5);
    assert_eq!(decode_metadata(schema, counts, &bytes)?, doc);
    e.undo()?;
    assert_eq!(*e.document(), gray);
    e.redo()?;
    assert_eq!(*e.document(), doc);
    let root = Root::new()?;
    save(root.0.join("frame.tack"), &doc, vec![])?;
    assert_eq!(TackFile::open(root.0.join("frame.tack"))?.document, doc);
    Ok(())
}
#[test]
fn local_fork_streams_embedded_originals_preserves_identity_and_refuses_corruption() -> R {
    let root = Root::new()?;
    let mut doc = Document::new(DocumentId::new(1)?, DocumentLimits::default());
    let source = SourceId::new(1)?;
    doc.apply(Command::AddSource(Source::embedded(source)))?;
    doc.apply(Command::AddAsset(ImageAsset::new(
        AssetId::new(1)?,
        source,
        [100, 100],
    )?))?;
    doc.apply(Command::AddObject {
        object: DocumentObject::image(
            ObjectId::new(1)?,
            AssetId::new(1)?,
            Transform::new([0.; 2], [100.; 2], 0., [false; 2])?,
        ),
        index: 0,
    })?;
    let path = root.0.join("shared.tack");
    let payload = b"opaque encoded image bytes";
    let original = root.0.join("original.bin");
    fs::write(&original, payload)?;
    save(
        &path,
        &doc,
        vec![BlobInput::original(source, 1, Payload::File(original))],
    )?;
    let board = Arc::new(TackFile::open(&path)?);
    let target = root.0.join("local.tack");
    let request = |target| independent_copy::Request {
        board_path: path.clone(),
        document: doc.clone(),
        board: board.clone(),
        sources: vec![],
        cache: None,
        server: None,
        target,
    };
    let opened = request(target.clone()).run(&AtomicBool::new(false))?;
    assert_ne!(opened.board.document.id(), doc.id());
    assert!(opened.sharing.is_none());
    assert_eq!(opened.board.document.object_order(), doc.object_order());
    let SourceLocation::Linked(link) = opened
        .board
        .document
        .source(source)
        .ok_or("source")?
        .location()
    else {
        return Err("not independent".into());
    };
    assert_eq!(
        fs::read(root.0.join(link.to_native().ok_or("native")?))?,
        payload
    );
    assert!(request(target).run(&AtomicBool::new(false)).is_err());
    let range = board.originals.get(&source).ok_or("payload")?.range;
    use std::io::{Seek, SeekFrom, Write};
    let mut f = fs::OpenOptions::new().write(true).open(&path)?;
    f.seek(SeekFrom::Start(range.offset))?;
    f.write_all(b"X")?;
    f.sync_all()?;
    let failed = root.0.join("failed.tack");
    assert!(
        request(failed.clone())
            .run(&AtomicBool::new(false))
            .is_err()
    );
    assert!(!failed.exists());
    assert!(!root.0.join("failed.tack.assets").exists());
    Ok(())
}
#[test]
fn rectangles_drag_all_directions_transform_duplicate_filter_and_style_defaults() -> R {
    use tack_app::{
        actions::{Action, ActionEvent, ActionPhase, Tool},
        annotation_tool::StyleAction,
        image_input::ImageInput,
        image_interaction::GestureKind,
        input::{PhysicalControl, PhysicalEvent, PointerButton},
    };
    use winit::event::{ElementState, MouseButton};
    let mut e = DocumentEditor::new(
        Document::new(DocumentId::new(1)?, DocumentLimits::default()),
        100,
    );
    let mut input = ImageInput::new()?;
    let mut camera = Camera::new([800, 600]);
    let invoke =
        |input: &mut ImageInput, e: &mut DocumentEditor, camera: &mut Camera, action| -> R {
            input.dispatch(
                ActionEvent {
                    action,
                    phase: ActionPhase::Invoke,
                },
                e,
                camera,
            )?;
            Ok(())
        };
    let button = |state| PhysicalEvent::Button {
        control: PhysicalControl::Pointer(PointerButton::Mouse(MouseButton::Left)),
        state,
        repeat: false,
    };
    for end in [[260., 260.], [140., 260.], [260., 140.], [140., 140.]] {
        invoke(
            &mut input,
            &mut e,
            &mut camera,
            Action::SelectTool(Tool::Rectangle),
        )?;
        input.cursor_moved([200., 200.], &e, &mut camera)?;
        input.physical(button(ElementState::Pressed), &mut e, &mut camera)?;
        input.cursor_moved(end, &e, &mut camera)?;
        input.physical(button(ElementState::Released), &mut e, &mut camera)?;
        let id = *e.document().object_order().last().ok_or("rectangle")?;
        let object = e.document().object(id).ok_or("rectangle")?;
        assert_eq!(object.transform().size(), [60.; 2]);
        let ObjectKind::Annotation(a) = object.kind() else {
            return Err("rectangle kind".into());
        };
        assert_eq!(a.style().width(), 3.);
        invoke(
            &mut input,
            &mut e,
            &mut camera,
            Action::AnnotationStyle(StyleAction::Wider),
        )?;
        assert_eq!(input.annotation.style.width(), 3.);
        let center = e
            .document()
            .object(id)
            .ok_or("object")?
            .transform()
            .center();
        for kind in [
            GestureKind::Move,
            GestureKind::Resize {
                handle: 4,
                center: false,
            },
            GestureKind::Rotate,
        ] {
            input.images.selection.select(Some(id), false);
            let before = e.document().clone();
            let center = e
                .document()
                .object(id)
                .ok_or("object")?
                .transform()
                .center();
            input.images.begin(kind, [center[0] + 20., center[1]], &e)?;
            input.images.update([center[0] + 30., center[1] + 15.])?;
            input.images.commit(&mut e)?;
            assert_ne!(*e.document(), before);
            e.undo()?;
            assert_eq!(*e.document(), before);
            e.redo()?;
            e.undo()?;
        }
        let before = e.document().clone();
        let (command, ids) =
            tack_app::duplicate::selection(e.document(), [id].into_iter(), [10.; 2])?;
        e.execute(command)?;
        assert_eq!(ids.len(), 1);
        assert_ne!(ids[0], id);
        e.undo()?;
        assert_eq!(*e.document(), before);
        input.images.selection.select_all(e.document());
        invoke(
            &mut input,
            &mut e,
            &mut camera,
            Action::ToggleAnnotationSelectionLock,
        )?;
        assert!(input.images.selection.is_empty());
        assert!(input.images.hit(e.document(), center).is_none());
        invoke(
            &mut input,
            &mut e,
            &mut camera,
            Action::ToggleAnnotationSelectionLock,
        )?;
    }
    let count = e.document().object_order().len();
    invoke(
        &mut input,
        &mut e,
        &mut camera,
        Action::SelectTool(Tool::Rectangle),
    )?;
    input.cursor_moved([200., 200.], &e, &mut camera)?;
    input.physical(button(ElementState::Pressed), &mut e, &mut camera)?;
    input.cursor_moved([200.1, 200.1], &e, &mut camera)?;
    input.physical(button(ElementState::Released), &mut e, &mut camera)?;
    assert_eq!(e.document().object_order().len(), count);
    for width in [1., 2., 3.] {
        let head = tack_app::annotation_geometry::arrow_head([0.; 2], [100., 0.], width);
        assert!(100. - head[1][0] >= 12.);
        assert!(head[1][1] >= 6.);
    }
    Ok(())
}
