//! Execute the real worker admission boundary, not just display-string mapping.
use std::{path::PathBuf, sync::atomic::AtomicBool};
use tack_app::local_import::{
    ImportRequest, ImportUpdate,
    failure::{Category, Stage},
};
use tack_assets::AssetError;
struct Fixture(PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
#[test]
fn missing_and_unsupported_sources_keep_specific_failure_receipts() -> Result<(), AssetError> {
    let path = std::env::temp_dir().join(format!(
        "tack-import-reasons-{}",
        tack_storage::new_document_id()?.value()
    ));
    std::fs::create_dir(&path)?;
    let root = Fixture(path);
    let gif = root.0.join("poster.gif");
    std::fs::write(&gif,b"GIF89a\x01\x00\x01\x00\x80\x00\x00\x00\x00\x00\xff\xff\xff\x21\xf9\x04\x01\x00\x00\x00\x00\x2c\x00\x00\x00\x00\x01\x00\x01\x00\x00\x02\x02\x44\x01\x00\x3b")?;
    let paths = vec![root.0.join("missing.png"), gif];
    let mut failures = Vec::new();
    let mut finished = false;
    let mut unexpected_admissions = 0;
    ImportRequest {
        temporary: None,
        paths,
        embedded: true,
        position: [0., 0.],
        viewport: [800., 600.],
        sampling: tack_core::ImageFiltering::Default,
        mouse_easter_zoom: None,
        work: root.0.join("work"),
        spool: None,
    }
    .run(&AtomicBool::new(false), |update| {
        match update {
            ImportUpdate::Failed {
                message, failure, ..
            } => failures.push((message, failure)),
            ImportUpdate::Finished { admitted, .. } => {
                assert_eq!(admitted, 0);
                finished = true;
            }
            ImportUpdate::Admitted(_) => unexpected_admissions += 1,
            _ => {}
        }
        true
    })?;
    assert!(finished);
    assert_eq!(unexpected_admissions, 0);
    assert_eq!(failures.len(), 2);
    assert_eq!(
        (failures[0].1.stage, failures[0].1.category),
        (Stage::Path, Category::Unavailable)
    );
    assert_eq!(
        (failures[1].1.stage, failures[1].1.category),
        (Stage::Metadata, Category::Variant)
    );
    assert_eq!(failures[1].1.code, "unsupported-format");
    for (message, _) in failures {
        assert!(!message.contains(root.0.to_string_lossy().as_ref()));
    }
    Ok(())
}
