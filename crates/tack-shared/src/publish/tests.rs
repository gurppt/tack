#![allow(clippy::unwrap_used)]
use super::*;
use tack_core::{AssetId, Command, DocumentId, DocumentLimits, ImageAsset};
struct Temporary(std::path::PathBuf);
impl Temporary {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "tack-shared-publish-test-{:032x}",
            tack_storage::new_document_id().unwrap().value()
        ));
        tack_storage::create_private_directory(&path, false).unwrap();
        Self(path)
    }
}
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn reverse_snapshot_preserves_ids_and_original_hash_and_never_overwrites() {
    let root = Temporary::new();
    let original = root.0.join("original");
    std::fs::write(&original, b"original encoded image bytes").unwrap();
    let source = Source::embedded(SourceId::new(2).unwrap());
    let mut document = Document::new(DocumentId::new(1).unwrap(), DocumentLimits::default());
    document.apply(Command::AddSource(source.clone())).unwrap();
    document
        .apply(Command::AddAsset(
            ImageAsset::new(AssetId::new(3).unwrap(), source.id(), [64, 48]).unwrap(),
        ))
        .unwrap();
    let prepared =
        prepare_source(&source, Payload::File(original), &AtomicBool::new(false)).unwrap();
    let destination = root.0.join("snapshot.tack");
    let rows = vec![(prepared.binding.clone(), prepared.payload)];
    save_local_snapshot(&destination, &document, &rows).unwrap();
    let opened = TackFile::open(&destination).unwrap();
    assert_eq!(opened.document, document);
    opened.verify_originals().unwrap();
    let (hash, size) = hash_reader(&mut opened.original_reader(source.id()).unwrap()).unwrap();
    assert_eq!(hash, prepared.binding.hash);
    assert_eq!(size, prepared.binding.size);
    assert!(save_local_snapshot(&destination, &document, &rows).is_err());
    assert!(save_local_snapshot(&root.0.join("missing.tack"), &document, &[]).is_err());
    assert!(!root.0.join("missing.tack").exists());
}

#[test]
fn corrupt_embedded_crc_and_cancelled_hash_are_refused() {
    let root = Temporary::new();
    let path = root.0.join("bad");
    std::fs::write(&path, b"content").unwrap();
    let source = Source::embedded(SourceId::new(2).unwrap());
    let payload = Payload::Stored {
        file: Arc::new(File::open(path).unwrap()),
        range: tack_storage::BlobRange {
            offset: 0,
            len: 7,
            crc32: 0,
        },
    };
    assert!(prepare_source(&source, payload.clone(), &AtomicBool::new(false)).is_err());
    assert!(prepare_source(&source, payload, &AtomicBool::new(true)).is_err());
}

#[test]
fn publishing_changed_link_refuses_before_source_hashing() {
    let root = Temporary::new();
    let path = root.0.join("linked.png");
    std::fs::write(&path, b"old image").unwrap();
    let expected = fingerprint(&std::fs::metadata(&path).unwrap()).unwrap();
    let source = Source::from_descriptor(
        SourceId::new(4).unwrap(),
        SourceLocation::Linked(tack_core::LinkedPath::native(&path).unwrap()),
        1,
        Some(expected),
    )
    .unwrap();
    assert!(linked_payload(&source, &root.0.join("board.tack")).is_ok());
    std::fs::write(&path, b"changed image with different dimensions").unwrap();
    assert!(linked_payload(&source, &root.0.join("board.tack")).is_err());
}
