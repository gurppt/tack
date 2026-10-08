//! Generated-corpus linked boards without decoder/preparation work.
//! Called by tools/generate_phase1l_corpus.py boards --builder EXISTING_BINARY.
//! Source dimensions are trusted generated manifest metadata, not decoder evidence.
use serde_json::{Value, json};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};
use tack_assets::{AssetError, BenchmarkBoard, source_fingerprint};
use tack_core::*;

fn require_registered(root: &Path, path: &Path, ownership: &Value) -> Result<(), AssetError> {
    let relative = path
        .strip_prefix(root)?
        .to_str()
        .ok_or("non-UTF8 generated path")?;
    if !ownership["files"]
        .as_array()
        .ok_or("ownership files")?
        .iter()
        .any(|entry| entry.as_str() == Some(relative))
    {
        return Err("output must be journaled by the corpus generator".into());
    }
    match fs::symlink_metadata(path) {
        Ok(_) => Err("output already exists".into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn guard_outputs(manifest: &Path, board: &Path, report: &Path) -> Result<(), AssetError> {
    let root = manifest.parent().ok_or("manifest directory")?;
    if root.file_name().and_then(|s| s.to_str()) != Some("phase1l_generated")
        || root
            .parent()
            .and_then(Path::file_name)
            .and_then(|s| s.to_str())
            != Some("test_file")
    {
        return Err("manifest must be in test_file/phase1l_generated".into());
    }
    for name in [".phase1l-owned", "ownership.json"] {
        if !fs::symlink_metadata(root.join(name))?.is_file() {
            return Err("ownership metadata must be regular files".into());
        }
    }
    if fs::read_to_string(root.join(".phase1l-owned"))? != "tack-phase1l-corpus-v1\n" {
        return Err("invalid corpus sentinel".into());
    }
    let ownership: Value = serde_json::from_slice(&fs::read(root.join("ownership.json"))?)?;
    if ownership["owner"] != "tack-phase1l-corpus-v1" || ownership["schema"] != 1 {
        return Err("invalid corpus ownership".into());
    }
    if !fs::symlink_metadata(root.join("boards"))?.is_dir() {
        return Err("boards must be a regular directory".into());
    }
    for path in [board, report] {
        if path.parent() != Some(root.join("boards").as_path()) {
            return Err("output must be in corpus boards directory".into());
        }
        require_registered(root, path, &ownership)?;
    }
    Ok(())
}

fn document(manifest: &Path) -> Result<Document, AssetError> {
    let geometry = BenchmarkBoard::read_manifest(manifest)?;
    let metadata: Value = serde_json::from_slice(&fs::read(manifest)?)?;
    let rows = metadata["objects"].as_array().ok_or("source metadata")?;
    let mut document = Document::new(tack_storage::new_document_id()?, DocumentLimits::default());
    for (index, (row, image)) in rows.iter().zip(geometry.objects).enumerate() {
        let mut dimensions = [0; 2];
        for (dimension, name) in dimensions.iter_mut().zip(["source_width", "source_height"]) {
            *dimension = u32::try_from(row[name].as_u64().ok_or("source dimensions")?)?;
        }
        let source = Source::from_descriptor(
            tack_storage::new_source_id()?,
            SourceLocation::Linked(LinkedPath::native(&image.path)?),
            1,
            Some(source_fingerprint(&image.path)?),
        )?;
        let asset = ImageAsset::new(tack_storage::new_asset_id()?, source.id(), dimensions)?;
        document.apply(Command::AddSource(source))?;
        document.apply(Command::AddAsset(asset))?;
        let rect = image.rect;
        let transform = Transform::new(
            [rect.x + rect.width / 2., rect.y + rect.height / 2.],
            [rect.width, rect.height],
            0.,
            [false; 2],
        )?;
        document.apply(Command::AddObject {
            object: DocumentObject::image(tack_storage::new_object_id()?, asset.id(), transform),
            index,
        })?;
    }
    Ok(document)
}

fn main() -> Result<(), AssetError> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 3 {
        return Err("usage: phase1l_corpus MANIFEST OUTPUT.tack REPORT.json".into());
    }
    let manifest = PathBuf::from(&args[0]).canonicalize()?;
    let output = PathBuf::from(&args[1]);
    let report = PathBuf::from(&args[2]);
    guard_outputs(&manifest, &output, &report)?;
    let document = document(&manifest)?;
    let owner = tack_storage::BoardLease::acquire_new(&output)?;
    owner.save(&document, vec![])?;
    let receipt = json!({"manifest": manifest, "output": output,
        "trusted_generated_source_metadata": true, "previews": false,
        "board_encoded_bytes": fs::metadata(&output)?.len()});
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(report)?;
    file.write_all(serde_json::to_string_pretty(&receipt)?.as_bytes())?;
    file.write_all(b"\n")?;
    println!("{receipt}");
    Ok(())
}
