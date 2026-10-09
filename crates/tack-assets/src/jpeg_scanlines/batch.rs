//! One bounded rectangular native crop scattered directly into requested tiles.
use super::*;

/// Fifteen full 258² guttered RGBA tiles fit the aggregate 4 MiB ceiling.
pub(crate) const MAX_BATCH_TILES: usize = 15;
const REGION_BYTES: u64 = 4 * 1024 * 1024;

struct Batch {
    region: Plan,
    tiles: Vec<Plan>,
}

fn batch_plan(size: [u32; 2], edges: &[u32]) -> Result<Batch, AssetError> {
    if edges.is_empty() || edges.len() > MAX_BATCH_TILES {
        return Err("JPEG regional tile count exceeds bounded batch policy".into());
    }
    let addresses = edges
        .iter()
        .map(|&edge| {
            Tile::from_tag(edge).ok_or_else(|| "JPEG regional batch requires tile addresses".into())
        })
        .collect::<Result<Vec<_>, AssetError>>()?;
    let mip = addresses[0].mip;
    let mut low = [u32::MAX; 2];
    let mut high = [0; 2];
    for (index, tile) in addresses.iter().enumerate() {
        if tile.mip != mip || addresses[..index].contains(tile) {
            return Err("JPEG regional batch requires distinct tiles at one mip".into());
        }
        for (axis, value) in [tile.x, tile.y].into_iter().enumerate() {
            low[axis] = low[axis].min(value);
            high[axis] = high[axis].max(value);
        }
    }
    // A filled tile rectangle excludes gaps and distant requests before I/O.
    if u64::from(high[0] - low[0] + 1) * u64::from(high[1] - low[1] + 1) != edges.len() as u64 {
        return Err("JPEG regional batch is not a contiguous tile rectangle".into());
    }
    let mut tiles = edges
        .iter()
        .map(|&edge| plan(size, edge))
        .collect::<Result<Vec<_>, _>>()?;
    let bytes = tiles
        .iter()
        .map(|p| u64::from(p.output[0]) * u64::from(p.output[1]) * 4)
        .sum::<u64>();
    if bytes > REGION_BYTES {
        return Err("JPEG regional tile output exceeds 4 MiB".into());
    }
    let start = std::array::from_fn(|i| tiles.iter().map(|p| p.crop_start[i]).min().unwrap_or(0));
    let end: [u32; 2] = std::array::from_fn(|i| {
        tiles
            .iter()
            .map(|p| p.crop_start[i] + p.native[i])
            .max()
            .unwrap_or(0)
    });
    let native = std::array::from_fn(|i| end[i] - start[i]);
    // Charge the rectangle as RGBA even though only one RGB row is allocated.
    if u64::from(native[0]) * u64::from(native[1]) * 4 > REGION_BYTES {
        return Err("JPEG regional crop exceeds 4 MiB".into());
    }
    let scale = tiles[0].scale;
    for tile in &mut tiles {
        tile.crop_start = start;
    }
    Ok(Batch {
        region: Plan {
            scale,
            native,
            output: native,
            crop: Some([native[0], native[1], start[0], start[1]]),
            stride: None,
            start: [0; 2],
            crop_start: start,
            mip_limit: [0; 2],
        },
        tiles,
    })
}

/// Pure admission shared with the scheduler; performs no source or helper I/O.
pub(crate) fn admissible_tiles(size: [u32; 2], edges: &[u32]) -> bool {
    batch_plan(size, edges).is_ok()
}

/// Output order matches `edges`. One source/header and mip, one native helper
/// invocation, at most 15 tiles and 4 MiB aggregate RGBA/crop footprint. The
/// crop is streamed one row at a time; no rectangular bitmap is materialized.
pub(crate) fn derive_tiles<R: Read + Send>(
    source: &mut R,
    header: Header,
    edges: &[u32],
    cancel: Option<&AtomicBool>,
) -> Result<Vec<image::RgbaImage>, AssetError> {
    let batch = batch_plan(header.size, edges)?;
    run(source, header, &batch.region, cancel, |reader| {
        raster_many(reader, batch.region.native, &batch.tiles, cancel)
    })
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod comparison;
