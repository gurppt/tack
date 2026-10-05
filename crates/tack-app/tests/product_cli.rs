#![allow(clippy::unwrap_used)]
use std::{fs, process::Command};
use tack_core::{Command as Edit, Document, DocumentLimits, Source};
use tack_storage::{TackFile, new_document_id, new_source_id, save};
#[test]
fn relative_link_repair_preserves_binding_or_refuses_save_as() {
    let root = std::env::temp_dir().join(format!(
        "tack-cli-test-{:032x}",
        new_document_id().unwrap().value()
    ));
    fs::create_dir(&root).unwrap();
    fs::create_dir(root.join("other")).unwrap();
    let input = root.join("input.tack");
    let output = root.join("other/output.tack");
    let mut document = Document::new(new_document_id().unwrap(), DocumentLimits::default());
    document
        .apply(Edit::AddSource(
            Source::linked(new_source_id().unwrap(), "relative.png").unwrap(),
        ))
        .unwrap();
    save(&input, &document, vec![]).unwrap();
    fs::write(&output, b"prior target").unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_tack-app"))
        .arg("repair")
        .arg(&input)
        .arg(&output)
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(
        String::from_utf8_lossy(&result.stderr)
            .contains("repair output already exists and differs from input")
    );
    assert_eq!(fs::read(&output).unwrap(), b"prior target");
    let same = root.join("same.tack");
    let result = Command::new(env!("CARGO_BIN_EXE_tack-app"))
        .arg("repair")
        .arg(&input)
        .arg(&same)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(TackFile::open(same).unwrap().document, document);
    let result = Command::new(env!("CARGO_BIN_EXE_tack-app"))
        .arg("repair")
        .arg(&input)
        .arg(&input)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(TackFile::open(&input).unwrap().document, document);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn create_refuses_existing_newer_work_before_source_import() {
    let root = std::env::temp_dir().join(format!(
        "tack-cli-create-test-{:032x}",
        new_document_id().unwrap().value()
    ));
    fs::create_dir(&root).unwrap();
    let target = root.join("future.tack");
    save(
        &target,
        &Document::new(new_document_id().unwrap(), DocumentLimits::default()),
        vec![],
    )
    .unwrap();
    let mut future = fs::read(&target).unwrap();
    future[12..16].copy_from_slice(&2u32.to_le_bytes());
    fs::write(&target, &future).unwrap();
    let run = |path: &std::path::Path| {
        Command::new(env!("CARGO_BIN_EXE_tack-app"))
            .arg("create")
            .arg(path)
            .arg("--linked")
            .arg(root.join("must-not-be-opened.png"))
            .output()
            .unwrap()
    };
    let result = run(&target);
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("new document target already exists"));
    assert_eq!(fs::read(&target).unwrap(), future);
    let input = root.join("understood.tack");
    save(
        &input,
        &Document::new(new_document_id().unwrap(), DocumentLimits::default()),
        vec![],
    )
    .unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_tack-app"))
        .arg("repair")
        .arg(&input)
        .arg(&target)
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(
        String::from_utf8_lossy(&result.stderr)
            .contains("repair output already exists and differs from input")
    );
    assert_eq!(fs::read(&target).unwrap(), future);
    #[cfg(unix)]
    {
        let link = root.join("dangling.tack");
        std::os::unix::fs::symlink(root.join("missing.tack"), &link).unwrap();
        let result = run(&link);
        assert!(!result.status.success());
        assert!(
            String::from_utf8_lossy(&result.stderr).contains("new document target already exists")
        );
        assert!(
            fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink()
        );
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn report_aliases_cannot_replace_document_source_or_existing_exports() {
    let root = std::env::temp_dir().join(format!(
        "tack-cli-report-test-{:032x}",
        new_document_id().unwrap().value()
    ));
    fs::create_dir(&root).unwrap();
    let target = root.join("new.tack");
    let result = Command::new(env!("CARGO_BIN_EXE_tack-app"))
        .arg("create")
        .arg(&target)
        .arg("--linked")
        .arg(root.join("must-not-be-opened.png"))
        .arg("--output")
        .arg(root.join("./new.tack"))
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("report output aliases document"));
    assert!(!target.exists());
    let input = root.join("input.tack");
    let doc = Document::new(new_document_id().unwrap(), DocumentLimits::default());
    save(&input, &doc, vec![]).unwrap();
    let before = fs::read(&input).unwrap();
    for command in [
        vec!["repair", "input.tack", "repaired.tack", "input.tack"],
        vec!["open", "input.tack", "--output", "input.tack"],
        vec!["query-scale", "input.tack"],
        vec!["--output", "input.tack"],
    ] {
        let result = Command::new(env!("CARGO_BIN_EXE_tack-app"))
            .current_dir(&root)
            .args(command)
            .output()
            .unwrap();
        assert!(!result.status.success());
        assert!(String::from_utf8_lossy(&result.stderr).contains("report output already exists"));
        assert_eq!(fs::read(&input).unwrap(), before);
    }
    let alias = root.join("hardlink.json");
    fs::hard_link(&input, &alias).unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_tack-app"))
        .current_dir(&root)
        .args(["query-scale", "hardlink.json"])
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert_eq!(fs::read(&input).unwrap(), before);
    assert!(!root.join("repaired.tack").exists());
    let report = root.join("valid.json");
    let result = Command::new(env!("CARGO_BIN_EXE_tack-app"))
        .arg("repair")
        .arg(&input)
        .arg(&target)
        .arg(&report)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(fs::read(&report).unwrap().starts_with(b"{"));
    assert_eq!(TackFile::open(&target).unwrap().document, doc);
    fs::remove_dir_all(root).unwrap();
}
