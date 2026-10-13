//! Policy witnesses use declared fingerprint lengths; large-file proof is separate.
use std::{fs, path::PathBuf};
use tack_assets::{AssetError, image_import_plan};
use turbojpeg::{Compressor, Image, PixelFormat, Subsamp};
struct Owned(PathBuf);
impl Drop for Owned {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
fn jpeg(progressive: bool) -> Result<Owned, AssetError> {
    let pixels = vec![91; 96 * 64 * 3];
    let mut encoder = Compressor::new()?;
    encoder.set_quality(85)?;
    encoder.set_progressive(progressive)?;
    encoder.set_subsamp(Subsamp::Sub2x2)?;
    let encoded = encoder.compress_to_vec(Image {
        pixels: &pixels,
        width: 96,
        height: 64,
        pitch: 96 * 3,
        format: PixelFormat::RGB,
    })?;
    let p = std::env::temp_dir().join(format!(
        "tack-policy-{}.jpg",
        tack_storage::new_document_id()?.value()
    ));
    use std::io::Write;
    fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&p)?
        .write_all(&encoded)?;
    Ok(Owned(p))
}
#[test]
fn larger_encoded_baseline_selects_rows_without_raising_native_whole_input_limit()
-> Result<(), AssetError> {
    let p = jpeg(false)?;
    let ordinary = image_import_plan(&p.0, fs::metadata(&p.0)?.len())?;
    assert!(!ordinary.streamed);
    assert_eq!(ordinary.max_encoded, 64 * 1024 * 1024);
    let larger = image_import_plan(&p.0, 87_000_000)?;
    assert!(larger.streamed);
    assert_eq!(larger.pixels, [96, 64]);
    assert!(larger.max_encoded > 87_000_000 && larger.max_encoded < 256 * 1024 * 1024);
    Ok(())
}
#[test]
fn progressive_larger_original_refuses_before_whole_encoded_allocation() -> Result<(), AssetError> {
    let p = jpeg(true)?;
    assert!(!image_import_plan(&p.0, fs::metadata(&p.0)?.len())?.streamed);
    let error = image_import_plan(&p.0, 87_000_000)
        .err()
        .ok_or("progressive oversized policy did not refuse")?;
    assert!(error.to_string().contains("progressive/multiscan"));
    Ok(())
}
