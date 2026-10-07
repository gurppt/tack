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
        let hint = "Up/Down choose; Enter confirm; Escape close";
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
                palette.accent_attention,
                &mut budget,
            );
        }
    }
}
