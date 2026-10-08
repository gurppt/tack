//! Explicit optional LAN publication; local startup never enters this path.
use std::{ffi::OsString, fs::OpenOptions, io::Write, path::PathBuf, sync::atomic::AtomicBool};
use tack_assets::AssetError;
use tack_shared::{Message, WireId};

pub fn run(command: &str, args: Vec<OsString>) -> Result<(), AssetError> {
    if command != "publish" {
        return Err("unknown shared command".into());
    }
    if args.len() < 2 {
        return Err("publish LOCAL_BOARD.tack NUMERIC_IP:PORT [--output REPORT.json]".into());
    }
    let input = PathBuf::from(&args[0]);
    let address = args[1]
        .to_str()
        .ok_or("server address must be text")?
        .to_owned();
    let mut output = None;
    let mut options = args.into_iter().skip(2);
    while let Some(option) = options.next() {
        match option.to_str() {
            Some("--output") => {
                output = Some(PathBuf::from(options.next().ok_or("output path required")?))
            }
            _ => return Err("unknown publish option".into()),
        }
    }
    crate::report_output::preflight(output.as_deref(), Some(&input))?;
    let client = WireId::new(tack_storage::new_document_id()?.value())?;
    let cancel = AtomicBool::new(false);
    // This CLI has no renderer. The GUI publishes/prepares originals only on workers.
    let snapshot = tack_shared::publish::publish_board(&address, &input, client, &cancel)?;
    let Message::Snapshot {
        board,
        revision,
        document,
        sources,
        ..
    } = snapshot
    else {
        return Err("publish response was not a snapshot".into());
    };
    let document = document.to_document()?;
    let receipt = serde_json::json!({"operation":"publish","board":board.to_string(),"client":client.to_string(),"revision":revision,"objects":document.object_order().len(),"sources":document.sources().count(),"available_originals":sources.len(),"authority":"server accepted; local board untouched","transport":"trusted LAN; no TLS/auth"});
    let text = serde_json::to_string_pretty(&receipt)?;
    if let Some(output) = output {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(output)?;
        file.write_all(text.as_bytes())?;
        file.write_all(b"\n")?;
    }
    println!("{text}");
    Ok(())
}
