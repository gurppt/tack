use tack_assets::{AssetError, NativeThumbnail};
use turbojpeg::{Compressor, Image, PixelFormat, Subsamp};

fn jpeg(width: usize, height: usize, progressive: bool, gray: bool) -> Result<Vec<u8>, AssetError> {
    let bpp = if gray { 1 } else { 3 };
    let pixels: Vec<u8> = (0..width * height * bpp).map(|i| (i % 251) as u8).collect();
    let mut compressor = Compressor::new()?;
    compressor.set_quality(85)?;
    compressor.set_progressive(progressive)?;
    compressor.set_subsamp(if gray { Subsamp::Gray } else { Subsamp::Sub2x2 })?;
    Ok(compressor.compress_to_vec(Image {
        pixels: pixels.as_slice(),
        width,
        height,
        pitch: width * bpp,
        format: if gray {
            PixelFormat::GRAY
        } else {
            PixelFormat::RGB
        },
    })?)
}

fn rejects(bytes: &[u8]) -> bool {
    NativeThumbnail::new(bytes)
        .and_then(NativeThumbnail::decode)
        .is_err()
}

#[test]
fn rgb_channels_keep_their_order() -> Result<(), AssetError> {
    let mut pixels = vec![0; 256 * 192 * 3];
    for (i, pixel) in pixels.chunks_exact_mut(3).enumerate() {
        pixel[if i % 256 < 128 { 0 } else { 2 }] = 240;
    }
    let mut compressor = Compressor::new()?;
    compressor.set_subsamp(Subsamp::None)?;
    compressor.set_quality(95)?;
    let bytes = compressor.compress_to_vec(Image {
        pixels: pixels.as_slice(),
        width: 256,
        height: 192,
        pitch: 256 * 3,
        format: PixelFormat::RGB,
    })?;
    let image = NativeThumbnail::new(&bytes)?.decode()?;
    let left = image.get_pixel(10, 10);
    let right = image.get_pixel(image.width() - 10, 10);
    assert!(left[0] > 220 && left[2] < 10);
    assert!(right[2] > 220 && right[0] < 10);
    Ok(())
}

#[test]
fn baseline_progressive_gray_and_aspect_ratios() -> Result<(), AssetError> {
    for (width, height) in [(6000, 4500), (257, 13), (13, 257), (31, 7)] {
        for progressive in [false, true] {
            for gray in [false, true] {
                let bytes = jpeg(width, height, progressive, gray)?;
                let decoded = NativeThumbnail::new(&bytes)?.decode()?;
                assert!(decoded.width() > 0 && decoded.height() > 0);
                assert!(decoded.as_raw().len() <= 2 * 1024 * 1024);
                let thumb = image::DynamicImage::ImageRgb8(decoded).thumbnail(128, 128);
                assert!(thumb.width() <= 128 && thumb.height() <= 128);
            }
        }
    }
    Ok(())
}

#[test]
fn malformed_inputs_are_recoverable() -> Result<(), AssetError> {
    assert!(rejects(&[]));
    assert!(rejects(b"not a JPEG"));
    let source = jpeg(256, 192, false, false)?;
    for length in [2, 20, source.len() / 2, source.len() - 2] {
        assert!(rejects(&source[..length]), "truncation at {length}");
    }
    let sof = source
        .windows(2)
        .position(|b| b == [0xff, 0xc0])
        .ok_or("SOF missing")?;
    let mut absurd = source.clone();
    absurd[sof + 5..sof + 9].copy_from_slice(&[255, 255, 255, 255]);
    assert!(rejects(&absurd));
    let mut precision = source.clone();
    precision[sof + 4] = 12;
    assert!(rejects(&precision));
    let sos = source
        .windows(2)
        .position(|b| b == [0xff, 0xda])
        .ok_or("SOS missing")?;
    let mut corrupt = source.clone();
    // Invalid marker in the entropy stream, preserving valid header metadata.
    let scan = sos + 2 + usize::from(u16::from_be_bytes([source[sos + 2], source[sos + 3]]));
    corrupt[scan..scan + 4].copy_from_slice(&[0xff, 0xc4, 0, 1]);
    assert!(rejects(&corrupt));
    let mut metadata = vec![0xff, 0xd8, 0xff, 0xe2, 0xff, 0xff];
    metadata.extend_from_slice(b"ICC_PROFILE\0");
    assert!(rejects(&metadata));
    assert!(rejects(&vec![0; 64 * 1024 * 1024 + 1]));
    Ok(())
}

#[test]
fn unsupported_color_lossless_arithmetic() -> Result<(), AssetError> {
    let pixels = vec![64; 64 * 48 * 4];
    for (format, lossless, arithmetic) in [
        (PixelFormat::CMYK, false, false),
        (PixelFormat::RGB, true, false),
        (PixelFormat::RGB, false, true),
    ] {
        let bpp = format.size();
        let mut compressor = Compressor::new()?;
        compressor.set_subsamp(Subsamp::None)?;
        compressor.set_lossless(lossless)?;
        compressor.set_arithmetic(arithmetic)?;
        let bytes = compressor.compress_to_vec(Image {
            pixels: pixels.as_slice(),
            width: 64,
            height: 48,
            pitch: 64 * bpp,
            format,
        })?;
        assert!(rejects(&bytes));
    }
    Ok(())
}

#[test]
fn bounded_deterministic_mutations_survive() -> Result<(), AssetError> {
    let source = jpeg(64, 48, true, false)?;
    let mut seed = 0x19b0_62a1_u64;
    for iteration in 0..1000 {
        let mut mutated = source.clone();
        for _ in 0..1 + iteration % 8 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let at = (seed as usize) % mutated.len();
            mutated[at] ^= (seed >> 32) as u8;
        }
        if let Ok(image) = NativeThumbnail::new(&mutated).and_then(NativeThumbnail::decode) {
            assert!(image.as_raw().len() <= 2 * 1024 * 1024);
        }
    }
    Ok(())
}
