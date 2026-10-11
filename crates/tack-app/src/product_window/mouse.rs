//! Native composition for the optional Mulot, using the ordinary import worker.
use super::*;
use tack_app::{actions::Tool, mouse_tool};
impl App {
    pub(super) fn mouse_canvas(&self) -> bool {
        self.pointer[0] >= 0.
            && self.pointer[1] >= 0.
            && self.pointer[0] < f64::from(self.camera.screen_size()[0])
            && !(self.chrome.desktop.visible && self.pointer[1] < 24. * self.camera.ui_scale())
            && self.context.is_none()
            && self.local.ui.is_none()
            && !self.chrome.toolbar.contains_point(self.pointer)
            && !self.chrome.toolbar.grip_hit(self.pointer)
            && self.pointer[1]
                < f64::from(self.camera.screen_size()[1])
                    - if self.local.profile.status_bar
                        || self.chrome.transient
                        || self.shared.is_some()
                    {
                        20. * self.camera.ui_scale()
                    } else {
                        0.
                    }
    }
    pub(super) fn mouse_event(&mut self, event: &WindowEvent, _consumed: bool) {
        let now = Instant::now();
        self.sync_mouse(now);
        if matches!(event, WindowEvent::CursorMoved { .. })
            && let Some(game) = &mut self.mouse.game
        {
            game.pointer(self.pointer);
        }
    }
    fn sync_mouse(&mut self, now: Instant) {
        self.dirty |= self
            .mouse
            .activate(self.input.active_tool() == Tool::Mouse, now);
        if self.input.active_tool() != Tool::Mouse {
            self.cursors.clear_mouse();
            if matches!(
                self.local.queued,
                Some(tack_app::local_worker::Operation::MouseGame { .. })
            ) {
                self.local.queued = None;
            }
            if self.local.worker.loading_mouse() {
                self.local.worker.cancel();
            }
            return;
        }
        if !self.mouse.loading && self.mouse.artwork.is_none() {
            self.mouse.loading = true;
            if let Err(error) = self.operation(tack_app::local_worker::Operation::MouseGame {
                ticket: self.mouse.ticket,
            }) {
                self.abort_mouse();
                self.interaction_error = Some(error.to_string());
            }
        }
    }
    pub(super) fn abort_mouse(&mut self) {
        if self.input.active_tool() == Tool::Mouse {
            self.input.cancel();
            self.input.annotation.tools.reset_pointer();
            if let Some(editor) = &mut self.editor {
                let _ = self.input.dispatch(
                    tack_app::actions::ActionEvent {
                        action: tack_app::actions::Action::SelectTool(Tool::Pointer),
                        phase: tack_app::actions::ActionPhase::Invoke,
                    },
                    editor,
                    &mut self.camera,
                );
            }
        }
        self.dirty |= self.mouse.activate(false, Instant::now());
        self.cursors.clear_mouse();
        if self.local.worker.loading_mouse() {
            self.local.worker.cancel();
        }
        if matches!(
            self.local.queued,
            Some(tack_app::local_worker::Operation::MouseGame { .. })
        ) {
            self.local.queued = None;
        }
        if let Some(window) = &self.window {
            if self.mouse_hidden {
                window.set_cursor_visible(true);
            }
            self.mouse_hidden = false;
            self.cursors.set(window, tack_app::cursors::Kind::Pointer);
        }
        self.cursor_icon = tack_app::cursors::Kind::Pointer;
        self.dirty = true;
    }
    pub(super) fn mouse_cursor(&mut self, e: &ActiveEventLoop) {
        if let Some(art) = &self.mouse.artwork {
            match self.cursors.install_mouse(e, &art.cheese) {
                Err(error) => {
                    self.abort_mouse();
                    self.interaction_error = Some(error.to_string());
                }
                Ok(true) => {
                    if let Some(window) = &self.window {
                        self.cursors.set(window, self.cursor_icon);
                    }
                }
                Ok(false) => {}
            }
        }
        if self.mouse.game.is_some() {
            self.update_cursor();
        }
    }
    pub(super) fn settle_mouse(&mut self) {
        let now = Instant::now();
        self.sync_mouse(now);
        let mut paw = None;
        let mut finished = false;
        if let Some(game) = &mut self.mouse.game {
            let changed = game.tick(now, self.camera.screen_size().map(f64::from));
            self.dirty |= changed;
            if changed && self.options.output.is_some() && self.mouse_trace.len() < 1024 {
                self.mouse_trace.push(json!({"ms":self.started.elapsed().as_secs_f64()*1000.,"state":format!("{:?}",game.state),"frame":game.frame+1,"at":game.at,"pointer":self.pointer,"dock":game.dock_target(),"hidden":game.hidden,"advances":game.advances,"puffs":game.puffs.len()}));
            }
            if changed && game.state == tack_app::mouse_game::State::Run {
                paw = Some(game.at.map(|v| v / self.camera.ui_scale()));
            }
            finished = game.state == tack_app::mouse_game::State::Finished;
        }
        if let Some(at) = paw {
            self.dirty |= self.mouse.motion(at, now);
        }
        if finished {
            self.abort_mouse();
        }
        if self.mouse.game.is_some() {
            self.update_cursor();
        }
        let (changed, fire) = self.mouse.settle(now);
        self.dirty |= changed;
        if fire && let Some(editor) = &self.editor {
            let center = self.camera.screen_to_world(std::array::from_fn(|i| {
                self.pointer[i] + [48., 32.][i] * self.camera.ui_scale()
            }));
            self.mouse_pending = Some((
                editor.document().id(),
                center,
                self.camera.zoom() / self.camera.ui_scale(),
            ));
        }
        let Some((document, center, zoom)) = self.mouse_pending else {
            return;
        };
        if self.load_failed
            || self.local.close_board
            || self
                .local
                .ui
                .as_ref()
                .is_some_and(|ui| ui.panel == tack_app::local_ui::Panel::Close)
            || self.local.close_ready
            || self.local.close_after_save
            || self.local.close_after_discard
            || self
                .editor
                .as_ref()
                .is_none_or(|e| e.document().id() != document)
        {
            self.mouse_pending = None;
            return;
        }
        // Existing operation completion wakes us; waiting adds no new timer/poll.
        if self.local.save_as.is_some()
            || self.local.saving_as
            || self.local.recovery_pending
            || self.save.active()
            || self.local.worker.active()
            || self.local.queued.is_some()
            || self.local.importing
        {
            return;
        }
        self.mouse_pending = None;
        if let Err(error) = self.insert_mouse_artwork(center, zoom) {
            self.interaction_error = Some(error.to_string());
            self.dirty = true;
        }
    }
    fn insert_mouse_artwork(&mut self, center: [f64; 2], zoom: f64) -> Result<(), AssetError> {
        if self.offline.is_some() && self.shared.is_none() {
            return Err("Offline shared copies are read only".into());
        }
        let editor = self.editor.as_mut().ok_or("document unavailable")?;
        if let Some(asset) = mouse_tool::existing_artwork(editor.document()) {
            // Reuse the normal embedded asset; no repeat file read or payload.
            editor.execute(tack_core::Command::AddObject {
                object: mouse_tool::easter_object(
                    tack_storage::new_object_id()?,
                    asset,
                    center,
                    zoom,
                )?,
                index: editor.document().object_order().len(),
            })?;
        } else {
            let request = tack_app::local_import::ImportRequest {
                temporary: None,
                paths: vec![tack_app::toolbar_icons::root().join("poo_easter.png")],
                embedded: true,
                position: center,
                sampling: tack_core::ImageFiltering::Nearest,
                viewport: [self.camera.viewport().width, self.camera.viewport().height],
                mouse_easter_zoom: Some(zoom),
                work: self.work.join("imports"),
                spool: self.local.spool.clone(),
            };
            self.mouse_import_document = self.editor.as_ref().map(|e| e.document().id());
            self.operation(tack_app::local_worker::Operation::Import(request))?;
            self.local.importing = true;
            self.local.import_rejected = 0;
        }
        self.dirty = true;
        Ok(())
    }
}
