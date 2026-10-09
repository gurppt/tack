#![allow(clippy::unwrap_used)]
use super::*;

fn tags(mip: u8, width: u32, height: u32) -> Vec<u32> {
    (0..height)
        .flat_map(|y| (0..width).map(move |x| Tile { mip, x, y }.tag().unwrap()))
        .collect()
}

#[test]
fn regional_admission_rejects_gaps_mixed_mips_duplicates_and_oversized_crops() {
    let size = [50_000; 2];
    let full = tags(0, 5, 3);
    let batch = batch_plan(size, &full).unwrap();
    assert_eq!(batch.tiles.len(), MAX_BATCH_TILES);
    assert_eq!(batch.region.crop_start[0] % 96, 0);
    assert!(
        u64::from(batch.region.native[0]) * u64::from(batch.region.native[1]) * 4 <= REGION_BYTES
    );
    assert!(admissible_tiles(size, &tags(0, 4, 3)));
    assert!(!admissible_tiles(size, &[]));
    assert!(!admissible_tiles(size, &tags(0, 4, 4)));
    assert!(!admissible_tiles(size, &[128]));
    assert!(!admissible_tiles(size, &[full[0], full[0]]));
    assert!(!admissible_tiles(size, &[full[0], full[2]]));
    assert!(!admissible_tiles(size, &[full[0], tags(1, 2, 1)[1]]));
    assert!(!admissible_tiles(size, &tags(4, 4, 3)));
    // Existing single-tile derivation remains available at coarse integer strides.
    assert!(plan(size, tags(5, 1, 1)[0]).is_ok());
    assert!(!admissible_tiles(size, &tags(5, 1, 1)));
    assert!(!admissible_tiles([32; 2], &tags(0, 2, 1)));
}

fn pnm(native: [u32; 2], origin: [u32; 2]) -> Vec<u8> {
    let mut bytes = format!("P6\n{} {}\n255\n", native[0], native[1]).into_bytes();
    for y in 0..native[1] {
        for x in 0..native[0] {
            bytes.extend([
                ((origin[0] + x) % 251) as u8,
                ((origin[1] + y) % 241) as u8,
                (((origin[0] + x) / 31 + (origin[1] + y) / 29) % 229) as u8,
            ]);
        }
    }
    bytes
}

#[test]
fn rectangular_scanline_scatter_matches_single_tiles_at_all_gutters_and_partial_edges() {
    // Input order need not equal geometric order. Include partial last row/column.
    for (size, mip, width, height) in [([520, 513], 0, 3, 3), ([8200, 4200], 4, 3, 2)] {
        let mut edges = tags(mip, width, height);
        edges.reverse();
        let batch = batch_plan(size, &edges).unwrap();
        let bytes = pnm(batch.region.native, batch.region.crop_start);
        let results = raster_many(
            &mut std::io::Cursor::new(&bytes),
            batch.region.native,
            &batch.tiles,
            None,
        )
        .unwrap();
        for (&edge, result) in edges.iter().zip(&results) {
            let single = plan(size, edge).unwrap();
            let wanted = raster(
                &mut std::io::Cursor::new(pnm(single.native, single.crop_start)),
                &single,
                None,
            )
            .unwrap();
            assert_eq!(result, &wanted, "size {size:?} mip {mip} edge {edge}");
        }
        let cancel = AtomicBool::new(true);
        assert!(
            raster_many(
                &mut std::io::Cursor::new(&bytes),
                batch.region.native,
                &batch.tiles,
                Some(&cancel)
            )
            .is_err()
        );
        assert!(
            raster_many(
                &mut std::io::Cursor::new(&bytes[..bytes.len() - 1]),
                batch.region.native,
                &batch.tiles,
                None
            )
            .is_err()
        );
        let mut excess = bytes;
        excess.push(0);
        assert!(
            raster_many(
                &mut std::io::Cursor::new(excess),
                batch.region.native,
                &batch.tiles,
                None
            )
            .is_err()
        );
    }
}

#[test]
fn real_native_regional_batch_matches_single_tiles_and_reads_source_once() {
    struct CountRead<'a> {
        bytes: &'a [u8],
        read: usize,
    }
    impl Read for CountRead<'_> {
        fn read(&mut self, target: &mut [u8]) -> std::io::Result<usize> {
            let read = self.bytes.read(target)?;
            self.read += read;
            Ok(read)
        }
    }
    for subsampling in [turbojpeg::Subsamp::None, turbojpeg::Subsamp::Sub2x2] {
        let bytes = super::super::tests::jpeg_subsampled(1030, 520, false, subsampling);
        let mut edges = tags(0, 5, 3);
        edges.reverse();
        let mut source = CountRead {
            bytes: &bytes,
            read: 0,
        };
        let header = jpeg_header::read(&mut source).unwrap();
        let results = derive_tiles(&mut source, header, &edges, None).unwrap();
        assert_eq!(results.len(), edges.len());
        assert!(source.read <= bytes.len());
        for (&edge, result) in edges.iter().zip(&results) {
            let mut input = bytes.as_slice();
            let header = jpeg_header::read(&mut input).unwrap();
            assert_eq!(result, &derive(&mut input, header, edge, None).unwrap());
        }
    }
}

#[test]
fn batch_refuses_progressive_truncated_and_cancelled_sources() {
    let edges = tags(0, 2, 1);
    let bytes = super::super::tests::jpeg(9000, 17, true);
    let mut input = bytes.as_slice();
    let header = jpeg_header::read(&mut input).unwrap();
    assert!(
        derive_tiles(&mut input, header, &edges, None)
            .unwrap_err()
            .to_string()
            .contains("progressive")
    );
    let bytes = super::super::tests::jpeg(9000, 17, false);
    let mut input = &bytes[..bytes.len() / 2];
    let header = jpeg_header::read(&mut input).unwrap();
    assert!(derive_tiles(&mut input, header, &edges, None).is_err());
    let mut input = bytes.as_slice();
    let header = jpeg_header::read(&mut input).unwrap();
    assert!(derive_tiles(&mut input, header, &edges, Some(&AtomicBool::new(true))).is_err());
}
