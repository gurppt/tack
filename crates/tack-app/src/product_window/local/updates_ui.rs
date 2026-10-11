//! Update UI has no authority over the board. Replacement waits for a clean app exit.
use super::*;
impl App {
    fn update_panel(&mut self, panel: Panel, rows: Vec<String>) {
        let current = rows.iter().any(|row| row.starts_with("Tack is up to date"));
        let mut rows = rows;
        rows.retain(|s| {
            !s.starts_with("Current:") && !s.starts_with("Channel:") && !s.starts_with("Available:")
        });
        rows.insert(0, "Source: gurppt/tack".into());
        rows.insert(
            0,
            format!("Channel: {}", self.local.profile.update_channel.label()),
        );
        rows.insert(
            0,
            format!(
                "Available: {}",
                self.local.update_offer.as_ref().map_or_else(
                    || if current {
                        env!("CARGO_PKG_VERSION").into()
                    } else {
                        "Not checked".into()
                    },
                    |o| o.manifest.version.clone()
                )
            ),
        );
        rows.insert(0, format!("Installed: {}", env!("CARGO_PKG_VERSION")));
        self.panel(panel);
        self.local.ui = Some(Box::new(LocalUi::daily(
            panel,
            tack_app::local_ui::DailyPanel {
                rows,
                ..Default::default()
            },
        )));
        self.dirty = true;
    }
    pub(in super::super) fn check_for_updates(&mut self) -> Result<(), AssetError> {
        self.local.update_offer = None;
        self.local.update_stage = None;
        self.update_panel(Panel::UpdateChecking, vec!["Not checked; use Check".into()]);
        Ok(())
    }
    pub(in super::super) fn update_check(&mut self) -> Result<(), AssetError> {
        if !cfg!(feature = "updater") {
            return Err("Updates omitted from this build".into());
        }
        if self.local.worker.active() {
            return Ok(());
        }
        self.local.update_offer = None;
        self.local.update_stage = None;
        self.update_panel(
            Panel::UpdateChecking,
            vec![
                format!("Current: {}", env!("CARGO_PKG_VERSION")),
                format!("Channel: {}", self.local.profile.update_channel.label()),
                "Checking published releases...".into(),
            ],
        );
        self.operation(Operation::Update(tack_app::updates::Request::Check {
            channel: self.local.profile.update_channel,
            work: self.work.join("update"),
        }))
    }
    pub(in super::super) fn update_response(
        &mut self,
        result: Result<tack_update::Response, String>,
    ) -> Result<(), AssetError> {
        if !self
            .local
            .ui
            .as_ref()
            .is_some_and(|u| matches!(u.panel, Panel::UpdateChecking | Panel::UpdateApplying))
        {
            return Ok(());
        }
        match result {
            Ok(tack_update::Response::Applying) => self.local.close_ready = true,
            Err(e) => self.update_panel(Panel::UpdateChecking, vec![format!("Error: {e}")]),
            Ok(tack_update::Response::Current) => self.update_panel(
                Panel::UpdateChecking,
                vec![format!(
                    "Tack is up to date - {}",
                    env!("CARGO_PKG_VERSION")
                )],
            ),
            Ok(tack_update::Response::Available(offer)) => {
                offer.manifest.validate()?;
                let rows = vec![
                    format!("Current: {}", env!("CARGO_PKG_VERSION")),
                    format!("Available: {}", offer.manifest.version),
                    format!(
                        "Channel: {} - {}",
                        offer.manifest.channel.label(),
                        &offer.manifest.git_sha[..8]
                    ),
                    offer.notes.chars().take(100).collect(),
                ];
                self.local.update_offer = Some(offer);
                self.update_panel(Panel::UpdateOffer, rows);
            }
            Ok(tack_update::Response::Staged {
                directory,
                manifest,
            }) => {
                manifest.validate()?;
                let expected = self
                    .local
                    .update_offer
                    .as_ref()
                    .ok_or("Missing download offer")?;
                let install = tack_app::updates::installation()?;
                if manifest != expected.manifest
                    || directory != install.join(format!(".tack-update-{}", manifest.version))
                {
                    return Err("Staged update does not match the accepted offer".into());
                }
                self.local.update_stage = Some(directory);
                self.update_panel(
                    Panel::UpdateReady,
                    vec![
                        format!("Verified: {}", manifest.version),
                        "Save your board before restarting.".into(),
                        "Editable icons and profile are preserved.".into(),
                    ],
                );
            }
        }
        if let Some(ui) = self.local.ui.as_mut().filter(|u| u.panel == Panel::Error) {
            ui.message = ui
                .daily_data()
                .and_then(|d| d.rows.first().cloned())
                .unwrap_or_default();
        }
        Ok(())
    }
    pub(in super::super) fn advance_update(&mut self) -> Result<(), AssetError> {
        let panel = self.local.ui.as_ref().map(|u| u.panel);
        if panel == Some(Panel::UpdateOffer) {
            let manifest = self
                .local
                .update_offer
                .as_ref()
                .ok_or("Update offer unavailable")?
                .manifest
                .clone();
            self.update_panel(
                Panel::UpdateChecking,
                vec![
                    "Downloading and verifying update...".into(),
                    "Escape cancels; your board is retained.".into(),
                ],
            );
            return self.operation(Operation::Update(tack_app::updates::Request::Download {
                manifest,
                work: self.work.join("update"),
            }));
        }
        if panel == Some(Panel::UpdateReady) {
            if self.editor.as_ref().is_some_and(|e| e.is_dirty())
                || self.local.worker.active()
                || self.local.profile_pending
                || self.save.active()
                || self.shared.as_ref().is_some_and(|s| s.has_in_flight())
            {
                if let Some(ui) = &mut self.local.ui {
                    ui.message =
                        "Save your board and wait for pending work before restarting.".into();
                }
                self.dirty = true;
                return Ok(());
            }
            let stage = self
                .local
                .update_stage
                .as_ref()
                .ok_or("Update stage unavailable")?;
            let stage = stage.clone();
            self.update_panel(Panel::UpdateApplying, vec!["Preparing restart...".into()]);
            self.operation(Operation::Update(tack_app::updates::Request::Apply {
                stage,
                work: self.work.join("update"),
            }))?;
        }
        Ok(())
    }
}
