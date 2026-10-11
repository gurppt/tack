use super::*;
/// Blocking device wait belongs only to this explicit worker operation, never the frame loop.
#[allow(clippy::too_many_arguments)]
pub(super) fn render(
    gpu: &mut Gpu,
    camera: &Camera,
    draws: &[DrawProductImage],
    overlay: &[tack_render::OverlayQuad],
    grid: Option<tack_render::GridView>,
    scene: &AnnotationScene,
    cancel: &AtomicBool,
) -> Result<Decoded, AssetError> {
    let [width, height] = camera.screen_size();
    dimensions(WorldRect {
        x: 0.,
        y: 0.,
        width: f64::from(width),
        height: f64::from(height),
    })?;
    if width > gpu.device.limits().max_texture_dimension_2d
        || height > gpu.device.limits().max_texture_dimension_2d
    {
        return Err("Snapshot exceeds this GPU texture limit".into());
    }
    let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("bounded board snapshot"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: gpu.format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    gpu.render_annotated(
        &target.create_view(&Default::default()),
        camera,
        draws,
        overlay,
        grid,
        tack_render::AnnotationDraws {
            primitives: &scene.primitives,
            order: &scene.order,
        },
    )?;
    let row = (width * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
        * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("snapshot readback"),
        size: u64::from(row) * u64::from(height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &target,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row),
                rows_per_image: Some(height),
            },
        },
        target.size(),
    );
    gpu.queue.submit([encoder.finish()]);
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    buffer.slice(..).map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx.send(r);
    });
    gpu.device.poll(wgpu::PollType::Wait {
        submission_index: None,
        timeout: Some(Duration::from_secs(10)),
    })?;
    rx.recv_timeout(Duration::from_secs(10))??;
    if cancel.load(Ordering::Relaxed) {
        return Err("Snapshot cancelled".into());
    }
    let mapped = buffer.slice(..).get_mapped_range();
    let mut rgba = Vec::with_capacity((width as usize) * (height as usize) * 4);
    for r in mapped.chunks_exact(row as usize) {
        rgba.extend_from_slice(&r[..width as usize * 4]);
    }
    drop(mapped);
    buffer.unmap();
    Ok(Decoded {
        width,
        height,
        rgba,
    })
}
use std::time::Duration;
