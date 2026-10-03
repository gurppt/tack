#![allow(clippy::unwrap_used, clippy::expect_used)]
use std::{
    fs,
    io::{Read, Seek, SeekFrom},
    path::PathBuf,
};
use tack_core::*;
use tack_storage::*;
fn root() -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "tack-storage-test-{:032x}",
        new_document_id().unwrap().value()
    ));
    fs::create_dir(&p).unwrap();
    p
}
fn fixture() -> Document {
    let mut d = Document::new(DocumentId::new(1).unwrap(), DocumentLimits::default());
    d.apply(Command::AddSource(Source::embedded(
        SourceId::new(2).unwrap(),
    )))
    .unwrap();
    d.apply(Command::AddAsset(
        ImageAsset::new(
            AssetId::new(3).unwrap(),
            SourceId::new(2).unwrap(),
            [6000, 4500],
        )
        .unwrap(),
    ))
    .unwrap();
    d.apply(Command::AddObject {
        object: DocumentObject::image(
            ObjectId::new(4).unwrap(),
            AssetId::new(3).unwrap(),
            Transform::new([10., 20.], [30., 40.], 0.7, [true, false]).unwrap(),
        ),
        index: 0,
    })
    .unwrap();
    d.apply(Command::SetCrop {
        object: ObjectId::new(4).unwrap(),
        crop: Crop::new(0.1, 0.2, 0.5, 0.7).unwrap(),
    })
    .unwrap();
    d.apply(Command::SetOpacity {
        object: ObjectId::new(4).unwrap(),
        opacity: Opacity::new(0.3).unwrap(),
    })
    .unwrap();
    d.apply(Command::SetImageFiltering {
        object: ObjectId::new(4).unwrap(),
        filtering: ImageFiltering::Nearest,
    })
    .unwrap();
    d
}
fn inputs(root: &std::path::Path) -> Vec<BlobInput> {
    fs::write(
        root.join("original"),
        b"original source independent of derived bytes",
    )
    .unwrap();
    fs::write(root.join("overview"), b"derived preview").unwrap();
    vec![
        BlobInput::original(
            SourceId::new(2).unwrap(),
            1,
            Payload::File(root.join("original")),
        ),
        BlobInput::overview(
            AssetId::new(3).unwrap(),
            1,
            [128, 96],
            1,
            Payload::File(root.join("overview")),
        ),
    ]
}
fn auth_crc(b: &mut [u8]) {
    let len = u64::from_le_bytes(b[16..24].try_into().unwrap()) as usize;
    let crc = crc32fast::hash(&b[80..80 + len]);
    b[40..44].copy_from_slice(&crc.to_le_bytes());
}
fn write_open(root: &std::path::Path, b: &[u8]) -> Result<TackFile> {
    fs::write(root.join("mutant.tack"), b).unwrap();
    TackFile::open(root.join("mutant.tack"))
}
#[test]
fn roundtrip_metadata_lazy_payload_and_generation_stability() {
    let p = root();
    let doc = fixture();
    save(p.join("a.tack"), &doc, inputs(&p)).unwrap();
    let loaded = TackFile::open(p.join("a.tack")).unwrap();
    assert_eq!(loaded.document, doc);
    assert_eq!(loaded.metadata_bytes_read, 433);
    assert_eq!(
        loaded.overview_bytes(AssetId::new(3).unwrap()).unwrap(),
        b"derived preview"
    );
    fs::remove_file(p.join("original")).unwrap();
    let mut r = loaded.original_reader(SourceId::new(2).unwrap()).unwrap();
    let mut b = Vec::new();
    r.read_to_end(&mut b).unwrap();
    assert_eq!(b, b"original source independent of derived bytes");
    let mut a = loaded.original_reader(SourceId::new(2).unwrap()).unwrap();
    let mut b = loaded.original_reader(SourceId::new(2).unwrap()).unwrap();
    a.seek(SeekFrom::Start(9)).unwrap();
    let mut first = [0; 8];
    b.read_exact(&mut first).unwrap();
    assert_eq!(&first, b"original");
    a.read_exact(&mut first).unwrap();
    assert_eq!(&first, b"source i");
    // Open generation remains stable even after target replacement.
    save(
        p.join("a.tack"),
        &doc,
        vec![BlobInput::original(
            SourceId::new(2).unwrap(),
            1,
            loaded.payload(loaded.originals[&SourceId::new(2).unwrap()].range),
        )],
    )
    .unwrap();
    assert_eq!(
        loaded.overview_bytes(AssetId::new(3).unwrap()).unwrap(),
        b"derived preview"
    );
    assert!(
        TackFile::open(p.join("a.tack"))
            .unwrap()
            .overviews
            .is_empty()
    );
    fs::remove_dir_all(p).unwrap();
}
#[test]
fn reader_rejects_structure_and_authoritative_corruption() {
    let p = root();
    save(p.join("a.tack"), &fixture(), inputs(&p)).unwrap();
    let good = fs::read(p.join("a.tack")).unwrap();
    for len in [0, 7, 79, 100, good.len() - 1] {
        assert!(write_open(&p, &good[..len]).is_err());
    }
    for (offset, data) in [
        (0, vec![0]),
        (8, 2u32.to_le_bytes().to_vec()),
        (12, 2u32.to_le_bytes().to_vec()),
        (16, u64::MAX.to_le_bytes().to_vec()),
        (48, u32::MAX.to_le_bytes().to_vec()),
        (52, u32::MAX.to_le_bytes().to_vec()),
        (56, u32::MAX.to_le_bytes().to_vec()),
        (68, vec![1]),
    ] {
        let mut b = good.clone();
        b[offset..offset + data.len()].copy_from_slice(&data);
        assert!(write_open(&p, &b).is_err(), "offset {offset}");
    }
    // Auth layout: document16, source length4+record28, asset42, object119, order16, original64.
    for (offset, data) in [
        (96, u32::MAX.to_le_bytes().to_vec()),
        (100, 2u16.to_le_bytes().to_vec()),
        (102, 0u128.to_le_bytes().to_vec()),
        (146, 999u128.to_le_bytes().to_vec()),
        (170, 2u16.to_le_bytes().to_vec()),
        (172, 2u16.to_le_bytes().to_vec()),
        (190, 999u128.to_le_bytes().to_vec()),
        (206, f64::NAN.to_le_bytes().to_vec()),
        (248, 1.1f64.to_le_bytes().to_vec()),
        (280, 2f64.to_le_bytes().to_vec()),
        (288, vec![99]),
        (289, 999u128.to_le_bytes().to_vec()),
        (345, u64::MAX.to_le_bytes().to_vec()),
    ] {
        let mut b = good.clone();
        b[offset..offset + data.len()].copy_from_slice(&data);
        auth_crc(&mut b);
        assert!(write_open(&p, &b).is_err(), "offset {offset}");
    }
    // Bounded malformed mutations must not panic; checksums recomputed to exercise parser.
    for i in 0..1000 {
        let mut b = good.clone();
        let offset = 80 + (i * 37 % (good.len() - 80));
        b[offset] ^= (i as u8).wrapping_add(1);
        auth_crc(&mut b);
        let _ = write_open(&p, &b);
    }
    fs::remove_dir_all(p).unwrap();
}
#[test]
fn derived_corruption_is_disposable_and_original_copy_checks_integrity() {
    let p = root();
    save(p.join("a.tack"), &fixture(), inputs(&p)).unwrap();
    let loaded = TackFile::open(p.join("a.tack")).unwrap();
    let mut b = fs::read(p.join("a.tack")).unwrap();
    let e = loaded.overviews[&AssetId::new(3).unwrap()];
    b[e.range.offset as usize] ^= 1;
    let broken = write_open(&p, &b).unwrap();
    assert_eq!(broken.document, loaded.document);
    assert!(broken.overview_bytes(AssetId::new(3).unwrap()).is_err());
    let mut b = fs::read(p.join("a.tack")).unwrap();
    b[305] ^= 1;
    auth_crc(&mut b);
    assert!(write_open(&p, &b).is_err());
    let mut b = fs::read(p.join("a.tack")).unwrap();
    b[433] ^= 1;
    let broken = write_open(&p, &b).unwrap();
    assert!(
        save(
            p.join("copy.tack"),
            &broken.document,
            vec![BlobInput::original(
                SourceId::new(2).unwrap(),
                1,
                broken.payload(broken.originals[&SourceId::new(2).unwrap()].range)
            )]
        )
        .is_err()
    );
    assert!(!p.join("copy.tack").exists());
    // Broken derived index CRC discards directory without damaging authority.
    let mut b = fs::read(p.join("a.tack")).unwrap();
    b[384] ^= 1;
    let broken = write_open(&p, &b).unwrap();
    assert_eq!(broken.document, loaded.document);
    assert_eq!(broken.discarded_overviews, 1);
    assert!(broken.overviews.is_empty());
    fs::remove_dir_all(p).unwrap();
}
#[test]
fn failed_replacements_preserve_target_and_clean_only_owned_temps() {
    let p = root();
    let doc = fixture();
    save(p.join("a.tack"), &doc, inputs(&p)).unwrap();
    let old = fs::read(p.join("a.tack")).unwrap();
    fs::write(p.join("other.tack-tmp-abandoned"), b"owned elsewhere").unwrap();
    for stage in [
        SaveStage::TemporaryCreated,
        SaveStage::PayloadCopied(0),
        SaveStage::MetadataWritten,
        SaveStage::FileSynced,
    ] {
        assert!(
            save_with_hook(p.join("a.tack"), &doc, inputs(&p), |s| if s == stage {
                Err(StorageError::Io(std::io::Error::other("injected failure")))
            } else {
                Ok(())
            })
            .is_err()
        );
        assert_eq!(fs::read(p.join("a.tack")).unwrap(), old);
        assert_eq!(
            fs::read_dir(&p)
                .unwrap()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_name().to_string_lossy().contains(".tack-tmp-"))
                .count(),
            1
        );
    }
    assert!(save(p.join("nonexistent/a.tack"), &doc, inputs(&p)).is_err());
    assert_eq!(fs::read(p.join("a.tack")).unwrap(), old);
    fs::remove_dir_all(p).unwrap();
}
#[test]
fn lossless_paths_revisions_dirty_undo_and_random_ids() {
    let p = root();
    let mut d = fixture();
    let foreign = if cfg!(unix) {
        LinkedPath::encoded(
            PathPlatform::Windows,
            true,
            &[b'C', 0, b':', 0, b'\\', 0, 0x00, 0xd8],
        )
        .unwrap()
    } else {
        LinkedPath::encoded(PathPlatform::Unix, true, b"/foreign/\xff").unwrap()
    };
    let s = Source::from_descriptor(
        SourceId::new(2).unwrap(),
        SourceLocation::Linked(foreign),
        7,
        Some(SourceFingerprint {
            size: 8,
            modified_seconds: 5,
            modified_nanos: 1,
        }),
    )
    .unwrap();
    let mut editor = DocumentEditor::new(d.clone(), 2);
    assert!(!editor.is_dirty());
    assert!(editor.execute(Command::SetSource(s.clone())).unwrap());
    assert!(editor.is_dirty());
    editor.mark_saved();
    assert!(!editor.is_dirty());
    editor.undo().unwrap();
    assert!(editor.is_dirty());
    assert_eq!(editor.document(), &d);
    editor.redo().unwrap();
    assert_eq!(editor.document().source(s.id()), Some(&s));
    d = editor.into_document();
    save(p.join("foreign.tack"), &d, vec![]).unwrap();
    let loaded = TackFile::open(p.join("foreign.tack")).unwrap();
    assert_eq!(loaded.document, d);
    let SourceLocation::Linked(path) = loaded.document.source(s.id()).unwrap().location() else {
        unreachable!()
    };
    assert!(path.to_native().is_none());
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        let path = PathBuf::from(std::ffi::OsString::from_vec(b"relative/\xff\xe9".to_vec()));
        let source = Source::from_descriptor(
            s.id(),
            SourceLocation::Linked(LinkedPath::native(&path).unwrap()),
            8,
            None,
        )
        .unwrap();
        d.apply(Command::SetSource(source)).unwrap();
        save(p.join("native.tack"), &d, vec![]).unwrap();
        assert_eq!(TackFile::open(p.join("native.tack")).unwrap().document, d);
    }
    let ids: std::collections::BTreeSet<_> = (0..1000).map(|_| new_object_id().unwrap()).collect();
    assert_eq!(ids.len(), 1000);
    assert_ne!(new_document_id().unwrap().value(), 0);
    assert_ne!(new_asset_id().unwrap().value(), 0);
    assert_ne!(new_source_id().unwrap().value(), 0);
    fs::remove_dir_all(p).unwrap();
}

// Unix sparse-file semantics avoid allocating 20 GiB on Windows CI disks.
#[cfg(unix)]
#[test]
fn metadata_only_open_of_sparse_twenty_gib_original() {
    let p = root();
    save(p.join("a.tack"), &fixture(), inputs(&p)).unwrap();
    let mut bytes = fs::read(p.join("a.tack")).unwrap();
    let length = 20u64 * 1024 * 1024 * 1024;
    bytes[32..40].copy_from_slice(&length.to_le_bytes());
    bytes[357..365].copy_from_slice(&(length - 433).to_le_bytes());
    auth_crc(&mut bytes);
    fs::write(p.join("a.tack"), bytes).unwrap();
    fs::OpenOptions::new()
        .write(true)
        .open(p.join("a.tack"))
        .unwrap()
        .set_len(length)
        .unwrap();
    let b = TackFile::open(p.join("a.tack")).unwrap();
    assert_eq!(b.metadata_bytes_read, 433);
    assert_eq!(b.document, fixture());
    assert!(b.overviews.is_empty());
    let mut r = b.original_reader(SourceId::new(2).unwrap()).unwrap();
    r.seek(SeekFrom::End(-8)).unwrap();
    let mut out = [1; 8];
    r.read_exact(&mut out).unwrap();
    assert_eq!(out, [0; 8]);
    assert_eq!(r.bytes_read, 8);
    fs::remove_dir_all(p).unwrap();
}
#[test]
fn source_revision_is_not_recycled_after_undo_divergence() {
    let mut editor = DocumentEditor::new(fixture(), 3);
    let id = SourceId::new(2).unwrap();
    let replacement = Source::from_descriptor(
        id,
        SourceLocation::Linked(LinkedPath::native(std::path::Path::new("new.jpg")).unwrap()),
        2,
        None,
    )
    .unwrap();
    assert!(
        editor
            .execute(Command::SetSource(replacement.clone()))
            .unwrap()
    );
    editor.undo().unwrap();
    assert_eq!(editor.next_source_revision(), Some(3));
    let before = editor.document().clone();
    assert!(editor.execute(Command::SetSource(replacement)).is_err());
    assert_eq!(editor.document(), &before);
    assert_eq!(editor.redo_len(), 1);
    let fresh = Source::from_descriptor(
        id,
        SourceLocation::Linked(LinkedPath::native(std::path::Path::new("other.jpg")).unwrap()),
        3,
        None,
    )
    .unwrap();
    editor.execute(Command::SetSource(fresh)).unwrap();
    assert_eq!(editor.redo_len(), 0);
}
#[test]
#[cfg(unix)]
fn process_kill_before_publication_keeps_previous_generation() {
    let p = root();
    save(p.join("a.tack"), &fixture(), inputs(&p)).unwrap();
    let old = fs::read(p.join("a.tack")).unwrap();
    for stage in ["temporary", "payload", "metadata", "synced"] {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "crash_child", "--nocapture"])
            .env("TACK_CRASH_TEST_ROOT", &p)
            .env("TACK_CRASH_TEST_STAGE", stage)
            .status()
            .unwrap();
        assert!(!status.success());
        assert_eq!(fs::read(p.join("a.tack")).unwrap(), old);
        assert_eq!(
            TackFile::open(p.join("a.tack")).unwrap().document,
            fixture()
        );
    }
    assert_eq!(
        fs::read_dir(&p)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains(".tack-tmp-"))
            .count(),
        4
    ); // Abandoned temps never auto-promoted/deleted.
    fs::remove_dir_all(p).unwrap();
}
#[test]
#[cfg(unix)]
fn crash_child() {
    let Some(root) = std::env::var_os("TACK_CRASH_TEST_ROOT") else {
        return;
    };
    let root = PathBuf::from(root);
    let wanted = std::env::var("TACK_CRASH_TEST_STAGE").unwrap();
    save_with_hook(root.join("a.tack"), &fixture(), inputs(&root), |stage| {
        let matching = matches!(
            (wanted.as_str(), stage),
            ("temporary", SaveStage::TemporaryCreated)
                | ("payload", SaveStage::PayloadCopied(0))
                | ("metadata", SaveStage::MetadataWritten)
                | ("synced", SaveStage::FileSynced)
        );
        if matching {
            let _ = std::process::Command::new("kill")
                .args(["-KILL", &std::process::id().to_string()])
                .status();
        }
        Ok(())
    })
    .unwrap();
}

#[cfg(unix)]
#[test]
fn save_creates_private_temporary_and_preserves_private_target_mode() {
    use std::os::unix::fs::PermissionsExt;
    let r = root();
    let target = r.join("private.tack");
    save_with_hook(&target, &fixture(), inputs(&r), |stage| {
        if stage == SaveStage::TemporaryCreated {
            let entry = fs::read_dir(&r)
                .unwrap()
                .find(|e| {
                    e.as_ref()
                        .unwrap()
                        .file_name()
                        .to_string_lossy()
                        .contains(".tack-tmp-")
                })
                .unwrap()
                .unwrap();
            assert_eq!(
                entry.metadata().unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        Ok(())
    })
    .unwrap();
    assert_eq!(
        fs::metadata(&target).unwrap().permissions().mode() & 0o777,
        0o600
    );
    fs::set_permissions(&target, fs::Permissions::from_mode(0o600)).unwrap();
    save(&target, &fixture(), inputs(&r)).unwrap();
    assert_eq!(
        fs::metadata(&target).unwrap().permissions().mode() & 0o777,
        0o600
    );
    fs::remove_dir_all(r).unwrap();
}
