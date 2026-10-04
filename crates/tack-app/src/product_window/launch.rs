//! Startup/argument ownership, outside the native render/event path.
use super::*;
pub fn run(args: Vec<OsString>, started: Instant) -> Result<(), AssetError> {
    let path = PathBuf::from(
        args.first()
            .ok_or("open FILE.tack [--seconds N] [--output REPORT] [--board-tour]")?,
    );
    let mut options = OpenOptions {
        path,
        seconds: None,
        output: None,
        tour: false,
        interaction: None,
        annotation_benchmark: false,
    };
    let mut it = args.into_iter().skip(1);
    while let Some(a) = it.next() {
        match a.to_str() {
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
    std::thread::Builder::new()
        .name("tack-document-read".into())
        .spawn(move || {
            let start = Instant::now();
            let result = TackFile::open(input)
                .map(|b| (b, start.elapsed().as_secs_f64() * 1000.))
                .map_err(|e| Box::new(e) as AssetError);
            let _ = read_proxy.send_event(Event::Loaded(Box::new(result)));
        })?;
    let work = std::env::temp_dir().join(format!(
        "tack-product-open-{:032x}",
        tack_storage::new_document_id()?.value()
    ));
    let mut app = App {
        options,
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
        camera: Camera::new([1280, 720]),
        input: ImageInput::new()?,
        editor: None,
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
    };
    events.run_app(&mut app)?;
    if let Some(editor) = &mut app.editor {
        app.save.finish(editor);
    }
    app.drain()?;
    app.report()?;
    let error = app
        .error
        .take()
        .or_else(|| app.save.last_error.take().map(AssetError::from));
    if app.editor.as_ref().is_some_and(|e| e.is_dirty()) {
        eprintln!("Tack closed with unsaved edits; Ctrl+S saves the committed document.");
    }
    drop(app);
    let _ = std::fs::remove_dir_all(work);
    if let Some(error) = error {
        return Err(error);
    }
    Ok(())
}
