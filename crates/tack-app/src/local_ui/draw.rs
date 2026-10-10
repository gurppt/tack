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
        self.modal = None;
        let start_quad = gizmo.quads.len();
        if self.panel == Panel::Toolbar {
            self.draw_toolbar(gizmo, camera, profile);
            return;
        }
        if matches!(
            self.panel,
            Panel::Preferences | Panel::Keymap | Panel::Theme
        ) {
            self.draw_settings(gizmo, camera, keymap, profile);
            return;
        }
        let palette = profile.theme.palette();
        if self.panel == Panel::About {
            self.layout[0] = camera.ui_scale();
            self.modal = Some(crate::modal_shell::ModalShell::short(
                camera,
                [600., 288.],
                true,
            ));
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
        let compact = matches!(
            self.panel,
            Panel::Error
                | Panel::ViewCapture
                | Panel::BookmarkName
                | Panel::Join
                | Panel::Connecting
                | Panel::Close
                | Panel::Recovery
                | Panel::Server
                | Panel::Info
                | Panel::Sharing
        );
        let width =
            (f64::from(screen[0]) / scale - 24.).clamp(1., if compact { 480. } else { 600. });
        let height = (f64::from(screen[1]) / scale - 24.).clamp(
            1.,
            if compact {
                if self.panel == Panel::Info {
                    310.
                } else {
                    240.
                }
            } else {
                405.
            },
        );
        if compact {
            self.modal = Some(crate::modal_shell::ModalShell::short(
                camera,
                [width, height],
                !matches!(
                    self.panel,
                    Panel::Close | Panel::Recovery | Panel::Connecting
                ),
            ));
            self.panel_offset = self.modal.map_or([0.; 2], |m| m.offset());
        }
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
        crate::modal_shell::ModalShell::work(camera, [width, height]).paint(gizmo, camera, palette);
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
            Panel::Theme => "Background - choose directly",
            Panel::Keymap => "Keymap - type to search",
            Panel::Recent => "Recent boards - open in another window",
            Panel::Close => "Unsaved work - save before closing?",
            Panel::Recovery => "Newer recovery available - normal save is unchanged",
            Panel::Error => "Tack - local operation error",
            Panel::About => "About Tack",
            Panel::ViewCapture => "Capture local camera view",
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
            Panel::Keymap | Panel::Preferences | Panel::Theme => Vec::new(),
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
            Panel::ViewCapture => vec!["Press keyboard/mouse shortcut".into()],
        };
        let first = if self.reveal_row {
            crate::ui_scroll::reveal(self.first, self.selected, self.visible, rows.len())
        } else {
            self.first.min(rows.len().saturating_sub(self.visible))
        };
        self.reveal_row = false;
        self.first = first;
        for (index, row) in rows.iter().enumerate().skip(first).take(self.visible) {
            let y = top + (index - first) as f64 * 22.;
            if index == self.selected {
                gizmo.pixel_rect(
                    camera,
                    [20. * scale, y * scale],
                    [(width - 4.) * scale, (y + 20.) * scale],
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
        self.scroll[0].draw(
            gizmo,
            camera,
            palette,
            [20., top, width + 4., top + self.visible as f64 * 22.],
            (first, self.visible, rows.len()),
        );
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
        if self.panel == Panel::ViewCapture {
            for (i, (label, command, enabled)) in [
                ("Confirm", Command::ConfirmCapture, self.staged.is_some()),
                ("Cancel", Command::Close, true),
            ]
            .into_iter()
            .enumerate()
            {
                let x = 24. + i as f64 * 120.;
                let rect = [x, height - 44., x + 108., height - 22.];
                self.hits.push(Hit {
                    rect,
                    command,
                    enabled,
                });
                gizmo.pixel_rect(
                    camera,
                    [rect[0] * scale, rect[1] * scale],
                    [rect[2] * scale, rect[3] * scale],
                    palette.selection,
                    None,
                );
                gizmo.ui_text(
                    camera,
                    [(x + 4.) * scale, (height - 42.) * scale],
                    100.,
                    label,
                    if enabled {
                        palette.text_primary
                    } else {
                        palette.text_disabled
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
        if matches!(self.panel, Panel::ViewCapture | Panel::Error) {
            for (index, line) in
                crate::about::wrap(&self.message, ((width - 32.) / 8.).floor().max(1.) as usize)
                    .into_iter()
                    .take(4)
                    .enumerate()
            {
                let y = 96. + index as f64 * 16.;
                if y + 16. < height - 44. {
                    gizmo.ui_text(
                        camera,
                        [24. * scale, y * scale],
                        width - 32.,
                        &line,
                        palette.accent_attention,
                        &mut budget,
                    );
                }
            }
        }
        if height >= 100. && !matches!(self.panel, Panel::ViewCapture | Panel::Error) {
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
        if let Some(shell) = self.modal {
            shell.translate(gizmo, start_quad, camera);
        }
    }
}
