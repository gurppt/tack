use crate::{
    Options,
    benchmark::{Frame, Measurements},
};
use std::time::Instant;
use tack_assets::{AssetError, AssetKey, Board, DecodeRequest, Loader};
use tack_core::{Camera, Lod};
use tack_render::{DrawImage, Gpu};

pub struct Session {
    pub board: Board,
    pub camera: Camera,
    pub gpu: Gpu,
    pub loader: Loader,
    pub measurements: Measurements,
    pub options: Options,
    pub started: Instant,
    animation_started: Instant,
    requests: Vec<DecodeRequest>,
    images: Vec<DrawImage>,
}

impl Session {
    pub fn new(
        board: Board,
        gpu: Gpu,
        options: Options,
        started: Instant,
    ) -> Result<Self, AssetError> {
        let pressure = options.scenario.as_deref() == Some("pressure");
        let loader = Loader::new(
            options.cache.clone(),
            if pressure { 16 } else { 64 } * 1024 * 1024,
            512 * 1024 * 1024,
            2,
        )?;
        let mut camera = Camera::new([1280, 720]);
        camera.set_view([6600.0, 5100.0], 0.1)?;
        Ok(Self {
            board,
            camera,
            gpu,
            loader,
            options,
            started,
            animation_started: Instant::now(),
            requests: Vec::new(),
            images: Vec::new(),
            measurements: Measurements::new(started.elapsed().as_secs_f64() * 1000.0),
        })
    }

    pub fn gpu_budget(options: &Options) -> usize {
        if options.scenario.as_deref() == Some("pressure") {
            24 * 1024 * 1024
        } else {
            128 * 1024 * 1024
        }
    }

    pub fn is_finished(&self) -> bool {
        self.options.scenario.is_some()
            && self.animation_started.elapsed().as_secs_f64() >= self.options.seconds
    }

    fn animate(&mut self) -> Result<(), AssetError> {
        let seconds = self.animation_started.elapsed().as_secs_f64();
        let Some(scenario) = self.options.scenario.as_deref() else {
            return Ok(());
        };
        match scenario {
            "pan" => {
                let index = (seconds * 20.0) as usize * 37 % self.board.objects.len();
                let r = self.board.objects[index].rect;
                self.camera
                    .set_view([r.x + r.width / 2.0, r.y + r.height / 2.0], 0.1)?;
            }
            "zoom" => {
                let zoom = 0.003 * ((seconds * 2.0).sin() * 0.5 + 0.5).mul_add(6.0, 0.0).exp();
                self.camera.set_view([6600.0, 5100.0], zoom)?;
            }
            "pressure" => {
                let index = (seconds / 2.0) as usize * 17 % self.board.objects.len();
                let r = self.board.objects[index].rect;
                self.camera
                    .set_view([r.x + r.width / 2.0, r.y + r.height / 2.0], 0.6)?;
            }
            "cold" | "warm" => self.camera.set_view([6600.0, 5100.0], 0.1)?,
            _ => {}
        }
        Ok(())
    }

    pub fn frame(&mut self, target: &wgpu::TextureView) -> Result<bool, AssetError> {
        let frame_start = Instant::now();
        self.animate()?;
        self.loader.poll();
        let available = self.gpu.begin_frame()?;
        self.requests.clear();
        self.images.clear();
        for object in self.board.visible(self.camera.viewport()) {
            let wanted = Lod::for_projected_edge(
                object.rect.width.max(object.rect.height) * self.camera.zoom(),
            );
            let mut best = Lod::ALL
                .into_iter()
                .rev()
                .filter(|lod| *lod <= wanted)
                .map(|lod| AssetKey { id: object.id, lod })
                .find(|key| self.gpu.contains(*key));
            let next = if best.is_none() {
                Lod::Thumbnail
            } else {
                wanted
            };
            let key = AssetKey {
                id: object.id,
                lod: next,
            };
            if !self.gpu.contains(key) {
                if available && let Some(image) = self.loader.get(key) {
                    self.gpu.upload(key, image);
                }
                if !self.gpu.contains(key) {
                    self.requests.push(DecodeRequest {
                        key,
                        path: object.path.clone(),
                        source_sha256: object.source_sha256.clone(),
                    });
                }
                if self.gpu.contains(key) {
                    best = Some(key);
                }
            }
            if best.is_some_and(|key| key.lod < wanted) && next != wanted {
                self.requests.push(DecodeRequest {
                    key: AssetKey {
                        id: object.id,
                        lod: wanted,
                    },
                    path: object.path.clone(),
                    source_sha256: object.source_sha256.clone(),
                });
            }
            self.images.push(DrawImage {
                rect: object.rect,
                key: best,
            });
        }
        let mut lods = [0; 3];
        let mut placeholders = 0;
        // Uploading a later object may evict an earlier texture under pressure.
        // Telemetry must reflect what the renderer will actually draw.
        for image in &mut self.images {
            if image.key.is_some_and(|key| !self.gpu.contains(key)) {
                image.key = None;
            }
            if let Some(key) = image.key {
                let index = match key.lod {
                    Lod::Thumbnail => 0,
                    Lod::Medium => 1,
                    Lod::Detail => 2,
                };
                lods[index] += 1;
            } else {
                placeholders += 1;
            }
        }
        self.loader.request(&self.requests);
        if available {
            self.gpu.render(target, &self.camera, &self.images)?;
        }
        let cpu_ms = frame_start.elapsed().as_secs_f64() * 1000.0;
        let gpu = self.gpu.stats();
        let assets = self.loader.stats();
        self.measurements.record(Frame {
            elapsed_ms: self.started.elapsed().as_secs_f64() * 1000.0,
            cpu_ms,
            acquire_ms: None,
            present_ms: None,
            callback_ms: None,
            visible: self.images.len(),
            culled: self.board.objects.len() - self.images.len(),
            placeholders,
            lods,
            cpu_cache_bytes: assets.cpu_bytes,
            gpu_resident_bytes: gpu.gpu_bytes,
            pending: assets.pending,
            stale: assets.stale,
            decode_errors: assets.errors,
            disk_hits: assets.disk_hits,
            decode_ms: assets.decode_ms,
            cpu_evictions: assets.cpu_evictions,
            gpu_evictions: gpu.evictions,
            uploads: gpu.uploads,
            upload_bytes: gpu.upload_bytes,
            submitted: available,
            completed_submissions: gpu.completed_submissions,
            in_flight: gpu.in_flight,
        });
        Ok(available)
    }

    pub fn has_ready_uploads(&self) -> bool {
        self.requests
            .iter()
            .any(|request| self.loader.has_cached(request.key))
    }
}
