#![allow(clippy::unwrap_used)]
use super::*;
use tack_core::{AssetId, DocumentId, DocumentLimits, ImageAsset};

struct Temporary(PathBuf);
impl Temporary {
    fn new() -> Self {
        Self(std::env::temp_dir().join(format!(
            "tack-shared-cache-test-{:032x}",
            tack_storage::new_document_id().unwrap().value()
        )))
    }
}
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn missing_binding_remains_a_usable_metadata_snapshot() {
    let root = Temporary::new();
    let mut document = Document::new(DocumentId::new(1).unwrap(), DocumentLimits::default());
    let source = SourceId::new(2).unwrap();
    document
        .apply(Command::AddSource(Source::embedded(source)))
        .unwrap();
    document
        .apply(Command::AddAsset(
            ImageAsset::new(AssetId::new(3).unwrap(), source, [6000, 3500]).unwrap(),
        ))
        .unwrap();
    let (_, board) = snapshot_view(&root.0, &document, &[]).unwrap();
    assert_eq!(board.document.id(), document.id());
    assert_eq!(
        board.document.asset(AssetId::new(3).unwrap()),
        document.asset(AssetId::new(3).unwrap())
    );
    let SourceLocation::Linked(path) = board.document.source(source).unwrap().location() else {
        unreachable!()
    };
    assert_eq!(
        path.to_native().unwrap(),
        PathBuf::from(".missing-source-00000000000000000000000000000002")
    );
    assert!(matches!(
        document.source(source).unwrap().location(),
        SourceLocation::Embedded
    ));
}

#[test]
fn snapshots_and_storage_sidecars_allow_verified_original_admission() {
    let root = Temporary::new();
    let mut document = Document::new(DocumentId::new(1).unwrap(), DocumentLimits::default());
    let source = SourceId::new(2).unwrap();
    document
        .apply(Command::AddSource(Source::embedded(source)))
        .unwrap();
    let bytes = b"source downloaded after native snapshot";
    let binding = SourceBinding {
        source: WireId::new(source.value()).unwrap(),
        revision: 1,
        hash: ContentHash::digest(bytes),
        size: bytes.len() as u64,
    };
    let (path, board) = snapshot_view(&root.0, &document, std::slice::from_ref(&binding)).unwrap();
    let lock_path = path.with_extension("tack.tack-lock");
    assert!(lock_path.is_file());
    let mut cache = OriginalCache::open(root.0.clone()).unwrap();
    let (temporary, mut file) = cache.begin(binding.size).unwrap();
    file.write_all(bytes).unwrap();
    drop(file);
    let original = cache
        .finish(&temporary, &binding.hash, binding.size)
        .unwrap();
    let SourceLocation::Linked(linked) = board.document.source(source).unwrap().location() else {
        unreachable!()
    };
    assert_eq!(
        path.parent().unwrap().join(linked.to_native().unwrap()),
        original
    );
    let mut reopened = OriginalCache::open(root.0.clone()).unwrap();
    assert_eq!(
        reopened
            .available(&binding.hash, binding.size, &AtomicBool::new(false))
            .unwrap(),
        Some(original.clone())
    );
    drop(board);
    for _ in 0..8 {
        let (_, board) = snapshot_view(&root.0, &document, std::slice::from_ref(&binding)).unwrap();
        let names: Vec<_> = fs::read_dir(&root.0)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        assert_eq!(names.iter().filter(|name| snapshot_name(name)).count(), 1);
        assert_eq!(
            names.iter().filter(|name| snapshot_lock_name(name)).count(),
            1
        );
        assert_eq!(
            OriginalCache::open(root.0.clone()).unwrap().bytes(),
            binding.size
        );
        assert!(original.exists());
        drop(board);
    }
}

#[test]
fn snapshot_retirement_respects_active_leases_and_rejects_nonempty_sidecars() {
    let root = Temporary::new();
    let document = Document::new(DocumentId::new(1).unwrap(), DocumentLimits::default());
    let (first, first_board) = snapshot_view(&root.0, &document, &[]).unwrap();
    let first_lock = first.with_extension("tack.tack-lock");
    let lease = tack_storage::lock_sidecar(&first_lock).unwrap();
    let (_, second_board) = snapshot_view(&root.0, &document, &[]).unwrap();
    assert!(first.exists());
    assert!(first_lock.exists());
    assert!(OriginalCache::open(root.0.clone()).is_ok());
    drop(lease);
    drop(first_board);
    drop(second_board);
    let (last, last_board) = snapshot_view(&root.0, &document, &[]).unwrap();
    assert!(!first.exists());
    assert!(!first_lock.exists());
    let last_lock = last.with_extension("tack.tack-lock");
    fs::write(&last_lock, b"not an owned empty storage lock").unwrap();
    assert!(OriginalCache::open(root.0.clone()).is_err());
    assert!(snapshot_view(&root.0, &document, &[]).is_err());
    assert_eq!(
        fs::read(&last_lock).unwrap(),
        b"not an owned empty storage lock"
    );
    drop(last_board);
}

#[test]
fn verifies_hash_rejects_corruption_and_accounts_missing_entries() {
    let root = Temporary::new();
    let mut cache = OriginalCache::open(root.0.clone()).unwrap();
    let bytes = b"encoded original";
    let hash = ContentHash::digest(bytes);
    let cancel = AtomicBool::new(false);
    let (temporary, mut file) = cache.begin(bytes.len() as u64).unwrap();
    file.write_all(bytes).unwrap();
    file.sync_all().unwrap();
    drop(file);
    let path = cache.finish(&temporary, &hash, bytes.len() as u64).unwrap();
    assert_eq!(cache.bytes(), bytes.len() as u64);
    assert_eq!(
        cache.available(&hash, bytes.len() as u64, &cancel).unwrap(),
        Some(path.clone())
    );
    fs::remove_file(&path).unwrap();
    assert!(
        cache
            .available(&hash, bytes.len() as u64, &cancel)
            .unwrap()
            .is_none()
    );
    assert_eq!(cache.bytes(), 0);
    fs::write(&path, b"corrupt bytes").unwrap();
    assert!(
        cache
            .available(&hash, bytes.len() as u64, &cancel)
            .unwrap()
            .is_none()
    );
    assert!(!path.exists());
    assert_eq!(cache.take_evicted(), vec![hash.clone(), hash]);
}

#[test]
fn startup_removes_owned_partial_and_evicts_oversized_sparse_original() {
    let root = Temporary::new();
    owned_directory(&root.0).unwrap();
    let partial = root.0.join(".asset-00000000000000000000000000000001.part");
    fs::write(&partial, b"aborted download").unwrap();
    let hash = ContentHash::digest(b"sparse admission probe");
    let original = root.0.join(hash.as_str());
    File::create(&original)
        .unwrap()
        .set_len(CACHE_BYTES + 1)
        .unwrap();
    let mut cache = OriginalCache::open(root.0.clone()).unwrap();
    assert!(!partial.exists());
    assert!(!original.exists());
    assert_eq!(cache.bytes(), 0);
    assert_eq!(cache.take_evicted(), vec![hash]);
    assert!(cache.begin(CACHE_BYTES + 1).is_err());
}

#[test]
fn unowned_or_symlink_entries_are_never_deleted() {
    let root = Temporary::new();
    owned_directory(&root.0).unwrap();
    let owner = root.0.join("owner-file");
    fs::write(&owner, b"keep").unwrap();
    assert!(OriginalCache::open(root.0.clone()).is_err());
    assert_eq!(fs::read(&owner).unwrap(), b"keep");
    #[cfg(unix)]
    {
        fs::remove_file(&owner).unwrap();
        let target = ContentHash::digest(b"alias");
        std::os::unix::fs::symlink(root.0.join(SENTINEL), root.0.join(target.as_str())).unwrap();
        assert!(OriginalCache::open(root.0.clone()).is_err());
        assert!(root.0.join(SENTINEL).exists());
    }
}
