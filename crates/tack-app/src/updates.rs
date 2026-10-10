//! Explicit helper requests on the existing local worker. No HTTPS library linked into UI.
use std::{
    io::Read,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};
use tack_update::{Error, Response};
pub enum Request {
    Apply {
        stage: PathBuf,
        work: PathBuf,
    },
    Check {
        channel: tack_update::Channel,
        work: PathBuf,
    },
    Download {
        manifest: tack_update::Manifest,
        work: PathBuf,
    },
}
pub fn installation() -> Result<PathBuf, Error> {
    std::env::current_exe()?
        .parent()
        .map(Path::to_owned)
        .ok_or_else(|| "Installation directory unavailable".into())
}
pub fn helper(directory: &Path) -> PathBuf {
    directory.join(if cfg!(windows) {
        "tack-updater.exe"
    } else {
        "tack-updater"
    })
}
impl Request {
    pub fn run(self, cancel: &AtomicBool) -> Result<Response, Error> {
        let install = installation()?;
        let (work, args) = match self {
            Self::Apply { stage, work } => {
                std::fs::create_dir_all(&work)?;
                let runner = work.join(format!(
                    "trusted-updater-{}{}",
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)?
                        .as_nanos(),
                    if cfg!(windows) { ".exe" } else { "" }
                ));
                let mut out = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&runner)?;
                let mut source = std::fs::File::open(helper(&install))?;
                if source.metadata()?.len() > 32 * 1024 * 1024 {
                    return Err("Installed helper size limit".into());
                }
                std::io::copy(&mut source, &mut out)?;
                out.sync_all()?;
                drop(out);
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    std::fs::set_permissions(&runner, std::fs::Permissions::from_mode(0o700))?;
                }
                if cancel.load(Ordering::Relaxed) {
                    let _ = std::fs::remove_file(&runner);
                    return Err("Update cancelled".into());
                }
                std::process::Command::new(runner)
                    .arg("apply")
                    .arg(&stage)
                    .arg(&install)
                    .arg(std::process::id().to_string())
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::fs::File::create(work.join("apply.log"))?)
                    .spawn()?;
                return Ok(Response::Applying);
            }
            Self::Check { channel, work } => {
                let channel = serde_json::to_value(channel)?
                    .as_str()
                    .ok_or("Update channel")?
                    .to_owned();
                (
                    work,
                    vec![
                        "check".into(),
                        channel.into(),
                        env!("CARGO_PKG_VERSION").into(),
                    ],
                )
            }
            Self::Download { manifest, work } => {
                manifest.validate()?;
                std::fs::create_dir_all(&work)?;
                let path = work.join("manifest.json");
                std::fs::write(&path, serde_json::to_vec(&manifest)?)?;
                (
                    work,
                    vec![
                        "stage".into(),
                        path.into_os_string(),
                        install.clone().into_os_string(),
                    ],
                )
            }
        };
        std::fs::create_dir_all(&work)?;
        let output = work.join("response.json");
        let error = work.join("error.txt");
        let mut child = std::process::Command::new(helper(&install))
            .args(args)
            .stdin(std::process::Stdio::null())
            .stdout(std::fs::File::create(&output)?)
            .stderr(std::fs::File::create(&error)?)
            .spawn()
            .map_err(|_| "Update helper unavailable. Download the portable package manually.")?;
        let deadline = Instant::now() + Duration::from_secs(130);
        loop {
            if cancel.load(Ordering::Relaxed) || Instant::now() > deadline {
                let _ = child.kill();
                let _ = child.wait();
                return Err("Update cancelled or timed out; board retained".into());
            }
            if let Some(status) = child.try_wait()? {
                if !status.success() {
                    return Err(read(&error, 4096)?.into());
                }
                let bytes = read(&output, tack_update::MAX_MANIFEST)?;
                return Ok(serde_json::from_str(&bytes)?);
            }
            std::thread::sleep(Duration::from_millis(40));
        }
    }
}
fn read(path: &Path, limit: usize) -> Result<String, Error> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        return Err("Update helper response limit".into());
    }
    Ok(String::from_utf8(bytes)?)
}
