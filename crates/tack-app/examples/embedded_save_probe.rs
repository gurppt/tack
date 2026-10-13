//! Storage-only experiment: valid owned PNG bytes; deliberately bypasses image admission.
use std::{fs::File, io::Read, path::PathBuf, sync::Arc, time::Instant};
use tack_core::{
    AssetId, Command, Document, DocumentLimits, DocumentObject, ImageAsset, ObjectId, Source,
    SourceId, Transform,
};
use tack_storage::{BlobInput, Payload, SaveStage, TackFile};

fn main() -> Result<(), tack_assets::AssetError> {
    let root = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("owned directory required")?,
    );
    if std::fs::read_to_string(root.join("OWNER_SAVE_EXPERIMENT.txt"))?
        != "P0 F disposable storage experiment; never a user board\n"
    {
        return Err("refusing unowned directory".into());
    }
    let operation = std::env::args().nth(2).ok_or("operation required")?;
    let path = root.join("storage-only.tack");
    let input = root.join("source.png");
    let source = SourceId::new(2)?;
    let object = ObjectId::new(4)?;
    let started = Instant::now();
    let (mut document, inputs) = if operation == "first" {
        if path.exists() {
            return Err("first save requires absent experimental board".into());
        }
        let mut header = [0; 24];
        File::open(&input)?.read_exact(&mut header)?;
        if &header[..8] != b"\x89PNG\r\n\x1a\n" {
            return Err("real PNG source required".into());
        }
        let width = u32::from_be_bytes(header[16..20].try_into()?);
        let height = u32::from_be_bytes(header[20..24].try_into()?);
        let mut d = Document::new(tack_storage::new_document_id()?, DocumentLimits::default());
        d.apply(Command::AddSource(Source::embedded(source)))?;
        d.apply(Command::AddAsset(ImageAsset::new(
            AssetId::new(3)?,
            source,
            [width, height],
        )?))?;
        d.apply(Command::AddObject {
            object: DocumentObject::image(
                object,
                AssetId::new(3)?,
                Transform::new([0.; 2], [100.; 2], 0., [false; 2])?,
            ),
            index: 0,
        })?;
        (
            d,
            vec![BlobInput::original(source, 1, Payload::File(input.clone()))],
        )
    } else {
        let board = Arc::new(TackFile::open(&path)?);
        let inputs = vec![BlobInput::original(
            source,
            1,
            board.payload(board.originals[&source].range),
        )];
        (board.document.clone(), inputs)
    };
    match operation.as_str() {
        "first" | "unchanged" | "second" => {}
        "minor" => {
            document.apply(Command::SetTransform {
                object,
                transform: Transform::new([1., 0.], [100.; 2], 0., [false; 2])?,
            })?;
        }
        "kill-metadata" | "kill-synced" => {
            document.apply(Command::SetTransform {
                object,
                transform: Transform::new([2., 0.], [100.; 2], 0., [false; 2])?,
            })?;
        }
        _ => return Err("unknown storage experiment operation".into()),
    }
    tack_storage::save_with_hook(&path, &document, inputs, |stage| {
        if (operation == "kill-metadata" && stage == SaveStage::MetadataWritten)
            || (operation == "kill-synced" && stage == SaveStage::FileSynced)
        {
            // Explicit process termination skips Rust destructors: temp generation remains.
            std::process::exit(86);
        }
        Ok(())
    })?;
    let save_ms = started.elapsed().as_secs_f64() * 1000.;
    let board = TackFile::open(&path)?;
    board.verify_originals()?;
    if board.document != document {
        return Err("reopened document mismatch".into());
    }
    let original = tack_shared::hash_reader(&mut board.original_reader(source)?)?;
    let input_hash = tack_shared::hash_reader(&mut File::open(input)?)?;
    if original != input_hash {
        return Err("reopened embedded original mismatch".into());
    }
    println!(
        "{}",
        serde_json::json!({"operation":operation,"save_ms":save_ms,
        "source_bytes":board.originals[&source].range.len,"board_bytes":std::fs::metadata(path)?.len(),
        "original_sha256":original.0.to_string(),"metadata_read_bytes":board.metadata_bytes_read,
        "reopen_crc_document_sha_verified":true,"scope":"storage only, no image admission/render claim"})
    );
    Ok(())
}
