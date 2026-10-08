//! Selection rectangles derived by the app; no object identity or selection authority.
use tack_assets::AssetError;
use tack_core::{Camera, Transform};

pub const MAX_SELECTION_RECTS: usize = 10_000;
#[derive(Clone, Copy, Debug)]
pub struct SelectionRect {
    pub transform: Transform,
    pub color: [f32; 4],
    /// Logical screen pixels, independent of zoom and DPI.
    pub width: f64,
}
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    origin: [f32; 2],
    axes: [f32; 4],
    color: [f32; 4],
    width: f32,
}
pub(crate) struct Selection {
    pipeline: wgpu::RenderPipeline,
    buffer: wgpu::Buffer,
    vertices: Vec<Vertex>,
    capacity: usize,
    grid: crate::pixel_grid::PixelGrid,
}
impl Selection {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("selection borders"),
            source: wgpu::ShaderSource::Wgsl(include_str!("selection.wgsl").into()),
        });
        let grid = crate::pixel_grid::PixelGrid::new(device);
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("selection borders"),
            bind_group_layouts: &[&grid.layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("selection borders"), layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader, entry_point: Some("vertex"), compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &wgpu::vertex_attr_array![0=>Float32x2,1=>Float32x4,2=>Float32x4,3=>Float32],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader, entry_point: Some("fragment"), compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format, blend: Some(wgpu::BlendState::ALPHA_BLENDING), write_mask: wgpu::ColorWrites::ALL })],
            }),
            primitive: Default::default(), depth_stencil: None, multisample: Default::default(), multiview: None, cache: None,
        });
        Self {
            pipeline,
            buffer: Self::buffer(device, 1),
            vertices: Vec::with_capacity(1),
            capacity: 1,
            grid,
        }
    }
    fn buffer(device: &wgpu::Device, capacity: usize) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("visible selection borders"),
            size: (capacity * std::mem::size_of::<Vertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }
    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        camera: &Camera,
        rects: &[SelectionRect],
    ) -> Result<(), AssetError> {
        if rects.len() > MAX_SELECTION_RECTS
            || rects.iter().any(|r| {
                !r.width.is_finite()
                    || !(0.1..=64.).contains(&r.width)
                    || r.color.iter().any(|v| !v.is_finite())
            })
        {
            return Err("selection rectangle limit or invalid style".into());
        }
        let capacity = rects
            .len()
            .max(1)
            .next_power_of_two()
            .min(MAX_SELECTION_RECTS);
        if capacity > self.capacity || capacity < self.capacity / 4 {
            self.capacity = capacity;
            self.buffer = Self::buffer(device, capacity);
            self.vertices = Vec::with_capacity(capacity);
        }
        self.grid.prepare(queue, camera);
        self.vertices.clear();
        for rect in rects {
            let t = rect.transform;
            let (sin, cos) = t.rotation().sin_cos();
            let world = |p: [f64; 2]| {
                [
                    t.center()[0] + cos * p[0] - sin * p[1],
                    t.center()[1] + sin * p[0] + cos * p[1],
                ]
            };
            let screen = |p| {
                camera
                    .world_to_screen(world(p))
                    .map(|v| v / camera.ui_scale())
            };
            let half = t.size().map(|v| v / 2.);
            let points = [
                [-half[0], -half[1]],
                [-half[0], half[1]],
                [half[0], -half[1]],
                [half[0], half[1]],
            ]
            .map(screen);
            let origin = points[0].map(|v| v as f32);
            let axes = [
                (points[2][0] - points[0][0]) as f32,
                (points[2][1] - points[0][1]) as f32,
                (points[1][0] - points[0][0]) as f32,
                (points[1][1] - points[0][1]) as f32,
            ];
            self.vertices.push(Vertex {
                origin,
                axes,
                color: rect.color,
                width: rect.width as f32,
            });
        }
        if !self.vertices.is_empty() {
            queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&self.vertices));
        }
        Ok(())
    }
    pub fn bytes(&self) -> (usize, usize) {
        (
            self.vertices.capacity() * std::mem::size_of::<Vertex>(),
            self.capacity * std::mem::size_of::<Vertex>(),
        )
    }
    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>) {
        if self.vertices.is_empty() {
            return;
        }
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.grid.bind_group, &[]);
        pass.set_vertex_buffer(0, self.buffer.slice(..));
        pass.draw(0..24, 0..self.vertices.len() as u32);
    }
}
