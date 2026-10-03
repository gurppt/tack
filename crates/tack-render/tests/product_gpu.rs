#![allow(clippy::unwrap_used)]
use tack_assets::{AssetError, Decoded};
use tack_core::*;
use tack_render::{DrawProductImage, Gpu, OverlayQuad, ProductKey, product_quad};
fn data() -> ImageRenderData {
    ImageRenderData {
        object_id: ObjectId::new(1).unwrap(),
        asset_id: AssetId::new(2).unwrap(),
        transform: Transform::new([32., 32.], [64., 64.], 0., [false, false]).unwrap(),
        crop: Crop::FULL,
        opacity: Opacity::OPAQUE,
        filtering: ImageFiltering::Nearest,
    }
}
#[test]
fn rotated_cropped_flipped_quad_keeps_source_geometry() {
    let mut d = data();
    d.transform = Transform::new(
        [10., 20.],
        [8., 4.],
        std::f64::consts::FRAC_PI_2,
        [true, false],
    )
    .unwrap();
    d.crop = Crop::new(0.1, 0.2, 0.5, 0.7).unwrap();
    d.opacity = Opacity::new(0.3).unwrap();
    let (corners, uv, opacity) = product_quad(d);
    for (a, b) in corners
        .into_iter()
        .zip([[12., 16.], [8., 16.], [12., 24.], [8., 24.]])
    {
        for (a, b) in a.into_iter().zip(b) {
            assert!((a - b).abs() < 1e-9);
        }
    }
    assert!((uv[0][0] - 0.6).abs() < 1e-6);
    assert!((uv[3][0] - 0.1).abs() < 1e-6);
    assert!((uv[3][1] - 0.9).abs() < 1e-6);
    assert!((opacity - 0.3).abs() < 1e-6);
}
fn pixels(gpu: &mut Gpu, d: ImageRenderData, key: ProductKey) -> Result<Vec<u8>, AssetError> {
    pixels_overlay(gpu, d, key, &[])
}
fn pixels_overlay(
    gpu: &mut Gpu,
    d: ImageRenderData,
    key: ProductKey,
    overlay: &[OverlayQuad],
) -> Result<Vec<u8>, AssetError> {
    let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("product readback"),
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
    let mut camera = Camera::new([64, 64]);
    camera.set_view([32., 32.], 1.)?;
    gpu.begin_frame()?;
    gpu.render_product_overlay(
        &target.create_view(&Default::default()),
        &camera,
        &[DrawProductImage {
            data: d,
            key: Some(key),
        }],
        overlay,
    )?;
    let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 64 * 256,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut e = gpu.device.create_command_encoder(&Default::default());
    e.copy_texture_to_buffer(
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
    gpu.queue.submit([e.finish()]);
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    buffer.slice(..).map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx.send(r);
    });
    gpu.device.poll(wgpu::PollType::Wait {
        submission_index: None,
        timeout: Some(std::time::Duration::from_secs(5)),
    })?;
    rx.recv_timeout(std::time::Duration::from_secs(5))??;
    let bytes = buffer.slice(..).get_mapped_range().to_vec();
    Ok(bytes)
}
fn pixel(bytes: &[u8], x: usize, y: usize) -> [u8; 4] {
    bytes[y * 256 + x * 4..y * 256 + x * 4 + 4]
        .try_into()
        .unwrap()
}
#[test]
#[ignore = "explicit native GPU readback"]
fn product_gpu_honors_sampling_alpha_crop_flip_rotation_and_revision() -> Result<(), AssetError> {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::from_env_or_default());
    let mut gpu = pollster::block_on(Gpu::new(
        &instance,
        None,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        1024 * 1024,
    ))?;
    eprintln!(
        "Product GPU: {} {:?}",
        gpu.adapter_info.name, gpu.adapter_info.backend
    );
    let mut d = data();
    let key = ProductKey {
        asset: d.asset_id,
        revision: 1,
    };
    gpu.begin_frame()?;
    assert!(gpu.upload_product(
        key,
        &Decoded {
            width: 2,
            height: 1,
            rgba: vec![255, 0, 0, 255, 0, 0, 255, 255]
        }
    ));
    let nearest = pixels(&mut gpu, d, key)?;
    assert!(pixel(&nearest, 31, 32)[0] > 240);
    assert!(pixel(&nearest, 32, 32)[2] > 240);
    d.filtering = ImageFiltering::Smooth;
    let smooth = pixels(&mut gpu, d, key)?;
    let p = pixel(&smooth, 32, 32);
    assert!(p[0] > 150 && p[2] > 150 && p[0] < 220 && p[2] < 220);
    d.filtering = ImageFiltering::Nearest;
    d.crop = Crop::new(0., 0., 0.5, 1.)?;
    let crop = pixels(&mut gpu, d, key)?;
    assert!(pixel(&crop, 48, 32)[0] > 240);
    d.crop = Crop::FULL;
    d.transform = Transform::new([32., 32.], [64., 64.], 0., [true, false])?;
    let flipped = pixels(&mut gpu, d, key)?;
    assert!(pixel(&flipped, 16, 32)[2] > 240);
    assert!(pixel(&flipped, 48, 32)[0] > 240);
    d.transform = Transform::new(
        [32., 32.],
        [48., 24.],
        std::f64::consts::FRAC_PI_2,
        [false, false],
    )?;
    let rotated = pixels(&mut gpu, d, key)?;
    assert!(pixel(&rotated, 32, 16)[0] > 240);
    assert!(pixel(&rotated, 32, 48)[2] > 240);
    assert!(pixel(&rotated, 8, 32)[0] < 100);
    d.transform = Transform::new([32., 32.], [64., 64.], 0., [false, false])?;
    d.crop = Crop::new(0., 0., 0.5, 1.)?;
    d.opacity = Opacity::new(0.5)?;
    let alpha = pixels(&mut gpu, d, key)?;
    let p = pixel(&alpha, 32, 32);
    assert!(p[0] > 170 && p[0] < 210 && p[1] < 80 && p[2] < 80);
    d.opacity = Opacity::new(0.)?;
    let transparent = pixels(&mut gpu, d, key)?;
    assert!(pixel(&transparent, 32, 32)[0] < 100);
    let changed = ProductKey {
        asset: key.asset,
        revision: 2,
    };
    gpu.begin_frame()?;
    assert!(gpu.upload_product(
        changed,
        &Decoded {
            width: 1,
            height: 1,
            rgba: vec![0, 255, 0, 255]
        }
    ));
    assert!(gpu.contains_product(key) && gpu.contains_product(changed));
    d.opacity = Opacity::OPAQUE;
    let green = pixels(&mut gpu, d, changed)?;
    assert!(pixel(&green, 32, 32)[1] > 240);
    Ok(())
}

#[test]
#[ignore = "explicit native GPU overlay readback"]
fn canvas_overlay_is_on_top_without_asset_or_document_changes() -> Result<(), AssetError> {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::from_env_or_default());
    let mut gpu = pollster::block_on(Gpu::new(
        &instance,
        None,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        1024 * 1024,
    ))?;
    let d = data();
    let key = ProductKey {
        asset: d.asset_id,
        revision: 1,
    };
    gpu.begin_frame()?;
    gpu.upload_product(
        key,
        &Decoded {
            width: 1,
            height: 1,
            rgba: vec![255, 0, 0, 255],
        },
    );
    let bytes = pixels_overlay(
        &mut gpu,
        d,
        key,
        &[OverlayQuad {
            points: [[20., 20.], [20., 40.], [40., 20.], [40., 40.]],
            color: [0., 1., 0., 1.],
        }],
    )?;
    assert_eq!(pixel(&bytes, 30, 30), [0, 255, 0, 255]);
    assert_eq!(pixel(&bytes, 10, 10), [255, 0, 0, 255]);
    assert!(gpu.contains_product(key));
    assert!(
        pixels_overlay(
            &mut gpu,
            d,
            key,
            &vec![
                OverlayQuad {
                    points: [[0.; 2]; 4],
                    color: [1.; 4]
                };
                tack_render::MAX_OVERLAY_QUADS + 1
            ]
        )
        .is_err()
    );
    Ok(())
}
