//! Storage-only measurement: synthetic embedded bytes, no image decoding.
use std::{collections::BTreeMap, fs, io::Write, path::PathBuf, sync::Arc, time::Instant};
use tack_app::{
    image_save::{Originals, retain_originals, snapshot_inputs},
    local_relink::RelinkReady,
};
use tack_core::*;
use tack_storage::*;

fn main() -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let root = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("output directory required")?,
    );
    fs::create_dir(&root)?;
    let source = root.join("synthetic-original.bin");
    let mut output = fs::File::create(&source)?;
    // Fully written, non-sparse storage payload. Not a representative photograph.
    let bytes: Vec<u8> = (0..65536)
        .map(|i| ((i * 17 + i / 251) % 256) as u8)
        .collect();
    for _ in 0..512 {
        output.write_all(&bytes)?;
    }
    output.sync_all()?;
    drop(output);
    let sid = SourceId::new(1)?;
    let aid = AssetId::new(1)?;
    let mut document = Document::new(new_document_id()?, DocumentLimits::default());
    document.apply(Command::AddSource(Source::embedded(sid)))?;
    document.apply(Command::AddAsset(ImageAsset::new(aid, sid, [2, 1])?))?;
    for i in 1..=5000 {
        document.apply(Command::AddObject {
            object: DocumentObject::image(
                ObjectId::new(i)?,
                aid,
                Transform::new([i as f64 * 10., 0.], [20., 10.], 0., [false; 2])?,
            ),
            index: document.object_order().len(),
        })?;
    }
    let path = root.join("board.tack");
    save(
        &path,
        &document,
        vec![BlobInput::original(sid, 1, Payload::File(source.clone()))],
    )?;
    fs::remove_file(source)?;
    let lease = BoardLease::acquire(&path)?;
    let mut board = lease.open()?;
    let before = fs::metadata(&path)?.len();
    let mut editor = DocumentEditor::new(board.document.clone(), 200);
    editor.execute(Command::AddObject {
        object: DocumentObject::frame(
            ObjectId::new(5001)?,
            "recovery edit".into(),
            Transform::new([0.; 2], [10.; 2], 0., [false; 2])?,
        )?,
        index: editor.document().object_order().len(),
    })?;
    let started = Instant::now();
    let recovery_bytes = lease.save_recovery(
        editor.document(),
        editor.generation(),
        snapshot_inputs(
            &board,
            editor.document(),
            &BTreeMap::new(),
            &Originals::new(),
        )?,
    )?;
    let recovery_ms = started.elapsed().as_secs_f64() * 1000.;
    // Real valid replacement PNG; original payload intentionally tests storage only.
    let replacement = root.join("replacement.png");
    fs::write(
        &replacement,
        [
            137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 2, 0, 0, 0, 1,
            8, 6, 0, 0, 0, 244, 34, 127, 138, 0, 0, 0, 14, 73, 68, 65, 84, 120, 156, 99, 248, 207,
            192, 240, 31, 132, 1, 17, 247, 3, 253, 227, 197, 245, 239, 0, 0, 0, 0, 73, 69, 78, 68,
            174, 66, 96, 130,
        ],
    )?;
    RelinkReady::read(
        editor.document().source(sid).ok_or("source")?.clone(),
        &replacement,
    )?
    .apply(&mut editor)?;
    lease.save(
        editor.document(),
        snapshot_inputs(
            &board,
            editor.document(),
            &BTreeMap::new(),
            &Originals::new(),
        )?,
    )?;
    let publication = lease.open()?;
    let after = fs::metadata(&path)?.len();
    let mut originals = Originals::new();
    retain_originals(&board, &publication, &editor, &mut originals);
    board = publication;
    let Payload::Stored { file, .. } = originals.values().next().ok_or("retained original")? else {
        return Err("expected pinned container".into());
    };
    let weak = Arc::downgrade(file);
    let metadata = file.metadata()?;
    #[cfg(unix)]
    let allocation = {
        use std::os::unix::fs::MetadataExt;
        serde_json::json!({"inode":metadata.ino(), "links":metadata.nlink(), "allocated_bytes":metadata.blocks()*512})
    };
    #[cfg(not(unix))]
    let allocation = serde_json::Value::Null;
    let retained_bytes = metadata.len();
    editor.undo()?;
    lease.save(
        editor.document(),
        snapshot_inputs(&board, editor.document(), &BTreeMap::new(), &originals)?,
    )?;
    lease.open()?.verify_originals()?;
    let undo_bytes = fs::metadata(&path)?.len();
    drop(originals);
    let released = weak.upgrade().is_none();
    let result = serde_json::json!({
        "scope":"storage isolation; 5000 objects, one shared 32 MiB synthetic blob; no codec/photo timing",
        "objects":5000, "original_bytes":32*1024*1024, "initial_board_bytes":before,
        "whole_board_recovery_bytes":recovery_bytes, "recovery_worker_ms":recovery_ms,
        "relinked_board_bytes":after, "retained_old_container_bytes":retained_bytes,
        "retained_old_container_allocation":allocation, "undo_saved_board_bytes":undo_bytes,
        "originals_verified_after_undo":true, "old_handle_released_after_explicit_drop":released,
        "release_scope":"explicit harness drop, not proof of production history compaction"
    });
    fs::write(
        root.join("summary.json"),
        format!("{}\n", serde_json::to_string_pretty(&result)?),
    )?;
    println!("{result}");
    Ok(())
}
