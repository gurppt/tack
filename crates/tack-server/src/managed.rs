//! Explicit desktop-host lifetime and bounded streaming offline checkpoint.
use std::path::{Path, PathBuf};
#[derive(Clone)]
pub struct Config {
    pub ready: PathBuf,
    pub snapshot: PathBuf,
    pub board: tack_shared::WireId,
}
pub fn checkpoint(config: &Config, boards: &Path) -> crate::Result<()> {
    if !boards.join(format!("{}.board", config.board)).exists() {
        return Ok(());
    }
    let authority = crate::Authority::open(boards, config.board)?;
    let assets = boards.parent().ok_or("authority root")?.join("assets");
    let sources: Vec<_> = authority
        .sources()
        .iter()
        .map(|b| {
            (
                b.clone(),
                tack_storage::Payload::File(assets.join(b.hash.to_string())),
            )
        })
        .collect();
    let (document, inputs) = tack_shared::publish::snapshot_inputs(authority.document(), &sources)
        .map_err(|e| e.to_string())?;
    let lease = tack_storage::BoardLease::acquire(&config.snapshot).map_err(|e| e.to_string())?;
    lease.save(&document, inputs).map_err(|e| e.to_string())?;
    Ok(())
}
