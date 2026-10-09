//! One explicit, cancellable launch using exactly the CLI join canvas/backend.
use crate::shared_address::SharedAddress;
use std::{
    path::Path,
    process::{Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};
use tack_assets::AssetError;
pub fn launch(
    address: &SharedAddress,
    work: &Path,
    cancel: &AtomicBool,
) -> Result<bool, AssetError> {
    std::fs::create_dir_all(work)?;
    let receipt = work.join(format!(
        "join-{:032x}.ready",
        tack_storage::new_document_id()?.value()
    ));
    let mut child = Command::new(std::env::current_exe()?)
        .arg("join")
        .arg(address.server.to_string())
        .arg(address.board.to_string())
        .arg("--join-receipt")
        .arg(&receipt)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    let started = Instant::now();
    let result = (|| {
        loop {
            // Cancellation wins over a readiness receipt in the same iteration.
            if cancel.load(Ordering::Relaxed) {
                return Ok(false);
            }
            if let Ok(bytes) = std::fs::read(&receipt) {
                if bytes == b"joined" {
                    return Ok(true);
                }
                if let Some(reason) = bytes.strip_prefix(b"failed: ") {
                    return Err(String::from_utf8_lossy(reason).into_owned().into());
                }
            }
            if let Some(status) = child.try_wait()? {
                return Err(
                    format!("Shared window stopped ({status}); local board retained").into(),
                );
            }
            if started.elapsed() > Duration::from_secs(15) {
                return Err("Shared join timed out; local board retained".into());
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    })();
    if !matches!(result, Ok(true)) {
        let _ = child.kill();
        let _ = child.wait();
    }
    let _ = std::fs::remove_file(receipt);
    result
}
