use super::*;
use tack_core::*;
struct Root(PathBuf);
impl Root {
    fn new() -> Result<Self, AssetError> {
        let path = std::env::temp_dir().join(format!(
            "tack-source-batch-{:032x}",
            tack_storage::new_document_id()?.value()
        ));
        fs::create_dir(&path)?;
        Ok(Self(path))
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn fixture(root: &Path) -> Result<(Document, TackFile, PathBuf), AssetError> {
    let mut doc = Document::new(DocumentId::new(1)?, DocumentLimits::default());
    let source = root.join("Original.PNG");
    fs::write(&source, b"\x89PNG\r\n\x1a\nbyte-exact-original")?;
    for n in 1..=2 {
        let source_id = SourceId::new(n)?;
        doc.apply(Command::AddSource(if n == 1 {
            Source::linked(source_id, &source)?
        } else {
            Source::embedded(source_id).with_original_path(Some(LinkedPath::native(&source)?))
        }))?;
        doc.apply(Command::AddAsset(ImageAsset::new(
            AssetId::new(n)?,
            source_id,
            [64, 32],
        )?))?;
        doc.apply(Command::AddObject {
            object: DocumentObject::image(
                ObjectId::new(n)?,
                AssetId::new(n)?,
                Transform::new([0., 0.], [64., 32.], 0., [false, false])?,
            ),
            index: doc.object_order().len(),
        })?;
    }
    for (id, name, center) in [(3, "Frame/CON", [0., 0.]), (4, "Frame\\CON", [500., 0.])] {
        doc.apply(Command::AddObject {
            object: DocumentObject::frame(
                ObjectId::new(id)?,
                name.into(),
                Transform::new(center, [200., 200.], 0., [false, false])?,
            )?,
            index: doc.object_order().len(),
        })?;
    }
    doc.apply(Command::AddObject {
        object: DocumentObject::annotation(
            ObjectId::new(5)?,
            Annotation::new(
                AnnotationKind::Text(TextObject::new(
                    "First note".into(),
                    16.,
                    TextAlignment::Left,
                )?),
                AnnotationStyle::default(),
            ),
            Transform::new([0., 0.], [80., 40.], 0., [false, false])?,
        )?,
        index: doc.object_order().len(),
    })?;
    let path = root.join("board.tack");
    tack_storage::save(
        &path,
        &doc,
        vec![tack_storage::BlobInput::original(
            SourceId::new(2)?,
            1,
            Payload::File(source),
        )],
    )?;
    let board = TackFile::open(&path)?;
    Ok((doc, board, path))
}
#[test]
fn byte_exact_current_linked_authoritative_embedded_duplicates_frames_notes()
-> Result<(), AssetError> {
    let root = Root::new()?;
    let (doc, board, path) = fixture(&root.0)?;
    let originals = Originals::new();
    let request = Request::new(&doc, None, &path, &board, &originals, true)?;
    let original = fs::read(root.0.join("Original.PNG"))?;
    fs::write(
        root.0.join("Original.PNG"),
        b"\x89PNG\r\n\x1a\nnew-current-linked",
    )?;
    let target = request.export_to(&root.0, &AtomicBool::new(false))?;
    assert_eq!(
        fs::read(target.join("Frame_CON/Original.PNG"))?,
        fs::read(root.0.join("Original.PNG"))?
    );
    assert_eq!(fs::read(target.join("Frame_CON/Original_2.PNG"))?, original);
    assert_eq!(
        fs::read_to_string(target.join("Frame_CON/notes.txt"))?,
        "First note\n\n"
    );
    let target2 = request.export_to(&root.0, &AtomicBool::new(false))?;
    assert_eq!(
        target2.file_name().and_then(|n| n.to_str()),
        Some("Tack Sources_2")
    );
    assert_eq!(
        fs::read(target.join("Frame_CON/notes.txt"))?,
        fs::read(target2.join("Frame_CON/notes.txt"))?
    );
    let mut selection = crate::image_interaction::SelectionState::default();
    selection.select(Some(ObjectId::new(3)?), false);
    let selected = Request::new(&doc, Some(&selection), &path, &board, &originals, false)?;
    assert_eq!(selected.images.len(), 2);
    assert_eq!(selected.notes.len(), 1);
    Ok(())
}
#[test]
fn portable_safe_names_and_suffixes() {
    assert_eq!(sanitize(".. /CON:*? "), "_CON___");
    assert_eq!(sanitize("CON.txt"), "_CON.txt");
    assert_eq!(sanitize("..."), "Untitled");
    assert!(sanitize(&"🍕".repeat(80)).len() <= 160);
    let mut names = BTreeSet::new();
    assert_eq!(unique("Name", "png", &mut names), "Name.png");
    assert_eq!(unique("NAME", "PNG", &mut names), "NAME_2.PNG");
    let mut frames = BTreeSet::from(["_unframed".to_string()]);
    assert_eq!(unique("_unframed", "", &mut frames), "_unframed_2");
}
#[test]
fn cancellation_and_symlink_destination_do_not_replace_existing_data() -> Result<(), AssetError> {
    let root = Root::new()?;
    let (doc, board, path) = fixture(&root.0)?;
    let request = Request::new(&doc, None, &path, &board, &Originals::new(), false)?;
    assert!(request.export_to(&root.0, &AtomicBool::new(true)).is_err());
    #[cfg(unix)]
    {
        let link = root.0.join("alias");
        std::os::unix::fs::symlink(&root.0, &link)?;
        assert!(request.export_to(&link, &AtomicBool::new(false)).is_err());
    }
    assert!(root.0.join("board.tack").is_file());
    Ok(())
}
use tack_storage::Payload;
