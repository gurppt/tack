//! Reproducible metadata-only stress; does not read or copy an original image.
use serde_json::json;
use std::{path::PathBuf, time::Instant};
use tack_core::*;
fn main() -> Result<(), tack_assets::AssetError> {
    let root = PathBuf::from(std::env::args_os().nth(1).ok_or("output directory")?);
    std::fs::create_dir_all(&root)?;
    let empty = Document::new(tack_storage::new_document_id()?, DocumentLimits::default());
    tack_storage::save(root.join("empty.tack"), &empty, vec![])?;
    let views = (1..=64)
        .map(|i| {
            Ok(CameraBookmark::new(
                BookmarkId::new(i)?,
                format!("View {i}"),
                [i as f64, -(i as f64)],
                6.5,
            )?)
        })
        .collect::<Result<Vec<_>, tack_assets::AssetError>>()?;
    let mut editor = DocumentEditor::new(empty, 200);
    let started = Instant::now();
    editor.execute(Command::SetCameraBookmarks(views))?;
    let bookmark_ms = started.elapsed().as_secs_f64() * 1000.;
    let bookmark_history_bytes = editor.history_bytes();
    tack_storage::save(root.join("max-bookmarks.tack"), editor.document(), vec![])?;
    let mut camera = Camera::new([800, 600]);
    for bookmark in editor.document().bookmarks() {
        bookmark.jump(&mut camera)?;
    }
    let mut duplicate = Vec::new();
    for count in [1000usize, 10000] {
        let mut doc = Document::new(tack_storage::new_document_id()?, DocumentLimits::default());
        let source = SourceId::new(1)?;
        let asset = AssetId::new(1)?;
        doc.apply(Command::AddSource(Source::from_descriptor(
            source,
            SourceLocation::Linked(LinkedPath::native(std::path::Path::new(
                "absent-900GiB.jpg",
            ))?),
            1,
            Some(SourceFingerprint {
                size: 900 * 1024 * 1024 * 1024,
                modified_seconds: 0,
                modified_nanos: 0,
            }),
        )?))?;
        doc.apply(Command::AddAsset(ImageAsset::new(
            asset, source, [50000; 2],
        )?))?;
        for index in 0..count {
            doc.apply(Command::AddObject {
                object: DocumentObject::image(
                    ObjectId::new(index as u128 + 1)?,
                    asset,
                    Transform::new([index as f64, 0.], [100.; 2], 0., [false; 2])?,
                ),
                index,
            })?;
        }
        let mut editor = DocumentEditor::new(doc, 200);
        let started = Instant::now();
        let (command, ids) = tack_app::duplicate::selection(
            editor.document(),
            editor.document().object_order().iter().copied(),
            [32.; 2],
        )?;
        let construct_ms = started.elapsed().as_secs_f64() * 1000.;
        let started = Instant::now();
        editor.execute(command)?;
        let commit_ms = started.elapsed().as_secs_f64() * 1000.;
        let after = editor.document().clone();
        if ids.len() != count
            || after.assets().count() != 1
            || after.sources().count() != 1
            || editor.undo_len() != 1
        {
            return Err("duplication stress invariant".into());
        }
        editor.undo()?;
        editor.redo()?;
        if editor.document() != &after {
            return Err("duplicate stress redo changed identities".into());
        }
        duplicate.push(json!({"objects":count,"construct_ms":construct_ms,"commit_ms":commit_ms,"history_bytes":editor.history_bytes(),"original_size_metadata":900u64*1024*1024*1024,"original_bytes_read":0,"asset_count":1,"source_count":1}));
    }
    let report = json!({"bookmark_count":64,"bookmark_commit_ms":bookmark_ms,"bookmark_history_bytes":bookmark_history_bytes,"bookmark_file_bytes":std::fs::metadata(root.join("max-bookmarks.tack"))?.len(),"duplicates":duplicate});
    std::fs::write(
        root.join("metadata-cost.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    println!("{report}");
    Ok(())
}
