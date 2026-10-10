use std::io::Cursor;
use tack_core::*;
use tack_shared::*;

fn fixture() -> std::result::Result<Document, Box<dyn std::error::Error>> {
    let mut doc = Document::new(DocumentId::new(1)?, DocumentLimits::default());
    let source = SourceId::new(2)?;
    let asset = AssetId::new(3)?;
    doc.apply(Command::AddSource(Source::embedded(source)))?;
    doc.apply(Command::AddAsset(ImageAsset::new(
        asset,
        source,
        [160, 90],
    )?))?;
    let transform = Transform::new([30., 40.], [160., 90.], 0.5, [true, false])?;
    let object = DocumentObject::image_with_properties(
        ObjectId::new(4)?,
        asset,
        transform,
        Crop::new(0.1, 0.2, 0.5, 0.4)?,
        Opacity::new(0.7)?,
        ImageFiltering::Nearest,
    );
    doc.apply(Command::AddObject { object, index: 0 })?;
    let frame = DocumentObject::frame(
        ObjectId::new(5)?,
        "Frame é".to_owned(),
        Transform::new([10., 20.], [300., 200.], 0., [false; 2])?,
    )?;
    doc.apply(Command::AddObject {
        object: frame,
        index: 1,
    })?;
    for (index, kind) in [
        AnnotationKind::Rect,
        AnnotationKind::Text(TextObject::new(
            "Note\n\t東京".to_owned(),
            18.,
            TextAlignment::Center,
        )?),
        AnnotationKind::Line(LineObject::new([[0., 0.], [1., 1.]])?),
        AnnotationKind::Arrow(LineObject::new([[0., 1.], [1., 0.]])?),
        AnnotationKind::Scribble(ScribbleObject::new(vec![[0., 0.], [0.5, 0.7], [1., 1.]])?),
    ]
    .into_iter()
    .enumerate()
    {
        let annotation = Annotation::new(
            kind,
            AnnotationStyle::new(
                Color([1, 2, 3, 4]),
                Some(Color([5, 6, 7, 8])),
                4.,
                Opacity::new(0.6)?,
            )?,
        );
        let object = DocumentObject::annotation(
            ObjectId::new(10 + index as u128)?,
            annotation,
            Transform::new([20., 20.], [200., 100.], 0., [false; 2])?,
        )?;
        doc.apply(Command::AddObject {
            object,
            index: index + 2,
        })?;
    }
    Ok(doc)
}
#[test]
fn snapshot_preserves_all_durable_kinds_and_properties()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    let doc = fixture()?;
    let record = DocumentRecord::from_document(&doc)?;
    assert_eq!(record.to_document()?, doc);
    let message = Message::Snapshot {
        board: WireId::new(1)?,
        revision: 18,
        source_high_water: 2,
        document: record,
        sources: vec![],
        clients: 3,
    };
    let mut frame = Vec::new();
    let size = write_message(&mut frame, &message)?;
    assert_eq!(size, frame.len());
    assert_eq!(read_message(&mut Cursor::new(frame))?, message);
    Ok(())
}
#[test]
fn semantic_command_roundtrip_all_variants() -> std::result::Result<(), Box<dyn std::error::Error>>
{
    let doc = fixture()?;
    let object = ObjectId::new(4)?;
    let source = SourceId::new(2)?;
    let asset = ImageAsset::new(AssetId::new(3)?, source, [160, 90])?;
    let mut commands = vec![
        Command::AddSource(Source::embedded(source)),
        Command::RemoveSource(source),
        Command::SetSource(Source::from_descriptor(
            source,
            SourceLocation::Embedded,
            2,
            None,
        )?),
        Command::AddAsset(asset),
        Command::RemoveAsset(asset.id()),
        Command::SetAsset(asset),
        Command::RemoveObject(object),
        Command::SetTransform {
            object,
            transform: Transform::new([3., 4.], [50., 60.], 0.3, [true, true])?,
        },
        Command::SetCrop {
            object,
            crop: Crop::new(0., 0., 0.3, 0.4)?,
        },
        Command::SetOpacity {
            object,
            opacity: Opacity::new(0.9)?,
        },
        Command::SetImageFiltering {
            object,
            filtering: ImageFiltering::Smooth,
        },
        Command::AddGroup(Group::new(
            GroupId::new(30)?,
            vec![object, ObjectId::new(31)?],
        )?),
        Command::RemoveGroup(GroupId::new(30)?),
        Command::SetFrameName {
            object: ObjectId::new(5)?,
            name: "rename".to_owned(),
        },
        Command::SetAnnotationStyle {
            object: ObjectId::new(10)?,
            style: AnnotationStyle::default(),
        },
        Command::SetText {
            object: ObjectId::new(11)?,
            text: TextObject::new("Changed".to_owned(), 14., TextAlignment::Right)?,
        },
        Command::SetZOrder { object, index: 2 },
    ];
    for (index, object) in doc.objects().enumerate() {
        commands.push(Command::AddObject {
            object: object.clone(),
            index,
        });
    }
    commands.push(Command::Batch(vec![Command::SetOpacity {
        object,
        opacity: Opacity::new(0.8)?,
    }]));
    for command in commands {
        let dto = CommandDto::from_command(&command)?;
        let bytes = serde_json::to_vec(&dto)?;
        let decoded: CommandDto = serde_json::from_slice(&bytes)?;
        assert_eq!(decoded.to_command()?, command);
    }
    Ok(())
}
#[test]
fn major_magic_truncation_and_declared_oversize_refused_before_body()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    let message = Message::Hello {
        board: WireId::new(1)?,
        client: WireId::new(2)?,
        revision: 0,
    };
    let mut frame = Vec::new();
    write_message(&mut frame, &message)?;
    for len in 0..frame.len() {
        assert!(
            read_message(&mut Cursor::new(&frame[..len])).is_err(),
            "accepted truncation {len}"
        );
    }
    let mut wrong = frame.clone();
    wrong[4..6].copy_from_slice(&2u16.to_be_bytes());
    assert!(matches!(
        read_message(&mut Cursor::new(wrong)),
        Err(Error::Version(2))
    ));
    let mut wrong = frame.clone();
    wrong[0] = 0;
    assert!(read_message(&mut Cursor::new(wrong)).is_err());
    let mut header = frame[..10].to_vec();
    header[6..10].copy_from_slice(&u32::MAX.to_be_bytes());
    assert!(read_message(&mut Cursor::new(header)).is_err());
    Ok(())
}
#[test]
fn malformed_ids_unknown_fields_and_geometry_are_refused() {
    for payload in [br#"{"type":"hello","board":"00000000000000000000000000000000","client":"00000000000000000000000000000002","revision":0}"#.as_slice(),br#"{"type":"hello","board":"../../x","client":"00000000000000000000000000000002","revision":0}"#.as_slice(),br#"{"type":"hello","board":"00000000000000000000000000000001","client":"00000000000000000000000000000002","revision":0,"extra":true}"#.as_slice(),br#"{"type":"unknown"}"#.as_slice(),br#"{"type":"edit","operation":"00000000000000000000000000000001","base":0,"sources":[],"command":{"kind":"set_opacity","object":"00000000000000000000000000000001","opacity":3}}"#.as_slice()]{assert!(decode_message(payload).is_err());}
}
#[test]
fn asset_range_and_hex_bounds() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let hash = ContentHash::digest(b"hello");
    assert_eq!(
        hash.to_string(),
        "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
    );
    for message in [
        Message::AssetGet {
            hash: hash.clone(),
            offset: 0,
            length: 0,
        },
        Message::AssetGet {
            hash: hash.clone(),
            offset: 0,
            length: (MAX_CHUNK_BYTES + 1) as u32,
        },
        Message::AssetGet {
            hash: hash.clone(),
            offset: u64::MAX,
            length: 1,
        },
        Message::AssetData {
            hash: hash.clone(),
            offset: 4,
            size: 4,
            bytes: "01".to_owned(),
        },
        Message::AssetChunk {
            hash: hash.clone(),
            offset: 0,
            bytes: "0Z".to_owned(),
        },
        Message::AssetChunk {
            hash: hash.clone(),
            offset: 0,
            bytes: "01".repeat(MAX_CHUNK_BYTES + 1),
        },
    ] {
        assert!(encode_message(&message).is_err());
    }
    assert!(ContentHash::parse("../../bad").is_err());
    assert!(ContentHash::parse(&"A".repeat(64)).is_err());
    let (stream_hash, size) = hash_reader(&mut Cursor::new(b"hello"))?;
    assert_eq!(stream_hash, hash);
    assert_eq!(size, 5);
    Ok(())
}
#[test]
fn remote_paths_are_never_authority() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let mut doc = fixture()?;
    let source = SourceId::new(2)?;
    doc.apply(Command::SetSource(Source::from_descriptor(
        source,
        SourceLocation::Linked(LinkedPath::native(std::path::Path::new("/etc/passwd"))?),
        2,
        None,
    )?))?;
    let converted = DocumentRecord::from_document(&doc)?.to_document()?;
    assert!(
        converted
            .sources()
            .all(|s| matches!(s.location(), SourceLocation::Embedded))
    );
    assert_eq!(converted.object_order(), doc.object_order());
    let (schema, counts, bytes) = tack_storage::encode_metadata(&doc)?;
    let hostile = DocumentRecord {
        schema,
        counts,
        metadata: encode_hex(&bytes),
    };
    assert!(hostile.to_document().is_err());
    Ok(())
}
#[test]
fn bounded_flat_batch_and_malformed_metadata() -> std::result::Result<(), Box<dyn std::error::Error>>
{
    assert!(
        CommandDto::Batch {
            edits: vec![CommandDto::Batch { edits: vec![] }]
        }
        .to_command()
        .is_err()
    );
    let command = CommandDto::RemoveObject {
        object: WireId::new(1)?,
    };
    assert!(
        CommandDto::Batch {
            edits: vec![command; 4097]
        }
        .to_command()
        .is_err()
    );
    let record = DocumentRecord {
        schema: 1,
        counts: [100_000; 3],
        metadata: "00".repeat(16),
    };
    assert!(record.to_document().is_err());
    let mut record = DocumentRecord::from_document(&fixture()?)?;
    record.schema = 99;
    assert!(record.to_document().is_err());
    Ok(())
}

#[test]
fn snapshot_source_revision_floor_survives_historical_restore()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    let document = DocumentRecord::from_document(&fixture()?)?;
    let mut message = Message::Snapshot {
        board: WireId::new(1)?,
        revision: 5,
        source_high_water: 2,
        document,
        sources: vec![],
        clients: 1,
    };
    assert!(encode_message(&message).is_ok());
    if let Message::Snapshot {
        source_high_water, ..
    } = &mut message
    {
        *source_high_water = 0;
    }
    assert!(encode_message(&message).is_err());
    Ok(())
}
