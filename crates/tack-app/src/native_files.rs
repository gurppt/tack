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
    ExportOriginal,
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
fn capture(
    command: Command,
    _work: &Path,
    cancel: &AtomicBool,
    limit: u64,
    timeout: Duration,
) -> Result<Vec<u8>, AssetError> {
    crate::clipboard::capture(command, cancel, limit, timeout)
}
#[cfg(not(target_os = "linux"))]
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
    pick_with_options(kind, work, cancel, &PickOptions::default())
}
pub fn pick_with_options(
    kind: Picker,
    work: &Path,
    cancel: &AtomicBool,
    options: &PickOptions,
) -> Result<Vec<PathBuf>, AssetError> {
    let directory = usable_directory(options.directory.as_deref());
    let suggested_name = options.suggested_name.clone().or_else(|| match kind {
        Picker::Save => Some("Untitled.tack".into()),
        Picker::ExportKeymap => Some("keymap.tackey".into()),
        _ => None,
    });
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
            Picker::Save | Picker::ExportKeymap | Picker::ExportOriginal => {
                command.args(["--save", "--confirm-overwrite"]);
            }
            Picker::Relink => {
                command.arg("--file-filter=Images | *.png *.jpg *.jpeg *.PNG *.JPG *.JPEG");
            }
            Picker::Open => {
                command.arg("--file-filter=Tack boards | *.tack *.TACK");
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
    let command = {
        let script = work.join("tack-file-picker.ps1");
        tack_storage::create_private_directory(work, true)?;
        std::fs::write(&script, WINDOWS_PICKER)?;
        let mut command = Command::new("powershell.exe");
        command
            .args(["-NoProfile", "-NonInteractive", "-STA", "-File"])
            .arg(script)
            .arg(format!("{kind:?}"))
            .arg(directory.as_deref().unwrap_or(Path::new("")))
            .arg(
                suggested_name
                    .as_deref()
                    .unwrap_or(std::ffi::OsStr::new("")),
            );
        command
    };
    #[cfg(not(any(target_os = "linux", windows)))]
    {
        let _ = (kind, work, cancel, directory, suggested_name);
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
if ($args[0] -eq 'Save' -or $args[0] -eq 'ExportKeymap' -or $args[0] -eq 'ExportOriginal') { $d = New-Object System.Windows.Forms.SaveFileDialog } else { $d = New-Object System.Windows.Forms.OpenFileDialog }
$d.Title = 'Tack'
$d.AddExtension = $false
if ($args[1]) { $d.InitialDirectory = $args[1] }
if ($args[2]) { $d.FileName = $args[2] }
if ($args[0] -eq 'Open' -or $args[0] -eq 'Save') { $d.Filter = 'Tack boards (*.tack)|*.tack' }
if ($args[0] -eq 'ImportKeymap') { $d.Filter = 'Tack keymaps (*.tackey)|*.tackey|Legacy Tack preferences (*.json)|*.json' }
if ($args[0] -eq 'ExportKeymap') { $d.Filter = 'Tack keymaps (*.tackey)|*.tackey' }
if ($args[0] -eq 'Import' -or $args[0] -eq 'Relink') { $d.Filter = 'Images (*.png;*.jpg;*.jpeg)|*.png;*.jpg;*.jpeg' }
if ($args[0] -eq 'Import') { $d.Multiselect = $true }
if ($d.ShowDialog() -ne [System.Windows.Forms.DialogResult]::OK) { exit 1 }
foreach ($p in $d.FileNames) { [Console]::WriteLine($p) }
$d.Dispose()
"#;

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
