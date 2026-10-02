use crate::{
    AssetError, Decoded,
    loader::{Job, Outcome},
};
use image::{ImageEncoder, ImageReader, Limits};
use std::{
    fs,
    io::{Cursor, Read},
    path::{Path, PathBuf},
    sync::atomic::Ordering,
    time::Instant,
};

pub(crate) struct DiskCache {
    dir: PathBuf,
    budget: usize,
    enabled: bool,
}

impl DiskCache {
    pub fn new(dir: PathBuf, budget: usize) -> Self {
        Self {
            dir,
            budget,
            enabled: true,
        }
    }

    fn disable(&mut self, error: &dyn std::fmt::Display) {
        tracing::warn!(%error, "display disk cache disabled; decoding source remains available");
        self.enabled = false;
    }

    fn trim_for(&self, additional: usize) -> Result<(), AssetError> {
        fs::create_dir_all(&self.dir)?;
        let mut entries = Vec::new();
        let mut used = 0_u64;
        for entry in fs::read_dir(&self.dir)? {
            let entry = entry?;
            let metadata = entry.metadata()?;
            if !metadata.is_file() {
                continue;
            }
            used += metadata.len();
            entries.push((metadata.modified()?, metadata.len(), entry.path()));
        }
        entries.sort_by_key(|e| e.0);
        for (_, bytes, path) in entries {
            if used.saturating_add(additional as u64) <= self.budget as u64 {
                break;
            }
            fs::remove_file(path)?;
            used -= bytes;
        }
        Ok(())
    }

    fn store(&self, path: &Path, pixels: &image::RgbaImage) -> Result<(), AssetError> {
        // Encode directly to a bounded output buffer; at most one LOD per worker.
        let mut bytes = Vec::new();
        image::codecs::png::PngEncoder::new(&mut bytes).write_image(
            pixels.as_raw(),
            pixels.width(),
            pixels.height(),
            image::ExtendedColorType::Rgba8,
        )?;
        if bytes.len() > self.budget {
            return Ok(());
        }
        self.trim_for(bytes.len())?;
        let temporary = path.with_extension("tmp");
        fs::write(&temporary, bytes)?;
        if path.exists() {
            fs::remove_file(path)?;
        }
        fs::rename(temporary, path)?;
        Ok(())
    }
}

fn read(path: &Path, max_width: u32, max_height: u32) -> Result<image::DynamicImage, AssetError> {
    // JPEG's decoder reads encoded input before applying decoded-image limits.
    // The capped read protects against both oversized files and concurrent growth.
    const MAX_ENCODED_BYTES: u64 = 64 * 1024 * 1024;
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(MAX_ENCODED_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_ENCODED_BYTES {
        return Err("encoded image exceeds 64 MiB".into());
    }
    let mut reader = ImageReader::new(Cursor::new(bytes)).with_guessed_format()?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(max_width);
    limits.max_image_height = Some(max_height);
    limits.max_alloc = Some(192 * 1024 * 1024);
    reader.limits(limits);
    Ok(reader.decode()?)
}

pub(crate) fn run(job: &Job, cache: &mut DiskCache) -> Outcome {
    let start = Instant::now();
    let mut disk_hit = false;
    let result = (|| -> Result<Option<Decoded>, AssetError> {
        if job.cancelled.load(Ordering::Relaxed) {
            return Ok(None);
        }
        if cache.enabled
            && let Err(error) = cache.trim_for(0)
        {
            cache.disable(error.as_ref());
        }
        let edge = job.request.key.lod.edge();
        let path = cache.dir.join(format!(
            "{}-{}-{edge}.png",
            job.request.key.id, job.request.source_sha256
        ));
        let cached = if cache.enabled && path.is_file() {
            match read(&path, edge, edge) {
                Ok(image) => Some(image),
                Err(_) => {
                    if let Err(error) = fs::remove_file(&path) {
                        cache.disable(&error);
                    }
                    None
                }
            }
        } else {
            None
        };
        let pixels = if let Some(image) = cached {
            disk_hit = true;
            image.into_rgba8()
        } else {
            // Decode and storage are worker-only. The codec itself is not preemptible.
            let original = read(&job.request.path, 6000, 4500)?;
            if job.cancelled.load(Ordering::Relaxed) {
                return Ok(None);
            }
            let pixels = original.thumbnail(edge, edge).into_rgba8();
            drop(original);
            if job.cancelled.load(Ordering::Relaxed) {
                return Ok(None);
            }
            if cache.enabled
                && let Err(error) = cache.store(&path, &pixels)
            {
                cache.disable(error.as_ref());
            }
            pixels
        };
        if job.cancelled.load(Ordering::Relaxed) {
            return Ok(None);
        }
        Ok(Some(Decoded {
            width: pixels.width(),
            height: pixels.height(),
            rgba: pixels.into_raw(),
        }))
    })();
    Outcome {
        key: job.request.key,
        result,
        disk_hit,
        decode_ms: start.elapsed().as_secs_f64() * 1000.0,
    }
}
