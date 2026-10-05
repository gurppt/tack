//! Startup/argument ownership, outside the native render/event path.
use super::*;
pub fn run(args: Vec<OsString>, started: Instant) -> Result<(), AssetError> {
    run_mode(args, started, false)
}
pub fn run_new(args: Vec<OsString>, started: Instant) -> Result<(), AssetError> {
    run_mode(args, started, true)
}
fn run_mode(args: Vec<OsString>, started: Instant, new: bool) -> Result<(), AssetError> {
    let mut local = LocalState::new()?;
    let untitled = new && args.is_empty();
    let path = if untitled {
        local.root.join("untitled-slot-0.tack")
    } else {
        PathBuf::from(
            args.first()
                .ok_or("open FILE.tack [--seconds N] [--output REPORT]")?,
        )
    };
    local.untitled = untitled;
    let mut options = OpenOptions {
        path,
        new,
        untitled,
        seconds: None,
        output: None,
        tour: false,
        interaction: None,
        annotation_benchmark: false,
        potato: false,
        supply_stress: false,
        immediate: false,
        dense: false,
        window_size: [1280, 720],
    };
    let mut it = args.into_iter().skip(1);
    while let Some(a) = it.next() {
        match a.to_str() {
            Some("--window-size") => {
                let size = it.next().ok_or("window size WIDTHxHEIGHT")?;
                options.window_size = parse_window_size(size.to_str().ok_or("window size text")?)?;
            }
            Some("--seconds") => {
                let value = it.next().ok_or("duration")?;
                let seconds: f64 = value.to_str().ok_or("duration text")?.parse()?;
                if !(1.0..=120.).contains(&seconds) {
                    return Err("duration 1..120 seconds".into());
                }
                options.seconds = Some(seconds);
            }
            Some("--output") => {
                options.output = Some(PathBuf::from(it.next().ok_or("output path")?))
            }
            Some("--dense-view") => options.dense = true,
            Some("--potato") => options.potato = true,
            Some("--supply-stress") => options.supply_stress = true,
            Some("--present-immediate") => options.immediate = true,
            Some("--board-tour") => options.tour = true,
            Some("--annotation-benchmark") => options.annotation_benchmark = true,
            Some("--interaction") => {
                options.interaction = Some(
                    it.next()
                        .ok_or("interaction scenario")?
                        .to_str()
                        .ok_or("scenario text")?
                        .to_owned(),
                )
            }
            _ => return Err("unknown open option".into()),
        }
    }
    if options.interaction.is_some()
        && (options.tour || options.seconds.is_none() || options.output.is_none())
    {
        return Err(
            "interaction benchmark requires --seconds and --output, without --board-tour".into(),
        );
    }
    if options.annotation_benchmark && (options.seconds.is_none() || options.output.is_none()) {
        return Err("annotation benchmark requires --seconds and --output".into());
    }
    let benchmark = options
        .interaction
        .clone()
        .map(ImageBenchmark::new)
        .transpose()?;
    crate::report_output::preflight(options.output.as_deref(), Some(&options.path))?;
    let events = EventLoop::<Event>::with_user_event().build()?;
    let proxy = events.create_proxy();
    let read_proxy = proxy.clone();
    let input = options.path.clone();
    let work = std::env::temp_dir().join(format!(
        "tack-product-open-{:032x}",
        tack_storage::new_document_id()?.value()
    ));
    let read_work = work.clone();
    let new_board = options.new;
    let untitled = options.untitled;
    let root = local.root.clone();
    std::thread::Builder::new()
        .name("tack-document-read".into())
        .spawn(move || {
            let start = Instant::now();
            let result = (|| -> Result<LoadedBoard, AssetError> {
                tack_storage::create_private_directory(&read_work, true)?;
                if untitled {
                    tack_storage::create_private_directory(&root, true)?;
                }
                if new_board && !untitled && std::fs::symlink_metadata(&input).is_ok() {
                    return Err("new board filename already exists".into());
                }
                let lease = if untitled {
                    let mut chosen = None;
                    for slot in 1..=16 {
                        let candidate = root.join(format!("untitled-slot-{slot}.tack"));
                        // Reclaim only an owned, verified empty crash seed with no
                        // recovery directory. Never infer discard from an empty base.
                        if let Ok(metadata) = std::fs::symlink_metadata(&candidate) {
                            // Reclamation owns this private leaf, never a symlink's
                            // external target (ordinary Open deliberately allows aliases).
                            if !metadata.is_file() || metadata.file_type().is_symlink() {
                                continue;
                            }
                            if let Ok(owner) = tack_storage::BoardLease::acquire(&candidate)
                                && !owner.recovery_directory().exists()
                                && let Ok(seed) = owner.open()
                                && seed.document.objects().next().is_none()
                                && seed.document.sources().next().is_none()
                                && owner.retire_empty_seed(seed.document.id()).is_ok()
                            {
                                chosen = Some(owner);
                                break;
                            }
                            continue;
                        }
                        if let Ok(owner) = tack_storage::BoardLease::acquire_new(&candidate)
                            && !owner.recovery_directory().exists()
                        {
                            chosen = Some(owner);
                            break;
                        }
                    }
                    Arc::new(chosen.ok_or(
                        "16 retained Untitled boards; open Recent boards to recover/save them",
                    )?)
                } else {
                    Arc::new(if new_board {
                        tack_storage::BoardLease::acquire_new(&input)?
                    } else {
                        tack_storage::BoardLease::acquire(&input)?
                    })
                };
                if new_board {
                    lease.save(
                        &tack_core::Document::new(
                            tack_storage::new_document_id()?,
                            tack_core::DocumentLimits::default(),
                        ),
                        Vec::new(),
                    )?;
                }
                let board = lease.open()?;
                let (recovery, warning) = match lease.recovery(board.document.id()) {
                    Ok(candidate) => (candidate.is_some(), None),
                    Err(e) => (
                        lease.owns_recovery(board.document.id()).unwrap_or(false),
                        Some(format!(
                            "Recovery invalid; normal file intact. Discard explicitly: {e}"
                        )),
                    ),
                };
                Ok(LoadedBoard {
                    board,
                    path: lease.path().to_owned(),
                    lease,
                    metadata_ms: start.elapsed().as_secs_f64() * 1000.,
                    recovery,
                    warning,
                })
            })();
            let _ = read_proxy.send_event(Event::Loaded(Box::new(result)));
        })?;
    let window_size = options.window_size;
    let mut app = App {
        options,
        local: Box::new(local),
        proxy,
        source_active: false,
        annotations: None,
        started,
        window: None,
        gpu: None,
        surface: None,
        config: None,
        board: None,
        assets: None,
        camera: Camera::new(window_size),
        input: ImageInput::new()?,
        editor: None,
        load_failed: false,
        save: ImageSave::default(),
        benchmark,
        draws: Vec::with_capacity(10000),
        error: None,
        dirty: true,
        drawable: true,
        occluded: false,
        next_frame: Instant::now(),
        metadata_ms: 0.,
        native_startup_ms: 0.,
        first_content_ms: None,
        first_frame_ms: None,
        gpu_setup_ms: 0.,
        useful_ms: None,
        frames: Vec::new(),
        extent: [0.; 2],
        work: work.clone(),
        camera_clamped: false,
        navigation_end_pending: 0,
        drain_ms: 0.,
        title: String::new(),
        interaction_error: None,
        redraws: 0,
        wakeups: 0,
        event_samples: Vec::new(),
        supply_pending: false,
        visibility: Default::default(),
        context: None,
        pointer: [0.; 2],
        cursor_icon: winit::window::CursorIcon::Default,
    };
    events.run_app(&mut app)?;
    app.local.worker.cancel();
    if let Some(editor) = &mut app.editor {
        app.save.finish(editor);
    }
    app.drain()?;
    app.report()?;
    let error = app
        .error
        .take()
        .or_else(|| app.save.last_error.take().map(AssetError::from));
    if app.editor.as_ref().is_some_and(|e| e.is_dirty()) && !app.local.close_after_discard {
        eprintln!(
            "Tack closed with unsaved edits. Only edits included in a completed recovery snapshot can be restored."
        );
    } else if !app.local.recovery_pending
        && let Some((lease, id)) = app.local.seed.take()
        && let Err(error) = lease.retire_empty_seed(id)
    {
        eprintln!("Untitled seed retained: {error}");
    }
    drop(app);
    let _ = std::fs::remove_dir_all(work);
    if let Some(error) = error {
        return Err(error);
    }
    Ok(())
}

// Developer measurements start at the requested physical size; OS resizing is
// still authoritative. This adds no preferences or ordinary event-path work.
fn parse_window_size(text: &str) -> Result<[u32; 2], AssetError> {
    let (width, height) = text.split_once('x').ok_or("window size WIDTHxHEIGHT")?;
    let size = [width.parse()?, height.parse()?];
    if !(320..=8192).contains(&size[0]) || !(240..=8192).contains(&size[1]) {
        return Err("window size 320x240..8192x8192".into());
    }
    Ok(size)
}
#[cfg(test)]
mod size_tests {
    use super::parse_window_size;
    #[test]
    fn diagnostic_window_size_is_bounded() {
        assert_eq!(parse_window_size("800x600").ok(), Some([800, 600]));
        for value in ["", "0x600", "800x0", "8193x600", "800x600x2", "-800x600"] {
            assert!(parse_window_size(value).is_err());
        }
    }
}
