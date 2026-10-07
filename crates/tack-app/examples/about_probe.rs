//! Explicit developer probe; no instrumentation in the ordinary window path.
use std::time::Instant;
use tack_app::about;
use tack_assets::AssetError;
const IMAGE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/about.png"));

fn main() -> Result<(), AssetError> {
    let mut samples = Vec::new();
    for _ in 0..51 {
        let start = Instant::now();
        let image = about::decode_image(IMAGE)?;
        std::hint::black_box(&image);
        samples.push(start.elapsed().as_secs_f64() * 1000.);
    }
    let first = samples.remove(0);
    samples.sort_by(f64::total_cmp);
    let metadata = &about::METADATA;
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "scope": "explicit release probe; isolated PNG CPU decode, no GPU upload",
            "name": metadata.name, "version": metadata.version, "author": metadata.author,
            "website": metadata.website, "contact": metadata.contact, "license": metadata.license,
            "encoded_bytes": IMAGE.len(), "image_size": about::IMAGE_SIZE,
            "decoded_rgba_bytes": about::IMAGE_SIZE[0] * about::IMAGE_SIZE[1] * 4,
            "first_decode_ms": first, "warm_decode_p50_ms": samples[25],
            "warm_decode_p99_ms": samples[49], "samples": samples,
        }))?
    );
    Ok(())
}
