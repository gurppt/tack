//! Settings-only immediate drawing and hit regions; no objects per action.
use super::*;
use crate::ui_theme::{Color, Palette};
struct Paint<'a> {
    gizmo: &'a mut ImageGizmo,
    camera: &'a Camera,
    palette: Palette,
    budget: usize,
}
impl Paint<'_> {
    fn rect(&mut self, r: [f64; 4], color: Color) {
        let s = self.camera.ui_scale();
        let limit = self
            .camera
            .screen_size()
            .map(|v| (f64::from(v) / s).floor());
        let r = [
            r[0].clamp(0., limit[0]),
            r[1].clamp(0., limit[1]),
            r[2].clamp(0., limit[0]),
            r[3].clamp(0., limit[1]),
        ];
        if r[2] <= r[0] || r[3] <= r[1] {
            return;
        }
        self.gizmo.pixel_rect(
            self.camera,
            [r[0] * s, r[1] * s],
            [r[2] * s, r[3] * s],
            color,
            None,
        );
    }
    fn text(&mut self, x: f64, y: f64, width: f64, text: &str, color: Color) {
        let s = self.camera.ui_scale();
        let limit = self
            .camera
            .screen_size()
            .map(|v| (f64::from(v) / s).floor());
        if x < 0. || y < 0. || y + 16. > limit[1] {
            return;
        }
        let width = width.min(limit[0] - x - 16.);
        if width <= 0. {
            return;
        }
        self.gizmo.ui_text(
            self.camera,
            [x * s, y * s],
            width,
            text,
            color,
            &mut self.budget,
        );
    }
}
impl LocalUi {
    fn button(
        &mut self,
        paint: &mut Paint<'_>,
        rect: [f64; 4],
        label: &str,
        command: Command,
        enabled: bool,
    ) {
        let p = paint.palette;
        let cursor = self.cursor.map(|v| v / self.layout[0]);
        let hit = Hit {
            rect,
            command,
            enabled,
        };
        let focused = self.focus == Some(command);
        let hover = hit.contains(cursor) && enabled;
        paint.rect(
            rect,
            if focused || hover {
                p.accent_primary
            } else {
                p.menu_border
            },
        );
        paint.rect(
            [rect[0] + 1., rect[1] + 1., rect[2] - 1., rect[3] - 1.],
            if focused || hover {
                p.selection
            } else {
                p.menu_bg
            },
        );
        paint.text(
            rect[0] + 5.,
            rect[1] + 3.,
            rect[2] - rect[0] - 10.,
            label,
            if !enabled {
                p.text_disabled
            } else if focused || hover {
                p.accent_primary
            } else {
                p.accent_secondary
            },
        );
        self.hits.push(hit);
    }
    pub(super) fn draw_settings(
        &mut self,
        gizmo: &mut ImageGizmo,
        camera: &Camera,
        keymap: &Keymap,
        profile: &Preferences,
    ) {
        let scale = camera.ui_scale();
        let screen = camera.screen_size();
        let width = (f64::from(screen[0]) / scale - 24.).clamp(1., 600.);
        let maximum = match self.panel {
            Panel::Preferences => 310.,
            Panel::Scale => 178.,
            Panel::Theme => 134.,
            _ => 440.,
        };
        let height = (f64::from(screen[1]) / scale - 24.).clamp(1., maximum);
        let key_panel = self.panel == Panel::Keymap;
        let top = if key_panel { 84. } else { 48. };
        let footer = if key_panel { 114. } else { 32. };
        let end = 12. + height;
        let footer_y = end - footer;
        self.layout = [scale, width, height, top];
        self.visible = ((footer_y - top) / 22.).floor().clamp(1., 12.) as usize;
        let mut paint = Paint {
            gizmo,
            camera,
            palette: profile.theme.palette(),
            budget: 900,
        };
        let p = paint.palette;
        paint.rect([12., 12., 12. + width, end], p.menu_border);
        paint.rect([13., 13., 11. + width, end - 1.], p.menu_bg);
        let heading = match self.panel {
            Panel::Preferences => "Preferences",
            Panel::Scale => "UI Scale - choose",
            Panel::Theme => "Background - choose",
            _ => "Keymap",
        };
        paint.text(24., 22., width - 24., heading, p.accent_primary);
        if key_panel {
            self.draw_keymap(&mut paint, width, footer_y, keymap);
        } else {
            self.draw_preferences(&mut paint, width, footer_y, profile);
        }
    }
    fn draw_keymap(&mut self, paint: &mut Paint<'_>, width: f64, footer: f64, keymap: &Keymap) {
        let p = paint.palette;
        self.button(
            paint,
            [24., 42., width, 64.],
            &format!("Search: {}_", self.search),
            Command::Search,
            !self.capture && self.confirm_reset.is_none(),
        );
        let column = 24. + (width - 24.) * 0.55;
        paint.text(24., 66., column - 28., "Action", p.text_secondary);
        paint.text(
            column,
            66.,
            width - column,
            "Current shortcut",
            p.accent_attention,
        );
        let actions = self.actions(keymap);
        self.selected = self.selected.min(actions.len().saturating_sub(1));
        self.first = self
            .selected
            .saturating_sub(self.visible / 2)
            .min(actions.len().saturating_sub(self.visible));
        for (index, action) in actions
            .iter()
            .enumerate()
            .skip(self.first)
            .take(self.visible)
        {
            let y = 84. + (index - self.first) as f64 * 22.;
            self.row(paint, index, y, width);
            paint.text(
                24.,
                y + 2.,
                column - 28.,
                action.label(),
                if index == self.selected {
                    p.accent_primary
                } else {
                    p.text_primary
                },
            );
            let bindings: Vec<_> = keymap
                .for_action(*action)
                .map(crate::context_menu::binding_label)
                .collect();
            let label = if bindings.is_empty() {
                "Unassigned".into()
            } else {
                bindings.join("; ")
            };
            paint.text(
                column,
                y + 2.,
                width - column,
                &label,
                if bindings.is_empty() {
                    p.text_disabled
                } else {
                    p.accent_attention
                },
            );
        }
        if actions.is_empty() {
            paint.text(
                24.,
                84.,
                width - 24.,
                "No matching actions",
                p.text_secondary,
            );
        }
        let available = !actions.is_empty();
        let gap = 4.;
        let cell = (width - 24. - gap * 2.) / 3.;
        let mut buttons = if self.confirm_reset.is_some() {
            vec![
                ("Confirm reset", Command::ConfirmReset, true),
                ("Cancel", Command::Cancel, true),
            ]
        } else if self.capture {
            vec![("Cancel capture", Command::Cancel, true)]
        } else {
            vec![
                ("Change", Command::Change, available),
                ("Unassign", Command::Unassign, available),
                ("Reset action", Command::Reset, available),
                ("Import...", Command::Import, true),
                ("Export...", Command::Export, true),
                ("Reset all...", Command::ResetAll, true),
                (
                    if self.release { "Release" } else { "Press" },
                    Command::Trigger,
                    true,
                ),
                (
                    if self.parents.is_empty() {
                        "Close"
                    } else {
                        "Back"
                    },
                    Command::Close,
                    true,
                ),
            ]
        };
        for (i, (label, command, enabled)) in buttons.drain(..).enumerate() {
            let x = 24. + (i % 3) as f64 * (cell + gap);
            let y = footer + (i / 3) as f64 * 26.;
            self.button(paint, [x, y, x + cell, y + 22.], label, command, enabled);
        }
        let status = if !self.message.is_empty() {
            self.message.clone()
        } else if self.capture {
            "Press new shortcut... Escape or Cancel to stop.".into()
        } else {
            actions.get(self.selected).map_or_else(
                || "Type to search; Escape clears search".into(),
                |a| {
                    format!(
                        "{}: {}",
                        a.label(),
                        keymap
                            .for_action(*a)
                            .map(|b| format!(
                                "{} [{:?}]",
                                crate::context_menu::binding_label(b),
                                b.trigger
                            ))
                            .collect::<Vec<_>>()
                            .join("; ")
                    )
                },
            )
        };
        // Two bounded lines make conflict/action text useful at 800x600, 2x.
        let chars = ((width - 24.) / 8.).floor().max(1.) as usize;
        let mut words = status.split_whitespace();
        let mut lines = [String::new(), String::new()];
        for line in &mut lines {
            while let Some(word) = words.clone().next() {
                if !line.is_empty() && line.chars().count() + word.chars().count() + 1 > chars {
                    break;
                }
                words.next();
                if !line.is_empty() {
                    line.push(' ');
                }
                line.push_str(word);
            }
        }
        for (i, line) in lines.iter().enumerate() {
            paint.text(
                24.,
                footer + 78. + i as f64 * 16.,
                width - 24.,
                line,
                if self.message.is_empty() && !self.capture {
                    p.text_secondary
                } else {
                    p.accent_attention
                },
            );
        }
    }
    fn row(&self, paint: &mut Paint<'_>, index: usize, y: f64, width: f64) {
        let cursor = self.cursor.map(|v| v / self.layout[0]);
        let p = paint.palette;
        if index == self.selected {
            paint.rect([20., y, width + 4., y + 20.], p.selection);
            paint.rect([20., y, 22., y + 20.], p.accent_primary);
        } else if cursor[0] >= 20.
            && cursor[0] < width + 4.
            && cursor[1] >= y
            && cursor[1] < y + 20.
        {
            paint.rect([20., y, width + 4., y + 20.], p.selection);
            paint.rect([20., y, 21., y + 20.], p.menu_border);
        }
    }
    fn draw_preferences(
        &mut self,
        paint: &mut Paint<'_>,
        width: f64,
        footer: f64,
        profile: &Preferences,
    ) {
        let p = paint.palette;
        let rows: Vec<(String, String)> = match self.panel {
            Panel::Preferences => vec![
                (
                    "Grid default".into(),
                    if profile.grid { "[x] On" } else { "[ ] Off" }.into(),
                ),
                ("Image sampling".into(), format!("{} >", profile.sampling)),
                (
                    "Import".into(),
                    if profile.embedded_import {
                        "Embedded >"
                    } else {
                        "Linked >"
                    }
                    .into(),
                ),
                (
                    "UI scale".into(),
                    if profile.ui_scale == 0 {
                        "Auto/System >".into()
                    } else {
                        format!("{}x >", profile.ui_scale)
                    },
                ),
                ("Background".into(), format!("{} >", profile.theme.label())),
                ("Handle Size".into(), format!("{} px", profile.handle_size)),
                ("Hit Radius".into(), format!("{} px", profile.hit_radius)),
                ("Keymap...".into(), String::new()),
                ("Import keymap...".into(), String::new()),
                ("Export preferences/keymap...".into(), String::new()),
                ("Close".into(), String::new()),
            ],
            Panel::Scale => ["Auto/System", "1x", "2x", "3x", "4x"]
                .into_iter()
                .enumerate()
                .map(|(i, label)| {
                    (
                        label.into(),
                        if i == usize::from(profile.ui_scale) {
                            "[x] Current"
                        } else {
                            "[ ]"
                        }
                        .into(),
                    )
                })
                .collect(),
            Panel::Theme => crate::ui_theme::Theme::ALL
                .into_iter()
                .map(|t| {
                    (
                        t.label().into(),
                        if t == profile.theme {
                            "[x] Current"
                        } else {
                            "[ ]"
                        }
                        .into(),
                    )
                })
                .collect(),
            _ => Vec::new(),
        };
        self.first = self
            .selected
            .saturating_sub(self.visible / 2)
            .min(rows.len().saturating_sub(self.visible));
        for (index, (label, value)) in rows.iter().enumerate().skip(self.first).take(self.visible) {
            let y = 48. + (index - self.first) as f64 * 22.;
            self.row(paint, index, y, width);
            let numeric = self.panel == Panel::Preferences && matches!(index, 5 | 6);
            paint.text(
                24.,
                y + 2.,
                if numeric {
                    width - 164.
                } else {
                    (width - 24.) * 0.52
                },
                label,
                if index == self.selected {
                    p.accent_primary
                } else {
                    p.text_primary
                },
            );
            if numeric {
                let v = if index == 5 {
                    profile.handle_size
                } else {
                    profile.hit_radius
                };
                let (min, max) = if index == 5 { (3, 21) } else { (5, 32) };
                self.button(
                    paint,
                    [width - 136., y, width - 104., y + 22.],
                    "-",
                    Command::Decrement(index),
                    v > min,
                );
                paint.text(width - 98., y + 3., 60., value, p.accent_primary);
                self.button(
                    paint,
                    [width - 32., y, width, y + 22.],
                    "+",
                    Command::Increment(index),
                    v < max,
                );
            } else {
                paint.text(
                    24. + (width - 24.) * 0.54,
                    y + 2.,
                    (width - 24.) * 0.46,
                    value,
                    p.accent_secondary,
                );
            }
        }
        let nested = matches!(self.panel, Panel::Scale | Panel::Theme);
        if nested {
            self.button(
                paint,
                [width - 72., footer + 4., width, footer + 26.],
                if self.parents.is_empty() {
                    "Close"
                } else {
                    "Back"
                },
                Command::Close,
                true,
            );
        }
        paint.text(
            24.,
            footer + 2.,
            width - if nested { 108. } else { 24. },
            if nested {
                "Click/Enter: preview"
            } else {
                "Click or Enter; Left/Right: -/+"
            },
            p.text_secondary,
        );
        paint.text(
            24.,
            footer + 18.,
            width - if nested { 108. } else { 24. },
            if self.parents.is_empty() {
                "Up/Down choose; Escape closes"
            } else {
                "Up/Down choose; Escape returns"
            },
            p.text_secondary,
        );
    }
}
