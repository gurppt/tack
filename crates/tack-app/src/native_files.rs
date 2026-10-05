//! Optional native helpers, fixed executable/arguments and bounded disk output.
//! Called only from local workers. No helper stays resident while unused.
#[cfg(target_os = "linux")]
use std::fs::File;
use std::{
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
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
}
fn capture(
    mut command: Command,
    work: &Path,
    cancel: &AtomicBool,
    limit: u64,
    timeout: Duration,
) -> Result<Vec<u8>, AssetError> {
    let _ = work;
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| "native file/clipboard helper unavailable on this host")?;
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
            Ok(Some(status)) if !status.success() => {
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
    #[cfg(target_os = "linux")]
    let command = {
        let mut command = Command::new("zenity");
        command.args(["--file-selection", "--title=Tack"]);
        match kind {
            Picker::Import => {
                command.args([
                    "--multiple",
                    "--separator=\n",
                    "--file-filter=Images | *.png *.jpg *.jpeg *.PNG *.JPG *.JPEG",
                ]);
            }
            Picker::Save | Picker::ExportKeymap => {
                command.args(["--save", "--confirm-overwrite"]);
            }
            Picker::Relink => {
                command.arg("--file-filter=Images | *.png *.jpg *.jpeg *.PNG *.JPG *.JPEG");
            }
            Picker::Open => {
                command.arg("--file-filter=Tack boards | *.tack");
            }
            Picker::ImportKeymap => {
                command.arg("--file-filter=Tack preferences | *.json");
            }
        }
        command
    };
    #[cfg(windows)]
    let command = {
        let script = work.join("tack-file-picker.ps1");
        tack_storage::create_private_directory(work, true)?;
        std::fs::write(&script, WINDOWS_PICKER)?;
        let mut command = Command::new("powershell.exe");
        command
            .args(["-NoProfile", "-NonInteractive", "-STA", "-File"])
            .arg(script)
            .arg(format!("{kind:?}"));
        command
    };
    #[cfg(not(any(target_os = "linux", windows)))]
    {
        let _ = (kind, work, cancel);
        return Err("native picker unavailable on this platform; use command-line paths".into());
    }
    #[cfg(any(target_os = "linux", windows))]
    {
        let bytes = capture(command, work, cancel, 64 * 1024, Duration::from_secs(300))?;
        let text = std::str::from_utf8(&bytes)?;
        let paths: Vec<_> = text
            .lines()
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
            .collect();
        if paths.is_empty() || paths.len() > crate::local_import::MAX_IMPORT_FILES {
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
}
#[cfg(windows)]
const WINDOWS_PICKER: &str = r#"[Console]::OutputEncoding = New-Object System.Text.UTF8Encoding($false)
Add-Type -AssemblyName System.Windows.Forms
if ($args[0] -eq 'Save' -or $args[0] -eq 'ExportKeymap') { $d = New-Object System.Windows.Forms.SaveFileDialog } else { $d = New-Object System.Windows.Forms.OpenFileDialog }
$d.Title = 'Tack'
if ($args[0] -eq 'Import') { $d.Multiselect = $true }
if ($d.ShowDialog() -ne [System.Windows.Forms.DialogResult]::OK) { exit 1 }
foreach ($p in $d.FileNames) { [Console]::WriteLine($p) }
$d.Dispose()
"#;

/// Text/reference retrieval is bounded before Rust allocation; no shell interpolation.
pub fn clipboard_text(work: &Path, cancel: &AtomicBool) -> Result<String, AssetError> {
    #[cfg(target_os = "linux")]
    let command = {
        let mut command = if std::env::var_os("WAYLAND_DISPLAY").is_some() {
            Command::new("wl-paste")
        } else {
            Command::new("xclip")
        };
        if std::env::var_os("WAYLAND_DISPLAY").is_some() {
            command.args(["--no-newline", "--type", "text/plain"]);
        } else {
            command.args(["-selection", "clipboard", "-out", "-target", "UTF8_STRING"]);
        }
        command
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
    #[cfg(any(target_os = "linux", windows))]
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

/// Optional PNG clipboard import on Linux. Encoded size is capped before allocation;
/// the existing header parser validates dimensions before any pixel decoding.
pub fn clipboard_image(work: &Path, cancel: &AtomicBool) -> Result<Option<PathBuf>, AssetError> {
    #[cfg(target_os = "linux")]
    {
        let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some();
        let mut types = Command::new(if wayland { "wl-paste" } else { "xclip" });
        if wayland {
            types.arg("--list-types");
        } else {
            types.args(["-selection", "clipboard", "-out", "-target", "TARGETS"]);
        }
        let types = match capture(types, work, cancel, 4096, Duration::from_secs(10)) {
            Ok(b) => b,
            Err(_) => return Ok(None),
        };
        if !std::str::from_utf8(&types)?
            .lines()
            .any(|s| s.trim() == "image/png")
        {
            return Ok(None);
        }
        let mut command = Command::new(if wayland { "wl-paste" } else { "xclip" });
        if wayland {
            command.args(["--no-newline", "--type", "image/png"]);
        } else {
            command.args(["-selection", "clipboard", "-out", "-target", "image/png"]);
        }
        let bytes = capture(
            command,
            work,
            cancel,
            crate::local_import::MAX_ENCODED_IMAGE,
            Duration::from_secs(10),
        )?;
        tack_storage::create_private_directory(work, true)?;
        let path = work.join("clipboard.png");
        match std::fs::symlink_metadata(&path) {
            Ok(m) if m.is_file() => std::fs::remove_file(&path)?,
            Ok(_) => return Err("unsafe clipboard staging file".into()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
        let mut options = File::options();
        options.write(true).create_new(true);
        use std::{io::Write, os::unix::fs::OpenOptionsExt};
        options.mode(0o600);
        let mut file = options.open(&path)?;
        file.write_all(&bytes)?;
        drop(file);
        drop(bytes);
        if let Err(e) = tack_assets::image_metadata(&path) {
            let _ = std::fs::remove_file(&path);
            return Err(e);
        }
        Ok(Some(path))
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (work, cancel);
        Ok(None)
    }
}
