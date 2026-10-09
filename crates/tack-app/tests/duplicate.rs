use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Read,
    path::PathBuf,
};
use tack_app::{duplicate, image_save::save_snapshot};
use tack_core::*;
use tack_storage::{BlobInput, Payload, TackFile, new_document_id, save};

type R<T = ()> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

struct Temp(PathBuf);
impl Temp {
    fn new() -> R<Self> {
        let path = std::env::temp_dir().join(format!(
            "tack-duplicate-test-{}",
            new_document_id()?.value()
        ));
        fs::create_dir(&path)?;
        Ok(Self(path))
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn fixture(limits: DocumentLimits) -> R<Document> {
    let mut doc = Document::new(DocumentId::new(1)?, limits);
    doc.apply(Command::AddSource(Source::linked(
        SourceId::new(1)?,
        "missing-original.jpg",
    )?))?;
    doc.apply(Command::AddSource(Source::embedded(SourceId::new(2)?)))?;
    for i in 1..=2 {
        doc.apply(Command::AddAsset(ImageAsset::new(
            AssetId::new(i)?,
            SourceId::new(i)?,
            [50_000; 2],
        )?))?;
    }
    let t = Transform::new([10., 20.], [100., 80.], 0.3, [true, false])?;
    for (id, asset) in [(1, 1), (2, 1), (3, 2)] {
        doc.apply(Command::AddObject {
            object: DocumentObject::image_with_properties(
                ObjectId::new(id)?,
                AssetId::new(asset)?,
                t,
                Crop::new(0.1, 0.2, 0.6, 0.5)?,
                Opacity::new(0.4)?,
                ImageFiltering::Nearest,
            ),
            index: doc.object_order().len(),
        })?;
    }
    for (id, kind) in [
        (
            4,
            AnnotationKind::Text(TextObject::new(
                "note 猫\nline".into(),
                32.,
                TextAlignment::Right,
            )?),
        ),
        (
            5,
            AnnotationKind::Arrow(LineObject::new([[0., 0.5], [1., 0.5]])?),
        ),
    ] {
        doc.apply(Command::AddObject {
            object: DocumentObject::annotation(
                ObjectId::new(id)?,
                Annotation::new(kind, AnnotationStyle::default()),
                t,
            )?,
            index: doc.object_order().len(),
        })?;
    }
    doc.apply(Command::AddObject {
        object: DocumentObject::frame(
            ObjectId::new(6)?,
            "Frame".into(),
            Transform::new([30., 40.], [200., 100.], 0., [false; 2])?,
        )?,
        index: doc.object_order().len(),
    })?;
    doc.apply(Command::AddGroup(Group::new(
        GroupId::new(1)?,
        vec![ObjectId::new(1)?, ObjectId::new(2)?, ObjectId::new(3)?],
    )?))?;
    doc.apply(Command::SetCameraBookmarks(vec![CameraBookmark::new(
        BookmarkId::new(1)?,
        "view".into(),
        [5., 6.],
        2.,
    )?]))?;
    Ok(doc)
}

#[test]
fn mixed_duplicate_preserves_properties_and_only_connects_new_group_members() -> R {
    let original = fixture(DocumentLimits::default())?;
    let mut editor = DocumentEditor::new(original.clone(), 200);
    // Scrambled/repeated selection must be canonical document order, once each.
    let selected = [6, 4, 1, 5, 3, 1]
        .into_iter()
        .map(ObjectId::new)
        .collect::<Result<Vec<_>, _>>()?;
    let (command, copies) = duplicate::selection(&original, selected.into_iter(), [32., 32.])?;
    assert_eq!(copies.len(), 5);
    let old_ids: BTreeSet<_> = original.object_order().iter().copied().collect();
    let new_ids: BTreeSet<_> = copies.iter().copied().collect();
    assert_eq!(new_ids.len(), copies.len());
    assert!(old_ids.is_disjoint(&new_ids));
    assert!(editor.execute(command)?);
    assert_eq!(editor.undo_len(), 1);
    let after = editor.document().clone();
    assert_eq!(
        &after.object_order()[original.object_order().len()..],
        copies.as_slice()
    );
    for (&old, &new) in [1, 3, 4, 5, 6].iter().zip(&copies) {
        let previous = original
            .object(ObjectId::new(old)?)
            .ok_or("missing original")?;
        let copy = after.object(new).ok_or("missing copy")?;
        assert_eq!(copy.kind(), previous.kind());
        assert_eq!(
            copy.transform().center(),
            [
                previous.transform().center()[0] + 32.,
                previous.transform().center()[1] + 32.
            ]
        );
        assert_eq!(copy.transform().size(), previous.transform().size());
        assert_eq!(copy.transform().rotation(), previous.transform().rotation());
        assert_eq!(copy.transform().flips(), previous.transform().flips());
        assert_eq!(after.object(previous.id()), Some(previous));
    }
    assert_eq!(
        after.assets().collect::<Vec<_>>(),
        original.assets().collect::<Vec<_>>()
    );
    assert_eq!(
        after.sources().collect::<Vec<_>>(),
        original.sources().collect::<Vec<_>>()
    );
    assert_eq!(after.bookmarks(), original.bookmarks());
    let groups: Vec<_> = after.groups().collect();
    assert_eq!(groups.len(), 2);
    let old_group = original.groups().next().ok_or("missing group")?;
    assert_eq!(after.group(old_group.id()), Some(old_group));
    let copied_group = groups
        .into_iter()
        .find(|g| g.id() != old_group.id())
        .ok_or("missing copied group")?;
    assert_eq!(copied_group.members().len(), 2);
    assert!(copied_group.members().iter().all(|id| new_ids.contains(id)));
    assert!(copied_group.members().contains(&copies[0]));
    assert!(copied_group.members().contains(&copies[1]));
    editor.undo()?;
    assert_eq!(editor.document(), &original);
    assert_eq!(editor.undo_len(), 0);
    editor.redo()?;
    assert_eq!(editor.document(), &after);
    Ok(())
}

#[test]
fn single_member_has_no_new_group_and_invalid_or_over_limit_selection_is_atomic() -> R {
    let original = fixture(DocumentLimits::default())?;
    let (single, copies) =
        duplicate::selection(&original, std::iter::once(ObjectId::new(1)?), [8., 8.])?;
    let mut doc = original.clone();
    doc.apply(single)?;
    assert_eq!(copies.len(), 1);
    assert_eq!(doc.groups().count(), 1);
    assert_eq!(doc.sources().count(), 2);
    assert_eq!(doc.assets().count(), 2);
    let before = original.clone();
    assert!(
        duplicate::selection(
            &original,
            [ObjectId::new(1)?, ObjectId::new(999)?].into_iter(),
            [8., 8.]
        )
        .is_err()
    );
    assert!(
        duplicate::selection(
            &original,
            std::iter::once(ObjectId::new(1)?),
            [f64::NAN, 0.]
        )
        .is_err()
    );
    assert_eq!(original, before);
    let limited = fixture(DocumentLimits {
        objects: 6,
        ..DocumentLimits::default()
    })?;
    assert!(duplicate::selection(&limited, std::iter::once(ObjectId::new(1)?), [8., 8.]).is_err());
    assert_eq!(limited.object_order().len(), 6);
    let mut editor = DocumentEditor::new(original, 200);
    let (empty, copies) = duplicate::selection(editor.document(), std::iter::empty(), [8., 8.])?;
    assert!(copies.is_empty());
    assert!(!editor.execute(empty)?);
    assert_eq!(editor.undo_len(), 0);
    Ok(())
}

#[test]
fn duplicate_save_reopen_reuses_single_embedded_blob_and_shared_linked_asset() -> R {
    let tmp = Temp::new()?;
    let path = tmp.0.join("board.tack");
    let blob = tmp.0.join("original.bytes");
    let original_bytes = vec![0x59; 4096];
    fs::write(&blob, &original_bytes)?;
    let original = fixture(DocumentLimits::default())?;
    save(
        &path,
        &original,
        vec![BlobInput::original(
            SourceId::new(2)?,
            1,
            Payload::File(blob.clone()),
        )],
    )?;
    let board = TackFile::open(&path)?;
    assert_eq!(board.originals.len(), 1);
    // Duplicating metadata must work after this auxiliary source file disappears.
    fs::remove_file(blob)?;
    let mut editor = DocumentEditor::new(board.document.clone(), 200);
    let (command, copies) = duplicate::selection(
        editor.document(),
        editor.document().object_order().iter().copied(),
        [32., 32.],
    )?;
    assert_eq!(copies.len(), 6);
    editor.execute(command)?;
    let duplicated = editor.document().clone();
    save_snapshot(path.clone(), &board, &duplicated, &BTreeMap::new())?;
    let reopened = TackFile::open(&path)?;
    assert_eq!(reopened.document, duplicated);
    assert_eq!(reopened.originals.len(), 1);
    assert_eq!(reopened.overviews.len(), board.overviews.len());
    assert_eq!(
        reopened.originals[&SourceId::new(2)?].range.len,
        original_bytes.len() as u64
    );
    let mut actual = Vec::new();
    reopened
        .original_reader(SourceId::new(2)?)?
        .read_to_end(&mut actual)?;
    assert_eq!(actual, original_bytes);
    for &id in &[ObjectId::new(1)?, ObjectId::new(2)?, copies[0], copies[1]] {
        let ObjectKind::Image(image) = duplicated.object(id).ok_or("missing duplicate")?.kind()
        else {
            return Err("expected image".into());
        };
        assert_eq!(image.asset_id(), AssetId::new(1)?);
    }
    editor.undo()?;
    assert_eq!(editor.document(), &original);
    editor.redo()?;
    assert_eq!(editor.document(), &duplicated);
    Ok(())
}
