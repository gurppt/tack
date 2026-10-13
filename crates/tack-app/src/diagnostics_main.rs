//! Artist launcher/exporter, separate from the ordinary Tack process.
use serde_json::json;
use std::{
    env, fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};
use tack_storage::diagnostics as diag;
const MARKER: &str = "Tack P0-WIN-IMAGE-CORE-02-CD owned diagnostics\n";
fn base() -> Result<PathBuf, Box<dyn std::error::Error>> {
    if let Some(root) = env::var_os("TACK_DIAGNOSTICS_HOME") {
        return Ok(std::path::absolute(root)?);
    }
    let root = if cfg!(windows) {
        env::var_os("LOCALAPPDATA")
    } else {
        env::var_os("XDG_STATE_HOME").or_else(|| {
            env::var_os("HOME").map(|p| PathBuf::from(p).join(".local/state").into_os_string())
        })
    }
    .ok_or("diagnostic home unavailable")?;
    Ok(PathBuf::from(root)
        .join("TackDiagnostics")
        .join("P0-WIN-IMAGE-CORE-02-CD"))
}
fn owned(root: &Path) -> std::io::Result<()> {
    tack_storage::create_private_directory(root, true)?;
    let marker = root.join("OWNER.txt");
    if !marker.exists() {
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(marker)?;
        f.write_all(MARKER.as_bytes())?;
    } else if fs::read_to_string(marker)? != MARKER {
        return Err(std::io::Error::other(
            "directory is not owned by diagnostics",
        ));
    }
    Ok(())
}
fn sessions(root: &Path, prefix: &str) -> std::io::Result<Vec<PathBuf>> {
    let mut entries = Vec::new();
    for e in fs::read_dir(root)? {
        let e = e?;
        let name = e.file_name();
        let name = name.to_string_lossy();
        if name
            .strip_prefix(prefix)
            .is_some_and(|s| s.len() == 32 && s.bytes().all(|b| b.is_ascii_hexdigit()))
            && e.file_type()?.is_dir()
            && fs::read_to_string(e.path().join("OWNER.txt"))
                .ok()
                .as_deref()
                == Some(MARKER)
        {
            entries.push(e.path());
        }
    }
    entries.sort_by_key(|p| fs::metadata(p).and_then(|m| m.modified()).ok());
    Ok(entries)
}
fn session_dir(
    root: &Path,
    prefix: &str,
    keep: usize,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    owned(root)?;
    let previous = sessions(root, prefix)?;
    for path in previous
        .iter()
        .take(previous.len().saturating_sub(keep - 1))
    {
        if path.join("ACTIVE").exists() {
            continue;
        }
        // Delete only this launcher's marked sessions/exports; never user boards/caches.
        fs::remove_dir_all(path)?;
    }
    if sessions(root, prefix)?.len() >= keep {
        return Err(
            "diagnostic session quota reached; close active diagnostic windows before retrying"
                .into(),
        );
    }
    let path = root.join(format!(
        "{prefix}{:032x}",
        tack_storage::new_document_id()?.value()
    ));
    owned(&path)?;
    Ok(path)
}
fn copy_new(source: &Path, target: &Path) -> std::io::Result<()> {
    let mut input = fs::File::open(source)?;
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(target)?;
    std::io::copy(&mut input, &mut output)?;
    Ok(())
}
fn export(root: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let output = session_dir(&root.join("exports"), "report-", 3)?;
    let source = root.join("sessions");
    let previous = if source.exists() {
        sessions(&source, "session-")?
    } else {
        vec![]
    };
    let mut manifest = Vec::new();
    let mut summary = String::from(
        "Tack diagnostic report (offline, structural metadata only).\nReview contents before sharing; no automatic upload.\nWindows physical acceptance must be recorded separately.\n\n",
    );
    let mut total = 0u64;
    for (index, session) in previous.iter().rev().take(6).enumerate() {
        let folder = output.join(format!("session-{index}"));
        fs::create_dir(&folder)?;
        for e in fs::read_dir(session)? {
            let e = e?;
            let path = e.path();
            let name = e.file_name();
            let text = name.to_string_lossy();
            let metadata = e.metadata()?;
            // Never export native stderr, WER dumps, profiles or arbitrary nearby files.
            if !e.file_type()?.is_file()
                || metadata.len() > 4 * 1024 * 1024
                || !(text.starts_with("diag-") && text.ends_with(".jsonl"))
            {
                continue;
            }
            total += metadata.len();
            if total > 96 * 1024 * 1024 {
                summary.push_str("Older logs omitted at export quota (96 MiB).\n");
                continue;
            }
            let bytes = fs::read(&path)?;
            let target = folder.join(&name);
            copy_new(&path, &target)?;
            manifest.push(json!({"file":format!("session-{index}/{text}"),"bytes":bytes.len(),"sha256":diag::fingerprint(&bytes)}));
            for line in bytes.split(|b| *b == b'\n') {
                if line.len() > 16384 {
                    continue;
                }
                if let Ok(value) = serde_json::from_slice::<serde_json::Value>(line)
                    && (value["severity"] == "error"
                        || value["event"] == "process_exit"
                        || value["event"] == "launch"
                        || value["event"] == "session_end"
                        || value["event"] == "writer_summary")
                    && summary.len() < 1024 * 1024
                {
                    summary.push_str(&format!(
                        "{} {} {} pid={} {}\n",
                        value["utc"],
                        value["category"],
                        value["event"],
                        value["pid"],
                        value["fields"]
                    ));
                }
            }
        }
    }
    fs::write(output.join("SUMMARY.txt"), summary)?;
    let executable = env::current_exe()?;
    let build = executable
        .parent()
        .ok_or("package directory")?
        .join("BUILD.json");
    if build.is_file() && fs::metadata(&build)?.len() < 65536 {
        copy_new(&build, &output.join("BUILD.json"))?;
    }
    for name in ["SUMMARY.txt", "BUILD.json"] {
        let path = output.join(name);
        if path.exists() {
            let bytes = fs::read(&path)?;
            manifest
                .push(json!({"file":name,"bytes":bytes.len(),"sha256":diag::fingerprint(&bytes)}));
        }
    }
    fs::write(
        output.join("SHA256_MANIFEST.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    Ok(output)
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let root = base()?;
    owned(&root)?;
    let command = env::args_os().nth(1).unwrap_or_else(|| "launch".into());
    if command == "export" {
        let output = export(&root)?;
        println!("Diagnostic report: {}", output.display());
        if cfg!(windows) {
            let _ = Command::new("explorer.exe").arg(&output).spawn();
        }
        return Ok(());
    }
    if command != "launch" {
        return Err("usage: tack-diagnostics launch [Tack arguments] | export".into());
    }
    let logs = session_dir(&root.join("sessions"), "session-", 6)?;
    let active = logs.join("ACTIVE");
    fs::write(&active, b"owned child live; do not rotate")?;
    env::set_current_dir(env::current_exe()?.parent().ok_or("package directory")?)?;
    // Session::start reads opt-in configuration. Rust 2024 forbids process-global
    // environment mutation, so the launcher supplies its own log directory through
    // the explicit initializer below; child settings use Command::env only.
    let _session = diag::Session::start_at(
        "tack-diagnostics",
        json!({"mission":"P0-WIN-IMAGE-CORE-02-CD","git_sha":env!("TACK_GIT_SHA")}),
        &logs,
    )?;
    let executable =
        env::current_exe()?.with_file_name(if cfg!(windows) { "tack.exe" } else { "tack" });
    diag::event("app", "child_binary", diag::binary_info(&executable));
    if cfg!(windows) {
        let result=Command::new("powershell.exe").args(["-NoProfile","-NonInteractive","-Command","[Environment]::OSVersion.Version.ToString();$PSVersionTable.PSVersion.ToString();$PSVersionTable.PSEdition;[Environment]::Is64BitProcess"]).output();
        match result {
            Ok(output) => {
                let text = String::from_utf8_lossy(&output.stdout);
                let values: Vec<_> = text
                    .lines()
                    .take(4)
                    .filter(|s| {
                        s.len() < 64 && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '.')
                    })
                    .collect();
                diag::event(
                    "system",
                    "powershell",
                    json!({"versions":values,"exit_code":output.status.code(),"picker_backend":"COM", "powershell_required_for_picker":false}),
                );
            }
            Err(error) => diag::event("system", "powershell_error", diag::error_info(&error)),
        }
    }

    let mut child = Command::new(executable)
        .args(env::args_os().skip(2))
        .env("TACK_DEV_DIAGNOSTICS", "1")
        .env("TACK_DIAGNOSTICS_DIR", &logs)
        .env("TACK_PROFILE_DIR", root.join("profile"))
        .env("TACK_DIAGNOSTICS_PARENT", diag::correlation(0))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let stderr = child.stderr.take().ok_or("stderr pipe")?;
    let err_reader = diag::drain_stderr(stderr, "app", 0)?;
    let mut stdout = child.stdout.take().ok_or("stdout pipe")?;
    let out_reader = std::thread::Builder::new()
        .name("tack-dev-stdout".into())
        .spawn(move || {
            let mut buf = [0; 4096];
            loop {
                match stdout.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {}
                }
            }
        })?;
    diag::event(
        "app",
        "child_spawned",
        json!({"pid":child.id(),"profile":"isolated"}),
    );
    let status = child.wait()?;
    let _ = err_reader.join();
    let _ = out_reader.join();
    #[cfg(unix)]
    let signal = {
        use std::os::unix::process::ExitStatusExt;
        status.signal()
    };
    #[cfg(not(unix))]
    let signal: Option<i32> = None;
    diag::event(
        "app",
        "process_exit",
        json!({"pid":child.id(),"success":status.success(),"exit_code":status.code(),"native_code_hex":status.code().map(|v|format!("0x{:08X}",v as u32)),"signal":signal,"kind":if signal.is_some(){"signal"}else if status.success(){"success"}else{"nonzero/native-code; inspect records"}}),
    );
    let _ = fs::remove_file(active);
    println!("Tack exited: {status}. Logs: {}", logs.display());
    println!("Double-click Export-Diagnostic-Report to collect this and previous sessions.");
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("Diagnostic launcher: {error}");
        std::process::exit(1);
    }
}
