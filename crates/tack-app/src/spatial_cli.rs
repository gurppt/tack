//! Benchmark-only generated metadata; never opens private sources/documents.
use serde_json::json;
use std::{ffi::OsString, path::PathBuf, time::Instant};
use tack_app::{
    image_interaction::{GestureKind, ImageInteraction},
    spatial_layout::{self, Layout},
};
use tack_assets::AssetError;
use tack_core::*;
fn fixture(count: usize) -> Result<Document, AssetError> {
    let mut d = Document::new(DocumentId::new(1)?, DocumentLimits::default());
    d.apply(Command::AddSource(Source::linked(
        SourceId::new(1)?,
        "missing-generated.png",
    )?))?;
    d.apply(Command::AddAsset(ImageAsset::new(
        AssetId::new(1)?,
        SourceId::new(1)?,
        [640, 480],
    )?))?;
    for i in 0..count {
        d.apply(Command::AddObject {
            object: DocumentObject::image(
                ObjectId::new(i as u128 + 1)?,
                AssetId::new(1)?,
                Transform::new(
                    [(i % 32) as f64 * 1024., (i / 32) as f64 * 1024.],
                    [640., 480.],
                    0.,
                    [false; 2],
                )?,
            ),
            index: i,
        })?;
    }
    Ok(d)
}
pub fn run(args: Vec<OsString>) -> Result<(), AssetError> {
    if args.len() != 1 {
        return Err("spatial-scale NEW_OUTPUT_DIRECTORY (benchmark only)".into());
    }
    let root = PathBuf::from(&args[0]);
    tack_storage::create_private_directory(&root, false)?;
    for count in [1000, 5000, 10000] {
        tack_storage::save(
            root.join(format!("snap-{count}.tack")),
            &fixture(count)?,
            vec![],
        )?;
    }
    let mut layout_runs = Vec::new();
    for count in [10, 100, 1000] {
        let mut editor = DocumentEditor::new(fixture(count)?, 200);
        let before = editor.document().clone();
        let ids = before.object_order().to_vec();
        for action in [
            Layout::Left,
            Layout::HorizontalCenter,
            Layout::Right,
            Layout::Top,
            Layout::VerticalCenter,
            Layout::Bottom,
            Layout::DistributeHorizontal,
            Layout::DistributeVertical,
            Layout::PackHorizontal,
            Layout::PackVertical,
        ] {
            let mut samples = Vec::new();
            for _ in 0..64 {
                let start = Instant::now();
                let command =
                    spatial_layout::arrange(editor.document(), ids.iter().copied(), action)?;
                let changed = editor.execute(command)?;
                samples.push(start.elapsed().as_secs_f64() * 1000.);
                if changed {
                    editor.undo()?;
                }
                if editor.document() != &before {
                    return Err("layout roundtrip invariant".into());
                }
            }
            layout_runs.push(json!({"count":count,"action":format!("{action:?}"),"action_ms":samples,"invariant":true}));
        }
    }
    let mut groups = Vec::new();
    for count in [10, 100] {
        let mut editor = DocumentEditor::new(fixture(count)?, 200);
        let ids = editor.document().object_order().to_vec();
        editor.execute(Command::AddGroup(Group::new(
            GroupId::new(1)?,
            ids.clone(),
        )?))?;
        let before = editor.document().clone();
        let mut input = ImageInteraction::default();
        input
            .selection
            .select_object(editor.document(), Some(ids[0]), false);
        let mut samples = Vec::new();
        for _ in 0..64 {
            let start = Instant::now();
            input.begin(GestureKind::Move, [0., 0.], &editor)?;
            input.update([128., 64.])?;
            input.commit(&mut editor)?;
            samples.push(start.elapsed().as_secs_f64() * 1000.);
            editor.undo()?;
            if editor.document() != &before {
                return Err("group roundtrip invariant".into());
            }
        }
        groups.push(json!({"members":count,"gesture_ms":samples,"invariant":true}));
    }
    let mut frames = Vec::new();
    for count in [10, 100, 1000] {
        let start = Instant::now();
        let mut d = Document::new(DocumentId::new(1)?, DocumentLimits::default());
        for i in 0..count {
            d.apply(Command::AddObject {
                object: DocumentObject::frame(
                    ObjectId::new(i as u128 + 1)?,
                    format!("Zone {i} — 猫"),
                    Transform::new(
                        [(i % 32) as f64 * 400., (i / 32) as f64 * 300.],
                        [320., 240.],
                        0.,
                        [false; 2],
                    )?,
                )?,
                index: i,
            })?;
        }
        let create_ms = start.elapsed().as_secs_f64() * 1000.;
        let path = root.join(format!("frames-{count}.tack"));
        let start = Instant::now();
        tack_storage::save(&path, &d, vec![])?;
        let save_ms = start.elapsed().as_secs_f64() * 1000.;
        let start = Instant::now();
        let reopened = tack_storage::TackFile::open(&path)?;
        let open_ms = start.elapsed().as_secs_f64() * 1000.;
        if reopened.document != d {
            return Err("frame roundtrip invariant".into());
        }
        frames.push(json!({"count":count,"create_ms":create_ms,"save_ms":save_ms,"open_ms":open_ms,"bytes":std::fs::metadata(path)?.len(),"invariant":true}));
    }
    let sizes = json!({"document":std::mem::size_of::<Document>(),"document_object":std::mem::size_of::<DocumentObject>(),"command":std::mem::size_of::<Command>(),"image_input":std::mem::size_of::<tack_app::image_input::ImageInput>()});
    let report = json!({"operation":"spatial-scale","generated_only":true,"source_pixels":false,"allocations":"not instrumented","layout":layout_runs,"groups":groups,"frames":frames,"sizes_bytes":sizes});
    crate::report_output::write_new(
        &root.join("metadata.json"),
        &serde_json::to_vec_pretty(&report)?,
    )?;
    Ok(())
}
