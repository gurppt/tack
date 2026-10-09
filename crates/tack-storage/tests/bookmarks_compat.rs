use std::{fs, path::PathBuf};
use tack_core::*;
use tack_storage::{TackFile, decode_metadata, encode_metadata, new_document_id, save};

type R<T = ()> = Result<T, Box<dyn std::error::Error>>;

struct Temp(PathBuf);
impl Temp {
    fn new() -> R<Self> {
        let path = std::env::temp_dir().join(format!(
            "tack-bookmark-compat-{}",
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

fn view(id: u128) -> R<CameraBookmark> {
    Ok(CameraBookmark::new(
        BookmarkId::new(id)?,
        format!("view{id}"),
        [123.125, -999.5],
        6.53125,
    )?)
}

#[test]
fn schema_one_two_three_remain_readable_and_four_preserves_all_object_kinds() -> R {
    // Independent legacy empty metadata records, not new-writer round trips.
    let empty = Document::new(DocumentId::new(1)?, DocumentLimits::default());
    let old1 = 1u128.to_le_bytes().to_vec();
    let mut old2 = old1.clone();
    old2.extend(0u32.to_le_bytes());
    assert_eq!(decode_metadata(1, [0; 3], &old1)?, empty);
    assert_eq!(decode_metadata(2, [0; 3], &old2)?, empty);
    assert_eq!(decode_metadata(3, [0; 3], &old2)?, empty);

    let mut doc = empty;
    doc.apply(Command::AddSource(Source::linked(
        SourceId::new(1)?,
        "absent.jpg",
    )?))?;
    doc.apply(Command::AddAsset(ImageAsset::new(
        AssetId::new(1)?,
        SourceId::new(1)?,
        [50_000; 2],
    )?))?;
    let transform = Transform::new([1., 2.], [40., 30.], 0., [false; 2])?;
    doc.apply(Command::AddObject {
        object: DocumentObject::image(ObjectId::new(1)?, AssetId::new(1)?, transform),
        index: 0,
    })?;
    doc.apply(Command::AddObject {
        object: DocumentObject::frame(ObjectId::new(2)?, "Frame".into(), transform)?,
        index: 1,
    })?;
    let kinds = [
        AnnotationKind::Text(TextObject::new(
            "note 猫\nline".into(),
            24.,
            TextAlignment::Center,
        )?),
        AnnotationKind::Rect,
        AnnotationKind::Line(LineObject::new([[0., 0.5], [1., 0.5]])?),
        AnnotationKind::Arrow(LineObject::new([[0., 0.], [1., 1.]])?),
        AnnotationKind::Scribble(ScribbleObject::new(vec![[0., 0.], [0.5, 0.7], [1., 1.]])?),
    ];
    for (i, kind) in kinds.into_iter().enumerate() {
        doc.apply(Command::AddObject {
            object: DocumentObject::annotation(
                ObjectId::new(i as u128 + 3)?,
                Annotation::new(kind, AnnotationStyle::default()),
                transform,
            )?,
            index: i + 2,
        })?;
    }
    let (schema3, counts3, bytes3) = encode_metadata(&doc)?;
    assert_eq!(schema3, 3);
    assert_eq!(decode_metadata(schema3, counts3, &bytes3)?, doc);
    doc.apply(Command::AddObject {
        object: DocumentObject::image(ObjectId::new(8)?, AssetId::new(1)?, transform),
        index: doc.object_order().len(),
    })?;
    doc.apply(Command::AddGroup(Group::new(
        GroupId::new(1)?,
        vec![ObjectId::new(1)?, ObjectId::new(8)?],
    )?))?;
    doc.apply(Command::SetCameraBookmarks(vec![view(1)?, view(2)?]))?;
    let (schema4, counts4, bytes4) = encode_metadata(&doc)?;
    assert_eq!(schema4, 4);
    assert_eq!(decode_metadata(schema4, counts4, &bytes4)?, doc);
    assert!(decode_metadata(3, counts4, &bytes4).is_err());
    assert!(decode_metadata(5, counts4, &bytes4).is_err());
    let tmp = Temp::new()?;
    let path = tmp.0.join("views.tack");
    save(&path, &doc, Vec::new())?;
    assert_eq!(&fs::read(&path)?[12..16], &4u32.to_le_bytes());
    assert_eq!(TackFile::open(&path)?.document, doc);
    Ok(())
}

#[test]
fn schema_four_rejects_malformed_bookmark_records_and_all_truncations() -> R {
    let mut doc = Document::new(DocumentId::new(1)?, DocumentLimits::default());
    doc.apply(Command::SetCameraBookmarks(vec![view(1)?, view(2)?]))?;
    let (schema, counts, bytes) = encode_metadata(&doc)?;
    // Empty doc ID, groups count and bookmark count precede 44-byte records.
    let first = 24;
    let second = first + 44 + doc.bookmarks()[0].name().len();
    let mutations = [
        (20, (MAX_CAMERA_BOOKMARKS as u32 + 1).to_le_bytes().to_vec()),
        (first, 2u16.to_le_bytes().to_vec()),
        (first + 2, 0u128.to_le_bytes().to_vec()),
        (
            second + 2,
            doc.bookmarks()[0].id().value().to_le_bytes().to_vec(),
        ),
        (first + 18, f64::NAN.to_le_bytes().to_vec()),
        (first + 26, f64::INFINITY.to_le_bytes().to_vec()),
        (first + 34, 0f64.to_le_bytes().to_vec()),
        (first + 34, 64.001f64.to_le_bytes().to_vec()),
        (
            first + 42,
            (MAX_BOOKMARK_NAME_BYTES as u16 + 1).to_le_bytes().to_vec(),
        ),
        (first + 44, vec![0xff]),
        (first + 44, vec![0]),
    ];
    for (offset, value) in mutations {
        let mut mutant = bytes.clone();
        mutant[offset..offset + value.len()].copy_from_slice(&value);
        assert!(
            decode_metadata(schema, counts, &mutant).is_err(),
            "offset {offset}"
        );
    }
    for end in 0..bytes.len() {
        assert!(
            decode_metadata(schema, counts, &bytes[..end]).is_err(),
            "prefix {end}"
        );
    }
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(decode_metadata(schema, counts, &trailing).is_err());
    assert_eq!(decode_metadata(schema, counts, &bytes)?, doc);
    Ok(())
}

#[test]
fn maximum_bookmarks_round_trip_without_renderable_or_source_records() -> R {
    let mut doc = Document::new(DocumentId::new(1)?, DocumentLimits::default());
    let views: Vec<_> = (1..=MAX_CAMERA_BOOKMARKS)
        .map(|i| {
            CameraBookmark::new(
                BookmarkId::new(i as u128)?,
                "é".repeat(64),
                [i as f64, -(i as f64)],
                if i % 2 == 0 { 0.001 } else { 64. },
            )
            .map_err(Box::<dyn std::error::Error>::from)
        })
        .collect::<R<_>>()?;
    doc.apply(Command::SetCameraBookmarks(views))?;
    let (schema, counts, bytes) = encode_metadata(&doc)?;
    assert_eq!(counts, [0; 3]);
    assert!(bytes.len() < 12 * 1024);
    assert_eq!(decode_metadata(schema, counts, &bytes)?, doc);
    let tmp = Temp::new()?;
    let path = tmp.0.join("max.tack");
    save(&path, &doc, Vec::new())?;
    let reopened = TackFile::open(path)?;
    assert_eq!(reopened.document, doc);
    assert!(reopened.originals.is_empty());
    assert!(reopened.overviews.is_empty());
    Ok(())
}
