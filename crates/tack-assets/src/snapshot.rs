//! Explicit bounded raster export adapters; reuse the production decoder and PNG encoder.
use crate::{AssetError, Decoded};
use std::{
    fs::File,
    path::Path,
    sync::{Arc, atomic::AtomicBool},
};
use tack_storage::{Payload, RangeReader};
pub fn decode(payload: &Payload, edge: u32, cancel: &AtomicBool) -> Result<Decoded, AssetError> {
    if ![128, 512, 2048].contains(&edge) {
        return Err("Unsupported snapshot display edge".into());
    }
    let pixels = match payload {
        Payload::File(path) => {
            let mut file = File::open(path)?;
            let len = file.metadata()?.len();
            crate::representation::derive_stream_edge_known(
                &mut file,
                edge,
                Some(cancel),
                Some(len),
            )?
            .0
        }
        Payload::Stored { file, range } => {
            crate::representation::derive_stream_edge_known(
                &mut RangeReader::new(Arc::clone(file), *range),
                edge,
                Some(cancel),
                Some(range.len),
            )?
            .0
        }
    };
    Ok(Decoded {
        width: pixels.width(),
        height: pixels.height(),
        rgba: pixels.into_raw(),
    })
}
pub fn write_png(pixels: &Decoded, path: &Path) -> Result<(), AssetError> {
    use image::ImageEncoder;
    let mut output = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    image::codecs::png::PngEncoder::new(&mut output).write_image(
        &pixels.rgba,
        pixels.width,
        pixels.height,
        image::ExtendedColorType::Rgba8,
    )?;
    output.sync_all()?;
    Ok(())
}
