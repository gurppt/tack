use std::{
    fs,
    path::PathBuf,
    time::{Duration, Instant},
};
use tack_app::{
    actions::Action,
    image_save::{ImageSave, Originals},
    local_worker::{LocalUpdate, LocalWorker, OpenedBoard, Operation},
    native_files,
    preferences::{self, Preferences},
};
use tack_core::*;
use tack_storage::*;
type R<T = ()> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
struct Root(PathBuf);
impl Root {
    fn new() -> R<Self> {
        let path = std::env::temp_dir().join(format!(
            "tack-files-test-{:032x}",
            new_document_id()?.value()
        ));
        fs::create_dir(&path)?;
        Ok(Self(path))
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn finish(worker: &mut LocalWorker) -> R<Vec<LocalUpdate>> {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut updates = Vec::new();
    while worker.active() {
        updates.extend(worker.poll());
        if Instant::now() >= deadline {
            return Err("local worker timeout".into());
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    Ok(updates)
}
#[test]
fn last_successful_board_folder_persists_and_cancel_missing_and_legacy_paths_are_safe() -> R {
    let root = Root::new()?;
    let directory = root.0.join("boards.v2");
    fs::create_dir(&directory)?;
    let board = directory.join("episode.tack");
    let mut profile = Preferences::defaults()?;
    assert!(profile.last_board_directory.is_none());
    profile.remember_board_directory(&board)?;
    assert_eq!(
        profile
            .last_board_directory
            .as_ref()
            .ok_or("folder")?
            .path()?,
        directory
    );
    let options = native_files::PickOptions {
        directory: Some(directory.clone()),
        suggested_name: None,
    };
    assert_eq!(
        native_files::usable_directory(options.directory.as_deref()),
        Some(directory.clone())
    );
    // Opening/cancelling a picker does not mutate the resident profile.
    assert_eq!(
        profile
            .last_board_directory
            .as_ref()
            .ok_or("folder")?
            .path()?,
        directory
    );
    let base = preferences::save_profile(&root.0, &profile, None, true, Some(&board))?;
    let restored = preferences::read(&root.0.join("preferences.json"))?;
    assert_eq!(
        restored
            .last_board_directory
            .as_ref()
            .ok_or("folder")?
            .path()?,
        directory
    );
    fs::remove_dir(&directory)?;
    assert!(native_files::usable_directory(options.directory.as_deref()).is_none());
    assert!(native_files::usable_directory(Some(&root.0.join("missing"))).is_none());
    assert!(native_files::usable_directory(Some(std::path::Path::new("relative"))).is_none());
    let mut legacy = serde_json::to_value(&profile)?;
    legacy
        .as_object_mut()
        .ok_or("profile object")?
        .remove("last_board_directory");
    let legacy: Preferences = serde_json::from_value(legacy)?;
    assert!(legacy.last_board_directory.is_none());
    preferences::save_profile(&root.0, &legacy, Some(base), false, None)?;
    assert!(
        preferences::read(&root.0.join("preferences.json"))?
            .last_board_directory
            .is_some()
    );
    Ok(())
}
#[test]
fn keymap_worker_exports_tackey_and_imports_legacy_json_without_mutating_on_failure() -> R {
    let root = Root::new()?;
    let profile = Preferences::defaults()?;
    let before = serde_json::to_vec(&profile)?;
    let mut worker = LocalWorker::default();
    let target = root.0.join("studio.v3");
    worker.start(
        Operation::Keymap {
            action: Action::ExportKeymap,
            path: target,
            profile: profile.clone(),
        },
        || {},
    )?;
    let updates = finish(&mut worker)?;
    assert!(
        updates
            .iter()
            .any(|update| matches!(update, LocalUpdate::Keymap(Action::ExportKeymap, Ok(None))))
    );
    let exported = root.0.join("studio.v3.tackey");
    assert_eq!(
        preferences::read(&exported)?.keymap()?.bindings(),
        profile.keymap()?.bindings()
    );
    let legacy = root.0.join("legacy.json");
    fs::copy(exported, &legacy)?;
    worker.start(
        Operation::Keymap {
            action: Action::ImportKeymap,
            path: legacy.clone(),
            profile: profile.clone(),
        },
        || {},
    )?;
    assert!(finish(&mut worker)?.iter().any(|update| matches!(
        update,
        LocalUpdate::Keymap(Action::ImportKeymap, Ok(Some(_)))
    )));
    fs::write(&legacy, b"corrupt JSON")?;
    worker.start(
        Operation::Keymap {
            action: Action::ImportKeymap,
            path: legacy,
            profile: profile.clone(),
        },
        || {},
    )?;
    assert!(
        finish(&mut worker)?
            .iter()
            .any(|update| matches!(update, LocalUpdate::Keymap(Action::ImportKeymap, Err(_))))
    );
    assert_eq!(serde_json::to_vec(&profile)?, before);
    Ok(())
}
#[test]
fn open_worker_publishes_owned_document_and_failures_do_not_touch_initial_seed() -> R {
    let root = Root::new()?;
    let doc = Document::new(new_document_id()?, DocumentLimits::default());
    let seed_path = root.0.join("untitled.tack");
    let lease = BoardLease::acquire_new(&seed_path)?;
    lease.save(&doc, vec![])?;
    let other_path = root.0.join("opened.tack");
    let other_doc = Document::new(new_document_id()?, DocumentLimits::default());
    save(&other_path, &other_doc, vec![])?;
    let mut worker = LocalWorker::default();
    worker.start(Operation::OpenBoard(other_path.clone()), || {})?;
    let updates = finish(&mut worker)?;
    let opened = updates
        .into_iter()
        .find_map(|update| {
            if let LocalUpdate::Opened(Ok(opened)) = update {
                Some(opened)
            } else {
                None
            }
        })
        .ok_or("owned opened board")?;
    assert_eq!(opened.board.document, other_doc);
    assert_eq!(opened.path, other_path.canonicalize()?);
    assert!(!opened.recovery);
    assert!(BoardLease::acquire(&other_path).is_err());
    assert_eq!(lease.open()?.document, doc);
    assert!(OpenedBoard::read(&root.0.join("missing.tack")).is_err());
    let corrupt = root.0.join("corrupt.tack");
    fs::write(&corrupt, b"invalid board")?;
    worker.start(Operation::OpenBoard(corrupt), || {})?;
    assert!(
        finish(&mut worker)?
            .iter()
            .any(|update| matches!(update, LocalUpdate::Opened(Err(_))))
    );
    assert_eq!(lease.open()?.document, doc);
    Ok(())
}
#[test]
fn save_as_appends_dotted_suffix_and_retains_existing_atomic_ownership() -> R {
    let root = Root::new()?;
    let doc = Document::new(new_document_id()?, DocumentLimits::default());
    let base = root.0.join("base.tack");
    save(&base, &doc, vec![])?;
    let board = std::sync::Arc::new(TackFile::open(&base)?);
    let mut editor = DocumentEditor::new(doc, 200);
    let raw = root.0.join("episode.v3");
    let mut job = ImageSave::default();
    job.start_as(
        raw.clone(),
        base,
        board,
        &editor,
        &Default::default(),
        &Originals::new(),
    )?;
    job.finish(&mut editor);
    assert!(job.last_error.is_none());
    assert!(!raw.exists());
    assert!(TackFile::open(root.0.join("episode.v3.tack")).is_ok());
    assert!(!editor.is_dirty());
    assert!(native_files::usable_directory(Some(&root.0.join("episode.v3.tack"))).is_none());
    Ok(())
}

#[test]
fn cancelled_open_drops_already_published_result_and_releases_new_board_lease() -> R {
    let root = Root::new()?;
    let target = root.0.join("opened.tack");
    let doc = Document::new(new_document_id()?, DocumentLimits::default());
    save(&target, &doc, vec![])?;
    let (sent, received) = std::sync::mpsc::sync_channel(4);
    let mut worker = LocalWorker::default();
    worker.start(Operation::OpenBoard(target.clone()), move || {
        let _ = sent.try_send(());
    })?;
    received.recv_timeout(Duration::from_secs(5))?;
    worker.cancel();
    assert!(
        !finish(&mut worker)?
            .iter()
            .any(|update| matches!(update, LocalUpdate::Opened(_)))
    );
    assert!(BoardLease::acquire(&target).is_ok());
    assert_eq!(TackFile::open(target)?.document, doc);
    Ok(())
}

#[test]
fn keymap_suffix_added_target_requires_explicit_replacement_choice() -> R {
    let root = Root::new()?;
    let raw = root.0.join("studio");
    let target = root.0.join("studio.tackey");
    fs::write(&target, b"existing keymap content")?;
    let mut worker = LocalWorker::default();
    worker.start(
        Operation::Keymap {
            action: Action::ExportKeymap,
            path: raw,
            profile: Preferences::defaults()?,
        },
        || {},
    )?;
    assert!(
        finish(&mut worker)?
            .iter()
            .any(|update| matches!(update, LocalUpdate::Keymap(Action::ExportKeymap, Err(_))))
    );
    assert_eq!(fs::read(&target)?, b"existing keymap content");
    worker.start(
        Operation::Keymap {
            action: Action::ExportKeymap,
            path: target.clone(),
            profile: Preferences::defaults()?,
        },
        || {},
    )?;
    assert!(
        finish(&mut worker)?
            .iter()
            .any(|update| matches!(update, LocalUpdate::Keymap(Action::ExportKeymap, Ok(None))))
    );
    assert!(preferences::read(&target).is_ok());
    Ok(())
}
