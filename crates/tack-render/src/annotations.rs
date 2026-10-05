//! Lazy, bounded instanced primitives in the existing canvas pass.
use tack_assets::AssetError;
use tack_core::Camera;
pub const MAX_ANNOTATION_PRIMITIVES: usize = 32768;
#[derive(Clone, Copy, Debug)]
pub struct AnnotationPrimitive {
    pub points: [[f64; 2]; 4],
    /// 1 rectangle, 3 square-capped segment, 4 triangle, 6 bitmap glyph, 7 solid box.
    pub kind: u32,
    pub size: [f64; 2],
    pub width: f64,
    pub opacity: f32,
    pub stroke: [f32; 4],
    pub fill: [f32; 4],
    pub mapping: [f32; 4],
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
    mapping: [f32; 4],
    bits0: [u32; 4],
    bits1: [u32; 4],
}
pub(crate) struct Annotations {
    pipeline: wgpu::RenderPipeline,
    buffer: wgpu::Buffer,
    instances: Vec<Instance>,
    capacity: usize,
    grid: crate::pixel_grid::PixelGrid,
}
impl Annotations {
    pub fn new(device: &wgpu::Device, _queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("annotation shapes and notes"),
            source: wgpu::ShaderSource::Wgsl(include_str!("annotations.wgsl").into()),
        });
        let grid = crate::pixel_grid::PixelGrid::new(device);
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("annotations"),
            bind_group_layouts: &[&grid.layout],
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
        Self {
            pipeline,
            buffer,
            instances: Vec::with_capacity(capacity),
            capacity,
            grid,
        }
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
        self.grid.prepare(queue, camera);
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
                width: ((p.width * camera.zoom() / camera.ui_scale())
                    .round()
                    .max(1.)
                    * camera.ui_scale()) as f32,
                kind: p.kind,
                stroke: p.stroke,
                fill: p.fill,
                mapping: p.mapping,
                bits0: [
                    if p.kind == 1 {
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
            pass.set_bind_group(0, &self.grid.bind_group, &[]);
            pass.draw(0..6, start as u32..end as u32);
        }
    }
    pub fn bytes(&self) -> (usize, usize) {
        (self.capacity * std::mem::size_of::<Instance>(), 0)
    }
}
