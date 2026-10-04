use tack_core::*;
type TestResult = Result<(), Box<dyn std::error::Error>>;

fn fixture(capacity: usize) -> Result<DocumentEditor, Box<dyn std::error::Error>> {
    let mut document = Document::new(DocumentId::new(1)?, DocumentLimits::default());
    document.apply(Command::AddSource(Source::linked(
        SourceId::new(1)?,
        "missing/不存在.jpg",
    )?))?;
    document.apply(Command::AddSource(Source::embedded(SourceId::new(2)?)))?;
    document.apply(Command::AddAsset(ImageAsset::new(
        AssetId::new(1)?,
        SourceId::new(1)?,
        [6000, 4500],
    )?))?;
    document.apply(Command::AddAsset(ImageAsset::new(
        AssetId::new(2)?,
        SourceId::new(2)?,
        [1, 1],
    )?))?;
    for id in [1, 2, 3] {
        document.apply(Command::AddObject {
            object: DocumentObject::image(
                ObjectId::new(id)?,
                AssetId::new(1)?,
                Transform::new([id as f64 * 100.0, 0.0], [80.0, 40.0], 0.0, [false; 2])?,
            ),
            index: document.object_order().len(),
        })?;
    }
    Ok(DocumentEditor::new(document, capacity))
}

#[test]
fn identities_and_validation_reject_invalid_values() -> TestResult {
    assert!(ObjectId::new(0).is_err());
    assert_eq!(AssetId::new(u128::MAX)?.value(), u128::MAX);
    for size in [[0.0, 1.0], [-1.0, 1.0], [f64::NAN, 1.0], [1e10, 1.0]] {
        assert!(Transform::new([0.0; 2], size, 0.0, [false; 2]).is_err());
    }
    assert!(Transform::new([f64::INFINITY, 0.0], [1.0; 2], 0.0, [false; 2]).is_err());
    assert!(Transform::new([0.0; 2], [1.0; 2], f64::NAN, [false; 2]).is_err());
    for value in [f64::NAN, f64::INFINITY, -0.1, 1.01] {
        assert!(Opacity::new(value).is_err());
    }
    assert_eq!(Opacity::new(0.0)?.value(), 0.0);
    for crop in [
        [0.0, 0.0, 0.0, 1.0],
        [-0.1, 0.0, 1.0, 1.0],
        [0.5, 0.0, 0.6, 1.0],
        [0.0, 0.0, f64::NAN, 1.0],
    ] {
        assert!(Crop::new(crop[0], crop[1], crop[2], crop[3]).is_err());
    }
    assert!(ImageAsset::new(AssetId::new(1)?, SourceId::new(1)?, [0, 1]).is_err());
    assert!(Source::linked(SourceId::new(1)?, "").is_err());
    assert!(Source::linked(SourceId::new(1)?, "x".repeat(MAX_SOURCE_PATH_BYTES + 1)).is_err());
    assert!(Source::linked(SourceId::new(1)?, "x\0y").is_err());
    // Finite components can still rotate beyond world limits.
    assert!(Transform::new([0.0; 2], [9e8; 2], std::f64::consts::FRAC_PI_4, [false; 2]).is_err());
    Ok(())
}

#[test]
fn creation_commands_undo_shared_asset_relationships_exactly() -> TestResult {
    let initial = Document::new(DocumentId::new(1)?, DocumentLimits::default());
    let mut editor = DocumentEditor::new(initial.clone(), 8);
    let source = Source::linked(SourceId::new(10)?, "not-opened/源.jpg")?;
    let asset = ImageAsset::new(AssetId::new(20)?, source.id(), [u32::MAX, u32::MAX])?;
    let transform = Transform::new([0.0; 2], [20.0, 10.0], 0.0, [false; 2])?;
    let mut states = vec![initial];
    for command in [
        Command::AddSource(source),
        Command::AddAsset(asset),
        Command::AddObject {
            object: DocumentObject::image(ObjectId::new(30)?, asset.id(), transform),
            index: 0,
        },
        Command::AddObject {
            object: DocumentObject::image(ObjectId::new(40)?, asset.id(), transform),
            index: 0,
        },
    ] {
        editor.execute(command)?;
        states.push(editor.document().clone());
    }
    for id in [30, 40] {
        assert_eq!(
            editor
                .document()
                .object_render_data(ObjectId::new(id)?)
                .ok_or("missing")?
                .asset_id,
            asset.id()
        );
    }
    // Massive source dimensions are metadata only, never pixel allocation.
    assert_eq!(editor.document().asset(asset.id()).copied(), Some(asset));
    for state in states[..4].iter().rev() {
        editor.undo()?;
        assert_eq!(editor.document(), state);
    }
    for state in &states[1..] {
        editor.redo()?;
        assert_eq!(editor.document(), state);
    }
    assert_eq!(
        editor.execute(Command::AddAsset(asset)),
        Err(CommandError::DuplicateAsset(asset.id()))
    );
    Ok(())
}

#[test]
fn commands_restore_exact_state_and_explicit_forward_properties() -> TestResult {
    let object = ObjectId::new(2)?;
    let transform = Transform::new([-500.0, 200.0], [120.0, 45.0], 0.8, [true, false])?;
    let crop = Crop::new(0.1, 0.2, 0.7, 0.6)?;
    let opacity = Opacity::new(0.25)?;
    let mut editor = fixture(100)?;
    let commands = [
        Command::SetTransform { object, transform },
        Command::SetCrop { object, crop },
        Command::SetOpacity { object, opacity },
        Command::SetImageFiltering {
            object,
            filtering: ImageFiltering::Nearest,
        },
        Command::SetZOrder { object, index: 0 },
        Command::RemoveObject(ObjectId::new(1)?),
        Command::AddObject {
            object: DocumentObject::image(ObjectId::new(4)?, AssetId::new(2)?, transform),
            index: 1,
        },
    ];
    let initial = editor.document().clone();
    let mut states = vec![initial.clone()];
    for command in &commands {
        assert!(editor.execute(command.clone())?);
        states.push(editor.document().clone());
    }
    let data = editor
        .document()
        .object_render_data(object)
        .ok_or("missing object")?;
    assert_eq!(
        (data.transform, data.crop, data.opacity, data.filtering),
        (transform, crop, opacity, ImageFiltering::Nearest)
    );
    assert_eq!(
        editor.document().object_order(),
        &[object, ObjectId::new(4)?, ObjectId::new(3)?]
    );
    assert_eq!(data.asset_id, AssetId::new(1)?);
    for state in states[..7].iter().rev() {
        assert!(editor.undo()?);
        assert_eq!(editor.document(), state);
    }
    assert!(!editor.undo()?);
    for state in &states[1..] {
        assert!(editor.redo()?);
        assert_eq!(editor.document(), state);
    }
    assert!(!editor.redo()?);
    // Source and asset removal/restoration is also exact, including path bytes.
    let before = editor.document().clone();
    for id in editor.document().object_order().to_vec() {
        editor.execute(Command::RemoveObject(id))?;
    }
    editor.execute(Command::RemoveAsset(AssetId::new(1)?))?;
    editor.execute(Command::RemoveSource(SourceId::new(1)?))?;
    for _ in 0..5 {
        editor.undo()?;
    }
    assert_eq!(editor.document(), &before);
    Ok(())
}

#[test]
fn invalid_commands_are_atomic_and_preserve_redo() -> TestResult {
    let mut editor = fixture(10)?;
    let object = ObjectId::new(1)?;
    editor.execute(Command::RemoveObject(object))?;
    editor.undo()?;
    let before = editor.document().clone();
    let duplicate = editor.document().object(object).ok_or("missing")?.clone();
    let invalid = [
        Command::RemoveObject(ObjectId::new(99)?),
        Command::SetOpacity {
            object: ObjectId::new(99)?,
            opacity: Opacity::OPAQUE,
        },
        Command::AddObject {
            object: duplicate.clone(),
            index: 0,
        },
        Command::AddObject {
            object: DocumentObject::image(
                ObjectId::new(99)?,
                AssetId::new(99)?,
                duplicate.transform(),
            ),
            index: 0,
        },
        Command::AddObject {
            object: DocumentObject::image(
                ObjectId::new(99)?,
                AssetId::new(1)?,
                duplicate.transform(),
            ),
            index: 99,
        },
        Command::SetZOrder { object, index: 3 },
        Command::RemoveAsset(AssetId::new(1)?),
        Command::RemoveSource(SourceId::new(1)?),
        Command::AddAsset(ImageAsset::new(
            AssetId::new(99)?,
            SourceId::new(99)?,
            [1, 1],
        )?),
        Command::AddSource(Source::embedded(SourceId::new(1)?)),
        Command::RemoveAsset(AssetId::new(99)?),
        Command::RemoveSource(SourceId::new(99)?),
    ];
    for command in invalid {
        assert!(editor.execute(command).is_err());
        assert_eq!(editor.document(), &before);
        assert_eq!((editor.undo_len(), editor.redo_len()), (0, 1));
    }
    assert!(!editor.execute(Command::SetOpacity {
        object,
        opacity: Opacity::OPAQUE
    })?);
    assert_eq!(editor.redo_len(), 1);
    editor.execute(Command::SetOpacity {
        object,
        opacity: Opacity::new(0.5)?,
    })?;
    assert_eq!(editor.redo_len(), 0);
    Ok(())
}

#[test]
fn metadata_limits_and_history_capacity_are_explicit() -> TestResult {
    let mut doc = Document::new(
        DocumentId::new(1)?,
        DocumentLimits {
            objects: 0,
            assets: 1,
            sources: 1,
        },
    );
    doc.apply(Command::AddSource(Source::embedded(SourceId::new(1)?)))?;
    assert_eq!(
        doc.apply(Command::AddSource(Source::embedded(SourceId::new(2)?))),
        Err(CommandError::LimitReached("sources"))
    );
    let asset = ImageAsset::new(AssetId::new(1)?, SourceId::new(1)?, [1, 1])?;
    doc.apply(Command::AddAsset(asset))?;
    assert_eq!(
        doc.apply(Command::AddAsset(ImageAsset::new(
            AssetId::new(2)?,
            SourceId::new(1)?,
            [1, 1]
        )?)),
        Err(CommandError::LimitReached("assets"))
    );
    let object = DocumentObject::image(
        ObjectId::new(1)?,
        AssetId::new(1)?,
        Transform::new([0.0; 2], [1.0; 2], 0.0, [false; 2])?,
    );
    assert_eq!(
        doc.apply(Command::AddObject { object, index: 0 }),
        Err(CommandError::LimitReached("objects"))
    );
    let mut editor = fixture(2)?;
    for v in [0.1, 0.2, 0.3] {
        editor.execute(Command::SetOpacity {
            object: ObjectId::new(1)?,
            opacity: Opacity::new(v)?,
        })?;
    }
    assert_eq!(editor.undo_len(), 2);
    editor.undo()?;
    editor.undo()?;
    assert!(!editor.undo()?);
    assert_eq!(
        editor
            .document()
            .object_render_data(ObjectId::new(1)?)
            .ok_or("missing")?
            .opacity
            .value(),
        0.1
    );
    assert_eq!(editor.redo_len(), 2);
    editor.clear_history();
    assert_eq!((editor.undo_len(), editor.redo_len()), (0, 0));
    let mut no_history = fixture(0)?;
    no_history.execute(Command::RemoveObject(ObjectId::new(1)?))?;
    assert!(!no_history.undo()?);
    Ok(())
}

#[test]
fn deterministic_random_sequences_round_trip_without_snapshots_in_history() -> TestResult {
    let mut editor = fixture(512)?;
    let initial = editor.document().clone();
    let mut seed = 0x1a_u64;
    for step in 0..400 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let object = ObjectId::new(u128::from(seed % 3 + 1))?;
        let command = match seed % 5 {
            0 => Command::SetOpacity {
                object,
                opacity: Opacity::new((seed % 100) as f64 / 100.0)?,
            },
            1 => Command::SetZOrder {
                object,
                index: (seed % 3) as usize,
            },
            2 => Command::SetImageFiltering {
                object,
                filtering: if seed & 8 == 0 {
                    ImageFiltering::Smooth
                } else {
                    ImageFiltering::Nearest
                },
            },
            3 => Command::SetCrop {
                object,
                crop: Crop::new(0.1, 0.1, 0.8, 0.8)?,
            },
            _ => Command::SetTransform {
                object,
                transform: Transform::new(
                    [step as f64, -(step as f64)],
                    [100.0, 50.0],
                    step as f64 * 0.01,
                    [false, true],
                )?,
            },
        };
        editor.execute(command)?;
    }
    let forward = editor.document().clone();
    while editor.undo()? {}
    assert_eq!(editor.document(), &initial);
    while editor.redo()? {}
    assert_eq!(editor.document(), &forward);
    Ok(())
}

#[test]
fn queries_cull_rotated_bounds_in_order_without_resolving_sources() -> TestResult {
    let mut editor = fixture(10)?;
    let id = ObjectId::new(3)?;
    editor.execute(Command::SetTransform {
        object: id,
        transform: Transform::new(
            [150.0, 0.0],
            [80.0, 40.0],
            std::f64::consts::FRAC_PI_2,
            [false; 2],
        )?,
    })?;
    editor.execute(Command::SetZOrder {
        object: id,
        index: 0,
    })?;
    let view = WorldRect::new(130.0, -30.0, 80.0, 60.0)?;
    let visible: Vec<_> = editor
        .document()
        .objects_in_view(view)
        .map(|d| d.object_id)
        .collect();
    assert_eq!(visible, vec![id, ObjectId::new(1)?, ObjectId::new(2)?]);
    let snapshot = editor.document().object_render_data(id).ok_or("missing")?;
    editor.execute(Command::RemoveObject(id))?;
    assert_eq!(snapshot.asset_id, AssetId::new(1)?);
    assert!(editor.document().object_render_data(id).is_none());
    Ok(())
}
