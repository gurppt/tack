//! Worker-only adapters into the existing scaled image/thumbnail representation.
use crate::{AssetError, Decoded, NativeThumbnail};
use image::{ImageEncoder, ImageReader, Limits};
use std::{
    fs::File,
    io::{BufReader, Cursor, Read, Seek},
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
    time::UNIX_EPOCH,
};
use tack_core::SourceFingerprint;
pub(crate) const ENCODED_LIMIT: u64 = 64 * 1024 * 1024;
#[derive(Clone, Copy, PartialEq, Eq)]
enum Format {
    Jpeg,
    Png,
    Other,
}
fn sniff<R: Read + Seek>(source: &mut R) -> Result<Format, AssetError> {
    let mut first = [0; 2];
    source.read_exact(&mut first)?;
    let format = if first == [0xff, 0xd8] {
        Format::Jpeg
    } else if first == [0x89, b'P'] {
        let mut rest = [0; 6];
        source.read_exact(&mut rest)?;
        if rest == *b"NG\r\n\x1a\n" {
            Format::Png
        } else {
            Format::Other
        }
    } else {
        Format::Other
    };
    source.seek(std::io::SeekFrom::Start(0))?;
    Ok(format)
}
fn check_cancel(cancel: Option<&AtomicBool>) -> Result<(), AssetError> {
    if cancel.is_some_and(|c| c.load(Ordering::Relaxed)) {
        return Err("image derivation cancelled".into());
    }
    Ok(())
}
fn ordinary_edge(edge: u32) -> Result<(), AssetError> {
    if ![8, 16, 32, 64, 128, 512, 2048].contains(&edge) {
        return Err("tile or unsupported display edge requires bounded image streaming".into());
    }
    Ok(())
}
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
        inner: BufReader::with_capacity(8192, File::open(path)?),
        count: 0,
    };
    let format = sniff(&mut source)?;
    let size = if format == Format::Jpeg {
        let header = crate::jpeg_header::read(&mut source)?;
        if !header.ordinary() {
            header.require_streamed()?;
        }
        header.size
    } else if format == Format::Png {
        let remaining = crate::huge_image::PNG_ENCODED_LIMIT.saturating_sub(source.count);
        crate::huge_image::png_dimensions(BudgetReader::with_limit(&mut source, remaining, None))?
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
    if format == Format::Other && (size[0] == 0 || size[1] == 0 || size[0] > 6000 || size[1] > 4500)
    {
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
    if bytes.len() > 20 * 1024 * 1024 {
        return Err("overview encoded budget".into());
    }
    let mut reader = ImageReader::new(Cursor::new(bytes)).with_guessed_format()?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(2048);
    limits.max_image_height = Some(2048);
    limits.max_alloc = Some(32 * 1024 * 1024);
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
#[cfg(test)]
pub(crate) fn derive_linked_edge(
    path: &Path,
    read: &mut u64,
    edge: u32,
) -> Result<(image::RgbaImage, u32), AssetError> {
    derive_linked_edge_cancel(path, read, edge, None)
}
pub(crate) fn derive_linked_edge_cancel(
    path: &Path,
    read: &mut u64,
    edge: u32,
    cancel: Option<&AtomicBool>,
) -> Result<(image::RgbaImage, u32), AssetError> {
    check_cancel(cancel)?;
    let mut reader = CountRead {
        inner: BufReader::with_capacity(8192, File::open(path)?),
        count: 0,
    };
    let result = (|| {
        if sniff(&mut reader)? == Format::Jpeg {
            let mut header_reader =
                BudgetReader::with_limit(&mut reader, crate::jpeg_header::ENCODED_LIMIT, cancel);
            let header = crate::jpeg_header::read(&mut header_reader)?;
            if crate::huge_image::Tile::from_tag(edge).is_some() || !header.ordinary() {
                return Ok((
                    crate::jpeg_scanlines::derive(&mut header_reader, header, edge, cancel)?,
                    5,
                ));
            }
            ordinary_edge(edge)?;
            reader.seek(std::io::SeekFrom::Start(0))?;
            let mut bytes = Vec::new();
            BudgetReader::with_limit(&mut reader, ENCODED_LIMIT + 1, cancel)
                .take(ENCODED_LIMIT + 1)
                .read_to_end(&mut bytes)?;
            if bytes.len() as u64 > ENCODED_LIMIT {
                return Err("JPEG exceeds 64 MiB decode input budget".into());
            }
            check_cancel(cancel)?;
            let pixels = NativeThumbnail::with_edge(&bytes, edge)?.decode()?;
            check_cancel(cancel)?;
            let bounded_edge = edge.min(pixels.width().max(pixels.height()));
            let pixels = image::DynamicImage::ImageRgb8(pixels)
                .thumbnail(bounded_edge, bounded_edge)
                .into_rgba8();
            check_cancel(cancel)?;
            return Ok((pixels, 7));
        }
        let remaining = crate::huge_image::PNG_ENCODED_LIMIT.saturating_sub(reader.count);
        derive_stream_edge_cancel(
            &mut BudgetReader::with_limit(&mut reader, remaining, cancel),
            edge,
            cancel,
        )
    })();
    *read = read
        .checked_add(reader.count)
        .ok_or("source read byte counter overflow")?;
    result
}
/// A provider may coalesce adjacent raster tiles; consumers use the same pixels
/// and tile keys regardless of the source codec. All work remains off-thread.
pub(crate) fn derive_linked_tiles_cancel(
    path: &Path,
    read: &mut u64,
    edges: &[u32],
    expected_size: [u32; 2],
    cancel: Option<&AtomicBool>,
) -> Result<Vec<image::RgbaImage>, AssetError> {
    let mut reader = CountRead {
        inner: BufReader::with_capacity(8192, File::open(path)?),
        count: 0,
    };
    let result = derive_stream_tiles_cancel(&mut reader, edges, expected_size, cancel);
    *read = read
        .checked_add(reader.count)
        .ok_or("source read counter overflow")?;
    result
}

pub(crate) fn derive_stream_tiles_cancel<R: Read + Seek + Send>(
    source: &mut R,
    edges: &[u32],
    expected_size: [u32; 2],
    cancel: Option<&AtomicBool>,
) -> Result<Vec<image::RgbaImage>, AssetError> {
    check_cancel(cancel)?;
    if edges.is_empty() || edges.len() > crate::jpeg_scanlines::MAX_BATCH_TILES {
        return Err("raster tile batch exceeds bounded admission".into());
    }
    if sniff(source)? == Format::Jpeg {
        let mut reader =
            BudgetReader::with_limit(source, crate::jpeg_header::ENCODED_LIMIT, cancel);
        let header = crate::jpeg_header::read(&mut reader)?;
        if header.size != expected_size {
            return Err("raster dimensions disagree with declared source; verify or relink".into());
        }
        // Coarse requests whose native stride footprint exceeds the regional
        // ceiling retain the existing bounded single-tile provider.
        if edges.len() == 1 && !crate::jpeg_scanlines::admissible_tiles(header.size, edges) {
            return Ok(vec![crate::jpeg_scanlines::derive(
                &mut reader,
                header,
                edges[0],
                cancel,
            )?]);
        }
        return crate::jpeg_scanlines::derive_tiles(&mut reader, header, edges, cancel);
    }
    if edges.len() != 1 {
        return Err("this raster provider admits one tile per region".into());
    }
    Ok(vec![derive_stream_edge_cancel(source, edges[0], cancel)?.0])
}
#[cfg(test)]
pub(crate) fn derive_stream<R: Read + Seek + Send>(
    source: &mut R,
) -> Result<image::RgbaImage, AssetError> {
    derive_stream_edge(source, 128)
}
#[cfg(test)]
pub(crate) fn derive_stream_edge<R: Read + Seek + Send>(
    source: &mut R,
    edge: u32,
) -> Result<image::RgbaImage, AssetError> {
    Ok(derive_stream_edge_cancel(source, edge, None)?.0)
}
pub(crate) fn derive_stream_edge_cancel<R: Read + Seek + Send>(
    source: &mut R,
    edge: u32,
    cancel: Option<&AtomicBool>,
) -> Result<(image::RgbaImage, u32), AssetError> {
    check_cancel(cancel)?;
    // One budget spans probing, metadata and decode, even after seeking back.
    let mut source = BufReader::with_capacity(
        8192,
        BudgetReader::with_limit(source, crate::huge_image::PNG_ENCODED_LIMIT, cancel),
    );
    let format = sniff(&mut source)?;
    if format == Format::Png {
        let (_size, precision_streamed) = crate::huge_image::png_layout(&mut source)?;
        source.seek(std::io::SeekFrom::Start(0))?;
        let streamed = crate::huge_image::Tile::from_tag(edge).is_some() || precision_streamed;
        if streamed {
            return Ok((crate::huge_image::derive(&mut source, edge, cancel)?, 3));
        }
        ordinary_edge(edge)?;
        return Ok((
            crate::huge_image::derive_normal(&mut source, edge, cancel)?,
            6,
        ));
    }
    if format == Format::Jpeg {
        let header = crate::jpeg_header::read(&mut source)?;
        if crate::huge_image::Tile::from_tag(edge).is_some() || !header.ordinary() {
            return Ok((
                crate::jpeg_scanlines::derive(&mut source, header, edge, cancel)?,
                5,
            ));
        }
        ordinary_edge(edge)?;
        source.seek(std::io::SeekFrom::Start(0))?;
        let mut bytes = Vec::new();
        (&mut source)
            .take(ENCODED_LIMIT + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 > ENCODED_LIMIT {
            return Err("JPEG exceeds 64 MiB decode input budget".into());
        }
        check_cancel(cancel)?;
        let decoded = NativeThumbnail::with_edge(&bytes, edge)?.decode()?;
        let bounded_edge = edge.min(decoded.width().max(decoded.height()));
        let pixels = image::DynamicImage::ImageRgb8(decoded)
            .thumbnail(bounded_edge, bounded_edge)
            .into_rgba8();
        check_cancel(cancel)?;
        return Ok((pixels, 7));
    }
    ordinary_edge(edge)?;
    let mut reader =
        ImageReader::new(BufReader::new(BudgetReader::new(&mut source))).with_guessed_format()?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(6000);
    limits.max_image_height = Some(4500);
    limits.max_alloc = Some(192 * 1024 * 1024);
    reader.limits(limits);
    let decoded = reader.decode()?;
    let bounded_edge = edge.min(decoded.width().max(decoded.height()));
    let thumbnail = decoded.thumbnail(bounded_edge, bounded_edge);
    drop(decoded);
    let pixels = thumbnail.into_rgba8();
    check_cancel(cancel)?;
    Ok((pixels, 8))
}

/// Bound cumulative parser reads including repeated seeks; storage originals can be larger.
struct BudgetReader<'a, R> {
    inner: R,
    remaining: u64,
    cancel: Option<&'a AtomicBool>,
}
impl<'a, R> BudgetReader<'a, R> {
    fn new(inner: R) -> Self {
        Self::with_limit(inner, ENCODED_LIMIT, None)
    }
    fn with_limit(inner: R, remaining: u64, cancel: Option<&'a AtomicBool>) -> Self {
        Self {
            inner,
            remaining,
            cancel,
        }
    }
}
impl<R: Read> Read for BudgetReader<'_, R> {
    fn read(&mut self, b: &mut [u8]) -> std::io::Result<usize> {
        if self.cancel.is_some_and(|c| c.load(Ordering::Relaxed)) {
            return Err(std::io::Error::other("image derivation cancelled"));
        }
        if b.is_empty() {
            return Ok(0);
        }
        if self.remaining == 0 {
            return Err(std::io::Error::other("image parser read budget exhausted"));
        }
        let nmax = b.len().min(self.remaining as usize);
        let n = self.inner.read(&mut b[..nmax])?;
        self.remaining -= n as u64;
        Ok(n)
    }
}
impl<R: Seek> Seek for BudgetReader<'_, R> {
    fn seek(&mut self, p: std::io::SeekFrom) -> std::io::Result<u64> {
        self.inner.seek(p)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::io::Write;

    fn gray_rows(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut bytes, width, height);
            encoder.set_color(png::ColorType::Grayscale);
            let mut writer = encoder.write_header().unwrap();
            {
                let mut stream = writer.stream_writer().unwrap();
                let row = vec![37; width as usize];
                for _ in 0..height {
                    stream.write_all(&row).unwrap();
                }
                stream.finish().unwrap();
            }
            writer.finish().unwrap();
        }
        bytes
    }
    fn fixture(bytes: &[u8]) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "tack-representation-{:032x}.png",
            tack_storage::new_document_id().unwrap().value()
        ));
        std::fs::write(&path, bytes).unwrap();
        path
    }

    #[test]
    fn png_routes_preserve_normal_generator_and_stream_large_huge_and_tiles() {
        let image = image::RgbaImage::from_pixel(13, 7, image::Rgba([10, 20, 30, 117]));
        let bytes = encode(&image).unwrap();
        let (normal, generator) =
            derive_stream_edge_cancel(&mut Cursor::new(&bytes), 128, None).unwrap();
        assert_eq!(generator, 6);
        // Native-sized sources are never enlarged to satisfy a nominal LOD tier.
        assert_eq!(normal.dimensions(), (13, 7));
        assert_eq!(normal.get_pixel(0, 0).0, [10, 20, 30, 117]);
        let tag = crate::huge_image::Tile { mip: 0, x: 0, y: 0 }
            .tag()
            .unwrap();
        let (tile, generator) =
            derive_stream_edge_cancel(&mut Cursor::new(&bytes), tag, None).unwrap();
        assert_eq!(generator, 3);
        assert_eq!(tile.dimensions(), image.dimensions());
        assert!(tile.as_raw() == image.as_raw());
        for (width, height) in [(2048, 2049), (8192, 1025), (7000, 1)] {
            let bytes = gray_rows(width, height);
            let (output, generator) =
                derive_stream_edge_cancel(&mut Cursor::new(bytes), 128, None).unwrap();
            assert_eq!(generator, 3);
            assert!(output.width() <= 128 && output.height() <= 128);
            assert_eq!(output.get_pixel(0, 0).0, [37, 37, 37, 255]);
        }
    }

    #[test]
    fn linked_png_and_metadata_use_streaming_for_wide_sources() {
        let bytes = gray_rows(7000, 1);
        let path = fixture(&bytes);
        let metadata = image_metadata(&path);
        let mut read = 0;
        let result = derive_linked_edge(&path, &mut read, 128);
        std::fs::remove_file(path).unwrap();
        assert_eq!(metadata.unwrap().0, [7000, 1]);
        let (image, generator) = result.unwrap();
        assert_eq!(generator, 3);
        assert_eq!(image.dimensions(), (128, 1));
        assert!(read <= crate::huge_image::PNG_ENCODED_LIMIT);
    }

    #[test]
    fn malformed_jpeg_tile_rejection_precedes_native_spawn_and_scale_cast() {
        let bytes = [0xff, 0xd8, 0xff, 0xd9];
        let tag = crate::huge_image::Tile { mip: 0, x: 0, y: 0 }
            .tag()
            .unwrap();
        let mut counted = CountRead {
            inner: Cursor::new(bytes),
            count: 0,
        };
        let error = derive_stream_edge_cancel(&mut counted, tag, None).unwrap_err();
        assert!(error.to_string().contains("JPEG"));
        assert!(counted.count <= 16);
        let path = fixture(&bytes);
        let mut read = 0;
        let result = derive_linked_edge_cancel(&path, &mut read, tag, None);
        let mut overflow = u64::MAX;
        let overflow_result = derive_linked_edge_cancel(&path, &mut overflow, tag, None);
        std::fs::remove_file(path).unwrap();
        assert!(result.unwrap_err().to_string().contains("JPEG"));
        assert!(read <= 16);
        assert!(
            overflow_result
                .unwrap_err()
                .to_string()
                .contains("counter overflow")
        );
        assert_eq!(overflow, u64::MAX);
    }

    #[test]
    fn cancellation_and_non_png_magic_do_not_enter_png_derivation() {
        let bytes = gray_rows(3, 2);
        let cancel = AtomicBool::new(true);
        let mut source = Cursor::new(&bytes);
        assert!(derive_stream_edge_cancel(&mut source, 128, Some(&cancel)).is_err());
        assert_eq!(source.position(), 0);
        let mut fake = Cursor::new(b"\x89Pgarbage".as_slice());
        assert!(sniff(&mut fake).unwrap() == Format::Other);
        assert!(derive_stream_edge_cancel(&mut fake, 128, None).is_err());
        let mut budget = BudgetReader::with_limit(Cursor::new([1; 10]), 3, None);
        let mut block = [0; 2];
        budget.read_exact(&mut block).unwrap();
        budget.seek(std::io::SeekFrom::Start(0)).unwrap();
        assert!(budget.read_exact(&mut block).is_err());
    }
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
        assert!(reader.bytes <= crate::jpeg_header::HEADER_LIMIT as u64 + 2);
        assert!(reader.bytes > crate::jpeg_header::HEADER_LIMIT as u64 - 65537);
    }
    #[test]
    fn sixteen_bit_working_risk_routes_to_rows_while_rgb8_keeps_normal_filtering() {
        for (color, depth, channels, expected) in [
            (png::ColorType::Rgba, png::BitDepth::Sixteen, 8usize, 3),
            (png::ColorType::Rgb, png::BitDepth::Eight, 3usize, 6),
        ] {
            let mut bytes = Vec::new();
            {
                let mut encoder = png::Encoder::new(&mut bytes, 2048, 2048);
                encoder.set_color(color);
                encoder.set_depth(depth);
                let mut writer = encoder.write_header().unwrap();
                {
                    let mut stream = writer.stream_writer().unwrap();
                    let row = vec![127; 2048 * channels];
                    for _ in 0..2048 {
                        stream.write_all(&row).unwrap();
                    }
                    stream.finish().unwrap();
                }
                writer.finish().unwrap();
            }
            let (output, generator) =
                derive_stream_edge_cancel(&mut Cursor::new(&bytes), 128, None).unwrap();
            assert_eq!(generator, expected);
            assert_eq!(output.dimensions(), (128, 128));
            assert_eq!(output.get_pixel(0, 0)[0], 127);
            assert!(output.as_raw().len() <= 128 * 128 * 4);
        }
    }
}
