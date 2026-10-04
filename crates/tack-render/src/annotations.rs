//! Lazy, bounded instanced primitives in the existing canvas pass.
use tack_assets::AssetError;
use tack_core::Camera;
use wgpu::util::DeviceExt;
pub const MAX_ANNOTATION_PRIMITIVES: usize = 32768;
pub const NOTE_ATLAS_SIZE: [u32; 2] = [1024, 1824];
static NOTE_ATLAS: &[u8] = include_bytes!("../../../assets/note-font/atlas.png");
static NOTE_SLOTS: &[u8] = include_bytes!("../../../assets/note-font/slots.bin");
/// Deterministic subset metrics. Missing scalars use the existing app bitmap fallback.
pub fn note_glyph_uv(character: char) -> Option<[f32; 4]> {
    let p = (character as usize).checked_mul(2)?;
    let bytes = NOTE_SLOTS.get(p..p + 2)?;
    let slot = u16::from_le_bytes([bytes[0], bytes[1]]);
    (slot != u16::MAX).then(|| {
        let x = f32::from(slot % 32) * 32.;
        let y = f32::from(slot / 32) * 48.;
        [x / 1024., y / 1824., 32. / 1024., 48. / 1824.]
    })
}
#[derive(Clone, Copy, Debug)]
pub struct AnnotationPrimitive {
    pub points: [[f64; 2]; 4],
    /// 1 rectangle, 2 ellipse, 3 capsule, 4 triangle, 5 SDF glyph, 6 bitmap fallback.
    pub kind: u32,
    pub size: [f64; 2],
    pub width: f64,
    pub opacity: f32,
    pub stroke: [f32; 4],
    pub fill: [f32; 4],
    pub atlas_uv: [f32; 4],
    pub bitmap: [u32; 8],
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CanvasDraw {
    Image(usize),
    Annotations { start: usize, end: usize },
}
#[derive(Clone, Copy)]
pub struct AnnotationDraws<'a> {
    pub primitives: &'a [AnnotationPrimitive],
    pub order: &'a [CanvasDraw],
}
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Instance {
    p0: [f32; 2],
    p1: [f32; 2],
    p2: [f32; 2],
    p3: [f32; 2],
    size: [f32; 2],
    width: f32,
    kind: u32,
    stroke: [f32; 4],
    fill: [f32; 4],
    atlas_uv: [f32; 4],
    bits0: [u32; 4],
    bits1: [u32; 4],
}
pub(crate) struct Annotations {
    pipeline: wgpu::RenderPipeline,
    buffer: wgpu::Buffer,
    instances: Vec<Instance>,
    capacity: usize,
    font: Option<wgpu::BindGroup>,
    empty_font: wgpu::BindGroup,
    layout: wgpu::BindGroupLayout,
}
impl Annotations {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("annotation shapes and notes"),
            source: wgpu::ShaderSource::Wgsl(include_str!("annotations.wgsl").into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("note atlas"),
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
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("annotations"),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let pipeline=device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label:Some("instanced annotations"),layout:Some(&pipeline_layout),
            vertex:wgpu::VertexState {module:&shader,entry_point:Some("vertex"),compilation_options:Default::default(),buffers:&[wgpu::VertexBufferLayout {array_stride:std::mem::size_of::<Instance>() as u64,step_mode:wgpu::VertexStepMode::Instance,attributes:&wgpu::vertex_attr_array![0=>Float32x2,1=>Float32x2,2=>Float32x2,3=>Float32x2,4=>Float32x2,5=>Float32,6=>Uint32,7=>Float32x4,8=>Float32x4,9=>Float32x4,10=>Uint32x4,11=>Uint32x4]}]},
            fragment:Some(wgpu::FragmentState {module:&shader,entry_point:Some("fragment"),compilation_options:Default::default(),targets:&[Some(wgpu::ColorTargetState {format,blend:Some(wgpu::BlendState::ALPHA_BLENDING),write_mask:wgpu::ColorWrites::ALL})]}),primitive:Default::default(),depth_stencil:None,multisample:Default::default(),multiview:None,cache:None,
        });
        let capacity = 128;
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("active annotation instances"),
            size: (capacity * std::mem::size_of::<Instance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let empty_font = Self::atlas_bind_group(device, queue, &layout, [1, 1], &[0]);
        Self {
            pipeline,
            buffer,
            instances: Vec::with_capacity(capacity),
            capacity,
            font: None,
            empty_font,
            layout,
        }
    }
    fn atlas_bind_group(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
        size: [u32; 2],
        bytes: &[u8],
    ) -> wgpu::BindGroup {
        let texture = device.create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: Some("lazy note distance atlas"),
                size: wgpu::Extent3d {
                    width: size[0],
                    height: size[1],
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            bytes,
        );
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("smooth note glyphs"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("note atlas"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(
                        &texture.create_view(&Default::default()),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        })
    }
    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        camera: &Camera,
        primitives: &[AnnotationPrimitive],
    ) -> Result<(), AssetError> {
        if primitives.len() > MAX_ANNOTATION_PRIMITIVES {
            return Err("annotation primitive limit".into());
        }
        if self.font.is_none() && primitives.iter().any(|p| p.kind == 5) {
            // Explicit first-use bounded decode of trusted embedded derivative, no I/O.
            let atlas = image::load_from_memory_with_format(NOTE_ATLAS, image::ImageFormat::Png)?
                .into_luma8();
            if [atlas.width(), atlas.height()] != NOTE_ATLAS_SIZE {
                return Err("note atlas dimensions".into());
            }
            self.font = Some(Self::atlas_bind_group(
                device,
                queue,
                &self.layout,
                NOTE_ATLAS_SIZE,
                atlas.as_raw(),
            ));
        }
        if primitives.len() > self.capacity {
            self.capacity = primitives
                .len()
                .next_power_of_two()
                .min(MAX_ANNOTATION_PRIMITIVES);
            self.buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("bounded annotation instances"),
                size: (self.capacity * std::mem::size_of::<Instance>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        self.instances.clear();
        for p in primitives {
            let points = p.points.map(|p| camera.world_to_clip(p));
            self.instances.push(Instance {
                p0: points[0],
                p1: points[1],
                p2: points[2],
                p3: points[3],
                size: p.size.map(|v| (v * camera.zoom()) as f32),
                width: (p.width * camera.zoom()).max(0.75) as f32,
                kind: p.kind,
                stroke: p.stroke,
                fill: p.fill,
                atlas_uv: p.atlas_uv,
                bits0: [
                    if matches!(p.kind, 1 | 2) {
                        p.opacity.to_bits()
                    } else {
                        p.bitmap[0]
                    },
                    p.bitmap[1],
                    p.bitmap[2],
                    p.bitmap[3],
                ],
                bits1: [p.bitmap[4], p.bitmap[5], p.bitmap[6], p.bitmap[7]],
            });
        }
        if !self.instances.is_empty() {
            queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&self.instances));
        }
        Ok(())
    }
    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>, start: usize, end: usize) {
        if start < end && end <= self.instances.len() {
            pass.set_pipeline(&self.pipeline);
            pass.set_vertex_buffer(0, self.buffer.slice(..));
            pass.set_bind_group(0, self.font.as_ref().unwrap_or(&self.empty_font), &[]);
            pass.draw(0..6, start as u32..end as u32);
        }
    }
    pub fn bytes(&self) -> (usize, usize) {
        (
            self.capacity * std::mem::size_of::<Instance>(),
            if self.font.is_some() {
                NOTE_ATLAS_SIZE[0] as usize * NOTE_ATLAS_SIZE[1] as usize
            } else {
                0
            },
        )
    }
}
