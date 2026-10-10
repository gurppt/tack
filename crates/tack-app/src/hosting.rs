//! Explicit owned subprocess lifecycle. No server construction for ordinary boards.
use crate::sharing::Descriptor;
use std::{
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};
use tack_assets::AssetError;
pub struct Hosted {
    child: Option<Child>,
    reaper: std::sync::mpsc::Sender<Child>,
    pub descriptor: Descriptor,
    pub snapshot: PathBuf,
}
impl Drop for Hosted {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            child.stdin.take();
            // The waiter was created before spawning the server, so Drop never
            // loses Child on a thread-spawn failure and never waits on the UI.
            let _ = self.reaper.send(child);
        }
    }
}
impl Hosted {
    pub fn start(
        root: &Path,
        snapshot: &Path,
        d: Descriptor,
        cancel: &AtomicBool,
    ) -> Result<Self, AssetError> {
        if !crate::sharing::owns(root, &d) {
            return Err(
                "This shared copy was hosted elsewhere. Use its invite or inspect offline.".into(),
            );
        }
        let address = d.address()?;
        let mut server = std::env::current_exe()?;
        server.set_file_name(if cfg!(windows) {
            "tack-server.exe"
        } else {
            "tack-server"
        });
        let authority = root.join("hosted").join(&d.board);
        tack_storage::create_private_directory(&authority, true)?;
        let receipt = authority.join("ready");
        let _ = std::fs::remove_file(&receipt);
        let (reaper, closed) = std::sync::mpsc::channel::<Child>();
        std::thread::Builder::new()
            .name("tack-host-close".into())
            .spawn(move || {
                if let Ok(mut child) = closed.recv() {
                    let start = Instant::now();
                    loop {
                        match child.try_wait() {
                            Ok(Some(_)) => break,
                            Err(_) => {
                                let _ = child.kill();
                                let _ = child.wait();
                                break;
                            }
                            Ok(None) => {}
                        }
                        if start.elapsed() > Duration::from_secs(60) {
                            let _ = child.kill();
                            let _ = child.wait();
                            break;
                        }
                        std::thread::sleep(Duration::from_millis(20));
                    }
                }
            })?;
        let mut command = Command::new(server);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let child = command
            .arg("--root")
            .arg(&authority)
            .arg("--listen")
            .arg(format!("0.0.0.0:{}", address.server.port()))
            .arg("--managed-ready")
            .arg(&receipt)
            .arg("--managed-snapshot")
            .arg(snapshot)
            .arg("--managed-board")
            .arg(&d.board)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        let mut host = Self {
            child: Some(child),
            reaper,
            descriptor: d,
            snapshot: snapshot.into(),
        };
        let started = Instant::now();
        loop {
            if cancel.load(Ordering::Relaxed) {
                host.abort();
                return Err("Sharing cancelled".into());
            }
            if let Ok(text) = std::fs::read_to_string(&receipt) {
                if text == "ready" {
                    break;
                }
                host.abort();
                return Err(text.into());
            }
            if let Some(status) = host.child.as_mut().ok_or("host stopped")?.try_wait()? {
                return Err(format!("Could not start sharing ({status}). Another board may already use this computer's sharing address. See Advanced.").into());
            }
            if started.elapsed() > Duration::from_secs(10) {
                host.abort();
                return Err("Sharing startup timed out".into());
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        if !authority
            .join("boards")
            .join(format!("{}.board", host.descriptor.board))
            .exists()
        {
            let client = tack_shared::WireId::new(tack_storage::new_document_id()?.value())?;
            if let Err(e) = tack_shared::publish::publish_board(
                &address.server.to_string(),
                snapshot,
                client,
                cancel,
            ) {
                host.abort();
                return Err(e.into());
            }
        }
        Ok(host)
    }
    pub fn stop(mut self) -> Result<(), AssetError> {
        if let Some(child) = &mut self.child {
            child.stdin.take();
            let start = Instant::now();
            loop {
                if let Some(status) = child.try_wait()? {
                    self.child = None;
                    if !status.success() {
                        return Err("Host stopped; offline copy refresh failed. Server authority remains on this computer.".into());
                    }
                    return Ok(());
                }
                if start.elapsed() > Duration::from_secs(60) {
                    let _ = child.kill();
                    let _ = child.wait();
                    self.child = None;
                    return Err(
                        "Host stopped after snapshot timeout; server authority retained".into(),
                    );
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        }
        Ok(())
    }
    fn abort(&mut self) {
        if let Some(mut c) = self.child.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}
pub fn launch_host(path: &Path, work: &Path, cancel: &AtomicBool) -> Result<bool, AssetError> {
    crate::join_launch::launch_command(
        Command::new(std::env::current_exe()?)
            .arg("open")
            .arg(path)
            .arg("--put-online"),
        work,
        cancel,
        Duration::from_secs(120),
    )
}
pub struct ShareRequest {
    pub document: tack_core::Document,
    pub board: std::sync::Arc<tack_storage::TackFile>,
    pub originals: crate::image_save::Originals,
    pub prepared: std::collections::BTreeMap<tack_core::AssetId, tack_assets::PreparedOverview>,
    pub source_path: PathBuf,
    pub target: PathBuf,
    pub profile: PathBuf,
    pub remote: Option<String>,
}
pub struct ShareReady {
    pub descriptor: Descriptor,
    pub snapshot: PathBuf,
    pub host: Option<Hosted>,
}
impl ShareRequest {
    pub fn run(self, work: &Path, cancel: &AtomicBool) -> Result<Option<ShareReady>, AssetError> {
        if cancel.load(Ordering::Relaxed) {
            return Ok(None);
        }
        if self.target.exists() || crate::sharing::sidecar(&self.target).exists() {
            return Err("Shared copy already exists. Open it and choose Put Online, or choose a new filename.".into());
        }
        let owner = if self.remote.is_none() {
            Some(crate::sharing::host_identity(&self.profile)?)
        } else {
            None
        };
        let address = match &self.remote {
            Some(v) => v.clone(),
            None => format!("{}:7337", crate::sharing_address::local(work, cancel)?),
        };
        let id = tack_storage::new_document_id()?;
        let mut doc = self.document.fork(id);
        let mut revision = doc
            .sources()
            .map(|s| s.revision())
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or("source revision exhausted")?;
        let sources: Vec<_> = doc.sources().cloned().collect();
        for source in sources {
            if let tack_core::SourceLocation::Linked(p) = source.location()
                && !p.is_absolute()
            {
                let path = p.to_native().ok_or("foreign linked path")?;
                let path = self
                    .source_path
                    .parent()
                    .ok_or("source directory")?
                    .join(path);
                doc.apply(tack_core::Command::SetSource(
                    tack_core::Source::from_descriptor(
                        source.id(),
                        tack_core::SourceLocation::Linked(tack_core::LinkedPath::native(&path)?),
                        source.revision(),
                        source.fingerprint(),
                    )?,
                ))?;
                revision = revision.checked_add(1).ok_or("source revision exhausted")?;
            }
        }
        let descriptor = Descriptor {
            version: 1,
            board: tack_shared::WireId::new(id.value())?.to_string(),
            owner,
            invite: format!("tack://{address}/{}", tack_shared::WireId::new(id.value())?),
        };
        descriptor.address()?;
        let lease = tack_storage::BoardLease::acquire_new(&self.target)?;
        let inputs =
            crate::image_save::snapshot_inputs(&self.board, &doc, &self.prepared, &self.originals)?;
        lease.save(&doc, inputs)?;
        descriptor.write_new(&self.target)?;
        drop(lease);
        let host = if let Some(address) = self.remote {
            let client = tack_shared::WireId::new(tack_storage::new_document_id()?.value())?;
            tack_shared::publish::publish_board(&address, &self.target, client, cancel)?;
            None
        } else {
            Some(Hosted::start(
                &self.profile,
                &self.target,
                descriptor.clone(),
                cancel,
            )?)
        };
        Ok(Some(ShareReady {
            descriptor,
            snapshot: self.target,
            host,
        }))
    }
}
