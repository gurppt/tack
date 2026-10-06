//! Three immutable local UI palettes. Colors are linear RGB for the canvas target.
use serde::{Deserialize, Serialize};
pub type Color = [f32; 4];
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Theme {
    VeryDark,
    #[default]
    NeutralGray,
    Light,
}
#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub background_a: Color,
    pub background_b: Color,
    pub menu_bg: Color,
    pub menu_border: Color,
    pub text_primary: Color,
    pub text_secondary: Color,
    pub text_disabled: Color,
    pub accent_primary: Color,
    pub accent_secondary: Color,
    pub accent_attention: Color,
    pub selection: Color,
    pub grid: Color,
    pub guide: Color,
}
impl Default for Palette {
    fn default() -> Self {
        Theme::default().palette()
    }
}
impl Theme {
    pub const ALL: [Self; 3] = [Self::VeryDark, Self::NeutralGray, Self::Light];
    pub fn label(self) -> &'static str {
        match self {
            Self::VeryDark => "Very Dark",
            Self::NeutralGray => "Neutral Gray",
            Self::Light => "Light",
        }
    }
    pub fn palette(self) -> Palette {
        let c = |r, g, b| [r, g, b, 1.];
        if self == Self::Light {
            return Palette {
                background_a: c(0.69, 0.70, 0.71),
                background_b: c(0.72, 0.73, 0.74),
                menu_bg: c(0.85, 0.86, 0.87),
                menu_border: c(0.16, 0.19, 0.21),
                text_primary: c(0.012, 0.016, 0.021),
                text_secondary: c(0.06, 0.075, 0.085),
                text_disabled: c(0.14, 0.155, 0.17),
                accent_primary: c(0.0, 0.115, 0.16),
                accent_secondary: c(0.28, 0.015, 0.15),
                accent_attention: c(0.095, 0.10, 0.0),
                selection: c(0.59, 0.75, 0.78),
                grid: c(0.36, 0.38, 0.40),
                guide: c(0.015, 0.18, 0.22),
            };
        }
        let (a, b, menu) = if self == Self::VeryDark {
            (
                c(0.008, 0.010, 0.014),
                c(0.011, 0.013, 0.018),
                c(0.019, 0.024, 0.032),
            )
        } else {
            (
                c(0.035, 0.040, 0.050),
                c(0.041, 0.046, 0.056),
                c(0.030, 0.038, 0.046),
            )
        };
        Palette {
            background_a: a,
            background_b: b,
            menu_bg: menu,
            menu_border: c(0.27, 0.34, 0.38),
            text_primary: c(0.87, 0.89, 0.91),
            text_secondary: c(0.51, 0.57, 0.61),
            text_disabled: c(0.27, 0.32, 0.35),
            accent_primary: c(0.15, 0.88, 0.95),
            accent_secondary: c(0.95, 0.25, 0.64),
            accent_attention: c(0.80, 0.94, 0.12),
            selection: c(0.025, 0.050, 0.065),
            grid: c(0.095, 0.12, 0.15),
            guide: c(0.16, 0.73, 0.82),
        }
    }
}
