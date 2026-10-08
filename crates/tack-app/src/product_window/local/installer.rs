//! Transactional installation: prepare fallible supply before replacing editor authority.
use super::*;
impl App {
    pub(in super::super) fn install_loaded(
        &mut self,
        loaded: LoadedBoard,
        reuse: bool,
    ) -> Result<(), AssetError> {
        let board = Arc::new(loaded.board);
        let limits = if self.options.potato {
            SupplyLimits::potato()
        } else {
            SupplyLimits::default()
        };
        let mut assets = ProductAssets::with_limits(
            Arc::clone(&board),
            &loaded.path,
            self.work.clone(),
            limits,
        )?;
        if self.options.lod_debug {
            assets.enable_lod_diagnostics();
        }
        let mut camera = self.camera;
        camera.set_view([0.; 2], 1.)?;
        let mut extent = [0f64; 2];
        let mut clamped = false;
        if let Some(object) = board
            .document
            .object_order()
            .first()
            .and_then(|id| board.document.object(*id))
        {
            let t = object.transform();
            let center = t.center().map(|v| v.clamp(-1e8, 1e8));
            clamped = center != t.center();
            camera.set_view(center, (600. / t.size()[0]).clamp(0.000001, 1000.))?;
        }
        for data in board
            .document
            .object_order()
            .iter()
            .filter_map(|id| board.document.object_render_data(*id))
        {
            let r = data.transform.bounds();
            extent[0] = extent[0].max(r.x + r.width);
            extent[1] = extent[1].max(r.y + r.height);
        }
        if self.options.annotation_benchmark {
            camera.set_view([600., 300.], 1.)?;
        }
        if self.options.dense {
            camera.set_view([extent[0] / 2., extent[1] / 2.], 0.02)?;
        }
        // Preferences mutations also validate before editor replacement.
        let mut profile = self.local.profile.clone();
        let private_seed = !reuse
            && loaded.path.parent() == Some(self.local.root.as_path())
            && loaded
                .path
                .file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with("untitled-slot-"));
        if (!self.local.untitled && !private_seed) || reuse {
            profile.remember(&loaded.path)?;
            profile.remember_board_directory(&loaded.path)?;
        }
        let mut input = ImageInput::new()?;
        input.keymap = profile.keymap()?;
        input.gizmo.style = self.input.gizmo.style;
        input.gizmo.palette = profile.theme.palette();
        input.gizmo.set_scale(camera.ui_scale());
        if reuse {
            self.release_about();
            self.input.cancel();
            self.input = input;
            self.context = None;
            self.annotations = None;
            self.draws.clear();
            self.save = ImageSave::default();
            self.local.originals.clear();
            self.local.spool = None;
            self.local.cache_generation = None;
            self.local.seed_remembered = false;
            self.local.import_status.clear();
            self.local.import_rejected = 0;
            self.local.recovery = RecoverySchedule::default();
            self.local.recovery_pending = false;
            self.local.ui = None;
            self.local.untitled = false;
            self.options.new = false;
            self.options.untitled = false;
            self.local.retire_seed = self.local.seed.is_some();
            self.navigation_end_pending = 0;
            self.supply_pending = false;
            self.interaction_error = None;
            self.first_content_ms = None;
            self.useful_ms = None;
            if let Some(gpu) = &mut self.gpu {
                gpu.clear_products();
            }
        } else if loaded.path.parent() == Some(self.local.root.as_path())
            && loaded
                .path
                .file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with("untitled-slot-"))
        {
            self.local.untitled = true;
            self.local.seed = Some((Arc::clone(&loaded.lease), board.document.id()));
        }
        self.options.path = loaded.path;
        self.local.lease = Some(loaded.lease);
        self.local.profile = profile;
        self.local.profile_pending |= !self.local.untitled;
        if let Some(previous) = self.assets.replace(assets) {
            // A fresh empty board has no requests; joining still stays off callbacks.
            std::thread::spawn(move || drop(previous));
        }
        self.metadata_ms = loaded.metadata_ms;
        self.camera = camera;
        self.camera_clamped = clamped;
        self.extent = extent;
        self.editor = Some(DocumentEditor::new(board.document.clone(), 200));
        self.board = Some(board);
        self.visibility.invalidate();
        self.load_failed = false;
        if !reuse {
            self.apply_preferences()?;
        }
        self.input.grid_visible = self.local.profile.grid;
        if loaded.recovery {
            self.local.recovery_pending = true;
            self.panel(Panel::Recovery);
        }
        if let Some(warning) = loaded.warning {
            self.local_error(warning);
        }
        self.dirty = true;
        if let Some(window) = &self.window {
            window.set_title("Tack — local board");
        }
        Ok(())
    }
}
