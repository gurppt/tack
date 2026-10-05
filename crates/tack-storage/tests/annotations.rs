use std::fs;
use tack_core::*;
use tack_storage::{TackFile, new_document_id, save};
type R = Result<(), Box<dyn std::error::Error>>;
#[test]
fn annotation_schema_roundtrip_refuses_malformed_authority_without_mutation() -> R {
    let root =
        std::env::temp_dir().join(format!("tack-annotations-{}", new_document_id()?.value()));
    fs::create_dir(&root)?;
    let path = root.join("a.tack");
    let mut d = Document::new(DocumentId::new(1)?, DocumentLimits::default());
    let kinds = vec![
        AnnotationKind::Text(TextObject::new(
            "UTF-8 猫 😀 é\nمرحبا".into(),
            24.,
            TextAlignment::Center,
        )?),
        AnnotationKind::Rect,
        AnnotationKind::Line(LineObject::new([[0., 0.5], [1., 0.5]])?),
        AnnotationKind::Arrow(LineObject::new([[1., 0.], [0., 1.]])?),
        AnnotationKind::Scribble(ScribbleObject::new(vec![[0., 0.], [0.3, 0.8], [1., 1.]])?),
    ];
    for (i, k) in kinds.into_iter().enumerate() {
        d.apply(Command::AddObject {
            object: DocumentObject::annotation(
                ObjectId::new(i as u128 + 1)?,
                Annotation::new(
                    k,
                    AnnotationStyle::new(
                        Color([3, 5, 8, 200]),
                        Some(Color([10, 20, 30, 90])),
                        2.5,
                        Opacity::new(0.6)?,
                    )?,
                ),
                Transform::new([i as f64 * 100., 20.], [80., 50.], 0.25, [true, false])?,
            )?,
            index: i,
        })?;
    }
    d.apply(Command::AddObject {
        object: DocumentObject::frame(
            ObjectId::new(99)?,
            "Frame".into(),
            Transform::new([0., 0.], [400., 200.], 0., [false; 2])?,
        )?,
        index: 0,
    })?;
    save(&path, &d, vec![])?;
    let original = fs::read(&path)?;
    assert_eq!(&original[12..16], &3u32.to_le_bytes());
    assert_eq!(TackFile::open(&path)?.document, d);
    let authlen = u64::from_le_bytes(original[16..24].try_into()?) as usize;
    // First frame is 59 bytes; first text starts after document id + frame.
    let text_start = 80 + 16 + 20 + 32 + 2 + 5;
    for (offset, bytes) in [
        (12, 4u32.to_le_bytes().to_vec()),
        (text_start + 2, 99u16.to_le_bytes().to_vec()),
        (text_start + 20, u32::MAX.to_le_bytes().to_vec()),
        (text_start + 24, f64::NAN.to_le_bytes().to_vec()),
        (text_start + 24 + 42 + 9, f64::NAN.to_le_bytes().to_vec()),
        (text_start + 24 + 67 + 8, vec![99]),
        (text_start + 24 + 67 + 9, u32::MAX.to_le_bytes().to_vec()),
    ] {
        let mut bad = original.clone();
        bad[offset..offset + bytes.len()].copy_from_slice(&bytes);
        let crc = crc32fast::hash(&bad[80..80 + authlen]);
        bad[40..44].copy_from_slice(&crc.to_le_bytes());
        fs::write(&path, &bad)?;
        assert!(TackFile::open(&path).is_err(), "offset {offset}");
        assert_eq!(fs::read(&path)?, bad);
    }
    fs::write(&path, &original)?;
    assert_eq!(TackFile::open(&path)?.document, d);
    fs::remove_dir_all(root)?;
    Ok(())
}
