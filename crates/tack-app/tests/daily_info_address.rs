use tack_app::{shared_address::SharedAddress, source_info};
use tack_assets::SourceState;
use tack_core::*;
type R = Result<(), Box<dyn std::error::Error + Send + Sync>>;
#[test]
fn shared_descriptor_is_strict_bounded_and_canonical() -> R {
    let id = "00000000000000000000000000000001";
    for endpoint in ["127.0.0.1:7337", "192.168.1.42:65535", "[::1]:7337"] {
        let parsed = SharedAddress::parse(&format!("{endpoint} {id}"))?;
        assert_eq!(SharedAddress::parse(&parsed.canonical())?, parsed);
    }
    for text in [
        "",
        "localhost:7337 1",
        "0.0.0.0:7337 1",
        "127.0.0.1:0 1",
        "127.0.0.1:65536 1",
        "tack://127.0.0.1:7337/00000000000000000000000000000000",
        "tack://127.0.0.1:7337/abcd/extra",
        "http://127.0.0.1:7337/1",
        "127.0.0.1:7337 1\nextra",
    ] {
        assert!(SharedAddress::parse(text).is_err(), "{text}");
    }
    assert!(SharedAddress::parse(&"x".repeat(257)).is_err());
    Ok(())
}
#[test]
fn metadata_info_works_for_unreadable_and_huge_sources_without_io_or_decode() -> R {
    for (path, state) in [
        (Some("/definitely/absent/huge.jpg"), SourceState::Missing),
        (
            Some("/definitely/absent/huge.png"),
            SourceState::Unavailable,
        ),
        (None, SourceState::Embedded),
    ] {
        let mut doc = Document::new(DocumentId::new(1)?, DocumentLimits::default());
        let source = match path {
            Some(p) => Source::linked(SourceId::new(1)?, p)?,
            None => Source::embedded(SourceId::new(1)?),
        };
        doc.apply(Command::AddSource(source))?;
        doc.apply(Command::AddAsset(ImageAsset::new(
            AssetId::new(1)?,
            SourceId::new(1)?,
            [50000; 2],
        )?))?;
        doc.apply(Command::AddObject {
            object: DocumentObject::image(
                ObjectId::new(1)?,
                AssetId::new(1)?,
                Transform::new([0.; 2], [100.; 2], 0., [false; 2])?,
            ),
            index: 0,
        })?;
        let before = doc.clone();
        let rows = source_info::rows(&doc, ObjectId::new(1)?, Some(state), Some(33_000_000))?;
        assert!(rows.iter().any(|r| r.contains("50000 x 50000")));
        assert!(rows.iter().any(|r| r.contains("33000000 encoded bytes")));
        assert!(rows.iter().any(|r| r.contains(&format!("{state:?}"))));
        assert_eq!(doc, before);
        assert!(source_info::rows(&doc, ObjectId::new(99)?, None, None).is_err());
    }
    Ok(())
}
