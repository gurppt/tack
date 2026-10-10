//! Bounded journal for replacement, interruption recovery and one previous package.
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tack_update::Error;
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub path: PathBuf,
    pub existed: bool,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Journal {
    pub committed: bool,
    pub entries: Vec<Entry>,
}
pub fn reject_links(root: &Path, relative: &Path) -> Result<(), Error> {
    let mut p = root.to_owned();
    for c in relative.components() {
        p.push(c);
        if std::fs::symlink_metadata(&p).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err("Install contains an asset symlink; download manually".into());
        }
    }
    Ok(())
}
fn read(backup: &Path) -> Result<Journal, Error> {
    reject_links(
        backup.parent().ok_or("Backup parent")?,
        Path::new(".tack-previous"),
    )?;
    reject_links(backup, Path::new("journal.json"))?;
    reject_links(backup, Path::new("journal.tmp"))?;
    let p = if backup.join("journal.json").exists() {
        backup.join("journal.json")
    } else {
        backup.join("journal.tmp")
    };
    let f = std::fs::File::open(&p)?;
    if f.metadata()?.len() > 32 * 1024 {
        return Err("Update journal too large".into());
    }
    let _ = f;
    let j: Journal = serde_json::from_slice(&tack_update::read_small(&p, 32 * 1024)?)?;
    if j.entries.len() > crate::package::MAX_FILES
        || j.entries.iter().any(|e| !crate::package::allowed(&e.path))
    {
        return Err("Unknown backup entries; preserve installation".into());
    }
    Ok(j)
}
fn write(backup: &Path, journal: &Journal) -> Result<(), Error> {
    use std::io::Write;
    let temporary = backup.join("journal.tmp");
    let mut f = std::fs::File::create(&temporary)?;
    f.write_all(&serde_json::to_vec(journal)?)?;
    f.sync_all()?;
    drop(f);
    let target = backup.join("journal.json");
    // Windows rename does not replace; small journal is still recoverable from .tmp.
    if target.exists() {
        std::fs::remove_file(&target)?;
    }
    std::fs::rename(temporary, target)?;
    Ok(())
}
pub fn recover(install: &Path) -> Result<bool, Error> {
    let backup = install.join(".tack-previous");
    if !backup.exists() {
        return Ok(false);
    }
    if read(&backup)?.committed {
        return Ok(false);
    }
    rollback(install)?;
    Ok(true)
}
pub fn rollback(install: &Path) -> Result<(), Error> {
    let backup = install.join(".tack-previous");
    let mut j = read(&backup)?;
    for entry in j.entries.iter().rev() {
        reject_links(install, &entry.path)?;
        reject_links(&backup, &entry.path)?;
        let destination = install.join(&entry.path);
        let previous = backup.join(&entry.path);
        if previous.is_file() {
            if destination.is_file() {
                std::fs::remove_file(&destination)?;
            }
            std::fs::rename(previous, destination)?;
        } else if !entry.existed
            && destination.is_file()
            && !entry.path.starts_with("gfx/icons")
            && !entry.path.starts_with("gfx/cursors")
        {
            std::fs::remove_file(destination)?;
        }
    }
    j.committed = true;
    write(&backup, &j)?;
    Ok(())
}
fn remove_empty(path: &Path) -> Result<(), Error> {
    for entry in std::fs::read_dir(path)? {
        let e = entry?;
        if e.file_type()?.is_dir() {
            remove_empty(&e.path())?;
        }
    }
    std::fs::remove_dir(path)?;
    Ok(())
}
// Enumerate everything before deleting a single old asset. Unknown files remain intact.
fn preflight_backup(backup: &Path, journal: &Journal) -> Result<(), Error> {
    let known: std::collections::BTreeSet<_> =
        journal.entries.iter().map(|e| e.path.as_path()).collect();
    fn walk(
        root: &Path,
        path: &Path,
        known: &std::collections::BTreeSet<&Path>,
        count: &mut usize,
    ) -> Result<(), Error> {
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            *count += 1;
            if *count > 512 {
                return Err("Backup path budget exceeded".into());
            }
            let p = entry.path();
            let relative = p.strip_prefix(root)?;
            let kind = entry.file_type()?;
            if kind.is_symlink() {
                return Err("Unknown backup symlink retained".into());
            }
            if kind.is_dir() {
                if !known.iter().any(|k| k.starts_with(relative)) {
                    return Err("Unknown backup directory retained".into());
                }
                walk(root, &p, known, count)?;
            } else if !kind.is_file()
                || (!known.contains(relative)
                    && !matches!(relative.to_str(), Some("journal.json" | "journal.tmp")))
            {
                return Err("Unknown backup file retained".into());
            }
        }
        Ok(())
    }
    walk(backup, backup, &known, &mut 0)
}
pub fn prepare(install: &Path, files: &[PathBuf]) -> Result<Journal, Error> {
    if files.len() > crate::package::MAX_FILES || files.iter().any(|p| !crate::package::allowed(p))
    {
        return Err("Invalid update replacement list".into());
    }
    let journal = Journal {
        committed: false,
        entries: files
            .iter()
            .map(|p| Entry {
                path: p.clone(),
                existed: install.join(p).exists(),
            })
            .collect(),
    };
    for e in &journal.entries {
        reject_links(install, &e.path)?;
        if install.join(&e.path).exists() && !install.join(&e.path).is_file() {
            return Err("An application asset is not a regular file; preserve installation".into());
        }
    }
    reject_links(install, Path::new(".tack-previous"))?;
    let backup = install.join(".tack-previous");
    if backup.exists() {
        let old = read(&backup)?;
        if !old.committed {
            rollback(install)?;
            return Err(
                "Interrupted update recovered; restart the previous version before trying again"
                    .into(),
            );
        }
        preflight_backup(&backup, &old)?;
        for e in old.entries {
            reject_links(&backup, &e.path)?;
            let p = backup.join(e.path);
            if p.is_file() {
                std::fs::remove_file(p)?;
            }
        }
        for name in ["journal.json", "journal.tmp"] {
            let path = backup.join(name);
            if path.exists() {
                std::fs::remove_file(path)?;
            }
        }
        remove_empty(&backup)?;
    }
    std::fs::create_dir(&backup)?;
    if let Err(error) = write(&backup, &journal) {
        // An incomplete known journal remains recoverable; an empty fresh directory does not.
        if !backup.join("journal.tmp").exists() {
            let _ = std::fs::remove_dir(&backup);
        }
        return Err(error);
    }
    Ok(journal)
}
pub fn replace(install: &Path, verified: &Path, journal: &Journal) -> Result<(), Error> {
    let backup = install.join(".tack-previous");
    for e in &journal.entries {
        let destination = install.join(&e.path);
        let previous = backup.join(&e.path);
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent)?;
        }
        if let Some(parent) = previous.parent() {
            std::fs::create_dir_all(parent)?;
        }
        if e.existed {
            std::fs::rename(&destination, &previous)?;
        }
        std::fs::rename(verified.join(&e.path), destination)?;
    }
    Ok(())
}
pub fn commit(install: &Path, mut journal: Journal) -> Result<(), Error> {
    journal.committed = true;
    write(&install.join(".tack-previous"), &journal)
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::Temp;
    #[test]
    fn rollback_is_exact_and_preserves_board_and_icons() -> Result<(), Error> {
        let root = Temp::new()?;
        let install = root.0.join("install");
        let stage = root.0.join("stage");
        std::fs::create_dir(&install)?;
        std::fs::create_dir(&stage)?;
        std::fs::write(install.join("tack"), b"old")?;
        std::fs::write(install.join("artist.tack"), b"board")?;
        std::fs::create_dir_all(install.join("gfx/icons"))?;
        std::fs::write(install.join("gfx/icons/pointer.png"), b"artist pixels")?;
        std::fs::write(stage.join("tack"), b"new")?;
        std::fs::write(stage.join("tack-updater"), b"new helper")?;
        let files = vec![PathBuf::from("tack"), PathBuf::from("tack-updater")];
        let j = prepare(&install, &files)?;
        replace(&install, &stage, &j)?;
        assert_eq!(std::fs::read(install.join("tack"))?, b"new");
        rollback(&install)?;
        assert_eq!(std::fs::read(install.join("tack"))?, b"old");
        assert!(!install.join("tack-updater").exists());
        assert_eq!(std::fs::read(install.join("artist.tack"))?, b"board");
        assert_eq!(
            std::fs::read(install.join("gfx/icons/pointer.png"))?,
            b"artist pixels"
        );
        Ok(())
    }
    #[test]
    fn interrupted_apply_is_recovered_before_next_update() -> Result<(), Error> {
        let root = Temp::new()?;
        std::fs::write(root.0.join("tack"), b"old")?;
        let files = vec![PathBuf::from("tack")];
        let _ = prepare(&root.0, &files)?;
        std::fs::rename(root.0.join("tack"), root.0.join(".tack-previous/tack"))?;
        assert!(prepare(&root.0, &files).is_err());
        assert_eq!(std::fs::read(root.0.join("tack"))?, b"old");
        Ok(())
    }
    #[cfg(unix)]
    #[test]
    fn backup_symlink_never_deletes_external_files() -> Result<(), Error> {
        let root = Temp::new()?;
        let external = Temp::new()?;
        std::fs::write(external.0.join("tack"), b"safe")?;
        std::os::unix::fs::symlink(&external.0, root.0.join(".tack-previous"))?;
        assert!(prepare(&root.0, &[PathBuf::from("tack")]).is_err());
        assert_eq!(std::fs::read(external.0.join("tack"))?, b"safe");
        Ok(())
    }
    #[test]
    fn committed_temporary_journal_can_be_replaced() -> Result<(), Error> {
        let root = Temp::new()?;
        std::fs::write(root.0.join("tack"), b"old")?;
        let files = vec![PathBuf::from("tack")];
        let j = prepare(&root.0, &files)?;
        commit(&root.0, j)?;
        let backup = root.0.join(".tack-previous");
        std::fs::rename(backup.join("journal.json"), backup.join("journal.tmp"))?;
        assert!(!recover(&root.0)?);
        let next = prepare(&root.0, &files)?;
        assert!(!next.committed);
        assert!(backup.join("journal.json").is_file());
        assert!(!backup.join("journal.tmp").exists());
        Ok(())
    }
    #[test]
    fn refused_update_retains_previous_backup_before_any_cleanup() -> Result<(), Error> {
        let root = Temp::new()?;
        let verified = root.0.join("verified");
        std::fs::create_dir(&verified)?;
        std::fs::write(root.0.join("tack"), b"old")?;
        std::fs::write(verified.join("tack"), b"new")?;
        let files = vec![PathBuf::from("tack")];
        let journal = prepare(&root.0, &files)?;
        replace(&root.0, &verified, &journal)?;
        commit(&root.0, journal)?;
        std::fs::create_dir(root.0.join("tack-server"))?;
        assert!(
            prepare(
                &root.0,
                &[PathBuf::from("tack"), PathBuf::from("tack-server")]
            )
            .is_err()
        );
        assert_eq!(std::fs::read(root.0.join(".tack-previous/tack"))?, b"old");
        std::fs::write(
            root.0.join(".tack-previous/artist.tack"),
            b"unknown user file",
        )?;
        assert!(prepare(&root.0, &files).is_err());
        assert_eq!(std::fs::read(root.0.join(".tack-previous/tack"))?, b"old");
        assert_eq!(
            std::fs::read(root.0.join(".tack-previous/artist.tack"))?,
            b"unknown user file"
        );
        Ok(())
    }
}
