use std::{sync::mpsc, time::Duration};
use tack_assets::{AssetError, AssetKey, Decoded};
use tack_core::{Camera, Lod, WorldRect};
use tack_render::{DrawImage, Gpu};

#[test]
#[ignore = "requires a working Vulkan, D3D12 or GL adapter"]
fn uploaded_texture_reaches_render_target() -> Result<(), AssetError> {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::from_env_or_default());
    let mut gpu = pollster::block_on(Gpu::new(
        &instance,
        None,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        1024 * 1024,
    ))?;
    let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("smoke target"),
        size: wgpu::Extent3d {
            width: 64,
            height: 64,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: gpu.format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let key = AssetKey {
        id: 0,
        lod: Lod::Thumbnail,
    };
    assert!(gpu.begin_frame()?);
    // A representation larger than its partition must never reach GPU staging.
    assert!(!gpu.upload(
        key,
        &Decoded {
            width: 512,
            height: 512,
            rgba: vec![255; 512 * 512 * 4],
        }
    ));
    assert_eq!(gpu.stats().uploads, 0);
    assert_eq!(gpu.stats().upload_bytes, 0);
    assert_eq!(gpu.stats().gpu_bytes, 0);
    assert!(gpu.upload(
        key,
        &Decoded {
            width: 1,
            height: 1,
            rgba: vec![255, 0, 0, 255]
        }
    ));
    let mut camera = Camera::new([64, 64]);
    camera.set_view([32.0, 32.0], 1.0)?;
    gpu.render(
        &target.create_view(&Default::default()),
        &camera,
        &[DrawImage {
            key: Some(key),
            rect: WorldRect::new(0.0, 0.0, 64.0, 64.0)?,
        }],
    )?;
    let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("smoke readback"),
        size: 64 * 256,
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
                bytes_per_row: Some(256),
                rows_per_image: Some(64),
            },
        },
        target.size(),
    );
    // Readback and waiting are confined to this explicit GPU test.
    gpu.queue.submit([encoder.finish()]);
    let (tx, rx) = mpsc::sync_channel(1);
    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
    gpu.device.poll(wgpu::PollType::Wait {
        submission_index: None,
        timeout: Some(Duration::from_secs(5)),
    })?;
    rx.recv_timeout(Duration::from_secs(5))??;
    let bytes = buffer.slice(..).get_mapped_range();
    let center = 32 * 256 + 32 * 4;
    assert!(bytes[center] > 240 && bytes[center + 1] < 10 && bytes[center + 2] < 10);
    gpu.begin_frame()?;
    if let Some(samples) = gpu.timing_samples() {
        assert_eq!(samples.len(), 1);
        assert!(samples[0].pass_ms.is_finite() && samples[0].pass_ms >= 0.0);
    }
    Ok(())
}
