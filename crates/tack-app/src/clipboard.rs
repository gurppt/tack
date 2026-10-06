//! On-demand clipboard normalization. No retained selection owner or watcher.
use std::{
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};
use tack_assets::AssetError;
pub enum Clipboard {
    Text(String),
    Image(PathBuf),
    Files {
        paths: Vec<PathBuf>,
        rejected: usize,
    },
}
/// Deterministic MIME priority, with editor paste restricted to text.
pub fn choose_mime(types: &str, text_only: bool) -> Option<&'static str> {
    let priority: &[&str] = if text_only {
        &[
            "text/plain;charset=utf-8",
            "UTF8_STRING",
            "text/plain",
            "STRING",
        ]
    } else {
        &[
            "text/uri-list",
            "x-special/gnome-copied-files",
            "image/png",
            "image/jpeg",
            "text/plain;charset=utf-8",
            "UTF8_STRING",
            "text/plain",
            "STRING",
        ]
    };
    priority
        .iter()
        .copied()
        .find(|mime| types.lines().any(|t| t.trim() == *mime))
}
pub fn references(text: &str) -> Result<(Vec<PathBuf>, usize), AssetError> {
    if text.len() > 64 * 1024 {
        return Err("Clipboard file list exceeds 64 KiB".into());
    }
    let mut paths = Vec::new();
    let mut rejected = 0;
    for line in text
        .lines()
        .map(str::trim)
        .filter(|s| !s.is_empty() && !s.starts_with('#'))
    {
        if line == "copy" || line == "cut" {
            continue;
        }
        if paths.len() + rejected >= crate::local_import::MAX_IMPORT_FILES {
            return Err("Clipboard contains too many file references".into());
        }
        match crate::native_files::reference_paths(line) {
            Ok(mut p) => paths.append(&mut p),
            Err(_) => rejected += 1,
        }
    }
    if paths.is_empty() {
        return Err(
            "No supported local clipboard files; remote or invalid references were rejected".into(),
        );
    }
    Ok((paths, rejected))
}
/// Plain text stays text unless it explicitly names file URIs or an existing
/// absolute PNG/JPEG path. Checking the legacy path form happens on the worker.
pub fn normalize_text(text: String, text_only: bool) -> Result<Clipboard, AssetError> {
    let file_reference = !text_only
        && text.lines().next().is_some_and(|line| {
            let line = line.trim();
            let path = Path::new(line);
            line.starts_with("file://")
                || path.is_absolute()
                    && path
                        .extension()
                        .and_then(|s| s.to_str())
                        .is_some_and(|ext| {
                            ["png", "jpg", "jpeg"]
                                .iter()
                                .any(|s| ext.eq_ignore_ascii_case(s))
                        })
                    && path.is_file()
        });
    if file_reference {
        let (paths, rejected) = references(&text)?;
        Ok(Clipboard::Files { paths, rejected })
    } else {
        Ok(Clipboard::Text(text))
    }
}
#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    use std::{
        fs::File,
        io::{Read, Write},
        os::{
            fd::AsRawFd,
            unix::{fs::OpenOptionsExt, process::CommandExt},
        },
        process::{Command, Stdio},
        sync::atomic::Ordering,
        time::{Duration, Instant},
    };
    /// Nonblocking pipe reads enforce cancellation even when a descendant holds stdout.
    pub(crate) fn capture(
        mut command: Command,
        cancel: &AtomicBool,
        limit: u64,
        timeout: Duration,
    ) -> Result<Vec<u8>, AssetError> {
        let mut child = command
            .process_group(0)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let result = (|| -> Result<Vec<u8>, AssetError> {
            let pipe = child
                .stdout
                .take()
                .ok_or("Clipboard helper has no output")?;
            // Linux O_NONBLOCK. Open our own pipe descriptor, never a user path.
            let mut pipe = File::options()
                .read(true)
                .custom_flags(0o4000)
                .open(format!("/proc/self/fd/{}", pipe.as_raw_fd()))?;
            let started = Instant::now();
            let mut bytes = Vec::new();
            let mut chunk = [0; 8192];
            let mut eof = false;
            loop {
                if cancel.load(Ordering::Relaxed) {
                    return Err("Native helper operation cancelled".into());
                }
                if started.elapsed() > timeout {
                    return Err("Native helper timed out; retry the operation".into());
                }
                if !eof {
                    let remaining = (limit + 1)
                        .saturating_sub(bytes.len() as u64)
                        .min(chunk.len() as u64) as usize;
                    match pipe.read(&mut chunk[..remaining]) {
                        Ok(0) => eof = true,
                        Ok(n) => {
                            bytes.extend_from_slice(&chunk[..n]);
                            if bytes.len() as u64 > limit {
                                return Err("Clipboard payload exceeds its size limit".into());
                            }
                            continue;
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                        Err(e) => return Err(e.into()),
                    }
                }
                match child.try_wait()? {
                    Some(status) if !status.success() => {
                        return Err("Native helper could not complete the operation; retry".into());
                    }
                    Some(_) if eof => return Ok(bytes),
                    _ => {}
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        })();
        // Only the newly owned process group; never a clipboard selection owner.
        let _ = Command::new("/bin/kill")
            .args(["-KILL", "--", &format!("-{}", child.id())])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        let _ = child.kill();
        let _ = child.wait();
        result
    }
    #[derive(Clone, Copy)]
    enum Backend {
        Xclip,
        Xsel,
        Wayland,
    }
    impl Backend {
        fn helper(self) -> &'static str {
            match self {
                Self::Xclip => "xclip",
                Self::Xsel => "xsel",
                Self::Wayland => "wl-paste",
            }
        }
        fn request(self, mime: &str) -> Command {
            let mut command = Command::new(self.helper());
            match self {
                Self::Xclip => {
                    command.args(["-selection", "clipboard", "-out", "-target", mime]);
                }
                Self::Xsel => {
                    command.args(["--clipboard", "--output"]);
                }
                Self::Wayland => {
                    if mime == "TARGETS" {
                        command.arg("--list-types");
                    } else {
                        command.args(["--no-newline", "--type", mime]);
                    }
                }
            }
            command
        }
    }
    fn available(helper: &str) -> bool {
        use std::os::unix::fs::PermissionsExt;
        std::env::var_os("PATH").is_some_and(|path| {
            std::env::split_paths(&path).any(|dir| {
                std::fs::metadata(dir.join(helper))
                    .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
            })
        })
    }
    fn backend() -> Result<Backend, AssetError> {
        let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some();
        let x11 = std::env::var_os("DISPLAY").is_some();
        let native_wayland =
            wayland && std::env::var("XDG_SESSION_TYPE").is_ok_and(|s| s == "wayland");
        if native_wayland && available("wl-paste") {
            return Ok(Backend::Wayland);
        }
        if x11 && available("xclip") {
            return Ok(Backend::Xclip);
        }
        if wayland && available("wl-paste") {
            return Ok(Backend::Wayland);
        }
        if x11 && available("xsel") {
            return Ok(Backend::Xsel);
        }
        if !wayland && !x11 {
            return Err(
                "Clipboard backend unavailable: start Tack in an X11 or Wayland desktop session"
                    .into(),
            );
        }
        Err(if native_wayland || wayland && !x11 {
            "Clipboard helper missing: install wl-clipboard (wl-paste)"
        } else {
            "Clipboard helper missing: install xclip for images/files, or xsel for text"
        }
        .into())
    }
    pub(super) fn read(
        work: &Path,
        cancel: &AtomicBool,
        text_only: bool,
    ) -> Result<Clipboard, AssetError> {
        let backend = backend()?;
        let types = if matches!(backend, Backend::Xsel) {
            "UTF8_STRING".into()
        } else {
            String::from_utf8(capture(
                backend.request("TARGETS"),
                cancel,
                4096,
                Duration::from_secs(3),
            )?)?
        };
        let chosen = choose_mime(&types, text_only);
        eprintln!(
            "clipboard backend={} advertised={:?} chosen={}",
            backend.helper(),
            types.lines().collect::<Vec<_>>(),
            chosen.unwrap_or("none")
        );
        let mime = chosen.ok_or(
            "Unsupported clipboard format: copy local image files, a PNG/JPEG screenshot, or text",
        )?;
        let image = mime.starts_with("image/");
        let bytes = capture(
            backend.request(mime),
            cancel,
            if image {
                crate::local_import::MAX_ENCODED_IMAGE
            } else {
                64 * 1024
            },
            Duration::from_secs(10),
        )?;
        // Format/provenance only; never log clipboard text, URI paths or image bytes.
        eprintln!(
            "clipboard backend={} advertised={:?} chosen={} bytes={}",
            backend.helper(),
            types.lines().collect::<Vec<_>>(),
            mime,
            bytes.len()
        );
        if bytes.is_empty() {
            return Err("No supported clipboard content; copy again".into());
        }
        if image {
            return stage_image(work, &bytes, mime).map(Clipboard::Image);
        }
        let text = String::from_utf8(bytes)?;
        if mime == "text/uri-list" || mime == "x-special/gnome-copied-files" {
            let (paths, rejected) = references(&text)?;
            return Ok(Clipboard::Files { paths, rejected });
        }
        normalize_text(text, text_only)
    }
    fn stage_image(work: &Path, bytes: &[u8], mime: &str) -> Result<PathBuf, AssetError> {
        tack_storage::create_private_directory(work, true)?;
        let path = work.join(if mime == "image/png" {
            "clipboard.png"
        } else {
            "clipboard.jpg"
        });
        match std::fs::symlink_metadata(&path) {
            Ok(m) if m.is_file() => std::fs::remove_file(&path)?,
            Ok(_) => return Err("Unsafe clipboard staging file".into()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
        let mut file = File::options()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)?;
        file.write_all(bytes)?;
        drop(file);
        if let Err(e) = tack_assets::image_metadata(&path) {
            let _ = std::fs::remove_file(&path);
            return Err(e);
        }
        Ok(path)
    }
}
#[cfg(target_os = "linux")]
pub(crate) use linux::capture;
pub fn read(work: &Path, cancel: &AtomicBool, text_only: bool) -> Result<Clipboard, AssetError> {
    #[cfg(target_os = "linux")]
    {
        linux::read(work, cancel, text_only)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let text = crate::native_files::clipboard_text(work, cancel)?;
        normalize_text(text, text_only)
    }
}

#[cfg(all(test, target_os = "linux"))]
mod capture_tests {
    use super::*;
    use std::{
        process::Command,
        time::{Duration, Instant},
    };
    #[test]
    fn active_capture_has_size_and_timeout_bounds_without_reader_thread() -> Result<(), AssetError>
    {
        let stop = AtomicBool::new(false);
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "printf '123456'"]);
        assert!(capture(command, &stop, 5, Duration::from_secs(1)).is_err());
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "printf '12345'"]);
        assert_eq!(
            capture(command, &stop, 5, Duration::from_secs(1))?,
            b"12345"
        );
        Ok(())
    }
    #[test]
    fn descendant_held_stdout_cannot_block_timeout_or_cancellation() {
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "sleep 10 & exit 0"]);
        let started = Instant::now();
        assert!(
            capture(
                command,
                &AtomicBool::new(false),
                64,
                Duration::from_millis(150)
            )
            .is_err()
        );
        assert!(started.elapsed() < Duration::from_secs(2));
        let started = Instant::now();
        assert!(
            capture(
                Command::new("/bin/sleep"),
                &AtomicBool::new(true),
                64,
                Duration::from_secs(10)
            )
            .is_err()
        );
        assert!(started.elapsed() < Duration::from_secs(2));
    }
}
