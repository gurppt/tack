use super::*;
impl LocalUi {
    pub fn draw(
        &mut self,
        gizmo: &mut ImageGizmo,
        camera: &Camera,
        keymap: &Keymap,
        profile: &Preferences,
    ) {
        self.hits.clear();
        self.icon_count = 0;
        self.panel_offset = [0.; 2];
        let start_quad = gizmo.quads.len();
        if self.panel == Panel::Toolbar {
            self.draw_toolbar(gizmo, camera, profile);
            return;
        }
        if matches!(
            self.panel,
            Panel::Preferences | Panel::Keymap | Panel::Scale | Panel::Theme
        ) {
            self.draw_settings(gizmo, camera, keymap, profile);
            return;
        }
        let palette = profile.theme.palette();
        if self.panel == Panel::About {
            self.about_layout = Some(crate::about::draw(
                gizmo,
                camera,
                palette,
                self.about_image,
                !self.message.is_empty(),
            ));
            return;
        }
        let scale = camera.ui_scale();
        let mut budget = 900;
        let screen = camera.screen_size();
        let width = (f64::from(screen[0]) / scale - 24.).clamp(1., 600.);
        let height = (f64::from(screen[1]) / scale - 24.).clamp(1., 405.);
        let top = if height < 100. {
            34.
        } else if height < 240. {
            46.
        } else {
            60.
        };
        let footer = if height >= 180. {
            66.
        } else if height >= 100. {
            24.
        } else {
            0.
        };
        self.layout = [scale, width, height, top];
        self.visible = ((height + 8. - top - footer) / 22.).floor().clamp(1., 12.) as usize;
        gizmo.pixel_rect(
            camera,
            [12. * scale, 12. * scale],
            [(12. + width) * scale, (12. + height) * scale],
            palette.menu_bg,
            None,
        );
        let heading = match self.panel {
            Panel::Sharing => "Share Board",
            Panel::Server => "Advanced: Tack server IP:port",
            Panel::Toolbar => "Edit Toolbar",
            Panel::Bookmarks => "Camera bookmarks - Enter jumps",
            Panel::BookmarkName => "Bookmark name - Ctrl+A replaces",
            Panel::Info => "Image information - existing metadata",
            Panel::Join => "Join Shared Board - paste invite",
            Panel::Connecting => "Join shared board",
            Panel::Menu => "Tack - local files",
            Panel::Preferences => "Preferences",
            Panel::Scale => "UI Scale - choose directly",
            Panel::Theme => "Background - choose directly",
            Panel::Keymap => "Keymap - type to search",
            Panel::Recent => "Recent boards - open in another window",
            Panel::Close => "Unsaved work - save before closing?",
            Panel::Recovery => "Newer recovery available - normal save is unchanged",
            Panel::Error => "Tack - local operation error",
            Panel::About => "About Tack",
        };
        gizmo.ui_text(
            camera,
            [24. * scale, if height < 100. { 14. } else { 22. } * scale],
            width - 24.,
            heading,
            palette.accent_primary,
            &mut budget,
        );
        let rows: Vec<String> = match self.panel {
            Panel::Toolbar => Vec::new(),
            Panel::Sharing | Panel::Server => self.daily_rows(),
            Panel::Bookmarks
            | Panel::BookmarkName
            | Panel::Info
            | Panel::Join
            | Panel::Connecting => self.daily_rows(),
            Panel::Menu => self
                .actions(keymap)
                .iter()
                .map(|a| a.label().into())
                .collect(),
            Panel::Keymap | Panel::Preferences | Panel::Scale | Panel::Theme => Vec::new(),
            Panel::Recent => profile
                .recent
                .iter()
                .map(|p| {
                    p.path()
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|e| e.to_string())
                })
                .collect(),
            Panel::Close => vec![
                "Save and close".into(),
                "Discard edits and recovery".into(),
                "Cancel".into(),
            ],
            Panel::Recovery => vec![
                "Restore recovery as unsaved work".into(),
                "Discard recovery and keep normal save".into(),
            ],
            Panel::Error | Panel::About => vec!["Close this message".into()],
        };
        let first = self
            .selected
            .saturating_sub(self.visible / 2)
            .min(rows.len().saturating_sub(self.visible));
        self.first = first;
        for (index, row) in rows.iter().enumerate().skip(first).take(self.visible) {
            let y = top + (index - first) as f64 * 22.;
            if index == self.selected {
                gizmo.pixel_rect(
                    camera,
                    [20. * scale, y * scale],
                    [(width + 4.) * scale, (y + 20.) * scale],
                    palette.selection,
                    None,
                );
            }
            gizmo.ui_text(
                camera,
                [24. * scale, (y + 2.) * scale],
                width - 24.,
                row,
                palette.text_primary,
                &mut budget,
            );
        }
        if height >= 240. && self.daily.is_some() {
            let commands: &[(&str, Command)] = match self.panel {
                Panel::Bookmarks => &[
                    ("Rename (F2)", Command::RenameBookmark),
                    ("Delete (Del)", Command::DeleteBookmark),
                    ("Close", Command::Close),
                ],
                Panel::BookmarkName | Panel::Join | Panel::Server => &[
                    ("Confirm", Command::ConfirmDaily),
                    ("Cancel", Command::Close),
                ],
                _ => &[("Close / Cancel", Command::Close)],
            };
            for (i, (label, command)) in commands.iter().enumerate() {
                let x = 24. + i as f64 * (width - 32.) / commands.len() as f64;
                let x2 = x + (width - 32.) / commands.len() as f64 - 4.;
                let enabled = !matches!(command, Command::RenameBookmark | Command::DeleteBookmark)
                    || self
                        .daily
                        .as_ref()
                        .is_some_and(|d| !d.shared && !d.bookmarks.is_empty());
                self.hits.push(Hit {
                    rect: [x, height - 44., x2, height - 22.],
                    command: *command,
                    enabled,
                });
                gizmo.pixel_rect(
                    camera,
                    [x * scale, (height - 44.) * scale],
                    [x2 * scale, (height - 22.) * scale],
                    palette.selection,
                    None,
                );
                gizmo.ui_text(
                    camera,
                    [x * scale, (height - 42.) * scale],
                    x2 - x,
                    label,
                    if enabled {
                        palette.text_primary
                    } else {
                        palette.text_secondary
                    },
                    &mut budget,
                );
            }
        }
        let hint = if self.panel == Panel::Bookmarks {
            "Enter jumps; F2 rename; Del delete; Escape close"
        } else {
            "Up/Down scroll; Enter confirm; Escape close"
        };
        if height >= 180. {
            gizmo.ui_text(
                camera,
                [24. * scale, (height - 66.) * scale],
                width - 24.,
                hint,
                palette.text_secondary,
                &mut budget,
            );
        }
        if height >= 100. {
            gizmo.ui_text(
                camera,
                [24. * scale, (height - 8.) * scale],
                width - 24.,
                &self.message,
                if self.message == "COPIED" {
                    palette.accent_secondary
                } else {
                    palette.accent_attention
                },
                &mut budget,
            );
        }
        if self.panel == Panel::Sharing
            && self
                .daily
                .as_ref()
                .is_some_and(|d| d.rows.first().is_some_and(|r| r == "SHARED BOARD OFFLINE"))
        {
            self.panel_offset = [
                ((f64::from(screen[0]) / scale - width) / 2. - 12.)
                    .max(0.)
                    .round(),
                ((f64::from(screen[1]) / scale - height) / 2. - 12.)
                    .max(0.)
                    .round(),
            ];
            let offset = self.panel_offset.map(|v| v * scale / camera.zoom());
            for quad in &mut gizmo.quads[start_quad..] {
                for point in &mut quad.points {
                    for i in 0..2 {
                        point[i] += offset[i];
                    }
                }
            }
        }
    }
}
