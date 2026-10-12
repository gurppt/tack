//! Isolated-process bounds/error/privacy check. No application state.
use serde_json::json;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _session =
        tack_storage::diagnostics::Session::start("diagnostic-probe", json!({"test":true}))?;
    let operation = tack_storage::diagnostics::Operation::new("storage", "negative_io");
    let failure = operation.stage("denied", || {
        Err::<(), _>(std::io::Error::from_raw_os_error(13))
    });
    assert_eq!(failure.err().and_then(|e| e.raw_os_error()), Some(13));
    if tack_storage::diagnostics::enabled() {
        tack_storage::diagnostics::event(
            "app",
            "error",
            tack_storage::diagnostics::error_info(&std::io::Error::other(
                "/home/private-user/private-title secret-note",
            )),
        );
        tack_storage::diagnostics::event(
            "system",
            "oversized",
            json!({"synthetic":"x".repeat(20000)}),
        );
        for _ in 0..3000 {
            tack_storage::diagnostics::event(
                "system",
                "burst",
                json!({"synthetic":"x".repeat(8000)}),
            );
        }
        tack_storage::diagnostics::flush();
    }
    Ok(())
}
