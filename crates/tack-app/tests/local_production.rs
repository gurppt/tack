use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use tack_app::{
    actions::Action,
    image_save::{ImageSave, Originals, snapshot_inputs},
    local_import::{ImportRequest, ImportUpdate, admit},
    local_relink::RelinkReady,
    native_files,
    preferences::{self, Preferences},
};
use tack_core::*;
use tack_storage::*;
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
struct Root(PathBuf);
impl Root {
    fn new() -> Result<Self> {
        let p = std::env::temp_dir().join(format!(
            "tack-local-test-{:032x}",
            new_document_id()?.value()
        ));
        fs::create_dir(&p)?;
        Ok(Self(p))
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
const PNG: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 2, 0, 0, 0, 1, 8, 6, 0,
    0, 0, 244, 34, 127, 138, 0, 0, 0, 14, 73, 68, 65, 84, 120, 156, 99, 248, 207, 192, 240, 31,
    132, 1, 17, 247, 3, 253, 227, 197, 245, 239, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];
fn request(root: &Root, paths: Vec<PathBuf>, embedded: bool) -> ImportRequest {
    ImportRequest {
        mouse_easter_zoom: None,
        temporary: None,
        paths,
        embedded,
        position: [0.; 2],
        sampling: ImageFiltering::Nearest,
        work: root.0.join("work"),
        spool: None,
    }
}
fn editor() -> Result<DocumentEditor> {
    Ok(DocumentEditor::new(
        Document::new(new_document_id()?, DocumentLimits::default()),
        200,
    ))
}
#[test]
fn progressive_embed_dedup_shared_spool_cancel_invalid_and_exact_undo() -> Result {
    let r = Root::new()?;
    let p = r.0.join("one.png");
    fs::write(&p, PNG)?;
    let q = r.0.join("two.png");
    fs::write(&q, PNG)?;
    let bad = r.0.join("bad.png");
    fs::write(&bad, b"not an image")?;
    let mut e = editor()?;
    let empty = e.document().clone();
    let mut originals = Originals::new();
    let mut files = Vec::new();
    let mut failed = 0;
    let mut finished = false;
    request(&r, vec![p.clone(), p.clone(), bad, q.clone()], true).run(
        &AtomicBool::new(false),
        |u| {
            match u {
                ImportUpdate::Admitted(image) => {
                    assert!(admit(&mut e, &image).is_ok());
                    if let Some(payload) = image.original {
                        if let Payload::Stored { file, .. } = &payload {
                            files.push(Arc::clone(file));
                        }
                        originals.insert((image.source.id(), image.source.revision()), payload);
                    }
                }
                ImportUpdate::Failed { .. } => failed += 1,
                ImportUpdate::Finished {
                    admitted, bytes, ..
                } => {
                    assert_eq!(admitted, 3);
                    assert_eq!(bytes, 2 * PNG.len() as u64);
                    finished = true;
                }
                _ => {}
            }
            true
        },
    )?;
    assert!(finished);
    assert_eq!(failed, 1);
    assert_eq!(e.document().sources().count(), 2);
    assert_eq!(e.document().assets().count(), 2);
    assert_eq!(e.document().objects().count(), 3);
    assert!(Arc::ptr_eq(&files[0], &files[1]));
    let seed = r.0.join("seed.tack");
    save(&seed, &empty, vec![])?;
    let board = TackFile::open(&seed)?;
    fs::remove_file(&p)?;
    fs::remove_file(&q)?;
    let inputs = snapshot_inputs(&board, e.document(), &BTreeMap::new(), &originals)?;
    save(r.0.join("embedded.tack"), e.document(), inputs)?;
    let loaded = TackFile::open(r.0.join("embedded.tack"))?;
    loaded.verify_originals()?;
    for source in loaded.document.sources() {
        let mut bytes = Vec::new();
        loaded
            .original_reader(source.id())?
            .read_to_end(&mut bytes)?;
        assert_eq!(bytes, PNG);
    }
    for _ in 0..3 {
        assert!(e.undo()?);
    }
    assert_eq!(e.document(), &empty);
    fs::write(&p, PNG)?;
    let stop = AtomicBool::new(false);
    let mut admitted = 0;
    request(&r, vec![p.clone(); 100], false).run(&stop, |u| {
        if matches!(u, ImportUpdate::Admitted(_)) {
            admitted += 1;
            stop.store(true, Ordering::Relaxed);
        }
        true
    })?;
    assert_eq!(admitted, 1);
    Ok(())
}
#[test]
fn relink_missing_shared_and_foreign_preserves_layout_and_does_not_recycle_revision() -> Result {
    let r = Root::new()?;
    let p = r.0.join("replacement.png");
    fs::write(&p, PNG)?;
    let mut e = editor()?;
    let sid = SourceId::new(1)?;
    let foreign = if cfg!(windows) {
        LinkedPath::encoded(PathPlatform::Unix, true, b"/missing.png")?
    } else {
        let bytes: Vec<u8> = "C:\\missing.png"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect();
        LinkedPath::encoded(PathPlatform::Windows, true, &bytes)?
    };
    assert!(foreign.to_native().is_none());
    let previous = Source::from_descriptor(sid, SourceLocation::Linked(foreign), 1, None)?;
    e.execute(Command::AddSource(previous.clone()))?;
    for i in 1..=2 {
        e.execute(Command::AddAsset(ImageAsset::new(
            AssetId::new(i)?,
            sid,
            [100, 100],
        )?))?;
        e.execute(Command::AddObject {
            object: DocumentObject::image(
                ObjectId::new(i)?,
                AssetId::new(i)?,
                Transform::new([i as f64, 9.], [10., 20.], 0.7, [true, false])?,
            ),
            index: e.document().object_order().len(),
        })?;
    }
    let before = e.document().clone();
    RelinkReady::read(previous.clone(), &p)?.apply(&mut e)?;
    let first = e.document().source(sid).ok_or("source")?.revision();
    assert!(first > 1);
    for a in e.document().assets() {
        assert_eq!(a.pixel_size(), [2, 1]);
    }
    for id in before.object_order() {
        assert_eq!(e.document().object(*id), before.object(*id));
    }
    assert!(e.undo()?);
    assert_eq!(e.document(), &before);
    RelinkReady::read(previous, &p)?.apply(&mut e)?;
    assert!(e.document().source(sid).ok_or("source")?.revision() > first);
    assert!(
        RelinkReady::read(
            e.document().source(sid).ok_or("source")?.clone(),
            &r.0.join("missing.png")
        )
        .is_err()
    );
    Ok(())
}
#[test]
fn recovery_worker_does_not_mark_clean_and_edits_during_save_remain_dirty() -> Result {
    let r = Root::new()?;
    let path = r.0.join("board.tack");
    let mut e = editor()?;
    let sid = new_source_id()?;
    let aid = new_asset_id()?;
    let original = r.0.join("original.png");
    fs::write(&original, PNG)?;
    e.execute(Command::AddSource(Source::embedded(sid)))?;
    e.execute(Command::AddAsset(ImageAsset::new(aid, sid, [2, 1])?))?;
    e.execute(Command::AddObject {
        object: DocumentObject::image(
            new_object_id()?,
            aid,
            Transform::new([100.; 2], [20., 10.], 0., [false; 2])?,
        ),
        index: 0,
    })?;
    save(
        &path,
        e.document(),
        vec![BlobInput::original(sid, 1, Payload::File(original))],
    )?;
    let owner = Arc::new(BoardLease::acquire(&path)?);
    let board = Arc::new(owner.open()?);
    let mut supply =
        tack_assets::ProductAssets::new(Arc::clone(&board), &path, r.0.join("display-cache"))?;
    supply.replace_view(
        &[tack_assets::ProductDemand {
            asset: aid,
            lod: Lod::Medium,
            edge: 512,
            priority: 2,
            resident: false,
        }],
        e.document(),
        &BTreeMap::new(),
    );
    assert_eq!(supply.stats().pending, 1);
    let mut save = ImageSave::default();
    e.execute(Command::AddObject {
        object: DocumentObject::frame(
            ObjectId::new(1)?,
            "one".into(),
            Transform::new([0.; 2], [10.; 2], 0., [false; 2])?,
        )?,
        index: 0,
    })?;
    save.start_owned(
        Arc::clone(&owner),
        Arc::clone(&board),
        &e,
        &BTreeMap::new(),
        &Originals::new(),
        true,
    )?;
    save.finish(&mut e);
    assert!(save.last_error.is_none());
    assert!(e.is_dirty());
    assert_eq!(save.recoveries, 1);
    assert_eq!(owner.open()?.document, board.document);
    save.start_owned(
        Arc::clone(&owner),
        Arc::clone(&board),
        &e,
        &BTreeMap::new(),
        &Originals::new(),
        false,
    )?;
    e.execute(Command::SetFrameName {
        object: ObjectId::new(1)?,
        name: "edited during save".into(),
    })?;
    let start = Instant::now();
    while save.active() && start.elapsed() < Duration::from_secs(5) {
        save.poll(&mut e);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(!save.active());
    assert!(save.last_error.is_none());
    assert!(e.is_dirty());
    assert_ne!(owner.open()?.document, *e.document());
    assert!(owner.recovery(e.document().id())?.is_none());
    let generation = e.generation();
    let start = Instant::now();
    while supply.stats().pending > 0 {
        supply.poll();
        supply.schedule();
        assert!(start.elapsed() < Duration::from_secs(5));
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(supply.stats().errors, 0);
    assert_eq!(e.generation(), generation);
    owner.open()?.verify_originals()?;
    Ok(())
}
#[test]
fn bounded_keymaps_unknown_conflicts_foreign_recent_profile_merge_and_protection() -> Result {
    let r = Root::new()?;
    let mut p = Preferences::defaults()?;
    let defaults = p.keymap()?;
    assert!(
        Action::ALL
            .iter()
            .enumerate()
            .all(|(i, action)| !Action::ALL[..i].contains(action))
    );
    assert!(defaults.for_action(Action::NewBoard).next().is_some());
    let path = r.0.join("export.json");
    preferences::write(&path, &p)?;
    assert_eq!(
        preferences::read(&path)?.keymap()?.bindings(),
        defaults.bindings()
    );
    p.keymap.push(p.keymap[0].clone());
    assert!(p.keymap().is_err());
    p.keymap.pop();
    p.keymap[0].action = "unknown future action".into();
    assert!(p.keymap().is_err());
    p = Preferences::defaults()?;
    p.keymap.clear();
    assert!(p.keymap()?.bindings().is_empty());
    let mut json = serde_json::to_value(&p)?;
    json["recent"] = serde_json::json!([{ "encoding":2, "bytes":"C:\\foreign.tack".encode_utf16().flat_map(u16::to_le_bytes).collect::<Vec<_>>() }]);
    let foreign: Preferences = serde_json::from_value(json)?;
    assert!(foreign.keymap().is_ok());
    let base = preferences::save_profile(&r.0, &p, None, true, Some(&r.0.join("a.tack")))?;
    let mut edited = p.clone();
    edited.grid = true;
    let changed =
        preferences::save_profile(&r.0, &edited, Some(base), true, Some(&r.0.join("b.tack")))?;
    assert!(preferences::save_profile(&r.0, &p, Some(base), true, None).is_err());
    preferences::save_profile(&r.0, &p, Some(changed), false, Some(&r.0.join("c.tack")))?;
    let merged = preferences::read(&r.0.join("preferences.json"))?;
    assert!(merged.grid);
    assert_eq!(merged.recent.len(), 3);
    fs::write(&path, vec![b' '; 256 * 1024 + 1])?;
    assert!(preferences::read(&path).is_err());
    assert!(native_files::reference_paths("https://example.test/a.png").is_err());
    #[cfg(unix)]
    assert_eq!(
        native_files::reference_paths("file:///tmp/a%20b.png")?,
        vec![PathBuf::from("/tmp/a b.png")]
    );
    assert!(native_files::reference_paths("file:///tmp/a%00.png").is_err());
    assert!(native_files::reference_paths("file://remote/tmp/a.png").is_err());
    assert!(native_files::reference_paths("file:///tmp/a%ZZ.png").is_err());
    assert!(native_files::reference_paths(&"a".repeat(65537)).is_err());
    assert_eq!(
        native_files::reference_paths(&format!(
            "file://localhost/{}",
            r.0.join("ref.png")
                .display()
                .to_string()
                .replace('\\', "/")
                .trim_start_matches('/')
        ))?
        .len(),
        1
    );
    Ok(())
}

#[test]
fn save_as_after_relink_undo_keeps_session_revision_high_water() -> Result {
    let r = Root::new()?;
    let source = r.0.join("a.png");
    fs::write(&source, PNG)?;
    let mut e = editor()?;
    let sid = SourceId::new(1)?;
    e.execute(Command::AddSource(Source::linked(sid, "a.png")?))?;
    e.execute(Command::AddAsset(ImageAsset::new(
        AssetId::new(1)?,
        sid,
        [2, 1],
    )?))?;
    let previous = e.document().source(sid).ok_or("source")?.clone();
    let base = r.0.join("old.tack");
    save(&base, e.document(), vec![])?;
    let board = Arc::new(TackFile::open(&base)?);
    RelinkReady::read(previous, &source)?.apply(&mut e)?;
    let issued = e.document().source(sid).ok_or("source")?.revision();
    e.undo()?;
    fs::create_dir(r.0.join("other"))?;
    let target = r.0.join("other/new.tack");
    let mut job = ImageSave::default();
    job.start_as(
        target.clone(),
        base,
        board,
        &e,
        &BTreeMap::new(),
        &Originals::new(),
    )?;
    job.finish(&mut e);
    assert!(job.last_error.is_none());
    let moved = TackFile::open(target)?;
    assert!(moved.document.source(sid).ok_or("source")?.revision() > issued);
    assert!(
        matches!(moved.document.source(sid).ok_or("source")?.location(),SourceLocation::Linked(p) if p.is_absolute())
    );
    Ok(())
}

#[test]
fn loaded_embedded_relink_save_undo_keeps_original_authority() -> Result {
    let r = Root::new()?;
    let p = r.0.join("image.png");
    fs::write(&p, PNG)?;
    let sid = SourceId::new(1)?;
    let mut e = editor()?;
    e.execute(Command::AddSource(Source::embedded(sid)))?;
    let path = r.0.join("board.tack");
    save(
        &path,
        e.document(),
        vec![BlobInput::original(sid, 1, Payload::File(p.clone()))],
    )?;
    let mut board = TackFile::open(&path)?;
    e = DocumentEditor::new(board.document.clone(), 200);
    RelinkReady::read(e.document().source(sid).ok_or("source")?.clone(), &p)?.apply(&mut e)?;
    let mut originals = Originals::new();
    save(
        &path,
        e.document(),
        snapshot_inputs(&board, e.document(), &BTreeMap::new(), &originals)?,
    )?;
    let publication = TackFile::open(&path)?;
    assert!(publication.originals.is_empty());
    tack_app::image_save::retain_originals(&board, &publication, &e, &mut originals);
    board = publication;
    e.undo()?;
    fs::remove_file(p)?;
    save(
        &path,
        e.document(),
        snapshot_inputs(&board, e.document(), &BTreeMap::new(), &originals)?,
    )?;
    let reopened = TackFile::open(path)?;
    reopened.verify_originals()?;
    let mut bytes = Vec::new();
    reopened.original_reader(sid)?.read_to_end(&mut bytes)?;
    assert_eq!(bytes, PNG);
    Ok(())
}

#[test]
fn shared_original_spool_survives_interleaved_range_reads_and_session_quota() -> Result {
    let r = Root::new()?;
    let p = r.0.join("first.png");
    fs::write(&p, PNG)?;
    let mut spool = None;
    let mut payload = None;
    request(&r, vec![p], true).run(&AtomicBool::new(false), |u| {
        if let ImportUpdate::Admitted(image) = u {
            payload = image.original;
        }
        true
    })?;
    let original = payload.ok_or("original")?;
    if let Payload::Stored { file, .. } = &original {
        spool = Some(Arc::clone(file));
    }
    let mut second = PNG.to_vec();
    second.resize(16 * 1024 * 1024, 0);
    let q = r.0.join("second.png");
    fs::write(&q, second)?;
    let pinned = original.clone();
    let reader = std::thread::spawn(move || -> Result {
        for _ in 0..1000 {
            if let Payload::Stored { file, range } = &pinned {
                let mut bytes = Vec::new();
                RangeReader::new(Arc::clone(file), *range).read_to_end(&mut bytes)?;
                assert_eq!(bytes, PNG);
            }
        }
        Ok(())
    });
    let mut next = request(&r, vec![q], true);
    next.spool = spool.clone();
    let mut admitted = 0;
    next.run(&AtomicBool::new(false), |u| {
        if matches!(u, ImportUpdate::Admitted(_)) {
            admitted += 1;
        }
        true
    })?;
    reader.join().map_err(|_| "reader thread failed")??;
    assert_eq!(admitted, 1);
    let file = spool.ok_or("spool")?;
    file.set_len(tack_app::local_import::MAX_SESSION_IMPORT_BYTES)?;
    let tiny = r.0.join("third.png");
    fs::write(&tiny, PNG)?;
    let mut over = request(&r, vec![tiny], true);
    over.spool = Some(file);
    let mut refused = false;
    over.run(&AtomicBool::new(false), |u| {
        if matches!(u, ImportUpdate::Failed { .. }) {
            refused = true;
        }
        true
    })?;
    assert!(refused);
    Ok(())
}

#[test]
fn mulot_is_an_ordinary_embedded_image_one_undo_and_exact_reopen() -> Result {
    use tack_app::mouse_tool;
    let root = Root::new()?;
    let png = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../gfx/icons/poo_easter.png");
    let mut req = request(&root, vec![png.clone()], true);
    req.position = [48., 32.];
    req.mouse_easter_zoom = Some(1.);
    let mut e = editor()?;
    let empty = e.document().clone();
    let mut original = None;
    let mut image = None;
    req.run(&AtomicBool::new(false), |update| {
        if let ImportUpdate::Admitted(admitted) = update {
            image = Some(admitted);
        }
        true
    })?;
    let admitted = image.ok_or("expected admitted Mulot image")?;
    let id = admitted.object.id();
    assert!(matches!(admitted.object.kind(), ObjectKind::Image(_)));
    assert_eq!(admitted.object.transform().size(), [36.; 2]);
    assert_eq!(admitted.object.transform().center(), [48., 32.]);
    if let Some(payload) = &admitted.original {
        original = Some(payload.clone());
    }
    admit(&mut e, &admitted)?;
    assert_eq!(e.undo_len(), 1);
    assert_eq!(e.document().object_order().last(), Some(&id));
    assert_eq!(
        mouse_tool::hover_status(e.document(), id),
        Some("Puzzo puzzo !")
    );
    e.undo()?;
    assert_eq!(*e.document(), empty);
    e.redo()?;
    let seed = root.0.join("seed.tack");
    save(&seed, &empty, vec![])?;
    let board = TackFile::open(&seed)?;
    let originals = BTreeMap::from([(
        (admitted.source.id(), admitted.source.revision()),
        original.ok_or("embedded original")?,
    )]);
    save(
        root.0.join("mulot.tack"),
        e.document(),
        snapshot_inputs(&board, e.document(), &BTreeMap::new(), &originals)?,
    )?;
    let loaded = TackFile::open(root.0.join("mulot.tack"))?;
    assert_eq!(loaded.document, *e.document());
    assert_eq!(
        mouse_tool::hover_status(&loaded.document, id),
        Some("Puzzo puzzo !")
    );
    let mut bytes = Vec::new();
    loaded
        .original_reader(admitted.source.id())?
        .read_to_end(&mut bytes)?;
    assert_eq!(bytes, fs::read(png)?);
    // Repeated activation reuses the same normal asset and inserts one image.
    let reused = mouse_tool::easter_object(new_object_id()?, admitted.asset.id(), [96., 64.], 1.)?;
    let reused_id = reused.id();
    e.execute(Command::AddObject {
        object: reused,
        index: e.document().object_order().len(),
    })?;
    assert_eq!(e.document().sources().count(), 1);
    assert_eq!(e.document().assets().count(), 1);
    assert_eq!(e.document().objects().count(), 2);
    assert_eq!(
        mouse_tool::hover_status(e.document(), reused_id),
        Some("Puzzo puzzo !")
    );
    e.undo()?;
    assert_eq!(e.document().objects().count(), 1);
    Ok(())
}

#[test]
fn concurrent_first_mouse_imports_have_independent_normal_asset_and_source_ids() -> Result {
    let roots = [Root::new()?, Root::new()?];
    let png = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../gfx/icons/poo_easter.png");
    let mut admitted = Vec::new();
    for root in &roots {
        let mut req = request(root, vec![png.clone()], true);
        req.mouse_easter_zoom = Some(1.);
        req.run(&AtomicBool::new(false), |update| {
            if let ImportUpdate::Admitted(image) = update {
                admitted.push(image);
            }
            true
        })?;
    }
    assert_ne!(admitted[0].source.id(), admitted[1].source.id());
    assert_ne!(admitted[0].asset.id(), admitted[1].asset.id());
    // Authority can accept both existing import batches; no joke protocol operation.
    let mut authority = editor()?;
    for image in &admitted {
        admit(&mut authority, image)?;
    }
    assert_eq!(authority.document().objects().count(), 2);
    for image in &admitted {
        assert_eq!(
            tack_app::mouse_tool::hover_status(authority.document(), image.object.id()),
            Some("Puzzo puzzo !")
        );
    }
    Ok(())
}

/// Explicit fixture for the native Save As/reset test; never uses an artist board.
#[test]
#[ignore = "writes the explicitly supplied owned native lifecycle fixture"]
fn mouse_lifecycle_relative_fixture() -> Result {
    let root = PathBuf::from(
        std::env::var_os("TACK_MOUSE_FIXTURE_ROOT").ok_or("owned fixture root required")?,
    );
    fs::create_dir_all(&root)?;
    fs::write(root.join("fixture.png"), PNG)?;
    let mut e = editor()?;
    let source = new_source_id()?;
    let asset = new_asset_id()?;
    e.execute(Command::AddSource(Source::linked(source, "fixture.png")?))?;
    e.execute(Command::AddAsset(ImageAsset::new(asset, source, [2, 1])?))?;
    e.execute(Command::AddObject {
        object: DocumentObject::image(
            new_object_id()?,
            asset,
            Transform::new([0.; 2], [320., 200.], 0., [false; 2])?,
        ),
        index: 0,
    })?;
    save(root.join("relative.tack"), e.document(), vec![])?;
    Ok(())
}
