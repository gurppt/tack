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
        let root = std::env::temp_dir().join(format!(
            "tack-product-test-{:032x}",
            new_document_id().unwrap().value()
        ));
        fs::create_dir(&root).unwrap();
        let pixels = image::RgbImage::from_fn(96, 64, |x, y| image::Rgb([x as u8, y as u8, 200]));
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
                ImageAsset::new(asset, source, [96, 64]).unwrap(),
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
