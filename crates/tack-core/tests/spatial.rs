use tack_core::*;
type R = Result<(), Box<dyn std::error::Error>>;
fn doc() -> Result<Document, Box<dyn std::error::Error>> {
    let mut d = Document::new(DocumentId::new(1)?, DocumentLimits::default());
    d.apply(Command::AddSource(Source::linked(
        SourceId::new(1)?,
        "missing.png",
    )?))?;
    d.apply(Command::AddAsset(ImageAsset::new(
        AssetId::new(1)?,
        SourceId::new(1)?,
        [100, 100],
    )?))?;
    for i in 1..=3 {
        d.apply(Command::AddObject {
            object: DocumentObject::image(
                ObjectId::new(i)?,
                AssetId::new(1)?,
                Transform::new([i as f64 * 100., 0.], [80., 40.], 0., [false; 2])?,
            ),
            index: d.object_order().len(),
        })?;
    }
    Ok(d)
}
#[test]
fn groups_are_flat_and_rollback_and_delete_undo_are_exact() -> R {
    let mut e = DocumentEditor::new(doc()?, 200);
    let initial = e.document().clone();
    let g = Group::new(GroupId::new(1)?, vec![ObjectId::new(2)?, ObjectId::new(1)?])?;
    assert!(Group::new(g.id(), vec![ObjectId::new(1)?, ObjectId::new(1)?]).is_err());
    assert!(Group::new(g.id(), vec![ObjectId::new(1)?]).is_err());
    e.execute(Command::AddGroup(g.clone()))?;
    let grouped = e.document().clone();
    assert!(e.execute(Command::RemoveObject(ObjectId::new(1)?)).is_err());
    assert!(
        e.execute(Command::AddGroup(Group::new(
            GroupId::new(2)?,
            vec![ObjectId::new(1)?, ObjectId::new(3)?]
        )?))
        .is_err()
    );
    assert!(
        e.execute(Command::Batch(vec![
            Command::RemoveGroup(g.id()),
            Command::RemoveObject(ObjectId::new(1)?),
            Command::RemoveObject(ObjectId::new(99)?)
        ]))
        .is_err()
    );
    assert_eq!(e.document(), &grouped);
    assert_eq!(e.undo_len(), 1);
    e.execute(Command::Batch(vec![
        Command::RemoveGroup(g.id()),
        Command::RemoveObject(ObjectId::new(1)?),
        Command::RemoveObject(ObjectId::new(2)?),
    ]))?;
    assert_eq!(e.document().object_order().len(), 1);
    assert!(e.document().groups().next().is_none());
    e.undo()?;
    assert_eq!(e.document(), &grouped);
    e.redo()?;
    e.undo()?;
    e.undo()?;
    assert_eq!(e.document(), &initial);
    Ok(())
}
#[test]
fn frames_are_named_axis_rectangles_without_media_or_ownership() -> R {
    let mut e = DocumentEditor::new(doc()?, 200);
    let original = e.document().clone();
    let id = ObjectId::new(100)?;
    let t = Transform::new([200., 0.], [400., 200.], 0., [false; 2])?;
    e.execute(Command::AddObject {
        object: DocumentObject::frame(id, "Références 猫 Ж 😀".into(), t)?,
        index: 0,
    })?;
    assert!(e.document().object_render_data(id).is_none());
    assert!(DocumentObject::frame(id, "".into(), t).is_err());
    assert!(DocumentObject::frame(id, "\n".into(), t).is_err());
    assert!(DocumentObject::frame(id, "é".repeat(129), t).is_err());
    assert!(
        e.execute(Command::SetTransform {
            object: id,
            transform: Transform::new([0., 0.], [20., 20.], 0.1, [false; 2])?
        })
        .is_err()
    );
    assert!(
        e.execute(Command::SetCrop {
            object: id,
            crop: Crop::FULL
        })
        .is_err()
    );
    assert!(
        e.execute(Command::AddGroup(Group::new(
            GroupId::new(3)?,
            vec![id, ObjectId::new(1)?]
        )?))
        .is_err()
    );
    let named = e.document().clone();
    e.execute(Command::Batch(vec![
        Command::SetFrameName {
            object: id,
            name: "Autres 猫".into(),
        },
        Command::SetTransform {
            object: id,
            transform: Transform::new([700., 300.], [500., 400.], 0., [false; 2])?,
        },
    ]))?;
    for i in original.object_order() {
        assert_eq!(original.object(*i), e.document().object(*i));
    }
    e.undo()?;
    assert_eq!(e.document(), &named);
    e.undo()?;
    assert_eq!(e.document(), &original);
    Ok(())
}
#[test]
fn history_accounts_for_owned_group_and_frame_payloads_inside_batches() -> R {
    let group = Group::new(
        GroupId::new(1)?,
        (1..1000)
            .map(ObjectId::new)
            .collect::<Result<Vec<_>, _>>()?,
    )?;
    let c = Command::Batch(vec![
        Command::AddGroup(group),
        Command::SetFrameName {
            object: ObjectId::new(1)?,
            name: "x".repeat(256),
        },
    ]);
    assert!(
        c.retained_bytes()
            >= 999 * std::mem::size_of::<ObjectId>() + 256 + 3 * std::mem::size_of::<Command>()
    );
    Ok(())
}
