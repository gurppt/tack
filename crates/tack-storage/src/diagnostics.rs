//! Opt-in offline forensic records. No writer, timer or allocation while disabled.
//! Callers supply structural metadata only; arbitrary error text is fingerprinted.
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    error::Error,
    fs::{File, OpenOptions},
    io::{BufWriter, Read, Write},
    path::{Path, PathBuf},
    sync::{
        OnceLock,
        atomic::{AtomicU64, Ordering},
        mpsc::{self, SyncSender},
    },
    time::{Instant, SystemTime, UNIX_EPOCH},
};
const QUEUE: usize = 256;
const FILE_BYTES: u64 = 4 * 1024 * 1024;
const FILES: usize = 3;
const MAX_EVENT: usize = 16 * 1024;
const RESERVE: u64 = 3 * 1024 * 1024 * 1024;
static STATE: OnceLock<State> = OnceLock::new();
static ATTEMPT: AtomicU64 = AtomicU64::new(1);
struct State {
    send: SyncSender<Message>,
    start: Instant,
    session: String,
    process: String,
    dropped: AtomicU64,
    written: AtomicU64,
    errors_only: bool,
}
enum Message {
    Record(String),
    Flush(SyncSender<()>),
    Stop,
}
pub struct Session {
    writer: Option<std::thread::JoinHandle<()>>,
}
impl Session {
    /// Only the dedicated launcher enables this. It supplies a private owned directory.
    pub fn start(process: &str, build: Value) -> std::io::Result<Option<Self>> {
        if std::env::var_os("TACK_DEV_DIAGNOSTICS").as_deref() != Some(std::ffi::OsStr::new("1")) {
            return Ok(None);
        }
        let root = PathBuf::from(
            std::env::var_os("TACK_DIAGNOSTICS_DIR")
                .ok_or_else(|| std::io::Error::other("diagnostic directory not provided"))?,
        );
        Self::start_at(process, build, &root)
    }
    pub fn start_at(process: &str, build: Value, root: &Path) -> std::io::Result<Option<Self>> {
        let root = root.to_path_buf();
        if !root.is_absolute() {
            return Err(std::io::Error::other(
                "diagnostic directory must be absolute",
            ));
        }
        crate::create_private_directory(&root, true)?;
        if !has_reserve(&root) {
            return Err(std::io::Error::other(
                "diagnostics need 3 GiB free disk space",
            ));
        }
        let session = format!(
            "{:032x}",
            crate::random_id().map_err(std::io::Error::other)?
        );
        let prefix = format!("diag-{}-{session}", std::process::id());
        let file = private_file(&root.join(format!("{prefix}-0.jsonl")))?;
        let (send, receive) = mpsc::sync_channel(QUEUE);
        STATE
            .set(State {
                send,
                start: Instant::now(),
                session,
                process: process.into(),
                dropped: AtomicU64::new(0),
                written: AtomicU64::new(0),
                errors_only: std::env::var("TACK_DIAGNOSTICS_LEVEL").is_ok_and(|v| v == "errors"),
            })
            .map_err(|_| std::io::Error::other("diagnostics already initialized"))?;
        let writer = std::thread::Builder::new()
            .name("tack-dev-log".into())
            .spawn(move || {
                let mut file = BufWriter::with_capacity(32 * 1024, file);
                let mut bytes = 0u64;
                let mut rotation = 0usize;
                while let Ok(message) = receive.recv() {
                    match message {
                        Message::Record(line) => {
                            if bytes + line.len() as u64 + 1 > FILE_BYTES - 2048 {
                                let _ = file.flush();
                                rotation = (rotation + 1) % FILES;
                                if !has_reserve(&root) {
                                    break;
                                }
                                let path = root.join(format!("{prefix}-{rotation}.jsonl"));
                                if rotation == 0 || path.exists() {
                                    let _ = std::fs::remove_file(&path);
                                }
                                match private_file(&path) {
                                    Ok(f) => file = BufWriter::new(f),
                                    Err(_) => break,
                                }
                                bytes = 0;
                            }
                            if writeln!(file, "{line}").and_then(|_| file.flush()).is_err() {
                                break;
                            }
                            bytes += line.len() as u64 + 1;
                            if let Some(state) = STATE.get() {
                                state.written.fetch_add(1, Ordering::Relaxed);
                            }
                        }
                        Message::Flush(done) => {
                            let _ = file.flush();
                            let _ = done.send(());
                        }
                        Message::Stop => {
                            if let Some(state)=STATE.get(){
                                let utc=SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
                                let _=writeln!(file,"{}",json!({"utc":utc_iso(utc.as_secs(),utc.subsec_nanos()),"pid":std::process::id(),"session":state.session,"category":"app","event":"writer_summary","severity":"info","fields":{"written":state.written.load(Ordering::Relaxed),"dropped":state.dropped.load(Ordering::Relaxed)}}));
                            }
                            break;
                        },
                    }
                }
                let _ = file.flush();
            })?;
        event(
            "app",
            "launch",
            json!({"build":build,"os":std::env::consts::OS,
            "arch":std::env::consts::ARCH,"parent_session":std::env::var("TACK_DIAGNOSTICS_PARENT").ok(),
            "quotas":{"queue":QUEUE,"event_bytes":MAX_EVENT,"file_bytes":FILE_BYTES,"files":FILES,"reserve_bytes":RESERVE}}),
        );
        let original = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            event(
                "app",
                "panic",
                json!({"payload":"omitted/content privacy",
                "location":info.location().map(|l|json!({"file_id":fingerprint(l.file().as_bytes()),"line":l.line(),"column":l.column()})),
                "backtrace":std::backtrace::Backtrace::force_capture().to_string().lines().filter(|l|!l.trim_start().starts_with("at ") && (l.contains("tack_") || l.contains("std::") || l.contains("core::"))).take(32).collect::<Vec<_>>()}),
            );
            flush();
            original(info);
        }));
        Ok(Some(Self {
            writer: Some(writer),
        }))
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        if let Some(state) = STATE.get() {
            event(
                "app",
                "session_end",
                json!({"written":state.written.load(Ordering::Relaxed),"dropped":state.dropped.load(Ordering::Relaxed)}),
            );
            let _ = state.send.send(Message::Stop);
        }
        if let Some(writer) = self.writer.take() {
            let _ = writer.join();
        }
    }
}
fn private_file(path: &Path) -> std::io::Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}
fn has_reserve(root: &Path) -> bool {
    #[cfg(unix)]
    let output = std::process::Command::new("df")
        .args(["-Pk", "--"])
        .arg(root)
        .output();
    #[cfg(windows)]
    return tack_windows_io::available_bytes(root).is_ok_and(|free| free >= RESERVE);
    #[cfg(not(any(unix, windows)))]
    return false;
    #[cfg(unix)]
    {
        let Ok(output) = output else {
            return false;
        };
        if !output.status.success() {
            return false;
        }
        let text = String::from_utf8_lossy(&output.stdout);
        #[cfg(unix)]
        let free = text
            .lines()
            .last()
            .and_then(|l| l.split_whitespace().nth(3))
            .and_then(|s| s.parse::<u64>().ok())
            .map(|n| n * 1024);
        free.is_some_and(|n| n >= RESERVE)
    }
}
pub fn enabled() -> bool {
    STATE.get().is_some()
}
pub fn flush() {
    if let Some(state) = STATE.get() {
        let (tx, rx) = mpsc::sync_channel(1);
        if state.send.try_send(Message::Flush(tx)).is_ok() {
            let _ = rx.recv_timeout(std::time::Duration::from_millis(500));
        }
    }
}
pub fn event(category: &str, name: &str, fields: Value) {
    let Some(state) = STATE.get() else {
        return;
    };
    if state.errors_only
        && !name.contains("error")
        && name != "panic"
        && name != "launch"
        && name != "process_exit"
    {
        return;
    }
    let thread = std::thread::current();
    let utc = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let line = json!({"utc_unix_ns":utc.as_nanos().to_string(),"utc":utc_iso(utc.as_secs(),utc.subsec_nanos()),
        "monotonic_us":state.start.elapsed().as_micros() as u64,"pid":std::process::id(),
        "process":state.process,"thread":thread.name().unwrap_or("unnamed"),"thread_id":format!("{:?}",thread.id()),
        "session":state.session,"category":category,"event":name,
        "severity":if name.contains("error") || name == "panic" {"error"} else {"info"},
        "dropped_events":state.dropped.load(Ordering::Relaxed),"fields":fields}).to_string();
    if line.len() > MAX_EVENT || state.send.try_send(Message::Record(line)).is_err() {
        state.dropped.fetch_add(1, Ordering::Relaxed);
    }
}
fn utc_iso(seconds: u64, nanos: u32) -> String {
    // Gregorian civil date from days since Unix epoch (no locale or timezone dependency).
    let z = seconds as i64 / 86400 + 719468;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let mut y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    y += i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}.{nanos:09}Z",
        seconds / 3600 % 24,
        seconds / 60 % 60,
        seconds % 60
    )
}
pub fn correlation(attempt: u64) -> String {
    STATE
        .get()
        .map_or_else(|| "disabled".into(), |s| format!("{}/{attempt}", s.session))
}
pub fn fingerprint(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub fn path_info(path: &Path) -> Value {
    let text = path.to_string_lossy();
    let lower = text.to_ascii_lowercase();
    let class = if lower.starts_with(r"\\?\unc\")
        || (lower.starts_with(r"\\") && !lower.starts_with(r"\\?\"))
        || lower.starts_with("//")
    {
        "unc"
    } else if lower.contains("onedrive") {
        "onedrive-candidate"
    } else if lower.contains("raster-cache-v1") {
        "cache"
    } else if path.is_absolute()
        || lower.starts_with(r"\\?\")
        || lower.as_bytes().get(1) == Some(&b':')
    {
        "local-absolute"
    } else {
        "relative"
    };
    json!({"class":class,"id":fingerprint(text.as_bytes()),"unicode":!text.is_ascii()})
}
pub fn error_info(error: &(dyn Error + 'static)) -> Value {
    let mut chain = Vec::new();
    let mut current = Some(error);
    while let Some(e) = current {
        if chain.len() == 16 {
            break;
        }
        let message = e.to_string();
        let io = e.downcast_ref::<std::io::Error>();
        let known_message = match e.downcast_ref::<crate::StorageError>() {
            Some(
                crate::StorageError::Invalid(s)
                | crate::StorageError::Unsupported(s)
                | crate::StorageError::Corrupt(s),
            ) => Some(*s),
            _ => None,
        };
        chain.push(json!({"message_id":fingerprint(message.as_bytes()),"known_message":known_message,"reason":reason(&message),
            "os_error":io.and_then(|e|e.raw_os_error()),"io_kind":io.map(|e|format!("{:?}",e.kind())),
            "os_message":io.filter(|e|e.raw_os_error().is_some()).map(std::string::ToString::to_string)}));
        current = e.source();
    }
    json!({"chain":chain,"privacy":"arbitrary messages fingerprinted, OS errors and known reasons preserved"})
}
fn reason(message: &str) -> Option<&'static str> {
    [
        "running scripts is disabled",
        "PSSecurityException",
        "UnauthorizedAccessException",
        "UnauthorizedAccess",
        "ExecutionPolicy",
        "Win32Exception",
        "Access is denied",
        "Permission denied",
        "sharing violation",
        "Address already in use",
        "new board requires an unused filename",
        "board changed outside Tack",
        "source changed while saving",
        "native operation cancelled or unavailable",
        "native file/clipboard helper unavailable",
        "timed out",
        "host stopped",
        "Sharing cancelled",
        "Shared copy already exists",
        "invalid",
        "corrupt",
    ]
    .into_iter()
    .find(|s| {
        message
            .to_ascii_lowercase()
            .contains(&s.to_ascii_lowercase())
    })
}
/// Never logs stdout (clipboard/picker may contain document content). Drain both pipes.
/// Prefix capture is bounded even if the child emits unlimited stderr.
pub fn drain_stderr(
    mut pipe: impl Read + Send + 'static,
    category: &'static str,
    attempt: u64,
) -> std::io::Result<std::thread::JoinHandle<()>> {
    std::thread::Builder::new().name("tack-dev-stderr".into()).spawn(move || {
        let mut buffer = [0u8;4096]; let mut prefix = Vec::new(); let mut total=0u64;
        loop { match pipe.read(&mut buffer) {
            Ok(0)=>break, Ok(n)=>{total+=n as u64; let take=n.min(65536usize.saturating_sub(prefix.len()));prefix.extend_from_slice(&buffer[..take]);},
            Err(e)=>{ event(category,"stderr_read_error",json!({"attempt":attempt,"error":error_info(&e)}));break;}
        }}
        let text=String::from_utf8_lossy(&prefix);
        event(category,if total>0{"child_stderr_error"}else{"child_stderr"},json!({"attempt":attempt,"bytes":total,"captured_bytes":prefix.len(),"truncated":total>prefix.len() as u64,"message_id":fingerprint(&prefix),"reason":reason(&text),"engine_tokens":text.split(|c:char|!c.is_ascii_alphanumeric()&&c!='_'&&c!='.').filter(|s|s.len()<128 && (["PSSecurityException","UnauthorizedAccessException","System.Management.Automation.PSSecurityException","System.ComponentModel.Win32Exception","Win32Exception","IOException","FileNotFoundException","RuntimeException","UnauthorizedAccess"].contains(s))).take(16).collect::<Vec<_>>() }));
    })
}
pub fn binary_info(path: &Path) -> Value {
    // Worker/startup only, never render-thread I/O.
    let result = (|| -> std::io::Result<String> {
        let mut file = File::open(path)?;
        let mut hash = Sha256::new();
        let mut buffer = [0; 65536];
        loop {
            let n = file.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            hash.update(&buffer[..n]);
        }
        Ok(format!("{:x}", hash.finalize()))
    })();
    match result {
        Ok(hash) => json!({"path":path_info(path),"sha256":hash}),
        Err(e) => json!({"path":path_info(path),"error":error_info(&e)}),
    }
}
/// Correlation is explicit and can be moved to a worker without thread-local globals.
pub struct Operation {
    category: &'static str,
    name: &'static str,
    attempt: u64,
    start: Option<Instant>,
}
impl Operation {
    pub fn new(category: &'static str, name: &'static str) -> Self {
        let start = enabled().then(Instant::now);
        let attempt = if start.is_some() {
            ATTEMPT.fetch_add(1, Ordering::Relaxed)
        } else {
            0
        };
        if start.is_some() {
            event(
                category,
                "begin",
                json!({"operation":name,"attempt":attempt}),
            );
        }
        Self {
            category,
            name,
            attempt,
            start,
        }
    }
    pub fn id(&self) -> u64 {
        self.attempt
    }
    pub fn stage<T, E: Error + 'static>(
        &self,
        name: &str,
        work: impl FnOnce() -> Result<T, E>,
    ) -> Result<T, E> {
        let Some(_) = self.start else {
            return work();
        };
        let started = Instant::now();
        event(
            self.category,
            "stage_begin",
            json!({"operation":self.name,"attempt":self.attempt,"stage":name}),
        );
        let result = work();
        event(
            self.category,
            if result.is_ok() {
                "stage_end"
            } else {
                "stage_error"
            },
            json!({"operation":self.name,"attempt":self.attempt,"stage":name,"duration_us":started.elapsed().as_micros() as u64,
            "result":if result.is_ok(){"ok"}else{"error"},"error":result.as_ref().err().map(|e|error_info(e))}),
        );
        result
    }
    pub fn note(&self, name: &str, fields: Value) {
        if self.start.is_some() {
            event(
                self.category,
                name,
                json!({"operation":self.name,"attempt":self.attempt,"data":fields}),
            );
        }
    }
}
impl Drop for Operation {
    fn drop(&mut self) {
        if let Some(start) = self.start {
            event(
                self.category,
                "end",
                json!({"operation":self.name,"attempt":self.attempt,"duration_us":start.elapsed().as_micros() as u64,"outcome":"inspect stage records"}),
            );
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn privacy_and_error_codes() {
        let path = path_info(Path::new("/home/private-name/artwork secret.png"));
        let text = path.to_string();
        assert!(!text.contains("private-name"));
        assert!(!text.contains("artwork"));
        let e = std::io::Error::from_raw_os_error(13);
        assert_eq!(error_info(&e)["chain"][0]["os_error"], 13);
    }
    #[test]
    fn windows_path_classes_are_not_confused_by_verbatim_prefix() {
        assert_eq!(
            path_info(Path::new(r"\\?\C:\Users\Private\board.tack"))["class"],
            "local-absolute"
        );
        assert_eq!(
            path_info(Path::new(r"\\?\UNC\host\share\board.tack"))["class"],
            "unc"
        );
        assert_eq!(
            path_info(Path::new(r"\\host\share\board.tack"))["class"],
            "unc"
        );
    }
    #[test]
    fn utc_is_stable() {
        assert_eq!(utc_iso(0, 42), "1970-01-01T00:00:00.000000042Z");
        assert_eq!(utc_iso(1791792000, 0), "2026-10-12T08:00:00.000000000Z");
    }
    #[test]
    fn off_does_not_start_writer() {
        assert!(!enabled());
        let operation = Operation::new("save", "test");
        assert_eq!(operation.id(), 0);
        assert_eq!(
            operation
                .stage("identity", || Ok::<_, std::io::Error>(4))
                .ok(),
            Some(4)
        );
    }
}
