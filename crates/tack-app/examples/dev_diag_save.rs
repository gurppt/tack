//! Disposable staged save measurement, requires a fixture-owner marker.
use std::{path::PathBuf, sync::Arc, time::Instant};
fn main() -> Result<(), tack_assets::AssetError> {
    let path = PathBuf::from(std::env::args_os().nth(1).ok_or("fixture board required")?);
    let root = path.parent().ok_or("fixture root")?;
    if std::fs::read_to_string(root.join("OWNER_DIAG_FIXTURES"))
        .ok()
        .as_deref()
        != Some("DEV-DIAG-WINLAN-01 disposable fixtures\n")
    {
        return Err("refusing unowned board".into());
    }
    let _session = tack_storage::diagnostics::Session::start(
        "save-probe",
        serde_json::json!({"scope":"owned fixture repeated durable saves"}),
    )?;
    let lease = tack_storage::BoardLease::acquire(&path)?;
    let mut board = Arc::new(lease.open()?);
    let mut times = Vec::new();
    for _ in 0..5 {
        let start = Instant::now();
        let inputs = tack_app::image_save::snapshot_inputs(
            &board,
            &board.document,
            &Default::default(),
            &Default::default(),
        )?;
        lease.save(&board.document, inputs)?;
        board = Arc::new(lease.open()?);
        times.push(start.elapsed().as_secs_f64() * 1000.);
    }
    println!(
        "{}",
        serde_json::json!({"scope":"5 unchanged saves, warm OS cache, durability unchanged","ms":times,"board_bytes":std::fs::metadata(path)?.len()})
    );
    Ok(())
}
