//! Lazy procedural grid: one uniform and fullscreen triangle in the canvas pass.
use tack_core::Camera;
use wgpu::util::DeviceExt;
#[derive(Clone, Copy, Debug)]
pub struct GridView {
    pub spacing: f64,
    pub dpi: f64,
}
impl GridView {
    pub fn parameters(self, camera: &Camera) -> [f32; 4] {
        let step = self.spacing * camera.zoom();
        let view = camera.viewport();
        [
            (-view.x * camera.zoom()).rem_euclid(step) as f32,
            (-view.y * camera.zoom()).rem_euclid(step) as f32,
            step as f32,
            self.dpi.ceil().max(1.) as f32,
        ]
    }
}
/// Screen-space local presentation; independent of board geometry/storage.
#[derive(Clone, Copy, Debug)]
pub struct Background {
    pub start: [f32; 4],
    pub end: [f32; 4],
    pub grid: [f32; 4],
}
impl Default for Background {
    fn default() -> Self {
        Self {
            start: [0.035, 0.04, 0.05, 1.],
            end: [0.041, 0.046, 0.056, 1.],
            grid: [0.095, 0.12, 0.15, 1.],
        }
    }
}
pub(crate) struct Grid {
    pipeline: wgpu::RenderPipeline,
    buffer: wgpu::Buffer,
    binding: wgpu::BindGroup,
}
impl Grid {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("procedural dots"),
            source: wgpu::ShaderSource::Wgsl(include_str!("grid.wgsl").into()),
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("procedural dots"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vertex"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fragment"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
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
            label: Some("grid parameters"),
            contents: &[0; 80],
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let binding = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("grid parameters"),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });
        Self {
            pipeline,
            buffer,
            binding,
        }
    }
    pub fn prepare(
        &self,
        queue: &wgpu::Queue,
        camera: &Camera,
        view: Option<GridView>,
        background: Background,
    ) {
        let size = camera.screen_size();
        let parameters = [
            view.map_or([0., 0., 1., 1.], |v| v.parameters(camera)),
            background.start,
            background.end,
            background.grid,
            [
                size[0] as f32,
                size[1] as f32,
                if view.is_some() { 1. } else { 0. },
                0.,
            ],
        ];
        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&parameters));
    }
    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>) {
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.binding, &[]);
        pass.draw(0..3, 0..1);
    }
}
