use crate::{Options, session::Session};
use serde::Serialize;
use std::{
    fs, thread,
    time::{Duration, Instant},
};
use tack_assets::{AssetError, Board};
use tack_render::Gpu;

#[derive(Serialize, Clone)]
pub struct Frame {
    pub elapsed_ms: f64,
    pub cpu_ms: f64,
    pub acquire_ms: Option<f64>,
    pub present_ms: Option<f64>,
    pub callback_ms: Option<f64>,
    pub visible: usize,
    pub culled: usize,
    pub placeholders: usize,
    pub lods: [usize; 3],
    pub requested_covered: usize,
    pub cpu_thumbnail_bytes: usize,
    pub gpu_thumbnail_bytes: usize,
    pub upload_cpu_ms: f64,
    pub cpu_cache_bytes: usize,
    pub gpu_resident_bytes: usize,
    pub pending: usize,
    pub stale: u64,
    pub decode_errors: u64,
    pub disk_hits: u64,
    pub decode_ms: f64,
    pub cpu_evictions: u64,
    pub gpu_evictions: u64,
    pub uploads: usize,
    pub upload_bytes: usize,
    pub submitted: bool,
    pub completed_submissions: usize,
    pub in_flight: usize,
    pub preparation_ready: Option<usize>,
}

pub struct Measurements {
    startup_ms: f64,
    frames: Vec<Frame>,
    latest_recorded: bool,
    surface_timeouts: usize,
    surface_reconfigures: usize,
    surface_other_errors: usize,
    failed_acquire_max_ms: f64,
    platform_events: Vec<serde_json::Value>,
}

impl Measurements {
    pub fn new(startup_ms: f64) -> Self {
        Self {
            startup_ms,
            frames: Vec::with_capacity(7200),
            latest_recorded: false,
            surface_timeouts: 0,
            surface_reconfigures: 0,
            surface_other_errors: 0,
            failed_acquire_max_ms: 0.0,
            platform_events: Vec::new(),
        }
    }

    pub fn record_platform_event(&mut self, value: serde_json::Value) {
        if self.platform_events.len() < 256 {
            self.platform_events.push(value);
        }
    }

    pub fn record(&mut self, frame: Frame) {
        // Interactive sessions retain bounded recent telemetry; scripted runs <= 120s.
        self.latest_recorded = self.frames.len() < 7200;
        if self.latest_recorded {
            self.frames.push(frame);
        }
    }

    pub fn record_window_costs(&mut self, acquire_ms: f64, present_ms: f64, callback_ms: f64) {
        if self.latest_recorded
            && let Some(frame) = self.frames.last_mut()
        {
            frame.acquire_ms = Some(acquire_ms);
            frame.present_ms = Some(present_ms);
            frame.callback_ms = Some(callback_ms);
        }
    }

    pub fn record_surface_error(&mut self, error: &wgpu::SurfaceError, acquire_ms: f64) {
        match error {
            wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated => {
                self.surface_reconfigures += 1
            }
            wgpu::SurfaceError::Timeout => self.surface_timeouts += 1,
            _ => self.surface_other_errors += 1,
        }
        self.failed_acquire_max_ms = self.failed_acquire_max_ms.max(acquire_ms);
    }

    pub fn report(&self, session: &Session) -> serde_json::Value {
        let mut cpu: Vec<_> = self.frames.iter().map(|f| f.cpu_ms).collect();
        cpu.sort_by(f64::total_cmp);
        let percentile = |p: f64| {
            cpu.get(((cpu.len().saturating_sub(1)) as f64 * p).ceil() as usize)
                .copied()
        };
        let intervals: Vec<_> = self
            .frames
            .windows(2)
            .map(|f| f[1].elapsed_ms - f[0].elapsed_ms)
            .collect();
        let mut sorted_intervals = intervals.clone();
        sorted_intervals.sort_by(f64::total_cmp);
        let wall = |p: f64| {
            sorted_intervals
                .get(((sorted_intervals.len().saturating_sub(1)) as f64 * p).ceil() as usize)
                .copied()
        };
        let info = &session.gpu.adapter_info;
        let summarize = |mut values: Vec<f64>| {
            values.sort_by(f64::total_cmp);
            let at = |p: f64| {
                values
                    .get(((values.len().saturating_sub(1)) as f64 * p).ceil() as usize)
                    .copied()
            };
            serde_json::json!({"samples": values.len(), "p50_ms": at(0.5), "p99_ms": at(0.99), "max_ms": values.last()})
        };
        let gpu_samples = session.gpu.timing_samples().map(|samples| {
            samples
                .iter()
                .map(|s| serde_json::json!({"submission": s.submission, "pass_ms": s.pass_ms}))
                .collect::<Vec<_>>()
        });
        let streaming = serde_json::json!({
            "thumbnail_decoder": tack_assets::THUMBNAIL_DECODER_ID,
            "platform_events": self.platform_events,
            "pending_limit": session.options.workers * tack_assets::MAX_PENDING_PER_WORKER,
            "upload_count_limit": tack_render::MAX_UPLOADS,
            "worker_retention_peak_bytes": session.loader.stats().worker_retention_peak_bytes,
            "rejected_cache_entries": session.loader.stats().rejected,
            "worker_stage_names": tack_assets::STAGE_NAMES,
            "worker_profiles": session.loader.profiles(),
            "worker_profile_dropped": session.loader.profile_dropped(),
            "coverage_episodes": session.coverage.episodes,
            "coverage_episodes_dropped": session.coverage.dropped,
        });
        let mut report = serde_json::json!({
           "schema": 1, "scenario": session.options.scenario,
           "headless": session.options.headless, "adapter": info.name,
           "backend": format!("{:?}", info.backend), "device_type": format!("{:?}", info.device_type),
           "driver": info.driver, "driver_info": info.driver_info,
           "objects": session.board.objects.len(), "duration_seconds": session.options.seconds,
           "workers": session.options.workers, "prefetch": session.options.prefetch,
           "startup_ms": self.startup_ms,
           "first_visible_content_ms": self.frames.iter().find(|f| f.submitted && f.placeholders < f.visible).map(|f| f.elapsed_ms),
           "cpu_p50_ms": percentile(0.5), "cpu_p99_ms": percentile(0.99), "cpu_max_ms": cpu.last(),
           "interval_p50_ms": wall(0.5), "interval_p99_ms": wall(0.99),
           "gpu_pass": session.gpu.timing_samples().map(|samples| summarize(samples.iter().map(|s| s.pass_ms).collect())),
           "gpu_pass_samples": gpu_samples,
           "gpu_timing_dropped": session.gpu.dropped_timings(),
           "gpu_timing_note": "Optional asynchronous timestamps measure the canvas render pass only, excluding uploads, surface acquisition and presentation. CPU callback and attempt cadence do not measure monitor presentation latency.",
           "acquire": summarize(self.frames.iter().filter_map(|f| f.acquire_ms).collect()),
           "present": summarize(self.frames.iter().filter_map(|f| f.present_ms).collect()),
           "callback": summarize(self.frames.iter().filter_map(|f| f.callback_ms).collect()),
           "surface_timeouts": self.surface_timeouts,
           "surface_reconfigures": self.surface_reconfigures,
           "surface_other_errors": self.surface_other_errors,
           "failed_acquire_max_ms": self.failed_acquire_max_ms,
           "frame_attempts": self.frames.len(), "submitted_frames": self.frames.iter().filter(|f| f.submitted).count(),
           "completed_submissions_at_report": session.gpu.stats().completed_submissions,
           "gpu_backpressure_frames": self.frames.iter().filter(|f| !f.submitted).count(),
           "cpu_cache_peak_bytes": self.frames.iter().map(|f| f.cpu_cache_bytes).max(),
           "gpu_resident_peak_bytes": self.frames.iter().map(|f| f.gpu_resident_bytes).max(),
           "pending_peak": self.frames.iter().map(|f| f.pending).max(),
           "rss_high_water_kib": rss_high_water(),
           "final": self.frames.last(), "frames": self.frames,
        });
        report["prepare"] = serde_json::json!(session.options.prepare);
        report["preparation"] = serde_json::json!(
            session
                .preparation
                .as_ref()
                .map(|p| p.report(&session.loader, session.options.workers))
        );
        if let (Some(report), Some(streaming)) = (report.as_object_mut(), streaming.as_object()) {
            report.extend(
                streaming
                    .iter()
                    .map(|(key, value)| (key.clone(), value.clone())),
            );
        }
        report
    }
}

fn rss_high_water() -> Option<u64> {
    // Report-only filesystem access; never sampled on the frame path.
    fs::read_to_string("/proc/self/status")
        .ok()?
        .lines()
        .find(|line| line.starts_with("VmHWM:"))?
        .split_whitespace()
        .nth(1)?
        .parse()
        .ok()
}

pub fn save_report(session: &mut Session) -> Result<(), AssetError> {
    let drain_started = Instant::now();
    if session.options.scenario.is_some() {
        session.loader.request(&[]);
        let deadline = Instant::now() + Duration::from_secs(30);
        while session.loader.stats().pending > 0 && Instant::now() < deadline {
            session.loader.poll();
            if let Some(preparation) = &mut session.preparation {
                preparation.observe(&session.loader);
            }
            thread::sleep(Duration::from_millis(1));
        }
        if session.loader.stats().pending > 0 {
            return Err("asset drain timed out".into());
        }
    }
    let asset_drain_ms = drain_started.elapsed().as_secs_f64() * 1000.0;
    // Readback/completion draining occurs only after the event loop or scripted run.
    let deadline = Instant::now() + Duration::from_secs(5);
    while (session.gpu.stats().in_flight > 0 || session.gpu.has_pending_timings())
        && Instant::now() < deadline
    {
        session.gpu.begin_frame()?;
        thread::sleep(Duration::from_millis(1));
    }
    let mut value = session.measurements.report(session);
    value["asset_drain_ms"] = asset_drain_ms.into();
    value["asset_pending_after_drain"] = session.loader.stats().pending.into();
    if let Some(path) = &session.options.output {
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, serde_json::to_vec_pretty(&value)?)?;
    }
    let mut summary = value;
    if let Some(object) = summary.as_object_mut() {
        object.remove("frames");
        object.remove("gpu_pass_samples");
        object.remove("worker_profiles");
        object.remove("coverage_episodes");
    }
    println!("{}", serde_json::to_string(&summary)?);
    Ok(())
}

pub fn headless(options: Options, started: Instant) -> Result<(), AssetError> {
    let board = Board::read_manifest(&options.manifest)?;
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::from_env_or_default());
    let gpu = pollster::block_on(Gpu::new(
        &instance,
        None,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        Session::gpu_budget(&options),
    ))?;
    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("offscreen benchmark"),
        size: wgpu::Extent3d {
            width: 1280,
            height: 720,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: gpu.format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = texture.create_view(&Default::default());
    let mut session = Session::new(board, gpu, options, started)?;
    let period = Duration::from_secs_f64(1.0 / 60.0);
    while !session.is_finished() {
        let tick = Instant::now();
        session.frame(&view)?;
        if let Some(remaining) = period.checked_sub(tick.elapsed()) {
            thread::sleep(remaining);
        }
    }
    save_report(&mut session)
}
