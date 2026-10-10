//! File transaction restricted to portable application assets. Editable icons are seeded only.
use std::{
    path::Path,
    time::{Duration, Instant},
};
use tack_update::{Error, Manifest};
fn wait_for_exit(pid: u32) -> Result<(), Error> {
    if pid == 0 || pid == std::process::id() {
        return Err("Invalid application PID".into());
    }
    let deadline = Instant::now() + Duration::from_secs(90);
    loop {
        #[cfg(target_os = "linux")]
        let alive = Path::new("/proc").join(pid.to_string()).exists();
        #[cfg(windows)]
        let alive = {
            let out = std::process::Command::new("tasklist")
                .args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"])
                .output()?;
            if !out.status.success() {
                return Err("Cannot determine application exit".into());
            }
            String::from_utf8_lossy(&out.stdout)
                .lines()
                .any(|s| s.split(',').nth(1) == Some(format!("\"{pid}\"").as_str()))
        };
        #[cfg(not(any(target_os = "linux", windows)))]
        let alive = false;
        if !alive {
            return Ok(());
        }
        if Instant::now() > deadline {
            return Err("Application did not exit; update retained in staging".into());
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}
pub fn apply(stage: &Path, install: &Path, pid: u32) -> Result<(), Error> {
    let install = install.canonicalize()?;
    let stage = stage.canonicalize()?;
    let manifest = Manifest::parse(&tack_update::read_small(
        &stage.join("manifest.json"),
        tack_update::MAX_MANIFEST,
    )?)?;
    if stage.parent() != Some(install.as_path())
        || stage.file_name().and_then(|v| v.to_str())
            != Some(format!(".tack-update-{}", manifest.version).as_str())
    {
        return Err("Update stage is outside installation".into());
    }
    tack_update::verify(
        &stage.join("package.zip"),
        manifest.asset(tack_update::platform()?)?,
    )?;
    crate::package::writable(&install)?;
    // Re-extract the verified archive before replacement; never trust a mutated staged file.
    let verified = stage.join("verified");
    crate::package::clear_verified(&stage, &manifest)?;
    let all = crate::package::extract(&stage.join("package.zip"), &verified)?;
    crate::package::identity(&verified, &manifest)?;
    wait_for_exit(pid)?;
    let files: Vec<_> = all
        .into_iter()
        .filter(|p| {
            !(p.starts_with("gfx/icons") || p.starts_with("gfx/cursors"))
                || !install.join(p).exists()
        })
        .collect();
    let journal = crate::transaction::prepare(&install, &files)?;
    let exe = install.join(if cfg!(windows) { "tack.exe" } else { "tack" });
    let result = crate::transaction::replace(&install, &verified, &journal).and_then(|_| {
        std::process::Command::new(&exe)
            .current_dir(&install)
            .spawn()?;
        Ok(())
    });
    if let Err(error) = result {
        crate::transaction::rollback(&install)?;
        let _ = std::process::Command::new(&exe)
            .current_dir(&install)
            .spawn();
        return Err(error);
    }
    crate::transaction::commit(&install, journal)?;
    crate::package::cleanup(&stage, &manifest)?;
    Ok(())
}
