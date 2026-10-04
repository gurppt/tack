use crate::timing::{GpuSample, Timings};
use bytemuck::{Pod, Zeroable};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Instant;
use tack_assets::{AssetError, AssetKey, Decoded};
use tack_core::{AssetId, ByteCache, Camera, ImageFiltering, ImageRenderData, Lod, WorldRect};
use wgpu::util::DeviceExt;

const MAX_OBJECTS: usize = 10000;
pub const UPLOAD_BUDGET_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_UPLOADS: usize = 8;
const MAX_IN_FLIGHT: usize = 3;
const BACKGROUND: wgpu::Color = wgpu::Color {
    r: 0.035,
    g: 0.04,
    b: 0.05,
    a: 1.0,
};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Vertex {
    position: [f32; 2],
    uv: [f32; 2],
    opacity: f32,
}

struct Texture {
    bind_group: wgpu::BindGroup,
    nearest: wgpu::BindGroup,
}

#[derive(Clone, Copy)]
pub struct DrawImage {
    pub rect: WorldRect,
    pub key: Option<AssetKey>,
}

/// Stable product representation identity, independent of benchmark u32 keys.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ProductKey {
    pub asset: AssetId,
    pub revision: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum TextureKey {
    Benchmark(AssetKey),
    Product(ProductKey),
}
impl TextureKey {
    fn is_thumbnail(self) -> bool {
        match self {
            Self::Benchmark(k) => k.lod == Lod::Thumbnail,
            Self::Product(_) => true,
        }
    }
}
#[derive(Clone, Copy)]
pub struct DrawProductImage {
    pub data: ImageRenderData,
    pub key: Option<ProductKey>,
}
trait CanvasImage {
    fn quad(&self) -> ([[f64; 2]; 4], [[f32; 2]; 4], f32);
    fn key(&self) -> Option<TextureKey>;
    fn nearest(&self) -> bool;
}
impl CanvasImage for DrawImage {
    fn quad(&self) -> ([[f64; 2]; 4], [[f32; 2]; 4], f32) {
        let r = self.rect;
        (
            [
                [r.x, r.y],
                [r.x, r.y + r.height],
                [r.x + r.width, r.y],
                [r.x + r.width, r.y + r.height],
            ],
            [[0., 0.], [0., 1.], [1., 0.], [1., 1.]],
            1.,
        )
    }
    fn key(&self) -> Option<TextureKey> {
        self.key.map(TextureKey::Benchmark)
    }
    fn nearest(&self) -> bool {
        false
    }
}
/// Numeric adapter used by the GPU and correctness tests; AABB is culling only.
pub fn product_quad(data: ImageRenderData) -> ([[f64; 2]; 4], [[f32; 2]; 4], f32) {
    let t = data.transform;
    let c = t.center();
    let size = t.size();
    let (sin, cos) = t.rotation().sin_cos();
    let flip = t.flips();
    let uv = data.crop.uv_rect();
    let mut corners = [[0.; 2]; 4];
    let mut tex = [[0f32; 2]; 4];
    for (i, [x, y]) in [[0., 0.], [0., 1.], [1., 0.], [1., 1.]]
        .into_iter()
        .enumerate()
    {
        let dx = (x - 0.5) * size[0];
        let dy = (y - 0.5) * size[1];
        corners[i] = [c[0] + cos * dx - sin * dy, c[1] + sin * dx + cos * dy];
        let x = if flip[0] { 1. - x } else { x };
        let y = if flip[1] { 1. - y } else { y };
        tex[i] = [(uv[0] + x * uv[2]) as f32, (uv[1] + y * uv[3]) as f32];
    }
    (corners, tex, data.opacity.value() as f32)
}
impl CanvasImage for DrawProductImage {
    fn quad(&self) -> ([[f64; 2]; 4], [[f32; 2]; 4], f32) {
        product_quad(self.data)
    }
    fn key(&self) -> Option<TextureKey> {
        self.key.map(TextureKey::Product)
    }
    fn nearest(&self) -> bool {
        self.data.filtering == ImageFiltering::Nearest
    }
}

#[derive(Default, Clone, Copy)]
pub struct RenderStats {
    pub gpu_bytes: usize,
    pub evictions: u64,
    pub uploads: usize,
    pub upload_bytes: usize,
    pub completed_submissions: usize,
    pub in_flight: usize,
    pub thumbnail_bytes: usize,
    pub upload_cpu_ms: f64,
}

/// Texture ownership and GPU submission only; no file access or codec calls.
pub struct Gpu {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub adapter_info: wgpu::AdapterInfo,
    pub format: wgpu::TextureFormat,
    pipeline: wgpu::RenderPipeline,
    overlay: crate::overlay::Overlay,
    annotations: Option<Box<crate::annotations::Annotations>>,
    grid: Option<crate::grid::Grid>,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    nearest_sampler: wgpu::Sampler,
    textures: ByteCache<TextureKey, Texture>,
    thumbnails: ByteCache<TextureKey, Texture>,
    placeholder: Texture,
    vertices: Vec<Vertex>,
    buffer: wgpu::Buffer,
    in_flight: Arc<AtomicUsize>,
    completed: Arc<AtomicUsize>,
    uploads: usize,
    upload_bytes: usize,
    upload_cpu_ms: f64,
    timings: Option<Timings>,
    submissions: usize,
}

impl Gpu {
    /// GPU initialization belongs to startup, outside frame timing.
    pub async fn new(
        instance: &wgpu::Instance,
        surface: Option<&wgpu::Surface<'_>>,
        format: wgpu::TextureFormat,
        budget_bytes: usize,
    ) -> Result<Self, AssetError> {
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: surface,
                force_fallback_adapter: false,
            })
            .await?;
        let format = if let Some(surface) = surface {
            surface
                .get_capabilities(&adapter)
                .formats
                .into_iter()
                .find(|f| f.is_srgb())
                .ok_or("surface has no sRGB format")?
        } else {
            format
        };
        let adapter_info = adapter.get_info();
        let features = adapter.features() & wgpu::Features::TIMESTAMP_QUERY;
        tracing::info!(adapter = %adapter_info.name, backend = ?adapter_info.backend,
            device_type = ?adapter_info.device_type, "renderer initialized");
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("Tack Mission 0"),
                required_limits: wgpu::Limits::default(),
                required_features: features,
                ..Default::default()
            })
            .await?;
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("image"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let nearest_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("canvas"),
            source: wgpu::ShaderSource::Wgsl(include_str!("canvas.wgsl").into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("canvas"),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("canvas"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vertex"),
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Float32],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fragment"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });
        let placeholder = Self::texture(
            &device,
            &queue,
            &layout,
            &sampler,
            &nearest_sampler,
            &Decoded {
                width: 1,
                height: 1,
                rgba: vec![65, 70, 82, 255],
            },
        );
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("visible quads"),
            contents: &vec![0; MAX_OBJECTS * 6 * std::mem::size_of::<Vertex>()],
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });
        let timings = features
            .contains(wgpu::Features::TIMESTAMP_QUERY)
            .then(|| Timings::new(&device, &queue));
        let overlay = crate::overlay::Overlay::new(&device, format);
        Ok(Self {
            device,
            queue,
            adapter_info,
            format,
            overlay,
            annotations: None,
            grid: None,
            pipeline,
            layout,
            sampler,
            nearest_sampler,
            textures: ByteCache::new(budget_bytes / 2),
            thumbnails: ByteCache::new(budget_bytes - budget_bytes / 2),
            placeholder,
            vertices: Vec::with_capacity(MAX_OBJECTS * 6),
            buffer,
            in_flight: Arc::new(AtomicUsize::new(0)),
            completed: Arc::new(AtomicUsize::new(0)),
            uploads: 0,
            upload_bytes: 0,
            upload_cpu_ms: 0.0,
            timings,
            submissions: 0,
        })
    }

    fn texture(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
        nearest_sampler: &wgpu::Sampler,
        image: &Decoded,
    ) -> Texture {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("display LOD"),
            size: wgpu::Extent3d {
                width: image.width,
                height: image.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &image.rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(image.width * 4),
                rows_per_image: Some(image.height),
            },
            texture.size(),
        );
        let view = texture.create_view(&Default::default());
        let make_group = |sampler: &wgpu::Sampler| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("display LOD"),
                layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(sampler),
                    },
                ],
            })
        };
        Texture {
            bind_group: make_group(sampler),
            nearest: make_group(nearest_sampler),
        }
    }

    pub fn contains(&self, key: AssetKey) -> bool {
        if key.lod == Lod::Thumbnail {
            self.thumbnails.contains(TextureKey::Benchmark(key))
        } else {
            self.textures.contains(TextureKey::Benchmark(key))
        }
    }

    /// Non-blocking GPU completion polling; skip work when submission capacity is full.
    pub fn begin_frame(&mut self) -> Result<bool, AssetError> {
        let _ = self.device.poll(wgpu::PollType::Poll)?;
        if let Some(timings) = &mut self.timings {
            timings.poll();
        }
        self.uploads = 0;
        self.upload_bytes = 0;
        self.upload_cpu_ms = 0.0;
        Ok(self.in_flight.load(Ordering::Relaxed) < MAX_IN_FLIGHT)
    }

    pub fn contains_product(&self, key: ProductKey) -> bool {
        self.thumbnails.contains(TextureKey::Product(key))
    }
    pub fn upload_product(&mut self, key: ProductKey, image: &Decoded) -> bool {
        self.upload_key(TextureKey::Product(key), image)
    }
    pub fn upload(&mut self, key: AssetKey, image: &Decoded) -> bool {
        self.upload_key(TextureKey::Benchmark(key), image)
    }
    fn upload_key(&mut self, key: TextureKey, image: &Decoded) -> bool {
        let bytes = image.rgba.len();
        if (if key.is_thumbnail() {
            self.thumbnails.contains(key)
        } else {
            self.textures.contains(key)
        }) || self.uploads >= MAX_UPLOADS
            || bytes > UPLOAD_BUDGET_BYTES - self.upload_bytes
            || bytes
                > if key.is_thumbnail() {
                    self.thumbnails.budget_bytes()
                } else {
                    self.textures.budget_bytes()
                }
        {
            return false;
        }
        let started = Instant::now();
        let texture = Self::texture(
            &self.device,
            &self.queue,
            &self.layout,
            &self.sampler,
            &self.nearest_sampler,
            image,
        );
        let cache = if key.is_thumbnail() {
            &mut self.thumbnails
        } else {
            &mut self.textures
        };
        if !cache.insert(key, texture, bytes) {
            return false;
        }
        self.uploads += 1;
        self.upload_bytes += bytes;
        self.upload_cpu_ms += started.elapsed().as_secs_f64() * 1000.0;
        true
    }

    pub fn render(
        &mut self,
        target: &wgpu::TextureView,
        camera: &Camera,
        images: &[DrawImage],
    ) -> Result<(), AssetError> {
        self.render_images(target, camera, images, &[], None, None)
    }
    pub fn render_product(
        &mut self,
        target: &wgpu::TextureView,
        camera: &Camera,
        images: &[DrawProductImage],
    ) -> Result<(), AssetError> {
        self.render_images(target, camera, images, &[], None, None)
    }
    pub fn render_product_overlay(
        &mut self,
        target: &wgpu::TextureView,
        camera: &Camera,
        images: &[DrawProductImage],
        overlay: &[crate::OverlayQuad],
    ) -> Result<(), AssetError> {
        self.render_images(target, camera, images, overlay, None, None)
    }
    pub fn render_spatial(
        &mut self,
        target: &wgpu::TextureView,
        camera: &Camera,
        images: &[DrawProductImage],
        overlay: &[crate::OverlayQuad],
        grid: Option<crate::GridView>,
    ) -> Result<(), AssetError> {
        self.render_images(target, camera, images, overlay, grid, None)
    }
    pub fn render_annotated(
        &mut self,
        target: &wgpu::TextureView,
        camera: &Camera,
        images: &[DrawProductImage],
        overlay: &[crate::OverlayQuad],
        grid: Option<crate::GridView>,
        scene: crate::AnnotationDraws<'_>,
    ) -> Result<(), AssetError> {
        self.render_images(target, camera, images, overlay, grid, Some(scene))
    }
    /// Retained instance bytes (CPU/GPU each) and fixed atlas texture payload.
    pub fn annotation_bytes(&self) -> (usize, usize) {
        self.annotations.as_ref().map_or((0, 0), |a| a.bytes())
    }
    fn render_images<T: CanvasImage>(
        &mut self,
        target: &wgpu::TextureView,
        camera: &Camera,
        images: &[T],
        overlay: &[crate::OverlayQuad],
        grid: Option<crate::GridView>,
        scene: Option<crate::AnnotationDraws<'_>>,
    ) -> Result<(), AssetError> {
        if let Some(scene) = scene {
            if scene.order.len() > crate::MAX_ANNOTATION_PRIMITIVES + MAX_OBJECTS
                || scene.primitives.len() > crate::MAX_ANNOTATION_PRIMITIVES
                || scene.order.iter().any(|d| match *d {
                    crate::CanvasDraw::Image(i) => i >= images.len(),
                    crate::CanvasDraw::Annotations { start, end } => {
                        start >= end || end > scene.primitives.len()
                    }
                })
            {
                return Err("invalid annotation draw ranges".into());
            }
            if !scene.primitives.is_empty() {
                let annotations = self.annotations.get_or_insert_with(|| {
                    Box::new(crate::annotations::Annotations::new(
                        &self.device,
                        &self.queue,
                        self.format,
                    ))
                });
                annotations.prepare(&self.device, &self.queue, camera, scene.primitives)?;
            }
        }
        if let Some(view) = grid {
            let dots = self
                .grid
                .get_or_insert_with(|| crate::grid::Grid::new(&self.device, self.format));
            dots.prepare(&self.queue, camera, view);
        }
        if images.len() > MAX_OBJECTS {
            return Err("too many visible objects".into());
        }
        self.vertices.clear();
        for image in images {
            let (points, uvs, opacity) = image.quad();
            for i in [0usize, 1, 2, 2, 1, 3] {
                self.vertices.push(Vertex {
                    position: camera.world_to_clip(points[i]),
                    uv: uvs[i],
                    opacity,
                });
            }
        }
        if !self.vertices.is_empty() {
            self.queue
                .write_buffer(&self.buffer, 0, bytemuck::cast_slice(&self.vertices));
        }
        self.overlay
            .prepare(&self.device, &self.queue, camera, overlay)?;
        let mut encoder = self.device.create_command_encoder(&Default::default());
        let timing_slot = self
            .timings
            .as_mut()
            .and_then(|t| t.reserve(self.submissions));
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("canvas"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(BACKGROUND),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: timing_slot
                    .and_then(|index| self.timings.as_ref().map(|t| t.writes(index))),
                occlusion_query_set: None,
            });
            if grid.is_some()
                && let Some(dots) = &self.grid
            {
                dots.draw(&mut pass);
            }
            pass.set_pipeline(&self.pipeline);
            pass.set_vertex_buffer(0, self.buffer.slice(..));
            let draws = scene.map_or(images.len(), |s| s.order.len());
            for draw in 0..draws {
                let index = if let Some(scene) = scene {
                    match scene.order[draw] {
                        crate::CanvasDraw::Annotations { start, end } => {
                            if let Some(annotations) = &self.annotations {
                                annotations.draw(&mut pass, start, end);
                            }
                            continue;
                        }
                        crate::CanvasDraw::Image(i) => i,
                    }
                } else {
                    draw
                };
                let image = &images[index];
                pass.set_pipeline(&self.pipeline);
                pass.set_vertex_buffer(0, self.buffer.slice(..));
                let texture = image
                    .key()
                    .and_then(|key| {
                        if key.is_thumbnail() {
                            self.thumbnails.get(key)
                        } else {
                            self.textures.get(key)
                        }
                    })
                    .unwrap_or(&self.placeholder);
                pass.set_bind_group(
                    0,
                    if image.nearest() {
                        &texture.nearest
                    } else {
                        &texture.bind_group
                    },
                    &[],
                );
                let first = index as u32 * 6;
                pass.draw(first..first + 6, 0..1);
            }
            self.overlay.draw(&mut pass);
        }
        if let Some(index) = timing_slot
            && let Some(timings) = &self.timings
        {
            timings.resolve(&mut encoder, index);
        }
        self.in_flight.fetch_add(1, Ordering::Relaxed);
        self.queue.submit([encoder.finish()]);
        self.submissions += 1;
        if let Some(index) = timing_slot
            && let Some(timings) = &self.timings
        {
            timings.map(index);
        }
        let in_flight = Arc::clone(&self.in_flight);
        let completed = Arc::clone(&self.completed);
        self.queue.on_submitted_work_done(move || {
            in_flight.fetch_sub(1, Ordering::Relaxed);
            completed.fetch_add(1, Ordering::Relaxed);
        });
        Ok(())
    }

    pub fn stats(&self) -> RenderStats {
        RenderStats {
            gpu_bytes: self.textures.used_bytes() + self.thumbnails.used_bytes(),
            evictions: self.textures.evictions() + self.thumbnails.evictions(),
            uploads: self.uploads,
            upload_bytes: self.upload_bytes,
            completed_submissions: self.completed.load(Ordering::Relaxed),
            in_flight: self.in_flight.load(Ordering::Relaxed),
            thumbnail_bytes: self.thumbnails.used_bytes(),
            upload_cpu_ms: self.upload_cpu_ms,
        }
    }

    pub fn timing_samples(&self) -> Option<&[GpuSample]> {
        self.timings.as_ref().map(Timings::samples)
    }

    pub fn dropped_timings(&self) -> usize {
        self.timings.as_ref().map_or(0, |t| t.dropped)
    }

    pub fn has_pending_timings(&self) -> bool {
        self.timings.as_ref().is_some_and(Timings::pending)
    }
}
