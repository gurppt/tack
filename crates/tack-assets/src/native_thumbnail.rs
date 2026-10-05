//! Safe, single-input thumbnail boundary. Native FFI belongs to turbojpeg, not Tack.
use crate::AssetError;
use turbojpeg::{Colorspace, Decompressor, Image, PixelFormat, ScalingFactor};

const MAX_SOURCE: usize = 64 * 1024 * 1024;
const MAX_OUTPUT: usize = 32 * 1024 * 1024;

/// Validated 128-pixel JPEG decode job, borrowing the same immutable encoded input
/// throughout header and decode. A fresh native handle is destroyed on every error
/// or completion; no pointer or caller-controlled output layout escapes.
pub struct NativeThumbnail<'a> {
    encoded: &'a [u8],
    decoder: Decompressor,
    width: usize,
    height: usize,
    pitch: usize,
    bytes: usize,
}

fn layout(width: usize, height: usize, limit: usize) -> Result<(usize, usize), AssetError> {
    let pitch = width.checked_mul(3).ok_or("JPEG pitch overflow")?;
    let bytes = pitch.checked_mul(height).ok_or("JPEG output overflow")?;
    if width == 0 || height == 0 || bytes > limit {
        return Err("JPEG thumbnail output exceeds allocation limit".into());
    }
    Ok((pitch, bytes))
}

impl<'a> NativeThumbnail<'a> {
    /// Read and validate metadata before allocating pixel storage. The source cap
    /// also bounds marker storage. Progressive native scratch remains proportional
    /// to full dimensions; the output limit alone is not a scratch-memory limit.
    pub fn new(encoded: &'a [u8]) -> Result<Self, AssetError> {
        Self::with_edge(encoded, 128)
    }

    /// Bounded display decode; never selects full resolution merely on zoom.
    pub fn with_edge(encoded: &'a [u8], edge: u32) -> Result<Self, AssetError> {
        if ![8, 16, 32, 64, 128, 512, 2048].contains(&edge) {
            return Err("invalid JPEG display edge".into());
        }
        if encoded.is_empty() || encoded.len() > MAX_SOURCE {
            return Err("JPEG encoded input is empty or exceeds 64 MiB".into());
        }
        let mut decoder = Decompressor::new()?;
        decoder.set_scan_limit(100)?;
        let header = decoder.read_header(encoded)?;
        if header.width == 0 || header.height == 0 || header.width > 6000 || header.height > 4500 {
            return Err("source dimensions exceed 6000x4500".into());
        }
        if header.is_lossless
            || header.is_arithmetic
            || !matches!(
                header.colorspace,
                Colorspace::RGB | Colorspace::YCbCr | Colorspace::Gray
            )
        {
            return Err("prototype JPEG source must be lossy RGB or grayscale".into());
        }
        // Match jpeg-decoder's old power-of-two scale selection: preserve at least
        // 128 pixels on the longest axis, or use full resolution for smaller files.
        let factor = [
            ScalingFactor::ONE_EIGHTH,
            ScalingFactor::ONE_QUARTER,
            ScalingFactor::ONE_HALF,
            ScalingFactor::ONE,
        ]
        .into_iter()
        .find(|factor| factor.scale(header.width.max(header.height)) >= edge as usize)
        .unwrap_or(ScalingFactor::ONE);
        let width = factor.scale(header.width);
        let height = factor.scale(header.height);
        let (pitch, bytes) = layout(width, height, MAX_OUTPUT)?;
        decoder.set_scaling_factor(factor)?;
        Ok(Self {
            encoded,
            decoder,
            width,
            height,
            pitch,
            bytes,
        })
    }

    /// Decode directly into an initialized Rust allocation and transfer it without
    /// copying. Checked pitch/length/nonzero dimensions satisfy the wrapper's
    /// assert_valid invariants. Its second header check sees the identical slice.
    /// Warnings, unsupported precision and malformed scans become recoverable Err.
    pub fn decode(mut self) -> Result<image::RgbImage, AssetError> {
        let mut pixels = Vec::new();
        pixels.try_reserve_exact(self.bytes)?;
        pixels.resize(self.bytes, 0);
        self.decoder.decompress(
            self.encoded,
            Image {
                pixels: pixels.as_mut_slice(),
                width: self.width,
                height: self.height,
                pitch: self.pitch,
                format: PixelFormat::RGB,
            },
        )?;
        image::RgbImage::from_raw(self.width as u32, self.height as u32, pixels)
            .ok_or_else(|| "invalid native JPEG output".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn layout_rejects_zero_overflow_and_allocation_limit() {
        assert!(layout(0, 10, MAX_OUTPUT).is_err());
        assert!(layout(10, 0, MAX_OUTPUT).is_err());
        assert!(layout(usize::MAX, 10, MAX_OUTPUT).is_err());
        assert!(layout(10, usize::MAX, MAX_OUTPUT).is_err());
        assert!(layout(750, 563, 1024).is_err());
        assert_eq!(layout(750, 563, MAX_OUTPUT).ok(), Some((2250, 1_266_750)));
    }
}
