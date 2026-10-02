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
    priorities: Vec<(u8, DecodeRequest)>,
    previous_center: [f64; 2],
    pub coverage: crate::coverage::Coverage,
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
            options.workers,
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
            priorities: Vec::new(),
            previous_center: [6600.0, 5100.0],
            coverage: Default::default(),
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
        crate::navigation::apply(&mut self.camera, &self.board, scenario, seconds)?;
        Ok(())
    }

    pub fn frame(&mut self, target: &wgpu::TextureView) -> Result<bool, AssetError> {
        let frame_start = Instant::now();
        self.animate()?;
        self.loader.poll();
        let available = self.gpu.begin_frame()?;
        self.requests.clear();
        self.priorities.clear();
        self.images.clear();
        let viewport = self.camera.viewport();
        for object in self.board.visible(viewport) {
            let wanted = Lod::for_projected_edge(
                object.rect.width.max(object.rect.height) * self.camera.zoom(),
            );
            // Always supply the cheap tier before medium and requested detail.
            // Upload/cache demand and worker priority use the same ordered pyramid.
            for lod in Lod::ALL.into_iter().filter(|lod| *lod <= wanted) {
                let key = AssetKey { id: object.id, lod };
                if Lod::ALL.into_iter().any(|higher| {
                    higher > lod
                        && higher <= wanted
                        && self.gpu.contains(AssetKey {
                            id: object.id,
                            lod: higher,
                        })
                }) {
                    continue;
                }
                if !self.gpu.contains(key) {
                    if available && let Some(image) = self.loader.get(key) {
                        self.gpu.upload(key, image);
                    }
                    if !self.gpu.contains(key) {
                        let priority = match lod {
                            Lod::Thumbnail => 0,
                            Lod::Medium => 1,
                            Lod::Detail => 2,
                        };
                        self.priorities.push((
                            priority,
                            DecodeRequest {
                                key,
                                path: object.path.clone(),
                                source_sha256: object.source_sha256.clone(),
                            },
                        ));
                    }
                }
            }
            let best = Lod::ALL
                .into_iter()
                .rev()
                .filter(|lod| *lod <= wanted)
                .map(|lod| AssetKey { id: object.id, lod })
                .find(|key| self.gpu.contains(*key));
            self.images.push(DrawImage {
                rect: object.rect,
                key: best,
            });
        }
        self.add_prefetch(viewport);
        self.priorities.sort_by_key(|(priority, _)| *priority);
        self.requests
            .extend(self.priorities.drain(..).map(|(_, request)| request));
        let mut lods = [0; 3];
        let mut placeholders = 0;
        let mut requested_covered = 0;
        let mut visible_lods = Vec::with_capacity(self.images.len());
        // Uploading a later object may evict an earlier texture under pressure.
        // Telemetry must reflect what the renderer will actually draw.
        for (image, object) in self.images.iter_mut().zip(self.board.visible(viewport)) {
            let wanted = Lod::for_projected_edge(
                object.rect.width.max(object.rect.height) * self.camera.zoom(),
            );
            image.key = Lod::ALL
                .into_iter()
                .rev()
                .filter(|lod| *lod <= wanted)
                .map(|lod| AssetKey { id: object.id, lod })
                .find(|key| self.gpu.contains(*key));
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
        for (image, object) in self.images.iter().zip(self.board.visible(viewport)) {
            let wanted = Lod::for_projected_edge(
                object.rect.width.max(object.rect.height) * self.camera.zoom(),
            );
            requested_covered += usize::from(image.key.is_some_and(|key| key.lod >= wanted));
            visible_lods.push((object.id, wanted, image.key.map(|key| key.lod)));
        }
        self.loader.request(&self.requests);
        if available {
            self.gpu.render(target, &self.camera, &self.images)?;
            self.coverage
                .observe(&visible_lods, self.started.elapsed().as_secs_f64() * 1000.0);
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
            requested_covered,
            cpu_thumbnail_bytes: assets.cpu_thumbnail_bytes,
            gpu_thumbnail_bytes: gpu.thumbnail_bytes,
            upload_cpu_ms: gpu.upload_cpu_ms,
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

    fn add_prefetch(&mut self, viewport: tack_core::WorldRect) {
        let center = [
            viewport.x + viewport.width / 2.0,
            viewport.y + viewport.height / 2.0,
        ];
        let delta = [
            center[0] - self.previous_center[0],
            center[1] - self.previous_center[1],
        ];
        self.previous_center = center;
        if self.options.prefetch == "none" {
            return;
        }
        let mut near = viewport;
        near.x -= viewport.width * 0.5;
        near.y -= viewport.height * 0.5;
        near.width *= 2.0;
        near.height *= 2.0;
        if self.options.prefetch == "directional" {
            near.x += if delta[0].abs() > 1.0 {
                delta[0].signum() * viewport.width * 0.5
            } else {
                0.0
            };
            near.y += if delta[1].abs() > 1.0 {
                delta[1].signum() * viewport.height * 0.5
            } else {
                0.0
            };
        }
        // Geometry candidates are bounded by the board; only the nearest 64
        // produce prefetch demand. Visible jobs always sort before these jobs.
        let mut nearby: Vec<_> = self
            .board
            .visible(near)
            .filter(|o| !o.rect.intersects(viewport))
            .collect();
        nearby.sort_by(|a, b| {
            let distance = |o: &tack_assets::ImageObject| {
                (o.rect.x + o.rect.width / 2.0 - center[0])
                    .hypot(o.rect.y + o.rect.height / 2.0 - center[1])
            };
            distance(a).total_cmp(&distance(b))
        });
        for object in nearby.into_iter().take(64) {
            let key = AssetKey {
                id: object.id,
                lod: Lod::Thumbnail,
            };
            if !self.gpu.contains(key) && !self.loader.has_cached(key) {
                self.priorities.push((
                    3,
                    DecodeRequest {
                        key,
                        path: object.path.clone(),
                        source_sha256: object.source_sha256.clone(),
                    },
                ));
            }
        }
    }

    pub fn has_ready_uploads(&self) -> bool {
        self.requests
            .iter()
            .any(|request| self.loader.has_cached(request.key))
    }
}
