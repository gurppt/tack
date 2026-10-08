//! Opt-in real GPU cache churn/readback. No timers or diagnostics in production.
use tack_app::supply_plan::displayed_lod;
use tack_assets::{AssetError, Decoded};
use tack_core::{
    AssetId, Camera, Crop, ImageFiltering, ImageRenderData, Lod, ObjectId, Opacity, SourceId,
    Transform,
};
use tack_render::{DrawProductImage, Gpu, ProductKey};

fn key(source: u128, revision: u64, lod: Lod) -> Result<ProductKey, AssetError> {
    Ok(ProductKey {
        source: SourceId::new(source)?,
        revision,
        lod,
        edge: lod.edge(),
        asset: (lod == Lod::Thumbnail).then_some(AssetId::new(source)?),
    })
}
fn put(
    gpu: &mut Gpu,
    source: u128,
    revision: u64,
    lod: Lod,
    native: bool,
) -> Result<(), AssetError> {
    assert!(gpu.begin_frame()?);
    let size = if native { 8 } else { 4 };
    let mut rgba = Vec::new();
    for y in 0..size {
        for x in 0..size {
            let color = if native {
                if (x + y) % 2 == 0 { 0 } else { 255 }
            } else {
                128
            };
            rgba.extend_from_slice(&[color, color, color, 255]);
        }
    }
    assert!(gpu.upload_product(
        key(source, revision, lod)?,
        &Decoded {
            width: size,
            height: size,
            rgba
        }
    ));
    Ok(())
}
fn draw_data(filtering: ImageFiltering, transformed: bool) -> Result<ImageRenderData, AssetError> {
    Ok(ImageRenderData {
        object_id: ObjectId::new(1)?,
        asset_id: AssetId::new(1)?,
        transform: Transform::new(
            [32., 32.],
            [64., 64.],
            if transformed { 0.3 } else { 0. },
            [transformed, false],
        )?,
        crop: if transformed {
            Crop::new(0.125, 0.125, 0.75, 0.75)?
        } else {
            Crop::FULL
        },
        opacity: Opacity::OPAQUE,
        filtering,
    })
}
fn pixels(gpu: &mut Gpu, data: ImageRenderData, chosen: ProductKey) -> Result<Vec<u8>, AssetError> {
    let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("long LOD churn readback"),
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
    assert!(gpu.begin_frame()?);
    gpu.render_product(
        &target.create_view(&Default::default()),
        &camera,
        &[DrawProductImage {
            data,
            key: Some(chosen),
        }],
    )?;
    let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 16384,
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
    gpu.queue.submit([encoder.finish()]);
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
    gpu.device.poll(wgpu::PollType::Wait {
        submission_index: None,
        timeout: Some(std::time::Duration::from_secs(5)),
    })?;
    receiver.recv_timeout(std::time::Duration::from_secs(5))??;
    Ok(buffer.slice(..).get_mapped_range().to_vec())
}
#[test]
#[ignore = "requires a supported GPU: thousands of real uploads/evictions and pixel readback"]
fn long_churn_keeps_native_pixels_across_thresholds_eviction_reentry_and_revisions()
-> Result<(), AssetError> {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::from_env_or_default());
    let mut gpu = pollster::block_on(Gpu::new(
        &instance,
        None,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        2048,
    ))?;
    put(&mut gpu, 1, 1, Lod::Thumbnail, false)?;
    put(&mut gpu, 1, 1, Lod::Detail, true)?;
    let mut references = Vec::new();
    for filtering in [ImageFiltering::Nearest, ImageFiltering::Smooth] {
        for transformed in [false, true] {
            let data = draw_data(filtering, transformed)?;
            let native = pixels(&mut gpu, data, key(1, 1, Lod::Detail)?)?;
            let filtered = pixels(&mut gpu, data, key(1, 1, Lod::Thumbnail)?)?;
            assert_ne!(
                native, filtered,
                "Nearest/Smooth cannot restore filtered preview pixels"
            );
            references.push((data, native));
        }
    }
    let bands = [80., 127., 128., 129., 512., 513., 512., 129., 128., 80.];
    for turn in 0..1200 {
        // Far-pan pressure displaces every detail texture; Thumbnail partition
        // remains independently valid. No whole GPU reset hides LRU history.
        for source in 10..=14 {
            put(&mut gpu, source, 1, Lod::Detail, true)?;
        }
        assert!(!gpu.contains_product(key(1, 1, Lod::Detail)?));
        assert_eq!(
            displayed_lod(Lod::Detail, |lod| key(1, 1, lod)
                .is_ok_and(|k| gpu.contains_product(k))),
            Some(Lod::Thumbnail)
        );
        put(&mut gpu, 1, 1, Lod::Detail, true)?;
        // A late lower publication must not steal display from valid native.
        put(&mut gpu, 1, 1, Lod::Medium, false)?;
        for band in bands {
            let selected = displayed_lod(Lod::for_projected_edge(band), |lod| {
                key(1, 1, lod).is_ok_and(|k| gpu.contains_product(k))
            });
            assert_eq!(
                selected,
                Some(Lod::Detail),
                "history valley at turn {turn}, band {band}"
            );
        }
        assert_eq!(gpu.product_size(key(1, 1, Lod::Detail)?), Some([8, 8]));
        if turn % 24 == 0 {
            for (data, expected) in &references {
                assert_eq!(
                    pixels(&mut gpu, *data, key(1, 1, Lod::Detail)?)?,
                    *expected,
                    "pixels changed after churn {turn}"
                );
            }
        }
    }
    // Source/revision authority excludes old resident data even when source and
    // desired band are unchanged. Current data refines through its own keys.
    assert_eq!(
        displayed_lod(Lod::Detail, |lod| key(1, 2, lod)
            .is_ok_and(|k| gpu.contains_product(k))),
        None
    );
    put(&mut gpu, 1, 2, Lod::Thumbnail, false)?;
    assert_eq!(
        displayed_lod(Lod::Detail, |lod| key(1, 2, lod)
            .is_ok_and(|k| gpu.contains_product(k))),
        Some(Lod::Thumbnail)
    );
    put(&mut gpu, 1, 2, Lod::Detail, true)?;
    assert_eq!(
        displayed_lod(Lod::Thumbnail, |lod| key(1, 2, lod)
            .is_ok_and(|k| gpu.contains_product(k))),
        Some(Lod::Detail)
    );
    Ok(())
}
