#![allow(clippy::unwrap_used)]
use super::*;
#[test]
fn cropped_native_layout_covers_every_mip_and_never_allocates_source_pixels() {
    for mip in 0..17 {
        let tag = Tile { mip, x: 0, y: 0 }.tag().unwrap();
        let p = plan([50_000, 512], tag).unwrap();
        assert!(p.output.into_iter().all(|n| n <= 258));
        assert!(p.scale <= 8);
        assert!(p.native[0] <= 6250 || mip < 3);
        assert!(p.native[1] <= 512);
    }
    assert_eq!(plan([50_000, 512], 128).unwrap().output, [128, 1]);
    assert_eq!(plan([32, 16], 128).unwrap().output, [32, 16]);
}
#[test]
fn raster_checks_native_dimensions_excess_truncation_and_cancellation() {
    let p = plan([4, 1], 128).unwrap();
    let mut good = b"P6\n4 1\n255\n".to_vec();
    good.extend([37; 12]);
    let r = raster(&mut std::io::Cursor::new(&good), &p, None).unwrap();
    assert_eq!(r.get_pixel(0, 0).0, [37, 37, 37, 255]);
    good.push(0);
    assert!(raster(&mut std::io::Cursor::new(&good), &p, None).is_err());
    let cancel = AtomicBool::new(true);
    assert!(raster(&mut std::io::Cursor::new(&good), &p, Some(&cancel)).is_err());
}

#[test]
fn mip_strides_and_neighbor_gutters_match_nonconstant_source_coordinates() {
    for mip in [0, 4, 5] {
        let mut results = Vec::new();
        for x in [0, 1] {
            let p = plan([50_000, 512], Tile { mip, x, y: 0 }.tag().unwrap()).unwrap();
            let mut bytes = format!("P6\n{} {}\n255\n", p.native[0], p.native[1]).into_bytes();
            for y in 0..p.native[1] {
                for x in 0..p.native[0] {
                    bytes.extend([(p.crop_start[0] + x) as u8, (p.crop_start[1] + y) as u8, 0]);
                }
            }
            let image = raster(&mut std::io::Cursor::new(bytes), &p, None).unwrap();
            for x in [0, 1, 2, 100, 256, 257] {
                let wanted =
                    (p.start[0] + x).saturating_sub(1).min(p.mip_limit[0]) * p.stride.unwrap();
                assert_eq!(image.get_pixel(x, 1)[0], wanted as u8);
            }
            assert_eq!(image.get_pixel(1, 0), image.get_pixel(1, 1));
            results.push(image);
        }
        assert_eq!(results[0].get_pixel(257, 1), results[1].get_pixel(1, 1));
        assert_eq!(results[0].get_pixel(256, 1), results[1].get_pixel(0, 1));
    }
    let p = plan([8193, 512], Tile { mip: 4, x: 2, y: 0 }.tag().unwrap()).unwrap();
    assert_eq!(p.output[0], 3);
    assert_eq!(sample(&p, 0, 1), sample(&p, 0, 2));
}

pub(super) fn jpeg(width: u32, height: u32, progressive: bool) -> Vec<u8> {
    jpeg_subsampled(width, height, progressive, turbojpeg::Subsamp::None)
}
pub(super) fn jpeg_subsampled(
    width: u32,
    height: u32,
    progressive: bool,
    subsample: turbojpeg::Subsamp,
) -> Vec<u8> {
    let image = image::RgbImage::from_fn(width, height, |x, y| {
        image::Rgb([(x % 251) as u8, (y % 241) as u8, ((x / 31) % 229) as u8])
    });
    let mut compressor = turbojpeg::Compressor::new().unwrap();
    compressor.set_quality(95).unwrap();
    compressor.set_subsamp(subsample).unwrap();
    compressor.set_progressive(progressive).unwrap();
    compressor
        .compress_to_vec(turbojpeg::Image {
            pixels: image.as_raw().as_slice(),
            width: width as usize,
            height: height as usize,
            pitch: width as usize * 3,
            format: turbojpeg::PixelFormat::RGB,
        })
        .unwrap()
}
#[test]
fn real_wide_decoder_overview_tiles_gutters_progressive_and_corruption() {
    let bytes = jpeg(9000, 17, false);
    let mut input = bytes.as_slice();
    let h = jpeg_header::read(&mut input).unwrap();
    let overview = derive(&mut input, h, 128, None).unwrap();
    assert_eq!(overview.dimensions(), (128, 1));
    let mut tiles = Vec::new();
    for x in [0, 1, 35] {
        let mut input = bytes.as_slice();
        let h = jpeg_header::read(&mut input).unwrap();
        tiles.push(derive(&mut input, h, Tile { mip: 0, x, y: 0 }.tag().unwrap(), None).unwrap());
    }
    assert_eq!(tiles[0].dimensions(), (258, 19));
    assert_eq!(tiles[0].get_pixel(257, 5), tiles[1].get_pixel(1, 5));
    assert_eq!(tiles[0].get_pixel(256, 5), tiles[1].get_pixel(0, 5));
    assert_eq!(tiles[2].dimensions(), (42, 19));
    assert_eq!(tiles[2].get_pixel(40, 5), tiles[2].get_pixel(41, 5));
    let progressive = jpeg(9000, 17, true);
    let mut input = progressive.as_slice();
    let h = jpeg_header::read(&mut input).unwrap();
    assert!(
        derive(&mut input, h, 128, None)
            .unwrap_err()
            .to_string()
            .contains("progressive")
    );
    let mut input = bytes[..bytes.len() / 2].as_ref();
    let h = jpeg_header::read(&mut input).unwrap();
    assert!(derive(&mut input, h, 128, None).is_err());
    let mut input = bytes.as_slice();
    let h = jpeg_header::read(&mut input).unwrap();
    let cancel = AtomicBool::new(true);
    assert!(derive(&mut input, h, 128, Some(&cancel)).is_err());
}
#[test]
fn blocked_native_raster_read_is_cancelled_and_reaped() {
    struct Slow<'a>(&'a [u8]);
    impl Read for Slow<'_> {
        fn read(&mut self, b: &mut [u8]) -> std::io::Result<usize> {
            std::thread::sleep(Duration::from_millis(80));
            self.0.read(b)
        }
    }
    let bytes = jpeg(9000, 17, false);
    let mut input = bytes.as_slice();
    let h = jpeg_header::read(&mut input).unwrap();
    let cancel = AtomicBool::new(false);
    let started = Instant::now();
    let error = std::thread::scope(|scope| {
        scope.spawn(|| {
            std::thread::sleep(Duration::from_millis(10));
            cancel.store(true, Ordering::Relaxed);
        });
        derive(&mut Slow(input), h, 128, Some(&cancel)).unwrap_err()
    });
    assert!(error.to_string().contains("cancelled"));
    assert!(started.elapsed() < Duration::from_secs(2));
}

#[test]
fn actual_native_mip_four_samples_match_scaled_scanlines_with_neighbor_gutters() {
    let bytes = jpeg(9000, 17, false);
    let mut input = bytes.as_slice();
    let h = jpeg_header::read(&mut input).unwrap();
    let scaled = derive(&mut input, h, 2048, None).unwrap();
    // 9000/8 >=edge? For2048 the DCT factor is1/4, followed byresize;
    // compare tile neighbors directly at same native1/8 scaling instead.
    assert_eq!(scaled.dimensions(), (2048, 4));
    let native = crate::NativeThumbnail::with_edge(&bytes, 512)
        .unwrap()
        .decode()
        .unwrap();
    assert_eq!(native.dimensions(), (1125, 3));
    let mut tiles = Vec::new();
    for x in [0, 1, 2] {
        let mut input = bytes.as_slice();
        let h = jpeg_header::read(&mut input).unwrap();
        tiles.push(derive(&mut input, h, Tile { mip: 4, x, y: 0 }.tag().unwrap(), None).unwrap());
    }
    for (tile_x, tile) in tiles.iter().enumerate() {
        for x in 0..tile.width() {
            let sx = ((tile_x as u32 * 256 + x).saturating_sub(1).min(562) * 2).min(1124);
            assert_eq!(&tile.get_pixel(x, 1).0[..3], &native.get_pixel(sx, 0).0);
        }
    }
    assert_eq!(tiles[0].get_pixel(257, 1), tiles[1].get_pixel(1, 1));
    assert_eq!(tiles[0].get_pixel(256, 1), tiles[1].get_pixel(0, 1));
    assert_eq!(tiles[1].get_pixel(257, 1), tiles[2].get_pixel(1, 1));
    assert_eq!(
        tiles[2].get_pixel(tiles[2].width() - 1, 1),
        tiles[2].get_pixel(tiles[2].width() - 2, 1)
    );
}

#[test]
fn subsampled_color_gutters_match_neighbor_interiors_across_native_tile_boundaries() {
    for subsampling in [turbojpeg::Subsamp::Sub2x2, turbojpeg::Subsamp::Sub2x1] {
        let bytes = jpeg_subsampled(9000, 520, false, subsampling);
        let mut tiles = Vec::new();
        for (x, y) in [(0, 0), (1, 0), (0, 1)] {
            let mut input = bytes.as_slice();
            let header = jpeg_header::read(&mut input).unwrap();
            tiles.push(
                derive(
                    &mut input,
                    header,
                    Tile { mip: 0, x, y }.tag().unwrap(),
                    None,
                )
                .unwrap(),
            );
        }
        for p in 1..257 {
            assert_eq!(
                tiles[0].get_pixel(257, p),
                tiles[1].get_pixel(1, p),
                "right gutter {subsampling:?} row {p}"
            );
            assert_eq!(
                tiles[0].get_pixel(256, p),
                tiles[1].get_pixel(0, p),
                "left gutter {subsampling:?} row {p}"
            );
            assert_eq!(
                tiles[0].get_pixel(p, 257),
                tiles[2].get_pixel(p, 1),
                "bottom gutter {subsampling:?} column {p}"
            );
            assert_eq!(
                tiles[0].get_pixel(p, 256),
                tiles[2].get_pixel(p, 0),
                "top gutter {subsampling:?} column {p}"
            );
        }
    }
}
