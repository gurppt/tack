//! Optional native helpers, fixed executable/arguments and bounded disk output.
//! Called only from local workers. No helper stays resident while unused.
use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::AtomicBool,
    time::Duration,
};
use tack_assets::AssetError;
#[derive(Clone, Copy, Debug)]
pub enum Picker {
    Open,
    Import,
    Save,
    Relink,
    ImportKeymap,
    ExportKeymap,
    ExportPreferences,
    ExportProfile,
    ImportProfile,
    ExportOriginal,
    ExportSources,
    ExportSnapshot,
}
/// Resident descriptors only; folder accessibility is checked on the picker worker.
#[derive(Clone, Default)]
pub struct PickOptions {
    pub directory: Option<PathBuf>,
    pub suggested_name: Option<std::ffi::OsString>,
}
/// Called only for explicit operations, never on the event/frame path.
pub fn usable_directory(path: Option<&Path>) -> Option<PathBuf> {
    let path = path?;
    if !path.is_absolute() || tack_core::LinkedPath::native(path).is_err() {
        return None;
    }
    std::fs::read_dir(path).ok().map(|_| path.to_owned())
}
#[cfg(not(target_os = "linux"))]
use std::{io::Read, process::Stdio, sync::atomic::Ordering, time::Instant};
#[cfg(target_os = "linux")]
pub(crate) fn capture(
    command: Command,
    _work: &Path,
    cancel: &AtomicBool,
    limit: u64,
    timeout: Duration,
) -> Result<Vec<u8>, AssetError> {
    crate::clipboard::capture(command, cancel, limit, timeout)
}
#[cfg(not(target_os = "linux"))]
pub(crate) fn capture(
    command: Command,
    work: &Path,
    cancel: &AtomicBool,
    limit: u64,
    timeout: Duration,
) -> Result<Vec<u8>, AssetError> {
    capture_with_cancel(command, work, cancel, limit, timeout, None)
}
#[cfg(not(target_os = "linux"))]
fn capture_with_cancel(
    mut command: Command,
    work: &Path,
    cancel: &AtomicBool,
    limit: u64,
    timeout: Duration,
    cancel_code: Option<i32>,
) -> Result<Vec<u8>, AssetError> {
    let _ = work;
    let diagnostic = tack_storage::diagnostics::Operation::new("filepicker", "native_helper");
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(if tack_storage::diagnostics::enabled() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .spawn()
        .inspect_err(|e| {
            if tack_storage::diagnostics::enabled() {
                diagnostic.note("spawn_error", tack_storage::diagnostics::error_info(e));
            }
        })
        .map_err(|_| "native file/clipboard helper unavailable on this host")?;
    if let Some(stderr) = child.stderr.take()
        && let Err(e) =
            tack_storage::diagnostics::drain_stderr(stderr, "filepicker", diagnostic.id())
    {
        diagnostic.note(
            "stderr_reader_error",
            tack_storage::diagnostics::error_info(&e),
        );
    }
    if tack_storage::diagnostics::enabled() {
        diagnostic.note("spawned",serde_json::json!({"pid":child.id(),"cancel_code":cancel_code,"limit":limit,"timeout_ms":timeout.as_millis() as u64}));
    }
    let mut pipe = child
        .stdout
        .take()
        .ok_or("native helper stdout unavailable")?;
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    let reader = std::thread::Builder::new()
        .name("tack-helper-output".into())
        .spawn(move || {
            let mut bytes = Vec::new();
            let result = (&mut pipe)
                .take(limit + 1)
                .read_to_end(&mut bytes)
                .map(|_| bytes);
            let _ = tx.send(result);
        });
    let reader = match reader {
        Ok(r) => r,
        Err(e) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(e.into());
        }
    };
    let started = Instant::now();
    let mut output = None;
    let result = loop {
        if cancel.load(Ordering::Relaxed) || started.elapsed() > timeout {
            if tack_storage::diagnostics::enabled() {
                diagnostic.note("cancel_or_timeout",serde_json::json!({"cancelled":cancel.load(Ordering::Relaxed),"timed_out":started.elapsed()>timeout}));
            }
            break Err("native operation cancelled or timed out".into());
        }
        if output.is_none() {
            match rx.try_recv() {
                Ok(Ok(bytes)) if bytes.len() as u64 <= limit => output = Some(bytes),
                Ok(Ok(_)) => break Err("native payload exceeds its limit".into()),
                Ok(Err(e)) => break Err(e.into()),
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    break Err("native helper output worker stopped".into());
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
            }
        }
        match child.try_wait() {
            Ok(Some(status)) if cancel_code.is_some() && status.code() == cancel_code => {
                if tack_storage::diagnostics::enabled() {
                    diagnostic.note("cancelled", serde_json::json!({"code":status.code()}));
                }
                break Ok(Vec::new());
            }
            Ok(Some(status)) if !status.success() => {
                if tack_storage::diagnostics::enabled() {
                    diagnostic.note("exit_error",serde_json::json!({"code":status.code(),"native_code_hex":status.code().map(|v|format!("0x{:08X}",v as u32))}));
                }
                break Err("native operation cancelled or unavailable".into());
            }
            Ok(Some(_)) if output.is_some() => {
                break output.take().ok_or_else(|| "native output missing".into());
            }
            Err(e) => break Err(e.into()),
            _ => {}
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let _ = child.kill();
    let _ = child.wait();
    let _ = reader.join();
    result
}

pub fn pick(kind: Picker, work: &Path, cancel: &AtomicBool) -> Result<Vec<PathBuf>, AssetError> {
    pick_with_options(kind, work, cancel, &PickOptions::default())
}
pub fn pick_with_options(
    kind: Picker,
    work: &Path,
    cancel: &AtomicBool,
    options: &PickOptions,
) -> Result<Vec<PathBuf>, AssetError> {
    let _diagnostic = tack_storage::diagnostics::Operation::new("filepicker", "pick");
    if tack_storage::diagnostics::enabled() {
        _diagnostic.note(
            "request",
            serde_json::json!({"kind":format!("{kind:?}"),"sta":cfg!(windows)}),
        );
    }
    let directory = usable_directory(options.directory.as_deref());
    let suggested_name = options.suggested_name.clone().or_else(|| match kind {
        Picker::ExportSnapshot => Some("Tack Snapshot.png".into()),
        Picker::Save => Some("Untitled.tack".into()),
        Picker::ExportProfile => Some("profile.tackprofile".into()),
        Picker::ExportPreferences => Some("preferences.json".into()),
        Picker::ExportKeymap => Some("keymap.tackey".into()),
        _ => None,
    });
    #[cfg(target_os = "linux")]
    let command = {
        let mut command = Command::new("zenity");
        command.args(["--file-selection", "--title=Tack"]);
        match kind {
            Picker::ExportSources => {
                command.arg("--directory");
            }
            Picker::Import => {
                command.args([
                    "--multiple",
                    "--separator=\n",
                    "--file-filter=Images | *.png *.jpg *.jpeg *.PNG *.JPG *.JPEG",
                ]);
            }
            Picker::Save
            | Picker::ExportKeymap
            | Picker::ExportPreferences
            | Picker::ExportProfile
            | Picker::ExportOriginal
            | Picker::ExportSnapshot => {
                command.args(["--save", "--confirm-overwrite"]);
            }
            Picker::Relink => {
                command.arg("--file-filter=Images | *.png *.jpg *.jpeg *.PNG *.JPG *.JPEG");
            }
            Picker::Open => {
                command.arg("--file-filter=Tack boards | *.tack *.TACK");
            }
            Picker::ImportProfile => {
                command.arg("--file-filter=Tack profiles | *.tackprofile *.TACKPROFILE");
            }
            Picker::ImportKeymap => {
                command.args([
                    "--file-filter=Tack keymaps | *.tackey *.TACKEY",
                    "--file-filter=Legacy Tack preferences | *.json *.JSON",
                ]);
            }
        }
        if let Some(name) = &suggested_name {
            let suggestion = directory
                .as_ref()
                .map_or_else(|| PathBuf::from(name), |dir| dir.join(name));
            let mut argument = std::ffi::OsString::from("--filename=");
            argument.push(suggestion);
            command.arg(argument);
        } else if let Some(directory) = &directory {
            let mut argument = std::ffi::OsString::from("--filename=");
            argument.push(directory);
            argument.push(std::path::MAIN_SEPARATOR.to_string());
            command.arg(argument);
        }
        command
    };
    #[cfg(windows)]
    {
        let (save, folder, multiple, filters) = windows_policy(kind);
        let result = tack_windows_dialogs::pick(
            tack_windows_dialogs::Request {
                save,
                folder,
                multiple,
                filters,
                directory: directory.as_deref(),
                name: suggested_name.as_deref(),
                owner: PICKER_PARENT.with(|p| p.get()),
                max_files: crate::local_import::MAX_IMPORT_FILES,
            },
            cancel,
        );
        if tack_storage::diagnostics::enabled() {
            match &result {
                Ok(paths) => _diagnostic.note("native_result", serde_json::json!({"backend":"COM", "count":paths.len(), "cancelled":paths.is_empty()})),
                Err(error) => _diagnostic.note("native_error", serde_json::json!({"backend":"COM", "stage":error.stage,"category":error.category(),"hresult":format!("0x{:08X}",error.code as u32)})),
            }
        }
        let _ = work;
        validate_paths(kind, result?)
    }
    #[cfg(not(any(target_os = "linux", windows)))]
    {
        let _ = (kind, work, cancel, directory, suggested_name);
        return Err("native picker unavailable on this platform; use command-line paths".into());
    }
    #[cfg(target_os = "linux")]
    {
        #[cfg(target_os = "linux")]
        let bytes = crate::clipboard::capture_with_cancel(
            command,
            cancel,
            64 * 1024,
            Duration::from_secs(300),
            Some(1),
        )?;
        let _ = work;
        let text = std::str::from_utf8(&bytes)?;
        let paths: Vec<_> = text
            .lines()
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
            .collect();
        if tack_storage::diagnostics::enabled() {
            _diagnostic.note("selected_paths",serde_json::json!({"count":paths.len(),"paths":paths.iter().take(16).map(|p|tack_storage::diagnostics::path_info(p)).collect::<Vec<_>>()}));
        }
        validate_paths(kind, paths)
    }
}
fn validate_paths(kind: Picker, paths: Vec<PathBuf>) -> Result<Vec<PathBuf>, AssetError> {
    if paths.is_empty() {
        return Ok(paths);
    }
    if paths.len() > crate::local_import::MAX_IMPORT_FILES {
        return Err("picker returned no paths or too many paths".into());
    }
    for path in &paths {
        tack_core::LinkedPath::native(path)?;
        if !path.is_absolute() {
            return Err("picker requires absolute local paths".into());
        }
    }
    if !matches!(kind, Picker::Import) && paths.len() != 1 {
        return Err("picker requires exactly one path".into());
    }
    Ok(paths)
}
#[cfg(windows)]
fn windows_policy(kind: Picker) -> (bool, bool, bool, &'static [(&'static str, &'static str)]) {
    let save = matches!(
        kind,
        Picker::Save
            | Picker::ExportKeymap
            | Picker::ExportPreferences
            | Picker::ExportProfile
            | Picker::ExportOriginal
            | Picker::ExportSnapshot
    );
    let filters: &[(&str, &str)] = match kind {
        Picker::Save | Picker::Open => &[("Tack boards", "*.tack")],
        Picker::Import | Picker::Relink => &[("Images", "*.png;*.jpg;*.jpeg")],
        Picker::ImportKeymap => &[
            ("Tack keymaps", "*.tackey"),
            ("Legacy Tack preferences", "*.json"),
        ],
        Picker::ExportKeymap => &[("Tack keymaps", "*.tackey")],
        Picker::ExportProfile | Picker::ImportProfile => &[("Tack profiles", "*.tackprofile")],
        Picker::ExportPreferences => &[("Tack preferences", "*.json")],
        Picker::ExportSnapshot => &[("PNG image", "*.png")],
        _ => &[],
    };
    (
        save,
        matches!(kind, Picker::ExportSources),
        matches!(kind, Picker::Import),
        filters,
    )
}
#[cfg(windows)]
thread_local! { static PICKER_PARENT: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) }; }
/// Scoped to the explicit worker. The worker retains its parent Arc<Window>.
pub(crate) struct PickerParent {
    #[cfg(windows)]
    previous: Option<usize>,
}
impl PickerParent {
    pub(crate) fn new(parent: Option<&winit::window::Window>) -> Self {
        #[cfg(windows)]
        {
            use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
            let owner =
                parent
                    .and_then(|w| w.window_handle().ok())
                    .and_then(|h| match h.as_raw() {
                        RawWindowHandle::Win32(h) => Some(h.hwnd.get() as usize),
                        _ => None,
                    });
            Self {
                previous: PICKER_PARENT.with(|p| p.replace(owner)),
            }
        }
        #[cfg(not(windows))]
        {
            let _ = parent;
            Self {}
        }
    }
}
impl Drop for PickerParent {
    fn drop(&mut self) {
        #[cfg(windows)]
        PICKER_PARENT.with(|p| p.set(self.previous));
    }
}

/// Text/reference retrieval is bounded before Rust allocation; no shell interpolation.
pub fn clipboard_text(work: &Path, cancel: &AtomicBool) -> Result<String, AssetError> {
    #[cfg(target_os = "linux")]
    return match crate::clipboard::read(work, cancel, true)? {
        crate::clipboard::Clipboard::Text(text) => Ok(text),
        _ => Err("No clipboard text available".into()),
    };
    #[cfg(windows)]
    let command = {
        let mut command = Command::new("powershell.exe");
        command.args(["-NoProfile", "-NonInteractive", "-STA", "-Command", "[Console]::OutputEncoding = New-Object System.Text.UTF8Encoding($false); Add-Type -AssemblyName System.Windows.Forms; [Console]::Write([System.Windows.Forms.Clipboard]::GetText())"]);
        command
    };
    #[cfg(not(any(target_os = "linux", windows)))]
    {
        let _ = (work, cancel);
        return Err("clipboard helper unavailable on this platform".into());
    }
    #[cfg(windows)]
    {
        Ok(String::from_utf8(capture(
            command,
            work,
            cancel,
            64 * 1024,
            Duration::from_secs(10),
        )?)?)
    }
}
pub fn reference_paths(text: &str) -> Result<Vec<PathBuf>, AssetError> {
    if text.len() > 64 * 1024 {
        return Err("clipboard reference payload too large".into());
    }
    let mut paths = Vec::new();
    for line in text
        .lines()
        .map(str::trim)
        .filter(|s| !s.is_empty() && !s.starts_with('#'))
    {
        let path = if let Some(uri) = line.strip_prefix("file://") {
            let uri = if let Some(local) = uri.strip_prefix("localhost/") {
                format!("/{local}")
            } else if uri.starts_with('/') {
                uri.to_owned()
            } else {
                return Err("clipboard URI requires an empty or localhost authority".into());
            };
            let mut decoded = Vec::with_capacity(uri.len());
            let mut cursor = 0;
            let input = uri.as_bytes();
            while cursor < input.len() {
                let byte = if input[cursor] == b'%' {
                    let a = *input.get(cursor + 1).ok_or("truncated URI escape")?;
                    let b = *input.get(cursor + 2).ok_or("truncated URI escape")?;
                    let hex = |b: u8| char::from(b).to_digit(16).ok_or("invalid URI escape");
                    cursor += 3;
                    ((hex(a)? << 4) | hex(b)?) as u8
                } else {
                    let byte = input[cursor];
                    cursor += 1;
                    byte
                };
                if byte == 0 {
                    return Err("clipboard URI contains a NUL".into());
                }
                decoded.push(byte);
                if decoded.len() > tack_core::MAX_SOURCE_PATH_BYTES {
                    return Err("clipboard path too long".into());
                }
            }
            #[cfg(unix)]
            {
                use std::os::unix::ffi::OsStringExt;
                PathBuf::from(std::ffi::OsString::from_vec(decoded))
            }
            #[cfg(not(unix))]
            {
                let mut text = String::from_utf8(decoded)?;
                if text.starts_with('/') && text.as_bytes().get(2) == Some(&b':') {
                    text.remove(0);
                }
                PathBuf::from(text)
            }
        } else {
            PathBuf::from(line)
        };
        if !path.is_absolute() {
            return Err("clipboard references must be absolute local paths".into());
        }
        tack_core::LinkedPath::native(&path)?;
        paths.push(path);
        if paths.len() > crate::local_import::MAX_IMPORT_FILES {
            return Err("too many clipboard references".into());
        }
    }
    if paths.is_empty() {
        return Err("clipboard contains no local image paths".into());
    }
    Ok(paths)
}
