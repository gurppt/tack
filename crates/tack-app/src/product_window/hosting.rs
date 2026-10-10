//! Artist-facing Share/Join using the existing authoritative backend.
use super::*;
use tack_app::{
    actions::Action,
    local_ui::{DailyPanel, Panel},
    local_worker::Operation,
    sharing::Choice,
};
impl App {
    pub(super) fn share_panel(&mut self) -> Result<(), AssetError> {
        let disconnected = self.offline.is_some()
            && self
                .shared
                .as_ref()
                .is_none_or(|s| s.state != tack_shared::client::ConnectionState::Connected);
        let (mut rows, mut choices) = if disconnected {
            (
                vec![
                    "SHARED BOARD OFFLINE".into(),
                    "Editing is disabled until you reconnect.".into(),
                    "RECONNECT".into(),
                    "SAVE TO LOCAL...".into(),
                ],
                vec![
                    Choice::None,
                    Choice::None,
                    if self.shared.is_some() {
                        Choice::Reconnect
                    } else {
                        Choice::Online
                    },
                    Choice::SaveLocal,
                ],
            )
        } else if self.host.is_some() {
            (
                vec![
                    "Board is online from this computer".into(),
                    "Original local board remains on disk".into(),
                    "Copy Invite".into(),
                    "Stop Sharing".into(),
                    "Advanced sharing...".into(),
                ],
                vec![
                    Choice::None,
                    Choice::None,
                    Choice::Copy,
                    Choice::Stop,
                    Choice::Advanced,
                ],
            )
        } else if self.shared.is_some() {
            (
                vec![
                    "Shared board".into(),
                    "Copy Invite".into(),
                    "Reconnect".into(),
                    "Advanced connection...".into(),
                ],
                vec![
                    Choice::None,
                    Choice::Copy,
                    Choice::Reconnect,
                    Choice::Advanced,
                ],
            )
        } else if self.offline.is_some() {
            (
                vec![
                    "Shared copy - offline inspection only".into(),
                    "Open Offline (read only)".into(),
                    "Put Online / Open Invite".into(),
                    "Advanced sharing...".into(),
                ],
                vec![
                    Choice::None,
                    Choice::Offline,
                    Choice::Online,
                    Choice::Advanced,
                ],
            )
        } else {
            (
                vec![
                    "Share from this computer".into(),
                    "Share on a Tack server...".into(),
                ],
                vec![Choice::Host, Choice::Remote],
            )
        };
        let invite = self
            .host
            .as_ref()
            .map(|h| h.descriptor.invite.clone())
            .or_else(|| {
                self.shared
                    .as_ref()
                    .map(|s| format!("tack://{}/{}", s.address, s.board))
            })
            .or_else(|| self.offline.as_ref().map(|d| d.invite.clone()));
        if let Some(invite) = invite {
            let columns = ((f64::from(self.camera.screen_size()[0]) / self.camera.ui_scale() - 64.)
                / 8.)
                .floor()
                .clamp(12., 68.) as usize;
            let mut links = vec!["Invite".into()];
            for chunk in invite.as_bytes().chunks(columns) {
                links.push(String::from_utf8_lossy(chunk).into_owned());
            }
            let count = links.len();
            rows.splice(1..1, links);
            choices.splice(1..1, std::iter::repeat_n(Choice::None, count));
        }
        self.daily_panel(
            Panel::Sharing,
            DailyPanel {
                rows,
                choices,
                ..Default::default()
            },
        );
        Ok(())
    }
    pub(super) fn sharing_choice(&mut self, choice: Choice) -> Result<(), AssetError> {
        match choice {
            Choice::None => {}
            Choice::Host => {
                self.share_remote = None;
                let name = tack_app::sharing::proposed(&self.options.path, self.local.untitled);
                self.daily_panel(
                    Panel::Sharing,
                    DailyPanel {
                        rows: vec![
                            "This computer hosts while Tack is running.".into(),
                            format!(
                                "Shared copy: {}",
                                name.file_name().unwrap_or_default().to_string_lossy()
                            ),
                            "Start Sharing".into(),
                        ],
                        choices: vec![Choice::None, Choice::None, Choice::Start],
                        ..Default::default()
                    },
                );
            }
            Choice::Remote => {
                self.daily_panel(
                    Panel::Server,
                    DailyPanel {
                        text: String::new(),
                        ..Default::default()
                    },
                );
            }
            Choice::Start => self.pick_share_copy()?,
            Choice::SaveLocal => self.local_action(Action::SaveToLocal)?,
            Choice::Copy => self.daily_action(Action::CopySharedBoardAddress)?,
            Choice::Stop => self.stop_hosting(false)?,
            Choice::Offline => self.local.ui = None,
            Choice::Online => self.put_online()?,
            Choice::Reconnect => {
                if let Some(s) = &self.shared {
                    s.client.reconnect()?;
                }
                self.local.ui = None;
            }
            Choice::Advanced => {
                let d = self
                    .host
                    .as_ref()
                    .map(|h| &h.descriptor)
                    .or(self.offline.as_ref());
                let mut rows = vec![
                    "Trusted LAN - no TLS/auth".into(),
                    "Default hosting address: port 7337".into(),
                ];
                if let Some(d) = d {
                    rows.push(d.invite.clone());
                } else if let Some(s) = &self.shared {
                    rows.push(format!("{} / {}", s.address, s.board));
                }
                rows.push("Protocol 2 - shared cache max 512 MiB".into());
                self.daily_panel(
                    Panel::Info,
                    DailyPanel {
                        rows,
                        ..Default::default()
                    },
                );
            }
        }
        self.dirty = true;
        Ok(())
    }
    fn pick_share_copy(&mut self) -> Result<(), AssetError> {
        if self.shared.is_some() || self.offline.is_some() {
            return Err("Shared copies cannot be republished as another authority".into());
        }
        if self.local.importing || self.save.active() || self.local.recovery_pending {
            return Err("Finish imports/save or recovery before sharing".into());
        }
        let suggested = tack_app::sharing::proposed(&self.options.path, self.local.untitled);
        let options = tack_app::native_files::PickOptions {
            directory: if self.local.untitled {
                self.local
                    .profile
                    .last_board_directory
                    .as_ref()
                    .and_then(|p| p.path().ok())
            } else {
                self.options.path.parent().map(PathBuf::from)
            },
            suggested_name: suggested.file_name().map(|s| s.to_os_string()),
        };
        self.operation(Operation::Pick(
            Action::ShareBoard,
            tack_app::native_files::Picker::Save,
            self.work.join("helpers"),
            options,
        ))
    }
    pub(super) fn share_to(&mut self, target: PathBuf) -> Result<(), AssetError> {
        if self.shared.is_some() || self.offline.is_some() {
            return Err("Share an ordinary local board".into());
        }
        let target = tack_app::file_names::board(target);
        let request = tack_app::hosting::ShareRequest {
            document: self
                .editor
                .as_ref()
                .ok_or("document unavailable")?
                .document()
                .clone(),
            board: self.board.clone().ok_or("board unavailable")?,
            originals: self.local.originals.clone(),
            prepared: self
                .assets
                .as_ref()
                .map(|a| a.prepared.clone())
                .unwrap_or_default(),
            source_path: self.options.path.clone(),
            target: target.clone(),
            profile: self.local.root.clone(),
            remote: self.share_remote.take(),
        };
        self.local.profile.remember(&target)?;
        self.local.profile_pending = true;
        self.operation(Operation::Share(
            Box::new(request),
            self.work.join("helpers"),
        ))?;
        self.daily_panel(Panel::Connecting, DailyPanel::default());
        Ok(())
    }
    pub(super) fn put_online(&mut self) -> Result<(), AssetError> {
        let d = self.offline.clone().ok_or("Open a shared copy first")?;
        if self.host.is_some() {
            return self.share_panel();
        }
        if d.owner.is_none() {
            let a = d.address()?;
            self.operation(Operation::Join {
                address: a,
                work: self.work.join("helpers"),
            })?;
        } else {
            let snapshot = self
                .shared_copy
                .clone()
                .unwrap_or_else(|| self.options.path.clone());
            self.operation(Operation::Host {
                profile: self.local.root.clone(),
                snapshot,
                descriptor: d,
            })?;
        }
        self.daily_panel(Panel::Connecting, DailyPanel::default());
        Ok(())
    }
    pub(super) fn abort_shared_transition(&mut self) {
        self.shared = None;
        self.host = None;
        self.offline = None;
        self.shared_copy = None;
        self.input.gesture_waiting = false;
        self.input.images.blocked.clear();
        self.input.images.reservation_pending = false;
        self.local.ui = None;
        self.dirty = true;
    }
    pub(super) fn share_ready(
        &mut self,
        result: Result<Option<tack_app::hosting::ShareReady>, String>,
    ) -> Result<(), AssetError> {
        let Some(ready) = result.map_err(AssetError::from)? else {
            self.local.ui = None;
            return Ok(());
        };
        let address = ready.descriptor.address()?;
        let mut state = shared::SharedState::start(
            address.server.to_string(),
            address.board,
            self.work.join("shared"),
            self.proxy.clone(),
        )?;
        state.transitioning = true;
        self.shared_copy = Some(ready.snapshot);
        self.offline = Some(ready.descriptor);
        self.host = ready.host;
        self.shared = Some(Box::new(state));
        self.interaction_error =
            Some("Sharing this view; the original local file remains on disk".into());
        self.dirty = true;
        Ok(())
    }
    pub(super) fn host_ready(
        &mut self,
        result: Result<tack_app::hosting::Hosted, String>,
    ) -> Result<(), AssetError> {
        let host = result.map_err(AssetError::from)?;
        let a = host.descriptor.address()?;
        self.shared_copy = Some(host.snapshot.clone());
        self.offline = Some(host.descriptor.clone());
        self.host = Some(host);
        let state = shared::SharedState::start(
            a.server.to_string(),
            a.board,
            self.work.join("shared"),
            self.proxy.clone(),
        )?;
        self.shared = Some(Box::new(state));
        self.local.ui = None;
        self.dirty = true;
        Ok(())
    }
    pub(super) fn stop_hosting(&mut self, close: bool) -> Result<(), AssetError> {
        let Some(host) = self.host.take() else {
            return Err("This window does not host a shared board".into());
        };
        self.shared_copy = Some(host.snapshot.clone());
        self.close_after_host = close;
        if let Some(e) = &mut self.editor {
            e.set_shared_writable(false);
        }
        self.shared = None;
        self.local.lease = None;
        self.operation(Operation::StopHost(host))?;
        self.daily_panel(
            Panel::Connecting,
            DailyPanel {
                rows: vec!["Stopping sharing and saving offline copy...".into()],
                ..Default::default()
            },
        );
        Ok(())
    }
    pub(super) fn host_stopped(&mut self, result: Result<(), String>) -> Result<(), AssetError> {
        result.map_err(AssetError::from)?;
        if self.close_after_host {
            self.local.close_ready = true;
            return Ok(());
        }
        if let Some(path) = self.shared_copy.clone() {
            self.local.reload_offline = true;
            self.operation(Operation::OpenBoard(path))?;
        }
        self.dirty = true;
        Ok(())
    }
}
