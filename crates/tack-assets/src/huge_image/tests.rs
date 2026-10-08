#![allow(clippy::unwrap_used)]
use super::*;
use std::io::{Cursor, Write};

fn encoded(size: [u32; 2], color: png::ColorType, depth: png::BitDepth, pixels: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, size[0], size[1]);
    encoder.set_color(color);
    encoder.set_depth(depth);
    let mut writer = encoder.write_header().unwrap();
    writer.write_image_data(pixels).unwrap();
    writer.finish().unwrap();
    bytes
}
fn change_header(bytes: &mut [u8], size: [u32; 2], interlaced: bool) {
    bytes[16..20].copy_from_slice(&size[0].to_be_bytes());
    bytes[20..24].copy_from_slice(&size[1].to_be_bytes());
    bytes[28] = u8::from(interlaced);
    let crc = crc32fast::hash(&bytes[12..29]);
    bytes[29..33].copy_from_slice(&crc.to_be_bytes());
}

#[test]
fn classification_arithmetic_and_tile_addresses_are_bounded() {
    assert_eq!(classify_png([1024, 1024]).unwrap(), ImageClass::Normal);
    assert_eq!(classify_png([2048, 2049]).unwrap(), ImageClass::Large);
    assert_eq!(classify_png([50000, 512]).unwrap(), ImageClass::HugeTiled);
    for size in [
        [0, 1],
        [1, 0],
        [u32::MAX, u32::MAX],
        [262145, 1],
        [262144, 262144],
    ] {
        assert!(decoded_bytes(size).is_err());
    }
    let tile = Tile { mip: 1, x: 1, y: 1 };
    assert_eq!(Tile::from_tag(tile.tag().unwrap()), Some(tile));
    assert_eq!(tile.dimensions([513, 515]).unwrap(), [1, 2]);
    assert!(
        Tile {
            mip: 32,
            x: 0,
            y: 0
        }
        .tag()
        .is_err()
    );
    assert!(
        Tile {
            mip: 0,
            x: 8192,
            y: 0
        }
        .tag()
        .is_err()
    );
    assert!(Tile { mip: 0, x: 2, y: 0 }.dimensions([257, 259]).is_err());
    assert!(mip_dimensions([1, 1], 64).is_err());
    assert!(mip_for_density([1, 1], f64::NAN).is_err());
    assert!(mip_for_density([1, 1], 0.).is_err());
}

#[test]
fn rgba_alpha_and_sampled_overview_match_source_coordinates() {
    let pixels: Vec<_> = (0..13)
        .flat_map(|y| {
            (0..19).flat_map(move |x| [x as u8, y as u8, (x + y) as u8, ((x * 13 + y) % 256) as u8])
        })
        .collect();
    let bytes = encoded(
        [19, 13],
        png::ColorType::Rgba,
        png::BitDepth::Eight,
        &pixels,
    );
    let output = derive(Cursor::new(&bytes), 8, None).unwrap();
    assert_eq!(output.dimensions(), (8, 5));
    for y in 0..5 {
        for x in 0..8 {
            let sx = x * 19 / 8;
            let sy = y * 13 / 5;
            let start = ((sy * 19 + sx) * 4) as usize;
            assert_eq!(&output.get_pixel(x, y).0[..], &pixels[start..start + 4]);
        }
    }
}

#[test]
fn border_tiles_are_cropped_without_padding_or_overread() {
    let pixels: Vec<_> = (0..515)
        .flat_map(|y| (0..513).flat_map(move |x| [(x % 251) as u8, (y % 251) as u8, 77, 123]))
        .collect();
    let bytes = encoded(
        [513, 515],
        png::ColorType::Rgba,
        png::BitDepth::Eight,
        &pixels,
    );
    let tile = Tile { mip: 1, x: 1, y: 1 };
    let output = derive(Cursor::new(&bytes), tile.tag().unwrap(), None).unwrap();
    assert_eq!(output.dimensions(), (1, 2));
    assert_eq!(output.get_pixel(0, 0).0, [10, 10, 77, 123]);
    assert_eq!(output.get_pixel(0, 1).0, [10, 12, 77, 123]);
    let outside = Tile { mip: 0, x: 3, y: 0 }.tag().unwrap();
    assert!(derive(Cursor::new(&bytes), outside, None).is_err());
}

#[test]
fn palette_transparency_low_bits_and_sixteen_bit_channels_are_normalized() {
    let mut palette = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut palette, 3, 1);
        encoder.set_color(png::ColorType::Indexed);
        encoder.set_depth(png::BitDepth::Two);
        encoder.set_palette(vec![10, 20, 30, 40, 50, 60, 70, 80, 90]);
        encoder.set_trns(vec![255, 0, 127]);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(&[0b00011000]).unwrap();
        writer.finish().unwrap();
    }
    let output = derive(Cursor::new(&palette), 8, None).unwrap();
    assert_eq!(
        output.into_raw(),
        [10, 20, 30, 255, 40, 50, 60, 0, 70, 80, 90, 127]
    );
    let bytes = encoded(
        [2, 1],
        png::ColorType::GrayscaleAlpha,
        png::BitDepth::Sixteen,
        &[0x12, 0x34, 0xab, 0xcd, 0xff, 0xff, 0x00, 0x00],
    );
    assert_eq!(
        derive(Cursor::new(bytes), 8, None).unwrap().into_raw(),
        [0x12, 0x12, 0x12, 0xab, 255, 255, 255, 0]
    );
    let bits = encoded(
        [3, 1],
        png::ColorType::Grayscale,
        png::BitDepth::One,
        &[0b10100000],
    );
    assert_eq!(
        derive(Cursor::new(bits), 8, None).unwrap().into_raw(),
        [255, 255, 255, 255, 0, 0, 0, 255, 255, 255, 255, 255]
    );
}

#[test]
fn metadata_accepts_huge_static_headers_without_allocating_source_rgba() {
    let mut bytes = encoded(
        [1, 1],
        png::ColorType::Rgb,
        png::BitDepth::Eight,
        &[1, 2, 3],
    );
    change_header(&mut bytes, [50000, 4096], false);
    assert_eq!(png_dimensions(Cursor::new(&bytes)).unwrap(), [50000, 4096]);
    assert!(derive(Cursor::new(&bytes), 128, None).is_err());
    change_header(&mut bytes, [50000, 4096], true);
    assert!(
        png_dimensions(Cursor::new(&bytes))
            .unwrap_err()
            .to_string()
            .contains("interlaced")
    );
    change_header(&mut bytes, [1, 1], true);
    assert_eq!(png_dimensions(Cursor::new(&bytes)).unwrap(), [1, 1]);
    assert!(derive(Cursor::new(&bytes), 128, None).is_err());
    change_header(&mut bytes, [7000, 1], true);
    assert!(
        png_dimensions(Cursor::new(&bytes))
            .unwrap_err()
            .to_string()
            .contains("interlaced")
    );
}

#[test]
fn animation_is_rejected_before_streaming_large_source_pixels() {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, 1, 1);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_animated(1, 0).unwrap();
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(&[1, 2, 3]).unwrap();
        writer.finish().unwrap();
    }
    assert!(
        derive(Cursor::new(&bytes), 8, None)
            .unwrap_err()
            .to_string()
            .contains("animated")
    );
    change_header(&mut bytes, [12000, 1024], false);
    // Keep the first APNG frame-control header consistent with the canvas.
    let frame = bytes.windows(4).position(|kind| kind == b"fcTL").unwrap();
    bytes[frame + 8..frame + 12].copy_from_slice(&12000u32.to_be_bytes());
    bytes[frame + 12..frame + 16].copy_from_slice(&1024u32.to_be_bytes());
    let crc = crc32fast::hash(&bytes[frame..frame + 30]);
    bytes[frame + 30..frame + 34].copy_from_slice(&crc.to_be_bytes());
    assert!(
        png_dimensions(Cursor::new(&bytes))
            .unwrap_err()
            .to_string()
            .contains("animated")
    );
    change_header(&mut bytes, [7000, 1], false);
    bytes[frame + 8..frame + 12].copy_from_slice(&7000u32.to_be_bytes());
    bytes[frame + 12..frame + 16].copy_from_slice(&1u32.to_be_bytes());
    let crc = crc32fast::hash(&bytes[frame..frame + 30]);
    bytes[frame + 30..frame + 34].copy_from_slice(&crc.to_be_bytes());
    assert!(
        png_dimensions(Cursor::new(&bytes))
            .unwrap_err()
            .to_string()
            .contains("animated")
    );
}

#[test]
fn cancellation_and_bad_crc_or_truncated_tail_never_return_output() {
    let bytes = encoded([3, 2], png::ColorType::Rgb, png::BitDepth::Eight, &[7; 18]);
    let cancel = AtomicBool::new(true);
    let mut source = Cursor::new(&bytes);
    assert!(derive(&mut source, 8, Some(&cancel)).is_err());
    assert_eq!(source.position(), 0);
    for length in [8, 32, bytes.len() - 4] {
        assert!(derive(Cursor::new(&bytes[..length]), 8, None).is_err());
    }
    let mut corrupt = bytes.clone();
    let last = corrupt.len() - 1;
    corrupt[last] ^= 1;
    assert!(derive(Cursor::new(corrupt), 8, None).is_err());
}

struct CancelOnRead<'a> {
    source: Cursor<Vec<u8>>,
    cancel: &'a AtomicBool,
}
impl Read for CancelOnRead<'_> {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        let count = self.source.read(bytes)?;
        self.cancel.store(true, Ordering::Relaxed);
        Ok(count)
    }
}
impl Seek for CancelOnRead<'_> {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        self.source.seek(position)
    }
}
#[test]
fn cancellation_after_source_read_discards_decode_output() {
    let bytes = encoded([3, 2], png::ColorType::Rgb, png::BitDepth::Eight, &[7; 18]);
    let cancel = AtomicBool::new(false);
    let source = CancelOnRead {
        source: Cursor::new(bytes),
        cancel: &cancel,
    };
    let error = derive(source, 8, Some(&cancel)).unwrap_err();
    assert!(error.to_string().contains("cancelled"));
}

#[test]
fn cumulative_read_budget_survives_seeks() {
    let mut source = SourceReader {
        inner: Cursor::new([1; 10]),
        remaining: 3,
        cancel: None,
    };
    let mut bytes = [0; 2];
    source.read_exact(&mut bytes).unwrap();
    source.seek(SeekFrom::Start(0)).unwrap();
    assert!(source.read_exact(&mut bytes).is_err());
    assert_eq!(source.remaining, 0);
}

#[test]
fn row_writer_fixture_has_no_full_frame_allocation() {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, 8192, 1025);
        encoder.set_color(png::ColorType::Grayscale);
        let mut writer = encoder.write_header().unwrap();
        let row: Vec<_> = (0..8192).map(|x| (x % 251) as u8).collect();
        {
            let mut stream = writer.stream_writer().unwrap();
            for _ in 0..1025 {
                stream.write_all(&row).unwrap();
            }
            stream.finish().unwrap();
        }
        writer.finish().unwrap();
    }
    let output = derive(Cursor::new(bytes), 128, None).unwrap();
    assert_eq!(output.dimensions(), (128, 16));
    assert_eq!(output.get_pixel(1, 0).0, [64, 64, 64, 255]);
}
