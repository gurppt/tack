use crate::{
    AssetError, Decoded, JobProfile,
    loader::{Job, Outcome},
    profile::Stage,
};
use image::{ImageEncoder, ImageReader, Limits};
use std::{
    fs,
    io::{Cursor, Read},
    path::{Path, PathBuf},
    sync::{Arc, atomic::Ordering},
    time::Instant,
};

#[derive(Clone)]
pub(crate) struct DiskCache {
    dir: PathBuf,
    budget: usize,
    enabled: bool,
    checked: bool,
}

impl DiskCache {
    pub fn new(dir: PathBuf, budget: usize) -> Self {
        Self {
            dir,
            budget,
            enabled: true,
            checked: false,
        }
    }

    pub(crate) fn prepare(&mut self) {
        if !self.checked
            && self.enabled
            && let Err(error) = self.trim_for(0)
        {
            self.disable(error.as_ref());
        }
        self.checked = true;
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
            let thumbnail = entry.file_name().to_string_lossy().ends_with("-128.png");
            entries.push((
                thumbnail,
                metadata.modified()?,
                metadata.len(),
                entry.path(),
            ));
        }
        // Prefer evicting larger LODs over the retained overview tier.
        entries.sort_by_key(|e| (e.0, e.1));
        for (_, _, bytes, path) in entries {
            if used.saturating_add(additional as u64) <= self.budget as u64 {
                break;
            }
            fs::remove_file(path)?;
            used -= bytes;
        }
        Ok(())
    }

    fn store(
        &self,
        path: &Path,
        pixels: &image::RgbaImage,
        profile: &mut JobProfile,
        job: &Job,
    ) -> Result<(), AssetError> {
        // Encode directly to a bounded output buffer; at most one LOD per worker.
        let mut bytes = Vec::new();
        profile.measure(Stage::Encode, || {
            image::codecs::png::PngEncoder::new(&mut bytes).write_image(
                pixels.as_raw(),
                pixels.width(),
                pixels.height(),
                image::ExtendedColorType::Rgba8,
            )
        })?;
        profile.bytes(Stage::Encode, bytes.len());
        if job.cancelled.load(Ordering::Relaxed) {
            profile.cancelled_after = Some("encode");
            return Ok(());
        }
        if bytes.len() > self.budget {
            return Ok(());
        }
        profile.measure(Stage::CacheMaintenance, || self.trim_for(bytes.len()))?;
        let temporary = path.with_extension("tmp");
        profile.bytes(Stage::CacheWrite, bytes.len());
        profile.measure(Stage::CacheWrite, || fs::write(&temporary, bytes))?;
        if path.exists() {
            fs::remove_file(path)?;
        }
        fs::rename(temporary, path)?;
        Ok(())
    }
}

fn read(
    path: &Path,
    max_width: u32,
    max_height: u32,
    cached: bool,
    profile: &mut JobProfile,
) -> Result<image::DynamicImage, AssetError> {
    // JPEG's decoder reads encoded input before applying decoded-image limits.
    // The capped read protects against both oversized files and concurrent growth.
    const MAX_ENCODED_BYTES: u64 = 64 * 1024 * 1024;
    let mut bytes = Vec::new();
    let stage = if cached {
        Stage::CacheRead
    } else {
        Stage::SourceRead
    };
    profile.measure(stage, || {
        fs::File::open(path)?
            .take(MAX_ENCODED_BYTES + 1)
            .read_to_end(&mut bytes)
    })?;
    profile.bytes(stage, bytes.len());
    profile.encoded_peak_bytes = bytes.len() * 2;
    if bytes.len() as u64 > MAX_ENCODED_BYTES {
        return Err("encoded image exceeds 64 MiB".into());
    }
    let mut reader = ImageReader::new(Cursor::new(bytes)).with_guessed_format()?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(max_width);
    limits.max_image_height = Some(max_height);
    limits.max_alloc = Some(192 * 1024 * 1024);
    reader.limits(limits);
    let decoder = profile.measure(Stage::Header, || reader.into_decoder())?;
    let original = profile.measure(Stage::Decode, || image::DynamicImage::from_decoder(decoder))?;
    profile.decoded_peak_bytes = original.as_bytes().len();
    Ok(original)
}

fn read_source(
    job: &Job,
    profile: &mut JobProfile,
) -> Result<Option<image::DynamicImage>, AssetError> {
    let mut bytes = Vec::new();
    profile.measure(Stage::SourceRead, || {
        fs::File::open(&job.request.path)?
            .take(64 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
    })?;
    if bytes.len() > 64 * 1024 * 1024 {
        return Err("encoded image exceeds 64 MiB".into());
    }
    profile.bytes(Stage::SourceRead, bytes.len());
    profile.encoded_peak_bytes = bytes.len();
    if job.cancelled.load(Ordering::Relaxed) {
        profile.cancelled_after = Some("source_read");
        return Ok(None);
    }
    if job.request.key.lod == tack_core::Lod::Thumbnail {
        let decoder = profile.measure(Stage::Header, || crate::NativeThumbnail::new(&bytes))?;
        if job.cancelled.load(Ordering::Relaxed) {
            profile.cancelled_after = Some("header");
            return Ok(None);
        }
        let pixels = profile.measure(Stage::Decode, || decoder.decode())?;
        profile.decoded_peak_bytes = pixels.as_raw().len();
        profile.bytes(Stage::Decode, pixels.as_raw().len());
        return Ok(Some(image::DynamicImage::ImageRgb8(pixels)));
    }
    let mut decoder = jpeg_decoder::Decoder::new(Cursor::new(&bytes));
    decoder.set_max_decoding_buffer_size(192 * 1024 * 1024);
    profile.measure(Stage::Header, || decoder.read_info())?;
    let info = decoder.info().ok_or("JPEG header missing")?;
    if info.width == 0 || info.height == 0 || info.width > 6000 || info.height > 4500 {
        return Err("source dimensions exceed 6000x4500".into());
    }
    if !matches!(
        info.pixel_format,
        jpeg_decoder::PixelFormat::RGB24 | jpeg_decoder::PixelFormat::L8
    ) {
        return Err("prototype JPEG source must be 8-bit RGB or grayscale".into());
    }
    let edge = job.request.key.lod.edge() as u16;
    decoder.scale(edge, edge)?;
    if job.cancelled.load(Ordering::Relaxed) {
        profile.cancelled_after = Some("header");
        return Ok(None);
    }
    let pixels = profile.measure(Stage::Decode, || decoder.decode())?;
    profile.decoded_peak_bytes = pixels.len();
    profile.bytes(Stage::Decode, pixels.len());
    let info = decoder.info().ok_or("decoded JPEG metadata missing")?;
    let dimensions = (u32::from(info.width), u32::from(info.height));
    match info.pixel_format {
        jpeg_decoder::PixelFormat::RGB24 => Ok(Some(image::DynamicImage::ImageRgb8(
            image::RgbImage::from_raw(dimensions.0, dimensions.1, pixels)
                .ok_or("invalid JPEG buffer")?,
        ))),
        jpeg_decoder::PixelFormat::L8 => Ok(Some(image::DynamicImage::ImageLuma8(
            image::GrayImage::from_raw(dimensions.0, dimensions.1, pixels)
                .ok_or("invalid JPEG buffer")?,
        ))),
        _ => Err("prototype JPEG source must be 8-bit RGB or grayscale".into()),
    }
}

pub(crate) fn run(job: &Job, cache: &mut DiskCache) -> Outcome {
    let start = Instant::now();
    let mut disk_hit = false;
    let mut profile = JobProfile {
        asset_id: job.request.key.id,
        edge: job.request.key.lod.edge(),
        ..Default::default()
    };
    profile.stage_ms[Stage::QueueWait as usize] = job.queued.elapsed().as_secs_f64() * 1000.0;
    let result = (|| -> Result<Option<Decoded>, AssetError> {
        if job.cancelled.load(Ordering::Relaxed) {
            profile.cancelled_after = Some("queue");
            return Ok(None);
        }
        profile.measure(Stage::CacheMaintenance, || cache.prepare());
        let edge = job.request.key.lod.edge();
        let path = cache.dir.join(format!(
            "{}-{}-{edge}.png",
            job.request.key.id, job.request.source_sha256
        ));
        let cached = if cache.enabled && path.is_file() {
            match read(&path, edge, edge, true, &mut profile) {
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
            let Some(original) = read_source(job, &mut profile)? else {
                return Ok(None);
            };
            if job.cancelled.load(Ordering::Relaxed) {
                profile.cancelled_after = Some("decode");
                return Ok(None);
            }
            let pixels = profile.measure(Stage::Resize, || {
                original.thumbnail(edge, edge).into_rgba8()
            });
            profile.resize_peak_bytes = pixels.as_raw().len();
            profile.bytes(
                Stage::Resize,
                original.as_bytes().len() + pixels.as_raw().len(),
            );
            drop(original);
            if job.cancelled.load(Ordering::Relaxed) {
                profile.cancelled_after = Some("resize");
                return Ok(None);
            }
            if cache.enabled
                && let Err(error) = cache.store(&path, &pixels, &mut profile, job)
            {
                cache.disable(error.as_ref());
            }
            pixels
        };
        if job.cancelled.load(Ordering::Relaxed) {
            profile.cancelled_after.get_or_insert("write");
            return Ok(None);
        }
        Ok(Some(Decoded {
            width: pixels.width(),
            height: pixels.height(),
            rgba: pixels.into_raw(),
        }))
    })();
    profile.disk_hit = disk_hit;
    profile.active_ms = start.elapsed().as_secs_f64() * 1000.0;
    Outcome {
        key: job.request.key,
        result: result.map(|image| image.map(Arc::new)),
        disk_hit,
        decode_ms: start.elapsed().as_secs_f64() * 1000.0,
        profile,
    }
}
