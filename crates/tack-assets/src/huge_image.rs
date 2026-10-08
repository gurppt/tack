//! Bounded static PNG row prototype. Tiles are disposable source/revision products.
use crate::AssetError;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::sync::atomic::{AtomicBool, Ordering};

pub const TILE_EDGE: u32 = 256;
pub const MONOLITHIC_WORKING_BYTES: u64 = 32 * 1024 * 1024;
pub const MAX_ROW_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_PIXELS: u64 = 4_294_967_295;
/// Cumulative source reads, including repeated seeks; encoded input is never buffered whole.
pub const PNG_ENCODED_LIMIT: u64 = 256 * 1024 * 1024;
const MAX_AXIS: u32 = 262_144;
const PNG_INTERNAL_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageClass {
    Normal,
    Large,
    HugeTiled,
}

pub fn decoded_bytes(size: [u32; 2]) -> Result<u64, AssetError> {
    let pixels = u64::from(size[0])
        .checked_mul(u64::from(size[1]))
        .ok_or("pixel count overflow")?;
    if size.contains(&0) || size.into_iter().any(|v| v > MAX_AXIS) || pixels > MAX_PIXELS {
        return Err("image dimensions exceed bounded streamed policy".into());
    }
    pixels
        .checked_mul(4)
        .ok_or_else(|| "decoded byte overflow".into())
}
pub fn classify_png(size: [u32; 2]) -> Result<ImageClass, AssetError> {
    let bytes = decoded_bytes(size)?;
    Ok(
        if bytes.checked_mul(2).ok_or("working estimate overflow")? <= MONOLITHIC_WORKING_BYTES {
            ImageClass::Normal
        } else if bytes <= MONOLITHIC_WORKING_BYTES {
            ImageClass::Large
        } else {
            ImageClass::HugeTiled
        },
    )
}
pub(crate) fn requires_streaming(size: [u32; 2]) -> Result<bool, AssetError> {
    Ok(classify_png(size)? != ImageClass::Normal || size[0] > 6000 || size[1] > 4500)
}

/// Encoded into the existing representation edge identity, never a texture size.
/// The marker is outside all ordinary 8..2048 edge values.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Tile {
    pub mip: u8,
    pub x: u32,
    pub y: u32,
}
impl Tile {
    pub fn tag(self) -> Result<u32, AssetError> {
        if self.mip > 31 || self.x >= 8192 || self.y >= 8192 {
            return Err("tile address overflow".into());
        }
        Ok(0x8000_0000 | (u32::from(self.mip) << 26) | (self.x << 13) | self.y)
    }
    pub fn from_tag(tag: u32) -> Option<Self> {
        (tag & 0x8000_0000 != 0).then_some(Self {
            mip: ((tag >> 26) & 31) as u8,
            x: (tag >> 13) & 8191,
            y: tag & 8191,
        })
    }
    pub fn dimensions(self, source: [u32; 2]) -> Result<[u32; 2], AssetError> {
        let size = mip_dimensions(source, self.mip)?;
        let start = [
            self.x.checked_mul(TILE_EDGE).ok_or("tile x overflow")?,
            self.y.checked_mul(TILE_EDGE).ok_or("tile y overflow")?,
        ];
        if (0..2).any(|i| start[i] >= size[i]) {
            return Err("tile outside source".into());
        }
        Ok(std::array::from_fn(|i| (size[i] - start[i]).min(TILE_EDGE)))
    }
}
pub fn mip_dimensions(source: [u32; 2], mip: u8) -> Result<[u32; 2], AssetError> {
    decoded_bytes(source)?;
    let scale = 1u64
        .checked_shl(u32::from(mip))
        .ok_or("mip shift overflow")?;
    Ok(source.map(|v| u64::from(v).div_ceil(scale) as u32))
}
pub fn mip_for_density(source: [u32; 2], projected: f64) -> Result<u8, AssetError> {
    decoded_bytes(source)?;
    if !projected.is_finite() || projected <= 0. {
        return Err("invalid projected density".into());
    }
    Ok((f64::from(source[0].max(source[1])) / projected)
        .log2()
        .floor()
        .clamp(0., 31.) as u8)
}

struct SourceReader<'a, R> {
    inner: R,
    remaining: u64,
    cancel: Option<&'a AtomicBool>,
}
impl<R: Read> Read for SourceReader<'_, R> {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        if self.cancel.is_some_and(|c| c.load(Ordering::Relaxed)) {
            return Err(std::io::Error::other("streamed decode cancelled"));
        }
        if bytes.is_empty() {
            return Ok(0);
        }
        if self.remaining == 0 {
            return Err(std::io::Error::other(
                "PNG cumulative 256 MiB read budget exhausted",
            ));
        }
        let count = bytes.len().min(self.remaining as usize);
        let read = self.inner.read(&mut bytes[..count])?;
        self.remaining -= read as u64;
        Ok(read)
    }
}
impl<R: Seek> Seek for SourceReader<'_, R> {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        self.inner.seek(position)
    }
}

fn streamed_layout(info: &png::Info<'_>) -> Result<bool, AssetError> {
    let size = [info.width, info.height];
    let source_pixel = (info.color_type.samples() as u64)
        * if info.bit_depth == png::BitDepth::Sixteen {
            2
        } else {
            1
        };
    let working = u64::from(size[0])
        .checked_mul(u64::from(size[1]))
        .and_then(|n| n.checked_mul(source_pixel.max(4)))
        .and_then(|n| n.checked_mul(2))
        .ok_or("PNG working estimate overflow")?;
    Ok(requires_streaming(size)? || working > MONOLITHIC_WORKING_BYTES)
}
fn decoder<R: Read + Seek>(
    source: R,
    streamed: bool,
    cancel: Option<&AtomicBool>,
) -> Result<png::Decoder<BufReader<SourceReader<'_, R>>>, AssetError> {
    let mut d = png::Decoder::new_with_limits(
        BufReader::with_capacity(
            8192,
            SourceReader {
                inner: source,
                remaining: PNG_ENCODED_LIMIT,
                cancel,
            },
        ),
        png::Limits {
            bytes: PNG_INTERNAL_BYTES,
        },
    );
    d.set_ignore_text_chunk(true);
    d.set_ignore_iccp_chunk(true);
    d.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let h = d.read_header_info()?;
    let needs_streaming = streamed_layout(h)?;
    let row_bits = u64::from(h.width)
        .checked_mul(h.color_type.samples() as u64)
        .and_then(|n| n.checked_mul(h.bit_depth as u64))
        .ok_or("raw row overflow")?;
    let raw_row = row_bits
        .div_ceil(8)
        .checked_add(1)
        .ok_or("raw row overflow")?;
    if raw_row > MAX_ROW_BYTES as u64 {
        return Err("PNG row exceeds streamed scratch budget".into());
    }
    if h.interlaced && (streamed || needs_streaming) {
        return Err("interlaced PNG unsupported by streamed prototype".into());
    }
    Ok(d)
}
pub fn png_dimensions<R: Read + Seek>(source: R) -> Result<[u32; 2], AssetError> {
    Ok(png_layout(source)?.0)
}
pub(crate) fn png_layout<R: Read + Seek>(source: R) -> Result<([u32; 2], bool), AssetError> {
    let reader = decoder(source, false, None)?.read_info()?;
    let info = reader.info();
    let streamed = streamed_layout(info)?;
    if streamed && info.animation_control.is_some() {
        return Err("animated large/huge PNG is outside static streamed support".into());
    }
    Ok(([info.width, info.height], streamed))
}
fn allocate(bytes: usize) -> Result<Vec<u8>, AssetError> {
    let mut v = Vec::new();
    v.try_reserve_exact(bytes)?;
    v.resize(bytes, 0);
    Ok(v)
}

/// Bounded ordinary PNG decode preserving thumbnail filtering; metadata is ignored.
/// Pixel buffers peak at 32 MiB, separately from the 8 MiB parser allowance.
pub(crate) fn derive_normal<R: Read + Seek>(
    source: R,
    edge: u32,
    cancel: Option<&AtomicBool>,
) -> Result<image::RgbaImage, AssetError> {
    let mut reader = decoder(source, false, cancel)?.read_info()?;
    if streamed_layout(reader.info())? {
        return Err("PNG requires streamed representation".into());
    }
    let bytes = reader.output_buffer_size().ok_or("PNG output size")?;
    if bytes > (MONOLITHIC_WORKING_BYTES / 2) as usize {
        return Err("normal PNG pixel budget".into());
    }
    let mut buffer = allocate(bytes)?;
    let output = reader.next_frame(&mut buffer)?;
    let (w, h, color) = (output.width, output.height, output.color_type);
    buffer.truncate(output.buffer_size());
    reader.finish()?;
    drop(reader);
    let decoded = match color {
        png::ColorType::Grayscale => image::DynamicImage::ImageLuma8(
            image::GrayImage::from_raw(w, h, buffer).ok_or("gray layout")?,
        ),
        png::ColorType::GrayscaleAlpha => image::DynamicImage::ImageLumaA8(
            image::GrayAlphaImage::from_raw(w, h, buffer).ok_or("gray alpha layout")?,
        ),
        png::ColorType::Rgb => image::DynamicImage::ImageRgb8(
            image::RgbImage::from_raw(w, h, buffer).ok_or("RGB layout")?,
        ),
        png::ColorType::Rgba => image::DynamicImage::ImageRgba8(
            image::RgbaImage::from_raw(w, h, buffer).ok_or("RGBA layout")?,
        ),
        _ => return Err("expanded normal PNG color".into()),
    };
    if cancel.is_some_and(|c| c.load(Ordering::Relaxed)) {
        return Err("PNG decode cancelled".into());
    }
    let thumbnail = decoded.thumbnail(edge, edge);
    drop(decoded);
    Ok(thumbnail.into_rgba8())
}

/// No full frame, full-width stripe or pyramid allocation. Scanline cancellation
/// is cooperative; PNG regions still pay sequential decompression from the start.
pub(crate) fn derive<R: Read + Seek>(
    source: R,
    edge: u32,
    cancel: Option<&AtomicBool>,
) -> Result<image::RgbaImage, AssetError> {
    if cancel.is_some_and(|c| c.load(Ordering::Relaxed)) {
        return Err("streamed decode cancelled".into());
    }
    let mut reader = decoder(source, true, cancel)?.read_info()?;
    if reader.info().animation_control.is_some() {
        return Err("animated PNG is outside static streamed support".into());
    }
    let size = [reader.info().width, reader.info().height];
    let tile = Tile::from_tag(edge);
    let (out_size, origin, step) = if let Some(t) = tile {
        let step = 1u64.checked_shl(u32::from(t.mip)).ok_or("mip scale")?;
        (
            t.dimensions(size)?,
            [
                u64::from(t.x) * u64::from(TILE_EDGE) * step,
                u64::from(t.y) * u64::from(TILE_EDGE) * step,
            ],
            [step as f64; 2],
        )
    } else {
        if ![8, 16, 32, 64, 128, 512, 2048].contains(&edge) {
            return Err("invalid streamed overview edge".into());
        }
        let factor = f64::from(edge) / f64::from(size[0].max(size[1]));
        let out = size.map(|v| (f64::from(v) * factor.min(1.)).round().max(1.) as u32);
        (
            out,
            [0; 2],
            std::array::from_fn(|i| f64::from(size[i]) / f64::from(out[i])),
        )
    };
    let (color, depth) = reader.output_color_type();
    if depth != png::BitDepth::Eight {
        return Err("PNG normalization did not produce 8-bit pixels".into());
    }
    let channels = match color {
        png::ColorType::Grayscale => 1,
        png::ColorType::GrayscaleAlpha => 2,
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
        _ => return Err("PNG palette not expanded".into()),
    };
    let line = reader
        .output_line_size(size[0])
        .ok_or("PNG row arithmetic")?;
    if line > MAX_ROW_BYTES {
        return Err("PNG transformed row budget".into());
    }
    let bytes = usize::try_from(
        u64::from(out_size[0])
            .checked_mul(u64::from(out_size[1]))
            .and_then(|n| n.checked_mul(4))
            .ok_or("output arithmetic")?,
    )?;
    if bytes > 16 * 1024 * 1024 {
        return Err("streamed display output budget".into());
    }
    let mut pixels = allocate(bytes)?;
    let mut row = allocate(line)?;
    let mut next = 0u32;
    for y in 0..size[1] {
        if cancel.is_some_and(|c| c.load(Ordering::Relaxed)) {
            return Err("streamed decode cancelled".into());
        }
        if reader.read_row(&mut row)?.is_none() {
            return Err("truncated PNG scanlines".into());
        }
        if next == out_size[1] {
            continue;
        }
        let wanted = origin[1] + (f64::from(next) * step[1]).floor() as u64;
        if u64::from(y) != wanted.min(u64::from(size[1] - 1)) {
            continue;
        }
        for x in 0..out_size[0] {
            let sx = (origin[0] + (f64::from(x) * step[0]).floor() as u64)
                .min(u64::from(size[0] - 1)) as usize;
            let src = row
                .get(sx * channels..sx * channels + channels)
                .ok_or("PNG row layout")?;
            let rgba = match channels {
                1 => [src[0], src[0], src[0], 255],
                2 => [src[0], src[0], src[0], src[1]],
                3 => [src[0], src[1], src[2], 255],
                _ => [src[0], src[1], src[2], src[3]],
            };
            let dst = ((next as usize) * (out_size[0] as usize) + x as usize) * 4;
            pixels[dst..dst + 4].copy_from_slice(&rgba);
        }
        next += 1;
    }
    if cancel.is_some_and(|c| c.load(Ordering::Relaxed)) {
        return Err("streamed decode cancelled".into());
    }
    reader.finish()?; // Consume tail chunks and validate CRCs even after sampled output is complete.
    if cancel.is_some_and(|c| c.load(Ordering::Relaxed)) {
        return Err("streamed decode cancelled".into());
    }
    if next != out_size[1] {
        return Err("PNG output rows incomplete".into());
    }
    image::RgbaImage::from_raw(out_size[0], out_size[1], pixels)
        .ok_or_else(|| "streamed pixel layout".into())
}

#[cfg(test)]
mod tests;
