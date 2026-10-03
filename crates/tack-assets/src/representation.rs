//! Worker-only adapters into the existing scaled image/thumbnail representation.
use crate::{AssetError, Decoded, NativeThumbnail};
use image::{ImageEncoder, ImageReader, Limits};
use std::{
    fs::File,
    io::{BufReader, Cursor, Read, Seek},
    path::Path,
    time::UNIX_EPOCH,
};
use tack_core::SourceFingerprint;
pub(crate) const ENCODED_LIMIT: u64 = 64 * 1024 * 1024;
pub fn source_fingerprint(path: &Path) -> Result<SourceFingerprint, AssetError> {
    let m = std::fs::metadata(path)?;
    if !m.is_file() {
        return Err("source is not a regular file".into());
    }
    let (modified_seconds, modified_nanos) = match m.modified()?.duration_since(UNIX_EPOCH) {
        Ok(t) => (i64::try_from(t.as_secs())?, t.subsec_nanos()),
        Err(e) => {
            let d = e.duration();
            let seconds = i64::try_from(d.as_secs())?;
            if d.subsec_nanos() == 0 {
                (-seconds, 0)
            } else {
                (-seconds - 1, 1_000_000_000 - d.subsec_nanos())
            }
        }
    };
    Ok(SourceFingerprint {
        size: m.len(),
        modified_seconds,
        modified_nanos,
    })
}
pub fn image_metadata(path: &Path) -> Result<([u32; 2], u64), AssetError> {
    let mut source = CountRead {
        inner: File::open(path)?,
        count: 0,
    };
    let mut magic = [0; 2];
    source.read_exact(&mut magic)?;
    source.seek(std::io::SeekFrom::Start(0))?;
    let size = if magic == [0xff, 0xd8] {
        let mut d = jpeg_decoder::Decoder::new((&mut source).take(ENCODED_LIMIT));
        d.set_max_decoding_buffer_size(192 * 1024 * 1024);
        d.read_info()?;
        let info = d.info().ok_or("JPEG metadata absent")?;
        [u32::from(info.width), u32::from(info.height)]
    } else {
        let mut reader = ImageReader::new(BufReader::new(BudgetReader::new(&mut source)))
            .with_guessed_format()?;
        let mut limits = Limits::default();
        limits.max_image_width = Some(6000);
        limits.max_image_height = Some(4500);
        limits.max_alloc = Some(192 * 1024 * 1024);
        reader.limits(limits);
        let (w, h) = reader.into_dimensions()?;
        [w, h]
    };
    if size[0] == 0 || size[1] == 0 || size[0] > 6000 || size[1] > 4500 {
        return Err("supported image dimensions are 1..6000 x 1..4500".into());
    }
    Ok((size, source.count))
}
pub(crate) struct CountRead<R> {
    pub inner: R,
    pub count: u64,
}
impl<R: Read> Read for CountRead<R> {
    fn read(&mut self, b: &mut [u8]) -> std::io::Result<usize> {
        let n = self.inner.read(b)?;
        self.count += n as u64;
        Ok(n)
    }
}
impl<R: Seek> Seek for CountRead<R> {
    fn seek(&mut self, p: std::io::SeekFrom) -> std::io::Result<u64> {
        self.inner.seek(p)
    }
}
pub(crate) fn decode_png(bytes: &[u8], size: Option<[u32; 2]>) -> Result<Decoded, AssetError> {
    if bytes.len() > 1024 * 1024 {
        return Err("overview encoded budget".into());
    }
    let mut reader = ImageReader::new(Cursor::new(bytes)).with_guessed_format()?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(512);
    limits.max_image_height = Some(512);
    limits.max_alloc = Some(4 * 1024 * 1024);
    reader.limits(limits);
    let pixels = reader.decode()?.into_rgba8();
    if size.is_some_and(|s| s != [pixels.width(), pixels.height()]) {
        return Err("overview dimensions mismatch".into());
    }
    Ok(Decoded {
        width: pixels.width(),
        height: pixels.height(),
        rgba: pixels.into_raw(),
    })
}
pub(crate) fn encode(pixels: &image::RgbaImage) -> Result<Vec<u8>, AssetError> {
    let mut bytes = Vec::new();
    image::codecs::png::PngEncoder::new(&mut bytes).write_image(
        pixels.as_raw(),
        pixels.width(),
        pixels.height(),
        image::ExtendedColorType::Rgba8,
    )?;
    Ok(bytes)
}
pub(crate) fn derive_linked(
    path: &Path,
    read: &mut u64,
) -> Result<(image::RgbaImage, u32), AssetError> {
    let mut reader = CountRead {
        inner: File::open(path)?,
        count: 0,
    };
    let result = (|| {
        let mut magic = [0; 2];
        reader.read_exact(&mut magic)?;
        reader.seek(std::io::SeekFrom::Start(0))?;
        if magic == [0xff, 0xd8] {
            let mut bytes = Vec::new();
            (&mut reader)
                .take(ENCODED_LIMIT + 1)
                .read_to_end(&mut bytes)?;
            if bytes.len() as u64 > ENCODED_LIMIT {
                return Err("JPEG exceeds 64 MiB decode input budget".into());
            }
            let pixels = NativeThumbnail::new(&bytes)?.decode()?;
            return Ok((
                image::DynamicImage::ImageRgb8(pixels)
                    .thumbnail(128, 128)
                    .into_rgba8(),
                1,
            ));
        }
        Ok((derive_stream(&mut reader)?, 2))
    })();
    *read += reader.count;
    result
}
pub(crate) fn derive_stream<R: Read + Seek>(
    source: &mut R,
) -> Result<image::RgbaImage, AssetError> {
    let mut magic = [0; 2];
    source.read_exact(&mut magic)?;
    source.seek(std::io::SeekFrom::Start(0))?;
    if magic == [0xff, 0xd8] {
        // Read budget also bounds hostile ICC/APP marker storage, not only output pixels.
        let mut d = jpeg_decoder::Decoder::new(source.take(ENCODED_LIMIT));
        d.set_max_decoding_buffer_size(192 * 1024 * 1024);
        d.read_info()?;
        let info = d.info().ok_or("JPEG metadata absent")?;
        if info.width == 0 || info.height == 0 || info.width > 6000 || info.height > 4500 {
            return Err("source dimensions exceed 6000x4500".into());
        }
        if !matches!(
            info.pixel_format,
            jpeg_decoder::PixelFormat::RGB24 | jpeg_decoder::PixelFormat::L8
        ) {
            return Err("unsupported JPEG pixel format".into());
        }
        d.scale(128, 128)?;
        let bytes = d.decode()?;
        let info = d.info().ok_or("JPEG metadata absent")?;
        let (w, h) = (u32::from(info.width), u32::from(info.height));
        let pixels = match info.pixel_format {
            jpeg_decoder::PixelFormat::RGB24 => image::DynamicImage::ImageRgb8(
                image::RgbImage::from_raw(w, h, bytes).ok_or("JPEG RGB output")?,
            ),
            jpeg_decoder::PixelFormat::L8 => image::DynamicImage::ImageLuma8(
                image::GrayImage::from_raw(w, h, bytes).ok_or("JPEG grayscale output")?,
            ),
            _ => return Err("JPEG pixel format".into()),
        };
        return Ok(pixels.thumbnail(128, 128).into_rgba8());
    }
    let mut reader =
        ImageReader::new(BufReader::new(BudgetReader::new(source))).with_guessed_format()?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(6000);
    limits.max_image_height = Some(4500);
    limits.max_alloc = Some(192 * 1024 * 1024);
    reader.limits(limits);
    Ok(reader.decode()?.thumbnail(128, 128).into_rgba8())
}

/// Bound cumulative parser reads including repeated seeks; storage originals can be larger.
struct BudgetReader<R> {
    inner: R,
    remaining: u64,
}
impl<R> BudgetReader<R> {
    fn new(inner: R) -> Self {
        Self {
            inner,
            remaining: ENCODED_LIMIT,
        }
    }
}
impl<R: Read> Read for BudgetReader<R> {
    fn read(&mut self, b: &mut [u8]) -> std::io::Result<usize> {
        if self.remaining == 0 {
            return Err(std::io::Error::other("image parser read budget exhausted"));
        }
        let nmax = b.len().min(self.remaining as usize);
        let n = self.inner.read(&mut b[..nmax])?;
        self.remaining -= n as u64;
        Ok(n)
    }
}
impl<R: Seek> Seek for BudgetReader<R> {
    fn seek(&mut self, p: std::io::SeekFrom) -> std::io::Result<u64> {
        self.inner.seek(p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct MarkerStream {
        pos: u64,
        bytes: u64,
        block: Vec<u8>,
    }
    impl Read for MarkerStream {
        fn read(&mut self, b: &mut [u8]) -> std::io::Result<usize> {
            let n = b
                .len()
                .min((96 * 1024 * 1024u64).saturating_sub(self.pos) as usize);
            for value in &mut b[..n] {
                *value = if self.pos < 2 {
                    [0xff, 0xd8][self.pos as usize]
                } else {
                    self.block[((self.pos - 2) % self.block.len() as u64) as usize]
                };
                self.pos += 1;
            }
            self.bytes += n as u64;
            Ok(n)
        }
    }
    impl Seek for MarkerStream {
        fn seek(&mut self, p: std::io::SeekFrom) -> std::io::Result<u64> {
            match p {
                std::io::SeekFrom::Start(n) => self.pos = n,
                _ => return Err(std::io::Error::other("test seek")),
            }
            Ok(self.pos)
        }
    }
    #[test]
    fn embedded_jpeg_parser_has_encoded_work_budget_for_repeated_icc_markers() {
        let mut block = vec![0; 65537];
        block[..4].copy_from_slice(&[0xff, 0xe2, 0xff, 0xff]);
        block[4..16].copy_from_slice(b"ICC_PROFILE\0");
        block[16] = 1;
        block[17] = 1;
        let mut reader = MarkerStream {
            pos: 0,
            bytes: 0,
            block,
        };
        assert!(derive_stream(&mut reader).is_err());
        assert!(reader.bytes <= ENCODED_LIMIT + 2);
        assert!(reader.bytes > ENCODED_LIMIT - 65537);
    }
}
