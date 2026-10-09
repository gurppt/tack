use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

struct TestRoot(PathBuf);

impl TestRoot {
    fn new() -> Self {
        static SERIAL: AtomicUsize = AtomicUsize::new(0);
        Self(std::env::temp_dir().join(format!(
            "tack-raw-tile-test-{}-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        )))
    }
}

impl Drop for TestRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn pixels(value: u8) -> Decoded {
    Decoded {
        width: 5,
        height: 7,
        rgba: vec![value; 5 * 7 * 4],
    }
}

fn key(value: u8) -> [u8; 80] {
    [value; 80]
}

#[test]
fn roundtrip_survives_independent_reopen_without_eager_reservation() -> Result<(), AssetError> {
    let root = TestRoot::new();
    let mut disk = TileDisk::open(&root.0, MAX_BUDGET)?;
    assert_eq!(disk.used_bytes(), FILE_HEADER_BYTES);
    assert_eq!(
        std::fs::metadata(root.0.join(FILE_NAME))?.len(),
        FILE_HEADER_BYTES as u64
    );
    assert_eq!(std::fs::read(root.0.join(FILE_NAME))?, file_header());
    assert_eq!(disk.get(key(1))?.map(|image| image.rgba), None);
    assert!(disk.put(key(1), &pixels(41))?);
    assert_eq!(disk.used_bytes(), FILE_HEADER_BYTES + SLOT_BYTES);
    assert_eq!(disk.payload_bytes(), 140);
    drop(disk);
    let mut reopened = TileDisk::open(&root.0, MAX_BUDGET)?;
    let image = reopened.get(key(1))?.ok_or("missing reopened tile")?;
    assert_eq!((image.width, image.height), (5, 7));
    assert_eq!(image.rgba, pixels(41).rgba);
    assert_eq!(reopened.entries(), 1);
    Ok(())
}

#[test]
fn quota_and_persisted_read_recency_evict_the_oldest_tile() -> Result<(), AssetError> {
    let root = TestRoot::new();
    let budget = FILE_HEADER_BYTES + SLOT_BYTES * 2 + 3;
    let mut disk = TileDisk::open(&root.0, budget)?;
    assert!(disk.put(key(1), &pixels(1))?);
    assert!(disk.put(key(2), &pixels(2))?);
    assert!(disk.get(key(1))?.is_some());
    drop(disk);
    let mut disk = TileDisk::open(&root.0, budget)?;
    assert!(disk.put(key(3), &pixels(3))?);
    assert!(disk.get(key(1))?.is_some());
    assert!(disk.get(key(2))?.is_none());
    assert!(disk.get(key(3))?.is_some());
    for value in 4..40 {
        assert!(disk.put(key(value), &pixels(value))?);
        assert_eq!(disk.entries(), 2);
        assert!(disk.used_bytes() <= budget);
    }
    assert_eq!(
        std::fs::metadata(root.0.join(FILE_NAME))?.len(),
        (FILE_HEADER_BYTES + 2 * SLOT_BYTES) as u64
    );
    Ok(())
}

#[test]
fn smaller_and_zero_quotas_trim_only_disposable_data() -> Result<(), AssetError> {
    let root = TestRoot::new();
    let mut disk = TileDisk::open(&root.0, FILE_HEADER_BYTES + SLOT_BYTES * 3)?;
    for value in 1..4 {
        assert!(disk.put(key(value), &pixels(value))?);
    }
    drop(disk);
    let mut disk = TileDisk::open(&root.0, FILE_HEADER_BYTES + SLOT_BYTES)?;
    assert_eq!(disk.used_bytes(), FILE_HEADER_BYTES + SLOT_BYTES);
    assert_eq!(disk.entries(), 1);
    assert!(disk.get(key(1))?.is_some());
    drop(disk);
    let mut disk = TileDisk::open(&root.0, FILE_HEADER_BYTES + SLOT_BYTES - 1)?;
    assert_eq!(disk.used_bytes(), FILE_HEADER_BYTES);
    assert!(!disk.put(key(4), &pixels(4))?);
    drop(disk);
    assert!(TileDisk::open(&root.0, FILE_HEADER_BYTES - 1).is_err());
    assert!(TileDisk::open(&root.0, 0).is_err());
    let disk = TileDisk::open(&root.0, usize::MAX)?;
    assert_eq!(disk.budget(), MAX_BUDGET);
    assert_eq!(disk.capacity, 251);
    Ok(())
}

#[test]
fn potato_quota_stays_bounded_across_more_visited_tiles() -> Result<(), AssetError> {
    let root = TestRoot::new();
    let budget = 4 * 1024 * 1024;
    let mut disk = TileDisk::open(&root.0, budget)?;
    for value in 1..40 {
        assert!(disk.put(key(value), &pixels(value))?);
        assert!(disk.used_bytes() <= budget);
    }
    assert_eq!(disk.entries(), 15);
    assert_eq!(disk.used_bytes(), FILE_HEADER_BYTES + 15 * SLOT_BYTES);
    drop(disk);
    let disk = TileDisk::open(&root.0, budget)?;
    assert_eq!(disk.entries(), 15);
    assert_eq!(disk.used_bytes(), FILE_HEADER_BYTES + 15 * SLOT_BYTES);
    Ok(())
}

#[test]
fn checked_dimensions_and_bytecount_reject_before_writing() -> Result<(), AssetError> {
    let root = TestRoot::new();
    let mut disk = TileDisk::open(&root.0, FILE_HEADER_BYTES + SLOT_BYTES)?;
    for (width, height) in [(0, 1), (1, 0), (259, 1), (1, 259), (u32::MAX, u32::MAX)] {
        assert!(!disk.put(
            key(1),
            &Decoded {
                width,
                height,
                rgba: vec![]
            }
        )?);
    }
    assert!(!disk.put(
        key(1),
        &Decoded {
            width: 1,
            height: 1,
            rgba: vec![0; 3]
        }
    )?);
    assert!(!disk.put(
        key(1),
        &Decoded {
            width: 1,
            height: 1,
            rgba: vec![0; 5]
        }
    )?);
    assert_eq!(disk.used_bytes(), FILE_HEADER_BYTES);
    let image = Decoded {
        width: MAX_TILE_EDGE,
        height: MAX_TILE_EDGE,
        rgba: vec![123; MAX_PAYLOAD],
    };
    assert!(disk.put(key(1), &image)?);
    assert_eq!(
        disk.get(key(1))?.ok_or("missing max tile")?.rgba,
        image.rgba
    );
    Ok(())
}

#[test]
fn duplicate_put_updates_one_slot_and_all_identity_bytes_matter() -> Result<(), AssetError> {
    let root = TestRoot::new();
    let mut disk = TileDisk::open(&root.0, FILE_HEADER_BYTES + SLOT_BYTES * 2)?;
    assert!(disk.put(key(1), &pixels(1))?);
    assert!(disk.put(key(1), &pixels(2))?);
    assert_eq!(disk.entries(), 1);
    assert_eq!(disk.used_bytes(), FILE_HEADER_BYTES + SLOT_BYTES);
    assert_eq!(
        disk.get(key(1))?.ok_or("missing replacement")?.rgba,
        pixels(2).rgba
    );
    for index in 0..80 {
        let mut changed = key(1);
        changed[index] ^= 1;
        assert!(disk.get(changed)?.is_none());
    }
    Ok(())
}

#[test]
fn corrupt_payload_is_a_miss_and_can_be_regenerated() -> Result<(), AssetError> {
    let root = TestRoot::new();
    let mut disk = TileDisk::open(&root.0, FILE_HEADER_BYTES + SLOT_BYTES)?;
    assert!(disk.put(key(1), &pixels(1))?);
    disk.file.seek(SeekFrom::Start(
        FILE_HEADER_BYTES as u64 + HEADER_BYTES as u64 + 17,
    ))?;
    disk.file.write_all(&[77])?;
    assert!(disk.get(key(1))?.is_none());
    assert_eq!(disk.entries(), 0);
    assert!(disk.put(key(1), &pixels(2))?);
    assert_eq!(
        disk.get(key(1))?.ok_or("missing regenerated tile")?.rgba,
        pixels(2).rgba
    );
    Ok(())
}

#[test]
fn corrupt_and_forged_headers_cannot_allocate_large_buffers() -> Result<(), AssetError> {
    let root = TestRoot::new();
    let mut disk = TileDisk::open(&root.0, FILE_HEADER_BYTES + SLOT_BYTES * 2)?;
    assert!(disk.put(key(1), &pixels(1))?);
    disk.file
        .seek(SeekFrom::Start(FILE_HEADER_BYTES as u64 + 88))?;
    disk.file.write_all(&u32::MAX.to_le_bytes())?;
    assert!(disk.get(key(1))?.is_none());
    assert!(disk.put(key(1), &pixels(1))?);
    let mut forged = encode_header(disk.slots[0].ok_or("missing entry")?);
    forged[88..92].copy_from_slice(&u32::MAX.to_le_bytes());
    forged[96..100].copy_from_slice(&u32::MAX.to_le_bytes());
    let crc = crc32fast::hash(&forged[..124]);
    forged[124..].copy_from_slice(&crc.to_le_bytes());
    disk.file.seek(SeekFrom::Start(slot_offset(0)))?;
    disk.file.write_all(&forged)?;
    drop(disk);
    let mut disk = TileDisk::open(&root.0, FILE_HEADER_BYTES + SLOT_BYTES * 2)?;
    assert_eq!(disk.entries(), 0);
    assert!(disk.get(key(1))?.is_none());
    assert!(disk.put(key(1), &pixels(3))?);
    Ok(())
}

#[test]
fn incomplete_append_is_discarded_on_open() -> Result<(), AssetError> {
    let root = TestRoot::new();
    let mut disk = TileDisk::open(&root.0, FILE_HEADER_BYTES + SLOT_BYTES * 2)?;
    assert!(disk.put(key(1), &pixels(1))?);
    disk.file
        .set_len((FILE_HEADER_BYTES + SLOT_BYTES + 50) as u64)?;
    drop(disk);
    let mut disk = TileDisk::open(&root.0, FILE_HEADER_BYTES + SLOT_BYTES * 2)?;
    assert_eq!(disk.used_bytes(), FILE_HEADER_BYTES + SLOT_BYTES);
    assert!(disk.get(key(1))?.is_some());
    Ok(())
}

#[test]
fn startup_deduplicates_forged_duplicate_slots() -> Result<(), AssetError> {
    let root = TestRoot::new();
    let mut disk = TileDisk::open(&root.0, FILE_HEADER_BYTES + SLOT_BYTES * 2)?;
    assert!(disk.put(key(1), &pixels(1))?);
    assert!(disk.put(key(2), &pixels(2))?);
    let mut duplicate = disk.slots[1].ok_or("missing entry")?;
    duplicate.key = key(1);
    disk.write_header(1, duplicate)?;
    drop(disk);
    let mut disk = TileDisk::open(&root.0, FILE_HEADER_BYTES + SLOT_BYTES * 2)?;
    assert_eq!(disk.entries(), 1);
    assert_eq!(
        disk.get(key(1))?.ok_or("missing newer duplicate")?.rgba,
        pixels(2).rgba
    );
    assert!(disk.put(key(3), &pixels(3))?);
    assert_eq!(disk.entries(), 2);
    Ok(())
}

#[test]
fn exclusive_owner_refuses_another_open_and_releases_on_drop() -> Result<(), AssetError> {
    let root = TestRoot::new();
    let disk = TileDisk::open(&root.0, FILE_HEADER_BYTES + SLOT_BYTES)?;
    assert!(TileDisk::open(&root.0, FILE_HEADER_BYTES + SLOT_BYTES).is_err());
    drop(disk);
    let disk = TileDisk::open(&root.0, FILE_HEADER_BYTES + SLOT_BYTES)?;
    drop(disk);
    assert!(root.0.join(LOCK_NAME).is_file());
    Ok(())
}

#[test]
fn deletion_and_recreation_leave_source_decode_independent() -> Result<(), AssetError> {
    let root = TestRoot::new();
    let mut disk = TileDisk::open(&root.0, FILE_HEADER_BYTES + SLOT_BYTES)?;
    assert!(disk.put(key(1), &pixels(1))?);
    match std::fs::remove_file(root.0.join(FILE_NAME)) {
        Ok(()) => {
            assert!(disk.get(key(1)).is_err());
            assert!(disk.put(key(2), &pixels(2)).is_err());
        }
        // Windows may refuse deletion of an open file. Delete after closing;
        // reopening still proves the cache's entire state is reconstructible.
        Err(error) if cfg!(windows) && error.kind() == std::io::ErrorKind::PermissionDenied => {}
        Err(error) => return Err(error.into()),
    }
    drop(disk);
    if root.0.join(FILE_NAME).exists() {
        std::fs::remove_file(root.0.join(FILE_NAME))?;
    }
    let mut disk = TileDisk::open(&root.0, FILE_HEADER_BYTES + SLOT_BYTES)?;
    assert!(disk.get(key(1))?.is_none());
    assert!(disk.put(key(1), &pixels(1))?);
    Ok(())
}

#[cfg(unix)]
#[test]
fn links_and_writable_directories_are_refused() -> Result<(), AssetError> {
    use std::os::unix::{fs::PermissionsExt, fs::symlink};
    let root = TestRoot::new();
    tack_storage::create_private_directory(&root.0, true)?;
    std::fs::set_permissions(&root.0, std::fs::Permissions::from_mode(0o777))?;
    assert!(TileDisk::open(&root.0, FILE_HEADER_BYTES + SLOT_BYTES).is_err());
    std::fs::set_permissions(&root.0, std::fs::Permissions::from_mode(0o700))?;
    symlink(root.0.join("missing"), root.0.join(FILE_NAME))?;
    assert!(TileDisk::open(&root.0, FILE_HEADER_BYTES + SLOT_BYTES).is_err());
    std::fs::remove_file(root.0.join(FILE_NAME))?;
    if root.0.join(LOCK_NAME).exists() {
        std::fs::remove_file(root.0.join(LOCK_NAME))?;
    }
    symlink(root.0.join("missing"), root.0.join(LOCK_NAME))?;
    assert!(TileDisk::open(&root.0, FILE_HEADER_BYTES + SLOT_BYTES).is_err());
    std::fs::remove_file(root.0.join(LOCK_NAME))?;
    let alias = root.0.join("alias");
    symlink(&root.0, &alias)?;
    assert!(TileDisk::open(&alias.join("nested"), SLOT_BYTES).is_err());
    Ok(())
}

#[cfg(unix)]
#[test]
fn file_or_lock_replacement_revokes_old_owner() -> Result<(), AssetError> {
    use std::os::unix::fs::OpenOptionsExt;
    let root = TestRoot::new();
    let mut disk = TileDisk::open(&root.0, FILE_HEADER_BYTES + SLOT_BYTES)?;
    assert!(disk.put(key(1), &pixels(1))?);
    std::fs::remove_file(root.0.join(LOCK_NAME))?;
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(root.0.join(LOCK_NAME))?;
    assert!(disk.get(key(1)).is_err());
    drop(disk);
    let mut disk = TileDisk::open(&root.0, FILE_HEADER_BYTES + SLOT_BYTES)?;
    std::fs::remove_file(root.0.join(FILE_NAME))?;
    File::create(root.0.join(FILE_NAME))?.set_len((FILE_HEADER_BYTES + SLOT_BYTES) as u64)?;
    assert!(disk.get(key(1)).is_err());
    Ok(())
}

#[test]
fn a_private_original_hardlinked_as_raw_cache_is_refused_before_truncation()
-> Result<(), AssetError> {
    #[cfg(unix)]
    use std::os::unix::fs::OpenOptionsExt;
    let root = TestRoot::new();
    tack_storage::create_private_directory(&root.0, false)?;
    let original = root.0.join("authoritative-original.jpg");
    let mut authority = Vec::new();
    image::codecs::jpeg::JpegEncoder::new(&mut authority).encode(
        &[37; 18],
        3,
        2,
        image::ExtendedColorType::Rgb8,
    )?;
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    options.open(&original)?.write_all(&authority)?;
    std::fs::hard_link(&original, root.0.join(FILE_NAME))?;
    assert!(TileDisk::open(&root.0, FILE_HEADER_BYTES + SLOT_BYTES).is_err());
    assert_eq!(std::fs::read(&original)?, authority);
    assert_eq!(std::fs::metadata(&original)?.len(), authority.len() as u64);
    Ok(())
}

#[test]
fn existing_empty_or_invalid_global_header_never_gets_mutated() -> Result<(), AssetError> {
    for bytes in [
        Vec::new(),
        vec![0; 19],
        b"TACKRAW1old-slot-layout".to_vec(),
        {
            let mut header = file_header().to_vec();
            header[15] ^= 1;
            header
        },
    ] {
        let root = TestRoot::new();
        tack_storage::create_private_directory(&root.0, false)?;
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        options.open(root.0.join(FILE_NAME))?.write_all(&bytes)?;
        assert!(TileDisk::open(&root.0, FILE_HEADER_BYTES + SLOT_BYTES).is_err());
        assert_eq!(std::fs::read(root.0.join(FILE_NAME))?, bytes);
    }
    Ok(())
}

#[test]
fn global_header_corruption_disables_reuse_until_disposable_file_is_deleted()
-> Result<(), AssetError> {
    let root = TestRoot::new();
    let mut disk = TileDisk::open(&root.0, FILE_HEADER_BYTES + SLOT_BYTES)?;
    assert!(disk.put(key(1), &pixels(1))?);
    disk.file.seek(SeekFrom::Start(0))?;
    disk.file.write_all(b"X")?;
    let corrupt = std::fs::read(root.0.join(FILE_NAME))?;
    assert!(disk.get(key(1)).is_err());
    assert!(disk.put(key(2), &pixels(2)).is_err());
    drop(disk);
    assert!(TileDisk::open(&root.0, FILE_HEADER_BYTES + SLOT_BYTES).is_err());
    assert_eq!(std::fs::read(root.0.join(FILE_NAME))?, corrupt);
    std::fs::remove_file(root.0.join(FILE_NAME))?;
    let mut disk = TileDisk::open(&root.0, FILE_HEADER_BYTES + SLOT_BYTES)?;
    assert!(disk.get(key(1))?.is_none());
    assert!(disk.put(key(1), &pixels(1))?);
    Ok(())
}

#[cfg(unix)]
#[test]
fn raw_or_lock_hardlink_added_after_open_prevents_further_mutation() -> Result<(), AssetError> {
    for name in [FILE_NAME, LOCK_NAME] {
        let root = TestRoot::new();
        let mut disk = TileDisk::open(&root.0, FILE_HEADER_BYTES + SLOT_BYTES)?;
        assert!(disk.put(key(1), &pixels(1))?);
        let original = root.0.join("external-authority");
        std::fs::hard_link(root.0.join(name), &original)?;
        let before = std::fs::read(&original)?;
        assert!(disk.get(key(1)).is_err()); // Recency must not write either.
        assert!(disk.put(key(2), &pixels(2)).is_err());
        assert_eq!(std::fs::read(&original)?, before);
        assert_eq!(disk.entries(), 1);
    }
    Ok(())
}

/// Finite, warm-OS-page-cache comparison; no claim of a cold SSD benchmark.
/// Optionally set TACK_TILE_BENCH_PNG to a real PNG tile (at most 258 per axis).
#[test]
#[ignore = "serial measured cache comparison; run explicitly with --nocapture"]
fn raw_cache_roundtrip_comparison() -> Result<(), AssetError> {
    use std::{hint::black_box, time::Instant};
    let root = TestRoot::new();
    let supplied = std::env::var_os("TACK_TILE_BENCH_PNG");
    let cases: Vec<_> = if let Some(path) = supplied {
        let path = PathBuf::from(path);
        if std::fs::metadata(&path)?.len() > 1024 * 1024 {
            return Err("comparison expects one small PNG tile".into());
        }
        let dimensions = image::ImageReader::open(&path)?
            .with_guessed_format()?
            .into_dimensions()?;
        if payload_size(dimensions.0, dimensions.1).is_none() {
            return Err("comparison tile dimensions exceed cache bounds".into());
        }
        let image = image::open(path)?.into_rgba8();
        vec![(
            "supplied_tile",
            Decoded {
                width: image.width(),
                height: image.height(),
                rgba: image.into_raw(),
            },
        )]
    } else {
        let mut state = 19_u32;
        let noisy = (0..256 * 256 * 4)
            .map(|index| {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                if index % 4 == 3 { 255 } else { state as u8 }
            })
            .collect();
        vec![
            (
                "flat_256",
                Decoded {
                    width: 256,
                    height: 256,
                    rgba: vec![97; 256 * 256 * 4],
                },
            ),
            (
                "noise_256",
                Decoded {
                    width: 256,
                    height: 256,
                    rgba: noisy,
                },
            ),
        ]
    };
    let mut receipts = Vec::new();
    for (name, image) in cases {
        let mut disk = TileDisk::open(&root.0, FILE_HEADER_BYTES + SLOT_BYTES)?;
        let image_buffer =
            image::RgbaImage::from_raw(image.width, image.height, image.rgba.clone())
                .ok_or("invalid comparison tile")?;
        let started = Instant::now();
        let png = crate::representation::encode(&image_buffer)?;
        let png_encode_ms = started.elapsed().as_secs_f64() * 1000.0;
        let png_path = root.0.join("comparison.png");
        std::fs::write(&png_path, &png)?;
        let started = Instant::now();
        if !disk.put(key(1), &image)? {
            return Err("comparison tile exceeds cache bounds".into());
        }
        let raw_put_ms = started.elapsed().as_secs_f64() * 1000.0;
        drop(disk);
        let started = Instant::now();
        let mut disk = TileDisk::open(&root.0, FILE_HEADER_BYTES + SLOT_BYTES)?;
        let reopen_ms = started.elapsed().as_secs_f64() * 1000.0;
        let expected_crc = crc32fast::hash(&image.rgba);
        let mut png_ms = Vec::new();
        let mut raw_ms = Vec::new();
        // Alternate order across trials; both include file I/O and validation.
        for trial in 0..64 {
            for route in if trial & 1 == 0 { [0, 1] } else { [1, 0] } {
                let started = Instant::now();
                let decoded = if route == 0 {
                    image::load_from_memory_with_format(
                        &std::fs::read(&png_path)?,
                        image::ImageFormat::Png,
                    )?
                    .into_rgba8()
                    .into_raw()
                } else {
                    disk.get(key(1))?.ok_or("missing comparison raw tile")?.rgba
                };
                let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
                assert_eq!(crc32fast::hash(&decoded), expected_crc);
                black_box(decoded);
                if route == 0 {
                    png_ms.push(elapsed_ms);
                } else {
                    raw_ms.push(elapsed_ms);
                }
            }
        }
        png_ms.sort_by(f64::total_cmp);
        raw_ms.sort_by(f64::total_cmp);
        receipts.push(serde_json::json!({
            "case": name, "dimensions": [image.width, image.height], "crc32": expected_crc,
            "trials": 64, "raw_payload_bytes": image.rgba.len(), "raw_slot_bytes": SLOT_BYTES, "raw_global_header_bytes": FILE_HEADER_BYTES,
            "png_bytes": png.len(), "png_encode_ms": png_encode_ms, "raw_put_ms": raw_put_ms,
            "raw_reopen_ms": reopen_ms, "png_get_p50_ms": png_ms[32], "png_get_p95_ms": png_ms[60],
            "raw_get_p50_ms": raw_ms[32], "raw_get_p95_ms": raw_ms[60],
        }));
    }
    println!(
        "{}",
        serde_json::json!({
            "comparison": "raw_checked_slots_vs_png_files", "os_page_cache": "warm",
            "codec": "existing image PNG decoder and representation encoder", "cases": receipts,
        })
    );
    Ok(())
}
