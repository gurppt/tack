use crate::timing::{GpuSample, Timings};
use bytemuck::{Pod, Zeroable};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use tack_assets::{AssetError, AssetKey, Decoded};
use tack_core::{ByteCache, Camera, WorldRect};
use wgpu::util::DeviceExt;

const MAX_OBJECTS: usize = 10000;
pub const UPLOAD_BUDGET_BYTES: usize = 16 * 1024 * 1024;
const MAX_UPLOADS: usize = 2;
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
}

struct Texture {
    bind_group: wgpu::BindGroup,
}

#[derive(Clone, Copy)]
pub struct DrawImage {
    pub rect: WorldRect,
    pub key: Option<AssetKey>,
}

#[derive(Default, Clone, Copy)]
pub struct RenderStats {
    pub gpu_bytes: usize,
    pub evictions: u64,
    pub uploads: usize,
    pub upload_bytes: usize,
    pub completed_submissions: usize,
    pub in_flight: usize,
}

/// Texture ownership and GPU submission only; no file access or codec calls.
pub struct Gpu {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub adapter_info: wgpu::AdapterInfo,
    pub format: wgpu::TextureFormat,
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    textures: ByteCache<AssetKey, Texture>,
    placeholder: Texture,
    vertices: Vec<Vertex>,
    buffer: wgpu::Buffer,
    in_flight: Arc<AtomicUsize>,
    completed: Arc<AtomicUsize>,
    uploads: usize,
    upload_bytes: usize,
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
                    attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2],
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
        Ok(Self {
            device,
            queue,
            adapter_info,
            format,
            pipeline,
            layout,
            sampler,
            textures: ByteCache::new(budget_bytes),
            placeholder,
            vertices: Vec::with_capacity(MAX_OBJECTS * 6),
            buffer,
            in_flight: Arc::new(AtomicUsize::new(0)),
            completed: Arc::new(AtomicUsize::new(0)),
            uploads: 0,
            upload_bytes: 0,
            timings,
            submissions: 0,
        })
    }

    fn texture(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
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
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
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
        });
        Texture { bind_group }
    }

    pub fn contains(&self, key: AssetKey) -> bool {
        self.textures.contains(key)
    }

    /// Non-blocking GPU completion polling; skip work when submission capacity is full.
    pub fn begin_frame(&mut self) -> Result<bool, AssetError> {
        let _ = self.device.poll(wgpu::PollType::Poll)?;
        if let Some(timings) = &mut self.timings {
            timings.poll();
        }
        self.uploads = 0;
        self.upload_bytes = 0;
        Ok(self.in_flight.load(Ordering::Relaxed) < MAX_IN_FLIGHT)
    }

    pub fn upload(&mut self, key: AssetKey, image: &Decoded) -> bool {
        let bytes = image.rgba.len();
        if self.contains(key)
            || self.uploads >= MAX_UPLOADS
            || bytes > UPLOAD_BUDGET_BYTES - self.upload_bytes
        {
            return false;
        }
        let texture = Self::texture(
            &self.device,
            &self.queue,
            &self.layout,
            &self.sampler,
            image,
        );
        if !self.textures.insert(key, texture, bytes) {
            return false;
        }
        self.uploads += 1;
        self.upload_bytes += bytes;
        true
    }

    pub fn render(
        &mut self,
        target: &wgpu::TextureView,
        camera: &Camera,
        images: &[DrawImage],
    ) -> Result<(), AssetError> {
        if images.len() > MAX_OBJECTS {
            return Err("too many visible objects".into());
        }
        self.vertices.clear();
        for image in images {
            let r = image.rect;
            for (point, uv) in [
                ([r.x, r.y], [0.0, 0.0]),
                ([r.x, r.y + r.height], [0.0, 1.0]),
                ([r.x + r.width, r.y], [1.0, 0.0]),
                ([r.x + r.width, r.y], [1.0, 0.0]),
                ([r.x, r.y + r.height], [0.0, 1.0]),
                ([r.x + r.width, r.y + r.height], [1.0, 1.0]),
            ] {
                self.vertices.push(Vertex {
                    position: camera.world_to_clip(point),
                    uv,
                });
            }
        }
        if !self.vertices.is_empty() {
            self.queue
                .write_buffer(&self.buffer, 0, bytemuck::cast_slice(&self.vertices));
        }
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
            pass.set_pipeline(&self.pipeline);
            pass.set_vertex_buffer(0, self.buffer.slice(..));
            for (index, image) in images.iter().enumerate() {
                let texture = image
                    .key
                    .and_then(|key| self.textures.get(key))
                    .unwrap_or(&self.placeholder);
                pass.set_bind_group(0, &texture.bind_group, &[]);
                let first = index as u32 * 6;
                pass.draw(first..first + 6, 0..1);
            }
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
            gpu_bytes: self.textures.used_bytes(),
            evictions: self.textures.evictions(),
            uploads: self.uploads,
            upload_bytes: self.upload_bytes,
            completed_submissions: self.completed.load(Ordering::Relaxed),
            in_flight: self.in_flight.load(Ordering::Relaxed),
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
