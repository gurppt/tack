//! Bounded untextured primitives in the common canvas pass, after images.
use tack_assets::AssetError;
use tack_core::Camera;
use wgpu::util::DeviceExt;
pub const MAX_OVERLAY_QUADS: usize = 128;
#[derive(Clone, Copy, Debug)]
pub struct OverlayQuad {
    pub points: [[f64; 2]; 4],
    pub color: [f32; 4],
}
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 2],
    color: [f32; 4],
}
pub(crate) struct Overlay {
    pipeline: wgpu::RenderPipeline,
    buffer: wgpu::Buffer,
    vertices: Vec<Vertex>,
}
impl Overlay {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("selection overlay"),
            source: wgpu::ShaderSource::Wgsl(include_str!("overlay.wgsl").into()),
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("selection overlay"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vertex"),
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0=>Float32x2,1=>Float32x4],
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
            contents: &vec![0; MAX_OVERLAY_QUADS * 6 * std::mem::size_of::<Vertex>()],
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });
        Self {
            pipeline,
            buffer,
            vertices: Vec::with_capacity(MAX_OVERLAY_QUADS * 6),
        }
    }
    pub fn prepare(
        &mut self,
        queue: &wgpu::Queue,
        camera: &Camera,
        quads: &[OverlayQuad],
    ) -> Result<(), AssetError> {
        if quads.len() > MAX_OVERLAY_QUADS {
            return Err("overlay primitive limit".into());
        }
        self.vertices.clear();
        for q in quads {
            for i in [0, 1, 2, 2, 1, 3] {
                self.vertices.push(Vertex {
                    position: camera.world_to_clip(q.points[i]),
                    color: q.color,
                });
            }
        }
        if !self.vertices.is_empty() {
            queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&self.vertices));
        }
        Ok(())
    }
    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>) {
        if !self.vertices.is_empty() {
            pass.set_pipeline(&self.pipeline);
            pass.set_vertex_buffer(0, self.buffer.slice(..));
            pass.draw(0..self.vertices.len() as u32, 0..1);
        }
    }
}
