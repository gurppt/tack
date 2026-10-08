#![allow(clippy::unwrap_used)]
use std::{
    fs,
    io::Read,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use tack_assets::{
    ProductAssets, ProductDemand, SourceState, huge_image::Tile, source_fingerprint,
};
use tack_core::*;
use tack_storage::*;

struct Fixture {
    root: PathBuf,
    document: Document,
    source: SourceId,
    asset: AssetId,
    original: Vec<u8>,
    embedded: bool,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
fn jpeg(value: u8) -> Vec<u8> {
    let image = image::RgbImage::from_fn(9000, 17, |x, y| {
        image::Rgb([value, (x % 251) as u8, (y % 241) as u8])
    });
    let mut bytes = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 90)
        .encode_image(&image)
        .unwrap();
    bytes
}
impl Fixture {
    fn new(embedded: bool) -> Self {
        let root = std::env::temp_dir().join(format!(
            "tack-jpeg-authority-{:032x}",
            new_document_id().unwrap().value()
        ));
        fs::create_dir(&root).unwrap();
        let original = jpeg(19);
        fs::write(root.join("original.jpg"), &original).unwrap();
        let source = new_source_id().unwrap();
        let asset = new_asset_id().unwrap();
        let mut document = Document::new(new_document_id().unwrap(), DocumentLimits::default());
        let location = if embedded {
            SourceLocation::Embedded
        } else {
            SourceLocation::Linked(
                LinkedPath::native(std::path::Path::new("original.jpg")).unwrap(),
            )
        };
        document
            .apply(Command::AddSource(
                Source::from_descriptor(
                    source,
                    location,
                    1,
                    Some(source_fingerprint(&root.join("original.jpg")).unwrap()),
                )
                .unwrap(),
            ))
            .unwrap();
        document
            .apply(Command::AddAsset(
                ImageAsset::new(asset, source, [9000, 17]).unwrap(),
            ))
            .unwrap();
        let originals = if embedded {
            vec![BlobInput::original(
                source,
                1,
                Payload::File(root.join("original.jpg")),
            )]
        } else {
            vec![]
        };
        save(root.join("seed.tack"), &document, originals).unwrap();
        Self {
            root,
            document,
            source,
            asset,
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
    fn demand(&self) -> [ProductDemand; 1] {
        [ProductDemand {
            asset: self.asset,
            lod: Lod::Detail,
            edge: Tile {
                mip: 0,
                x: 33,
                y: 0,
            }
            .tag()
            .unwrap(),
            priority: 1,
            resident: false,
        }]
    }
    fn check_original(&self, board: &TackFile) {
        if self.embedded {
            let mut bytes = Vec::new();
            board
                .original_reader(self.source)
                .unwrap()
                .read_to_end(&mut bytes)
                .unwrap();
            assert_eq!(bytes, self.original);
        } else {
            assert_eq!(
                fs::read(self.root.join("original.jpg")).unwrap(),
                self.original
            );
        }
    }
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
fn save_reopen(embedded: bool) {
    let f = Fixture::new(embedded);
    let (board, mut assets) = f.load("seed.tack");
    assets.request(f.asset);
    settle(&mut assets);
    assert!(assets.supports_jpeg_tiles(f.source, 1));
    let preview = assets.prepared[&f.asset].clone();
    assets.replace_view(&f.demand(), &f.document, &Default::default());
    settle(&mut assets);
    let edge = f.demand()[0].edge;
    let tile = assets.get_rep(f.asset, 1, Lod::Detail, edge).unwrap();
    assert_eq!([tile.width, tile.height], [258, 19]);
    let expected = tile.rgba.clone();
    let mut blobs = vec![BlobInput::overview(
        f.asset,
        1,
        preview.size,
        preview.generator,
        Payload::File(preview.path),
    )];
    if embedded {
        blobs.push(BlobInput::original(
            f.source,
            1,
            board.payload(board.originals[&f.source].range),
        ));
    }
    save(f.root.join("saved.tack"), &f.document, blobs).unwrap();
    f.check_original(&board);
    drop(assets);
    drop(board);
    if embedded {
        fs::remove_file(f.root.join("original.jpg")).unwrap();
    }
    let (board, mut assets) = f.load("saved.tack");
    assert_eq!(board.document, f.document);
    assert_eq!(board.overviews.len(), 1);
    f.check_original(&board);
    assets.replace_view(&f.demand(), &f.document, &Default::default());
    settle(&mut assets);
    assert_eq!(
        assets.get_rep(f.asset, 1, Lod::Detail, edge).unwrap().rgba,
        expected
    );
    assert!(assets.supports_jpeg_tiles(f.source, 1));
    assert_eq!(assets.stats().source_bytes, 0);
    assert_eq!(assets.stats().repair_cache_hits, 1);
    drop(assets);
    let tilepath = f
        .root
        .join("derived")
        .join(format!("{:032x}-1-5-{edge}.png", f.source.value()));
    fs::remove_file(tilepath).unwrap();
    let (_, mut assets) = f.load("saved.tack");
    assets.replace_view(&f.demand(), &f.document, &Default::default());
    settle(&mut assets);
    assert_eq!(
        assets.get_rep(f.asset, 1, Lod::Detail, edge).unwrap().rgba,
        expected
    );
    assert!(assets.stats().derived_bytes > 0);
    f.check_original(&board);
}
#[test]
fn linked_jpeg_save_reopen_keeps_original_and_reuses_disposable_gutter_tile() {
    save_reopen(false);
}
#[test]
fn embedded_jpeg_save_reopen_keeps_original_and_reconstructs_missing_tile() {
    save_reopen(true);
}

#[test]
fn changed_link_refuses_cached_jpeg_detail_and_relink_revision_replaces_pixels() {
    let f = Fixture::new(false);
    let (_, mut assets) = f.load("seed.tack");
    let demand = f.demand();
    let edge = demand[0].edge;
    assets.replace_view(&demand, &f.document, &Default::default());
    settle(&mut assets);
    let old = assets
        .get_rep(f.asset, 1, Lod::Detail, edge)
        .unwrap()
        .rgba
        .clone();
    drop(assets);
    let changed = jpeg(177);
    fs::write(f.root.join("original.jpg"), &changed).unwrap();
    let (_, mut assets) = f.load("seed.tack");
    assets.replace_view(&demand, &f.document, &Default::default());
    settle(&mut assets);
    assert_eq!(assets.states[&f.source], SourceState::Changed);
    assert!(!assets.supports_jpeg_tiles(f.source, 1));
    assert!(assets.get_rep(f.asset, 1, Lod::Detail, edge).is_none());
    let mut document = f.document.clone();
    document
        .apply(Command::SetSource(
            Source::from_descriptor(
                f.source,
                SourceLocation::Linked(
                    LinkedPath::native(std::path::Path::new("original.jpg")).unwrap(),
                ),
                2,
                Some(source_fingerprint(&f.root.join("original.jpg")).unwrap()),
            )
            .unwrap(),
        ))
        .unwrap();
    assets.sync_document(&document);
    assets.replace_view(&demand, &document, &Default::default());
    settle(&mut assets);
    assert!(assets.get_rep(f.asset, 1, Lod::Detail, edge).is_none());
    assert_ne!(
        assets.get_rep(f.asset, 2, Lod::Detail, edge).unwrap().rgba,
        old
    );
    assert!(assets.supports_jpeg_tiles(f.source, 2));
    assert!(!assets.supports_jpeg_tiles(f.source, 1));
    assert_eq!(fs::read(f.root.join("original.jpg")).unwrap(), changed);
}
