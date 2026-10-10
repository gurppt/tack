use tack_core::*;
type R = Result<(), Box<dyn std::error::Error>>;
fn id(n: u128) -> Result<ObjectId, InvalidId> {
    ObjectId::new(n)
}
fn fixture() -> Result<Document, Box<dyn std::error::Error>> {
    let mut doc = Document::new(DocumentId::new(1)?, DocumentLimits::default());
    doc.apply(Command::AddSource(Source::embedded(SourceId::new(1)?)))?;
    doc.apply(Command::AddAsset(ImageAsset::new(
        AssetId::new(1)?,
        SourceId::new(1)?,
        [100, 50],
    )?))?;
    for n in 1..=3 {
        doc.apply(Command::AddObject {
            object: DocumentObject::image(
                id(n)?,
                AssetId::new(1)?,
                Transform::new([n as f64 * 13.25, 7.5], [100., 50.], 0.4, [true, false])?,
            ),
            index: doc.object_order().len(),
        })?;
    }
    for n in [10, 11] {
        doc.apply(Command::AddObject {
            object: DocumentObject::frame(
                id(n)?,
                format!("Frame {n}"),
                Transform::new([0., 0.], [400., 300.], 0., [false; 2])?,
            )?,
            index: doc.object_order().len(),
        })?;
    }
    doc.apply(Command::AddObject {
        object: DocumentObject::annotation(
            id(20)?,
            Annotation::new(AnnotationKind::Rect, AnnotationStyle::default()),
            Transform::new([90., 80.], [40., 30.], 0.2, [false; 2])?,
        )?,
        index: doc.object_order().len(),
    })?;
    Ok(doc)
}
#[test]
fn frame_translation_resize_unlink_delete_and_history_are_exact() -> R {
    let mut editor = DocumentEditor::new(fixture()?, 40);
    let initial = editor.document().clone();
    editor.execute(Command::SetFrameLinks(vec![
        (id(1)?, Some(id(10)?)),
        (id(20)?, Some(id(10)?)),
    ]))?;
    let linked = editor.document().clone();
    let next = Transform::new([31.1, -17.7], [400., 300.], 0., [false; 2])?;
    editor.execute(Command::Batch(vec![Command::SetTransform {
        object: id(10)?,
        transform: next,
    }]))?;
    let moved = editor.document().clone();
    for child in [1, 20] {
        let before = linked.object(id(child)?).ok_or("child")?.transform();
        let after = moved.object(id(child)?).ok_or("child")?.transform();
        assert_eq!(
            after.center(),
            [before.center()[0] + 31.1, before.center()[1] - 17.7]
        );
        assert_eq!(after.size(), before.size());
        assert_eq!(after.rotation(), before.rotation());
        assert_eq!(after.flips(), before.flips());
    }
    assert_eq!(linked.object(id(2)?), moved.object(id(2)?));
    editor.undo()?;
    assert_eq!(editor.document(), &linked);
    editor.redo()?;
    assert_eq!(editor.document(), &moved);
    editor.execute(Command::SetTransform {
        object: id(10)?,
        transform: Transform::new([21., 9.], [450., 270.], 0., [false; 2])?,
    })?;
    assert_eq!(editor.document().object(id(1)?), moved.object(id(1)?));
    editor.undo()?;
    editor.execute(Command::SetFrameLinks(vec![(id(1)?, None)]))?;
    assert_eq!(editor.document().object(id(1)?), moved.object(id(1)?));
    editor.undo()?;
    assert_eq!(editor.document(), &moved);
    editor.execute(Command::Batch(vec![Command::RemoveObject(id(10)?)]))?;
    assert_eq!(editor.document().frame_parent(id(1)?), None);
    assert_eq!(editor.document().frame_parent(id(20)?), None);
    assert_eq!(editor.document().object(id(1)?), moved.object(id(1)?));
    editor.undo()?;
    assert_eq!(editor.document(), &moved);
    editor.redo()?;
    editor.undo()?;
    assert_eq!(editor.document(), &moved);
    editor.undo()?;
    editor.undo()?;
    assert_eq!(editor.document(), &initial);
    Ok(())
}
#[test]
fn group_linkage_is_atomic_and_ungroup_retains_parent() -> R {
    let mut editor = DocumentEditor::new(fixture()?, 40);
    let group = Group::new(GroupId::new(1)?, vec![id(1)?, id(2)?])?;
    editor.execute(Command::AddGroup(group.clone()))?;
    let initial = editor.document().clone();
    assert!(
        editor
            .execute(Command::SetFrameLinks(vec![(id(1)?, Some(id(10)?))]))
            .is_err()
    );
    assert_eq!(editor.document(), &initial);
    editor.execute(Command::SetFrameLinks(vec![
        (id(1)?, Some(id(10)?)),
        (id(2)?, Some(id(10)?)),
    ]))?;
    assert_eq!(
        editor
            .document()
            .linked_children(id(10)?)
            .collect::<Vec<_>>(),
        vec![id(1)?, id(2)?]
    );
    editor.execute(Command::RemoveGroup(group.id()))?;
    assert_eq!(editor.document().frame_parent(id(1)?), Some(id(10)?));
    editor.execute(Command::SetFrameLinks(vec![(id(1)?, Some(id(11)?))]))?;
    assert!(editor.execute(Command::AddGroup(group)).is_err());
    assert!(
        editor
            .execute(Command::SetFrameLinks(vec![(id(10)?, Some(id(11)?))]))
            .is_err()
    );
    assert!(
        editor
            .execute(Command::SetFrameLinks(vec![(id(3)?, Some(id(2)?))]))
            .is_err()
    );
    Ok(())
}
#[test]
fn failed_batch_restores_links_indices_and_positions() -> R {
    let mut editor = DocumentEditor::new(fixture()?, 40);
    editor.execute(Command::SetFrameLinks(vec![(id(1)?, Some(id(10)?))]))?;
    let before = editor.document().clone();
    let history = editor.undo_len();
    assert!(
        editor
            .execute(Command::Batch(vec![
                Command::SetTransform {
                    object: id(10)?,
                    transform: Transform::new([17., 9.], [400., 300.], 0., [false; 2])?
                },
                Command::RemoveObject(id(10)?),
                Command::RemoveObject(id(999)?)
            ]))
            .is_err()
    );
    assert_eq!(editor.document(), &before);
    assert_eq!(editor.undo_len(), history);
    editor.execute(Command::RemoveObject(id(1)?))?;
    assert!(editor.document().linked_children(id(10)?).next().is_none());
    editor.undo()?;
    assert_eq!(editor.document(), &before);
    Ok(())
}
