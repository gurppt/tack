//! Opt-in bounded header evidence. No pixel decode, file paths or raw error text.
use serde_json::{Value, json};
use std::{
    fs::File,
    io::{BufReader, Read},
    path::Path,
};
pub fn diagnostic_header(path: &Path) -> Value {
    if !tack_storage::diagnostics::enabled() {
        return Value::Null;
    }
    let Ok(mut file) = File::open(path) else {
        return json!({"format":"unavailable"});
    };
    let mut signature = [0; 33];
    let mut filled = 0;
    while filled < signature.len() {
        match file.read(&mut signature[filled..]) {
            Ok(0) | Err(_) => break,
            Ok(n) => filled += n,
        }
    }
    if filled >= 2 && signature[..2] == [255, 216] {
        let input = std::io::Cursor::new(&signature[..filled]).chain(file);
        return match crate::jpeg_header::read(&mut BufReader::with_capacity(8192, input)) {
            Ok(h) => json!({"format":"jpeg","pixels":h.size,"precision":h.precision,
                "progressive":h.frame_marker==0xc2,"baseline_interleaved":h.baseline_interleaved,
                "scratch_bound":h.scratch_bound,"header_limit":crate::jpeg_header::HEADER_LIMIT}),
            Err(_) => json!({"format":"jpeg","header":"unavailable within bounded parser"}),
        };
    }
    if filled >= 33 && signature[..8] == *b"\x89PNG\r\n\x1a\n" && signature[12..16] == *b"IHDR" {
        return json!({"format":"png","header_only":true,
            "pixels":[u32::from_be_bytes([signature[16],signature[17],signature[18],signature[19]]),
                u32::from_be_bytes([signature[20],signature[21],signature[22],signature[23]])],
            "bit_depth":signature[24],"color_type":signature[25],"interlace":signature[28]});
    }
    if filled >= 6 && (&signature[..6] == b"GIF87a" || &signature[..6] == b"GIF89a") {
        return json!({"format":"gif","supported":false});
    }
    json!({"format":"other/unknown"})
}
pub fn image_failure_kind(error: &crate::AssetError) -> &'static str {
    match error.downcast_ref::<image::ImageError>() {
        Some(image::ImageError::Unsupported(_)) => "unsupported-format",
        Some(image::ImageError::Limits(_)) => "decoder-resource-limit",
        Some(image::ImageError::Decoding(_)) => "malformed-image",
        Some(image::ImageError::IoError(_)) => "decoder-io",
        _ => "other",
    }
}
pub fn representation_failure_reason(error: &crate::AssetError) -> &'static str {
    match error.to_string().as_str() {
        "JPEG exceeds 64 MiB decode input budget" => {
            "JPEG exceeds the 64 MiB ordinary decode-input limit"
        }
        "JPEG progressive/multiscan coefficient scratch exceeds 192 MiB bound" => {
            "JPEG progressive/multiscan coefficient scratch exceeds 192 MiB"
        }
        "prototype JPEG source must be lossy RGB or grayscale" => {
            "This JPEG precision/colorspace or coding variant is unsupported"
        }
        "bounded huge JPEG requires 8-bit single-scan baseline RGB/grayscale; progressive/multiscan requires a coefficient-buffer strategy" => {
            "Large JPEG progressive/multiscan is unsupported by the bounded decoder"
        }
        "interlaced PNG unsupported by streamed prototype" => {
            "Large interlaced PNG is unsupported by the streamed decoder"
        }
        "source changed during derivation" => "Source changed while preparing this representation",
        "source missing or changed; explicit relink/revision required" => {
            "Source is missing or changed; refresh or relink required"
        }
        "image parser read budget exhausted" => "Encoded read budget exhausted",
        _ => match image_failure_kind(error) {
            "unsupported-format" => "Image format is unsupported",
            "decoder-resource-limit" => "Decoder resource limit exceeded",
            "malformed-image" => "Image data is invalid or truncated",
            "decoder-io" => "Image data could not be read",
            _ => "Decoder could not prepare this representation",
        },
    }
}
