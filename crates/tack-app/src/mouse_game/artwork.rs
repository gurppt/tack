//! Tiny PNGs decoded on the existing local worker; cached color masks, no textures.
use crate::{
    image_gizmo::ImageGizmo,
    mouse_game::{Game, State},
};
use std::{
    collections::BTreeMap,
    io::Read,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};
use tack_assets::AssetError;
use tack_core::Camera;
pub const CYCLES: [(&str, usize, [u32; 2]); 7] = [
    ("animated_mulot_run", 3, [16, 16]),
    ("animated_mulot_idle", 3, [16, 16]),
    ("animated_mulot_attack", 15, [16, 16]),
    ("animated_mulot_munch", 2, [16, 16]),
    ("explosion", 12, [16, 32]),
    ("smoky_a", 4, [8, 8]),
    ("smoky_b", 4, [8, 8]),
];
const MAX_MASKS: usize = 64;
#[derive(Clone)]
struct Mask {
    y: usize,
    color: [f32; 4],
    normal: [u32; 8],
    flipped: [u32; 8],
}
pub struct Frame {
    size: [u32; 2],
    masks: Vec<Mask>,
}
pub struct Artwork {
    frames: Vec<Vec<Frame>>,
    pub cheese: Vec<u8>,
}
pub fn root() -> PathBuf {
    let installed = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.join("gfx/mouse-game")));
    installed
        .filter(|p| p.is_dir())
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../gfx/mouse-game"))
}
fn png(path: &Path, size: [u32; 2]) -> Result<Vec<u8>, AssetError> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(65537)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 65536 {
        return Err("Mouse PNG exceeds 64 KiB".into());
    }
    let decoded = tack_assets::decode_ui_png(&bytes)?;
    if [decoded.width, decoded.height] != size {
        return Err("Mouse PNG dimensions differ from authored contract".into());
    }
    Ok(decoded.rgba)
}
fn frame(path: &Path, size: [u32; 2]) -> Result<Frame, AssetError> {
    let rgba = png(path, size)?;
    let mut colors = BTreeMap::<(usize, [u8; 4]), ([u32; 8], [u32; 8])>::new();
    for y in 0..size[1] as usize {
        for x in 0..size[0] as usize {
            let i = (y * size[0] as usize + x) * 4;
            let p = [rgba[i], rgba[i + 1], rgba[i + 2], rgba[i + 3]];
            if p[3] == 0 {
                continue;
            }
            let entry = colors.entry((y / 16, p)).or_default();
            entry.0[(y % 16) / 2] |= 1 << ((y % 2) * 16 + 15 - x);
            entry.1[(y % 16) / 2] |= 1 << ((y % 2) * 16 + 15 - (size[0] as usize - 1 - x));
        }
    }
    if colors.len() > MAX_MASKS {
        return Err("Mouse PNG exceeds 64 color masks".into());
    }
    let masks = colors
        .into_iter()
        .map(|((tile, p), (normal, flipped))| {
            let color = std::array::from_fn(|i| {
                let s = f32::from(p[i]) / 255.;
                if i == 3 || s <= 0.04045 {
                    if i == 3 { s } else { s / 12.92 }
                } else {
                    ((s + 0.055) / 1.055).powf(2.4)
                }
            });
            Mask {
                y: tile * 16,
                color,
                normal,
                flipped,
            }
        })
        .collect();
    Ok(Frame { size, masks })
}
impl Artwork {
    pub fn load(directory: &Path, cancel: &AtomicBool) -> Result<Self, AssetError> {
        let mut frames = Vec::with_capacity(7);
        for (prefix, count, size) in CYCLES {
            let mut cycle = Vec::with_capacity(count);
            for n in 1..=count {
                if cancel.load(Ordering::Relaxed) {
                    return Err("Mouse asset load cancelled".into());
                }
                cycle.push(frame(&directory.join(format!("{prefix}{n}.png")), size)?);
            }
            frames.push(cycle);
        }
        Ok(Self {
            frames,
            cheese: png(&directory.join("cheese_cursor.png"), [16, 16])?,
        })
    }
    pub fn draw(&self, game: &Game, g: &mut ImageGizmo, c: &Camera) {
        for p in &game.puffs {
            self.frames[if p.b { 6 } else { 5 }][p.frame].draw(p.at, p.flip, g, c);
        }
        let cycle = match game.state {
            State::Run => 0,
            State::Idle => 1,
            State::Attack => 2,
            State::Munch => 3,
            State::Explosion => 4,
            _ => return,
        };
        self.frames[cycle][game.frame].draw(game.at, game.flip, g, c);
    }
    pub fn mask_bytes(&self) -> usize {
        self.frames
            .iter()
            .flatten()
            .map(|f| f.masks.len() * std::mem::size_of::<Mask>())
            .sum::<usize>()
            + self.cheese.len()
    }
}
impl Frame {
    fn draw(&self, at: [f64; 2], flip: bool, g: &mut ImageGizmo, c: &Camera) {
        // Full authored canvas, shared bottom-center anchor, explosion extends upward.
        let origin = [
            (at[0] - f64::from(self.size[0])).round(),
            (at[1] - f64::from(self.size[1]) * 2.).round(),
        ];
        for m in &self.masks {
            if g.quads.len() >= tack_render::MAX_OVERLAY_QUADS {
                break;
            }
            let lo = [origin[0], origin[1] + m.y as f64 * 2.];
            let hi = [lo[0] + 32., lo[1] + 32.];
            g.quads.push(tack_render::OverlayQuad {
                dashed: false,
                bitmap: Some(if flip { m.flipped } else { m.normal }),
                color: m.color,
                points: [lo, [lo[0], hi[1]], [hi[0], lo[1]], hi].map(|p| c.screen_to_world(p)),
            });
        }
    }
}
