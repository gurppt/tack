//! Minimal local vertical-slice CLI; no final import/manipulation UI.
use serde_json::json;
use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};
use tack_assets::{AssetError, BenchmarkBoard, ProductAssets, image_metadata, source_fingerprint};
use tack_core::*;
use tack_storage::{self as storage, BlobInput, Payload, TackFile};
struct Workspace(PathBuf);
impl Workspace {
    fn new() -> Result<Self, AssetError> {
        let p = std::env::temp_dir().join(format!(
            "tack-product-{:032x}",
            storage::new_document_id()?.value()
        ));
        storage::create_private_directory(&p, false)?;
        Ok(Self(p))
    }
}
impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn emit(path: Option<&Path>, report: &serde_json::Value) -> Result<(), AssetError> {
    let text = serde_json::to_string_pretty(report)?;
    if let Some(p) = path {
        crate::report_output::write_new(p, text.as_bytes())?;
    }
    println!("{text}");
    Ok(())
}
pub fn run(command: &str, args: Vec<OsString>, started: Instant) -> Result<(), AssetError> {
    match command {
        "create" => create(args, started),
        "repair" => repair(args, started),
        "inspect" => inspect(args),
        "open" => crate::product_window::run(args, started),
        "new" => crate::product_window::run_new(args, started),
        "query-scale" => query_scale(args),
        "annotation-scale" => crate::annotation_cli::run(args),
        "supply-scale" => crate::supply_cli::run(args),
        "spatial-scale" => crate::spatial_cli::run(args),
        _ => Err("unknown product command".into()),
    }
}
// Creating a new identity is not an edit of an understood existing document.
// Refuse files (including unsupported boards) and dangling symlinks without clobber.
fn require_new_target(path: &Path) -> storage::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(_) => Err(storage::StorageError::Invalid(
            "new document target already exists",
        )),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}

// Repair may replace only the understood input generation, never unrelated work.
fn require_repair_target(input: &Path, target: &Path) -> storage::Result<()> {
    match fs::symlink_metadata(target) {
        Ok(metadata) if metadata.is_file() && target.canonicalize()? == input.canonicalize()? => {
            Ok(())
        }
        Ok(_) => Err(storage::StorageError::Invalid(
            "repair output already exists and differs from input",
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}
fn create(args: Vec<OsString>, started: Instant) -> Result<(), AssetError> {
    let target = PathBuf::from(args.first().ok_or(
        "create OUTPUT.tack --linked|--embedded IMAGE... [--manifest PATH] [--output REPORT]",
    )?);
    require_new_target(&target)?;
    let mut embedded = false;
    let mut manifest = None;
    let mut output = None;
    let mut paths = Vec::new();
    let mut it = args.into_iter().skip(1);
    while let Some(arg) = it.next() {
        match arg.to_str() {
            Some("--embedded") => embedded = true,
            Some("--linked") => embedded = false,
            Some("--manifest") => manifest = Some(PathBuf::from(it.next().ok_or("manifest path")?)),
            Some("--output") => output = Some(PathBuf::from(it.next().ok_or("report path")?)),
            Some(s) if s.starts_with("--") => return Err("unknown create option".into()),
            _ => paths.push(PathBuf::from(arg)),
        }
    }
    crate::report_output::preflight(output.as_deref(), Some(&target))?;
    let mut layouts = Vec::new();
    if let Some(p) = manifest {
        if !paths.is_empty() {
            return Err("choose image arguments or benchmark manifest".into());
        }
        for o in BenchmarkBoard::read_manifest(&p)?.objects {
            paths.push(o.path);
            layouts.push(o.rect);
        }
    }
    if paths.is_empty() || paths.len() > 100_000 {
        return Err("import needs 1..100000 images".into());
    }
    let work = Workspace::new()?;
    let mut editor = DocumentEditor::new(
        Document::new(storage::new_document_id()?, DocumentLimits::default()),
        0,
    );
    let mut originals = Vec::new();
    let mut header_bytes = 0u64;
    let mut original_bytes = 0u64;
    for (index, path) in paths.iter().enumerate() {
        let path = path.canonicalize()?;
        let fingerprint = source_fingerprint(&path)?;
        let (pixels, read) = image_metadata(&path)?;
        header_bytes += read;
        let source = storage::new_source_id()?;
        let asset = storage::new_asset_id()?;
        let object = storage::new_object_id()?;
        let location = if embedded {
            original_bytes += fingerprint.size;
            originals.push(BlobInput::original(source, 1, Payload::File(path.clone())));
            SourceLocation::Embedded
        } else {
            SourceLocation::Linked(LinkedPath::native(&path)?)
        };
        editor.execute(Command::AddSource(Source::from_descriptor(
            source,
            location,
            1,
            Some(fingerprint),
        )?))?;
        editor.execute(Command::AddAsset(ImageAsset::new(asset, source, pixels)?))?;
        let rect = if let Some(r) = layouts.get(index) {
            *r
        } else {
            WorldRect::new(
                (index % 32) as f64 * 680.,
                (index / 32) as f64 * 530.,
                600.,
                600. * f64::from(pixels[1]) / f64::from(pixels[0]),
            )?
        };
        let transform = Transform::new(
            [rect.x + rect.width / 2., rect.y + rect.height / 2.],
            [rect.width, rect.height],
            0.,
            [false, false],
        )?;
        editor.execute(Command::AddObject {
            object: DocumentObject::image(object, asset, transform),
            index,
        })?;
    }
    let metadata_import_ms = started.elapsed().as_secs_f64() * 1000.;
    let seed = work.0.join("seed.tack");
    storage::save(&seed, editor.document(), originals)?;
    let board = Arc::new(TackFile::open(&seed)?);
    let (assets, progress, preparation_ms) =
        prepare_all(Arc::clone(&board), &seed, work.0.join("overview"))?;
    let inputs = save_inputs(&board, &assets);
    let save_started = Instant::now();
    let owner = storage::BoardLease::acquire_new(&target)?;
    owner.save(editor.document(), inputs)?;
    let save_ms = save_started.elapsed().as_secs_f64() * 1000.;
    editor.mark_saved();
    let stats = assets.stats();
    emit(
        output.as_deref(),
        &json!({"operation":"create","embedded":embedded,"objects":editor.document().object_order().len(),"metadata_import_ms":metadata_import_ms,"preparation_ms":preparation_ms,"progress":progress,"save_ms":save_ms,"total_ms":started.elapsed().as_secs_f64()*1000.,"file_bytes":fs::metadata(&target)?.len(),"metadata_source_bytes":header_bytes,"source_bytes_read":header_bytes+stats.source_bytes+original_bytes,"container_bytes_read":stats.container_bytes+board.metadata_bytes_read+original_bytes,"derived_bytes_written":stats.derived_bytes,"overview_reused":stats.reused,"overview_generated":stats.regenerated,"errors":stats.errors,"peak_pending":stats.peak_pending,"dirty_after_save":editor.is_dirty()}),
    )
}
fn prepare_all(
    board: Arc<TackFile>,
    path: &Path,
    dir: PathBuf,
) -> Result<(ProductAssets, Vec<serde_json::Value>, f64), AssetError> {
    let mut assets = ProductAssets::new(Arc::clone(&board), path, dir)?;
    let ids: Vec<_> = board.document.assets().map(|a| a.id()).collect();
    let start = Instant::now();
    let mut index = 0;
    let mut progress = Vec::new();
    let mut milestone = 0;
    loop {
        assets.poll();
        while index < ids.len() {
            if assets.request(ids[index])
                || assets.failed(ids[index])
                || assets.get(ids[index]).is_some()
            {
                index += 1;
            } else {
                break;
            }
        }
        let stats = assets.stats();
        let fraction = stats.completed * 100 / ids.len().max(1);
        if fraction >= milestone {
            progress.push(json!({"ready":stats.completed-stats.errors,"errors":stats.errors,"elapsed_ms":start.elapsed().as_secs_f64()*1000.}));
            milestone = match milestone {
                0 => 10,
                10 => 25,
                25 => 50,
                50 => 75,
                75 => 90,
                90 => 100,
                _ => 101,
            };
        }
        if index == ids.len() && stats.pending == 0 {
            break;
        }
        if start.elapsed() > Duration::from_secs(600) {
            return Err("overview preparation timed out".into());
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    let elapsed = start.elapsed().as_secs_f64() * 1000.;
    if assets.stats().errors > 0 {
        return Err("one or more overviews could not be prepared; target was not replaced".into());
    }
    Ok((assets, progress, elapsed))
}
fn save_inputs(board: &TackFile, assets: &ProductAssets) -> Vec<BlobInput> {
    let mut inputs = Vec::new();
    for (id, e) in &board.originals {
        inputs.push(BlobInput::original(*id, e.revision, board.payload(e.range)));
    }
    for a in board.document.assets() {
        if let Some(p) = assets.prepared.get(&a.id()) {
            inputs.push(BlobInput::overview(
                p.asset,
                p.revision,
                p.size,
                p.generator,
                Payload::File(p.path.clone()),
            ));
        } else if let Some(e) = board.overviews.get(&a.id()) {
            inputs.push(BlobInput::overview(
                a.id(),
                e.revision,
                [e.width, e.height],
                e.generator,
                board.payload(e.range),
            ));
        }
    }
    inputs
}
fn repair(args: Vec<OsString>, started: Instant) -> Result<(), AssetError> {
    let input = PathBuf::from(
        args.first()
            .ok_or("repair INPUT.tack OUTPUT.tack [REPORT.json]")?,
    );
    let target = PathBuf::from(args.get(1).ok_or("repair output")?);
    let output = args.get(2).map(PathBuf::from);
    crate::report_output::preflight(output.as_deref(), Some(&target))?;
    require_repair_target(&input, &target)?;
    let owner = if fs::symlink_metadata(&target).is_ok() {
        storage::BoardLease::acquire(&target)?
    } else {
        storage::BoardLease::acquire_new(&target)?
    };
    let work = Workspace::new()?;
    let board = Arc::new(if owner.path() == input.canonicalize()? {
        owner.open()?
    } else {
        TackFile::open(&input)?
    });
    if board
        .document
        .sources()
        .any(|s| matches!(s.location(), SourceLocation::Linked(p) if !p.is_absolute()))
        && input
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."))
            .canonicalize()?
            != target
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(Path::new("."))
                .canonicalize()?
    {
        return Err(Box::new(storage::StorageError::Unsupported(
            "relative links require same-directory repair; explicit relink needed for Save As",
        )));
    }
    require_repair_target(&input, &target)?;
    let (assets, _, preparation_ms) =
        prepare_all(Arc::clone(&board), &input, work.0.join("overview"))?;
    let save_start = Instant::now();
    require_repair_target(&input, &target)?;
    owner.save(&board.document, save_inputs(&board, &assets))?;
    let stats = assets.stats();
    emit(
        output.as_deref(),
        &json!({"operation":"repair","preparation_ms":preparation_ms,"save_ms":save_start.elapsed().as_secs_f64()*1000.,"total_ms":started.elapsed().as_secs_f64()*1000.,"overview_reused":stats.reused,"overview_regenerated":stats.regenerated,"source_bytes":stats.source_bytes,"container_bytes":stats.container_bytes+board.metadata_bytes_read,"derived_bytes_written":stats.derived_bytes,"objects":board.document.object_order().len(),"errors":stats.errors}),
    )
}
fn inspect(args: Vec<OsString>) -> Result<(), AssetError> {
    let start = Instant::now();
    let b = TackFile::open(PathBuf::from(args.first().ok_or("inspect FILE.tack")?))?;
    emit(
        None,
        &json!({"document_id":format!("{:032x}",b.document.id().value()),"objects":b.document.object_order().len(),"assets":b.document.assets().count(),"sources":b.document.sources().count(),"embedded":b.originals.len(),"overviews":b.overviews.len(),"discarded_overviews":b.discarded_overviews,"metadata_bytes":b.metadata_bytes_read,"metadata_ms":start.elapsed().as_secs_f64()*1000.,"original_bytes_read":0}),
    )
}
fn query_scale(args: Vec<OsString>) -> Result<(), AssetError> {
    crate::report_output::preflight(args.first().map(Path::new), None)?;
    let work = Workspace::new()?;
    let mut reports = Vec::new();
    for count in [1000, 5000, 10000] {
        let mut d = Document::new(storage::new_document_id()?, DocumentLimits::default());
        let source = storage::new_source_id()?;
        let asset = storage::new_asset_id()?;
        d.apply(Command::AddSource(Source::linked(source, "missing.jpg")?))?;
        d.apply(Command::AddAsset(ImageAsset::new(
            asset,
            source,
            [6000, 4500],
        )?))?;
        for index in 0..count {
            d.apply(Command::AddObject {
                object: DocumentObject::image(
                    storage::new_object_id()?,
                    asset,
                    Transform::new(
                        [(index % 100) as f64 * 680., (index / 100) as f64 * 530.],
                        [600., 450.],
                        0.,
                        [false, false],
                    )?,
                ),
                index,
            })?;
        }
        let p = work.0.join("scale.tack");
        let start = Instant::now();
        storage::save(&p, &d, vec![])?;
        let save_ms = start.elapsed().as_secs_f64() * 1000.;
        let start = Instant::now();
        let b = TackFile::open(&p)?;
        let load_ms = start.elapsed().as_secs_f64() * 1000.;
        let viewport = WorldRect::new(-400., -300., 2000., 1400.)?;
        let mut times = Vec::new();
        let mut visible = 0;
        for _ in 0..100 {
            let start = Instant::now();
            visible = std::hint::black_box(b.document.objects_in_view(viewport).count());
            times.push(start.elapsed().as_secs_f64() * 1000.);
        }
        times.sort_by(f64::total_cmp);
        reports.push(json!({"objects":count,"file_bytes":fs::metadata(p)?.len(),"metadata_read_bytes":b.metadata_bytes_read,"save_ms":save_ms,"load_ms":load_ms,"query_visible":visible,"query_p50_ms":times[49],"query_p99_ms":times[98],"query_max_ms":times[99]}));
    }
    emit(
        args.first().map(Path::new),
        &json!({"operation":"query-scale","complexity":"ordered O(n log n) scan; allocation-free query; no spatial index","runs":reports}),
    )
}
