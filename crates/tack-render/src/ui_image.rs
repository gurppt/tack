//! One lazy UI texture, reusing the image pipeline in the existing canvas pass.
use super::{Gpu, Texture, Vertex};
use tack_assets::{AssetError, Decoded};
use tack_core::Camera;
use wgpu::util::DeviceExt;

pub(super) struct UiImage {
    texture: Texture,
    buffer: wgpu::Buffer,
    rect: [f64; 4],
    bytes: usize,
}
impl UiImage {
    pub(super) fn prepare(&self, queue: &wgpu::Queue, camera: &Camera) {
        let [x, y, w, h] = self.rect;
        let points = [[x, y], [x, y + h], [x + w, y], [x + w, y + h]];
        let uvs = [[0., 0.], [0., 1.], [1., 0.], [1., 1.]];
        let vertices: [Vertex; 6] = [0, 1, 2, 2, 1, 3].map(|i| Vertex {
            position: camera.world_to_clip(camera.screen_to_world(points[i])),
            uv: uvs[i],
            opacity: 1.,
        });
        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&vertices));
    }
    pub(super) fn draw(&self, pass: &mut wgpu::RenderPass<'_>, pipeline: &wgpu::RenderPipeline) {
        pass.set_pipeline(pipeline);
        pass.set_vertex_buffer(0, self.buffer.slice(..));
        pass.set_bind_group(0, &self.texture.nearest, &[]);
        pass.draw(0..6, 0..1);
    }
}
impl Gpu {
    pub fn set_ui_image(&mut self, image: &Decoded) -> Result<(), AssetError> {
        if image.width == 0
            || image.height == 0
            || image.width > 256
            || image.height > 256
            || image.rgba.len() != image.width as usize * image.height as usize * 4
        {
            return Err("Invalid compact UI image dimensions/payload".into());
        }
        self.ui_image = Some(Box::new(UiImage {
            texture: Self::texture(
                &self.device,
                &self.queue,
                &self.layout,
                &self.sampler,
                &self.nearest_sampler,
                image,
            ),
            buffer: self
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("active About vertices"),
                    contents: &[0; 6 * std::mem::size_of::<Vertex>()],
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                }),
            rect: [0.; 4],
            bytes: image.rgba.len() + 6 * std::mem::size_of::<Vertex>(),
        }));
        Ok(())
    }
    pub fn set_ui_image_rect(&mut self, rect: [f64; 4]) {
        if let Some(image) = &mut self.ui_image {
            image.rect = rect;
        }
    }
    pub fn clear_ui_image(&mut self) {
        self.ui_image = None;
    }
    pub fn ui_image_bytes(&self) -> usize {
        self.ui_image.as_ref().map_or(0, |image| image.bytes)
    }
}
