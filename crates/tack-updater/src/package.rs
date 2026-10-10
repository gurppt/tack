//! Fresh private staging only; no user-data paths or links in portable archives.
use std::{
    io::{Read, Seek, SeekFrom, Write},
    path::{Component, Path, PathBuf},
};
use tack_update::{Error, MAX_UNPACKED, Manifest, Response};
pub const MAX_FILES: usize = 256;
pub fn allowed(path: &Path) -> bool {
    let Some(s) = path.to_str() else {
        return false;
    };
    if s.split('/').any(|part| {
        let stem = part.split('.').next().unwrap_or("").to_ascii_uppercase();
        part.ends_with(['.', ' '])
            || matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
            || (stem.len() == 4
                && (stem.starts_with("COM") || stem.starts_with("LPT"))
                && stem.as_bytes()[3].is_ascii_digit())
    }) || s.contains('\\')
        || s.contains(':')
        || path
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return false;
    }
    matches!(
        s,
        "tack"
            | "tack.exe"
            | "tack-server"
            | "tack-server.exe"
            | "tack-updater"
            | "tack-updater.exe"
            | "tack-jpeg-decoder"
            | "tack-jpeg-decoder.exe"
            | "tack-about.png"
            | "tack-about-logo.png"
            | "BUILD.json"
            | "README.txt"
            | "LISEZ-MOI.txt"
            | "lib/libXi.so.6"
    ) || (s.starts_with("tack-icon")
        && path.components().count() == 1
        && (s.ends_with(".png") || s.ends_with(".ico")))
        || (s.starts_with("gfx/cursors/") && s.ends_with(".png") && path.components().count() == 3)
        || (s.starts_with("gfx/icons/") && s.ends_with(".png") && path.components().count() <= 4)
        || (s.starts_with("LICENSES/") && path.components().count() == 2)
}
pub fn writable(install: &Path) -> Result<(), Error> {
    let probe = install.join(format!(".tack-update-write-check-{}", std::process::id()));
    let f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)
        .map_err(
            |_| "Update unavailable for this installation. Download the new package manually.",
        )?;
    drop(f);
    std::fs::remove_file(probe)?;
    Ok(())
}
pub fn extract(archive: &Path, destination: &Path) -> Result<Vec<PathBuf>, Error> {
    std::fs::create_dir(destination)?;
    let mut file = std::fs::File::open(archive)?;
    let len = file.metadata()?.len();
    if len > tack_update::MAX_ARCHIVE {
        return Err("Update archive size limit".into());
    }
    let tail_len = len.min(65557);
    file.seek(SeekFrom::End(-(tail_len as i64)))?;
    let mut tail = vec![0; tail_len as usize];
    file.read_exact(&mut tail)?;
    let end = (0..tail.len().saturating_sub(21))
        .rev()
        .find(|i| {
            tail[*i..].starts_with(b"PK\x05\x06")
                && *i + 22 + usize::from(u16::from_le_bytes([tail[*i + 20], tail[*i + 21]]))
                    == tail.len()
        })
        .ok_or("Missing ZIP directory")?;
    let count = u16::from_le_bytes([tail[end + 10], tail[end + 11]]) as usize;
    let directory = u32::from_le_bytes(tail[end + 12..end + 16].try_into()?);
    if count > MAX_FILES || directory > 128 * 1024 || tail[end + 4..end + 8] != [0; 4] {
        return Err("ZIP directory/entry count bound or multi-disk ZIP".into());
    }
    file.rewind()?;
    let mut zip = zip::ZipArchive::new(file)?;
    if zip.len() > MAX_FILES {
        return Err("Update file count exceeded".into());
    }
    let mut files = Vec::new();
    let mut total = 0u64;
    let mut names = std::collections::BTreeSet::new();
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        if entry.is_dir() {
            continue;
        }
        let path = entry.enclosed_name().ok_or("Unsafe archive path")?;
        if !entry.is_file()
            || entry
                .unix_mode()
                .is_some_and(|m| m & 0o170000 != 0 && m & 0o170000 != 0o100000)
        {
            return Err("Update contains a link or special file".into());
        }
        let relative = path.components().skip(1).collect::<PathBuf>();
        if !allowed(&relative) || !names.insert(relative.to_string_lossy().to_lowercase()) {
            return Err("Unknown or duplicate portable asset".into());
        }
        total = total.checked_add(entry.size()).ok_or("Update overflow")?;
        if total > MAX_UNPACKED {
            return Err("Unpacked update exceeds 256 MiB".into());
        }
        let target = destination.join(&relative);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut out = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)?;
        let expected = entry.size();
        let size = std::io::copy(&mut entry.by_ref().take(expected + 1), &mut out)?;
        if size != entry.size() {
            return Err("Archive entry size mismatch".into());
        }
        out.flush()?;
        out.sync_all()?;
        #[cfg(unix)]
        if relative.components().count() == 1
            && matches!(
                relative.to_str(),
                Some("tack" | "tack-server" | "tack-updater" | "tack-jpeg-decoder")
            )
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o755))?;
        }
        files.push(relative);
    }
    Ok(files)
}
pub fn identity(package: &Path, manifest: &Manifest) -> Result<(), Error> {
    let bytes = tack_update::read_small(&package.join("BUILD.json"), 32 * 1024)?;
    if bytes.len() > 32 * 1024 {
        return Err("Package identity too large".into());
    }
    let record: serde_json::Value = serde_json::from_slice(&bytes)?;
    if record.get("commit").and_then(|v| v.as_str()) != Some(&manifest.git_sha)
        || record.get("version").and_then(|v| v.as_str()) != Some(&manifest.version)
        || record.get("channel").and_then(|v| v.as_str())
            != Some(match manifest.channel {
                tack_update::Channel::Dev => "dev",
                tack_update::Channel::Stable => "stable",
            })
        || record.get("protocol_major").and_then(|v| v.as_u64())
            != Some(u64::from(manifest.protocol_major))
    {
        return Err("Package version/commit mismatch".into());
    }
    let ext = if cfg!(windows) { ".exe" } else { "" };
    for n in ["tack", "tack-server", "tack-updater", "tack-jpeg-decoder"] {
        if !package.join(format!("{n}{ext}")).is_file() {
            return Err("Required package executable missing".into());
        }
    }
    for n in [
        "tack-about.png",
        "tack-about-logo.png",
        "gfx/icons/pointer.png",
    ] {
        if !package.join(n).is_file() {
            return Err("Required package artwork missing".into());
        }
    }
    Ok(())
}
pub fn stage(manifest: &Manifest, install: &Path) -> Result<Response, Error> {
    manifest.validate()?;
    let install = install.canonicalize()?;
    writable(&install)?;
    let asset = manifest.asset(tack_update::platform()?)?;
    let directory = install.join(format!(".tack-update-{}", manifest.version));
    if directory.exists() {
        cleanup(&directory, manifest)?;
    }
    std::fs::create_dir(&directory).map_err(|_|"An update staging directory already exists; retain it for interrupted-update inspection")?;
    std::fs::write(
        directory.join("manifest.json"),
        serde_json::to_vec(manifest)?,
    )?;
    let result = (|| {
        let archive = directory.join("package.zip");
        crate::network::download(&manifest.url(&asset.name), &archive, asset.size)?;
        tack_update::verify(&archive, asset)?;
        extract(&archive, &directory.join("package"))?;
        identity(&directory.join("package"), manifest)?;
        std::fs::write(
            directory.join("manifest.json"),
            serde_json::to_vec(manifest)?,
        )?;
        Ok(Response::Staged {
            directory: directory.clone(),
            manifest: manifest.clone(),
        })
    })();
    if result.is_err() {
        let _ = std::fs::remove_dir_all(&directory);
    }
    result
}

/// Remove only a recognized interrupted stage, after enumerating every bounded path.
fn owned_files(directory: &Path, expected: &Manifest) -> Result<Vec<PathBuf>, Error> {
    crate::transaction::reject_links(
        directory.parent().ok_or("Stage parent")?,
        Path::new(directory.file_name().ok_or("Stage name")?),
    )?;
    crate::transaction::reject_links(directory, Path::new("manifest.json"))?;
    let manifest = Manifest::parse(&tack_update::read_small(
        &directory.join("manifest.json"),
        tack_update::MAX_MANIFEST,
    )?)?;
    if &manifest != expected {
        return Err("Unknown update stage retained".into());
    }
    fn walk(root: &Path, path: &Path, files: &mut Vec<PathBuf>) -> Result<(), Error> {
        for e in std::fs::read_dir(path)? {
            let e = e?;
            let p = e.path();
            let relative = p.strip_prefix(root)?;
            if e.file_type()?.is_symlink() {
                return Err("Unknown stage symlink retained".into());
            }
            if files.len() > 512 {
                return Err("Stage file count limit".into());
            }
            if e.file_type()?.is_dir() {
                if matches!(
                    relative.to_str(),
                    Some(
                        "package"
                            | "verified"
                            | "package/gfx"
                            | "verified/gfx"
                            | "package/gfx/icons"
                            | "verified/gfx/icons"
                            | "package/gfx/cursors"
                            | "verified/gfx/cursors"
                            | "package/gfx/icons/work_icons"
                            | "verified/gfx/icons/work_icons"
                            | "package/lib"
                            | "verified/lib"
                            | "package/LICENSES"
                            | "verified/LICENSES"
                    )
                ) {
                    walk(root, &p, files)?;
                } else {
                    return Err("Unknown stage directory retained".into());
                }
            } else {
                let known = matches!(
                    relative.to_str(),
                    Some("manifest.json" | "package.zip" | "apply.log")
                ) || relative
                    .strip_prefix("package")
                    .or_else(|_| relative.strip_prefix("verified"))
                    .is_ok_and(allowed);
                if !known {
                    return Err("Unknown stage file retained".into());
                }
                files.push(p);
            }
        }
        Ok(())
    }
    let mut files = Vec::new();
    walk(directory, directory, &mut files)?;
    Ok(files)
}
fn empty(p: &Path) -> Result<(), Error> {
    for e in std::fs::read_dir(p)? {
        let e = e?;
        if e.file_type()?.is_dir() {
            empty(&e.path())?;
        }
    }
    std::fs::remove_dir(p)?;
    Ok(())
}
pub fn cleanup(directory: &Path, expected: &Manifest) -> Result<(), Error> {
    for p in owned_files(directory, expected)? {
        std::fs::remove_file(p)?;
    }
    empty(directory)
}
/// An interrupted extraction can be regenerated from the still-verified archive.
pub fn clear_verified(directory: &Path, expected: &Manifest) -> Result<(), Error> {
    let verified = directory.join("verified");
    let files = owned_files(directory, expected)?;
    if verified.exists() {
        for p in files.into_iter().filter(|p| p.starts_with(&verified)) {
            std::fs::remove_file(p)?;
        }
        empty(&verified)?;
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::Temp;
    fn archive(path: &Path, names: &[&str]) -> Result<(), Error> {
        let mut zip = zip::ZipWriter::new(std::fs::File::create(path)?);
        for name in names {
            zip.start_file(
                *name,
                zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Stored),
            )?;
            zip.write_all(b"fixture")?;
        }
        zip.finish()?;
        Ok(())
    }
    #[test]
    fn only_portable_files_can_be_extracted() -> Result<(), Error> {
        let t = Temp::new()?;
        let zip = t.0.join("package.zip");
        archive(&zip, &["root/tack", "root/gfx/cursors/cursor_pointer.png"])?;
        let files = extract(&zip, &t.0.join("good"))?;
        assert_eq!(files.len(), 2);
        assert_eq!(std::fs::read(t.0.join("good/tack"))?, b"fixture");
        Ok(())
    }
    #[test]
    fn traversal_and_user_data_are_refused() -> Result<(), Error> {
        let t = Temp::new()?;
        for (i, name) in [
            "root/../../outside",
            "root/profile/preferences.json",
            "root/artist.tack",
            "root/gfx/icons/../../bad",
            "root/LICENSES/CON.txt",
            "root/LICENSES/notice. ",
            "root/LICENSES/LPT1.txt",
        ]
        .iter()
        .enumerate()
        {
            let zip = t.0.join(format!("{i}.zip"));
            archive(&zip, &[name])?;
            assert!(extract(&zip, &t.0.join(format!("case{i}"))).is_err());
        }
        assert!(!t.0.join("outside").exists());
        Ok(())
    }
    #[test]
    fn unknown_partial_stage_is_retained() -> Result<(), Error> {
        let t = Temp::new()?;
        let manifest = tack_update::Manifest {
            schema: 1,
            version: "0.1.0-dev.2".into(),
            channel: tack_update::Channel::Dev,
            git_sha: "a".repeat(40),
            protocol_major: tack_update::PROTOCOL_MAJOR,
            assets: vec![tack_update::Asset {
                platform: "linux-x86_64".into(),
                name: "tack-linux-x86_64.zip".into(),
                size: 1,
                sha256: "b".repeat(64),
            }],
        };
        let d = t.0.join(".tack-update-0.1.0-dev.2");
        std::fs::create_dir(&d)?;
        std::fs::write(d.join("manifest.json"), serde_json::to_vec(&manifest)?)?;
        std::fs::write(d.join("package.zip"), b"partial")?;
        std::fs::create_dir(d.join("verified"))?;
        std::fs::write(d.join("verified/tack"), b"half extraction")?;
        clear_verified(&d, &manifest)?;
        assert!(!d.join("verified").exists());
        assert_eq!(std::fs::read(d.join("package.zip"))?, b"partial");
        cleanup(&d, &manifest)?;
        assert!(!d.exists());
        std::fs::create_dir(&d)?;
        std::fs::write(d.join("manifest.json"), serde_json::to_vec(&manifest)?)?;
        std::fs::write(d.join("artist.tack"), b"board")?;
        assert!(cleanup(&d, &manifest).is_err());
        assert_eq!(std::fs::read(d.join("artist.tack"))?, b"board");
        Ok(())
    }
}
