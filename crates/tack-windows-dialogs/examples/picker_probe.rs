//! Explicit integration probe: no CLI probe claims physical Windows validation.
#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mode = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "cancel-open".into());
    if mode == "repeat-cancel" {
        for _ in 0..3 {
            exercise("cancel-open")?;
        }
        return Ok(());
    }
    exercise(&mode)
}
#[cfg(windows)]
fn exercise(mode: &str) -> Result<(), Box<dyn std::error::Error>> {
    use std::{
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
        time::{Duration, Instant},
    };
    use tack_windows_dialogs::{Request, pick};
    let stop = Arc::new(AtomicBool::new(mode == "pre-cancel"));
    let stopper = if mode.starts_with("cancel-") {
        let stop = Arc::clone(&stop);
        Some(std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(700));
            stop.store(true, Ordering::Relaxed);
        }))
    } else {
        None
    };
    let started = Instant::now();
    let name = std::ffi::OsStr::new("essai é 東京.tack");
    let dir = std::env::current_dir()?;
    let result = pick(
        Request {
            save: mode == "cancel-save",
            folder: mode == "cancel-folder",
            multiple: mode == "cancel-multiple",
            filters: &[("Tack boards", "*.tack")],
            directory: Some(&dir),
            name: Some(name),
            owner: (mode == "invalid-owner").then_some(1),
            max_files: 4096,
        },
        &stop,
    );
    if let Some(stopper) = stopper {
        stopper.join().map_err(|_| "probe stopper failed")?;
    }
    match result {
        Ok(paths) => {
            println!(
                "result=ok count={} elapsed_ms={:.3}",
                paths.len(),
                started.elapsed().as_secs_f64() * 1000.
            );
            if !paths.is_empty() {
                return Err("unexpected paths in cancellation probe".into());
            }
        }
        Err(e) if mode == "invalid-owner" && e.code as u32 == 0x80070006 => println!(
            "result=expected-error stage={} hresult=0x{:08X}",
            e.stage, e.code as u32
        ),
        Err(e) => return Err(e.into()),
    }
    println!(
        "available_disk_bytes={}",
        tack_windows_dialogs::available_bytes(&dir)?
    );
    Ok(())
}
#[cfg(not(windows))]
fn main() {
    println!("Windows-only integration probe");
}
