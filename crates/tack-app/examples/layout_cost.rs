//! Developer-only CPU/allocation evidence. No GPU, idle work or runtime service.
use std::{error::Error, time::Instant};
use tack_app::{
    spatial_layout::{self, Layout},
    visibility::Visibility,
};
use tack_core::*;
type R<T = ()> = Result<T, Box<dyn Error + Send + Sync>>;
fn main() -> R {
    let args: Vec<_> = std::env::args().collect();
    let n: usize = args.get(1).ok_or("count")?.parse()?;
    let mode = args.get(2).map(String::as_str).unwrap_or("full");
    let mut d = Document::new(DocumentId::new(1)?, DocumentLimits::default());
    d.apply(Command::AddSource(Source::linked(
        SourceId::new(1)?,
        "generated.png",
    )?))?;
    d.apply(Command::AddAsset(ImageAsset::new(
        AssetId::new(1)?,
        SourceId::new(1)?,
        [100, 100],
    )?))?;
    for i in 0..n {
        d.apply(Command::AddObject {
            object: DocumentObject::image(
                ObjectId::new(i as u128 + 1)?,
                AssetId::new(1)?,
                Transform::new(
                    [i as f64 * 13., (i % 7) as f64 * 19.],
                    if i % 2 == 0 { [120., 80.] } else { [60., 140.] },
                    0.,
                    [false; 2],
                )?,
            ),
            index: i,
        })?;
    }
    let mut e = DocumentEditor::new(d, 10);
    let start = Instant::now();
    let command = if mode == "baseline" {
        Command::Batch(vec![])
    } else {
        spatial_layout::arrange(
            e.document(),
            e.document().object_order().iter().copied(),
            Layout::Grid,
        )?
    };
    let plan_ms = start.elapsed().as_secs_f64() * 1000.;
    let planned_bytes = command.retained_bytes();
    let start = Instant::now();
    if mode == "full" {
        e.execute(command)?;
    } else {
        drop(command);
    }
    let apply_ms = start.elapsed().as_secs_f64() * 1000.;
    let history_bytes = e.history_bytes();
    let all =
        spatial_layout::bounds(e.document().objects().map(|o| o.transform())).ok_or("bounds")?;
    let start = Instant::now();
    let mut visibility = Visibility::default();
    visibility.refresh(e.document(), e.generation());
    let visible = if visibility.cached() {
        visibility.candidates(all, |_| false).count()
    } else {
        e.document().objects_in_view(all).count()
    };
    let redraw_query_ms = start.elapsed().as_secs_f64() * 1000.;
    println!(
        "{}",
        serde_json::json!({"n":n,"mode":mode,"plan_ms":plan_ms,"apply_ms":apply_ms,"planned_command_bytes":planned_bytes,"history_bytes":history_bytes,"redraw_query_ms":redraw_query_ms,"visibility_bytes":visibility.bytes(),"visible":visible,"scope":"CPU plan, atomic history apply, first redraw visibility rebuild; actual GPU redraw measured in native matrix; allocator separately instrumented via glibc memusage"})
    );
    Ok(())
}
