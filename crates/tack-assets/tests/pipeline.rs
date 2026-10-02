use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant},
};
use tack_assets::{AssetError, AssetKey, DecodeRequest, Loader};
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
    assert_eq!(loader.stats().pending, 2);
    loader.request(&[]);
    wait(&mut loader);
    assert_eq!(loader.stats().cpu_bytes, 0);
    assert_eq!(loader.stats().stale, 2);
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
    Ok(())
}
