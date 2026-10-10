//! One tiny nearest atlas and at most 32 quads, using the existing image pipeline.
use super::{Gpu, Texture, Vertex};
use tack_assets::{AssetError, Decoded};
use tack_core::Camera;
#[derive(Clone, Copy, Default)]
pub struct UiIcon {
    pub rect: [f64; 4],
    pub index: usize,
    pub disabled: bool,
}
pub(super) struct UiIcons {
    texture: Texture,
    rows: usize,
    buffer: wgpu::Buffer,
    icons: [UiIcon; 32],
    count: usize,
    pub(super) overlay_split: Option<usize>,
}
impl UiIcons {
    pub(super) fn prepare(&self, queue: &wgpu::Queue, camera: &Camera) {
        let blank = Vertex {
            position: [0.; 2],
            uv: [0.; 2],
            opacity: 1.,
        };
        let mut vertices = [blank; 192];
        for (j, icon) in self.icons[..self.count].iter().enumerate() {
            let [x, y, w, h] = icon.rect;
            let i = icon.index;
            let u = (i % 8) as f32 / 8.;
            let v = (i / 8) as f32 / self.rows as f32;
            let points = [[x, y], [x, y + h], [x + w, y], [x + w, y + h]];
            let uv = [
                [u, v],
                [u, v + 1. / self.rows as f32],
                [u + 0.125, v],
                [u + 0.125, v + 1. / self.rows as f32],
            ];
            for (k, p) in [0, 1, 2, 2, 1, 3].into_iter().enumerate() {
                vertices[j * 6 + k] = Vertex {
                    position: camera.world_to_clip(camera.screen_to_world(points[p])),
                    uv: uv[p],
                    opacity: if icon.disabled { 0.35 } else { 1. },
                };
            }
        }
        if self.count > 0 {
            queue.write_buffer(
                &self.buffer,
                0,
                bytemuck::cast_slice(&vertices[..self.count * 6]),
            );
        }
    }
    pub(super) fn draw(&self, pass: &mut wgpu::RenderPass<'_>, pipeline: &wgpu::RenderPipeline) {
        if self.count == 0 {
            return;
        }
        pass.set_pipeline(pipeline);
        pass.set_vertex_buffer(0, self.buffer.slice(..));
        pass.set_bind_group(0, &self.texture.nearest, &[]);
        pass.draw(0..self.count as u32 * 6, 0..1);
    }
}
impl Gpu {
    pub fn set_ui_icon_atlas(&mut self, image: &Decoded) -> Result<(), AssetError> {
        if image.width != 128
            || !matches!(image.height, 32 | 64 | 80)
            || image.rgba.len() != (128 * image.height * 4) as usize
        {
            return Err("UI icon atlas dimensions".into());
        }
        self.ui_icons = Some(Box::new(UiIcons {
            rows: image.height as usize / 16,
            texture: Self::texture(
                &self.device,
                &self.queue,
                &self.layout,
                &self.sampler,
                &self.nearest_sampler,
                image,
            ),
            buffer: self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("32 primitive UI icon quads"),
                size: 192 * std::mem::size_of::<Vertex>() as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            icons: [UiIcon::default(); 32],
            count: 0,
            overlay_split: None,
        }));
        Ok(())
    }
    pub fn set_ui_icons(&mut self, icons: &[UiIcon]) -> Result<(), AssetError> {
        if icons.len() > 32
            || icons.iter().any(|i| {
                i.index >= self.ui_icons.as_ref().map_or(16, |a| a.rows * 8)
                    || i.rect.iter().any(|v| !v.is_finite())
            })
        {
            return Err("UI icon draw bound".into());
        }
        if let Some(atlas) = &mut self.ui_icons {
            atlas.icons[..icons.len()].copy_from_slice(icons);
            atlas.count = icons.len();
        }
        Ok(())
    }
    pub fn set_ui_icon_overlay_split(&mut self, split: Option<usize>) {
        if let Some(atlas) = &mut self.ui_icons {
            atlas.overlay_split = split;
        }
    }
    pub fn clear_ui_icons(&mut self) {
        self.ui_icons = None;
    }
    pub fn ui_icon_bytes(&self) -> usize {
        if self.ui_icons.is_some() {
            self.ui_icons.as_ref().map_or(0, |a| 128 * a.rows * 16 * 4)
                + 192 * std::mem::size_of::<Vertex>()
        } else {
            0
        }
    }
}
