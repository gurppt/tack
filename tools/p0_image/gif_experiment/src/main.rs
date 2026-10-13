//! Cost and first-image semantics experiment; not a production decoder.
use image::{ImageDecoder, Limits};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{BufReader, Read, Seek, SeekFrom},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Instant,
};
struct Counted {
    file: File,
    bytes: Arc<AtomicU64>,
}
impl Read for Counted {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        let n = self.file.read(out)?;
        let total = self.bytes.fetch_add(n as u64, Ordering::Relaxed) + n as u64;
        if total > 64 * 1024 * 1024 {
            return Err(std::io::Error::other("experiment cumulative read limit"));
        }
        Ok(n)
    }
}
impl Seek for Counted {
    fn seek(&mut self, offset: SeekFrom) -> std::io::Result<u64> {
        self.file.seek(offset)
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).ok_or("owned fixture")?;
    let started = Instant::now();
    let mut file = File::open(&path)?;
    if file.metadata()?.len() > 64 * 1024 * 1024 {
        return Err("encoded limit".into());
    }
    let mut header = [0u8; 10];
    file.read_exact(&mut header)?;
    let count = Arc::new(AtomicU64::new(10));
    let reader = BufReader::new(Counted {
        file: File::open(&path)?,
        bytes: Arc::clone(&count),
    });
    #[cfg(feature = "gif-preview")]
    let mut decoder = {
        if &header[..6] != b"GIF87a" && &header[..6] != b"GIF89a" {
            return Err("GIF signature".into());
        }
        let w = u16::from_le_bytes([header[6], header[7]]);
        let h = u16::from_le_bytes([header[8], header[9]]);
        if w == 0 || h == 0 || w > 2048 || h > 2048 {
            return Err("logical screen limit".into());
        }
        // Metadata is consumed by new BEFORE set_limits, with decoder defaults.
        image::codecs::gif::GifDecoder::new(reader)?
    };
    #[cfg(not(feature = "gif-preview"))]
    let mut decoder = image::codecs::png::PngDecoder::new(reader)?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(2048);
    limits.max_image_height = Some(2048);
    limits.max_alloc = Some(16 * 1024 * 1024);
    decoder.set_limits(limits)?;
    let (w, h) = decoder.dimensions();
    let len = decoder.total_bytes() as usize;
    let mut output = Vec::new();
    output.try_reserve_exact(len)?;
    output.resize(len, 0);
    decoder.read_image(&mut output)?;
    println!(
        "first_image {} {} bytes={} source_read={} ms={:.3} sha256={:x}",
        w,
        h,
        output.len(),
        count.load(Ordering::Relaxed),
        started.elapsed().as_secs_f64() * 1000.,
        Sha256::digest(&output)
    );
    if let Some(out) = std::env::args().nth(2) {
        image::save_buffer(out, &output, w, h, image::ColorType::Rgba8)?;
    }
    Ok(())
}
