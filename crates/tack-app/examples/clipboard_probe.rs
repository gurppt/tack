//! Native clipboard diagnostic; on demand, no window/selection owner.
fn main() -> Result<(), tack_assets::AssetError> {
    let work = std::env::args_os()
        .nth(1)
        .ok_or("usage: clipboard_probe PRIVATE_WORK [text]")?;
    if std::env::args().nth(2).as_deref() == Some("worker-cancel") {
        use std::sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        };
        use tack_app::local_worker::{LocalUpdate, LocalWorker, Operation};
        let ready = Arc::new(AtomicBool::new(false));
        let notification = Arc::clone(&ready);
        let root = std::path::PathBuf::from(&work);
        let mut worker = LocalWorker::default();
        worker.start(
            Operation::Clipboard {
                work: root.clone(),
                ticket: None,
            },
            move || {
                notification.store(true, Ordering::Relaxed);
            },
        )?;
        let started = std::time::Instant::now();
        while !ready.load(Ordering::Relaxed) {
            if started.elapsed().as_secs() > 12 {
                return Err("worker notification timeout".into());
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let staged_before = root.join("clipboard.png").is_file();
        worker.cancel();
        let mut delivered = 0;
        while worker.active() {
            delivered += worker
                .poll()
                .iter()
                .filter(|u| matches!(u, LocalUpdate::Clipboard { .. }))
                .count();
            if started.elapsed().as_secs() > 14 {
                return Err("cancel cleanup timeout".into());
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        println!(
            "{}",
            serde_json::json!({"kind":"cancelled", "staged_before":staged_before, "staged_after":root.join("clipboard.png").exists(), "delivered":delivered})
        );
        return Ok(());
    }
    let text_only = std::env::args().nth(2).is_some_and(|s| s == "text");
    let result = tack_app::clipboard::read(
        std::path::Path::new(&work),
        &std::sync::atomic::AtomicBool::new(false),
        text_only,
    );
    let value = match result {
        Ok(tack_app::clipboard::Clipboard::Image(path)) => {
            serde_json::json!({"kind":"embedded image", "staged":path, "imported":0})
        }
        Ok(tack_app::clipboard::Clipboard::Text(text)) => {
            serde_json::json!({"kind":"text", "bytes":text.len(), "imported":0})
        }
        Ok(tack_app::clipboard::Clipboard::Files { paths, rejected }) => {
            serde_json::json!({"kind":"local files", "count":paths.len(), "rejected":rejected, "imported":0})
        }
        Err(error) => serde_json::json!({"kind":"error", "error":error.to_string(), "imported":0}),
    };
    println!("{value}");
    Ok(())
}
