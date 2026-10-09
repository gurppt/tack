use super::*;
use std::io::{Read, Seek, SeekFrom};
use tack_core::{Command, DocumentLimits, LinkedPath};
use tack_storage::{new_asset_id, new_document_id, new_source_id};

struct Fixture {
    root: PathBuf,
    board: Arc<TackFile>,
    asset: ImageAsset,
    source: Source,
}

impl Fixture {
    fn new(record_fingerprint: bool) -> Result<Self, AssetError> {
        let root = std::env::temp_dir().join(format!(
            "tack-region-products-{:032x}",
            new_document_id()?.value()
        ));
        tack_storage::create_private_directory(&root, false)?;
        write_jpeg(&root.join("source.jpg"), 0)?;
        let fingerprint = if record_fingerprint {
            Some(representation::source_fingerprint(
                &root.join("source.jpg"),
            )?)
        } else {
            None
        };
        let source = Source::from_descriptor(
            new_source_id()?,
            SourceLocation::Linked(LinkedPath::native(Path::new("source.jpg"))?),
            1,
            fingerprint,
        )?;
        let asset = ImageAsset::new(new_asset_id()?, source.id(), [1024, 768])?;
        let mut document = Document::new(new_document_id()?, DocumentLimits::default());
        document.apply(Command::AddSource(source.clone()))?;
        document.apply(Command::AddAsset(asset))?;
        tack_storage::save(root.join("board.tack"), &document, vec![])?;
        let board = Arc::new(TackFile::open(root.join("board.tack"))?);
        Ok(Self {
            root,
            board,
            asset,
            source,
        })
    }

    fn job(&self, x: u32, y: u32) -> Result<Job, AssetError> {
        Ok(Job {
            board: Arc::clone(&self.board),
            asset: self.asset,
            source: self.source.clone(),
            original: None,
            shared_path: None,
            lod: Lod::Medium,
            edge: Tile { mip: 0, x, y }.tag()?,
            cancel: Some(Arc::new(AtomicBool::new(false))),
        })
    }

    fn rectangle(&self) -> Result<Vec<Job>, AssetError> {
        (0..3)
            .flat_map(|y| (0..4).map(move |x| (x, y)))
            .map(|(x, y)| self.job(x, y))
            .collect()
    }

    fn persistent(&self) -> Mutex<Persistent> {
        Mutex::new(Persistent::new(
            Some(self.root.join("persistent")),
            4 * 1024 * 1024,
        ))
    }

    fn load(&self, jobs: Vec<Job>, persistent: &Mutex<Persistent>) -> Vec<Outcome> {
        load_batch(
            WorkBatch { jobs, jpeg: true },
            &self.root,
            &self.root.join("repair"),
            &Mutex::new(crate::decode::DiskCache::new(
                self.root.join("repair"),
                4 * 1024 * 1024,
            )),
            persistent,
        )
    }

    fn assets(&self, limits: SupplyLimits) -> Result<ProductAssets, AssetError> {
        let mut assets = ProductAssets::with_tile_cache(
            Arc::clone(&self.board),
            &self.root.join("board.tack"),
            self.root.join("repair"),
            limits,
            Some(self.root.join("persistent")),
        )?;
        // Capability is normally learned from the overview. These scheduler
        // tests set the already-known fact directly to avoid timing dependence.
        assets
            .streamed_jpeg
            .insert((self.source.id(), self.source.revision()));
        assets
            .states
            .insert(self.source.id(), SourceState::Available);
        Ok(assets)
    }

    fn demands(&self, jobs: &[Job]) -> Vec<ProductDemand> {
        jobs.iter()
            .map(|job| ProductDemand {
                asset: self.asset.id(),
                lod: job.lod,
                edge: job.edge,
                priority: 2,
                resident: false,
            })
            .collect()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn write_jpeg(path: &Path, variant: u8) -> Result<(), AssetError> {
    let pixels = image::RgbImage::from_fn(1024, 768, |x, y| {
        image::Rgb([
            (x % 251) as u8,
            (y % 241) as u8,
            ((x / 17 + y / 19) as u8).wrapping_add(variant),
        ])
    });
    let mut encoded = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(
        &mut encoded,
        if variant == 0 { 95 } else { 65 },
    )
    .encode(pixels.as_raw(), 1024, 768, image::ExtendedColorType::Rgb8)?;
    std::fs::write(path, encoded)?;
    Ok(())
}

fn counts(outcomes: &[Outcome]) -> (usize, usize, u64) {
    (
        outcomes.iter().map(|o| o.region_jobs).sum(),
        outcomes.iter().filter(|o| o.tile_cache_hit).count(),
        outcomes.iter().map(|o| o.source_bytes).sum(),
    )
}

#[test]
fn gather_admits_a_shuffled_twelve_tile_rectangle_without_extra_demand() -> Result<(), AssetError> {
    let f = Fixture::new(true)?;
    let mut jobs = f.rectangle()?;
    let first = jobs.remove(0);
    let original_cancel = first.cancel.clone().ok_or("missing cancellation")?;
    jobs.reverse();
    let batch = gather(first, true, &mut jobs);
    assert_eq!(batch.len(), 12);
    assert!(jobs.is_empty());
    assert!(crate::jpeg_scanlines::admissible_tiles(
        [1024, 768],
        &batch.iter().map(|job| job.edge).collect::<Vec<_>>()
    ));
    assert!(batch.iter().all(|job| {
        job.cancel
            .as_ref()
            .is_some_and(|cancel| Arc::ptr_eq(cancel, &original_cancel))
    }));
    Ok(())
}

#[test]
fn gather_holes_and_diagonals_never_create_unrequested_tiles() -> Result<(), AssetError> {
    let f = Fixture::new(true)?;
    let mut queue = vec![f.job(1, 0)?, f.job(0, 1)?, f.job(2, 2)?];
    let batch = gather(f.job(0, 0)?, true, &mut queue);
    assert_eq!(batch.len(), 2);
    assert_eq!(queue.len(), 2);
    let addresses: Vec<_> = batch.iter().map(|job| job.edge).collect();
    assert!(crate::jpeg_scanlines::admissible_tiles(
        [1024, 768],
        &addresses
    ));
    assert!(
        queue
            .iter()
            .any(|job| Tile::from_tag(job.edge) == Some(Tile { mip: 0, x: 2, y: 2 }))
    );
    Ok(())
}

#[test]
fn gather_keeps_source_revision_mip_lod_and_dimensions_separate() -> Result<(), AssetError> {
    let f = Fixture::new(true)?;
    let ordinary = f.job(1, 0)?;
    let mut wrong_source = ordinary.clone();
    wrong_source.source = Source::from_descriptor(
        new_source_id()?,
        f.source.location().clone(),
        1,
        f.source.fingerprint(),
    )?;
    let mut wrong_revision = ordinary.clone();
    wrong_revision.source = Source::from_descriptor(
        f.source.id(),
        f.source.location().clone(),
        2,
        f.source.fingerprint(),
    )?;
    let mut wrong_mip = ordinary.clone();
    wrong_mip.edge = Tile { mip: 1, x: 0, y: 0 }.tag()?;
    let mut wrong_lod = ordinary.clone();
    wrong_lod.lod = Lod::Detail;
    let mut wrong_dimensions = ordinary.clone();
    wrong_dimensions.asset = ImageAsset::new(new_asset_id()?, f.source.id(), [1024, 512])?;
    let mut queue = vec![
        wrong_source,
        wrong_revision,
        wrong_mip,
        wrong_lod,
        wrong_dimensions,
        ordinary,
    ];
    let batch = gather(f.job(0, 0)?, true, &mut queue);
    assert_eq!(batch.len(), 2);
    assert_eq!(queue.len(), 5);
    assert_eq!(batch[1].source.id(), f.source.id());
    assert_eq!(batch[1].source.revision(), 1);
    let mut queue = vec![f.job(1, 0)?];
    assert_eq!(gather(f.job(0, 0)?, false, &mut queue).len(), 1);
    assert_eq!(queue.len(), 1);
    Ok(())
}

#[test]
fn potato_admits_four_products_and_sharp_zoomout_revokes_the_batch() -> Result<(), AssetError> {
    let f = Fixture::new(true)?;
    let mut assets = f.assets(SupplyLimits::potato())?;
    assets.replace_view(
        &f.demands(&f.rectangle()?),
        &f.board.document,
        &BTreeMap::new(),
    );
    assert_eq!(assets.pending.len(), 4);
    assert_eq!(assets.stats().pending, 4);
    assert!(assets.queue.is_empty());
    let cancel = assets.workers[0]
        .cancel
        .clone()
        .ok_or("missing batch cancellation")?;
    let overview = [ProductDemand {
        asset: f.asset.id(),
        lod: Lod::Thumbnail,
        edge: 128,
        priority: 0,
        resident: false,
    }];
    assets.replace_view(&overview, &f.board.document, &BTreeMap::new());
    assert!(cancel.load(Ordering::Relaxed));
    assert!(assets.queue.is_empty()); // Four active products still occupy the cap.
    Ok(())
}

#[test]
fn overlapping_pan_keeps_batch_alive_but_distant_view_cancels_it() -> Result<(), AssetError> {
    let f = Fixture::new(true)?;
    let mut assets = f.assets(SupplyLimits {
        workers: 1,
        ..SupplyLimits::default()
    })?;
    let jobs = f.rectangle()?;
    assets.replace_view(&f.demands(&jobs), &f.board.document, &BTreeMap::new());
    assert_eq!(assets.pending.len(), 12);
    let cancel = assets.workers[0]
        .cancel
        .clone()
        .ok_or("missing batch cancellation")?;
    assets.replace_view(
        &f.demands(&[f.job(1, 1)?]),
        &f.board.document,
        &BTreeMap::new(),
    );
    assert!(!cancel.load(Ordering::Relaxed));
    assets.replace_view(&[], &f.board.document, &BTreeMap::new());
    assert!(cancel.load(Ordering::Relaxed));
    assert!(assets.queue.is_empty());
    Ok(())
}

#[test]
fn restart_reuses_twelve_native_tiles_without_regional_decode_or_source_reads()
-> Result<(), AssetError> {
    let f = Fixture::new(true)?;
    let original_board = std::fs::read(f.root.join("board.tack"))?;
    let persistent = f.persistent();
    let cold = f.load(f.rectangle()?, &persistent);
    assert!(cold.iter().all(|o| o.result.is_ok()));
    assert_eq!(counts(&cold).0, 1);
    assert_eq!(counts(&cold).1, 0);
    assert!(counts(&cold).2 > 0);
    assert!(
        cold.iter()
            .all(|o| o.tile_cache_write_bytes == 258 * 258 * 4)
    );
    assert!(cold.iter().all(|o| o.tile_disk_bytes <= 4 * 1024 * 1024));
    drop(persistent);
    let reopened = f.persistent();
    let hot = f.load(f.rectangle()?, &reopened);
    assert_eq!(counts(&hot), (0, 12, 0));
    for (a, b) in cold.iter().zip(&hot) {
        assert_eq!(
            a.result.as_ref().map_err(|e| e.to_string())?.rgba,
            b.result.as_ref().map_err(|e| e.to_string())?.rgba
        );
    }
    assert_eq!(std::fs::read(f.root.join("board.tack"))?, original_board);
    Ok(())
}

#[test]
fn cached_holes_decode_only_the_new_contiguous_band() -> Result<(), AssetError> {
    let f = Fixture::new(true)?;
    let persistent = f.persistent();
    let band = (0..3).map(|y| f.job(0, y)).collect::<Result<Vec<_>, _>>()?;
    assert_eq!(counts(&f.load(band, &persistent)).0, 1);
    let rectangle = f.load(f.rectangle()?, &persistent);
    assert!(rectangle.iter().all(|o| o.result.is_ok()));
    assert_eq!(counts(&rectangle).1, 3);
    assert_eq!(counts(&rectangle).0, 1);
    assert_eq!(
        rectangle
            .iter()
            .filter(|o| o.tile_cache_write_bytes > 0)
            .count(),
        9
    );
    Ok(())
}

#[test]
fn new_revision_misses_persistent_pixels_even_with_the_same_original() -> Result<(), AssetError> {
    let f = Fixture::new(true)?;
    let persistent = f.persistent();
    let cold = f.load(vec![f.job(0, 0)?], &persistent);
    assert_eq!(counts(&cold).0, 1);
    let mut revised = f.job(0, 0)?;
    revised.source = Source::from_descriptor(
        f.source.id(),
        f.source.location().clone(),
        2,
        f.source.fingerprint(),
    )?;
    let second = f.load(vec![revised], &persistent);
    assert!(second[0].result.is_ok());
    assert_eq!(counts(&second).0, 1);
    assert_eq!(counts(&second).1, 0);
    assert_eq!(second[0].key.revision, 2);
    Ok(())
}

#[test]
fn late_persistent_hit_for_old_revision_cannot_publish_into_current_view() -> Result<(), AssetError>
{
    let f = Fixture::new(true)?;
    let mut assets = f.assets(SupplyLimits::potato())?;
    let old = f.job(0, 0)?;
    let mut current = f.board.document.clone();
    current.apply_inverse(Command::SetSource(Source::from_descriptor(
        f.source.id(),
        f.source.location().clone(),
        2,
        f.source.fingerprint(),
    )?))?;
    let demand = [ProductDemand {
        asset: f.asset.id(),
        lod: Lod::Medium,
        edge: old.edge,
        priority: 2,
        resident: true,
    }];
    assets.replace_view(&demand, &current, &BTreeMap::new());
    // Supply a completed worker outcome deterministically, after revision2 has
    // become current; no sleeps or races with a native decoder are needed.
    let (sender, receiver) = mpsc::sync_channel(1);
    assets.workers[0].receiver = receiver;
    assets.pending.insert(old.key(), 0);
    let mut outcome = empty_outcome(&old);
    outcome.tile_cache_hit = true;
    outcome.result = Ok(Decoded {
        width: 1,
        height: 1,
        rgba: vec![19; 4],
    });
    sender
        .send(vec![outcome])
        .map_err(|_| "could not supply delayed outcome")?;
    assert!(assets.poll());
    assert_eq!(assets.stats().discarded, 1);
    assert_eq!(assets.stats().cpu_bytes, 0);
    assert!(!assets.cached(old.key()));
    Ok(())
}

#[test]
fn recorded_fingerprint_change_refuses_hot_pixels_before_source_decode() -> Result<(), AssetError> {
    let f = Fixture::new(true)?;
    let persistent = f.persistent();
    assert!(f.load(vec![f.job(0, 0)?], &persistent)[0].result.is_ok());
    write_jpeg(&f.root.join("source.jpg"), 71)?;
    assert_ne!(
        Some(representation::source_fingerprint(
            &f.root.join("source.jpg")
        )?),
        f.source.fingerprint()
    );
    let changed = f.load(vec![f.job(0, 0)?], &persistent);
    assert!(changed[0].result.is_err());
    assert_eq!(counts(&changed), (0, 0, 0));
    Ok(())
}

#[test]
fn unrecorded_fingerprint_still_keys_actual_source_and_rebuilds_changed_pixels()
-> Result<(), AssetError> {
    let f = Fixture::new(false)?;
    let persistent = f.persistent();
    let cold = f.load(vec![f.job(0, 0)?], &persistent);
    assert_eq!(counts(&f.load(vec![f.job(0, 0)?], &persistent)), (0, 1, 0));
    write_jpeg(&f.root.join("source.jpg"), 71)?;
    let changed = f.load(vec![f.job(0, 0)?], &persistent);
    assert_eq!(counts(&changed).0, 1);
    assert_eq!(counts(&changed).1, 0);
    assert_ne!(
        cold[0].result.as_ref().map_err(|e| e.to_string())?.rgba,
        changed[0].result.as_ref().map_err(|e| e.to_string())?.rgba
    );
    Ok(())
}

#[test]
fn corrupt_raw_payload_regenerates_from_authoritative_original() -> Result<(), AssetError> {
    let f = Fixture::new(true)?;
    let persistent = f.persistent();
    let cold = f.load(vec![f.job(0, 0)?], &persistent);
    assert!(cold[0].result.is_ok());
    drop(persistent);
    let mut raw = File::options()
        .read(true)
        .write(true)
        .open(f.root.join("persistent/tile-detail-v1.raw"))?;
    raw.seek(SeekFrom::Start(16 + 128 + 17))?;
    let mut byte = [0];
    raw.read_exact(&mut byte)?;
    raw.seek(SeekFrom::Start(16 + 128 + 17))?;
    raw.write_all(&[byte[0] ^ 0xff])?;
    drop(raw);
    let reopened = f.persistent();
    let rebuilt = f.load(vec![f.job(0, 0)?], &reopened);
    assert_eq!(counts(&rebuilt).0, 1);
    assert_eq!(counts(&rebuilt).1, 0);
    assert_eq!(
        cold[0].result.as_ref().map_err(|e| e.to_string())?.rgba,
        rebuilt[0].result.as_ref().map_err(|e| e.to_string())?.rgba
    );
    assert_eq!(counts(&f.load(vec![f.job(0, 0)?], &reopened)), (0, 1, 0));
    Ok(())
}

#[test]
fn another_cache_owner_never_prevents_authoritative_regional_decode() -> Result<(), AssetError> {
    let f = Fixture::new(true)?;
    let owner = TileDisk::open(&f.root.join("persistent"), 4 * 1024 * 1024)?;
    let persistent = f.persistent();
    let result = f.load(vec![f.job(0, 0)?], &persistent);
    assert!(result[0].result.is_ok());
    assert_eq!(counts(&result).0, 1);
    assert_eq!(counts(&result).1, 0);
    assert_eq!(result[0].tile_cache_write_bytes, 0);
    assert_eq!(result[0].tile_disk_bytes, 0);
    drop(owner);
    Ok(())
}

fn pad_jpeg_comments(path: &Path, target: usize) -> Result<(), AssetError> {
    let mut bytes = std::fs::read(path)?;
    let mut remaining = target
        .checked_sub(bytes.len())
        .ok_or("padding target too small")?;
    while remaining > 0 {
        let mut count = remaining.min(65_537);
        if (1..4).contains(&(remaining - count)) {
            count -= 4;
        }
        if count < 4 {
            return Err("JPEG comment requires four bytes".into());
        }
        let mut comment = vec![0; count];
        comment[..2].copy_from_slice(&[0xff, 0xfe]);
        comment[2..4].copy_from_slice(&((count - 2) as u16).to_be_bytes());
        bytes.splice(2..2, comment);
        remaining -= count;
    }
    std::fs::write(path, bytes)?;
    Ok(())
}

#[test]
fn copied_document_relative_sources_with_equal_stats_have_separate_cache_namespaces()
-> Result<(), AssetError> {
    let f = Fixture::new(false)?;
    let copy_root = f.root.join("copy");
    tack_storage::create_private_directory(&copy_root, false)?;
    std::fs::copy(f.root.join("board.tack"), copy_root.join("board.tack"))?;
    let source_path_a = f.root.join("source.jpg");
    let source_path_b = copy_root.join("source.jpg");
    write_jpeg(&source_path_b, 71)?;
    let target = std::fs::metadata(&source_path_a)?
        .len()
        .max(std::fs::metadata(&source_path_b)?.len()) as usize
        + 8;
    for path in [&source_path_a, &source_path_b] {
        pad_jpeg_comments(path, target)?;
        let modified = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_720_000_000);
        File::options()
            .write(true)
            .open(path)?
            .set_times(std::fs::FileTimes::new().set_modified(modified))?;
    }
    let job_a = f.job(0, 0)?;
    let mut job_b = job_a.clone();
    job_b.board = Arc::new(TackFile::open(copy_root.join("board.tack"))?);
    let identity_a = source_identity(&job_a, Some(&source_path_a))?;
    let identity_b = source_identity(&job_b, Some(&source_path_b))?;
    assert_eq!(identity_a, identity_b);
    assert_eq!(job_a.board.document.id(), job_b.board.document.id());
    assert_eq!(job_a.key(), job_b.key());
    assert_ne!(
        cache_key(
            &job_a,
            identity_a,
            true,
            cache_namespace(&job_a, Some(&source_path_a))?
        ),
        cache_key(
            &job_b,
            identity_b,
            true,
            cache_namespace(&job_b, Some(&source_path_b))?
        )
    );
    let persistent = f.persistent();
    let a = f.load(vec![job_a], &persistent);
    let b = load_batch(
        WorkBatch {
            jobs: vec![job_b],
            jpeg: true,
        },
        &copy_root,
        &f.root.join("repair"),
        &Mutex::new(crate::decode::DiskCache::new(
            f.root.join("repair"),
            4 * 1024 * 1024,
        )),
        &persistent,
    );
    assert_eq!(counts(&a).0, 1);
    assert_eq!(counts(&b).0, 1);
    assert_eq!(counts(&b).1, 0);
    assert_ne!(
        a[0].result.as_ref().map_err(|e| e.to_string())?.rgba,
        b[0].result.as_ref().map_err(|e| e.to_string())?.rgba
    );
    Ok(())
}

#[test]
fn declared_huge_dimensions_disagree_with_jpeg_header_before_decode() -> Result<(), AssetError> {
    let fixture = Fixture::new(true)?;
    let mut job = fixture.job(0, 0)?;
    job.asset = ImageAsset::new(job.asset.id(), job.source.id(), [50_000, 50_000])?;
    let result = fixture.load(vec![job], &fixture.persistent());
    assert_eq!(result.len(), 1);
    let error = result[0]
        .result
        .as_ref()
        .err()
        .ok_or("unexpected pixel output")?;
    assert!(error.to_string().contains("dimensions disagree"));
    assert!(result[0].source_bytes < 4096);
    assert_eq!(result[0].tile_cache_write_bytes, 0);
    Ok(())
}
