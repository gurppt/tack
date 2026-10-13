//! Real admission → supply → durable save/reopen, only on marked private fixtures.
use std::{
    collections::BTreeMap,
    fs::File,
    path::PathBuf,
    sync::{Arc, atomic::AtomicBool},
    time::{Duration, Instant},
};
use tack_app::{
    image_save,
    local_import::{self, ImportRequest, ImportUpdate},
};
use tack_assets::{AssetError, ProductAssets, ProductDemand};
use tack_core::{Document, DocumentEditor, DocumentLimits, Lod};
fn main() -> Result<(), AssetError> {
    let root = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("owned fixture directory required")?,
    );
    if std::fs::read_to_string(root.join("OWNER_STAGE_A.txt"))?
        != "P0-WIN-IMAGE-CORE-02 Stage A disposable fixtures\n"
    {
        return Err("unowned fixture directory".into());
    }
    let ext = std::env::args().nth(2).ok_or("jpg/png required")?;
    if !["jpg", "png"].contains(&ext.as_str()) {
        return Err("jpg/png required".into());
    }
    let embedded = std::env::args().nth(3).as_deref() == Some("embedded");
    let file = std::env::args()
        .nth(4)
        .unwrap_or_else(|| format!("noise-8000x9000.{ext}"));
    if file.contains(['/', '\\']) || file == "." || file == ".." {
        return Err("fixture basename required".into());
    }
    let source = root.join(&file);
    let encoded = std::fs::metadata(&source)?.len();
    let plan = tack_assets::image_import_plan(&source, encoded)?;
    let work = root.join(format!(
        "proof-{ext}-{embedded}-{}",
        tack_storage::new_document_id()?.value()
    ));
    tack_storage::create_private_directory(&work, true)?;
    std::fs::write(
        work.join("OWNER_PROBE.txt"),
        b"P0 D owned admission/supply/save experiment\n",
    )?;
    let seed = work.join("seed.tack");
    let empty = Document::new(tack_storage::new_document_id()?, DocumentLimits::default());
    tack_storage::save(&seed, &empty, vec![])?;
    let board = Arc::new(tack_storage::TackFile::open(&seed)?);
    let mut editor = DocumentEditor::new(empty, 200);
    let mut originals = BTreeMap::new();
    let mut failure = None;
    let started = Instant::now();
    ImportRequest {
        temporary: None,
        paths: vec![source.clone()],
        embedded,
        position: [0., 0.],
        viewport: [800., 600.],
        sampling: tack_core::ImageFiltering::Default,
        mouse_easter_zoom: None,
        work: work.join("imports"),
        spool: None,
    }
    .run(&AtomicBool::new(false), |update| {
        match update {
            ImportUpdate::Admitted(image) => {
                if let Err(e) = local_import::admit(&mut editor, &image) {
                    failure = Some(e.to_string());
                    return false;
                }
                if let Some(original) = image.original {
                    originals.insert((image.source.id(), image.source.revision()), original);
                }
            }
            ImportUpdate::Failed { message, .. } => failure = Some(message),
            _ => {}
        }
        true
    })?;
    if let Some(e) = failure {
        return Err(e.into());
    }
    let admission_ms = started.elapsed().as_secs_f64() * 1000.;
    let asset = *editor
        .document()
        .assets()
        .next()
        .ok_or("no admitted image")?;
    let mut assets = ProductAssets::new(Arc::clone(&board), &seed, work.join("derived"))?;
    let mut results = Vec::new();
    for (lod, edge) in [
        (Lod::Thumbnail, 128),
        (Lod::Medium, 512),
        (Lod::Detail, 2048),
    ] {
        let started = Instant::now();
        assets.replace_view(
            &[ProductDemand {
                asset: asset.id(),
                lod,
                edge,
                priority: 2,
                resident: false,
            }],
            editor.document(),
            &originals,
        );
        loop {
            assets.poll();
            if let Some(pixels) = assets.get_rep(asset.id(), 1, lod, edge) {
                results.push(serde_json::json!({"tier":format!("{lod:?}"),"pixels":[pixels.width,pixels.height],
                    "rgba_bytes":pixels.rgba.len(),"ms":started.elapsed().as_secs_f64()*1000.}));
                break;
            }
            if assets.stats().pending == 0 && assets.stats().errors > 0 {
                return Err(assets
                    .failure_reason(asset.id())
                    .unwrap_or("representation failed")
                    .into());
            }
            if started.elapsed() > Duration::from_secs(45) {
                return Err("representation deadline".into());
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    }
    let saved = work.join("saved.tack");
    let inputs =
        image_save::snapshot_inputs(&board, editor.document(), &assets.prepared, &originals)?;
    let started = Instant::now();
    tack_storage::save(&saved, editor.document(), inputs)?;
    let save_ms = started.elapsed().as_secs_f64() * 1000.;
    let loaded = Arc::new(tack_storage::TackFile::open(&saved)?);
    loaded.verify_originals()?;
    if loaded.document != *editor.document() {
        return Err("reopened document differs".into());
    }
    let expected = tack_shared::hash_reader(&mut File::open(&source)?)?;
    let original_hash = if embedded {
        let actual = tack_shared::hash_reader(&mut loaded.original_reader(asset.source_id())?)?;
        if actual != expected {
            return Err("embedded original differs".into());
        }
        Some(actual.0.to_string())
    } else {
        None
    };
    let stats = assets.stats();
    println!(
        "{}",
        serde_json::json!({"file":file,"format":ext,"embedded":embedded,"encoded_bytes":encoded,
        "pixels":plan.pixels,"admission_limit":plan.max_encoded,"streamed":plan.streamed,
        "admission_ms":admission_ms,"representations":results,"save_ms":save_ms,"saved_path":saved,
        "source_sha256":expected.0.to_string(),"embedded_sha256":original_hash,
        "source_read_bytes":stats.source_bytes,"container_read_bytes":stats.container_bytes,
        "cpu_cache_peak":stats.cpu_peak,"decoder_requests":stats.codec_requests,"errors":stats.errors,
        "scope":"real production supply; GPU presentation and physical Win10 still separate"})
    );
    Ok(())
}
