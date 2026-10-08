//! Worker-only, disposable libjpeg-turbo tool boundary. No FFI or full frame.
//! New regions scan the sequential compressed stream; derived tiles are reused
//! by the existing disk cache. This does not promise random JPEG region access.
use crate::{
    AssetError,
    huge_image::{TILE_EDGE, Tile},
    jpeg_header::{self, Header},
};
use std::{
    io::{BufRead, BufReader, Read, Write},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

const HELPER_SECONDS: u64 = 30;
const TARGET_BYTES: usize = 16 * 1024 * 1024;

struct Process(Arc<Mutex<Child>>);
impl Drop for Process {
    fn drop(&mut self) {
        if let Ok(mut child) = self.0.lock() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn helper() -> Result<PathBuf, AssetError> {
    let executable = std::env::current_exe()?;
    let directory = executable
        .parent()
        .ok_or("decoder executable directory missing")?;
    let name = if cfg!(windows) {
        "tack-jpeg-decoder.exe"
    } else {
        "tack-jpeg-decoder"
    };
    let path = directory.join(name);
    if !path.is_file() {
        return Err("bounded JPEG helper is missing; restore tack-jpeg-decoder beside Tack".into());
    }
    Ok(path)
}

struct Plan {
    scale: u32,
    native: [u32; 2],
    output: [u32; 2],
    crop: Option<[u32; 4]>,
    stride: Option<u32>,
    start: [u32; 2],
    crop_start: [u32; 2],
    mip_limit: [u32; 2],
}
fn plan(size: [u32; 2], edge: u32) -> Result<Plan, AssetError> {
    if let Some(tile) = Tile::from_tag(edge) {
        let logical = tile.dimensions(size)?;
        let output = logical.map(|n| n + 2);
        let scale = 1u32 << tile.mip.min(3);
        let step = 1u64
            .checked_shl(u32::from(tile.mip))
            .ok_or("JPEG tile scale")?;
        let native_step = u32::try_from(step / u64::from(scale))?;
        let start = [tile.x * TILE_EDGE, tile.y * TILE_EDGE];
        let mip_limit = crate::huge_image::mip_dimensions(size, tile.mip)?.map(|n| n - 1);
        let first = std::array::from_fn::<_, 2, _>(|i| start[i].saturating_sub(1) * native_step);
        // Manual 96-pixel alignment (LCM of 8,16,24,32) covers every
        // admitted sampling factor. Thus libjpeg_crop_scanline cannot silently expand the output leftward.
        let crop_start = [first[0] / 96 * 96, first[1]];
        let last = std::array::from_fn::<_, 2, _>(|i| {
            (start[i] + logical[i]).min(mip_limit[i]) * native_step
        });
        let region = std::array::from_fn(|i| last[i] - crop_start[i] + 1);
        return Ok(Plan {
            scale,
            native: region,
            output,
            crop: Some([region[0], region[1], crop_start[0], crop_start[1]]),
            stride: Some(native_step),
            start,
            crop_start,
            mip_limit,
        });
    }
    if ![8, 16, 32, 64, 128, 512, 2048].contains(&edge) {
        return Err("invalid JPEG representation edge".into());
    }
    let scale = [8, 4, 2, 1]
        .into_iter()
        .find(|n| size[0].max(size[1]).div_ceil(*n) >= edge)
        .unwrap_or(1);
    let native = size.map(|n| n.div_ceil(scale));
    let ratio = f64::from(edge.min(size[0].max(size[1]))) / f64::from(size[0].max(size[1]));
    let output = size.map(|n| (f64::from(n) * ratio).round().max(1.) as u32);
    Ok(Plan {
        scale,
        native,
        output,
        crop: None,
        stride: None,
        start: [0; 2],
        crop_start: [0; 2],
        mip_limit: [0; 2],
    })
}

fn sample(plan: &Plan, axis: usize, pixel: u32) -> u32 {
    match plan.stride {
        Some(step) => {
            (plan.start[axis] + pixel)
                .saturating_sub(1)
                .min(plan.mip_limit[axis])
                * step
                - plan.crop_start[axis]
        }
        None => {
            (u64::from(pixel) * u64::from(plan.native[axis]) / u64::from(plan.output[axis])) as u32
        }
    }
}

fn allocate(bytes: usize) -> Result<Vec<u8>, AssetError> {
    if bytes > TARGET_BYTES {
        return Err("JPEG bounded output allocation exceeded".into());
    }
    let mut v = Vec::new();
    v.try_reserve_exact(bytes)?;
    v.resize(bytes, 0);
    Ok(v)
}

fn raster<R: BufRead>(
    source: &mut R,
    plan: &Plan,
    cancel: Option<&AtomicBool>,
) -> Result<image::RgbaImage, AssetError> {
    // Pinned djpeg P6 header has exactly three lines. No unbounded read_line.
    let mut lines = Vec::new();
    for _ in 0..3 {
        let mut line = Vec::new();
        (&mut *source).take(128).read_until(b'\n', &mut line)?;
        if line.last() != Some(&b'\n') {
            return Err("invalid JPEG helper PNM header".into());
        }
        lines.push(line);
    }
    if lines[0] != b"P6\n" || lines[2] != b"255\n" {
        return Err("JPEG helper output is not RGB8".into());
    }
    let dimensions = std::str::from_utf8(&lines[1])?
        .split_whitespace()
        .map(str::parse::<u32>)
        .collect::<Result<Vec<_>, _>>()?;
    if dimensions.as_slice() != plan.native {
        return Err("JPEG helper crop/output dimensions mismatch".into());
    }
    let row_bytes = usize::try_from(
        u64::from(plan.native[0])
            .checked_mul(3)
            .ok_or("JPEG row arithmetic")?,
    )?;
    if row_bytes > 3 * jpeg_header::CODEC_MAX_AXIS as usize {
        return Err("JPEG native row exceeds bound".into());
    }
    let bytes = usize::try_from(
        u64::from(plan.output[0])
            .checked_mul(u64::from(plan.output[1]))
            .and_then(|n| n.checked_mul(4))
            .ok_or("JPEG output arithmetic")?,
    )?;
    let mut output = allocate(bytes)?;
    let mut row = allocate(row_bytes)?;
    let mut next = 0u32;
    for y in 0..plan.native[1] {
        if cancel.is_some_and(|c| c.load(Ordering::Relaxed)) {
            return Err("JPEG derivation cancelled".into());
        }
        source.read_exact(&mut row)?;
        while next < plan.output[1] && y == sample(plan, 1, next) {
            for x in 0..plan.output[0] {
                let sx = sample(plan, 0, x) as usize;
                let dst = (next as usize * plan.output[0] as usize + x as usize) * 4;
                output[dst..dst + 3].copy_from_slice(&row[sx * 3..sx * 3 + 3]);
                output[dst + 3] = 255;
            }
            next += 1;
        }
    }
    if next != plan.output[1] || !source.fill_buf()?.is_empty() {
        return Err("JPEG helper raster incomplete or has excess pixels".into());
    }
    image::RgbaImage::from_raw(plan.output[0], plan.output[1], output)
        .ok_or_else(|| "JPEG bounded layout".into())
}

fn feed<R: Read>(
    source: &mut R,
    prefix: &[u8],
    mut input: impl Write,
    cancel: Option<&AtomicBool>,
) -> Result<(), AssetError> {
    input.write_all(prefix)?;
    let mut left = jpeg_header::ENCODED_LIMIT
        .checked_sub(prefix.len() as u64)
        .ok_or("JPEG encoded budget")?;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        if cancel.is_some_and(|c| c.load(Ordering::Relaxed)) {
            return Err("JPEG derivation cancelled".into());
        }
        let allowed = buffer.len().min(usize::try_from(left.saturating_add(1))?);
        let count = source.read(&mut buffer[..allowed])?;
        if count == 0 {
            return Ok(());
        }
        if count as u64 > left {
            return Err("JPEG encoded stream exceeds 256 MiB bound".into());
        }
        left -= count as u64;
        input.write_all(&buffer[..count])?;
    }
}

/// At most one child and two short-lived support threads per admitted image job.
/// The watcher interrupts blocked pipe reads as well as scanline work. It exists
/// only while decoding; settled documents acquire no new service or timer.
pub fn derive<R: Read + Send>(
    source: &mut R,
    header: Header,
    edge: u32,
    cancel: Option<&AtomicBool>,
) -> Result<image::RgbaImage, AssetError> {
    header.require_streamed()?;
    if cancel.is_some_and(|c| c.load(Ordering::Relaxed)) {
        return Err("JPEG derivation cancelled".into());
    }
    let plan = plan(header.size, edge)?;
    let mut command = Command::new(helper()?);
    command
        .args([
            "-strict",
            "-maxscans",
            "1",
            "-maxmemory",
            "16384",
            "-rgb",
            "-pnm",
            "-scale",
        ])
        .arg(format!("1/{}", plan.scale));
    if let Some([w, h, x, y]) = plan.crop {
        command.arg("-crop").arg(format!("{w}x{h}+{x}+{y}"));
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let input = child.stdin.take().ok_or("JPEG helper input pipe missing")?;
    let stdout = child
        .stdout
        .take()
        .ok_or("JPEG helper output pipe missing")?;
    let child = Process(Arc::new(Mutex::new(child)));
    let done = AtomicBool::new(false);
    std::thread::scope(|scope| -> Result<_, AssetError> {
        let watched = Arc::clone(&child.0);
        let finished = &done;
        let watcher = std::thread::Builder::new()
            .name("tack-jpeg-watch".into())
            .spawn_scoped(scope, move || {
                let start = Instant::now();
                while !finished.load(Ordering::Acquire) {
                    if cancel.is_some_and(|c| c.load(Ordering::Relaxed))
                        || start.elapsed() >= Duration::from_secs(HELPER_SECONDS)
                    {
                        if let Ok(mut c) = watched.lock() {
                            let _ = c.kill();
                        }
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(5));
                }
            })?;
        let prefix = &header.prefix;
        let feeder = match std::thread::Builder::new()
            .name("tack-jpeg-feed".into())
            .spawn_scoped(scope, move || feed(source, prefix, input, cancel))
        {
            Ok(feeder) => feeder,
            Err(e) => {
                done.store(true, Ordering::Release);
                if let Ok(mut c) = child.0.lock() {
                    let _ = c.kill();
                }
                return Err(e.into());
            }
        };
        let result = raster(&mut BufReader::with_capacity(8192, stdout), &plan, cancel);
        if result.is_err()
            && let Ok(mut c) = child.0.lock()
        {
            let _ = c.kill();
        }
        let status = loop {
            let status = child
                .0
                .lock()
                .map_err(|_| "JPEG child lock poisoned")?
                .try_wait()?;
            if let Some(status) = status {
                break status;
            }
            std::thread::sleep(Duration::from_millis(5));
        };
        done.store(true, Ordering::Release);
        watcher.join().map_err(|_| "JPEG watcher failed")?;
        let fed = feeder.join().map_err(|_| "JPEG feeder failed")?;
        if cancel.is_some_and(|c| c.load(Ordering::Relaxed)) {
            return Err("JPEG derivation cancelled".into());
        }
        let pixels = result?;
        if !status.success() {
            return Err("bounded JPEG helper failed: malformed/truncated scan or 30-second active decode limit".into());
        }
        // A valid JPEG may finish before trailing bytes can be written. All other
        // source errors (read limits, cancellation, storage failures) remain fatal.
        if let Err(e) = fed
            && e.downcast_ref::<std::io::Error>()
                .is_none_or(|e| e.kind() != std::io::ErrorKind::BrokenPipe)
        {
            return Err(e);
        }
        Ok(pixels)
    })
}

#[cfg(test)]
mod tests;
