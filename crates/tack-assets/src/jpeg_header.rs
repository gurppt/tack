//! The bounded route validates and pins the complete header before native parsing.
//! An interleaved baseline scan does not allocate full-image coefficient arrays.
use crate::AssetError;
use std::io::Read;

pub const HEADER_LIMIT: usize = 1024 * 1024;
pub const CODEC_MAX_AXIS: u32 = 65_500; // libjpeg-turbo JPEG_MAX_DIMENSION.
pub const ENCODED_LIMIT: u64 = 256 * 1024 * 1024;

#[derive(Debug)]
pub struct Header {
    pub size: [u32; 2],
    pub prefix: Vec<u8>,
    pub baseline_interleaved: bool,
    pub scratch_bound: u64,
}

impl Header {
    pub fn ordinary(&self) -> bool {
        // Preserve the existing native ordinary route, but select using both
        // dimension and working-set risk; exceeding it routes to scanlines.
        self.size[0] <= 6000
            && self.size[1] <= 4500
            && u64::from(self.size[0]) * u64::from(self.size[1]) <= 27_000_000
    }
    pub fn require_streamed(&self) -> Result<(), AssetError> {
        if !self.baseline_interleaved {
            return Err("bounded huge JPEG requires 8-bit single-scan baseline RGB/grayscale; progressive/multiscan requires a coefficient-buffer strategy".into());
        }
        if self.scratch_bound > 16 * 1024 * 1024 {
            return Err("JPEG scanline scratch exceeds 16 MiB bound".into());
        }
        Ok(())
    }
}

fn capture<R: Read>(source: &mut R, prefix: &mut Vec<u8>, count: usize) -> Result<(), AssetError> {
    let end = prefix
        .len()
        .checked_add(count)
        .ok_or("JPEG header arithmetic")?;
    if end > HEADER_LIMIT {
        return Err("JPEG marker header exceeds 1 MiB parser budget".into());
    }
    prefix.try_reserve(count)?;
    let start = prefix.len();
    prefix.resize(end, 0);
    source.read_exact(&mut prefix[start..])?;
    Ok(())
}

/// Stops immediately after the first SOS. The prefix is replayed verbatim into
/// the child, avoiding a header-validation/use race on mutable linked files.
pub fn read<R: Read>(source: &mut R) -> Result<Header, AssetError> {
    let mut prefix = Vec::new();
    capture(source, &mut prefix, 2)?;
    if prefix != [0xff, 0xd8] {
        return Err("JPEG SOI marker missing".into());
    }
    let mut frame = None;
    loop {
        capture(source, &mut prefix, 1)?;
        if prefix.last() != Some(&0xff) {
            return Err("invalid JPEG marker boundary".into());
        }
        loop {
            capture(source, &mut prefix, 1)?;
            if prefix.last() != Some(&0xff) {
                break;
            }
        }
        let marker = *prefix.last().ok_or("JPEG marker missing")?;
        if matches!(marker, 0 | 0xd8 | 0xd9 | 0xd0..=0xd7 | 1) {
            return Err("invalid standalone JPEG header marker".into());
        }
        capture(source, &mut prefix, 2)?;
        let n = prefix.len();
        let length = usize::from(u16::from_be_bytes([prefix[n - 2], prefix[n - 1]]));
        if length < 2 {
            return Err("invalid JPEG marker length".into());
        }
        capture(source, &mut prefix, length - 2)?;
        let payload = &prefix[n..];
        if matches!(marker, 0xc0..=0xcf) && !matches!(marker, 0xc4 | 0xc8 | 0xcc) {
            if frame.is_some() || payload.len() < 6 {
                return Err("invalid JPEG frame header".into());
            }
            let size = [
                u32::from(u16::from_be_bytes([payload[3], payload[4]])),
                u32::from(u16::from_be_bytes([payload[1], payload[2]])),
            ];
            let components = usize::from(payload[5]);
            if size.contains(&0) || size.iter().any(|n| *n > CODEC_MAX_AXIS) {
                return Err("JPEG dimensions exceed the codec's 65500-axis limit".into());
            }
            if ![1, 3].contains(&components) || payload.len() != 6 + 3 * components {
                return Err("JPEG must have one or three components".into());
            }
            let mut ids = Vec::new();
            let mut sampling = 0u64;
            for c in payload[6..].chunks_exact(3) {
                let (h, v) = (c[1] >> 4, c[1] & 15);
                if ids.contains(&c[0]) || h == 0 || v == 0 || h > 4 || v > 4 || c[2] > 3 {
                    return Err("invalid JPEG component/sampling layout".into());
                }
                sampling = sampling
                    .checked_add(u64::from(h) * u64::from(v))
                    .ok_or("JPEG MCU arithmetic")?;
                ids.push(c[0]);
            }
            if sampling > 10 {
                return Err("JPEG MCU exceeds codec block limit".into());
            }
            frame = Some((size, ids, marker == 0xc0 && payload[0] == 8));
        } else if marker == 0xda {
            let (size, ids, baseline) = frame.ok_or("JPEG SOS precedes frame")?;
            if payload.is_empty() {
                return Err("invalid JPEG scan header".into());
            }
            let count = usize::from(payload[0]);
            if count == 0 || count > ids.len() || payload.len() != 1 + 2 * count + 3 {
                return Err("invalid JPEG scan component count".into());
            }
            let mut scan = Vec::new();
            for c in payload[1..1 + 2 * count].chunks_exact(2) {
                if !ids.contains(&c[0]) || scan.contains(&c[0]) || c[1] >> 4 > 3 || c[1] & 15 > 3 {
                    return Err("invalid JPEG scan component/table".into());
                }
                scan.push(c[0]);
            }
            let tail = &payload[1 + 2 * count..];
            let baseline_interleaved = baseline && count == ids.len() && tail == [0, 63, 0];
            // Conservative bound: full-width component iMCU sample rows,
            // upsampler/context rows, PNM row and decoder tables. No pixels*height.
            let scratch_bound = u64::from(size[0])
                .checked_mul(192)
                .and_then(|n| n.checked_add(1024 * 1024))
                .ok_or("JPEG scratch arithmetic")?;
            return Ok(Header {
                size,
                prefix,
                baseline_interleaved,
                scratch_bound,
            });
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    fn header(marker: u8, width: u16, count: u8) -> Vec<u8> {
        let mut b = vec![255, 216, 255, marker, 0, 17, 8, 0, 32];
        b.extend(width.to_be_bytes());
        b.extend([
            3,
            1,
            0x22,
            0,
            2,
            0x11,
            1,
            3,
            0x11,
            1,
            255,
            218,
            0,
            (6 + 2 * count),
            count,
        ]);
        for i in 1..=count {
            b.extend([i, 0]);
        }
        b.extend([0, 63, 0]);
        b
    }
    #[test]
    fn wide_baseline_has_row_bound_and_pinned_header_but_progressive_and_split_scans_refuse() {
        let bytes = header(0xc0, 50_000, 3);
        let h = read(&mut bytes.as_slice()).unwrap();
        assert_eq!(h.size, [50_000, 32]);
        assert!(!h.ordinary());
        assert!(h.require_streamed().is_ok());
        assert!(h.scratch_bound < 11 * 1024 * 1024);
        assert_eq!(h.prefix, bytes);
        for bytes in [header(0xc2, 50_000, 3), header(0xc0, 50_000, 1)] {
            assert!(
                read(&mut bytes.as_slice())
                    .unwrap()
                    .require_streamed()
                    .is_err()
            );
        }
        assert!(read(&mut header(0xc0, 65_501, 3).as_slice()).is_err());
    }
    #[test]
    fn malformed_components_lengths_and_marker_storm_stop_before_native() {
        let mut bytes = header(0xc0, 512, 3);
        bytes[13] = 0xf1;
        assert!(read(&mut bytes.as_slice()).is_err());
        assert!(read(&mut [255, 216, 255, 224, 0, 1].as_slice()).is_err());
        let mut storm = vec![255, 216];
        storm.resize(HEADER_LIMIT + 1, 255);
        assert!(
            read(&mut storm.as_slice())
                .unwrap_err()
                .to_string()
                .contains("budget")
        );
    }
}
