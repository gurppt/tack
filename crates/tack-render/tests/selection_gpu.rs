use tack_assets::AssetError;
use tack_core::{Camera, Transform};
use tack_render::{AnnotationDraws, Gpu, SelectionRect};

fn capture(gpu: &mut Gpu, rects: &[SelectionRect], scale: f64) -> Result<Vec<u8>, AssetError> {
    let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("selection readback"),
        size: wgpu::Extent3d {
            width: 320,
            height: 320,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: gpu.format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let mut camera = Camera::new([320, 320]);
    camera.set_view([160., 160.], 1.)?;
    camera.set_ui_scale(scale);
    gpu.begin_frame()?;
    gpu.render_annotated_selected(
        &target.create_view(&Default::default()),
        &camera,
        &[],
        &[],
        None,
        AnnotationDraws {
            primitives: &[],
            order: &[],
        },
        rects,
    )?;
    let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("selection pixels"),
        size: 320 * 1280,
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
                bytes_per_row: Some(1280),
                rows_per_image: Some(320),
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
        timeout: Some(std::time::Duration::from_secs(5)),
    })?;
    rx.recv_timeout(std::time::Duration::from_secs(5))??;
    let bytes = buffer.slice(..).get_mapped_range().to_vec();
    Ok(bytes)
}
#[test]
#[ignore = "explicit native GPU selection batch, rotation, fractional DPI and bounded memory readback"]
fn every_selected_rectangle_is_crisp_above_ui_overlay_budget_and_releases_storage()
-> Result<(), AssetError> {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::from_env_or_default());
    let mut gpu = pollster::block_on(Gpu::new(
        &instance,
        None,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        1024 * 1024,
    ))?;
    assert_eq!(gpu.selection_bytes(), (0, 0));
    // One border in each disjoint 10x10 screen cell. 1,024 members formerly
    // exceeded the 2,048-quad UI budget when using four outline strips per image.
    let rects: Vec<_> = (0..1024)
        .map(|i| {
            Ok(SelectionRect {
                transform: Transform::new(
                    [(i % 32) as f64 * 10. + 5., (i / 32) as f64 * 10. + 5.],
                    [6., 6.],
                    if i % 2 == 0 { 0.3 } else { 0. },
                    [false; 2],
                )?,
                color: [1., 0., 0., 1.],
                width: 1.,
            })
        })
        .collect::<Result<_, tack_core::GeometryError>>()?;
    for scale in [1., 1.25, 1.5, 2.] {
        let pixels = capture(&mut gpu, &rects, scale)?;
        for i in 0..1024 {
            let x = (i % 32) * 10;
            let y = (i / 32) * 10;
            assert!(
                (y..y + 10).any(|y| (x..x + 10).any(|x| {
                    let p = (y * 320 + x) * 4;
                    pixels[p] > 250 && pixels[p + 1] < 4 && pixels[p + 2] < 4
                })),
                "missing selected member {i}, DPI {scale}"
            );
        }
        assert!(
            pixels.chunks_exact(4).all(|p| p[0] < 100 || p[0] > 250),
            "selection has no smoothed edge samples"
        );
        let (cpu, gpu_bytes) = gpu.selection_bytes();
        assert!(cpu > 0 && cpu <= 1024 * 44);
        assert!(gpu_bytes > 0 && gpu_bytes <= 1024 * 44);
    }
    let pixels = capture(&mut gpu, &[], 1.)?;
    assert_eq!(gpu.selection_bytes(), (0, 0));
    assert!(pixels.chunks_exact(4).all(|p| p[0] < 100));
    Ok(())
}
