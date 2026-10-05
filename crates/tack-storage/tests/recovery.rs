use std::{fs, path::PathBuf};
use tack_core::*;
use tack_storage::*;
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
struct Root(PathBuf);
impl Root {
    fn new() -> Result<Self> {
        let p = std::env::temp_dir().join(format!(
            "tack-recovery-test-{:032x}",
            new_document_id()?.value()
        ));
        fs::create_dir(&p)?;
        Ok(Self(p))
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn doc() -> Result<Document> {
    Ok(Document::new(new_document_id()?, DocumentLimits::default()))
}
fn edit(d: &mut Document) -> Result {
    d.apply(Command::AddObject {
        object: DocumentObject::frame(
            ObjectId::new(1)?,
            "Recovery".into(),
            Transform::new([0.; 2], [100.; 2], 0., [false; 2])?,
        )?,
        index: 0,
    })?;
    Ok(())
}
#[test]
fn recovery_is_separate_dirty_newer_and_atomic_across_inactive_snapshot_failure() -> Result {
    let r = Root::new()?;
    let path = r.0.join("board.tack");
    let normal = doc()?;
    save(&path, &normal, vec![])?;
    let bytes = fs::read(&path)?;
    let lease = BoardLease::acquire(&path)?;
    let mut changed = normal.clone();
    edit(&mut changed)?;
    lease.save_recovery(&changed, 1, vec![])?;
    assert_eq!(fs::read(&path)?, bytes);
    let (recovered, generation) = lease.recovery(normal.id())?.ok_or("no recovery")?;
    assert_eq!(recovered.document, changed);
    assert_eq!(generation, 1);
    assert!(DocumentEditor::recovered(recovered.document, 200).is_dirty());
    changed.apply(Command::SetFrameName {
        object: ObjectId::new(1)?,
        name: "Newer".into(),
    })?;
    assert!(
        lease
            .save_recovery_with_hook(&changed, 2, vec![], || Err(StorageError::Invalid(
                "interrupted before manifest"
            )))
            .is_err()
    );
    assert_eq!(
        lease
            .recovery(normal.id())?
            .ok_or("lost previous recovery")?
            .1,
        1
    );
    lease.save_recovery(&changed, 2, vec![])?;
    assert_eq!(
        lease
            .recovery(normal.id())?
            .ok_or("missing newer")?
            .0
            .document,
        changed
    );
    assert_eq!(fs::read(&path)?, bytes);
    lease.save(&changed, vec![])?;
    assert!(lease.recovery(normal.id())?.is_none()); // old base never offered over new normal
    lease.discard_recovery(normal.id())?;
    assert_eq!(lease.open()?.document, changed);
    Ok(())
}
#[test]
fn corrupt_recovery_and_unknown_entries_never_modify_authority() -> Result {
    let r = Root::new()?;
    let path = r.0.join("normal.tack");
    let mut d = doc()?;
    save(&path, &d, vec![])?;
    let normal = fs::read(&path)?;
    let lease = BoardLease::acquire(&path)?;
    edit(&mut d)?;
    lease.save_recovery(&d, 1, vec![])?;
    let dir = lease.recovery_directory();
    fs::write(dir.join("state.meta"), b"corrupt")?;
    assert!(lease.recovery(d.id()).is_err());
    assert_eq!(fs::read(&path)?, normal);
    fs::write(dir.join("user-file"), b"keep")?;
    assert!(lease.discard_recovery(DocumentId::new(999)?).is_err());
    lease.discard_recovery(d.id())?;
    assert_eq!(fs::read(dir.join("user-file"))?, b"keep");
    assert_eq!(fs::read(&path)?, normal);
    Ok(())
}
#[test]
fn ownership_refuses_second_writer_external_replace_and_new_target_race() -> Result {
    let r = Root::new()?;
    let a = r.0.join("a.tack");
    let b = r.0.join("b.tack");
    let d = doc()?;
    save(&a, &d, vec![])?;
    save(&b, &d, vec![])?;
    let lease = BoardLease::acquire(&a)?;
    assert!(BoardLease::acquire(&a).is_err());
    let other = BoardLease::acquire(&b)?;
    assert!(save(&a, &d, vec![]).is_err());
    fs::write(&a, fs::read(&b)?)?;
    assert!(lease.save(&d, vec![]).is_err());
    drop(lease);
    drop(other);
    let _again = BoardLease::acquire(&a)?;
    assert!(BoardLease::acquire_new(&b).is_err());
    let fresh = BoardLease::acquire_new(r.0.join("new.tack"))?;
    fresh.save(&d, vec![])?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        symlink(&b, r.0.join("alias.tack"))?;
        let _owner = BoardLease::acquire(&b)?;
        assert!(BoardLease::acquire(r.0.join("alias.tack")).is_err());
    }
    Ok(())
}

#[test]
fn owned_empty_seed_cleanup_keeps_stable_sidecar_and_reuses_the_slot() -> Result {
    let r = Root::new()?;
    let path = r.0.join("untitled-slot-1.tack");
    let seed = doc()?;
    let lease = BoardLease::acquire_new(&path)?;
    lease.save(&seed, vec![])?;
    let mut draft = seed.clone();
    edit(&mut draft)?;
    lease.save_recovery(&draft, 1, vec![])?;
    assert!(lease.recovery_directory().exists());
    lease.retire_empty_seed(seed.id())?;
    assert!(!path.exists());
    assert!(!lease.recovery_directory().exists());
    assert!(r.0.join("untitled-slot-1.tack.tack-lock").exists());
    let next = doc()?;
    lease.save(&next, vec![])?;
    assert_eq!(lease.open()?.document, next);
    Ok(())
}
