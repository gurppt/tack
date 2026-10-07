//! Build-validated immutable metadata and one explicitly requested compact PNG.
use crate::{image_gizmo::ImageGizmo, ui_theme::Palette};
use tack_core::Camera;

pub struct Metadata {
    pub name: &'static str,
    pub version: &'static str,
    pub author: &'static str,
    pub website: &'static str,
    pub contact: &'static str,
    pub license: &'static str,
    pub copyright: &'static str,
    pub tagline: &'static str,
    pub source: &'static str,
}
include!(concat!(env!("OUT_DIR"), "/about_metadata.rs"));
pub const PACKAGE_NAME: &str = "tack-about.png";

/// Invoked only by the explicit local About worker, never during startup.
pub fn load_image() -> Result<tack_assets::Decoded, tack_assets::AssetError> {
    let executable = std::env::current_exe()?;
    let path = executable
        .parent()
        .ok_or("About package directory unavailable")?
        .join(PACKAGE_NAME);
    read_image(&path)
}

pub fn read_image(path: &std::path::Path) -> Result<tack_assets::Decoded, tack_assets::AssetError> {
    use std::io::Read;
    let file = std::fs::File::open(path)?;
    let mut bytes = Vec::new();
    file.take(128 * 1024 + 1).read_to_end(&mut bytes)?;
    decode_image(&bytes)
}

pub fn decode_image(bytes: &[u8]) -> Result<tack_assets::Decoded, tack_assets::AssetError> {
    let image = tack_assets::decode_ui_png(bytes)?;
    if [image.width, image.height] != IMAGE_SIZE {
        return Err("About artwork dimensions do not match its compact package".into());
    }
    Ok(image)
}

#[derive(Clone, Copy, Debug)]
pub struct Layout {
    pub panel: [f64; 4],
    pub image: [f64; 4],
    pub text: [f64; 4],
    pub close: [f64; 4],
    pub scale: f64,
}
impl Layout {
    pub fn new(camera: &Camera) -> Self {
        let scale = camera.ui_scale();
        let screen = camera.screen_size().map(|v| f64::from(v) / scale);
        let width = (screen[0] - 24.).clamp(32., 600.);
        let height = (screen[1] - 24.).clamp(32., 288.);
        let x = ((screen[0] - width) / 2.).floor().max(0.);
        let y = ((screen[1] - height) / 2.).floor().max(0.);
        let image_height = (height - 64.).clamp(1., 224.);
        let image_width =
            (image_height * f64::from(IMAGE_SIZE[0]) / f64::from(IMAGE_SIZE[1])).round();
        let image_x = x + width - 12. - image_width;
        Self {
            panel: [x, y, width, height],
            image: [image_x, y + 32., image_width, image_height],
            text: [x + 12., y + 32., (image_x - x - 24.).max(1.), image_height],
            close: [x + width - 76., y + height - 28., 64., 20.],
            scale,
        }
    }
    pub fn close_hit(self, pointer: [f64; 2]) -> bool {
        let [x, y, w, h] = self.close;
        let p = pointer.map(|v| v / self.scale);
        p[0] >= x && p[0] < x + w && p[1] >= y && p[1] < y + h
    }
    pub fn physical_image(self) -> [f64; 4] {
        self.image.map(|v| v * self.scale)
    }
}

pub fn wrap(text: &str, cells: usize) -> Vec<String> {
    let cells = cells.max(2);
    let advance = |c: char| if c.is_ascii() { 1 } else { 2 };
    let mut lines = Vec::new();
    let mut line = String::new();
    let mut width = 0;
    for word in text.split_whitespace() {
        let word_width: usize = word.chars().map(advance).sum();
        if !line.is_empty() && width + 1 + word_width <= cells {
            line.push(' ');
            line.push_str(word);
            width += 1 + word_width;
            continue;
        }
        if !line.is_empty() {
            lines.push(std::mem::take(&mut line));
            width = 0;
        }
        for c in word.chars() {
            if width + advance(c) > cells {
                lines.push(std::mem::take(&mut line));
                width = 0;
            }
            line.push(c);
            width += advance(c);
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

pub fn text_rows(cells: usize, palette: Palette) -> Vec<(String, [f32; 4])> {
    let mut rows = Vec::new();
    let mut add = |text: String, color| {
        rows.extend(wrap(&text, cells).into_iter().map(|s| (s, color)));
    };
    add(METADATA.name.into(), palette.accent_primary);
    add(
        format!("Version {}", METADATA.version),
        palette.text_primary,
    );
    if !METADATA.tagline.is_empty() {
        add(METADATA.tagline.into(), palette.text_secondary);
    }
    for (label, value, link) in [
        ("Author", METADATA.author, false),
        ("Website", METADATA.website, true),
        ("Contact", METADATA.contact, true),
        ("License", METADATA.license, false),
        ("Copyright", METADATA.copyright, false),
        ("Source", METADATA.source, true),
    ] {
        if value.is_empty() && matches!(label, "Copyright" | "Source") {
            continue;
        }
        add(
            format!(
                "{label}: {}",
                if value.is_empty() { "Not set" } else { value }
            ),
            if link {
                palette.accent_primary
            } else {
                palette.text_primary
            },
        );
    }
    rows
}

pub fn draw(
    gizmo: &mut ImageGizmo,
    camera: &Camera,
    palette: Palette,
    image: bool,
    failed: bool,
) -> Layout {
    let layout = Layout::new(camera);
    let scale = layout.scale;
    let [x, y, w, h] = layout.panel;
    let mut budget = 900;
    gizmo.pixel_rect(
        camera,
        [x * scale, y * scale],
        [(x + w) * scale, (y + h) * scale],
        palette.menu_border,
        None,
    );
    gizmo.pixel_rect(
        camera,
        [(x + 1.) * scale, (y + 1.) * scale],
        [(x + w - 1.) * scale, (y + h - 1.) * scale],
        palette.menu_bg,
        None,
    );
    gizmo.ui_text(
        camera,
        [(x + 12.) * scale, (y + 8.) * scale],
        w - 24.,
        "About Tack",
        palette.accent_primary,
        &mut budget,
    );
    let [tx, ty, tw, th] = layout.text;
    for (i, (line, color)) in text_rows((tw / 8.).floor() as usize, palette)
        .iter()
        .enumerate()
    {
        if (i + 1) as f64 * 16. > th {
            break;
        }
        gizmo.ui_text(
            camera,
            [tx * scale, (ty + i as f64 * 16.) * scale],
            tw,
            line,
            *color,
            &mut budget,
        );
    }
    if !image {
        let [ix, iy, iw, _] = layout.image;
        gizmo.ui_text(
            camera,
            [ix * scale, iy * scale],
            iw,
            if failed {
                "Artwork unavailable"
            } else {
                "Preparing artwork"
            },
            palette.accent_attention,
            &mut budget,
        );
    }
    let [cx, cy, cw, ch] = layout.close;
    gizmo.pixel_rect(
        camera,
        [cx * scale, cy * scale],
        [(cx + cw) * scale, (cy + ch) * scale],
        palette.selection,
        None,
    );
    gizmo.ui_text(
        camera,
        [(cx + 12.) * scale, (cy + 2.) * scale],
        cw - 12.,
        "Close",
        palette.text_primary,
        &mut budget,
    );
    layout
}
