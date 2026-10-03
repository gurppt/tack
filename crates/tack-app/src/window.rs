use crate::{Options, benchmark, session::Session};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use tack_app::navigation_input::NavigationInput;
use tack_assets::{AssetError, BenchmarkBoard};
use tack_render::Gpu;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};

struct App {
    options: Options,
    board: Option<BenchmarkBoard>,
    session: Option<Session>,
    window: Option<Arc<Window>>,
    surface: Option<wgpu::Surface<'static>>,
    config: Option<wgpu::SurfaceConfiguration>,
    started: Instant,
    error: Option<AssetError>,
    input: NavigationInput,
    dirty: bool,
    next_frame: Instant,
    drawable: bool,
    occluded: bool,
}

impl App {
    fn initialize(&mut self, event_loop: &ActiveEventLoop) -> Result<(), AssetError> {
        let window = Arc::new(
            event_loop.create_window(
                Window::default_attributes()
                    .with_title("Tack — Mission 0.5")
                    .with_resizable(self.options.scenario.is_none())
                    .with_inner_size(winit::dpi::PhysicalSize::new(1280, 720)),
            )?,
        );
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::from_env_or_default());
        let surface = instance.create_surface(Arc::clone(&window))?;
        let gpu = pollster::block_on(Gpu::new(
            &instance,
            Some(&surface),
            wgpu::TextureFormat::Bgra8UnormSrgb,
            Session::gpu_budget(&self.options),
        ))?;
        let size = window.inner_size();
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: gpu.format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: 2,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
        };
        surface.configure(&gpu.device, &config);
        let board = self.board.take().ok_or("board already consumed")?;
        let mut session = Session::new(board, gpu, self.options.clone(), self.started)?;
        session.camera.resize([config.width, config.height]);
        session.measurements.record_platform_event(serde_json::json!({"event":"initialize", "scale_factor":window.scale_factor(), "size":[config.width,config.height]}));
        self.session = Some(session);
        self.config = Some(config);
        self.surface = Some(surface);
        self.window = Some(window);
        Ok(())
    }

    fn redraw(&mut self) -> Result<(), AssetError> {
        if !self.drawable || self.occluded {
            return Ok(());
        }
        let callback_start = Instant::now();
        self.next_frame = Instant::now() + Duration::from_secs_f64(1.0 / 60.0);
        let (Some(session), Some(surface), Some(config)) =
            (&mut self.session, &self.surface, &self.config)
        else {
            return Ok(());
        };
        let frame = match surface.get_current_texture() {
            Ok(frame) => frame,
            Err(error) => {
                session
                    .measurements
                    .record_surface_error(&error, callback_start.elapsed().as_secs_f64() * 1000.0);
                self.dirty = true;
                match error {
                    wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated => {
                        surface.configure(&session.gpu.device, config);
                        return Ok(());
                    }
                    wgpu::SurfaceError::Timeout | wgpu::SurfaceError::Other => return Ok(()),
                    _ => return Err(error.into()),
                }
            }
        };
        let acquire_ms = callback_start.elapsed().as_secs_f64() * 1000.0;
        let mut present_ms = 0.0;
        if session.frame(&frame.texture.create_view(&Default::default()))? {
            let present_start = Instant::now();
            if let Some(window) = &self.window {
                window.pre_present_notify();
            }
            frame.present();
            present_ms = present_start.elapsed().as_secs_f64() * 1000.0;
            self.dirty = false;
        } else {
            self.dirty = true;
        }
        session.measurements.record_window_costs(
            acquire_ms,
            present_ms,
            callback_start.elapsed().as_secs_f64() * 1000.0,
        );
        Ok(())
    }

    fn handle(&mut self, event: WindowEvent) -> Result<(), AssetError> {
        let Some(session) = &mut self.session else {
            return Ok(());
        };
        self.dirty |= self.input.handle(
            &event,
            &mut session.camera,
            session.options.scenario.is_none(),
        );
        match event {
            WindowEvent::Resized(size) => {
                session.measurements.record_platform_event(
                    serde_json::json!({"event":"resize", "size":[size.width,size.height]}),
                );
                self.drawable = size.width > 0 && size.height > 0;
                if !self.drawable {
                    return Ok(());
                }
                if let (Some(config), Some(surface)) = (&mut self.config, &self.surface) {
                    config.width = size
                        .width
                        .min(session.gpu.device.limits().max_texture_dimension_2d);
                    config.height = size
                        .height
                        .min(session.gpu.device.limits().max_texture_dimension_2d);
                    surface.configure(&session.gpu.device, config);
                    session.camera.resize([config.width, config.height]);
                    self.dirty = true;
                }
            }
            WindowEvent::Occluded(occluded) => {
                session.measurements.record_platform_event(
                    serde_json::json!({"event":"occluded", "occluded":occluded}),
                );
                self.occluded = occluded;
                self.dirty = true;
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                session.measurements.record_platform_event(
                    serde_json::json!({"event":"scale", "scale_factor":scale_factor}),
                );
            }
            _ => {}
        }
        Ok(())
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.session.is_none()
            && let Err(error) = self.initialize(event_loop)
        {
            self.error = Some(error);
            event_loop.exit();
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        if matches!(event, WindowEvent::CloseRequested) {
            event_loop.exit();
            return;
        }
        let result = if matches!(event, WindowEvent::RedrawRequested) {
            self.redraw()
        } else {
            self.handle(event)
        };
        if let Err(error) = result {
            self.error = Some(error);
            event_loop.exit();
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let Some(session) = &mut self.session else {
            return;
        };
        if session.is_finished() {
            event_loop.exit();
            return;
        }
        if !self.drawable || self.occluded {
            event_loop.set_control_flow(if session.options.scenario.is_some() {
                ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(100))
            } else {
                ControlFlow::Wait
            });
            return;
        }
        let active = self.dirty
            || session.options.scenario.is_some()
            || session.loader.stats().pending > 0
            || session.has_ready_uploads();
        let completing = session.gpu.stats().in_flight > 0 || session.gpu.has_pending_timings();
        if completing && let Err(error) = session.gpu.begin_frame() {
            self.error = Some(error);
            event_loop.exit();
            return;
        }
        if active {
            if Instant::now() >= self.next_frame
                && let Some(window) = &self.window
            {
                window.request_redraw();
                self.next_frame = Instant::now() + Duration::from_secs_f64(1.0 / 60.0);
            }
            event_loop.set_control_flow(ControlFlow::WaitUntil(self.next_frame));
        } else if completing {
            event_loop.set_control_flow(ControlFlow::WaitUntil(
                Instant::now() + Duration::from_millis(16),
            ));
        } else {
            event_loop.set_control_flow(ControlFlow::Wait);
        }
    }
}

pub fn run(options: Options, started: Instant) -> Result<(), AssetError> {
    let board = BenchmarkBoard::read_manifest(&options.manifest)?;
    let mut app = App {
        options,
        board: Some(board),
        session: None,
        window: None,
        surface: None,
        config: None,
        started,
        error: None,
        input: NavigationInput::prototype()?,
        dirty: true,
        next_frame: Instant::now(),
        drawable: true,
        occluded: false,
    };
    EventLoop::new()?.run_app(&mut app)?;
    if let Some(error) = app.error {
        return Err(error);
    }
    if let Some(session) = &mut app.session {
        benchmark::save_report(session)?;
    }
    Ok(())
}
