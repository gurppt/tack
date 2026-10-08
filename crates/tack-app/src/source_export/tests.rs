use super::*;
use tack_core::*;
type R<T = ()> = Result<T, AssetError>;
struct Root(PathBuf);
impl Root {
    fn new() -> R<Self> {
        let path = std::env::temp_dir().join(format!(
            "tack-export-test-{:032x}",
            tack_storage::new_document_id()?.value()
        ));
        fs::create_dir(&path)?;
        Ok(Self(path))
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn document(location: SourceLocation) -> R<Document> {
    let mut doc = Document::new(DocumentId::new(1)?, DocumentLimits::default());
    doc.apply(Command::AddSource(Source::from_descriptor(
        SourceId::new(1)?,
        location,
        1,
        None,
    )?))?;
    doc.apply(Command::AddAsset(ImageAsset::new(
        AssetId::new(1)?,
        SourceId::new(1)?,
        [6000, 3500],
    )?))?;
    let object = DocumentObject::image(
        ObjectId::new(1)?,
        AssetId::new(1)?,
        Transform::new([20., 30.], [80., 60.], 0.7, [true, false])?,
    );
    doc.apply(Command::AddObject { object, index: 0 })?;
    doc.apply(Command::SetCrop {
        object: ObjectId::new(1)?,
        crop: Crop::new(0.1, 0.2, 0.7, 0.7)?,
    })?;
    Ok(doc)
}
#[test]
fn linked_and_embedded_large_originals_copy_identically_without_baking_geometry() -> R {
    let root = Root::new()?;
    let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
    bytes.extend((0..(3 * 128 * 1024 + 37)).map(|index| (index % 251) as u8));
    let source_path = root.0.join("production.v3.PNG");
    fs::write(&source_path, &bytes)?;
    let stop = AtomicBool::new(false);
    for embedded in [false, true] {
        let location = if embedded {
            SourceLocation::Embedded
        } else {
            SourceLocation::Linked(LinkedPath::native(&source_path)?)
        };
        let doc = document(location)?;
        let board_path = root.0.join(if embedded {
            "embedded.tack"
        } else {
            "linked.tack"
        });
        let originals = if embedded {
            vec![tack_storage::BlobInput::original(
                SourceId::new(1)?,
                1,
                Payload::File(source_path.clone()),
            )]
        } else {
            vec![]
        };
        tack_storage::save(&board_path, &doc, originals)?;
        let board = TackFile::open(&board_path)?;
        let request = OriginalExport::new(
            &doc,
            ObjectId::new(1)?,
            &board_path,
            &board,
            &Originals::new(),
        )?;
        let payload = request.payload()?;
        let name = request.suggested_name(&payload)?;
        if embedded {
            assert!(name.to_string_lossy().ends_with(".png"));
        } else {
            assert_eq!(name, "production.v3.PNG");
        }
        let output = root.0.join(if embedded {
            "extracted.PNG"
        } else {
            "copy.PNG"
        });
        copy_atomic(&payload, &output, &stop, embedded)?;
        let result = fs::read(output)?;
        assert_eq!(crc32fast::hash(&result), crc32fast::hash(&bytes));
        assert_eq!(result, bytes);
        assert_eq!(doc, board.document);
    }
    Ok(())
}
#[test]
fn corrupt_truncated_cancelled_and_invalid_target_exports_retain_destination() -> R {
    let root = Root::new()?;
    let bytes = vec![42u8; 300 * 1024];
    let source = root.0.join("source.png");
    fs::write(&source, &bytes)?;
    let file = Arc::new(File::open(&source)?);
    let target = root.0.join("target.png");
    fs::write(&target, b"existing destination")?;
    let payload = |length, crc| Payload::Stored {
        file: Arc::clone(&file),
        range: tack_storage::BlobRange {
            offset: 0,
            len: length,
            crc32: crc,
        },
    };
    let valid = payload(bytes.len() as u64, crc32fast::hash(&bytes));
    assert!(copy_atomic(&valid, &target, &AtomicBool::new(true), true).is_err());
    assert!(
        copy_atomic(
            &payload(bytes.len() as u64, 0),
            &target,
            &AtomicBool::new(false),
            true
        )
        .is_err()
    );
    assert!(
        copy_atomic(
            &payload(bytes.len() as u64 + 1, 0),
            &target,
            &AtomicBool::new(false),
            true
        )
        .is_err()
    );
    assert_eq!(fs::read(&target)?, b"existing destination");
    assert!(copy_atomic(&valid, &root.0, &AtomicBool::new(false), true).is_err());
    for entry in fs::read_dir(&root.0)? {
        assert!(!entry?.file_name().to_string_lossy().ends_with(".tmp"));
    }
    #[cfg(unix)]
    {
        let alias = root.0.join("alias.png");
        std::os::unix::fs::symlink(&target, &alias)?;
        assert!(copy_atomic(&valid, &alias, &AtomicBool::new(false), true).is_err());
        assert_eq!(fs::read(&target)?, b"existing destination");
    }
    Ok(())
}
#[test]
fn embedded_revision_must_match_and_missing_link_reports_error() -> R {
    let root = Root::new()?;
    let source = root.0.join("missing.png");
    let doc = document(SourceLocation::Linked(LinkedPath::native(&source)?))?;
    let path = root.0.join("board.tack");
    tack_storage::save(&path, &doc, vec![])?;
    let board = TackFile::open(&path)?;
    let request = OriginalExport::new(&doc, ObjectId::new(1)?, &path, &board, &Originals::new())?;
    assert!(
        request
            .payload()
            .err()
            .is_some_and(|e| e.to_string().contains("missing or unavailable"))
    );
    let mut changed = doc.clone();
    changed.apply(Command::SetSource(Source::from_descriptor(
        SourceId::new(1)?,
        SourceLocation::Embedded,
        2,
        None,
    )?))?;
    assert!(
        OriginalExport::new(
            &changed,
            ObjectId::new(1)?,
            &path,
            &board,
            &Originals::new()
        )
        .is_err()
    );
    Ok(())
}

#[test]
fn stream_cancellation_is_checked_between_bounded_chunks() -> R {
    struct Input<'a> {
        cancel: &'a AtomicBool,
        reads: usize,
        largest: usize,
    }
    impl Read for Input<'_> {
        fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
            self.reads += 1;
            self.largest = self.largest.max(buffer.len());
            if self.reads == 2 {
                self.cancel.store(true, Ordering::Relaxed);
            }
            buffer.fill(17);
            Ok(buffer.len())
        }
    }
    let stop = AtomicBool::new(false);
    let mut input = Input {
        cancel: &stop,
        reads: 0,
        largest: 0,
    };
    let mut output = std::io::sink();
    let error = copy_chunks(&mut input, &mut output, 16 * 1024 * 1024, &stop)
        .err()
        .ok_or("cancellation error")?;
    assert!(error.to_string().contains("cancelled"));
    assert_eq!(input.reads, 2);
    assert_eq!(input.largest, 128 * 1024);
    Ok(())
}
