//! Bounded untextured primitives in the common canvas pass, after images.
use tack_assets::AssetError;
use tack_core::Camera;
use wgpu::util::DeviceExt;
pub const MAX_OVERLAY_QUADS: usize = 2048;
const INITIAL_QUADS: usize = 128;
#[derive(Clone, Copy, Debug)]
pub struct OverlayQuad {
    pub points: [[f64; 2]; 4],
    pub color: [f32; 4],
    pub bitmap: Option<[u32; 8]>,
}
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 2],
    color: [f32; 4],
    bits0: [u32; 4],
    bits1: [u32; 4],
    bitmap: u32,
    origin: [f32; 2],
    axes: [f32; 4],
}
pub(crate) struct Overlay {
    pipeline: wgpu::RenderPipeline,
    buffer: wgpu::Buffer,
    vertices: Vec<Vertex>,
    capacity: usize,
    grid: crate::pixel_grid::PixelGrid,
}
impl Overlay {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("selection overlay"),
            source: wgpu::ShaderSource::Wgsl(include_str!("overlay.wgsl").into()),
        });
        let grid = crate::pixel_grid::PixelGrid::new(device);
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("pixel overlay"),
            bind_group_layouts: &[&grid.layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("selection overlay"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vertex"),
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0=>Float32x2,1=>Float32x4,2=>Uint32x4,3=>Uint32x4,4=>Uint32,5=>Float32x2,6=>Float32x4],
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
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("bounded overlay vertices"),
            contents: &vec![0; INITIAL_QUADS * 6 * std::mem::size_of::<Vertex>()],
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });
        Self {
            pipeline,
            buffer,
            vertices: Vec::with_capacity(INITIAL_QUADS * 6),
            capacity: INITIAL_QUADS,
            grid,
        }
    }
    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        camera: &Camera,
        quads: &[OverlayQuad],
    ) -> Result<(), AssetError> {
        if quads.len() > MAX_OVERLAY_QUADS {
            return Err("overlay primitive limit".into());
        }
        if quads.len() > self.capacity {
            self.capacity = quads.len().next_power_of_two().min(MAX_OVERLAY_QUADS);
            self.buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("active spatial overlays"),
                size: (self.capacity * 6 * std::mem::size_of::<Vertex>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        self.grid.prepare(queue, camera);
        self.vertices.clear();
        for q in quads {
            let scale = camera.ui_scale();
            let points = q
                .points
                .map(|p| camera.world_to_screen(p).map(|v| v / scale));
            let origin = points[0].map(|v| v as f32);
            let axes = [
                (points[2][0] - points[0][0]) as f32,
                (points[2][1] - points[0][1]) as f32,
                (points[1][0] - points[0][0]) as f32,
                (points[1][1] - points[0][1]) as f32,
            ];
            let lo: [f64; 2] = std::array::from_fn(|i| {
                points
                    .iter()
                    .map(|p| p[i])
                    .fold(f64::INFINITY, f64::min)
                    .floor()
            });
            let hi: [f64; 2] = std::array::from_fn(|i| {
                points
                    .iter()
                    .map(|p| p[i])
                    .fold(f64::NEG_INFINITY, f64::max)
                    .ceil()
            });
            let envelope = [lo, [lo[0], hi[1]], [hi[0], lo[1]], hi];
            for i in [0, 1, 2, 2, 1, 3] {
                self.vertices.push(Vertex {
                    position: camera
                        .world_to_clip(camera.screen_to_world(envelope[i].map(|v| v * scale))),
                    color: q.color,
                    bits0: q.bitmap.map_or([0; 4], |b| [b[0], b[1], b[2], b[3]]),
                    bits1: q.bitmap.map_or([0; 4], |b| [b[4], b[5], b[6], b[7]]),
                    bitmap: u32::from(q.bitmap.is_some()),
                    origin,
                    axes,
                });
            }
        }
        if !self.vertices.is_empty() {
            queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&self.vertices));
        }
        Ok(())
    }
    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>) {
        self.draw_range(pass, 0, usize::MAX);
    }
    /// Split the existing overlay around toolbar icons so popups occlude them.
    pub fn draw_range(&self, pass: &mut wgpu::RenderPass<'_>, start: usize, end: usize) {
        let start = start.saturating_mul(6).min(self.vertices.len()) as u32;
        let end = end.saturating_mul(6).min(self.vertices.len()) as u32;
        if start < end {
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.grid.bind_group, &[]);
            pass.set_vertex_buffer(0, self.buffer.slice(..));
            pass.draw(start..end, 0..1);
        }
    }
}
