use std::{
    collections::BTreeMap,
    fs,
    sync::Arc,
    time::{Duration, Instant},
};
use tack_app::{
    image_interaction::{GestureKind, ImageInteraction},
    image_save::{ImageSave, save_snapshot},
};
use tack_core::*;
use tack_storage::{TackFile, new_document_id, save};
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
struct Fixture {
    root: std::path::PathBuf,
    path: std::path::PathBuf,
    board: Arc<TackFile>,
    editor: DocumentEditor,
}
impl Fixture {
    fn new() -> Result<Self> {
        let root = std::env::temp_dir().join(format!(
            "tack-interaction-test-{:032x}",
            new_document_id()?.value()
        ));
        fs::create_dir(&root)?;
        let path = root.join("board.tack");
        let mut doc = Document::new(new_document_id()?, DocumentLimits::default());
        doc.apply(Command::AddSource(Source::linked(
            SourceId::new(1)?,
            "missing.png",
        )?))?;
        doc.apply(Command::AddAsset(ImageAsset::new(
            AssetId::new(1)?,
            SourceId::new(1)?,
            [6000, 4500],
        )?))?;
        for id in 1..=3 {
            doc.apply(Command::AddObject {
                object: DocumentObject::image(
                    ObjectId::new(id)?,
                    AssetId::new(1)?,
                    Transform::new([id as f64 * 100., 0.], [100., 80.], 0.3, [id == 2, false])?,
                ),
                index: doc.object_order().len(),
            })?;
        }
        save(&path, &doc, vec![])?;
        let board = Arc::new(TackFile::open(&path)?);
        let editor = DocumentEditor::new(board.document.clone(), 200);
        Ok(Self {
            root,
            path,
            board,
            editor,
        })
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
#[test]
fn all_durable_interactions_and_delete_undo_round_trip_without_selection() -> Result {
    for kind in [
        GestureKind::Move,
        GestureKind::Resize {
            handle: 4,
            center: false,
        },
        GestureKind::Rotate,
        GestureKind::Crop { handle: 3 },
        GestureKind::Opacity,
    ] {
        let mut f = Fixture::new()?;
        let mut images = ImageInteraction::default();
        images.selection.select(Some(ObjectId::new(2)?), false);
        images.begin(kind, [240., 0.], &f.editor)?;
        images.update([218., 22.])?;
        images.commit(&mut f.editor)?;
        let expected = f.editor.document().clone();
        save_snapshot(f.path.clone(), &f.board, &expected, &BTreeMap::new())?;
        drop(images);
        assert_eq!(TackFile::open(&f.path)?.document, expected);
        assert!(ImageInteraction::default().selection.is_empty());
    }
    let mut f = Fixture::new()?;
    let mut images = ImageInteraction::default();
    images.selection.select_all(f.editor.document());
    images.begin(GestureKind::Move, [0.; 2], &f.editor)?;
    images.update([50., -25.])?;
    images.commit(&mut f.editor)?;
    images.flip(&mut f.editor, 0)?;
    images.flip(&mut f.editor, 1)?;
    for filtering in [
        ImageFiltering::Nearest,
        ImageFiltering::Smooth,
        ImageFiltering::Default,
    ] {
        images.filtering(&mut f.editor, Some(filtering))?;
        save_snapshot(
            f.path.clone(),
            &f.board,
            f.editor.document(),
            &BTreeMap::new(),
        )?;
        assert_eq!(TackFile::open(&f.path)?.document, *f.editor.document());
    }
    let before = f.editor.document().clone();
    images.delete(&mut f.editor)?;
    f.editor.undo()?;
    assert_eq!(f.editor.document(), &before);
    save_snapshot(
        f.path.clone(),
        &f.board,
        f.editor.document(),
        &BTreeMap::new(),
    )?;
    assert_eq!(TackFile::open(&f.path)?.document, before);
    Ok(())
}
#[test]
fn owned_save_generation_does_not_lose_edits_and_exact_save_clears_dirty() -> Result {
    let mut f = Fixture::new()?;
    let id = ObjectId::new(1)?;
    let mut worker = ImageSave::default();
    f.editor.execute(Command::SetOpacity {
        object: id,
        opacity: Opacity::new(0.5)?,
    })?;
    let snapshot = f.editor.document().clone();
    assert!(worker.start(
        f.path.clone(),
        Arc::clone(&f.board),
        &f.editor,
        &BTreeMap::new()
    )?);
    assert!(!worker.start(
        f.path.clone(),
        Arc::clone(&f.board),
        &f.editor,
        &BTreeMap::new()
    )?);
    f.editor.execute(Command::SetOpacity {
        object: id,
        opacity: Opacity::new(0.6)?,
    })?;
    worker.finish(&mut f.editor);
    assert!(worker.last_error.is_none());
    assert!(f.editor.is_dirty());
    assert_eq!(TackFile::open(&f.path)?.document, snapshot);
    worker.start(
        f.path.clone(),
        Arc::clone(&f.board),
        &f.editor,
        &BTreeMap::new(),
    )?;
    let start = Instant::now();
    while worker.active() {
        worker.poll(&mut f.editor);
        if start.elapsed() > Duration::from_secs(5) {
            return Err("save timed out".into());
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(!f.editor.is_dirty());
    assert_eq!(TackFile::open(&f.path)?.document, *f.editor.document());
    Ok(())
}
#[test]
fn failed_worker_save_preserves_dirty_and_previous_valid_file() -> Result {
    let mut f = Fixture::new()?;
    let previous = fs::read(&f.path)?;
    f.editor.execute(Command::SetOpacity {
        object: ObjectId::new(1)?,
        opacity: Opacity::new(0.2)?,
    })?;
    let mut worker = ImageSave::default();
    worker.start(
        f.root.join("absent/board.tack"),
        Arc::clone(&f.board),
        &f.editor,
        &BTreeMap::new(),
    )?;
    worker.finish(&mut f.editor);
    assert!(worker.last_error.is_some());
    assert!(f.editor.is_dirty());
    assert_eq!(fs::read(&f.path)?, previous);
    TackFile::open(&f.path)?;
    Ok(())
}

#[test]
fn repaired_overview_handle_survives_cache_eviction_before_save() -> Result {
    let f = Fixture::new()?;
    let overview = f.root.join("disposable-overview.png");
    fs::write(&overview, b"bounded derived fixture")?;
    let payload = tack_storage::Payload::pin_overview(&overview)?;
    fs::remove_file(&overview)?;
    let output = f.root.join("pinned.tack");
    save(
        &output,
        f.editor.document(),
        vec![tack_storage::BlobInput::overview(
            AssetId::new(1)?,
            1,
            [16, 16],
            1,
            payload,
        )],
    )?;
    let reopened = TackFile::open(output)?;
    assert_eq!(
        reopened.overview_bytes(AssetId::new(1)?)?,
        b"bounded derived fixture"
    );
    Ok(())
}
