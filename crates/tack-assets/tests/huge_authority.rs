#![allow(clippy::unwrap_used)]
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};
use tack_assets::{
    ProductAssets, ProductDemand, SourceState, huge_image::Tile, source_fingerprint,
};
use tack_core::*;
use tack_storage::*;

struct HugeBoard {
    root: PathBuf,
    asset: AssetId,
    source: SourceId,
    document: Document,
    original: Vec<u8>,
    embedded: bool,
}

impl HugeBoard {
    fn new(embedded: bool) -> Self {
        let root = std::env::temp_dir().join(format!(
            "tack-huge-authority-{:032x}",
            new_document_id().unwrap().value()
        ));
        fs::create_dir(&root).unwrap();
        let path = root.join("original.png");
        write_huge_png(&path);
        let original = fs::read(&path).unwrap();
        assert!(original.len() < 1024 * 1024);
        let asset = new_asset_id().unwrap();
        let source = new_source_id().unwrap();
        let location = if embedded {
            SourceLocation::Embedded
        } else {
            SourceLocation::Linked(LinkedPath::native(Path::new("original.png")).unwrap())
        };
        let mut document = Document::new(new_document_id().unwrap(), DocumentLimits::default());
        document
            .apply(Command::AddSource(
                Source::from_descriptor(
                    source,
                    location,
                    1,
                    Some(source_fingerprint(&path).unwrap()),
                )
                .unwrap(),
            ))
            .unwrap();
        document
            .apply(Command::AddAsset(
                ImageAsset::new(asset, source, [8192, 1025]).unwrap(),
            ))
            .unwrap();
        document
            .apply(Command::AddObject {
                object: DocumentObject::image(
                    new_object_id().unwrap(),
                    asset,
                    Transform::new([0., 0.], [8192., 1025.], 0., [false, false]).unwrap(),
                ),
                index: 0,
            })
            .unwrap();
        let originals = if embedded {
            vec![BlobInput::original(source, 1, Payload::File(path))]
        } else {
            vec![]
        };
        save(root.join("seed.tack"), &document, originals).unwrap();
        Self {
            root,
            asset,
            source,
            document,
            original,
            embedded,
        }
    }

    fn load(&self, name: &str) -> (Arc<TackFile>, ProductAssets) {
        let path = self.root.join(name);
        let board = Arc::new(TackFile::open(&path).unwrap());
        let assets =
            ProductAssets::new(Arc::clone(&board), &path, self.root.join("derived")).unwrap();
        (board, assets)
    }

    fn check_original(&self, board: &TackFile) {
        assert_eq!(board.originals.len(), usize::from(self.embedded));
        if self.embedded {
            assert_eq!(
                board.originals[&self.source].range.len,
                self.original.len() as u64
            );
            let mut bytes = Vec::new();
            board
                .original_reader(self.source)
                .unwrap()
                .read_to_end(&mut bytes)
                .unwrap();
            assert_eq!(bytes, self.original);
        }
    }
}

impl Drop for HugeBoard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn write_huge_png(path: &Path) {
    let mut encoder = png::Encoder::new(fs::File::create(path).unwrap(), 8192, 1025);
    encoder.set_color(png::ColorType::Grayscale);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().unwrap();
    {
        let mut stream = writer.stream_writer().unwrap();
        let mut row = [0; 8192];
        for y in 0..1025 {
            row.fill((y % 251) as u8);
            stream.write_all(&row).unwrap();
        }
        stream.finish().unwrap();
    }
    writer.finish().unwrap();
}

fn settle(assets: &mut ProductAssets) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while assets.stats().pending > 0 {
        assets.poll();
        assets.schedule();
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn check_recovery_authority(fixture: &HugeBoard, board: &TackFile) {
    let path = fixture.root.join("saved.tack");
    let normal_bytes = fs::read(&path).unwrap();
    let lease = BoardLease::acquire(&path).unwrap();
    let mut changed = fixture.document.clone();
    changed
        .apply(Command::AddObject {
            object: DocumentObject::frame(
                new_object_id().unwrap(),
                "Recovery edit".into(),
                Transform::new([10.; 2], [100.; 2], 0., [false; 2]).unwrap(),
            )
            .unwrap(),
            index: 1,
        })
        .unwrap();
    let overview = board.overviews[&fixture.asset];
    let mut blobs = vec![BlobInput::overview(
        fixture.asset,
        1,
        [overview.width, overview.height],
        overview.generator,
        board.payload(overview.range),
    )];
    if fixture.embedded {
        blobs.push(BlobInput::original(
            fixture.source,
            1,
            board.payload(board.originals[&fixture.source].range),
        ));
    }
    lease.save_recovery(&changed, 1, blobs).unwrap();
    drop(lease);
    let lease = BoardLease::acquire(&path).unwrap();
    let (recovered, generation) = lease.recovery(changed.id()).unwrap().unwrap();
    assert_eq!(generation, 1);
    assert_eq!(recovered.document, changed);
    assert_eq!(
        recovered
            .document
            .source(fixture.source)
            .unwrap()
            .revision(),
        1
    );
    assert_eq!(recovered.overviews.len(), 1);
    assert_eq!(recovered.overviews[&fixture.asset].generator, 2);
    fixture.check_original(&recovered);
    assert!(DocumentEditor::recovered(recovered.document, 200).is_dirty());
    assert_eq!(fs::read(path).unwrap(), normal_bytes);
}

fn save_reopen_authority(embedded: bool) {
    let fixture = HugeBoard::new(embedded);
    let (board, mut assets) = fixture.load("seed.tack");
    assert!(assets.request(fixture.asset));
    settle(&mut assets);
    assert!(assets.supports_tiles(fixture.source, 1));
    let overview = assets.prepared[&fixture.asset].clone();
    assert_eq!(overview.generator, 2); // Existing on-disk format stays compatible.
    assert!(
        overview
            .path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .ends_with("-1-3-128.png")
    );
    let tile = Tile {
        mip: 0,
        x: 31,
        y: 4,
    }
    .tag()
    .unwrap();
    let demand = [ProductDemand {
        asset: fixture.asset,
        lod: Lod::Detail,
        edge: tile,
        priority: 2,
        resident: false,
    }];
    assets.replace_view(&demand, &fixture.document, &Default::default());
    settle(&mut assets);
    let image = assets.get_rep(fixture.asset, 1, Lod::Detail, tile).unwrap();
    assert_eq!([image.width, image.height], [256, 1]);
    let expected_tile = image.rgba.clone();
    let tile_path = fixture
        .root
        .join("derived")
        .join(format!("{:032x}-1-3-{tile}.png", fixture.source.value()));
    assert!(tile_path.is_file());
    let mut blobs = vec![BlobInput::overview(
        fixture.asset,
        1,
        overview.size,
        overview.generator,
        Payload::File(overview.path),
    )];
    if embedded {
        blobs.push(BlobInput::original(
            fixture.source,
            1,
            board.payload(board.originals[&fixture.source].range),
        ));
    }
    save(fixture.root.join("saved.tack"), &fixture.document, blobs).unwrap();
    assert_eq!(
        fs::read(fixture.root.join("original.png")).unwrap(),
        fixture.original
    );
    drop(assets);
    drop(board);
    if embedded {
        fs::remove_file(fixture.root.join("original.png")).unwrap();
    }

    let (board, mut assets) = fixture.load("saved.tack");
    assert_eq!(board.document, fixture.document);
    assert_eq!(board.overviews.len(), 1); // Tiles never become container authority.
    assert_eq!(board.overviews[&fixture.asset].generator, 2);
    fixture.check_original(&board);
    assert!(assets.request(fixture.asset));
    settle(&mut assets);
    let expected_state = if embedded {
        SourceState::Embedded
    } else {
        SourceState::Available
    };
    assert_eq!(assets.states[&fixture.source], expected_state);
    assert_eq!(assets.stats().source_bytes, 0);
    assets.replace_view(&demand, &fixture.document, &Default::default());
    settle(&mut assets);
    assert_eq!(
        assets
            .get_rep(fixture.asset, 1, Lod::Detail, tile)
            .unwrap()
            .rgba,
        expected_tile
    );
    assert!(assets.supports_tiles(fixture.source, 1));
    assert_eq!(assets.stats().repair_cache_hits, 1);
    assert_eq!(assets.stats().source_bytes, 0);
    drop(assets);
    check_recovery_authority(&fixture, &board);

    // Removing a disposable tile affects neither saved metadata nor originals.
    fs::remove_file(tile_path).unwrap();
    let (_, mut assets) = fixture.load("saved.tack");
    assets.replace_view(&demand, &fixture.document, &Default::default());
    settle(&mut assets);
    assert_eq!(
        assets
            .get_rep(fixture.asset, 1, Lod::Detail, tile)
            .unwrap()
            .rgba,
        expected_tile
    );
    assert_eq!(assets.stats().repair_cache_hits, 0);
    assert!(assets.stats().derived_bytes > 0);
    fixture.check_original(&board);
    if embedded {
        assert_eq!(assets.stats().source_bytes, 0);
        assert!(assets.stats().container_bytes > 0);
    } else {
        assert!(assets.stats().source_bytes > 0);
        assert_eq!(
            fs::read(fixture.root.join("original.png")).unwrap(),
            fixture.original
        );
    }
}

#[test]
fn linked_huge_png_save_reopen_preserves_original_and_reuses_disposable_tile() {
    save_reopen_authority(false);
}

#[test]
fn embedded_huge_png_save_reopen_preserves_original_and_repairs_disposable_tile() {
    save_reopen_authority(true);
}
