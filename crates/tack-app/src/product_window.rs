//! Native product document composition. I/O/metadata loading and preview supply are workers.
use serde_json::json;
use std::{
    ffi::OsString,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use tack_app::{image_benchmark::ImageBenchmark, image_input::ImageInput, image_save::ImageSave};
use tack_assets::{AssetError, ProductAssets, ProductDemand, SourceState, SupplyLimits};
use tack_core::{Camera, DocumentEditor, DocumentQuery, Lod};
use tack_render::{DrawProductImage, Gpu, ProductKey};
use tack_storage::TackFile;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};
mod chrome;
mod hosting;
mod huge;
mod local;
mod shared;
use local::{LoadedBoard, LocalState};
struct OpenOptions {
    join_receipt: Option<PathBuf>,
    put_online: bool,
    path: PathBuf,
    new: bool,
    untitled: bool,
    seconds: Option<f64>,
    output: Option<PathBuf>,
    tour: bool,
    interaction: Option<String>,
    annotation_benchmark: bool,
    potato: bool,
    supply_stress: bool,
    lod_debug: bool,
    huge_tiles: bool,
    immediate: bool,
    dense: bool,
    window_size: [u32; 2],
}
enum Event {
    Loaded(Box<Result<LoadedBoard, AssetError>>),
    LocalReady,
    SourceDone(Result<(), String>),
    SharedReady,
}
struct App {
    options: OpenOptions,
    chrome: chrome::Chrome,
    host: Option<tack_app::hosting::Hosted>,
    offline: Option<tack_app::sharing::Descriptor>,
    shared_copy: Option<PathBuf>,
    share_remote: Option<String>,
    close_after_host: bool,
    local: Box<LocalState>,
    shared: Option<Box<shared::SharedState>>,
    proxy: winit::event_loop::EventLoopProxy<Event>,
    source_active: bool,
    annotations: Option<Box<tack_app::annotation_scene::AnnotationScene>>,
    started: Instant,
    window: Option<Arc<Window>>,
    gpu: Option<Gpu>,
    surface: Option<wgpu::Surface<'static>>,
    config: Option<wgpu::SurfaceConfiguration>,
    board: Option<Arc<TackFile>>,
    assets: Option<ProductAssets>,
    camera: Camera,
    input: ImageInput,
    editor: Option<DocumentEditor>,
    load_failed: bool,
    save: ImageSave,
    benchmark: Option<ImageBenchmark>,
    draws: Vec<DrawProductImage>,
    error: Option<AssetError>,
    dirty: bool,
    drawable: bool,
    occluded: bool,
    next_frame: Instant,
    metadata_ms: f64,
    native_startup_ms: f64,
    first_content_ms: Option<f64>,
    first_frame_ms: Option<f64>,
    gpu_setup_ms: f64,
    useful_ms: Option<f64>,
    frames: Vec<serde_json::Value>,
    extent: [f64; 2],
    work: PathBuf,
    camera_clamped: bool,
    navigation_end_pending: usize,
    drain_ms: f64,
    title: String,
    interaction_error: Option<String>,
    redraws: u64,
    wakeups: u64,
    event_samples: Vec<f64>,
    wheel_samples: Vec<f64>,
    supply_pending: bool,
    visibility: tack_app::visibility::Visibility,
    context: Option<Box<tack_app::context_menu::ContextMenu>>,
    pointer: [f64; 2],
    cursor_icon: tack_app::cursors::Kind,
    cursors: tack_app::cursors::Cache,
    native_modifiers: tack_app::input::Modifiers,
    lod_trace: Option<tack_app::lod_diagnostics::LodDiagnostics>,
}
impl App {
    fn initialize(&mut self, event_loop: &ActiveEventLoop) -> Result<(), AssetError> {
        let window = Arc::new(
            event_loop.create_window(
                Window::default_attributes()
                    .with_window_icon(tack_app::icon::window())
                    .with_title(if self.shared.is_some() {
                        "Tack — connecting shared board"
                    } else {
                        "Tack — loading local board"
                    })
                    .with_inner_size(winit::dpi::PhysicalSize::new(
                        self.options.window_size[0],
                        self.options.window_size[1],
                    )),
            )?,
        );
        self.cursors = tack_app::cursors::Cache::load(event_loop);
        self.cursors.set(&window, tack_app::cursors::Kind::Pointer);
        let gpu_started = Instant::now();
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::from_env_or_default());
        let surface = instance.create_surface(Arc::clone(&window))?;
        let mut gpu = pollster::block_on(Gpu::new(
            &instance,
            Some(&surface),
            wgpu::TextureFormat::Bgra8UnormSrgb,
            if self.options.potato {
                16 * 1024 * 1024
            } else {
                128 * 1024 * 1024
            },
        ))?;
        if let Some(atlas) = self.chrome.atlas.take() {
            gpu.set_ui_icon_atlas(&atlas)?;
        }
        gpu.configure_diagnostics(self.options.output.is_some());
        if self.options.potato {
            gpu.constrain_uploads(1024 * 1024, 2);
        }
        let size = window.inner_size();
        self.camera.resize([size.width.max(1), size.height.max(1)]);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: gpu.format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: if self.options.immediate {
                wgpu::PresentMode::AutoNoVsync
            } else {
                wgpu::PresentMode::Fifo
            },
            desired_maximum_frame_latency: 2,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
        };
        surface.configure(&gpu.device, &config);
        self.gpu_setup_ms = gpu_started.elapsed().as_secs_f64() * 1000.;
        self.input.gizmo.set_scale(window.scale_factor());
        self.camera.set_ui_scale(window.scale_factor());
        window.set_ime_allowed(true);
        self.window = Some(window);
        self.gpu = Some(gpu);
        self.surface = Some(surface);
        self.config = Some(config);
        self.native_startup_ms = self.started.elapsed().as_secs_f64() * 1000.;
        if self.shared.is_some() {
            self.apply_preferences()?;
        }
        Ok(())
    }
    fn redraw(&mut self) -> Result<(), AssetError> {
        if !self.drawable || self.occluded {
            return Ok(());
        }
        self.refresh_chrome();
        self.redraws += 1;
        let start = Instant::now();
        let (Some(gpu), Some(surface), Some(config)) = (&mut self.gpu, &self.surface, &self.config)
        else {
            return Ok(());
        };
        if !gpu.begin_frame()? {
            self.dirty = true;
            return Ok(());
        }
        let acquire_start = Instant::now();
        let frame = match surface.get_current_texture() {
            Ok(f) => f,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                surface.configure(&gpu.device, config);
                self.dirty = true;
                return Ok(());
            }
            Err(wgpu::SurfaceError::Timeout | wgpu::SurfaceError::Other) => {
                self.dirty = true;
                return Ok(());
            }
            Err(e) => return Err(e.into()),
        };
        let acquire_ms = acquire_start.elapsed().as_secs_f64() * 1000.;
        if self.started.elapsed().as_secs_f64() > 1.0
            && let (Some(script), Some(editor)) = (&mut self.benchmark, &mut self.editor)
        {
            script.drive(&mut self.input, editor, &mut self.camera)?;
        }
        let cpu_start = Instant::now();
        self.draws.clear();
        let mut recognizable = 0;
        let mut near = Vec::new();
        if let Some(editor) = &self.editor {
            if self.options.tour {
                let fraction = (self.started.elapsed().as_secs_f64() / 12.).min(1.);
                self.camera.set_view(
                    [
                        self.extent[0] / 2.,
                        4500. + fraction * (self.extent[1] - 9000.).max(0.),
                    ],
                    0.006,
                )?;
            }
            if self.options.supply_stress {
                let t = (self.started.elapsed().as_secs_f64() - 1.).max(0.);
                let phase = (t / 2.) as usize;
                let first = editor
                    .document()
                    .object_order()
                    .first()
                    .and_then(|id| editor.document().object(*id));
                let last = editor
                    .document()
                    .object_order()
                    .last()
                    .and_then(|id| editor.document().object(*id));
                if let Some(o) = if phase.is_multiple_of(2) { first } else { last } {
                    let zoom = if phase % 4 < 2 { 0.8 } else { 3. };
                    let mut center = o.transform().center();
                    center[0] += (t * 7.).sin() * 80.;
                    self.camera.set_view(center, zoom)?;
                }
            }
            let document = editor.document();
            let viewport = self.camera.viewport();
            let vicinity = tack_core::WorldRect::new(
                viewport.x - viewport.width * 0.25,
                viewport.y - viewport.height * 0.25,
                viewport.width * 1.5,
                viewport.height * 1.5,
            )?;
            self.visibility.refresh(document, editor.generation());
            let selected = !self.input.images.selection.is_empty();
            let cached = self.visibility.cached();
            let candidates = self
                .visibility
                .candidates(vicinity, |id| {
                    selected && self.input.images.selection.contains(id)
                })
                .chain(document.object_order().iter().copied().take(if cached {
                    0
                } else {
                    usize::MAX
                }));
            for mut data in candidates
                .filter_map(|id| document.object_render_data(id))
                .map(|d| self.input.images.preview(d))
            {
                if data.filtering == tack_core::ImageFiltering::Default {
                    data.filtering = if self.local.profile.sampling == "Nearest" {
                        tack_core::ImageFiltering::Nearest
                    } else {
                        tack_core::ImageFiltering::Smooth
                    };
                }
                let bounds = data.transform.bounds();
                if bounds.intersects(viewport) {
                    self.draws.push(DrawProductImage { data, key: None });
                    if self.draws.len() > 10000 {
                        return Err("viewport exceeds current 10000-object renderer limit".into());
                    }
                } else if near.len() < 16 && bounds.intersects(vicinity) {
                    near.push(data);
                }
            }
        }
        let query_ms = cpu_start.elapsed().as_secs_f64() * 1000.;
        let logical_images = self.draws.len();
        if let (Some(shared), Some(editor)) = (&mut self.shared, &self.editor) {
            shared.request_visible(editor, &self.draws);
        }
        let mut tile_counts = [0usize; 2];
        let supply_start = Instant::now();
        self.supply_pending = false;
        let mut detailed = 0;
        let mut overview_edge = 128;
        let mut detail_reserved = 0;
        let mut desired_count = 0;
        let mut resolved_count = 0;
        if let (Some(editor), Some(assets)) = (&self.editor, &mut self.assets) {
            let document = editor.document();
            let needs: Vec<_> = self
                .draws
                .iter()
                .filter_map(|draw| {
                    let a = document.asset(draw.data.asset_id)?;
                    let s = document.source(a.source_id())?;
                    let uv = draw.data.crop.uv_rect();
                    Some(tack_app::supply_plan::Need {
                        asset: a.id(),
                        source: s.id(),
                        projected: if assets.supports_jpeg_tiles(s.id(), s.revision())
                            || (self.options.huge_tiles
                                && assets.supports_tiles(s.id(), s.revision()))
                        {
                            512. * uv[2].min(uv[3])
                        } else {
                            tack_app::supply_plan::projected_edge(
                                draw.data.transform,
                                self.camera.zoom(),
                            )
                        },
                        source_edge: a.pixel_size().into_iter().max().unwrap_or(0),
                        source_size: [Lod::Medium, Lod::Detail].into_iter().fold(
                            a.pixel_size(),
                            |size, lod| {
                                let key = ProductKey {
                                    asset: None,
                                    source: s.id(),
                                    revision: s.revision(),
                                    lod,
                                    edge: lod.edge(),
                                };
                                assets
                                    .detail_pixel_size(s.id(), s.revision(), lod)
                                    .into_iter()
                                    .chain(gpu.product_size(key))
                                    .fold(size, |size, resident| {
                                        std::array::from_fn(|axis| size[axis].max(resident[axis]))
                                    })
                            },
                        ),
                        filtering: draw.data.filtering,
                        crop: [uv[2], uv[3]],
                    })
                })
                .collect();
            let plan = tack_app::supply_plan::plan(
                &needs,
                if self.options.potato {
                    16 * 1024 * 1024
                } else {
                    128 * 1024 * 1024
                },
                assets.limits().max_lod,
            );
            overview_edge = plan.overview_edge;
            detail_reserved = plan.detail_reserved;
            let mut demands = Vec::with_capacity(self.draws.len() * 2 + near.len());
            for draw in &self.draws {
                let s = document
                    .asset(draw.data.asset_id)
                    .and_then(|a| document.source(a.source_id()))
                    .ok_or("render source reference")?;
                assets.observe_current(draw.data.asset_id, s.revision(), s.id());
                let desired = plan.lods[&s.id()];
                let key = |lod| ProductKey {
                    asset: (lod == Lod::Thumbnail).then_some(draw.data.asset_id),
                    source: s.id(),
                    revision: s.revision(),
                    lod,
                    edge: if lod == Lod::Thumbnail {
                        plan.overview_edge
                    } else {
                        lod.edge()
                    },
                };
                demands.push(ProductDemand {
                    asset: draw.data.asset_id,
                    lod: Lod::Thumbnail,
                    edge: plan.overview_edge,
                    priority: tack_app::supply_plan::preview_priority(
                        draw.data.transform,
                        self.camera.zoom(),
                    ),
                    resident: gpu.contains_product(key(Lod::Thumbnail)),
                });
                if desired != Lod::Thumbnail {
                    demands.push(ProductDemand {
                        asset: draw.data.asset_id,
                        lod: desired,
                        edge: desired.edge(),
                        priority: 2,
                        resident: gpu.contains_product(key(desired)),
                    });
                }
            }
            let mut tiles = Some({
                huge::Tiles::plan(
                    &self.draws,
                    &self.camera,
                    document,
                    assets,
                    gpu,
                    huge::TilePolicy {
                        reserved: plan.detail_reserved,
                        png: self.options.huge_tiles,
                    },
                    &mut demands,
                )
            });
            for data in &near {
                demands.push(ProductDemand {
                    asset: data.asset_id,
                    lod: Lod::Thumbnail,
                    edge: plan.overview_edge,
                    priority: 3,
                    resident: false,
                });
            }
            // Background work is never proactively requested at rest (class 4).
            demands.sort_by_key(|d| d.priority);
            // The plan fits each partition. Make all admitted residents newer than
            // obsolete textures before any insertion can evict them.
            for demand in &demands {
                if demand.priority == 3 {
                    continue;
                }
                if let Some(source) = document
                    .asset(demand.asset)
                    .and_then(|a| document.source(a.source_id()))
                {
                    gpu.touch_product(ProductKey {
                        asset: (demand.lod == Lod::Thumbnail).then_some(demand.asset),
                        source: source.id(),
                        revision: source.revision(),
                        lod: demand.lod,
                        edge: demand.edge,
                    });
                }
            }
            assets.replace_view(&demands, document, &self.local.originals);
            assets.poll();
            assets.schedule();
            for draw in &mut self.draws {
                let s = document
                    .asset(draw.data.asset_id)
                    .and_then(|a| document.source(a.source_id()))
                    .ok_or("render source reference")?;
                let desired = plan.lods[&s.id()];
                let key = |lod| ProductKey {
                    asset: (lod == Lod::Thumbnail).then_some(draw.data.asset_id),
                    source: s.id(),
                    revision: s.revision(),
                    lod,
                    edge: if lod == Lod::Thumbnail {
                        plan.overview_edge
                    } else {
                        lod.edge()
                    },
                };
                for lod in [Lod::Thumbnail, desired] {
                    let k = key(lod);
                    if !gpu.contains_product(k)
                        && let Some(image) =
                            assets.get_rep(draw.data.asset_id, s.revision(), lod, k.edge)
                    {
                        gpu.upload_product(k, image);
                    }
                }
            }
            if let Some(tiles) = &tiles {
                tiles.upload(assets, gpu);
            }
            // Select and account only after all uploads/evictions. Earlier draws
            // must not advertise quality that later insertions have removed.
            for draw in &mut self.draws {
                let s = document
                    .asset(draw.data.asset_id)
                    .and_then(|a| document.source(a.source_id()))
                    .ok_or("render source reference")?;
                let desired = plan.lods[&s.id()];
                let key = |lod| ProductKey {
                    asset: (lod == Lod::Thumbnail).then_some(draw.data.asset_id),
                    source: s.id(),
                    revision: s.revision(),
                    lod,
                    edge: if lod == Lod::Thumbnail {
                        plan.overview_edge
                    } else {
                        lod.edge()
                    },
                };
                draw.key = tack_app::supply_plan::displayed_lod(desired, |lod| {
                    gpu.contains_product(key(lod))
                })
                .map(key);
                recognizable += usize::from(draw.key.is_some());
                detailed += usize::from(draw.key.is_some_and(|k| k.lod != Lod::Thumbnail));
                desired_count += usize::from(desired != Lod::Thumbnail);
                resolved_count += usize::from(draw.key.is_some_and(|k| k.lod >= desired));
                self.supply_pending |= !gpu.contains_product(key(desired))
                    && !assets.failed_rep(draw.data.asset_id, desired, key(desired).edge);
            }
            if let Some(tiles) = &mut tiles {
                self.supply_pending |= tiles.display(&mut self.draws, assets, gpu);
                tile_counts = [tiles.requested, tiles.ready];
            }
        }
        let supply_ms = supply_start.elapsed().as_secs_f64() * 1000.;
        if let Some(editor) = &self.editor {
            self.input.build_overlay(editor, &self.camera);
            if let Some(shared) = &self.shared {
                shared
                    .leases
                    .overlay(&mut self.input.gizmo, editor.document(), &self.camera);
            }
        }
        let active_tool = self.input.active_tool();
        chrome::draw(
            &self.chrome,
            &mut self.input.gizmo,
            &self.camera,
            &self.local.profile,
            active_tool,
            gpu,
            (
                self.local.ui.is_some(),
                self.shared.as_ref().map(|s| s.ui_connection()).or_else(|| {
                    self.offline
                        .as_ref()
                        .map(|_| tack_app::capabilities::UiConnection::Offline)
                }),
                self.input.images.selection.annotations_locked,
            ),
        )?;
        // Reserve the bounded popup budget only while a popup is open.
        if self.context.is_some() {
            self.input
                .gizmo
                .quads
                .truncate(tack_render::MAX_OVERLAY_QUADS - 1024);
        }
        gpu.set_ui_icon_overlay_split(self.context.as_ref().map(|_| self.input.gizmo.quads.len()));
        if let Some(menu) = &mut self.context {
            menu.theme = self.local.profile.theme;
            menu.draw(&mut self.input.gizmo, &self.camera);
        }
        if let Some(ui) = &mut self.local.ui {
            self.input
                .gizmo
                .quads
                .truncate(tack_render::MAX_OVERLAY_QUADS - 1024);
            ui.draw(
                &mut self.input.gizmo,
                &self.camera,
                &self.input.keymap,
                &self.local.profile,
            );
            if ui.panel == tack_app::local_ui::Panel::Toolbar {
                gpu.set_ui_icons(ui.icons())?;
            }
            if ui.about_image
                && let Some(layout) = ui.about_layout
            {
                gpu.set_ui_image_rect(layout.physical_image());
                let mut logo_rect = layout.physical_logo();
                logo_rect[2] = f64::from(ui.about_logo_size[0]) * layout.scale;
                logo_rect[3] = f64::from(ui.about_logo_size[1]) * layout.scale;
                gpu.set_ui_logo_rect(logo_rect);
            }
        }
        let palette = self.local.profile.theme.palette();
        gpu.set_background(tack_render::Background {
            fill: palette.background,
            grid: palette.grid,
        });
        let grid = self.input.grid_visible.then(|| tack_render::GridView {
            spacing: tack_app::spatial_snap::grid_spacing(
                self.camera.zoom(),
                self.input.gizmo.scale,
            ),
            dpi: self.input.gizmo.scale,
        });
        let annotated = self
            .editor
            .as_ref()
            .is_some_and(|e| e.document().annotation_count() > 0)
            || self.input.annotation.creation.is_some()
            || !self.input.annotation.scribble.is_empty()
            || self.input.annotation.eraser.is_some()
            || self.input.annotation.edit.is_some();
        let render_start = Instant::now();
        let scene_ms =
            render_start.duration_since(cpu_start).as_secs_f64() * 1000. - query_ms - supply_ms;
        if annotated {
            let scene = self.annotations.get_or_insert_with(Box::default);
            scene.palette = palette;
            scene.caret_visible = self.input.caret.visible;
            if let Some(editor) = &self.editor {
                scene.build(
                    editor.document(),
                    &self.input.images,
                    &self.input.annotation,
                    &self.camera,
                    &self.draws,
                );
            }
            gpu.render_annotated_selected(
                &frame.texture.create_view(&Default::default()),
                &self.camera,
                &self.draws,
                &self.input.gizmo.quads,
                grid,
                tack_render::AnnotationDraws {
                    primitives: &scene.primitives,
                    order: &scene.order,
                },
                &self.input.gizmo.selection,
            )?;
        } else {
            if let Some(scene) = &mut self.annotations {
                scene.primitives.clear();
                scene.order.clear();
                scene.glyphs = 0;
                scene.omitted = 0;
                scene.layout_ms = 0.;
                scene.build_ms = 0.;
            }
            gpu.render_spatial_selected(
                &frame.texture.create_view(&Default::default()),
                &self.camera,
                &self.draws,
                &self.input.gizmo.quads,
                grid,
                &self.input.gizmo.selection,
            )?;
        }
        let cpu_ms = cpu_start.elapsed().as_secs_f64() * 1000.;
        let elapsed_ms = self.started.elapsed().as_secs_f64() * 1000.;
        self.first_frame_ms.get_or_insert(elapsed_ms);
        if recognizable > 0 && self.first_content_ms.is_none() {
            self.first_content_ms = Some(elapsed_ms);
        }
        if !self.draws.is_empty()
            && recognizable * 100 >= logical_images * 80
            && self.useful_ms.is_none()
        {
            self.useful_ms = Some(elapsed_ms);
        }
        let stats = gpu.stats();
        let present_start = Instant::now();
        if let Some(w) = &self.window {
            w.pre_present_notify();
        }
        frame.present();
        let present_ms = present_start.elapsed().as_secs_f64() * 1000.;
        if self.options.output.is_some() && self.frames.len() < 7200 {
            self.frames.push(json!({"window_size":self.camera.screen_size(),"lod_trace":self.lod_trace.as_mut().and_then(|trace| trace.record(&self.camera, &self.draws, self.editor.as_ref().map(|e|e.document()), self.assets.as_ref(), gpu, overview_edge)),"annotations":self.annotations.as_ref().map(|a|json!({"primitives":a.primitives.len(),"glyphs":a.glyphs,"omitted":a.omitted,"layout_ms":a.layout_ms,"build_ms":a.build_ms})),"camera":self.camera.screen_to_world(self.camera.screen_size().map(|v|f64::from(v)/2.)),"zoom":self.camera.zoom(),"overview_edge":overview_edge,"detail_reserved":detail_reserved,"desired_detail":desired_count,"quality_resolved":resolved_count,"scene_ms":scene_ms,"query_ms":query_ms,"supply_ms":supply_ms,"detailed":detailed,"supply":self.assets.as_ref().map(|a| {let s=a.stats();json!({"pending":s.pending,"queued":s.queued,"cpu_bytes":s.cpu_bytes,"evictions":s.evictions,"source_bytes":s.source_bytes,"container_bytes":s.container_bytes,"decode_count":s.decode_count,"codec_requests":s.codec_requests,"region_jobs":s.region_jobs,"tile_outputs":s.tile_outputs,"tile_cache_hits":s.tile_cache_hits,"tile_cache_read_bytes":s.tile_cache_read_bytes,"tile_cache_write_bytes":s.tile_cache_write_bytes,"tile_disk_bytes":s.tile_disk_bytes,"decoded_bytes":s.decoded_bytes,"decode_ms":s.decode_ms,"discarded":s.discarded})}),"encode_ms":stats.encode_ms,"submit_ms":stats.submit_ms,"poll_ms":stats.poll_ms,"elapsed_ms":elapsed_ms,"cpu_ms":cpu_ms,"visible":logical_images,"tiles_requested":tile_counts[0],"tiles_ready":tile_counts[1],"recognizable":recognizable,"upload_cpu_ms":stats.upload_cpu_ms,"upload_bytes":stats.upload_bytes,"uploads":stats.uploads,"gpu_bytes":stats.gpu_bytes,"in_flight":stats.in_flight,"acquire_ms":acquire_ms,"present_ms":present_ms,"callback_ms":start.elapsed().as_secs_f64()*1000.}));
        }
        self.dirty = self.supply_pending;
        Ok(())
    }
    fn start_source(&mut self, action: tack_app::actions::Action) {
        use tack_app::{
            actions::Action,
            source_actions::{SourceOperation, SourceRequest},
        };
        let result = (|| -> Result<(), AssetError> {
            if self.source_active {
                return Err("a source action is already running".into());
            }
            let editor = self.editor.as_ref().ok_or("no open document")?;
            if self.input.images.selection.len() != 1 {
                return Err("source actions require one selected image".into());
            }
            let id = self
                .input
                .images
                .selection
                .ids()
                .next()
                .ok_or("no selected image")?;
            let operation = match action {
                Action::OpenSource => SourceOperation::Open,
                Action::RevealSource => SourceOperation::Reveal,
                Action::CopySourcePath => SourceOperation::CopyPath,
                _ => return Err("invalid source action".into()),
            };
            let request = SourceRequest::new(editor.document(), id, &self.options.path, operation)?;
            let proxy = self.proxy.clone();
            std::thread::Builder::new()
                .name("tack-source-action".into())
                .spawn(move || {
                    let result = request.perform().map_err(|e| e.to_string());
                    let _ = proxy.send_event(Event::SourceDone(result));
                })?;
            self.source_active = true;
            Ok(())
        })();
        self.interaction_error = result.err().map(|e| e.to_string());
    }
    fn drain(&mut self) -> Result<(), AssetError> {
        self.navigation_end_pending = self.assets.as_ref().map(|a| a.stats().pending).unwrap_or(0);
        let start = Instant::now();
        if let Some(a) = &mut self.assets {
            a.suspend();
        }
        loop {
            if let Some(a) = &mut self.assets {
                a.poll();
            }
            if let Some(g) = &mut self.gpu {
                g.begin_frame()?;
            }
            let pending = self.assets.as_ref().is_some_and(|a| a.stats().pending > 0);
            let gpu_pending = self
                .gpu
                .as_ref()
                .is_some_and(|g| g.stats().in_flight > 0 || g.has_pending_timings());
            if !pending && !gpu_pending {
                break;
            }
            if start.elapsed() > Duration::from_secs(30) {
                return Err("product shutdown drain timed out".into());
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        self.drain_ms = start.elapsed().as_secs_f64() * 1000.;
        Ok(())
    }
    fn report(&mut self) -> Result<(), AssetError> {
        let Some(path) = &self.options.output else {
            return Ok(());
        };
        let gpu = self.gpu.as_ref();
        let stats = self.assets.as_ref().map(|a| a.stats()).unwrap_or_default();
        let counts = |state| {
            self.assets
                .as_ref()
                .map(|a| a.states.values().filter(|s| **s == state).count())
                .unwrap_or(0)
        };
        let samples: Vec<_> = gpu
            .and_then(|g| g.timing_samples())
            .map(|s| s.iter().map(|s| json!({"pass_ms":s.pass_ms})).collect())
            .unwrap_or_default();
        let mut report = json!({"about":{"requests":self.local.about_ticket,"image_gpu_bytes":gpu.map_or(0, |g| g.ui_image_bytes()),"version":tack_app::about::METADATA.version},"window_size":self.camera.screen_size(),"load_failed":self.load_failed,"local":{"operations":self.local.worker.operations,"recoveries":self.save.recoveries,"recovery_generation":self.save.recovery_generation,"recovery_bytes":self.save.recovery_bytes,"last_storage_worker_ms":self.save.last_worker_ms,"ui_scale":self.camera.ui_scale(),"discarded_asset_results":stats.discarded},"first_frame_ms":self.first_frame_ms,"gpu_setup_ms":self.gpu_setup_ms,"annotation_resources":gpu.map(|g|g.annotation_bytes()),"annotations":self.editor.as_ref().map(|e|e.document().annotation_count()).unwrap_or(0),"spatial":{"grid":self.input.grid_visible,"snapping":self.input.snap.enabled,"frames":self.editor.as_ref().map(|e|e.document().objects().filter(|o|matches!(o.kind(),tack_core::ObjectKind::Frame(_))).count()).unwrap_or(0),"groups":self.editor.as_ref().map(|e|e.document().groups().count()).unwrap_or(0)},"redraw_count":self.redraws,"wait_count":self.wakeups,"operation":"open","editing":self.editor.as_ref().map(|e|json!({"dirty":e.is_dirty(),"generation":e.generation(),"undo_entries":e.undo_len(),"redo_entries":e.redo_len(),"selected":self.input.images.selection.len(),"save_completed":self.save.completed})),"interaction":self.benchmark.as_ref().map(|b|json!({"scenario":b.name(),"input_ms":b.samples,"snap_query_ms":b.snap_queries,"commits":b.commits,"cancels":b.cancels,"invariants":b.invariants})),"native_startup_ms":self.native_startup_ms,"camera_clamped":self.camera_clamped,"navigation_end_pending":self.navigation_end_pending,"drain_ms":self.drain_ms,"chrome":{"toolbar_cells":self.chrome.toolbar.buttons[..self.chrome.toolbar.count].iter().map(|b|json!({"rect":b.rect,"action":b.action.map(|a|a.id())})).collect::<Vec<_>>(),"toolbar_bounds":self.chrome.toolbar.bounds,"selection_outline_count":self.input.gizmo.selection.len(),"caret_deadline_active":self.input.caret.deadline().is_some(),"toolbar_entries":self.chrome.toolbar.count,"placement":format!("{:?}",self.local.profile.toolbar.placement),"config_bytes":serde_json::to_vec(&self.local.profile.toolbar)?.len(),"icon_atlas_gpu_bytes":gpu.map(|g|g.ui_icon_bytes()).unwrap_or(0),"status_bar":self.local.profile.status_bar},"state_counts_scope":"requested sources only","recognizable_clock":"CPU submission before present","metadata_load_ms":self.metadata_ms,"metadata_bytes":self.board.as_ref().map(|b|b.metadata_bytes_read).unwrap_or(0),"first_recognizable_ms":self.first_content_ms,"ordinary_view_80_percent_ms":self.useful_ms,"source_bytes_before_detail":stats.source_bytes,"container_bytes":stats.container_bytes+self.board.as_ref().map(|b|b.metadata_bytes_read).unwrap_or(0),"overview_reused":stats.reused,"overview_regenerated":stats.regenerated,"errors":stats.errors,"source_missing":counts(SourceState::Missing),"source_changed":counts(SourceState::Changed),"source_foreign":counts(SourceState::Foreign),"source_unavailable":counts(SourceState::Unavailable),"peak_pending":stats.peak_pending,"cpu_payload_bytes":stats.cpu_bytes,"adapter":gpu.map(|g|g.adapter_info.name.clone()),"backend":gpu.map(|g|format!("{:?}",g.adapter_info.backend)),"frames":self.frames,"gpu_samples":samples,"detail":"bounded projected 128/512/2048 display supply"});
        let supply_receipt = json!({"potato":self.options.potato,"present_mode":self.config.as_ref().map(|c|format!("{:?}",c.present_mode)),"event_samples_ms":self.event_samples,"wheel_samples_ms":self.wheel_samples,"codec_requests":stats.codec_requests,"region_jobs":stats.region_jobs,"tile_outputs":stats.tile_outputs,"tile_cache_hits":stats.tile_cache_hits,"tile_cache_read_bytes":stats.tile_cache_read_bytes,"tile_cache_write_bytes":stats.tile_cache_write_bytes,"tile_disk_bytes":stats.tile_disk_bytes,"decoded_bytes":stats.decoded_bytes,"cpu_payload_peak":stats.cpu_peak,"cpu_evictions":stats.evictions,"reprioritized":stats.reprioritized,"discarded":stats.discarded,"peak_queued":stats.peak_queued,"visibility_memo_bytes":self.visibility.bytes()});
        if let Some(shared) = &self.shared {
            let canonical = self
                .editor
                .as_ref()
                .map(|editor| tack_storage::encode_metadata(editor.document()))
                .transpose()?;
            report["shared"] = json!({"address":shared.address,"board":shared.board,
                "state":format!("{:?}",shared.state),"revision":shared.revision,
                "accepted":shared.accepted,"refused":shared.refused,
                "traffic_probes":shared.probes,
                "canonical_sha256":canonical.map(|(_,_,bytes)|tack_shared::ContentHash::digest(&bytes)),
                "object_count":self.editor.as_ref().map(|editor|editor.document().objects().count()),
                "source_count":self.editor.as_ref().map(|editor|editor.document().sources().count())});
        }
        if let Some(obj) = report.as_object_mut()
            && let Some(extra) = supply_receipt.as_object()
        {
            obj.extend(extra.iter().map(|(k, v)| (k.clone(), v.clone())));
        }
        crate::report_output::write_new(path, &serde_json::to_vec_pretty(&report)?)?;
        Ok(())
    }
}
impl ApplicationHandler<Event> for App {
    fn resumed(&mut self, e: &ActiveEventLoop) {
        if self.window.is_none()
            && let Err(error) = self.initialize(e)
        {
            self.error = Some(error);
            e.exit();
        }
    }
    fn user_event(&mut self, e: &ActiveEventLoop, event: Event) {
        match event {
            Event::Loaded(result) => {
                let result = (*result).and_then(|loaded| self.install_loaded(loaded, false));
                if let Err(error) = result {
                    if self.options.seconds.is_some() {
                        self.error = Some(error);
                        e.exit();
                    } else {
                        self.fail_load();
                        self.editor = tack_storage::new_document_id().ok().map(|id| {
                            DocumentEditor::new(
                                tack_core::Document::new(id, tack_core::DocumentLimits::default()),
                                200,
                            )
                        });
                        self.local_error(error.to_string());
                    }
                }
            }
            Event::LocalReady => self.poll_local(),
            Event::SharedReady => self.poll_shared(),
            Event::SourceDone(result) => {
                self.source_active = false;
                self.interaction_error = result.err();
                self.dirty = true;
            }
        }
    }
    fn window_event(&mut self, e: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let started = self.options.output.is_some().then(Instant::now);
        let wheel =
            self.options.output.is_some() && matches!(&event, WindowEvent::MouseWheel { .. });
        self.sync_leases();
        self.handle_window_event(e, id, event);
        self.sync_leases();
        if let Some(t) = started
            && self.event_samples.len() < 7200
        {
            let ms = t.elapsed().as_secs_f64() * 1000.;
            self.event_samples.push(ms);
            if wheel && self.wheel_samples.len() < 7200 {
                self.wheel_samples.push(ms);
            }
        }
    }

    fn about_to_wait(&mut self, e: &ActiveEventLoop) {
        self.wakeups += 1;
        self.poll_shared();
        self.sync_leases();
        self.flush_shared();
        if self.options.output.is_some()
            && let Some(shared) = &mut self.shared
        {
            shared.probe(self.started);
        }
        self.poll_local();
        if self
            .input
            .settle_link_feedback(Instant::now(), self.editor.as_ref())
        {
            self.dirty = true;
        }
        if self.local.feedback.settle(Instant::now()) {
            self.chrome.last_state = "";
            if let Some(ui) = &mut self.local.ui
                && ui.message == "COPIED"
            {
                ui.message.clear();
            }
            self.dirty = true;
        }
        if let Some(ui) = &mut self.local.ui
            && ui.feedback.settle(Instant::now())
        {
            self.dirty = true;
        }
        if let Some(ui) = &mut self.local.ui {
            let editing =
                self.input.edit_focused && self.drawable && !self.occluded && ui.text_editing();
            if ui.caret.update(editing, Instant::now()) {
                self.dirty = true;
            }
        }
        let editing = self.input.edit_focused
            && self.drawable
            && !self.occluded
            && self.local.ui.is_none()
            && self.context.is_none()
            && (self.input.name_edit.is_some() || self.input.annotation.edit.is_some());
        if self.input.caret.update(editing, Instant::now()) {
            self.dirty = true;
        }
        self.poll_storage();
        self.invalidate_context();
        if self.local.close_ready {
            if self.local.close_board {
                self.local.close_board = false;
                self.local.close_ready = false;
                self.input.cancel();
                self.local.close_after_save = false;
                self.local.close_after_discard = false;
                if let Err(error) = self.operation(tack_app::local_worker::Operation::EmptyBoard(
                    self.local.root.clone(),
                )) {
                    self.local_error(error.to_string());
                } else {
                    self.panel(tack_app::local_ui::Panel::Connecting);
                }
            } else {
                e.exit();
                return;
            }
        }
        if self
            .options
            .seconds
            .is_some_and(|s| self.started.elapsed().as_secs_f64() >= s)
        {
            e.exit();
            return;
        }
        if let Some(a) = &mut self.assets {
            if !self.drawable || self.occluded {
                a.suspend();
            } else {
                self.dirty |= a.stats().pending > 0;
            }
        }
        if self.editor.is_none()
            && let (Some(shared), Some(window)) = (&self.shared, &self.window)
        {
            let title = format!(
                "Tack · shared {} / {} · {:?} · F5 reconnect · {}",
                shared.address,
                shared.board,
                shared.state,
                self.interaction_error
                    .as_deref()
                    .unwrap_or("snapshot pending")
            );
            let title = if self.options.output.is_some()
                || std::env::var_os("TACK_NATIVE_DIAGNOSTICS").is_some()
            {
                title
            } else {
                format!(
                    "Tack · Shared board · {}",
                    match shared.state {
                        tack_shared::client::ConnectionState::Connecting
                        | tack_shared::client::ConnectionState::Reconnecting => "Connecting",
                        _ => "Offline",
                    }
                )
            };
            if self.title != title {
                window.set_title(&title);
                self.title = title;
            }
        }
        if let (Some(editor), Some(window)) = (&self.editor, &self.window) {
            let state = if self.shared.is_some() {
                "server authority"
            } else if self.save.active() {
                "saving"
            } else if self.save.last_error.is_some() {
                "save failed"
            } else if editor.is_dirty() {
                "modified"
            } else {
                "saved"
            };
            let mode = if self.input.images.crop_mode {
                "crop"
            } else {
                "transform"
            };
            let missing = self
                .assets
                .as_ref()
                .map(|a| {
                    a.states
                        .values()
                        .filter(|s| {
                            matches!(
                                s,
                                SourceState::Missing
                                    | SourceState::Foreign
                                    | SourceState::Unavailable
                            )
                        })
                        .count()
                })
                .unwrap_or(0);
            let changed = self
                .assets
                .as_ref()
                .map(|a| {
                    a.states
                        .values()
                        .filter(|s| **s == SourceState::Changed)
                        .count()
                })
                .unwrap_or(0);
            let frame_name = self
                .input
                .name_edit
                .as_ref()
                .map(|edit| edit.value.as_str())
                .or_else(|| {
                    self.input.images.selection.ids().find_map(|id| {
                        match editor.document().object(id)?.kind() {
                            tack_core::ObjectKind::Frame(n) => Some(n.as_str()),
                            _ => None,
                        }
                    })
                });
            let title = format!(
                "Tack — {} · {state} · {mode} · {} selected · {missing} missing / {changed} changed sources{}{}{}",
                if self.local.untitled {
                    "Untitled".into()
                } else {
                    self.options
                        .path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default()
                },
                self.input.images.selection.len(),
                frame_name.map(|n| format!(" · {n}")).unwrap_or_default(),
                if self.input.name_edit.is_some() {
                    " · Enter confirm / Esc cancel"
                } else {
                    ""
                },
                self.save
                    .last_error
                    .as_ref()
                    .or(self.interaction_error.as_ref())
                    .map(|s| format!(" · {s}"))
                    .unwrap_or_default()
            );
            let warning = if self.annotations.as_ref().is_some_and(|a| a.omitted > 0) {
                " · annotation display limit: some objects omitted"
            } else if self
                .input
                .annotation
                .creation
                .as_ref()
                .is_some_and(|c| c.capped)
            {
                " · stroke point limit reached"
            } else {
                ""
            };
            let note = if self.input.annotation.edit.is_some() {
                " · Ctrl+Enter confirm / Esc cancel / Ctrl+A replace"
            } else {
                ""
            };
            let title = format!(
                "{title} · {}{note}{warning}{}",
                self.input.annotation.tools.tool().label(),
                if self.source_active {
                    " · source action"
                } else {
                    ""
                }
            );
            let title = if self.local.import_status.is_empty() {
                title
            } else {
                format!("{title} · {}", self.local.import_status)
            };
            let title = if let Some(shared) = &self.shared {
                format!(
                    "{title} · shared {} / {} · {:?} · rev {} · F5 reconnect",
                    shared.address, shared.board, shared.state, shared.revision
                )
            } else {
                title
            };
            let title = if self.shared.is_some()
                && self.options.output.is_none()
                && std::env::var_os("TACK_NATIVE_DIAGNOSTICS").is_none()
            {
                format!(
                    "Tack · Shared board · {} · {} selected",
                    match self.shared.as_ref().map(|s| s.state) {
                        Some(tack_shared::client::ConnectionState::Connected) => "Online",
                        Some(
                            tack_shared::client::ConnectionState::Connecting
                            | tack_shared::client::ConnectionState::Reconnecting,
                        ) => "Connecting",
                        _ => "Offline",
                    },
                    self.input.images.selection.len()
                )
            } else {
                title
            };
            let title = if self.options.output.is_none()
                && std::env::var_os("TACK_NATIVE_DIAGNOSTICS").is_none()
            {
                let name = if self.local.untitled {
                    "Untitled".into()
                } else {
                    self.options
                        .path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default()
                };
                format!("Tack — {name}{}", if editor.is_dirty() { " *" } else { "" })
            } else {
                title
            };
            if self.title != title {
                window.set_title(&title);
                self.title = title;
            }
        }
        let pending = self.assets.as_ref().is_some_and(|a| a.stats().pending > 0);
        let completing = self
            .gpu
            .as_ref()
            .is_some_and(|g| g.stats().in_flight > 0 || g.has_pending_timings());
        if completing
            && let Some(g) = &mut self.gpu
            && let Err(error) = g.begin_frame()
        {
            self.error = Some(error);
            e.exit();
            return;
        }
        let active = (self.dirty
            || self.options.tour
            || self.options.supply_stress
            || self.benchmark.is_some()
            || self.options.annotation_benchmark)
            && self.drawable
            && !self.occluded;
        if active && self.drawable && !self.occluded && Instant::now() >= self.next_frame {
            if let Some(w) = &self.window {
                w.request_redraw();
            }
            self.next_frame = Instant::now() + Duration::from_millis(16);
        }
        let periodic =
            active || pending || completing || self.save.active() || self.options.seconds.is_some();
        let deadline = self
            .local
            .drop_deadline
            .into_iter()
            .chain(
                self.local
                    .recovery
                    .deadline()
                    .filter(|_| !self.local.recovery_pending),
            )
            .chain(
                self.shared
                    .as_ref()
                    .filter(|_| self.options.output.is_some())
                    .and_then(|shared| shared.probe_deadline(self.started)),
            )
            .chain(self.input.caret.deadline())
            .chain(self.input.link_feedback.deadline())
            .chain(self.local.feedback.deadline())
            .chain(self.local.ui.as_ref().and_then(|ui| ui.feedback.deadline()))
            .chain(self.local.ui.as_ref().and_then(|ui| ui.caret.deadline()))
            .chain(self.shared.as_ref().and_then(|s| s.leases.deadline()))
            .min();
        e.set_control_flow(if periodic {
            ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(16))
        } else if let Some(deadline) = deadline {
            ControlFlow::WaitUntil(deadline)
        } else {
            ControlFlow::Wait
        });
    }
}
mod launch;
pub use launch::{run, run_new, run_shared};

impl App {
    fn handle_window_event(&mut self, e: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match &event {
            WindowEvent::ModifiersChanged(modifiers) => {
                self.native_modifiers = modifiers.state().into();
            }
            WindowEvent::Focused(false) => self.native_modifiers = Default::default(),
            _ => {}
        }
        if matches!(&event, WindowEvent::KeyboardInput {event, ..} if event.state == winit::event::ElementState::Pressed)
            || matches!(
                &event,
                WindowEvent::MouseInput {
                    state: winit::event::ElementState::Pressed,
                    ..
                }
            )
        {
            self.input.status.clear();
            if !self.local.importing {
                self.local.import_status.clear();
            }
            if !self.load_failed {
                self.interaction_error = None;
            }
            if !self.save.active() {
                self.save.last_error = None;
            }
            self.chrome.last_state = "";
        }
        if let WindowEvent::Focused(focused) = &event {
            self.input.edit_focused = *focused;
        }
        if matches!(&event, WindowEvent::KeyboardInput { event, .. }
            if event.state == winit::event::ElementState::Pressed
            && event.physical_key == winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::F5))
            && let Some(shared) = &mut self.shared
        {
            if let Some(editor) = &mut self.editor {
                editor.set_shared_writable(false);
            }
            if let Err(error) = shared.client.reconnect() {
                self.interaction_error = Some(error.to_string());
            }
            self.dirty = true;
            return;
        }
        if matches!(event, WindowEvent::CloseRequested) {
            self.local.close_board = false;
            if self.host.is_some() {
                if let Err(error) = self.stop_hosting(true) {
                    self.local_error(error.to_string());
                }
                return;
            }
            if self.load_failed {
                e.exit();
                return;
            }
            if let Some(editor) = &mut self.editor
                && let Err(error) = self.input.commit_drafts(editor)
            {
                self.local_error(error.to_string());
                return;
            }
            self.input.cancel();
            self.local.worker.cancel();
            if self.editor.as_ref().is_some_and(|editor| editor.is_dirty())
                && self.options.seconds.is_none()
            {
                self.panel(tack_app::local_ui::Panel::Close);
            } else {
                e.exit();
            }
            return;
        }
        if matches!(&event, WindowEvent::KeyboardInput { event, .. }
            if event.state == winit::event::ElementState::Pressed
            && event.physical_key == winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::Escape))
            && self.local.ui.is_none()
            && self.context.is_none()
        {
            self.local.worker.cancel();
            self.local.queued = None;
        }
        if let WindowEvent::DroppedFile(path) = &event {
            if self.load_failed {
                return;
            }
            if self.local.recovery_pending {
                self.local_error("Choose recovery restore/discard before importing");
            } else if self.local.drops.len() < tack_app::local_import::MAX_IMPORT_FILES {
                self.local.drops.push(path.clone());
                self.local.drop_deadline = Some(Instant::now() + Duration::from_millis(50));
            } else {
                self.local_error("Drop exceeds 4096-image limit");
            }
            return;
        }
        if let WindowEvent::CursorMoved { position, .. } = &event {
            self.pointer = [position.x, position.y];
        }
        let result = if matches!(event, WindowEvent::RedrawRequested) {
            self.redraw()
        } else {
            let ui = self
                .menu_access_event(&event)
                .and_then(|used| {
                    if used {
                        Ok(true)
                    } else {
                        self.chrome_event(&event)
                    }
                })
                .and_then(|used| {
                    if used {
                        Ok(true)
                    } else {
                        self.context_event(&event)
                    }
                })
                .and_then(|used| {
                    if used {
                        Ok(true)
                    } else {
                        self.local_ui_event(&event)
                    }
                });
            let consumed = match ui {
                Ok(consumed) => consumed,
                Err(error) => {
                    self.local_error(error.to_string());
                    true
                }
            };
            if !consumed
                && !self.load_failed
                && !self.local.saving_as
                && !self.options.tour
                && self.benchmark.is_none()
                && let Some(editor) = &mut self.editor
            {
                if matches!(&event,WindowEvent::KeyboardInput {event,..} if event.state==winit::event::ElementState::Pressed)
                    || matches!(
                        event,
                        WindowEvent::MouseInput {
                            state: winit::event::ElementState::Pressed,
                            ..
                        }
                    )
                {
                    self.interaction_error = None;
                }
                match self.input.handle(&event, editor, &mut self.camera) {
                    Ok(requested) => {
                        if requested {
                            self.local.manual = true;
                        }
                    }
                    Err(error) => self.interaction_error = Some(error.to_string()),
                }
                self.dirty = true;
            }
            if self.local.manual && self.local.untitled {
                self.local.manual = false;
                if let Err(error) = self.local_action(tack_app::actions::Action::SaveAs) {
                    self.local_error(error.to_string());
                }
            }
            if self.local.manual && self.shared.is_some() {
                self.local.manual = false;
                self.interaction_error = Some("Shared edits are saved by the server".into());
            }
            if let Some(action) = self.input.pending_local.take()
                && let Err(error) = self.local_action(action)
            {
                self.local_error(error.to_string());
            }
            if let Some(action) = self.input.pending_source.take() {
                self.start_source(action);
            }
            match event {
                WindowEvent::Resized(size) => {
                    self.drawable = size.width > 0 && size.height > 0;
                    if self.drawable
                        && let (Some(c), Some(s), Some(g)) =
                            (&mut self.config, &self.surface, &self.gpu)
                    {
                        c.width = size.width.min(g.device.limits().max_texture_dimension_2d);
                        c.height = size.height.min(g.device.limits().max_texture_dimension_2d);
                        self.camera.resize([c.width, c.height]);
                        s.configure(&g.device, c);
                        self.dirty = true;
                    }
                    self.chrome_layout();
                }
                WindowEvent::Occluded(b) => self.occluded = b,
                WindowEvent::ScaleFactorChanged { .. } => {
                    if let Err(error) = self.apply_preferences() {
                        self.local_error(error.to_string());
                    }
                    // ImageInput already handles the native scale event and
                    // cancels gestures while preserving modal text drafts.
                }
                _ => {}
            }
            self.update_cursor();
            Ok(())
        };
        if let Err(error) = result {
            self.error = Some(error);
            e.exit();
        }
    }
}
mod menus;
