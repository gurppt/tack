//! Reproducible developer evidence, using the shipping bitmap overlay/GPU path.
//! This is offscreen rendering, not a native-window or human usability test.
use std::{error::Error, io::Write, path::Path, time::Instant};
use tack_app::{
    actions::Action,
    context_menu::{self, Command as MenuCommand, Context, ContextMenu, Group},
    image_gizmo::ImageGizmo,
    image_input::product_keymap,
    image_interaction::SelectionState,
    local_ui::{LocalUi, Panel},
    preferences::Preferences,
};
use tack_assets::Decoded;
use tack_core::*;
use tack_render::{DrawProductImage, Gpu, ProductKey};
type R<T = ()> = Result<T, Box<dyn Error + Send + Sync>>;
fn fixture() -> R<DocumentEditor> {
    let mut doc = Document::new(DocumentId::new(1)?, DocumentLimits::default());
    doc.apply(Command::AddSource(Source::linked(
        SourceId::new(1)?,
        "generated.png",
    )?))?;
    doc.apply(Command::AddAsset(ImageAsset::new(
        AssetId::new(1)?,
        SourceId::new(1)?,
        [2, 2],
    )?))?;
    for i in 1..=5 {
        let t = Transform::new([-210. + i as f64 * 65., 0.], [100., 80.], 0., [false; 2])?;
        let o = match i {
            1 | 2 => DocumentObject::image(ObjectId::new(i)?, AssetId::new(1)?, t),
            3 => DocumentObject::annotation(
                ObjectId::new(i)?,
                Annotation::new(
                    AnnotationKind::Text(TextObject::new("Note".into(), 16., TextAlignment::Left)?),
                    AnnotationStyle::default(),
                ),
                t,
            )?,
            4 => DocumentObject::frame(ObjectId::new(i)?, "Frame".into(), t)?,
            _ => DocumentObject::annotation(
                ObjectId::new(i)?,
                Annotation::new(AnnotationKind::Rect, AnnotationStyle::default()),
                t,
            )?,
        };
        doc.apply(Command::AddObject {
            object: o,
            index: doc.object_order().len(),
        })?;
    }
    Ok(DocumentEditor::new(doc, 200))
}
fn render(
    gpu: &mut Gpu,
    camera: &Camera,
    editor: &DocumentEditor,
    gizmo: &ImageGizmo,
    key: ProductKey,
    path: &Path,
) -> R {
    let [width, height] = camera.screen_size();
    let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("context evidence"),
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
    let images: Vec<_> = [1, 2]
        .into_iter()
        .filter_map(|id| {
            editor
                .document()
                .object_render_data(ObjectId::new(id).ok()?)
        })
        .map(|data| DrawProductImage {
            data,
            key: Some(key),
        })
        .collect();
    gpu.begin_frame()?;
    let mut scene = tack_app::annotation_scene::AnnotationScene::default();
    scene.build(
        editor.document(),
        &tack_app::image_interaction::ImageInteraction::default(),
        &tack_app::annotation_tool::AnnotationInput::default(),
        camera,
        &images,
    );
    gpu.render_annotated(
        &target.create_view(&Default::default()),
        camera,
        &images,
        &gizmo.quads,
        None,
        tack_render::AnnotationDraws {
            primitives: &scene.primitives,
            order: &scene.order,
        },
    )?;
    let stride = (width * 4).div_ceil(256) * 256;
    let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: u64::from(stride) * u64::from(height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut enc = gpu.device.create_command_encoder(&Default::default());
    enc.copy_texture_to_buffer(
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
                bytes_per_row: Some(stride),
                rows_per_image: Some(height),
            },
        },
        target.size(),
    );
    gpu.queue.submit([enc.finish()]);
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    buffer.slice(..).map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx.send(r);
    });
    gpu.device.poll(wgpu::PollType::Wait {
        submission_index: None,
        timeout: Some(std::time::Duration::from_secs(10)),
    })?;
    rx.recv_timeout(std::time::Duration::from_secs(10))??;
    let bytes = buffer.slice(..).get_mapped_range();
    let mut output = std::io::BufWriter::new(std::fs::File::create(path)?);
    write!(output, "P6\n{width} {height}\n255\n")?;
    for row in bytes.chunks(stride as usize) {
        for pixel in row[..width as usize * 4].chunks_exact(4) {
            output.write_all(&pixel[..3])?;
        }
    }
    output.flush()?;
    Ok(())
}
fn main() -> R {
    let dir = std::env::args().nth(1).ok_or("output directory required")?;
    let dir = Path::new(&dir);
    std::fs::create_dir_all(dir)?;
    let editor = fixture()?;
    let map = product_keymap()?;
    let catalog: Vec<_> = Action::ALL.into_iter().map(|a| serde_json::json!({"id": a.id(), "label": a.label(), "shortcut": context_menu::shortcut(&map, a), "bindings": map.for_action(a).map(|b| format!("{:?} {:?} {:?}", b.control, b.modifiers, b.trigger)).collect::<Vec<_>>() })).collect();
    std::fs::write(
        dir.join("actions.json"),
        serde_json::to_vec_pretty(&catalog)?,
    )?;
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::from_env_or_default());
    let mut gpu = pollster::block_on(Gpu::new(
        &instance,
        None,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        16 * 1024 * 1024,
    ))?;
    gpu.configure_diagnostics(false);
    let key = ProductKey {
        asset: Some(AssetId::new(1)?),
        source: SourceId::new(1)?,
        lod: Lod::Thumbnail,
        edge: 128,
        revision: 1,
    };
    gpu.begin_frame()?;
    gpu.upload_product(
        key,
        &Decoded {
            width: 2,
            height: 2,
            rgba: vec![
                190, 100, 45, 255, 90, 125, 155, 255, 70, 145, 125, 255, 165, 105, 140, 255,
            ],
        },
    );
    let mut rows = Vec::new();
    for screen in [[800, 600], [1024, 768]] {
        for scale in [1., 2., 4., 8.] {
            let mut camera = Camera::new(screen);
            camera.set_ui_scale(scale);
            for (name, ids) in [
                ("canvas", &[][..]),
                ("image", &[1][..]),
                ("multiple", &[1, 2][..]),
                ("note", &[3][..]),
                ("frame", &[4][..]),
                ("annotation", &[5][..]),
                ("application", &[][..]),
            ] {
                let mut selection = SelectionState::default();
                for id in ids {
                    selection.select(Some(ObjectId::new(*id)?), true);
                }
                let context = Context::selection(&editor, &selection, false, false);
                let start = Instant::now();
                let mut menu = if name == "application" {
                    ContextMenu::application(context, &camera, &map)
                } else {
                    ContextMenu::new(
                        context,
                        [screen[0] as f64 - 2., screen[1] as f64 - 2.],
                        &camera,
                        &map,
                    )
                };
                let construction_us = start.elapsed().as_secs_f64() * 1e6;
                if name == "image" || name == "application" {
                    let group = if name == "image" {
                        Group::Sampling
                    } else {
                        Group::File
                    };
                    let index = menu
                        .root_items()
                        .iter()
                        .position(|i| i.command == MenuCommand::Submenu(group))
                        .ok_or("submenu absent")?;
                    for _ in 0..menu.root_items()[..=index]
                        .iter()
                        .filter(|i| i.command != MenuCommand::Heading)
                        .count()
                    {
                        menu.key(winit::keyboard::KeyCode::ArrowDown, &map, &camera);
                    }
                    menu.key(winit::keyboard::KeyCode::ArrowRight, &map, &camera);
                }
                let mut gizmo = ImageGizmo::default();
                let start = Instant::now();
                menu.draw(&mut gizmo, &camera);
                let draw_us = start.elapsed().as_secs_f64() * 1e6;
                let file = format!("{name}-{}x{}-{}x.ppm", screen[0], screen[1], scale as u32);
                render(&mut gpu, &camera, &editor, &gizmo, key, &dir.join(&file))?;
                rows.push(serde_json::json!({"file":file, "kind":name,"screen":screen,"scale":scale,"owned_menu_bytes":menu.allocated_bytes(),"quads":gizmo.quads.len(),"overlay_capacity_bytes":gizmo.quads.capacity()*std::mem::size_of::<tack_render::OverlayQuad>(),"construction_us":construction_us,"draw_build_us":draw_us,"rectangles":menu.rectangles().into_iter().map(|r|[r.x,r.y,r.width,r.height]).collect::<Vec<_>>() }));
            }
            for panel in [
                Panel::Preferences,
                Panel::Keymap,
                Panel::Close,
                Panel::Recovery,
                Panel::Error,
                Panel::Recent,
            ] {
                let mut ui = LocalUi::new(panel);
                let mut gizmo = ImageGizmo::default();
                ui.draw(&mut gizmo, &camera, &map, &Preferences::defaults()?);
                let file = format!(
                    "panel-{panel:?}-{}x{}-{}x.ppm",
                    screen[0], screen[1], scale as u32
                );
                render(&mut gpu, &camera, &editor, &gizmo, key, &dir.join(&file))?;
            }
        }
    }
    let mut gizmo = ImageGizmo::default();
    let start = Instant::now();
    for _ in 0..10000 {
        gizmo.quads.clear();
        std::hint::black_box(&gizmo.quads);
    }
    let closed_redraw_mean_us = start.elapsed().as_secs_f64() * 1e6 / 10000.;
    std::fs::write(
        dir.join("evidence.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"scope":"software Vulkan offscreen; no native event-loop, human or idle/RSS measurement", "adapter":gpu.adapter_info.name,"backend":format!("{:?}",gpu.adapter_info.backend), "closed_button_redraw_mean_us":closed_redraw_mean_us,"closed_button_quads":gizmo.quads.len(),"cases":rows}),
        )?,
    )?;
    Ok(())
}
