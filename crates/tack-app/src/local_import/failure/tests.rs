use super::*;
#[test]
fn exact_stage_category_and_limit_survive_without_private_error_text() {
    let f = Failure::from_error(
        Stage::Admission,
        Facts {
            encoded_bytes: Some(87_000_000),
            pixels: Some([8000, 9000]),
            encoded_limit: None,
        },
        &"image exceeds 64 MiB encoded-input limit".into(),
    );
    assert_eq!(
        (f.stage, f.category, f.code),
        (Stage::Admission, Category::Budget, "encoded-limit")
    );
    assert_eq!(f.limit, Some(67_108_864));
    assert!(f.description().contains("87000000 bytes"));
    let f = Failure::from_error(
        Stage::Metadata,
        Facts::default(),
        &"provider failed at /home/private/secret.png".into(),
    );
    assert!(!f.description().contains("private"));
    let f = Failure::from_error(
        Stage::Path,
        Facts::default(),
        &std::io::Error::from(ErrorKind::PermissionDenied).into(),
    );
    assert_eq!(f.category, Category::Permission);
}
#[test]
fn batch_failure_summary_bounds_samples_and_preserves_all_counts() {
    let mut s = FailureSummary::default();
    let f = Failure::from_error(
        Stage::SourceChanged,
        Facts::default(),
        &"source changed during import".into(),
    );
    for _ in 0..500 {
        s.record(Path::new("/private/folder/visible.png"), &f);
    }
    assert_eq!(s.samples.len(), 16);
    let rows = s.rows(2, true);
    assert!(rows.iter().any(|s| s == "500: source changed"));
    assert!(rows.iter().any(|s| s.contains("484 further failures")));
    assert!(rows.iter().all(|s| !s.contains("/private")));
    assert!(rows.len() <= 28);
}
#[test]
fn foreign_windows_filename_cannot_leak_its_parent_into_linux_error_details() {
    let mut summary = FailureSummary::default();
    summary.record(
        Path::new(r"C:\Users\private\missing.png"),
        &Failure::from_error(
            Stage::Path,
            Facts::default(),
            &std::io::Error::from(ErrorKind::NotFound).into(),
        ),
    );
    let rows = summary.rows(0, false);
    assert!(rows.iter().any(|s| s.contains("missing.png")));
    assert!(
        rows.iter()
            .all(|s| !s.contains("Users") && !s.contains("private"))
    );
}
