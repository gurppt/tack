//! Mulot only: one bounded local queue and one one-shot deadline, no polling.
use crate::{image_gizmo::ImageGizmo, local_import::AdmittedImage};
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};
use tack_assets::AssetError;
use tack_core::{AssetId, Camera, Document, DocumentObject, ImageAsset, ImageFiltering, Transform};

pub const LIMIT: usize = 64;
pub const SPACING: f64 = 18.;
pub const PAW_LIFETIME: Duration = Duration::from_millis(800);
const PING_STEP: Duration = Duration::from_millis(120);
pub const EASTER_WAIT: Duration = Duration::from_secs(600);
// Only an asset identity convention, not a persistent object variant or metadata field.
// Keep 96 random bits from the normal import: concurrent first inserts never share IDs.
const ARTWORK_PREFIX: u128 = 0x69b4_ce61_u128 << 96;
const RANDOM_MASK: u128 = (1_u128 << 96) - 1;
fn is_artwork(id: AssetId) -> bool {
    id.value() & !RANDOM_MASK == ARTWORK_PREFIX
}
pub fn existing_artwork(doc: &Document) -> Option<AssetId> {
    doc.assets()
        .find(|asset| is_artwork(asset.id()))
        .map(|asset| asset.id())
}
pub fn hover_status(doc: &Document, object: tack_core::ObjectId) -> Option<&'static str> {
    match doc.object(object)?.kind() {
        tack_core::ObjectKind::Image(image) if is_artwork(image.asset_id()) => {
            Some("Puzzo puzzo !")
        }
        _ => None,
    }
}
pub fn easter_object(
    id: tack_core::ObjectId,
    asset: AssetId,
    center: [f64; 2],
    zoom: f64,
) -> Result<DocumentObject, AssetError> {
    Ok(DocumentObject::image_with_properties(
        id,
        asset,
        Transform::new(center, [36. / zoom; 2], 0., [false; 2])?,
        tack_core::Crop::FULL,
        tack_core::Opacity::OPAQUE,
        ImageFiltering::Nearest,
    ))
}
pub fn prepare_easter(image: &mut AdmittedImage, zoom: f64) -> Result<(), AssetError> {
    image.asset = ImageAsset::new(
        AssetId::new(ARTWORK_PREFIX | (image.asset.id().value() & RANDOM_MASK))?,
        image.source.id(),
        image.asset.pixel_size(),
    )?;
    image.object = easter_object(
        image.object.id(),
        image.asset.id(),
        image.object.transform().center(),
        zoom,
    )?;
    image.sampling = ImageFiltering::Nearest;
    Ok(())
}

/// N, NE, E, SE, S, SW, W, NW: source orientation, X/Y mirrors.
pub const DIRECTIONS: [(usize, bool, bool); 8] = [
    (0, false, false),
    (1, true, false),
    (2, true, false),
    (1, true, true),
    (0, false, true),
    (1, false, true),
    (2, false, false),
    (1, false, false),
];
fn direction(d: [f64; 2]) -> usize {
    ((d[0].atan2(-d[1]) / std::f64::consts::FRAC_PI_4).round() as i32).rem_euclid(8) as usize
}
#[derive(Clone, Copy, Default)]
struct Sprite {
    bits: [u32; 8],
    color: [f32; 4],
}
#[derive(Clone, Copy, Debug)]
enum Effect {
    Paw {
        at: [f64; 2],
        sprite: usize,
        expires: Instant,
    },
    Ping {
        at: [f64; 2],
        born: Instant,
        stage: u32,
    },
}
impl Effect {
    fn deadline(self) -> Instant {
        match self {
            Self::Paw { expires, .. } => expires,
            Self::Ping { born, stage, .. } => born + PING_STEP * (stage + 1),
        }
    }
}
pub struct MouseTool {
    active: bool,
    easter: Option<Instant>,
    wait: Duration,
    anchor: Option<[f64; 2]>,
    remaining: f64,
    right: bool,
    effects: VecDeque<Effect>,
    sprites: [Sprite; 16],
    pub spawns: u64,
}
impl Default for MouseTool {
    fn default() -> Self {
        Self::new(EASTER_WAIT)
    }
}
impl MouseTool {
    pub fn new(wait: Duration) -> Self {
        Self {
            active: false,
            easter: None,
            wait,
            anchor: None,
            remaining: SPACING,
            right: false,
            effects: VecDeque::with_capacity(LIMIT),
            sprites: [Sprite::default(); 16],
            spawns: 0,
        }
    }
    /// Startup boundary only; preserves the authored PNGs and never writes them.
    pub fn load(&mut self) {
        let root = crate::toolbar_icons::root();
        let names = [
            ["pawL_n", "pawR_n"],
            ["pawL_45", "pawR_45"],
            ["pawL_w", "pawR_w"],
        ];
        for (d, &(source, x, y)) in DIRECTIONS.iter().enumerate() {
            for foot in 0..2 {
                // A single mirror reverses handedness: swap the authored foot.
                let name = names[source][foot ^ usize::from(x ^ y)];
                match load_sprite(&root.join(format!("{name}.png")), x, y) {
                    Ok(sprite) => self.sprites[d * 2 + foot] = sprite,
                    Err(e) => eprintln!("[tack/mulot] {name}: {e}"),
                }
            }
        }
    }
    pub fn activate(&mut self, active: bool, now: Instant) -> bool {
        if active == self.active {
            return false;
        }
        self.active = active;
        self.easter = active.then_some(now + self.wait);
        self.anchor = None;
        self.remaining = SPACING;
        self.right = false;
        let changed = !self.effects.is_empty();
        self.effects.clear();
        changed
    }
    pub fn leave_canvas(&mut self) {
        self.anchor = None;
        self.remaining = SPACING;
    }
    fn push(&mut self, effect: Effect) {
        if self.effects.len() == LIMIT {
            self.effects.pop_front();
        }
        self.effects.push_back(effect);
    }
    pub fn motion(&mut self, next: [f64; 2], now: Instant) -> bool {
        if !self.active || !next.into_iter().all(f64::is_finite) {
            return false;
        }
        let Some(previous) = self.anchor.replace(next) else {
            return false;
        };
        let delta = [next[0] - previous[0], next[1] - previous[1]];
        let distance = delta[0].hypot(delta[1]);
        if distance == 0. || !distance.is_finite() {
            return false;
        }
        let unit = delta.map(|v| v / distance);
        let dir = direction(delta);
        let mut travel = self.remaining;
        let mut emitted = 0;
        // Bound work even for a synthetic pointer jump of millions of pixels.
        while travel <= distance && emitted < LIMIT {
            let side = if self.right { -3. } else { 3. };
            let at = [
                (previous[0] + unit[0] * travel + unit[1] * side - 8.).round(),
                (previous[1] + unit[1] * travel - unit[0] * side - 8.).round(),
            ];
            self.push(Effect::Paw {
                at,
                sprite: dir * 2 + usize::from(self.right),
                expires: now + PAW_LIFETIME,
            });
            self.right = !self.right;
            travel += SPACING;
            emitted += 1;
        }
        self.remaining = if travel > distance {
            travel - distance
        } else {
            SPACING
        };
        emitted > 0
    }
    pub fn click(&mut self, at: [f64; 2], now: Instant) -> bool {
        if !self.active || !at.into_iter().all(f64::is_finite) {
            return false;
        }
        self.push(Effect::Ping {
            at: at.map(f64::round),
            born: now,
            stage: 0,
        });
        true
    }
    /// Returns (visible effects changed, one-shot insertion requested).
    pub fn settle(&mut self, now: Instant) -> (bool, bool) {
        let mut changed = false;
        self.effects.retain_mut(|effect| match effect {
            Effect::Paw { expires, .. } => {
                let keep = now < *expires;
                changed |= !keep;
                keep
            }
            Effect::Ping { born, stage, .. } => {
                let next = (now.saturating_duration_since(*born).as_millis()
                    / PING_STEP.as_millis()) as u32;
                changed |= next != *stage;
                *stage = next.min(3);
                next < 3
            }
        });
        let fire = self.easter.is_some_and(|at| now >= at);
        if fire {
            self.easter = None;
            self.spawns += 1;
        }
        (changed, fire)
    }
    pub fn deadline(&self) -> Option<Instant> {
        self.effects
            .iter()
            .map(|e| e.deadline())
            .chain(self.easter)
            .min()
    }
    pub fn effect_count(&self) -> usize {
        self.effects.len()
    }
    pub fn draw(&self, g: &mut ImageGizmo, c: &Camera) {
        let scale = c.ui_scale();
        for effect in &self.effects {
            match *effect {
                Effect::Paw { at, sprite, .. } => {
                    let sprite = self.sprites[sprite];
                    g.pixel_rect(
                        c,
                        at.map(|v| v * scale),
                        at.map(|v| (v + 16.) * scale),
                        sprite.color,
                        Some(sprite.bits),
                    );
                }
                Effect::Ping { at, stage, .. } => {
                    let radius = 3. + f64::from(stage) * 3.;
                    // Four crisp one-pixel arms, three discrete expansion states.
                    for (lo, hi) in [
                        ([-radius, -1.], [-radius + 1., 2.]),
                        ([radius, -1.], [radius + 1., 2.]),
                        ([-1., -radius], [2., -radius + 1.]),
                        ([-1., radius], [2., radius + 1.]),
                    ] {
                        g.pixel_rect(
                            c,
                            std::array::from_fn(|i| (at[i] + lo[i]) * scale),
                            std::array::from_fn(|i| (at[i] + hi[i]) * scale),
                            g.palette.accent_primary,
                            None,
                        );
                    }
                }
            }
        }
    }
}
fn load_sprite(path: &std::path::Path, flip_x: bool, flip_y: bool) -> Result<Sprite, AssetError> {
    use std::io::Read;
    let file = std::fs::File::open(path)?;
    let mut bytes = Vec::new();
    file.take(16 * 1024 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > 16 * 1024 {
        return Err("paw PNG exceeds 16 KiB".into());
    }
    let image = tack_assets::decode_ui_png(&bytes)?;
    if image.width > 16 || image.height > 16 {
        return Err("paw PNG exceeds 16x16".into());
    }
    let mut sprite = Sprite::default();
    let mut authored_color = None;
    for y in 0..image.height as usize {
        for x in 0..image.width as usize {
            let p = &image.rgba[(y * image.width as usize + x) * 4..][..4];
            if p[3] == 0 {
                continue;
            }
            if p[3] != 255 || authored_color.is_some_and(|color: [u8; 3]| color != p[..3]) {
                return Err("paw needs hard alpha and one authored color".into());
            }
            authored_color = Some([p[0], p[1], p[2]]);
            let x = if flip_x {
                image.width as usize - 1 - x
            } else {
                x
            } + (16 - image.width as usize) / 2;
            let y = if flip_y {
                image.height as usize - 1 - y
            } else {
                y
            } + (16 - image.height as usize) / 2;
            sprite.bits[y / 2] |= 1 << ((y % 2) * 16 + 15 - x);
        }
    }
    if let Some(color) = authored_color {
        sprite.color = [0., 0., 0., 1.];
        for (i, channel) in color.into_iter().enumerate() {
            let s = f32::from(channel) / 255.;
            sprite.color[i] = if s <= 0.04045 {
                s / 12.92
            } else {
                ((s + 0.055) / 1.055).powf(2.4)
            };
        }
    }
    Ok(sprite)
}
#[cfg(test)]
mod tests;
