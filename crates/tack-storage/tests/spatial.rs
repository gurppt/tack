use std::fs;
use tack_core::*;
use tack_storage::{TackFile, new_document_id, save};
type R = Result<(), Box<dyn std::error::Error>>;
#[test]
fn old_schema_and_all_spatial_records_roundtrip_exact_with_missing_sources() -> R {
    let dir = std::env::temp_dir().join(format!(
        "tack-spatial-roundtrip-{}",
        new_document_id()?.value()
    ));
    fs::create_dir(&dir)?;
    let path = dir.join("a.tack");
    let mut d = Document::new(DocumentId::new(1)?, DocumentLimits::default());
    d.apply(Command::AddSource(Source::linked(
        SourceId::new(1)?,
        "does-not-exist.png",
    )?))?;
    d.apply(Command::AddAsset(ImageAsset::new(
        AssetId::new(1)?,
        SourceId::new(1)?,
        [10, 10],
    )?))?;
    for i in 1..=3 {
        d.apply(Command::AddObject {
            object: DocumentObject::image(
                ObjectId::new(i)?,
                AssetId::new(1)?,
                Transform::new(
                    [i as f64 * 32., 15.],
                    [18., 27.],
                    i as f64 * 0.1,
                    [false, true],
                )?,
            ),
            index: d.object_order().len(),
        })?;
    }
    save(&path, &d, vec![])?;
    let old = fs::read(&path)?;
    assert_eq!(&old[12..16], &1u32.to_le_bytes());
    assert_eq!(TackFile::open(&path)?.document, d);
    let mut e = DocumentEditor::new(d, 200);
    e.execute(Command::AddGroup(Group::new(
        GroupId::new(9)?,
        vec![ObjectId::new(1)?, ObjectId::new(2)?],
    )?))?;
    e.execute(Command::AddObject {
        object: DocumentObject::frame(
            ObjectId::new(99)?,
            "Références 猫 Ж 😀".into(),
            Transform::new([32., 15.], [400., 200.], 0., [false; 2])?,
        )?,
        index: 0,
    })?;
    e.execute(Command::SetFrameName {
        object: ObjectId::new(99)?,
        name: "Textures 🌿".into(),
    })?;
    e.execute(Command::SetTransform {
        object: ObjectId::new(99)?,
        transform: Transform::new([100., 200.], [500., 300.], 0., [false; 2])?,
    })?;
    save(&path, e.document(), vec![])?;
    let new = fs::read(&path)?;
    assert_eq!(&new[12..16], &2u32.to_le_bytes());
    assert_eq!(&TackFile::open(&path)?.document, e.document());
    let grouped = e.document().clone();
    e.execute(Command::Batch(vec![
        Command::RemoveGroup(GroupId::new(9)?),
        Command::RemoveObject(ObjectId::new(1)?),
        Command::RemoveObject(ObjectId::new(2)?),
        Command::RemoveObject(ObjectId::new(99)?),
    ]))?;
    save(&path, e.document(), vec![])?;
    assert_eq!(&TackFile::open(&path)?.document, e.document());
    e.undo()?;
    assert_eq!(e.document(), &grouped);
    save(&path, e.document(), vec![])?;
    assert_eq!(&TackFile::open(&path)?.document, e.document());
    // Refusal never mutates the file; old readers refuse schema 2 at its header.
    let original = fs::read(&path)?;
    let mut future = original.clone();
    future[12..16].copy_from_slice(&4u32.to_le_bytes());
    fs::write(&path, &future)?;
    assert!(TackFile::open(&path).is_err());
    assert_eq!(fs::read(&path)?, future);
    let mut downgrade = original.clone();
    downgrade[12..16].copy_from_slice(&1u32.to_le_bytes());
    fs::write(&path, &downgrade)?;
    assert!(TackFile::open(&path).is_err());
    // Group references are validated after checksums, not accepted as opaque IDs.
    let mut bad = original.clone();
    let authlen = u64::from_le_bytes(bad[16..24].try_into()?) as usize;
    let end = 80 + authlen;
    bad[end - 16..end].copy_from_slice(&9999u128.to_le_bytes());
    let crc = crc32fast::hash(&bad[80..end]);
    bad[40..44].copy_from_slice(&crc.to_le_bytes());
    fs::write(&path, &bad)?;
    assert!(TackFile::open(&path).is_err());
    fs::write(&path, &old)?;
    assert_eq!(TackFile::open(&path)?.document.object_order().len(), 3);
    fs::remove_dir_all(&dir)?;
    Ok(())
}
