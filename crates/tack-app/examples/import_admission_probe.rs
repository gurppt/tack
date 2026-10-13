//! Explicit Stage A worker admission probe, only on marked disposable fixtures.
use std::{path::PathBuf, sync::atomic::AtomicBool, time::Instant};
use tack_app::local_import::{ImportRequest, ImportUpdate};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let expected_admit = std::env::args_os().any(|s| s == "--expect-admit");
    let root = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .ok_or("fixture directory required")?;
    if std::fs::read_to_string(root.join("OWNER_STAGE_A.txt"))?
        != "P0-WIN-IMAGE-CORE-02 Stage A disposable fixtures\n"
    {
        return Err("not a marked disposable fixture directory".into());
    }
    for ext in ["jpg", "png"] {
        let path = root.join(format!("noise-8000x9000.{ext}"));
        let (pixels, header_bytes) =
            tack_assets::image_metadata(&path).map_err(|e| e.to_string())?;
        let encoded = std::fs::metadata(&path)?.len();
        for embedded in [false, true] {
            let mut rejected = 0;
            let mut admitted = 0;
            let mut reason = String::new();
            let mut code = "";
            let started = Instant::now();
            let request = ImportRequest {
                temporary: None,
                paths: vec![path.clone()],
                embedded,
                position: [0., 0.],
                viewport: [800., 600.],
                sampling: tack_core::ImageFiltering::Default,
                mouse_easter_zoom: None,
                work: root.join(format!("probe-{ext}-{embedded}")),
                spool: None,
            };
            request
                .run(&AtomicBool::new(false), |event| {
                    match event {
                        ImportUpdate::Failed {
                            message, failure, ..
                        } => {
                            rejected += 1;
                            reason = message;
                            code = failure.code;
                        }
                        ImportUpdate::Admitted(_) => admitted += 1,
                        _ => {}
                    };
                    true
                })
                .map_err(|e| e.to_string())?;
            println!(
                "{}",
                serde_json::json!({"file":path.file_name().map(|name|name.to_string_lossy()),"embedded":embedded,"pixels":pixels,"encoded_bytes":encoded,"header_bytes":header_bytes,"admitted":admitted,"rejected":rejected,"reason":reason,"elapsed_ms":started.elapsed().as_secs_f64()*1000.})
            );
            if if expected_admit {
                admitted != 1 || rejected != 0
            } else {
                admitted != 0 || rejected != 1 || code != "encoded-limit"
            } {
                return Err("unexpected first admission boundary".into());
            }
        }
    }
    Ok(())
}
