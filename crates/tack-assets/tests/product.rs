#![allow(clippy::unwrap_used)]
use std::{
    fs,
    io::{Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};
use tack_assets::{ProductAssets, SourceState, image_metadata, source_fingerprint};
use tack_core::*;
use tack_storage::*;
struct Fixture {
    root: PathBuf,
    asset: AssetId,
    source: SourceId,
    document: Document,
}
impl Fixture {
    fn new(embedded: bool) -> Self {
        Self::sized(embedded, [96, 64])
    }
    fn sized(embedded: bool, size: [u32; 2]) -> Self {
        let root = std::env::temp_dir().join(format!(
            "tack-product-test-{:032x}",
            new_document_id().unwrap().value()
        ));
        fs::create_dir(&root).unwrap();
        let pixels =
            image::RgbImage::from_fn(size[0], size[1], |x, y| image::Rgb([x as u8, y as u8, 200]));
        pixels.save(root.join("source.png")).unwrap();
        let source = new_source_id().unwrap();
        let asset = new_asset_id().unwrap();
        let location = if embedded {
            SourceLocation::Embedded
        } else {
            SourceLocation::Linked(LinkedPath::native(Path::new("source.png")).unwrap())
        };
        let s = Source::from_descriptor(
            source,
            location,
            1,
            Some(source_fingerprint(&root.join("source.png")).unwrap()),
        )
        .unwrap();
        let mut document = Document::new(new_document_id().unwrap(), DocumentLimits::default());
        document.apply(Command::AddSource(s)).unwrap();
        document
            .apply(Command::AddAsset(
                ImageAsset::new(asset, source, size).unwrap(),
            ))
            .unwrap();
        document
            .apply(Command::AddObject {
                object: DocumentObject::image(
                    new_object_id().unwrap(),
                    asset,
                    Transform::new([50., 50.], [96., 64.], 0., [false, false]).unwrap(),
                ),
                index: 0,
            })
            .unwrap();
        Self {
            root,
            asset,
            source,
            document,
        }
    }
    fn seed(&self) {
        let originals = if matches!(
            self.document.source(self.source).unwrap().location(),
            SourceLocation::Embedded
        ) {
            vec![BlobInput::original(
                self.source,
                1,
                Payload::File(self.root.join("source.png")),
            )]
        } else {
            vec![]
        };
        save(self.root.join("seed.tack"), &self.document, originals).unwrap();
    }
    fn load(&self, file: &str) -> (Arc<TackFile>, ProductAssets) {
        let path = self.root.join(file);
        let b = Arc::new(TackFile::open(&path).unwrap());
        let a = ProductAssets::new(
            Arc::clone(&b),
            &path,
            self.root.join(format!(
                "derived-{:032x}",
                new_document_id().unwrap().value()
            )),
        )
        .unwrap();
        (b, a)
    }
    fn wait(&self, assets: &mut ProductAssets) {
        assert!(assets.request(self.asset));
        let start = Instant::now();
        while assets.stats().pending > 0 {
            assets.poll();
            assert!(start.elapsed() < Duration::from_secs(5));
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    fn prepared(&self) {
        self.seed();
        let (b, mut a) = self.load("seed.tack");
        self.wait(&mut a);
        let p = a.prepared.get(&self.asset).unwrap();
        let mut inputs = vec![BlobInput::overview(
            self.asset,
            1,
            p.size,
            p.generator,
            Payload::File(p.path.clone()),
        )];
        if let Some(e) = b.originals.get(&self.source) {
            inputs.push(BlobInput::original(self.source, 1, b.payload(e.range)));
        }
        save(self.root.join("ready.tack"), &self.document, inputs).unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
#[test]
fn deferred_shared_source_admits_no_codec_or_io_until_verified_supply_arrives() {
    use tack_assets::ProductDemand;
    let f = Fixture::new(false);
    f.seed();
    let (_, mut assets) = f.load("seed.tack");
    let mut canonical = f.document.clone();
    canonical
        .apply_inverse(Command::SetSource(
            Source::from_descriptor(f.source, SourceLocation::Embedded, 1, None).unwrap(),
        ))
        .unwrap();
    assets.defer_shared_source(f.source, 1);
    let demand = [ProductDemand {
        asset: f.asset,
        lod: Lod::Thumbnail,
        edge: 128,
        priority: 2,
        resident: false,
    }];
    for _ in 0..4 {
        assert!(!assets.request_current(f.asset, &canonical, None));
        assert!(assets.failed(f.asset)); // No redraw retry loop while CAS is pending.
        assets.replace_view(&demand, &canonical, &Default::default());
        assets.schedule();
        assert!(!assets.poll());
    }
    let stats = assets.stats();
    assert_eq!(stats.pending, 0);
    assert_eq!(stats.queued, 0);
    assert_eq!(stats.codec_requests, 0);
    assert_eq!(stats.decode_count, 0);
    assert_eq!(stats.source_bytes, 0);
    assert_eq!(stats.container_bytes, 0);
    assert_eq!(stats.derived_bytes, 0);
    assert_eq!(stats.errors, 0);
    assert_eq!(assets.states[&f.source], SourceState::Missing);
    assert!(assets.failed(f.asset));
    assert!(assets.get(f.asset).is_none());

    // Only an off-thread CAS verification result may install this private path.
    assets.set_shared_source(f.source, 1, f.root.join("source.png"));
    assert!(!assets.failed(f.asset));
    assets.replace_view(&demand, &canonical, &Default::default());
    assets.schedule();
    settle_view(&mut assets);
    let image = assets.get(f.asset).unwrap();
    assert_eq!([image.width, image.height], [128, 85]);
    assert!(!image.rgba.is_empty());
    assert_eq!(assets.states[&f.source], SourceState::Available);
    assert_eq!(assets.stats().codec_requests, 1);
    assert_eq!(assets.stats().errors, 0);
    assert!(assets.stats().source_bytes > 0);
    assert_eq!(assets.stats().container_bytes, 0);
    assert_eq!(
        canonical.source(f.source).unwrap().location(),
        &SourceLocation::Embedded
    );
}

#[test]
fn delayed_verified_shared_job_cannot_publish_into_a_new_deferred_revision() {
    let f = Fixture::new(false);
    f.seed();
    let (_, mut assets) = f.load("seed.tack");
    let mut canonical = f.document.clone();
    canonical
        .apply_inverse(Command::SetSource(
            Source::from_descriptor(f.source, SourceLocation::Embedded, 1, None).unwrap(),
        ))
        .unwrap();
    assets.set_shared_source(f.source, 1, f.root.join("source.png"));
    assert!(assets.request_current(f.asset, &canonical, None));
    // Hold publication until an authoritative rejoin/relink advances the source.
    canonical
        .apply(Command::SetSource(
            Source::from_descriptor(f.source, SourceLocation::Embedded, 2, None).unwrap(),
        ))
        .unwrap();
    assets.sync_document(&canonical);
    assets.defer_shared_source(f.source, 2);
    assert!(!assets.request_current(f.asset, &canonical, None));
    settle_view(&mut assets);
    assert_eq!(assets.stats().discarded, 1);
    assert_eq!(assets.stats().errors, 0);
    assert!(assets.get_rep(f.asset, 1, Lod::Thumbnail, 128).is_none());
    assert!(assets.get(f.asset).is_none());
    assert_eq!(assets.states[&f.source], SourceState::Missing);

    assets.set_shared_source(f.source, 2, f.root.join("source.png"));
    assert!(assets.request_current(f.asset, &canonical, None));
    settle_view(&mut assets);
    assert!(assets.get_rep(f.asset, 2, Lod::Thumbnail, 128).is_some());
    assert_eq!(assets.stats().codec_requests, 2);
    assert!(assets.stats().peak_pending <= assets.limits().requests);
    assert!(assets.stats().cpu_peak <= assets.limits().cpu_bytes);
}
#[test]
fn linked_reuse_missing_changed_and_independent_repair() {
    let f = Fixture::new(false);
    f.prepared();
    let (b, mut a) = f.load("ready.tack");
    f.wait(&mut a);
    assert_eq!(a.stats().reused, 1);
    assert_eq!(a.stats().source_bytes, 0);
    assert_eq!(a.states[&f.source], SourceState::Available);
    drop(a);
    fs::rename(f.root.join("source.png"), f.root.join("hidden.png")).unwrap();
    let (_, mut a) = f.load("ready.tack");
    f.wait(&mut a);
    assert_eq!(a.stats().reused, 1);
    assert_eq!(a.stats().errors, 0);
    assert_eq!(a.states[&f.source], SourceState::Missing);
    assert_eq!(a.stats().source_bytes, 0);
    drop(a);
    fs::rename(f.root.join("hidden.png"), f.root.join("source.png")).unwrap();
    let e = b.overviews[&f.asset];
    let mut file = fs::OpenOptions::new()
        .write(true)
        .open(f.root.join("ready.tack"))
        .unwrap();
    file.seek(SeekFrom::Start(e.range.offset)).unwrap();
    file.write_all(b"broken!").unwrap();
    drop(file);
    let (broken, mut a) = f.load("ready.tack");
    assert_eq!(broken.document, f.document);
    f.wait(&mut a);
    assert_eq!(a.stats().regenerated, 1);
    assert!(a.stats().source_bytes > 0);
    assert_eq!(a.stats().errors, 0);
    drop(a);
    fs::OpenOptions::new()
        .append(true)
        .open(f.root.join("source.png"))
        .unwrap()
        .write_all(b"changed")
        .unwrap();
    let (_, mut a) = f.load("ready.tack");
    f.wait(&mut a);
    assert_eq!(a.states[&f.source], SourceState::Changed);
    assert_eq!(a.stats().errors, 1);
    assert_eq!(a.stats().source_bytes, 0);
    assert!(a.get(f.asset).is_none());
}
#[test]
fn embedded_original_survives_deletion_and_preview_repairs_without_external_io() {
    let f = Fixture::new(true);
    f.prepared();
    let (b, mut a) = f.load("ready.tack");
    fs::remove_file(f.root.join("source.png")).unwrap();
    f.wait(&mut a);
    assert_eq!(a.states[&f.source], SourceState::Embedded);
    assert_eq!(a.stats().reused, 1);
    assert_eq!(a.stats().source_bytes, 0);
    drop(a);
    save(
        f.root.join("missing-preview.tack"),
        &f.document,
        vec![BlobInput::original(
            f.source,
            1,
            b.payload(b.originals[&f.source].range),
        )],
    )
    .unwrap();
    let (_, mut a) = f.load("missing-preview.tack");
    f.wait(&mut a);
    assert_eq!(a.stats().regenerated, 1);
    assert_eq!(a.stats().source_bytes, 0);
    assert!(a.stats().container_bytes > 0);
    assert_eq!(a.get(f.asset).unwrap().rgba.len(), 128 * 85 * 4);
}
#[test]
fn jpeg_metadata_and_native_generator_use_existing_tier() {
    let f = Fixture::new(false);
    image::RgbImage::new(600, 450)
        .save(f.root.join("source.jpg"))
        .unwrap();
    let (size, bytes) = image_metadata(&f.root.join("source.jpg")).unwrap();
    assert_eq!(size, [600, 450]);
    assert!(bytes > 0);
    let s = Source::from_descriptor(
        f.source,
        SourceLocation::Linked(LinkedPath::native(Path::new("source.jpg")).unwrap()),
        2,
        Some(source_fingerprint(&f.root.join("source.jpg")).unwrap()),
    )
    .unwrap();
    let mut d = f.document.clone();
    d.apply(Command::SetSource(s)).unwrap();
    save(f.root.join("jpeg.tack"), &d, vec![]).unwrap();
    let (_, mut a) = f.load("jpeg.tack");
    f.wait(&mut a);
    assert_eq!(a.prepared[&f.asset].generator, 1);
    assert_eq!(a.prepared[&f.asset].size, [128, 96]);
}

#[test]
fn one_bad_missing_or_stale_preview_does_not_affect_another() {
    let mut f = Fixture::new(false);
    f.prepared();
    let valid = Arc::new(TackFile::open(f.root.join("ready.tack")).unwrap());
    let second = new_asset_id().unwrap();
    f.document
        .apply(Command::AddAsset(
            ImageAsset::new(second, f.source, [96, 64]).unwrap(),
        ))
        .unwrap();
    let e = valid.overviews[&f.asset];
    let make = || {
        vec![
            BlobInput::overview(
                f.asset,
                1,
                [e.width, e.height],
                e.generator,
                valid.payload(e.range),
            ),
            BlobInput::overview(
                second,
                1,
                [e.width, e.height],
                e.generator,
                valid.payload(e.range),
            ),
        ]
    };
    save(f.root.join("two.tack"), &f.document, make()).unwrap();
    let good = fs::read(f.root.join("two.tack")).unwrap();
    let run = |name: &str| {
        let (b, mut a) = f.load(name);
        assert!(a.request(f.asset));
        assert!(a.request(second));
        let start = Instant::now();
        while a.stats().pending > 0 {
            a.poll();
            assert!(start.elapsed() < Duration::from_secs(5));
            std::thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(b.document, f.document);
        assert_eq!(a.stats().reused, 1);
        assert_eq!(a.stats().regenerated, 1);
        assert_eq!(a.stats().errors, 0);
    };
    let b = TackFile::open(f.root.join("two.tack")).unwrap();
    let range = b.overviews[&second].range;
    let mut corrupt = good.clone();
    corrupt[range.offset as usize] ^= 1;
    fs::write(f.root.join("corrupt.tack"), corrupt).unwrap();
    run("corrupt.tack");
    save(
        f.root.join("missing.tack"),
        &f.document,
        vec![BlobInput::overview(
            f.asset,
            1,
            [e.width, e.height],
            e.generator,
            valid.payload(e.range),
        )],
    )
    .unwrap();
    run("missing.tack");
    let mut stale = good;
    let auth = u64::from_le_bytes(stale[16..24].try_into().unwrap()) as usize;
    let dir = 80 + auth;
    let index =
        if u128::from_le_bytes(stale[dir + 4..dir + 20].try_into().unwrap()) == second.value() {
            0
        } else {
            1
        };
    stale[dir + index * 64 + 20..dir + index * 64 + 28].copy_from_slice(&2u64.to_le_bytes());
    let crc = crc32fast::hash(&stale[dir..dir + 128]);
    stale[44..48].copy_from_slice(&crc.to_le_bytes());
    fs::write(f.root.join("stale.tack"), stale).unwrap();
    run("stale.tack");
}
#[test]
fn foreign_path_and_changed_fingerprint_keep_last_known_preview() {
    let mut f = Fixture::new(false);
    f.prepared();
    let b = TackFile::open(f.root.join("ready.tack")).unwrap();
    let e = b.overviews[&f.asset];
    let foreign = if cfg!(unix) {
        LinkedPath::encoded(
            PathPlatform::Windows,
            true,
            &[b'C', 0, b':', 0, b'\\', 0, b'x', 0],
        )
        .unwrap()
    } else {
        LinkedPath::encoded(PathPlatform::Unix, true, b"/missing/x").unwrap()
    };
    let source =
        Source::from_descriptor(f.source, SourceLocation::Linked(foreign), 2, None).unwrap();
    f.document.apply(Command::SetSource(source)).unwrap();
    save(
        f.root.join("foreign.tack"),
        &f.document,
        vec![BlobInput::overview(
            f.asset,
            2,
            [e.width, e.height],
            e.generator,
            b.payload(e.range),
        )],
    )
    .unwrap();
    let (_, mut a) = f.load("foreign.tack");
    f.wait(&mut a);
    assert_eq!(a.states[&f.source], SourceState::Foreign);
    assert_eq!(a.stats().reused, 1);
    assert_eq!(a.stats().source_bytes, 0);
    drop(a);
    fs::OpenOptions::new()
        .append(true)
        .open(f.root.join("source.png"))
        .unwrap()
        .write_all(b"changed")
        .unwrap();
    let (_, mut a) = f.load("ready.tack");
    f.wait(&mut a);
    assert_eq!(a.states[&f.source], SourceState::Changed);
    assert_eq!(a.stats().reused, 1);
    assert_eq!(a.stats().source_bytes, 0);
}

#[test]
fn cpu_evictions_reuse_bounded_private_repair_cache() {
    let mut f = Fixture::new(false);
    let second = new_asset_id().unwrap();
    f.document
        .apply(Command::AddAsset(
            ImageAsset::new(second, f.source, [96, 64]).unwrap(),
        ))
        .unwrap();
    f.seed();
    let path = f.root.join("seed.tack");
    let board = Arc::new(TackFile::open(&path).unwrap());
    let dir = f.root.join("cache");
    let mut a =
        ProductAssets::with_budgets(board, &path, dir.clone(), 128 * 85 * 4, 1024 * 1024).unwrap();
    for id in [f.asset, second, f.asset, second, f.asset, second] {
        assert!(a.request(id));
        let start = Instant::now();
        while a.stats().pending > 0 {
            a.poll();
            assert!(start.elapsed() < Duration::from_secs(5));
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(a.get(id).is_some());
    }
    assert_eq!(a.stats().regenerated, 2);
    assert_eq!(a.stats().repair_cache_hits, 4);
    assert_eq!(fs::read_dir(&dir).unwrap().count(), 2);
    let entries: Vec<_> = fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().metadata().unwrap())
        .collect();
    assert!(entries.iter().map(|m| m.len()).sum::<u64>() <= 1024 * 1024);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(&dir).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert!(
            entries
                .iter()
                .all(|m| m.permissions().mode() & 0o777 == 0o600)
        );
    }
}

#[test]
fn embedded_jpeg_uses_ranged_streaming_generator_after_original_deletion() {
    let f = Fixture::new(false);
    let path = f.root.join("standalone.jpg");
    image::RgbImage::from_fn(600, 450, |x, y| image::Rgb([x as u8, y as u8, 80]))
        .save(&path)
        .unwrap();
    let mut d = Document::new(new_document_id().unwrap(), DocumentLimits::default());
    d.apply(Command::AddSource(Source::embedded(f.source)))
        .unwrap();
    d.apply(Command::AddAsset(
        ImageAsset::new(f.asset, f.source, [600, 450]).unwrap(),
    ))
    .unwrap();
    save(
        f.root.join("embedded-jpeg.tack"),
        &d,
        vec![BlobInput::original(
            f.source,
            1,
            Payload::File(path.clone()),
        )],
    )
    .unwrap();
    fs::remove_file(path).unwrap();
    let (_, mut assets) = f.load("embedded-jpeg.tack");
    f.wait(&mut assets);
    assert_eq!(assets.stats().errors, 0);
    assert_eq!(assets.stats().source_bytes, 0);
    assert!(assets.stats().container_bytes > 0);
    assert_eq!(assets.prepared[&f.asset].generator, 2);
    assert_eq!(assets.get(f.asset).unwrap().width, 128);
}

#[test]
fn more_than_256_missing_sources_settle_and_removed_ids_do_not_republish() {
    let f = Fixture::new(false);
    let mut document = Document::new(new_document_id().unwrap(), DocumentLimits::default());
    let mut ids = Vec::new();
    for i in 1..=300 {
        let s = SourceId::new(i).unwrap();
        let a = AssetId::new(i).unwrap();
        document
            .apply(Command::AddSource(
                Source::linked(s, format!("absent-{i}.png")).unwrap(),
            ))
            .unwrap();
        document
            .apply(Command::AddAsset(ImageAsset::new(a, s, [20, 20]).unwrap()))
            .unwrap();
        ids.push(a);
    }
    let path = f.root.join("many-missing.tack");
    save(&path, &document, vec![]).unwrap();
    let board = Arc::new(TackFile::open(&path).unwrap());
    let mut supply = ProductAssets::new(board, &path, f.root.join("negative-cache")).unwrap();
    let start = Instant::now();
    while supply.stats().completed < 300 && start.elapsed() < Duration::from_secs(10) {
        for id in &ids {
            supply.request(*id);
        }
        supply.poll();
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(supply.stats().completed, 300);
    for _ in 0..3 {
        for id in &ids {
            assert!(supply.failed(*id));
            assert!(!supply.request(*id));
        }
        assert!(!supply.poll());
    }
    supply.sync_document(&Document::new(document.id(), DocumentLimits::default()));
    assert!(supply.states.is_empty());
    assert!(supply.prepared.is_empty());
    for id in ids {
        assert!(!supply.failed(id));
    }
}

fn settle_view(a: &mut ProductAssets) {
    let start = Instant::now();
    while a.stats().pending > 0 {
        a.poll();
        a.schedule();
        assert!(start.elapsed() < Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(1));
    }
}
#[test]
fn high_lod_is_worker_only_bounded_and_reuses_derived_cache() {
    use tack_assets::ProductDemand;
    let f = Fixture::sized(true, [1600, 1000]);
    f.prepared();
    let (_, mut a) = f.load("ready.tack");
    let demand = [ProductDemand {
        asset: f.asset,
        lod: Lod::Medium,
        edge: 512,
        priority: 2,
        resident: false,
    }];
    a.replace_view(&demand, &f.document, &Default::default());
    settle_view(&mut a);
    let i = a.get_rep(f.asset, 1, Lod::Medium, 512).unwrap();
    assert_eq!(i.width, 512);
    assert!(i.rgba.len() <= 512 * 512 * 4);
    assert!(a.prepared.is_empty()); // High LOD cannot become persisted document authority.
    assert!(a.stats().container_bytes > 0);
    let completed = a.stats().completed;
    for _ in 0..30 {
        a.replace_view(&demand, &f.document, &Default::default());
        a.poll();
    }
    assert_eq!(a.stats().completed, completed);
    assert_eq!(a.stats().pending, 0);
}
#[test]
fn stale_view_completion_is_discarded_and_suspended_supply_settles() {
    use tack_assets::ProductDemand;
    let f = Fixture::sized(false, [900, 600]);
    f.seed();
    let (_, mut a) = f.load("seed.tack");
    a.replace_view(
        &[ProductDemand {
            asset: f.asset,
            lod: Lod::Detail,
            edge: 2048,
            priority: 2,
            resident: false,
        }],
        &f.document,
        &Default::default(),
    );
    a.suspend();
    settle_view(&mut a);
    assert_eq!(a.stats().discarded, 1);
    assert_eq!(a.stats().cpu_bytes, 0);
    assert!(a.prepared.is_empty());
    assert!(a.states.is_empty());
    a.replace_view(
        &[ProductDemand {
            asset: f.asset,
            lod: Lod::Thumbnail,
            edge: 32,
            priority: 0,
            resident: false,
        }],
        &f.document,
        &Default::default(),
    );
    settle_view(&mut a);
    assert_eq!(a.get_rep(f.asset, 1, Lod::Thumbnail, 32).unwrap().width, 32);
}
#[test]
fn potato_limits_and_existing_disk_cache_are_enforced_without_new_writes() {
    use tack_assets::{ProductDemand, SupplyLimits};
    let f = Fixture::sized(false, [512, 512]);
    f.prepared();
    let path = f.root.join("ready.tack");
    let dir = f.root.join("bounded");
    fs::create_dir(&dir).unwrap();
    fs::write(dir.join("old-2048.png"), vec![0; 10 * 1024 * 1024]).unwrap();
    let mut a = ProductAssets::with_limits(
        Arc::new(TackFile::open(&path).unwrap()),
        &path,
        dir.clone(),
        SupplyLimits::potato(),
    )
    .unwrap();
    a.replace_view(
        &[ProductDemand {
            asset: f.asset,
            lod: Lod::Thumbnail,
            edge: 128,
            priority: 2,
            resident: false,
        }],
        &f.document,
        &Default::default(),
    );
    settle_view(&mut a);
    assert_eq!(a.stats().derived_bytes, 0);
    assert!(!dir.join("old-2048.png").exists());
    assert!(a.stats().peak_pending <= 4);
    assert!(a.stats().cpu_peak <= 8 * 1024 * 1024);
    assert!(
        fs::read_dir(&dir)
            .unwrap()
            .map(|p| p.unwrap().metadata().unwrap().len())
            .sum::<u64>()
            <= 8 * 1024 * 1024
    );
}

#[test]
fn distant_jump_replaces_old_queued_demand_and_progresses_new_view() {
    use tack_assets::{ProductDemand, SupplyLimits};
    let mut f = Fixture::sized(false, [640, 400]);
    let mut ids = vec![f.asset];
    for _ in 0..24 {
        let id = new_asset_id().unwrap();
        f.document
            .apply(Command::AddAsset(
                ImageAsset::new(id, f.source, [640, 400]).unwrap(),
            ))
            .unwrap();
        ids.push(id);
    }
    f.seed();
    let path = f.root.join("seed.tack");
    let mut a = ProductAssets::with_limits(
        Arc::new(TackFile::open(&path).unwrap()),
        &path,
        f.root.join("jump-cache"),
        SupplyLimits::potato(),
    )
    .unwrap();
    let demands: Vec<_> = ids[..20]
        .iter()
        .map(|id| ProductDemand {
            asset: *id,
            lod: Lod::Thumbnail,
            edge: 128,
            priority: 0,
            resident: false,
        })
        .collect();
    a.replace_view(&demands, &f.document, &Default::default());
    assert!(a.stats().pending <= 4);
    assert!(a.stats().queued > 0);
    let new = [ProductDemand {
        asset: ids[24],
        lod: Lod::Thumbnail,
        edge: 128,
        priority: 0,
        resident: false,
    }];
    a.replace_view(&new, &f.document, &Default::default());
    settle_view(&mut a);
    assert!(a.stats().reprioritized > 0);
    assert!(a.stats().discarded > 0);
    assert!(a.get_rep(ids[24], 1, Lod::Thumbnail, 128).is_some());
    assert!(a.get_rep(ids[1], 1, Lod::Thumbnail, 128).is_none());
    let completed = a.stats().completed;
    for _ in 0..100 {
        a.replace_view(&new, &f.document, &Default::default());
        a.poll();
    }
    assert_eq!(completed, a.stats().completed);
    assert_eq!(a.stats().pending, 0);
}

#[test]
fn closing_revokes_queue_without_deriving_closed_view() {
    use tack_assets::{ProductDemand, SupplyLimits};
    let mut f = Fixture::sized(false, [640, 400]);
    let mut ids = vec![f.asset];
    for _ in 0..6 {
        let id = new_asset_id().unwrap();
        f.document
            .apply(Command::AddAsset(
                ImageAsset::new(id, f.source, [640, 400]).unwrap(),
            ))
            .unwrap();
        ids.push(id);
    }
    f.seed();
    let path = f.root.join("seed.tack");
    let mut a = ProductAssets::with_limits(
        Arc::new(TackFile::open(&path).unwrap()),
        &path,
        f.root.join("close-cache"),
        SupplyLimits::potato(),
    )
    .unwrap();
    let demand: Vec<_> = ids
        .iter()
        .map(|id| ProductDemand {
            asset: *id,
            lod: Lod::Thumbnail,
            edge: 128,
            priority: 0,
            resident: false,
        })
        .collect();
    a.replace_view(&demand, &f.document, &Default::default());
    assert!(a.stats().queued > 0);
    a.suspend();
    settle_view(&mut a);
    assert_eq!(a.stats().completed, 1);
    assert_eq!(a.stats().cpu_bytes, 0);
    assert_eq!(a.stats().pending, 0);
}

#[test]
fn repeated_expensive_source_leaves_admission_for_small_visible_source() {
    use tack_assets::ProductDemand;
    let mut f = Fixture::sized(false, [640, 400]);
    let mut ids = vec![f.asset];
    for _ in 0..24 {
        let id = new_asset_id().unwrap();
        f.document
            .apply(Command::AddAsset(
                ImageAsset::new(id, f.source, [640, 400]).unwrap(),
            ))
            .unwrap();
        ids.push(id);
    }
    let small_source = new_source_id().unwrap();
    let small_asset = new_asset_id().unwrap();
    image::RgbImage::new(8, 8)
        .save(f.root.join("small.png"))
        .unwrap();
    f.document
        .apply(Command::AddSource(
            Source::from_descriptor(
                small_source,
                SourceLocation::Linked(LinkedPath::native(Path::new("small.png")).unwrap()),
                1,
                None,
            )
            .unwrap(),
        ))
        .unwrap();
    f.document
        .apply(Command::AddAsset(
            ImageAsset::new(small_asset, small_source, [8, 8]).unwrap(),
        ))
        .unwrap();
    ids.push(small_asset);
    f.seed();
    let (_, mut a) = f.load("seed.tack");
    let demands: Vec<_> = ids
        .iter()
        .map(|id| ProductDemand {
            asset: *id,
            lod: Lod::Thumbnail,
            edge: 128,
            priority: u8::from(*id == small_asset),
            resident: false,
        })
        .collect();
    a.replace_view(&demands, &f.document, &Default::default());
    // 25 aliases must not fill all 16 requests ahead of a lower-priority source.
    assert_eq!(a.stats().pending, 3);
    assert_eq!(a.stats().queued, 1);
    settle_view(&mut a);
    assert!(a.get_rep(small_asset, 1, Lod::Thumbnail, 128).is_some());
    assert!(a.get_rep(ids[2], 1, Lod::Thumbnail, 128).is_none());
    assert_eq!(a.stats().completed, 3);
    // The remaining aliases eventually resolve through subsequent visible demand.
    for _ in 0..24 {
        a.replace_view(&demands, &f.document, &Default::default());
        settle_view(&mut a);
    }
    assert!(
        ids.iter()
            .all(|id| a.get_rep(*id, 1, Lod::Thumbnail, 128).is_some())
    );
}
#[test]
fn delayed_zoom_result_cannot_suppress_the_new_tier_or_return_to_detail() {
    use tack_assets::ProductDemand;
    let f = Fixture::sized(false, [900, 600]);
    f.seed();
    let (_, mut a) = f.load("seed.tack");
    a.enable_lod_diagnostics();
    let demand = |lod| {
        [ProductDemand {
            asset: f.asset,
            lod,
            edge: lod.edge(),
            priority: 2,
            resident: false,
        }]
    };
    a.replace_view(&demand(Lod::Detail), &f.document, &Default::default());
    let first = a.rep_trace(f.asset, 1, Lod::Detail, 2048).unwrap();
    assert!(first.request_generation.is_some());
    // Explicitly withhold publication across the camera's tier change. Production
    // workers have no camera epoch ownership; wanted keys control relevance.
    std::thread::sleep(Duration::from_millis(20));
    a.replace_view(&demand(Lod::Medium), &f.document, &Default::default());
    let deadline = Instant::now() + Duration::from_secs(10);
    while a.stats().discarded == 0 {
        a.poll();
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(a.stats().discarded, 1);
    let stale = a.rep_trace(f.asset, 1, Lod::Detail, 2048).unwrap();
    assert!(stale.publication.is_some_and(|(_, accepted)| !accepted));
    assert!(a.get_lod(f.asset, 1, Lod::Detail).is_none());
    // Admission remains live after rejecting the obsolete worker result.
    a.replace_view(&demand(Lod::Medium), &f.document, &Default::default());
    a.schedule();
    settle_view(&mut a);
    assert_eq!(a.get_lod(f.asset, 1, Lod::Medium).unwrap().width, 512);
    a.replace_view(&demand(Lod::Detail), &f.document, &Default::default());
    a.schedule();
    settle_view(&mut a);
    assert_eq!(a.get_lod(f.asset, 1, Lod::Detail).unwrap().width, 2048);
    assert_eq!(a.stats().pending, 0);
    assert!(a.stats().peak_pending <= a.limits().requests);
    assert!(a.stats().cpu_peak <= a.limits().cpu_bytes);
}
#[test]
fn revision_change_rejects_delayed_pixels_then_admits_current_revision() {
    use tack_assets::ProductDemand;
    let f = Fixture::sized(false, [600, 400]);
    f.seed();
    let (_, mut assets) = f.load("seed.tack");
    let demand = [ProductDemand {
        asset: f.asset,
        lod: Lod::Medium,
        edge: 512,
        priority: 2,
        resident: false,
    }];
    assets.replace_view(&demand, &f.document, &Default::default());
    let mut document = f.document.clone();
    let old = document.source(f.source).unwrap();
    let source =
        Source::from_descriptor(f.source, old.location().clone(), 2, old.fingerprint()).unwrap();
    document.apply(Command::SetSource(source)).unwrap();
    assets.replace_view(&demand, &document, &Default::default());
    settle_view(&mut assets);
    assert_eq!(assets.stats().discarded, 1);
    assert!(assets.get_rep(f.asset, 1, Lod::Medium, 512).is_none());
    assert!(assets.get_rep(f.asset, 2, Lod::Medium, 512).is_some());
    assert_eq!(assets.stats().codec_requests, 2);
    assert_eq!(assets.stats().decode_count, 2);
    assert!(assets.stats().decoded_bytes > 0);
    assert_eq!(assets.stats().pending, 0);
}

fn tile_demand(asset: AssetId) -> [tack_assets::ProductDemand; 1] {
    [tack_assets::ProductDemand {
        asset,
        lod: Lod::Detail,
        edge: tack_assets::huge_image::Tile { mip: 0, x: 1, y: 0 }
            .tag()
            .unwrap(),
        priority: 2,
        resident: false,
    }]
}

fn load_tile_cache(f: &Fixture) -> ProductAssets {
    let path = f.root.join("ready.tack");
    ProductAssets::new(
        Arc::new(TackFile::open(&path).unwrap()),
        &path,
        f.root.join("shared-tiles"),
    )
    .unwrap()
}

#[test]
fn unavailable_link_cannot_publish_warm_tiles_but_keeps_last_known_overview() {
    for state in [
        SourceState::Changed,
        SourceState::Missing,
        SourceState::Unavailable,
    ] {
        // Wide enough to use the streaming generator without a large fixture.
        let f = Fixture::sized(false, [7000, 1]);
        f.prepared();
        let demand = tile_demand(f.asset);
        let edge = demand[0].edge;
        let mut assets = load_tile_cache(&f);
        assets.replace_view(&demand, &f.document, &Default::default());
        settle_view(&mut assets);
        assert_eq!(
            assets.get_rep(f.asset, 1, Lod::Detail, edge).unwrap().width,
            256
        );
        assert!(assets.supports_tiles(f.source, 1));
        assert!(assets.stats().derived_bytes > 0);
        drop(assets);
        let source = f.root.join("source.png");
        match state {
            SourceState::Changed => fs::OpenOptions::new()
                .append(true)
                .open(&source)
                .unwrap()
                .write_all(b"changed")
                .unwrap(),
            SourceState::Missing => fs::remove_file(&source).unwrap(),
            SourceState::Unavailable => {
                fs::remove_file(&source).unwrap();
                fs::create_dir(&source).unwrap();
            }
            _ => unreachable!(),
        }
        let mut assets = load_tile_cache(&f);
        f.wait(&mut assets);
        assert_eq!(assets.states[&f.source], state);
        assert!(assets.get_current(f.asset, 1).is_some());
        assets.replace_view(&demand, &f.document, &Default::default());
        settle_view(&mut assets);
        assert_eq!(assets.states[&f.source], state);
        assert!(assets.get_rep(f.asset, 1, Lod::Detail, edge).is_none());
        assert!(assets.failed_rep(f.asset, Lod::Detail, edge));
        assert!(!assets.supports_tiles(f.source, 1));
        assert_eq!(assets.stats().repair_cache_bytes, 0);
        assert_eq!(assets.stats().source_bytes, 0);
        assert!(assets.get_current(f.asset, 1).is_some());
    }
}

#[test]
fn tile_capability_tracks_source_state_and_explicit_revision() {
    let f = Fixture::sized(true, [7000, 1]);
    f.prepared();
    let demand = tile_demand(f.asset);
    let mut assets = load_tile_cache(&f);
    assets.replace_view(&demand, &f.document, &Default::default());
    settle_view(&mut assets);
    assert!(assets.supports_tiles(f.source, 1));
    assert!(!assets.supports_tiles(f.source, 2));
    for state in [
        SourceState::Changed,
        SourceState::Missing,
        SourceState::Unavailable,
        SourceState::Foreign,
    ] {
        assets.states.insert(f.source, state);
        assert!(!assets.supports_tiles(f.source, 1));
    }
    assets.states.insert(f.source, SourceState::Embedded);
    assert!(assets.supports_tiles(f.source, 1));
    let mut document = f.document.clone();
    let source = document.source(f.source).unwrap();
    document
        .apply(Command::SetSource(
            Source::from_descriptor(f.source, source.location().clone(), 2, source.fingerprint())
                .unwrap(),
        ))
        .unwrap();
    assets.sync_document(&document);
    assert!(!assets.supports_tiles(f.source, 1));
    assert!(!assets.supports_tiles(f.source, 2));
    drop(assets);
    fs::remove_file(f.root.join("source.png")).unwrap();
    let mut assets = load_tile_cache(&f);
    assets.replace_view(&demand, &f.document, &Default::default());
    settle_view(&mut assets);
    assert!(
        assets
            .get_rep(f.asset, 1, Lod::Detail, demand[0].edge)
            .is_some()
    );
    assert!(assets.supports_tiles(f.source, 1));
    assert_eq!(assets.stats().repair_cache_hits, 1);
    assert_eq!(assets.stats().source_bytes, 0);
}

#[test]
fn stale_tile_completion_does_not_poison_readmission() {
    let f = Fixture::sized(false, [7000, 1]);
    f.prepared();
    let demand = tile_demand(f.asset);
    let edge = demand[0].edge;
    let mut assets = load_tile_cache(&f);
    assets.replace_view(&demand, &f.document, &Default::default());
    assets.suspend();
    settle_view(&mut assets);
    assert_eq!(assets.stats().discarded, 1);
    assert_eq!(assets.stats().errors, 0);
    assert!(!assets.failed_rep(f.asset, Lod::Detail, edge));
    assert!(assets.get_rep(f.asset, 1, Lod::Detail, edge).is_none());
    assert!(!assets.supports_tiles(f.source, 1));
    assets.replace_view(&demand, &f.document, &Default::default());
    settle_view(&mut assets);
    assert!(assets.get_rep(f.asset, 1, Lod::Detail, edge).is_some());
    assert!(assets.supports_tiles(f.source, 1));
    assert_eq!(assets.stats().errors, 0);
}

#[test]
fn cancelled_huge_tile_can_be_requested_again() {
    let mut f = Fixture::sized(false, [7000, 1]);
    let size = [8192, 1025]; // Decoded RGBA crosses the monolithic ceiling.
    let source_path = f.root.join("source.png");
    {
        let mut encoder =
            png::Encoder::new(fs::File::create(&source_path).unwrap(), size[0], size[1]);
        encoder.set_color(png::ColorType::Grayscale);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        {
            let mut stream = writer.stream_writer().unwrap();
            let row = vec![93; size[0] as usize];
            for _ in 0..size[1] {
                stream.write_all(&row).unwrap();
            }
            stream.finish().unwrap();
        }
        writer.finish().unwrap();
    }
    let location = f.document.source(f.source).unwrap().location().clone();
    f.document
        .apply(Command::SetSource(
            Source::from_descriptor(
                f.source,
                location,
                2,
                Some(source_fingerprint(&source_path).unwrap()),
            )
            .unwrap(),
        ))
        .unwrap();
    f.document
        .apply(Command::SetAsset(
            ImageAsset::new(f.asset, f.source, size).unwrap(),
        ))
        .unwrap();
    f.seed();
    let (_, mut assets) = f.load("seed.tack");
    let demand = tile_demand(f.asset);
    let edge = demand[0].edge;
    assets.replace_view(&demand, &f.document, &Default::default());
    // Cancels the bounded running job before publication, even if its worker
    // finishes first; this must not put the representation in the failed set.
    assets.suspend();
    settle_view(&mut assets);
    assert_eq!(assets.stats().discarded, 1);
    assert_eq!(assets.stats().errors, 0);
    assert!(!assets.failed_rep(f.asset, Lod::Detail, edge));
    assets.replace_view(&demand, &f.document, &Default::default());
    settle_view(&mut assets);
    assert!(assets.get_rep(f.asset, 2, Lod::Detail, edge).is_some());
    assert!(assets.supports_tiles(f.source, 2));
    assert_eq!(assets.stats().errors, 0);
}
