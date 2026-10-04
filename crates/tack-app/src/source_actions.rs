//! Explicit user-requested source I/O/process boundary; never called by gestures.
use std::{
    ffi::OsString,
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};
use tack_core::{Document, ObjectId, SourceLocation};
type Error = Box<dyn std::error::Error + Send + Sync>;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceOperation {
    Open,
    Reveal,
    CopyPath,
}
pub struct SourceRequest {
    location: SourceLocation,
    board: PathBuf,
    operation: SourceOperation,
}
impl SourceRequest {
    /// Only resident descriptor lookup here. Resolution and validation run on demand off-thread.
    pub fn new(
        doc: &Document,
        id: ObjectId,
        board: &Path,
        operation: SourceOperation,
    ) -> Result<Self, Error> {
        let o = doc.object(id).ok_or("selected object no longer exists")?;
        let tack_core::ObjectKind::Image(image) = o.kind() else {
            return Err("source actions require one image".into());
        };
        let source = doc
            .asset(image.asset_id())
            .and_then(|a| doc.source(a.source_id()))
            .ok_or("missing image source descriptor")?;
        Ok(Self {
            location: source.location().clone(),
            board: board.to_owned(),
            operation,
        })
    }
    pub fn resolve(&self) -> Result<PathBuf, Error> {
        let SourceLocation::Linked(descriptor) = &self.location else {
            return Err("embedded image has no external source path".into());
        };
        let path = descriptor
            .to_native()
            .ok_or("foreign or unresolved source path; relink is required")?;
        let path = if path.is_absolute() {
            path
        } else {
            let board = std::path::absolute(&self.board)
                .map_err(|_| "board location unavailable for relative source")?;
            board.parent().ok_or("board parent unavailable")?.join(path)
        };
        let path = fs::canonicalize(path).map_err(|_| "linked source is missing or unavailable")?;
        if !path.is_absolute() {
            return Err("source resolution did not produce an absolute path".into());
        }
        let metadata = fs::metadata(&path)?;
        if !metadata.is_file() {
            return Err("linked source is not a regular image file".into());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if metadata.permissions().mode() & 0o111 != 0 {
                return Err("refusing executable source file".into());
            }
        }
        let extension = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if !matches!(extension.as_str(), "png" | "jpg" | "jpeg") {
            return Err("source actions support JPEG/PNG files only".into());
        }
        let mut header = [0u8; 8];
        let read = File::open(&path)?.read(&mut header)?;
        let valid = if extension == "png" {
            read == 8 && header == *b"\x89PNG\r\n\x1a\n"
        } else {
            read >= 3 && header[..3] == [255, 216, 255]
        };
        if !valid {
            return Err("source file does not have supported image content".into());
        }
        Ok(path)
    }
    pub fn invocation(&self) -> Result<SourceInvocation, Error> {
        let path = self.resolve()?;
        match self.operation {
            SourceOperation::Open | SourceOperation::Reveal => {
                let target = if self.operation == SourceOperation::Reveal {
                    path.parent().ok_or("source parent unavailable")?.to_owned()
                } else {
                    path
                };
                #[cfg(target_os = "linux")]
                {
                    Ok(SourceInvocation {
                        executable: "gio".into(),
                        args: vec!["open".into(), target.into_os_string()],
                        stdin: None,
                    })
                }
                #[cfg(windows)]
                {
                    Ok(SourceInvocation {
                        executable: "explorer.exe".into(),
                        args: vec![target.into_os_string()],
                        stdin: None,
                    })
                }
                #[cfg(not(any(target_os = "linux", windows)))]
                {
                    let _ = target;
                    Err("native source opening is unavailable on this platform".into())
                }
            }
            SourceOperation::CopyPath => {
                let text = path.to_str().ok_or(
                    "clipboard paths require valid Unicode; original descriptor is unchanged",
                )?;
                #[cfg(windows)]
                let bytes: Vec<u8> = text.encode_utf16().flat_map(u16::to_le_bytes).collect();
                #[cfg(not(windows))]
                let bytes = text.as_bytes().to_vec();
                if bytes.len() > 4096 {
                    return Err("clipboard path exceeds 4096-byte limit".into());
                }
                #[cfg(target_os = "linux")]
                {
                    let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some();
                    Ok(SourceInvocation {
                        executable: if wayland { "wl-copy" } else { "xclip" }.into(),
                        args: if wayland {
                            vec![]
                        } else {
                            vec!["-selection".into(), "clipboard".into(), "-in".into()]
                        },
                        stdin: Some(bytes),
                    })
                }
                #[cfg(windows)]
                {
                    Ok(SourceInvocation {
                        executable: "clip.exe".into(),
                        args: vec![],
                        stdin: Some(bytes),
                    })
                }
                #[cfg(not(any(target_os = "linux", windows)))]
                {
                    let _ = bytes;
                    Err("clipboard source paths are unavailable on this platform".into())
                }
            }
        }
    }
    pub fn perform(self) -> Result<(), Error> {
        self.invocation()?.run()
    }
}
/// Fixed executable and separate native arguments; document data is never executable text.
#[derive(Debug)]
pub struct SourceInvocation {
    executable: OsString,
    args: Vec<OsString>,
    stdin: Option<Vec<u8>>,
}
impl SourceInvocation {
    pub fn executable(&self) -> &std::ffi::OsStr {
        &self.executable
    }
    pub fn args(&self) -> &[OsString] {
        &self.args
    }
    pub fn stdin_bytes(&self) -> Option<&[u8]> {
        self.stdin.as_deref()
    }
    fn run(self) -> Result<(), Error> {
        let mut child = Command::new(&self.executable)
            .args(&self.args)
            .stdin(if self.stdin.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(
                |_| "native source helper is unavailable (gio / xclip / wl-copy / Explorer / clip)",
            )?;
        if let Some(bytes) = self.stdin {
            let result = child
                .stdin
                .take()
                .ok_or("clipboard stdin unavailable")
                .and_then(|mut pipe| pipe.write_all(&bytes).map_err(|_| "clipboard write failed"));
            if let Err(error) = result {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error.into());
            }
        }
        let start = Instant::now();
        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    return if status.success() {
                        Ok(())
                    } else {
                        Err("native source helper failed".into())
                    };
                }
                Ok(None) if start.elapsed() < Duration::from_secs(10) => {
                    std::thread::sleep(Duration::from_millis(10))
                }
                Ok(None) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err("native source helper timed out".into());
                }
                Err(error) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(error.into());
                }
            }
        }
    }
}
