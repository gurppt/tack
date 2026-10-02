//! Isolated measurement reference only; the application's Thumbnail path has no
//! decoder switch or fallback. Both paths perform the loader's exact final resize.
use std::{fs, io::Cursor, path::PathBuf, time::Instant};
use tack_assets::{AssetError, NativeThumbnail};

fn rss_kib() -> Option<u64> {
    fs::read_to_string("/proc/self/status")
        .ok()?
        .lines()
        .find(|line| line.starts_with("VmRSS:"))?
        .split_whitespace()
        .nth(1)?
        .parse()
        .ok()
}

fn measure(mode: &str, bytes: &[u8]) -> Result<(image::RgbaImage, serde_json::Value), AssetError> {
    let start = Instant::now();
    let (original, header_ms, decode_ms) = if mode == "native" {
        let decoder = NativeThumbnail::new(bytes)?;
        let header_ms = start.elapsed().as_secs_f64() * 1000.;
        let decode_start = Instant::now();
        let image = decoder.decode()?;
        (
            image::DynamicImage::ImageRgb8(image),
            header_ms,
            decode_start.elapsed().as_secs_f64() * 1000.,
        )
    } else if mode == "reference" {
        let mut decoder = jpeg_decoder::Decoder::new(Cursor::new(bytes));
        decoder.set_max_decoding_buffer_size(192 * 1024 * 1024);
        decoder.read_info()?;
        decoder.scale(128, 128)?;
        let header_ms = start.elapsed().as_secs_f64() * 1000.;
        let decode_start = Instant::now();
        let pixels = decoder.decode()?;
        let info = decoder.info().ok_or("JPEG info missing")?;
        let original = match info.pixel_format {
            jpeg_decoder::PixelFormat::RGB24 => image::DynamicImage::ImageRgb8(
                image::RgbImage::from_raw(info.width.into(), info.height.into(), pixels)
                    .ok_or("invalid RGB")?,
            ),
            jpeg_decoder::PixelFormat::L8 => image::DynamicImage::ImageLuma8(
                image::GrayImage::from_raw(info.width.into(), info.height.into(), pixels)
                    .ok_or("invalid grayscale")?,
            ),
            _ => return Err("reference requires RGB/grayscale".into()),
        };
        (
            original,
            header_ms,
            decode_start.elapsed().as_secs_f64() * 1000.,
        )
    } else {
        return Err("choose native or reference".into());
    };
    let dimensions = [original.width(), original.height()];
    let decoded_bytes = original.as_bytes().len();
    let resize_start = Instant::now();
    let thumb = original.thumbnail(128, 128).into_rgba8();
    let resize_ms = resize_start.elapsed().as_secs_f64() * 1000.;
    let total_ms = start.elapsed().as_secs_f64() * 1000.;
    let row = serde_json::json!({"header_ms":header_ms,"decode_ms":decode_ms,
        "resize_ms":resize_ms,"total_ms":total_ms,"decoded_dimensions":dimensions,
        "decoded_bytes":decoded_bytes,"thumbnail_dimensions":[thumb.width(),thumb.height()],
        "thumbnail_bytes":thumb.as_raw().len(),"rss_kib_after_decode":rss_kib()});
    Ok((thumb, row))
}

fn main() -> Result<(), AssetError> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 5 {
        return Err(
            "usage: thumbnail_microbench native|reference sources-dir repetitions output.json"
                .into(),
        );
    }
    let mode = &args[1];
    let repetitions: usize = args[3].parse()?;
    if !(1..=100).contains(&repetitions) {
        return Err("repetitions must be 1..=100".into());
    }
    let mut paths: Vec<PathBuf> = fs::read_dir(&args[2])?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "jpg"))
        .collect();
    paths.sort();
    paths.truncate(8);
    if paths.is_empty() {
        return Err("JPEG source directory is empty".into());
    }
    let mut rows = Vec::new();
    let rss_before = rss_kib();
    let output = PathBuf::from(&args[4]);
    for repetition in 0..repetitions {
        for path in &paths {
            let bytes = fs::read(path)?;
            let (thumb, mut row) = measure(mode, &bytes)?;
            row["source"] = serde_json::json!(path.file_name().and_then(|s| s.to_str()));
            row["iteration"] = serde_json::json!(repetition);
            row["encoded_bytes"] = serde_json::json!(bytes.len());
            rows.push(row);
            if repetition == 0 {
                let name = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .ok_or("bad source filename")?;
                thumb.save(output.with_file_name(format!("{mode}-{name}.png")))?;
            }
        }
    }
    let report = serde_json::json!({"mode":mode,"rows":rows,"rss_before_kib":rss_before,
        "rss_after_kib":rss_kib(),"repetitions":repetitions,
        "limits":"Source read outside timing; total = header + decode + identical image::thumbnail/RGBA. RSS Linux sampled after decode, not peak scratch."});
    fs::write(output, serde_json::to_vec_pretty(&report)?)?;
    Ok(())
}
