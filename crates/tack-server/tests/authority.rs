use std::{fs, path::PathBuf};
use tack_core::*;
use tack_server::{AssetStore, Authority};
use tack_shared::{CommandDto, ContentHash, SourceBinding, WireId};
type R = Result<(), Box<dyn std::error::Error>>;
struct Work(PathBuf);
impl Work {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let path = std::env::temp_dir().join(format!(
            "tack-server-test-{}",
            tack_storage::new_document_id()?.value()
        ));
        fs::create_dir(&path)?;
        Ok(Self(path))
    }
}
impl Drop for Work {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn fixture() -> Result<Document, Box<dyn std::error::Error>> {
    let mut document = Document::new(DocumentId::new(10)?, DocumentLimits::default());
    document.apply(Command::AddSource(Source::embedded(SourceId::new(1)?)))?;
    document.apply(Command::AddAsset(ImageAsset::new(
        AssetId::new(2)?,
        SourceId::new(1)?,
        [10, 20],
    )?))?;
    document.apply(Command::AddObject {
        object: DocumentObject::image(
            ObjectId::new(3)?,
            AssetId::new(2)?,
            Transform::new([10., 20.], [30., 40.], 0., [false, false])?,
        ),
        index: 0,
    })?;
    Ok(document)
}
fn edit(x: f64) -> Result<CommandDto, Box<dyn std::error::Error>> {
    Ok(CommandDto::from_command(&Command::SetTransform {
        object: ObjectId::new(3)?,
        transform: Transform::new([x, 20.], [30., 40.], 0., [false, false])?,
    })?)
}
#[test]
fn authority_dedupe_stale_conflict_persisted_undo() -> R {
    let work = Work::new()?;
    let mut authority = Authority::publish(&work.0, fixture()?, vec![])?;
    let a = WireId::new(20)?;
    let b = WireId::new(21)?;
    let first = WireId::new(30)?;
    assert_eq!(authority.edit(a, first, 0, edit(11.)?, vec![])?.revision, 1);
    assert_eq!(authority.edit(a, first, 0, edit(11.)?, vec![])?.revision, 1);
    assert!(
        authority
            .edit(b, WireId::new(31)?, 0, edit(12.)?, vec![])
            .is_err()
    );
    authority.edit(b, WireId::new(32)?, 1, edit(12.)?, vec![])?;
    assert!(authority.history(a, WireId::new(33)?, 2, false).is_err());
    authority.history(b, WireId::new(34)?, 2, false)?;
    assert_eq!(
        authority
            .document()
            .object(ObjectId::new(3)?)
            .ok_or("object")?
            .transform()
            .center(),
        [11., 20.]
    );
    let mut restored = Authority::open(&work.0, authority.board())?;
    assert_eq!(restored.revision(), 3);
    assert_eq!(restored.edit(a, first, 0, edit(11.)?, vec![])?.revision, 1);
    restored.history(b, WireId::new(35)?, 3, true)?;
    assert_eq!(restored.revision(), 4);
    assert!(Authority::publish(&work.0, fixture()?, vec![]).is_err());
    Ok(())
}
#[test]
fn chained_own_undo_and_source_binding_restore() -> R {
    let work = Work::new()?;
    let source = WireId::new(1)?;
    let old = SourceBinding {
        source,
        revision: 1,
        hash: ContentHash::digest(b"old"),
        size: 3,
    };
    let mut authority = Authority::publish(&work.0, fixture()?, vec![old.clone()])?;
    let client = WireId::new(20)?;
    authority.edit(client, WireId::new(31)?, 0, edit(11.)?, vec![])?;
    authority.edit(client, WireId::new(32)?, 1, edit(12.)?, vec![])?;
    authority.history(client, WireId::new(33)?, 2, false)?;
    authority.history(client, WireId::new(34)?, 3, false)?;
    assert_eq!(
        authority
            .document()
            .object(ObjectId::new(3)?)
            .ok_or("object")?
            .transform()
            .center(),
        [10., 20.]
    );
    let replacement =
        Source::from_descriptor(SourceId::new(1)?, SourceLocation::Embedded, 2, None)?;
    let command = CommandDto::from_command(&Command::SetSource(replacement.clone()))?;
    let new = SourceBinding {
        source,
        revision: 2,
        hash: ContentHash::digest(b"new"),
        size: 3,
    };
    authority.edit(client, WireId::new(35)?, 4, command.clone(), vec![new])?;
    authority.history(client, WireId::new(36)?, 5, false)?;
    assert_eq!(authority.sources(), &[old]);
    assert!(
        authority
            .edit(client, WireId::new(37)?, 6, command, vec![])
            .is_err()
    );
    Ok(())
}
#[test]
fn corrupt_snapshot_refused_and_interrupted_pending_not_authority() -> R {
    let work = Work::new()?;
    let authority = Authority::publish(&work.0, fixture()?, vec![])?;
    let path = work.0.join(format!("{}.board", authority.board()));
    fs::write(path.with_extension("pending"), b"truncated publication")?;
    assert_eq!(Authority::open(&work.0, authority.board())?.revision(), 0);
    let mut bytes = fs::read(&path)?;
    let last = bytes.last_mut().ok_or("empty")?;
    *last ^= 1;
    fs::write(&path, bytes)?;
    assert!(Authority::open(&work.0, authority.board()).is_err());
    Ok(())
}
#[test]
fn cas_streaming_dedupe_offsets_quota_hash_cancel_and_ranges() -> R {
    let work = Work::new()?;
    let assets = AssetStore::new(&work.0.join("assets"), 100_000)?;
    let original = vec![42; 90_000];
    let hash = ContentHash::digest(&original);
    let mut upload = assets.begin(hash.clone(), 90_000)?.ok_or("upload")?;
    assert!(upload.append(1, &original[..65_536]).is_err());
    assert_eq!(upload.append(0, &original[..65_536])?, 65_536);
    assert!(
        assets
            .begin(ContentHash::digest(b"too much"), 20_000)
            .is_err()
    );
    upload.append(65_536, &original[65_536..])?;
    assert_eq!(upload.commit()?, 90_000);
    assert!(assets.begin(hash.clone(), 90_000)?.is_none());
    assert_eq!(assets.stored_bytes()?, 90_000);
    let (size, bytes) = assets.range(hash.clone(), 65_536, 65_536)?;
    assert_eq!(size, 90_000);
    assert_eq!(bytes, original[65_536..]);
    assert!(assets.range(hash.clone(), 90_001, 1).is_err());
    assert!(assets.range(hash, 0, 0).is_err());
    let wrong = ContentHash::digest(b"right");
    let mut bad = assets.begin(wrong.clone(), 5)?.ok_or("upload")?;
    bad.append(0, b"wrong")?;
    assert!(bad.commit().is_err());
    assert!(assets.length(wrong.clone())?.is_none());
    drop(assets.begin(wrong.clone(), 5)?);
    assert!(assets.begin(wrong, 5)?.is_some());
    assert_eq!(assets.stored_bytes()?, 90_000);
    Ok(())
}

#[test]
fn cas_startup_only_removes_recognized_regular_uploads() -> R {
    let work = Work::new()?;
    let root = work.0.join("assets");
    fs::create_dir(&root)?;
    let hash = ContentHash::digest(b"aborted original");
    let recognized = root.join(format!("{hash}.upload"));
    fs::write(&recognized, b"partial")?;
    AssetStore::new(&root, 100)?;
    assert!(!recognized.exists());

    let unknown = root.join("owner-notes.upload");
    fs::write(&unknown, b"preserve")?;
    assert!(AssetStore::new(&root, 100).is_err());
    assert_eq!(fs::read(&unknown)?, b"preserve");
    fs::remove_file(unknown)?;

    fs::create_dir(&recognized)?;
    assert!(AssetStore::new(&root, 100).is_err());
    assert!(recognized.is_dir());
    fs::remove_dir(&recognized)?;

    #[cfg(unix)]
    {
        let target = work.0.join("preserved-outside-cas");
        fs::write(&target, b"keep target")?;
        std::os::unix::fs::symlink(&target, &recognized)?;
        assert!(AssetStore::new(&root, 100).is_err());
        assert!(fs::symlink_metadata(&recognized)?.file_type().is_symlink());
        assert_eq!(fs::read(target)?, b"keep target");
    }
    Ok(())
}

#[test]
fn history_client_eviction_never_permanently_bricks_board() -> R {
    let work = Work::new()?;
    let mut authority = Authority::publish(&work.0, fixture()?, vec![])?;
    let oldest = WireId::new(100)?;
    let operation = WireId::new(200)?;
    for i in 0..20 {
        authority.edit(
            WireId::new(100 + i)?,
            WireId::new(200 + i)?,
            i as u64,
            edit(100. + i as f64)?,
            vec![],
        )?;
    }
    assert_eq!(authority.revision(), 20);
    assert!(authority.receipt(oldest, operation).is_none());
    assert!(
        authority
            .edit(oldest, operation, 0, edit(100.)?, vec![])
            .is_err()
    );
    let mut reopened = Authority::open(&work.0, authority.board())?;
    reopened.edit(oldest, WireId::new(300)?, 20, edit(500.)?, vec![])?;
    assert_eq!(reopened.revision(), 21);
    Ok(())
}

#[test]
fn binding_cannot_change_without_source_replacement() -> R {
    let work = Work::new()?;
    let old = SourceBinding {
        source: WireId::new(1)?,
        revision: 1,
        hash: ContentHash::digest(b"old"),
        size: 3,
    };
    let mut authority = Authority::publish(&work.0, fixture()?, vec![old.clone()])?;
    let forged = SourceBinding {
        hash: ContentHash::digest(b"different"),
        size: 9,
        ..old.clone()
    };
    assert!(
        authority
            .edit(
                WireId::new(20)?,
                WireId::new(30)?,
                0,
                edit(300.)?,
                vec![forged]
            )
            .is_err()
    );
    assert_eq!(authority.revision(), 0);
    assert_eq!(authority.sources(), &[old]);
    Ok(())
}
