use tack_core::*;
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
fn fixture() -> Result<DocumentEditor> {
    let mut doc = Document::new(DocumentId::new(1)?, DocumentLimits::default());
    doc.apply(Command::AddSource(Source::embedded(SourceId::new(1)?)))?;
    doc.apply(Command::AddAsset(ImageAsset::new(
        AssetId::new(1)?,
        SourceId::new(1)?,
        [100, 80],
    )?))?;
    for id in 1..=3 {
        doc.apply(Command::AddObject {
            object: DocumentObject::image(
                ObjectId::new(id)?,
                AssetId::new(1)?,
                Transform::new([id as f64 * 100., 0.], [100., 80.], 0., [false; 2])?,
            ),
            index: doc.object_order().len(),
        })?;
    }
    Ok(DocumentEditor::new(doc, 200))
}
#[test]
fn batch_failure_rolls_back_and_preserves_history_and_generation() -> Result {
    let mut e = fixture()?;
    let before = e.document().clone();
    let one = ObjectId::new(1)?;
    let missing = ObjectId::new(99)?;
    let transform = Transform::new([5., 7.], [200., 100.], 0.3, [true, false])?;
    assert!(
        e.execute(Command::Batch(vec![
            Command::RemoveObject(one),
            Command::SetTransform {
                object: missing,
                transform
            }
        ]))
        .is_err()
    );
    assert_eq!(e.document(), &before);
    assert_eq!(e.generation(), 0);
    assert_eq!(e.undo_len(), 0);
    assert!(!e.is_dirty());
    assert!(
        e.execute(Command::Batch(vec![Command::Batch(vec![])]))
            .is_err()
    );
    assert!(
        e.execute(Command::Batch(vec![Command::RemoveSource(SourceId::new(
            1
        )?)]))
        .is_err()
    );
    assert_eq!(e.document(), &before);
    e.execute(Command::Batch(vec![
        Command::SetTransform {
            object: one,
            transform,
        },
        Command::SetCrop {
            object: one,
            crop: Crop::new(0.1, 0.2, 0.7, 0.6)?,
        },
        Command::RemoveObject(ObjectId::new(3)?),
    ]))?;
    let after = e.document().clone();
    assert_eq!(e.undo_len(), 1);
    e.undo()?;
    assert_eq!(e.document(), &before);
    e.redo()?;
    assert_eq!(e.document(), &after);
    e.undo()?;
    assert!(
        e.execute(Command::Batch(vec![Command::SetTransform {
            object: missing,
            transform
        }]))
        .is_err()
    );
    assert_eq!(e.redo_len(), 1);
    e.redo()?;
    assert_eq!(e.document(), &after);
    Ok(())
}
#[test]
fn save_acknowledgement_only_clears_exact_successful_generation() -> Result {
    let mut e = fixture()?;
    let id = ObjectId::new(1)?;
    e.execute(Command::SetOpacity {
        object: id,
        opacity: Opacity::new(0.5)?,
    })?;
    let generation = e.generation();
    e.execute(Command::SetOpacity {
        object: id,
        opacity: Opacity::new(0.6)?,
    })?;
    assert!(!e.mark_saved_generation(generation));
    assert!(e.is_dirty());
    assert!(e.mark_saved_generation(e.generation()));
    assert!(!e.is_dirty());
    e.undo()?;
    assert!(e.is_dirty());
    assert!(!e.mark_saved_generation(generation));
    Ok(())
}
#[test]
fn large_image_batch_history_remains_byte_bounded() -> Result {
    let mut e = fixture()?;
    let id = ObjectId::new(1)?;
    for n in 0..12 {
        let edits = (0..100_000)
            .map(|i| {
                Ok(Command::SetOpacity {
                    object: id,
                    opacity: Opacity::new(((i + n) % 2) as f64)?,
                })
            })
            .collect::<std::result::Result<Vec<_>, GeometryError>>()?;
        e.execute(Command::Batch(edits))?;
        assert!(e.history_bytes() <= 32 * 1024 * 1024);
    }
    assert!(e.undo_len() < 12);
    Ok(())
}
