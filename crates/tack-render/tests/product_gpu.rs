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
    pixels_spatial(gpu, d, key, overlay, None)
}
fn pixels_spatial(
    gpu: &mut Gpu,
    d: ImageRenderData,
    key: ProductKey,
    overlay: &[OverlayQuad],
    grid: Option<tack_render::GridView>,
) -> Result<Vec<u8>, AssetError> {
    pixels_scene(gpu, d, key, overlay, grid, None)
}
fn pixels_scene(
    gpu: &mut Gpu,
    d: ImageRenderData,
    key: ProductKey,
    overlay: &[OverlayQuad],
    grid: Option<tack_render::GridView>,
    scene: Option<tack_render::AnnotationDraws<'_>>,
) -> Result<Vec<u8>, AssetError> {
    pixels_scene_scale(gpu, d, key, overlay, grid, scene, 1.)
}
#[allow(clippy::too_many_arguments)]
fn pixels_scene_scale(
    gpu: &mut Gpu,
    d: ImageRenderData,
    key: ProductKey,
    overlay: &[OverlayQuad],
    grid: Option<tack_render::GridView>,
    scene: Option<tack_render::AnnotationDraws<'_>>,
    scale: f64,
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
    camera.set_ui_scale(scale);
    gpu.begin_frame()?;
    let images = [DrawProductImage {
        data: d,
        key: Some(key),
    }];
    if let Some(scene) = scene {
        gpu.render_annotated(
            &target.create_view(&Default::default()),
            &camera,
            &images,
            overlay,
            grid,
            scene,
        )?;
    } else {
        gpu.render_spatial(
            &target.create_view(&Default::default()),
            &camera,
            &images,
            overlay,
            grid,
        )?;
    }
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

#[test]
fn grid_parameters_preserve_world_origin_zoom_and_fractional_dpi() -> Result<(), AssetError> {
    let mut c = Camera::new([100, 100]);
    c.set_view([1e8, -1e8], 0.01)?;
    let grid = tack_render::GridView {
        spacing: 4096.,
        dpi: 1.25,
    };
    let params = grid.parameters(&c);
    assert!(params[0] >= 0. && params[0] < params[2]);
    assert!(params[1] >= 0. && params[1] < params[2]);
    assert_eq!(params[3], 2.);
    c.pan([params[2] as f64, 0.])?;
    let next = grid.parameters(&c);
    assert!((next[0] - params[0]).abs() < 0.01);
    Ok(())
}

#[test]
#[ignore = "explicit native GPU grid and bitmap readback"]
fn procedural_grid_hidden_path_and_bitmap_labels_are_crisp() -> Result<(), AssetError> {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::from_env_or_default());
    let mut gpu = pollster::block_on(Gpu::new(
        &instance,
        None,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        1024 * 1024,
    ))?;
    let mut d = data();
    d.opacity = Opacity::new(0.)?;
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
    let hidden = pixels_spatial(&mut gpu, d, key, &[], None)?;
    let visible = pixels_spatial(
        &mut gpu,
        d,
        key,
        &[],
        Some(tack_render::GridView {
            spacing: 32.,
            dpi: 1.,
        }),
    )?;
    assert_ne!(pixel(&hidden, 0, 0), pixel(&visible, 0, 0));
    assert_eq!(pixel(&hidden, 1, 1), pixel(&visible, 1, 1));
    assert_eq!(pixel(&visible, 0, 0), pixel(&visible, 32, 32));
    let hidden_again = pixels_spatial(&mut gpu, d, key, &[], None)?;
    assert_eq!(hidden, hidden_again);
    let bitmap = OverlayQuad {
        points: [[16., 16.], [16., 32.], [32., 16.], [32., 32.]],
        color: [0., 1., 0., 1.],
        bitmap: Some([0x8000; 8]),
    };
    let glyph = pixels_spatial(&mut gpu, d, key, &[bitmap], None)?;
    assert_eq!(pixel(&glyph, 16, 16), [0, 255, 0, 255]);
    assert_eq!(pixel(&glyph, 17, 16), pixel(&hidden, 17, 16));
    assert_eq!(pixel(&glyph, 16, 17), pixel(&hidden, 16, 17));
    Ok(())
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
            bitmap: None,
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
                    bitmap: None,
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

#[test]
#[ignore = "explicit native annotation shader, pixel coverage and mixed-order pixel readback"]
fn annotations_preserve_order_and_bounded_bitmap_primitives() -> Result<(), AssetError> {
    use tack_render::{AnnotationDraws, AnnotationPrimitive, CanvasDraw};
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::from_env_or_default());
    let mut gpu = pollster::block_on(Gpu::new(
        &instance,
        None,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        1024 * 1024,
    ))?;
    assert_eq!(gpu.annotation_bytes(), (0, 0));
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
    let mut p = AnnotationPrimitive {
        points: [[10., 10.], [10., 54.], [54., 10.], [54., 54.]],
        size: [40., 40.],
        width: 2.,
        opacity: 1.,
        kind: 1,
        stroke: [0., 1., 0., 1.],
        fill: [0., 1., 0., 1.],
        mapping: [0.; 4],
        bitmap: [0; 8],
    };
    let on_top = pixels_scene(
        &mut gpu,
        d,
        key,
        &[],
        None,
        Some(AnnotationDraws {
            primitives: &[p],
            order: &[
                CanvasDraw::Image(0),
                CanvasDraw::Annotations { start: 0, end: 1 },
            ],
        }),
    )?;
    assert_eq!(pixel(&on_top, 32, 32), [0, 255, 0, 255]);
    assert_eq!(gpu.annotation_bytes().1, 0);
    let below = pixels_scene(
        &mut gpu,
        d,
        key,
        &[],
        None,
        Some(AnnotationDraws {
            primitives: &[p],
            order: &[
                CanvasDraw::Annotations { start: 0, end: 1 },
                CanvasDraw::Image(0),
            ],
        }),
    )?;
    assert_eq!(pixel(&below, 32, 32), [255, 0, 0, 255]);
    p.kind = 6;
    p.mapping = [0., 0., 1., 1.];
    p.bitmap = [u32::MAX; 8];
    p.fill = [0.; 4];
    let glyph = pixels_scene(
        &mut gpu,
        d,
        key,
        &[],
        None,
        Some(AnnotationDraws {
            primitives: &[p],
            order: &[
                CanvasDraw::Image(0),
                CanvasDraw::Annotations { start: 0, end: 1 },
            ],
        }),
    )?;
    assert_ne!(glyph, below);
    assert_eq!(gpu.annotation_bytes().1, 0);
    assert!(
        pixels_scene(
            &mut gpu,
            d,
            key,
            &[],
            None,
            Some(AnnotationDraws {
                primitives: &[p],
                order: &[CanvasDraw::Annotations { start: 0, end: 2 }]
            })
        )
        .is_err()
    );
    assert!(
        pixels_scene(
            &mut gpu,
            d,
            key,
            &[],
            None,
            Some(AnnotationDraws {
                primitives: &vec![p; tack_render::MAX_ANNOTATION_PRIMITIVES + 1],
                order: &[]
            })
        )
        .is_err()
    );
    p.kind = 1;
    p.width = 10.;
    p.opacity = 0.5;
    p.stroke = [0., 1., 0., 1.];
    p.fill = [0., 1., 0., 1.];
    let alpha = pixels_scene(
        &mut gpu,
        d,
        key,
        &[],
        None,
        Some(AnnotationDraws {
            primitives: &[p],
            order: &[
                CanvasDraw::Image(0),
                CanvasDraw::Annotations { start: 0, end: 1 },
            ],
        }),
    )?;
    assert_eq!(pixel(&alpha, 32, 32), pixel(&alpha, 13, 32));
    let segment = AnnotationPrimitive {
        points: [[4., 26.], [4., 38.], [38., 26.], [38., 38.]],
        size: [22., 0.],
        width: 10.,
        kind: 3,
        opacity: 0.5,
        stroke: [0., 1., 0., 0.5],
        fill: [0.; 4],
        mapping: [0.; 4],
        bitmap: [0; 8],
    };
    let mut joined = segment;
    joined.points = [[26., 26.], [26., 38.], [60., 26.], [60., 38.]];
    joined.mapping = [10., 32., 32., 32.];
    joined.bitmap[0] = 1;
    let stroke = pixels_scene(
        &mut gpu,
        d,
        key,
        &[],
        None,
        Some(AnnotationDraws {
            primitives: &[segment, joined],
            order: &[
                CanvasDraw::Image(0),
                CanvasDraw::Annotations { start: 0, end: 2 },
            ],
        }),
    )?;
    assert_eq!(pixel(&stroke, 20, 32), pixel(&stroke, 32, 32));
    // Two sRGB framebuffer blends can quantize by one code value at the stroke join.
    assert!(
        pixel(&stroke, 20, 32)
            .into_iter()
            .zip(pixel(&stroke, 36, 32))
            .all(|(a, b)| a.abs_diff(b) <= 1)
    );
    assert!(gpu.annotation_bytes().0 <= tack_render::MAX_ANNOTATION_PRIMITIVES * 128);
    Ok(())
}

#[test]
#[ignore = "explicit fractional-DPI bitmap and hard edge GPU readback"]
fn integer_logical_pixels_preserve_all_bitmap_rows_without_aa() -> Result<(), AssetError> {
    use tack_render::{AnnotationDraws, AnnotationPrimitive, CanvasDraw};
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
    for scale in [1_f64, 1.5, 2.5] {
        let integer: f64 = scale.round();
        let lo = 4. * integer;
        let hi = 20. * integer;
        let p = AnnotationPrimitive {
            points: [[lo, lo], [lo, hi], [hi, lo], [hi, hi]],
            kind: 6,
            size: [16. * integer; 2],
            width: 1.,
            opacity: 1.,
            stroke: [0., 1., 0., 1.],
            fill: [0.; 4],
            mapping: [0., 0., 1., 1.],
            bitmap: [0x0000ffff; 8],
        };
        let image = pixels_scene_scale(
            &mut gpu,
            d,
            key,
            &[],
            None,
            Some(AnnotationDraws {
                primitives: &[p],
                order: &[
                    CanvasDraw::Image(0),
                    CanvasDraw::Annotations { start: 0, end: 1 },
                ],
            }),
            scale,
        )?;
        for row in 0..16 {
            for dy in 0..integer as usize {
                assert_eq!(
                    pixel(
                        &image,
                        (lo + 2.) as usize,
                        lo as usize + row * integer as usize + dy
                    ),
                    if row % 2 == 0 {
                        [0, 255, 0, 255]
                    } else {
                        [255, 0, 0, 255]
                    }
                );
            }
        }
        assert!(
            image
                .chunks_exact(4)
                .all(|p| p == [0, 255, 0, 255] || p == [255, 0, 0, 255])
        );
        let overlay = OverlayQuad {
            points: p.points,
            color: p.stroke,
            bitmap: Some(p.bitmap),
        };
        let image = pixels_scene_scale(&mut gpu, d, key, &[overlay], None, None, scale)?;
        for row in 0..16 {
            assert_eq!(
                pixel(
                    &image,
                    (lo + 2.) as usize,
                    lo as usize + row * integer as usize
                ),
                if row % 2 == 0 {
                    [0, 255, 0, 255]
                } else {
                    [255, 0, 0, 255]
                }
            );
        }
        assert!(
            image
                .chunks_exact(4)
                .all(|p| p == [0, 255, 0, 255] || p == [255, 0, 0, 255])
        );
    }
    Ok(())
}
