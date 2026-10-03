use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant},
};
use tack_assets::{
    AssetError, AssetKey, BenchmarkBoard, BenchmarkImage, DecodeRequest, Loader,
    OverviewPreparation,
};
use tack_core::Lod;

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Result<Self, AssetError> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "tack-pipeline-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&dir)?;
        Ok(Self(dir))
    }

    fn request(&self, id: u32) -> Result<DecodeRequest, AssetError> {
        let path = self.0.join(format!("{id}.jpg"));
        image::RgbImage::from_pixel(256, 192, image::Rgb([210, 30, 20])).save(&path)?;
        Ok(DecodeRequest {
            key: AssetKey {
                id,
                lod: Lod::Thumbnail,
            },
            path,
            source_sha256: "a".repeat(64),
        })
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn wait(loader: &mut Loader) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        loader.poll();
        if loader.stats().pending == 0 {
            break;
        }
        assert!(Instant::now() < deadline, "worker must complete or cancel");
        thread::sleep(Duration::from_millis(1));
    }
}

fn board(requests: &[DecodeRequest]) -> Result<BenchmarkBoard, AssetError> {
    Ok(BenchmarkBoard {
        objects: requests
            .iter()
            .map(|r| {
                Ok(BenchmarkImage {
                    id: r.key.id,
                    path: r.path.clone(),
                    source_sha256: r.source_sha256.clone(),
                    rect: tack_core::WorldRect::new(r.key.id as f64 * 300.0, 0.0, 256.0, 192.0)?,
                })
            })
            .collect::<Result<_, AssetError>>()?,
    })
}

fn prepare(loader: &mut Loader, prep: &mut OverviewPreparation) {
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut requests = Vec::new();
    while !prep.settled() {
        loader.poll();
        prep.observe(loader);
        prep.remaining(&mut requests);
        loader.request(&requests);
        assert!(
            Instant::now() < deadline,
            "preparation must settle despite bad assets/evictions"
        );
        thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn preparation_progress_survives_eviction_bad_asset_and_partial_restart() -> Result<(), AssetError>
{
    let fixture = Fixture::new()?;
    let requests: Vec<_> = (0..20)
        .map(|id| fixture.request(id))
        .collect::<Result<_, _>>()?;
    fs::write(&requests[19].path, b"invalid source")?;
    let board = board(&requests)?;
    let cache = fixture.0.join("cache");
    let started = Instant::now();
    let mut loader = Loader::new(cache.clone(), 128 * 128 * 4, 1024 * 1024, 2)?;
    let mut prep = OverviewPreparation::new(&board, started);
    loader.request(&requests[..8]);
    wait(&mut loader);
    prep.observe(&loader);
    assert_eq!(prep.ready(), 8);
    assert!(loader.stats().cpu_evictions > 0);
    loader.request(&requests);
    loader.request(&[]);
    wait(&mut loader);
    prep.observe(&loader);
    let persisted = prep.ready();
    assert!((8..=19).contains(&persisted));
    let mut restart = Loader::new(cache.clone(), 128 * 128 * 4, 1024 * 1024, 2)?;
    let mut prep = OverviewPreparation::new(&board, Instant::now());
    prepare(&mut restart, &mut prep);
    assert_eq!(prep.ready(), 19);
    assert_eq!(prep.report(&restart, 2)["errors"], 1);
    assert!(prep.report(&restart, 2)["threshold_ms"]["100"].is_null());
    assert_eq!(restart.stats().disk_hits, persisted as u64);
    assert_eq!(
        restart.profiles().len(),
        20,
        "ready IDs never re-decode after RAM eviction"
    );
    // Exactly two prepared entries need repair; all other valid entries reuse SSD.
    let path = |id| {
        cache
            .join(format!("worker-{}", id % 4))
            .join(format!("{id}-{}-128.png", "a".repeat(64)))
    };
    fs::remove_file(path(0))?;
    fs::write(path(1), b"corrupt")?;
    let mut repaired = Loader::new(cache, 128 * 128 * 4, 1024 * 1024, 2)?;
    let mut prep = OverviewPreparation::new(&board, Instant::now());
    prepare(&mut repaired, &mut prep);
    assert_eq!(prep.ready(), 19);
    assert_eq!(repaired.stats().disk_hits, 17);
    assert_eq!(prep.report(&repaired, 2)["source_read_attempts"], 3);
    Ok(())
}

#[test]
fn preparation_cannot_claim_persistence_when_disposable_cache_is_unavailable()
-> Result<(), AssetError> {
    let fixture = Fixture::new()?;
    let request = fixture.request(0)?;
    let board = board(std::slice::from_ref(&request))?;
    let cache = fixture.0.join("file");
    fs::write(&cache, b"keep")?;
    let mut loader = Loader::new(cache, 128 * 128 * 4, 1024 * 1024, 1)?;
    let mut prep = OverviewPreparation::new(&board, Instant::now());
    prepare(&mut loader, &mut prep);
    assert_eq!(prep.ready(), 0);
    assert_eq!(prep.report(&loader, 1)["errors"], 1);
    assert!(
        loader.has_cached(request.key),
        "ordinary RAM delivery still works"
    );
    Ok(())
}

#[test]
fn persisted_stale_result_is_prepared_without_entering_cpu_cache() -> Result<(), AssetError> {
    let fixture = Fixture::new()?;
    let request = fixture.request(0)?;
    let board = board(std::slice::from_ref(&request))?;
    let cache = fixture.0.join("cache");
    let mut loader = Loader::new(cache.clone(), 128 * 128 * 4, 1024 * 1024, 1)?;
    let mut prep = OverviewPreparation::new(&board, Instant::now());
    loader.request(std::slice::from_ref(&request));
    let final_path = cache
        .join("worker-0")
        .join(format!("0-{}-128.png", "a".repeat(64)));
    let deadline = Instant::now() + Duration::from_secs(5);
    while !final_path.exists() {
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(1));
    }
    loader.request(&[]);
    wait(&mut loader);
    prep.observe(&loader);
    assert_eq!(prep.ready(), 1);
    assert_eq!(loader.stats().cpu_bytes, 0);
    assert_eq!(loader.stats().stale, 1);
    Ok(())
}

#[test]
fn background_is_one_thumbnail_per_worker_and_shares_foreground_bounds() -> Result<(), AssetError> {
    let fixture = Fixture::new()?;
    let requests: Vec<_> = (0..30)
        .map(|id| fixture.request(id))
        .collect::<Result<_, _>>()?;
    let mut loader = Loader::new(fixture.0.join("cache"), 1024 * 1024, 1024 * 1024, 2)?;
    loader.request_with_background(&[], &requests);
    assert_eq!(loader.stats().pending, 2);
    // Promotion and overlapping lists must not enqueue a second copy.
    loader.request_with_background(&requests[..20], &requests);
    assert_eq!(loader.stats().pending, 16);
    wait(&mut loader);
    let unique: std::collections::HashSet<_> =
        loader.profiles().iter().map(|p| p.asset_id).collect();
    assert_eq!(unique.len(), 16);
    assert_eq!(loader.profiles().len(), 16);
    assert!(
        unique.iter().all(|id| *id < 20),
        "foreground admission precedes remaining board"
    );
    let mut detail = requests[29].clone();
    detail.key.lod = Lod::Detail;
    loader.request_with_background(&[], &[detail]);
    assert_eq!(
        loader.stats().pending,
        0,
        "background preparation only accepts the cheap tier"
    );
    Ok(())
}

#[test]
fn historical_ready_and_terminal_errors_remain_disjoint_after_later_failure()
-> Result<(), AssetError> {
    let fixture = Fixture::new()?;
    let requests: Vec<_> = (0..2)
        .map(|id| fixture.request(id))
        .collect::<Result<_, _>>()?;
    let board = board(&requests)?;
    let cache = fixture.0.join("cache");
    let mut loader = Loader::new(cache.clone(), 128 * 128 * 4, 1024 * 1024, 1)?;
    let mut prep = OverviewPreparation::new(&board, Instant::now());
    for request in &requests {
        loader.request(std::slice::from_ref(request));
        wait(&mut loader);
        prep.observe(&loader);
    }
    assert_eq!(prep.ready(), 2);
    assert!(prep.settled());
    assert!(!loader.has_cached(requests[0].key));
    fs::remove_file(
        cache
            .join("worker-0")
            .join(format!("0-{}-128.png", "a".repeat(64))),
    )?;
    fs::remove_file(&requests[0].path)?;
    loader.request(&requests[..1]);
    wait(&mut loader);
    prep.observe(&loader);
    assert_eq!(loader.stats().errors, 1);
    assert_eq!(
        prep.ready(),
        2,
        "historical progress is not a live inventory"
    );
    assert_eq!(prep.report(&loader, 1)["errors"], 0);
    assert!(prep.settled(), "sets must never overlap");
    Ok(())
}

#[test]
fn source_cache_warm_hit_and_corrupt_cache_recovery() -> Result<(), AssetError> {
    let fixture = Fixture::new()?;
    let request = fixture.request(0)?;
    let cache = fixture.0.join("cache");
    let mut loader = Loader::new(cache.clone(), 128 * 128 * 4, 1024 * 1024, 1)?;
    loader.request(std::slice::from_ref(&request));
    wait(&mut loader);
    assert!(loader.has_cached(request.key));
    assert_eq!(loader.stats().disk_hits, 0);
    let mut warm = Loader::new(cache.clone(), 128 * 128 * 4, 1024 * 1024, 1)?;
    warm.request(std::slice::from_ref(&request));
    wait(&mut warm);
    assert_eq!(warm.stats().disk_hits, 1);
    let path = cache
        .join("worker-0")
        .join(format!("0-{}-128.png", "a".repeat(64)));
    fs::write(path, b"broken cache")?;
    let mut repair = Loader::new(cache, 128 * 128 * 4, 1024 * 1024, 1)?;
    repair.request(std::slice::from_ref(&request));
    wait(&mut repair);
    assert!(repair.has_cached(request.key));
    assert_eq!(repair.stats().errors, 0);
    Ok(())
}

#[test]
fn unavailable_disposable_cache_does_not_hide_valid_images() -> Result<(), AssetError> {
    let fixture = Fixture::new()?;
    let request = fixture.request(0)?;
    let cache = fixture.0.join("file-instead-of-directory");
    fs::write(&cache, b"keep")?;
    let mut loader = Loader::new(cache.clone(), 128 * 128 * 4, 1024 * 1024, 1)?;
    loader.request(std::slice::from_ref(&request));
    wait(&mut loader);
    assert!(loader.has_cached(request.key));
    assert_eq!(loader.stats().errors, 0);
    assert_eq!(fs::read(cache)?, b"keep");
    Ok(())
}

#[test]
fn obsolete_results_never_enter_cpu_cache_and_jobs_remain_bounded() -> Result<(), AssetError> {
    let fixture = Fixture::new()?;
    let requests: Vec<_> = (0..20)
        .map(|id| fixture.request(id))
        .collect::<Result<_, _>>()?;
    let mut loader = Loader::new(fixture.0.join("cache"), 128 * 128 * 4, 1024 * 1024, 2)?;
    loader.request(&requests);
    let pending = loader.stats().pending;
    assert!(pending <= 2 * tack_assets::MAX_PENDING_PER_WORKER);
    loader.request(&[]);
    wait(&mut loader);
    assert_eq!(loader.stats().cpu_bytes, 0);
    assert_eq!(loader.stats().stale, pending as u64);
    Ok(())
}

#[test]
fn oversized_encoded_input_is_rejected_without_unbounded_read() -> Result<(), AssetError> {
    let fixture = Fixture::new()?;
    let request = fixture.request(0)?;
    fs::OpenOptions::new()
        .write(true)
        .open(&request.path)?
        .set_len(64 * 1024 * 1024 + 1)?;
    let mut loader = Loader::new(fixture.0.join("cache"), 128 * 128 * 4, 1024 * 1024, 1)?;
    loader.request(std::slice::from_ref(&request));
    wait(&mut loader);
    assert!(!loader.has_cached(request.key));
    assert_eq!(loader.stats().errors, 1);
    assert_eq!(loader.profiles()[0].stage_bytes[0], 64 * 1024 * 1024 + 1);
    Ok(())
}

#[test]
fn malformed_source_is_suppressed_and_worker_continues() -> Result<(), AssetError> {
    let fixture = Fixture::new()?;
    let bad = fixture.request(0)?;
    fs::write(&bad.path, [0xff, 0xd8, 0xff, 0xc0, 0, 1])?;
    let good = fixture.request(1)?;
    let mut loader = Loader::new(fixture.0.join("cache"), 1024 * 1024, 1024 * 1024, 1)?;
    loader.request(&[bad.clone(), good.clone()]);
    wait(&mut loader);
    assert_eq!(loader.stats().errors, 1);
    assert!(!loader.has_cached(bad.key));
    assert!(loader.has_cached(good.key));
    loader.request(&[bad, good]);
    loader.poll();
    assert_eq!(loader.stats().pending, 0);
    assert_eq!(loader.stats().errors, 1);
    Ok(())
}

#[test]
fn too_large_refinement_is_not_retried_and_thumbnail_survives() -> Result<(), AssetError> {
    let fixture = Fixture::new()?;
    let thumbnail = fixture.request(0)?;
    let mut loader = Loader::new(fixture.0.join("cache"), 64 * 1024, 1024 * 1024, 1)?;
    loader.request(std::slice::from_ref(&thumbnail));
    wait(&mut loader);
    let mut detail = thumbnail.clone();
    detail.key.lod = Lod::Detail;
    loader.request(std::slice::from_ref(&detail));
    wait(&mut loader);
    assert!(loader.has_cached(thumbnail.key));
    assert!(!loader.has_cached(detail.key));
    assert_eq!(loader.stats().rejected, 1);
    for _ in 0..10 {
        loader.request(std::slice::from_ref(&detail));
        loader.poll();
        assert_eq!(loader.stats().pending, 0);
    }
    Ok(())
}

#[test]
fn cache_identity_survives_worker_count_changes_with_global_quota() -> Result<(), AssetError> {
    let fixture = Fixture::new()?;
    let requests: Vec<_> = (0..8)
        .map(|id| fixture.request(id))
        .collect::<Result<_, _>>()?;
    let cache = fixture.0.join("cache");
    let budget = 16 * 1024;
    for workers in [4, 1, 2] {
        let mut loader = Loader::new(cache.clone(), 1024 * 1024, budget, workers)?;
        loader.request(&requests);
        wait(&mut loader);
        assert_eq!(loader.stats().completed, 8);
        if workers != 4 {
            assert_eq!(loader.stats().disk_hits, 8);
        }
        let mut bytes = 0;
        for shard in fs::read_dir(&cache)? {
            for entry in fs::read_dir(shard?.path())? {
                bytes += entry?.metadata()?.len();
            }
        }
        assert!(bytes <= budget as u64);
    }
    Ok(())
}
