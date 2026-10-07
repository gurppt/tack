//! Compact packaged UI PNG decode, explicitly requested on a worker.
use crate::{AssetError, Decoded};
use image::{ImageFormat, ImageReader, Limits};
use std::io::Cursor;

pub fn decode_ui_png(bytes: &[u8]) -> Result<Decoded, AssetError> {
    if bytes.is_empty() || bytes.len() > 128 * 1024 {
        return Err("About artwork is missing or exceeds its packaged size limit".into());
    }
    let mut reader = ImageReader::with_format(Cursor::new(bytes), ImageFormat::Png);
    let mut limits = Limits::default();
    limits.max_image_width = Some(256);
    limits.max_image_height = Some(256);
    limits.max_alloc = Some(1024 * 1024);
    reader.limits(limits);
    let image = reader.decode()?.into_rgba8();
    if image.width() == 0 || image.height() == 0 {
        return Err("Empty About artwork".into());
    }
    Ok(Decoded {
        width: image.width(),
        height: image.height(),
        rgba: image.into_raw(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::ImageEncoder;

    fn png(width: u32, height: u32) -> Result<Vec<u8>, AssetError> {
        let mut bytes = Vec::new();
        image::codecs::png::PngEncoder::new(&mut bytes).write_image(
            &vec![255; width as usize * height as usize * 4],
            width,
            height,
            image::ExtendedColorType::Rgba8,
        )?;
        Ok(bytes)
    }
    #[test]
    fn decoder_enforces_dimensions_before_accepting_compact_payload() -> Result<(), AssetError> {
        for (width, height) in [(257, 1), (1, 257)] {
            let encoded = png(width, height)?;
            assert!(encoded.len() < 128 * 1024);
            assert!(decode_ui_png(&encoded).is_err());
        }
        let image = decode_ui_png(&png(256, 256)?)?;
        assert_eq!(image.rgba.len(), 256 * 256 * 4);
        Ok(())
    }
}
