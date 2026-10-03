//! Native product document composition. I/O/metadata loading and preview supply are workers.
use serde_json::json;
use std::{
    ffi::OsString,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use tack_app::navigation_input::NavigationInput;
use tack_assets::{AssetError, ProductAssets, SourceState};
use tack_core::{Camera, DocumentQuery};
use tack_render::{DrawProductImage, Gpu, ProductKey};
use tack_storage::TackFile;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};
struct OpenOptions {
    path: PathBuf,
    seconds: Option<f64>,
    output: Option<PathBuf>,
    tour: bool,
}
enum Event {
    Loaded(Result<(TackFile, f64), AssetError>),
}
struct App {
    options: OpenOptions,
    started: Instant,
    window: Option<Arc<Window>>,
    gpu: Option<Gpu>,
    surface: Option<wgpu::Surface<'static>>,
    config: Option<wgpu::SurfaceConfiguration>,
    board: Option<Arc<TackFile>>,
    assets: Option<ProductAssets>,
    camera: Camera,
    input: NavigationInput,
    draws: Vec<DrawProductImage>,
    error: Option<AssetError>,
    dirty: bool,
    drawable: bool,
    occluded: bool,
    next_frame: Instant,
    metadata_ms: f64,
    native_startup_ms: f64,
    first_content_ms: Option<f64>,
    useful_ms: Option<f64>,
    frames: Vec<serde_json::Value>,
    extent: [f64; 2],
    work: PathBuf,
    camera_clamped: bool,
    navigation_end_pending: usize,
    drain_ms: f64,
}
impl App {
    fn initialize(&mut self, event_loop: &ActiveEventLoop) -> Result<(), AssetError> {
        let window = Arc::new(
            event_loop.create_window(
                Window::default_attributes()
                    .with_title("Tack — loading local board")
                    .with_inner_size(winit::dpi::PhysicalSize::new(1280, 720)),
            )?,
        );
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::from_env_or_default());
        let surface = instance.create_surface(Arc::clone(&window))?;
        let gpu = pollster::block_on(Gpu::new(
            &instance,
            Some(&surface),
            wgpu::TextureFormat::Bgra8UnormSrgb,
            128 * 1024 * 1024,
        ))?;
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: gpu.format,
            width: 1280,
            height: 720,
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: 2,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
        };
        surface.configure(&gpu.device, &config);
        self.window = Some(window);
        self.gpu = Some(gpu);
        self.surface = Some(surface);
        self.config = Some(config);
        self.native_startup_ms = self.started.elapsed().as_secs_f64() * 1000.;
        Ok(())
    }
    fn loaded(&mut self, board: TackFile, ms: f64) -> Result<(), AssetError> {
        self.metadata_ms = ms;
        let board = Arc::new(board);
        if let Some(id) = board.document.object_order().first() {
            let o = board.document.object(*id).ok_or("document order")?;
            let t = o.transform();
            let center = t.center().map(|v| v.clamp(-1e8, 1e8));
            self.camera_clamped = center != t.center();
            self.camera
                .set_view(center, (600. / t.size()[0]).clamp(0.000001, 1000.))?;
        }
        for data in board
            .document
            .object_order()
            .iter()
            .filter_map(|id| board.document.object_render_data(*id))
        {
            let r = data.transform.bounds();
            self.extent[0] = self.extent[0].max(r.x + r.width);
            self.extent[1] = self.extent[1].max(r.y + r.height);
        }
        self.assets = Some(ProductAssets::new(
            Arc::clone(&board),
            &self.options.path,
            self.work.clone(),
        )?);
        self.board = Some(board);
        self.dirty = true;
        if let Some(w) = &self.window {
            w.set_title("Tack — local board");
        }
        Ok(())
    }
    fn redraw(&mut self) -> Result<(), AssetError> {
        if !self.drawable || self.occluded {
            return Ok(());
        }
        let start = Instant::now();
        let (Some(gpu), Some(surface), Some(config)) = (&mut self.gpu, &self.surface, &self.config)
        else {
            return Ok(());
        };
        if !gpu.begin_frame()? {
            self.dirty = true;
            return Ok(());
        }
        let frame = match surface.get_current_texture() {
            Ok(f) => f,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                surface.configure(&gpu.device, config);
                self.dirty = true;
                return Ok(());
            }
            Err(wgpu::SurfaceError::Timeout | wgpu::SurfaceError::Other) => {
                self.dirty = true;
                return Ok(());
            }
            Err(e) => return Err(e.into()),
        };
        let acquire_ms = start.elapsed().as_secs_f64() * 1000.;
        let cpu_start = Instant::now();
        self.draws.clear();
        let mut recognizable = 0;
        if let (Some(board), Some(assets)) = (&self.board, &mut self.assets) {
            assets.poll();
            if self.options.tour {
                let fraction = (self.started.elapsed().as_secs_f64() / 12.).min(1.);
                self.camera.set_view(
                    [
                        self.extent[0] / 2.,
                        4500. + fraction * (self.extent[1] - 9000.).max(0.),
                    ],
                    0.006,
                )?;
            }
            for data in board.document.objects_in_view(self.camera.viewport()) {
                let source = board
                    .document
                    .asset(data.asset_id)
                    .and_then(|a| board.document.source(a.source_id()))
                    .ok_or("render source reference")?;
                let key = ProductKey {
                    asset: data.asset_id,
                    revision: source.revision(),
                };
                if !gpu.contains_product(key) {
                    if let Some(image) = assets.get(data.asset_id) {
                        gpu.upload_product(key, image);
                    } else {
                        assets.request(data.asset_id);
                    }
                }
                let key = gpu.contains_product(key).then_some(key);

                self.draws.push(DrawProductImage { data, key });
                if self.draws.len() > 10000 {
                    return Err("viewport exceeds current 10000-object renderer limit".into());
                }
            }
        }
        // A later upload may evict an earlier visible texture.
        for draw in &mut self.draws {
            draw.key = draw.key.filter(|key| gpu.contains_product(*key));
            recognizable += usize::from(draw.key.is_some());
        }
        gpu.render_product(
            &frame.texture.create_view(&Default::default()),
            &self.camera,
            &self.draws,
        )?;
        let cpu_ms = cpu_start.elapsed().as_secs_f64() * 1000.;
        let elapsed_ms = self.started.elapsed().as_secs_f64() * 1000.;
        if recognizable > 0 && self.first_content_ms.is_none() {
            self.first_content_ms = Some(elapsed_ms);
        }
        if !self.draws.is_empty()
            && recognizable * 100 >= self.draws.len() * 80
            && self.useful_ms.is_none()
        {
            self.useful_ms = Some(elapsed_ms);
        }
        let stats = gpu.stats();
        let present_start = Instant::now();
        if let Some(w) = &self.window {
            w.pre_present_notify();
        }
        frame.present();
        let present_ms = present_start.elapsed().as_secs_f64() * 1000.;
        if self.options.output.is_some() && self.frames.len() < 7200 {
            self.frames.push(json!({"elapsed_ms":elapsed_ms,"cpu_ms":cpu_ms,"visible":self.draws.len(),"recognizable":recognizable,"upload_cpu_ms":stats.upload_cpu_ms,"upload_bytes":stats.upload_bytes,"uploads":stats.uploads,"gpu_bytes":stats.gpu_bytes,"in_flight":stats.in_flight,"acquire_ms":acquire_ms,"present_ms":present_ms,"callback_ms":start.elapsed().as_secs_f64()*1000.}));
        }
        self.dirty = self.draws.iter().any(|d| {
            d.key.is_none()
                && self
                    .assets
                    .as_ref()
                    .is_some_and(|a| !a.failed(d.data.asset_id))
        });
        Ok(())
    }
    fn drain(&mut self) -> Result<(), AssetError> {
        self.navigation_end_pending = self.assets.as_ref().map(|a| a.stats().pending).unwrap_or(0);
        let start = Instant::now();
        loop {
            if let Some(a) = &mut self.assets {
                a.poll();
            }
            if let Some(g) = &mut self.gpu {
                g.begin_frame()?;
            }
            let pending = self.assets.as_ref().is_some_and(|a| a.stats().pending > 0);
            let gpu_pending = self
                .gpu
                .as_ref()
                .is_some_and(|g| g.stats().in_flight > 0 || g.has_pending_timings());
            if !pending && !gpu_pending {
                break;
            }
            if start.elapsed() > Duration::from_secs(30) {
                return Err("product shutdown drain timed out".into());
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        self.drain_ms = start.elapsed().as_secs_f64() * 1000.;
        Ok(())
    }
    fn report(&mut self) -> Result<(), AssetError> {
        let Some(path) = &self.options.output else {
            return Ok(());
        };
        let gpu = self.gpu.as_ref();
        let stats = self.assets.as_ref().map(|a| a.stats()).unwrap_or_default();
        let counts = |state| {
            self.assets
                .as_ref()
                .map(|a| a.states.values().filter(|s| **s == state).count())
                .unwrap_or(0)
        };
        let samples: Vec<_> = gpu
            .and_then(|g| g.timing_samples())
            .map(|s| s.iter().map(|s| json!({"pass_ms":s.pass_ms})).collect())
            .unwrap_or_default();
        let report = json!({"operation":"open","native_startup_ms":self.native_startup_ms,"camera_clamped":self.camera_clamped,"navigation_end_pending":self.navigation_end_pending,"drain_ms":self.drain_ms,"state_counts_scope":"requested sources only","recognizable_clock":"CPU submission before present","metadata_load_ms":self.metadata_ms,"metadata_bytes":self.board.as_ref().map(|b|b.metadata_bytes_read).unwrap_or(0),"first_recognizable_ms":self.first_content_ms,"ordinary_view_80_percent_ms":self.useful_ms,"source_bytes_before_detail":stats.source_bytes,"container_bytes":stats.container_bytes+self.board.as_ref().map(|b|b.metadata_bytes_read).unwrap_or(0),"overview_reused":stats.reused,"overview_regenerated":stats.regenerated,"errors":stats.errors,"source_missing":counts(SourceState::Missing),"source_changed":counts(SourceState::Changed),"source_foreign":counts(SourceState::Foreign),"peak_pending":stats.peak_pending,"cpu_payload_bytes":stats.cpu_bytes,"adapter":gpu.map(|g|g.adapter_info.name.clone()),"backend":gpu.map(|g|format!("{:?}",g.adapter_info.backend)),"frames":self.frames,"gpu_samples":samples,"detail":"overview-only vertical slice; no detail request issued"});
        crate::report_output::write_new(path, &serde_json::to_vec_pretty(&report)?)?;
        Ok(())
    }
}
impl ApplicationHandler<Event> for App {
    fn resumed(&mut self, e: &ActiveEventLoop) {
        if self.window.is_none()
            && let Err(error) = self.initialize(e)
        {
            self.error = Some(error);
            e.exit();
        }
    }
    fn user_event(&mut self, e: &ActiveEventLoop, event: Event) {
        let Event::Loaded(result) = event;
        let result = result.and_then(|(b, ms)| self.loaded(b, ms));
        if let Err(error) = result {
            self.error = Some(error);
            e.exit();
        }
    }
    fn window_event(&mut self, e: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        if matches!(event, WindowEvent::CloseRequested) {
            e.exit();
            return;
        }
        let result = if matches!(event, WindowEvent::RedrawRequested) {
            self.redraw()
        } else {
            self.dirty |= self
                .input
                .handle(&event, &mut self.camera, !self.options.tour);
            match event {
                WindowEvent::Resized(size) => {
                    self.drawable = size.width > 0 && size.height > 0;
                    if self.drawable
                        && let (Some(c), Some(s), Some(g)) =
                            (&mut self.config, &self.surface, &self.gpu)
                    {
                        c.width = size.width.min(g.device.limits().max_texture_dimension_2d);
                        c.height = size.height.min(g.device.limits().max_texture_dimension_2d);
                        self.camera.resize([c.width, c.height]);
                        s.configure(&g.device, c);
                        self.dirty = true;
                    }
                }
                WindowEvent::Occluded(b) => self.occluded = b,
                _ => {}
            }
            Ok(())
        };
        if let Err(error) = result {
            self.error = Some(error);
            e.exit();
        }
    }
    fn about_to_wait(&mut self, e: &ActiveEventLoop) {
        if self
            .options
            .seconds
            .is_some_and(|s| self.started.elapsed().as_secs_f64() >= s)
        {
            e.exit();
            return;
        }
        if let Some(a) = &mut self.assets {
            self.dirty |= a.poll();
            if let Some(w) = &self.window {
                let missing = a
                    .states
                    .values()
                    .filter(|s| {
                        matches!(
                            s,
                            SourceState::Missing | SourceState::Foreign | SourceState::Unavailable
                        )
                    })
                    .count();
                let changed = a
                    .states
                    .values()
                    .filter(|s| **s == SourceState::Changed)
                    .count();
                if self.dirty && (missing > 0 || changed > 0) {
                    w.set_title(&format!(
                        "Tack — {missing} missing / {changed} changed sources · last-known previews"
                    ));
                }
            }
        }
        let pending = self.assets.as_ref().is_some_and(|a| a.stats().pending > 0);
        let completing = self
            .gpu
            .as_ref()
            .is_some_and(|g| g.stats().in_flight > 0 || g.has_pending_timings());
        if completing
            && let Some(g) = &mut self.gpu
            && let Err(error) = g.begin_frame()
        {
            self.error = Some(error);
            e.exit();
            return;
        }
        let active = (self.dirty || self.options.tour) && self.drawable && !self.occluded;
        if active && self.drawable && !self.occluded && Instant::now() >= self.next_frame {
            if let Some(w) = &self.window {
                w.request_redraw();
            }
            self.next_frame = Instant::now() + Duration::from_millis(16);
        }
        e.set_control_flow(
            if active || pending || completing || self.options.seconds.is_some() {
                ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(16))
            } else {
                ControlFlow::Wait
            },
        );
    }
}
pub fn run(args: Vec<OsString>, started: Instant) -> Result<(), AssetError> {
    let path = PathBuf::from(
        args.first()
            .ok_or("open FILE.tack [--seconds N] [--output REPORT] [--board-tour]")?,
    );
    let mut options = OpenOptions {
        path,
        seconds: None,
        output: None,
        tour: false,
    };
    let mut it = args.into_iter().skip(1);
    while let Some(a) = it.next() {
        match a.to_str() {
            Some("--seconds") => {
                let value = it.next().ok_or("duration")?;
                let seconds: f64 = value.to_str().ok_or("duration text")?.parse()?;
                if !(1.0..=120.).contains(&seconds) {
                    return Err("duration 1..120 seconds".into());
                }
                options.seconds = Some(seconds);
            }
            Some("--output") => {
                options.output = Some(PathBuf::from(it.next().ok_or("output path")?))
            }
            Some("--board-tour") => options.tour = true,
            _ => return Err("unknown open option".into()),
        }
    }
    crate::report_output::preflight(options.output.as_deref(), Some(&options.path))?;
    let events = EventLoop::<Event>::with_user_event().build()?;
    let proxy = events.create_proxy();
    let input = options.path.clone();
    std::thread::Builder::new()
        .name("tack-document-read".into())
        .spawn(move || {
            let start = Instant::now();
            let result = TackFile::open(input)
                .map(|b| (b, start.elapsed().as_secs_f64() * 1000.))
                .map_err(|e| Box::new(e) as AssetError);
            let _ = proxy.send_event(Event::Loaded(result));
        })?;
    let work = std::env::temp_dir().join(format!(
        "tack-product-open-{:032x}",
        tack_storage::new_document_id()?.value()
    ));
    let mut app = App {
        options,
        started,
        window: None,
        gpu: None,
        surface: None,
        config: None,
        board: None,
        assets: None,
        camera: Camera::new([1280, 720]),
        input: NavigationInput::prototype()?,
        draws: Vec::with_capacity(10000),
        error: None,
        dirty: true,
        drawable: true,
        occluded: false,
        next_frame: Instant::now(),
        metadata_ms: 0.,
        native_startup_ms: 0.,
        first_content_ms: None,
        useful_ms: None,
        frames: Vec::new(),
        extent: [0.; 2],
        work: work.clone(),
        camera_clamped: false,
        navigation_end_pending: 0,
        drain_ms: 0.,
    };
    events.run_app(&mut app)?;
    app.drain()?;
    app.report()?;
    let error = app.error.take();
    drop(app);
    let _ = std::fs::remove_dir_all(work);
    if let Some(error) = error {
        return Err(error);
    }
    Ok(())
}
