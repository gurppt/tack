//! Explicit integration probe: no CLI probe claims physical Windows validation.
#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mode = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "cancel-open".into());
    if mode == "owner-cancel-pump" || mode == "owner-cancel-block" {
        return owner_probe(mode == "owner-cancel-block");
    }
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
        tack_windows_io::available_bytes(&dir)?
    );
    Ok(())
}
#[cfg(not(windows))]
fn main() {
    println!("Windows-only integration probe");
}

#[cfg(windows)]
fn owner_probe(block_owner: bool) -> Result<(), Box<dyn std::error::Error>> {
    use std::{
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
        time::{Duration, Instant},
    };
    use windows::Win32::UI::Input::KeyboardAndMouse::IsWindowEnabled;
    use windows::{
        Win32::{
            Foundation::HWND,
            UI::WindowsAndMessaging::{
                CW_USEDEFAULT, CreateWindowExW, DestroyWindow, DispatchMessageW, MSG, PM_REMOVE,
                PeekMessageW, SW_SHOW, ShowWindow, TranslateMessage, WINDOW_EX_STYLE,
                WS_OVERLAPPEDWINDOW,
            },
        },
        core::w,
    };
    // SAFETY: built-in STATIC class, terminated static strings and null optional
    // owner/menu/instance/data; window is destroyed on this creating UI thread.
    let hwnd = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("STATIC"),
            w!("Tack owned probe"),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            640,
            480,
            None,
            None,
            None,
            None,
        )?
    };
    // SAFETY: live owned handle, no lifetime transfer.
    unsafe {
        let _ = ShowWindow(hwnd, SW_SHOW);
    }
    let owner = hwnd.0 as usize;
    let stop = Arc::new(AtomicBool::new(false));
    let stop_worker = Arc::clone(&stop);
    let worker = std::thread::spawn(move || {
        tack_windows_dialogs::pick(
            tack_windows_dialogs::Request {
                save: false,
                folder: false,
                multiple: false,
                filters: &[],
                directory: None,
                name: None,
                owner: Some(owner),
                max_files: 1,
            },
            &stop_worker,
        )
    });
    let started = Instant::now();
    let mut cancelled = false;
    while !worker.is_finished() {
        // SAFETY: dispatch only this owned process/thread's messages, on its UI
        // thread. Native modal WM_ENABLE sends must be serviced before joining.
        unsafe {
            let mut msg = MSG::default();
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            if !cancelled && !IsWindowEnabled(hwnd).as_bool() {
                cancelled = true;
                stop.store(true, Ordering::Relaxed);
                println!("owner_disabled=true cancel_requested=true block_owner={block_owner}");
                if block_owner {
                    // Explicit failing control, run only under owned timeout:
                    // reproduce UI teardown joining before pumping WM_ENABLE.
                    let _ = worker.join();
                    return Err("blocking control unexpectedly completed".into());
                }
            }
        }
        if started.elapsed() > Duration::from_secs(8) {
            return Err("owned dialog cancellation deadline".into());
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    let paths = worker.join().map_err(|_| "owned probe worker failed")??;
    // SAFETY: worker/COM finished before inspecting and destroying the owner.
    unsafe {
        if !IsWindowEnabled(hwnd).as_bool() {
            return Err("owner stayed disabled".into());
        }
        DestroyWindow(HWND(owner as *mut _))?;
    }
    if !cancelled || !paths.is_empty() {
        return Err("owned dialog did not exercise cancellation".into());
    }
    println!(
        "owner_enabled=true result=ok elapsed_ms={:.3}",
        started.elapsed().as_secs_f64() * 1000.
    );
    Ok(())
}
