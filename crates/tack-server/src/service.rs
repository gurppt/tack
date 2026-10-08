mod transfers;
use crate::{AssetStore, Authority, Change, MAX_BOARDS, MAX_CLIENTS, Result, Upload};
use std::{
    collections::BTreeMap,
    io::Write,
    net::{Shutdown, TcpListener, TcpStream},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
        mpsc::{SyncSender, sync_channel},
    },
    thread,
};
use tack_shared::{Message, RefusalCode, WireId, read_message, write_message};
use transfers::asset_message;

const MAX_CONNECTIONS: usize = 32;
const QUEUE_BYTES: usize = 8 * 1024 * 1024;
#[derive(Clone)]
pub struct ServerConfig {
    pub listen: String,
    pub root: PathBuf,
    pub asset_quota: u64,
}
impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            listen: "127.0.0.1:7337".into(),
            root: "tack-server-data".into(),
            asset_quota: 4 * 1024 * 1024 * 1024,
        }
    }
}
struct Out {
    send: SyncSender<Arc<Vec<u8>>>,
    bytes: Arc<AtomicUsize>,
    socket: TcpStream,
}
impl Out {
    fn send(&self, bytes: Arc<Vec<u8>>) -> bool {
        let size = bytes.len();
        let old = self.bytes.fetch_add(size, Ordering::AcqRel);
        if old > 0 && old.saturating_add(size) > QUEUE_BYTES {
            self.bytes.fetch_sub(size, Ordering::AcqRel);
            let _ = self.socket.shutdown(Shutdown::Both);
            return false;
        }
        if self.send.try_send(bytes).is_err() {
            self.bytes.fetch_sub(size, Ordering::AcqRel);
            let _ = self.socket.shutdown(Shutdown::Both);
            return false;
        }
        true
    }
}
struct Subscriber {
    board: WireId,
    client: WireId,
    out: Arc<Out>,
}
struct Hub {
    root: PathBuf,
    boards: BTreeMap<WireId, Authority>,
    subscribers: BTreeMap<usize, Subscriber>,
}
pub fn serve(config: ServerConfig) -> Result<()> {
    if !config.root.exists() {
        tack_storage::create_private_directory(&config.root, true).map_err(|e| e.to_string())?;
    }
    let _ownership = tack_storage::lock_sidecar(&config.root.join("authority.lock"))
        .map_err(|e| e.to_string())?;
    let boards_root = config.root.join("boards");
    if !boards_root.exists() {
        tack_storage::create_private_directory(&boards_root, false).map_err(|e| e.to_string())?;
    }
    discard_pending(&boards_root)?;
    let assets = AssetStore::new(&config.root.join("assets"), config.asset_quota)?;
    let hub = Arc::new(Mutex::new(Hub {
        root: boards_root,
        boards: BTreeMap::new(),
        subscribers: BTreeMap::new(),
    }));
    let active = Arc::new(AtomicUsize::new(0));
    let ids = AtomicUsize::new(1);
    let listener = TcpListener::bind(&config.listen).map_err(|e| e.to_string())?;
    println!(
        "tack-server trusted LAN only; protocol={} listen={} root={} asset_quota={}",
        tack_shared::PROTOCOL_MAJOR,
        listener.local_addr().map_err(|e| e.to_string())?,
        config.root.display(),
        config.asset_quota
    );
    for socket in listener.incoming() {
        let socket = match socket {
            Ok(s) => s,
            Err(e) => {
                eprintln!("accept: {e}");
                continue;
            }
        };
        if active.fetch_add(1, Ordering::AcqRel) >= MAX_CONNECTIONS {
            active.fetch_sub(1, Ordering::AcqRel);
            let _ = socket.shutdown(Shutdown::Both);
            continue;
        }
        let hub = Arc::clone(&hub);
        let assets = assets.clone();
        let active = Arc::clone(&active);
        let id = ids.fetch_add(1, Ordering::Relaxed);
        let result = thread::Builder::new()
            .name("tack-lan-session".into())
            .spawn(move || {
                if let Err(e) = connection(socket, id, &hub, assets) {
                    eprintln!("session {id}: {}", bounded_reason(&e));
                }
                if let Ok(mut hub) = hub.lock() {
                    hub.subscribers.remove(&id);
                }
                active.fetch_sub(1, Ordering::AcqRel);
            });
        if let Err(e) = result {
            return Err(format!("session worker: {e}"));
        }
    }
    Ok(())
}
fn discard_pending(root: &std::path::Path) -> Result<()> {
    // Only strictly named interrupted snapshots belong to this authority.
    for entry in std::fs::read_dir(root).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        if entry.path().extension().is_some_and(|s| s == "pending") {
            let name = entry.file_name();
            let name = name.to_str().ok_or("invalid pending filename")?;
            let id = name.strip_suffix(".pending").ok_or("pending filename")?;
            WireId::parse(id).map_err(|e| e.to_string())?;
            let metadata = std::fs::symlink_metadata(entry.path()).map_err(|e| e.to_string())?;
            if !metadata.file_type().is_file() {
                return Err("pending authority is not a regular file".into());
            }
            std::fs::remove_file(entry.path()).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
fn connection(
    mut socket: TcpStream,
    id: usize,
    hub: &Arc<Mutex<Hub>>,
    assets: AssetStore,
) -> Result<()> {
    socket.set_nodelay(true).map_err(|e| e.to_string())?;
    let writer = socket.try_clone().map_err(|e| e.to_string())?;
    writer
        .set_write_timeout(Some(std::time::Duration::from_secs(5)))
        .map_err(|e| e.to_string())?;
    let (send, receive) = sync_channel::<Arc<Vec<u8>>>(tack_shared::MAX_QUEUE_MESSAGES);
    let queued = Arc::new(AtomicUsize::new(0));
    let writer_queued = Arc::clone(&queued);
    let writer_shutdown = socket.try_clone().map_err(|e| e.to_string())?;
    let writer = thread::Builder::new()
        .name("tack-lan-writer".into())
        .spawn(move || {
            let mut writer = writer;
            while let Ok(bytes) = receive.recv() {
                let result = writer.write_all(&bytes);
                writer_queued.fetch_sub(bytes.len(), Ordering::AcqRel);
                if result.is_err() {
                    break;
                }
            }
            let _ = writer_shutdown.shutdown(Shutdown::Both);
        })
        .map_err(|e| e.to_string())?;
    let out = Arc::new(Out {
        send,
        bytes: queued,
        socket: socket.try_clone().map_err(|e| e.to_string())?,
    });
    let mut upload: Option<Upload> = None;
    let result = (|| {
        loop {
            let message = match read_message(&mut socket) {
                Ok(message) => message,
                Err(e) => {
                    reply(
                        &out,
                        refusal(
                            None,
                            0,
                            if matches!(e, tack_shared::Error::Version(_)) {
                                RefusalCode::VersionMismatch
                            } else {
                                RefusalCode::Protocol
                            },
                            &e.to_string(),
                        ),
                    )?;
                    return Err(e.to_string());
                }
            };
            if matches!(
                &message,
                Message::AssetBegin { .. }
                    | Message::AssetChunk { .. }
                    | Message::AssetCommit { .. }
                    | Message::AssetGet { .. }
                    | Message::AssetCancel { .. }
            ) {
                if let Some(response) = asset_message(message, &assets, &mut upload)? {
                    reply(&out, response)?;
                }
                continue;
            }
            if let Message::Edit { sources, .. } | Message::Publish { sources, .. } = &message {
                let mismatch = sources.iter().any(|b| {
                    assets
                        .length(b.hash.clone())
                        .is_ok_and(|s| s.is_some_and(|size| size != b.size))
                });
                if mismatch {
                    reply(
                        &out,
                        refusal(
                            None,
                            0,
                            RefusalCode::InvalidAsset,
                            "binding length differs from existing CAS",
                        ),
                    )?;
                    continue;
                }
            }
            control(message, id, hub, &out)?;
        }
    })();
    // Remove the hub's sender before joining; otherwise an idle writer never exits.
    if let Ok(mut hub) = hub.lock() {
        hub.subscribers.remove(&id);
    }
    drop(out);
    let _ = socket.shutdown(Shutdown::Read);
    let _ = writer.join();
    let _ = socket.shutdown(Shutdown::Both);
    result
}
fn frame(message: &Message) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    write_message(&mut bytes, message).map_err(|e| e.to_string())?;
    Ok(bytes)
}
fn reply(out: &Out, message: Message) -> Result<()> {
    let bytes = Arc::new(frame(&message)?);
    if out.send(bytes) {
        Ok(())
    } else {
        Err("bounded outgoing queue exhausted".into())
    }
}
fn control(message: Message, id: usize, hub: &Arc<Mutex<Hub>>, out: &Arc<Out>) -> Result<()> {
    let mut hub = hub.lock().map_err(|_| "authority lock poisoned")?;
    match message {
        Message::Hello {
            board,
            client,
            revision: _,
        } => join(&mut hub, id, board, client, out),
        Message::Publish {
            board,
            client,
            document,
            sources,
        } => {
            if hub.boards.contains_key(&board) || hub.root.join(format!("{board}.board")).exists() {
                return reply(
                    out,
                    refusal(None, 0, RefusalCode::Conflict, "board already exists"),
                );
            }
            if hub.boards.len() >= MAX_BOARDS {
                return reply(
                    out,
                    refusal(None, 0, RefusalCode::Busy, "board capacity reached"),
                );
            }
            let result = document
                .to_document()
                .map_err(|e| e.to_string())
                .and_then(|document| {
                    if document.id().value() != board.value() {
                        return Err("publish identity mismatch".into());
                    }
                    Authority::publish(&hub.root, document, sources)
                });
            match result {
                Ok(authority) => {
                    hub.boards.insert(board, authority);
                    join(&mut hub, id, board, client, out)
                }
                Err(e) => reply(out, refusal(None, 0, RefusalCode::InvalidOperation, &e)),
            }
        }
        Message::Edit {
            operation,
            base,
            command,
            sources,
        } => apply(
            &mut hub,
            id,
            operation,
            base,
            Some((command, sources)),
            false,
            out,
        ),
        Message::Undo { operation, base } => apply(&mut hub, id, operation, base, None, false, out),
        Message::Redo { operation, base } => apply(&mut hub, id, operation, base, None, true, out),
        _ => reply(
            out,
            refusal(None, 0, RefusalCode::Protocol, "unexpected client message"),
        ),
    }
}
fn join(hub: &mut Hub, id: usize, board: WireId, client: WireId, out: &Arc<Out>) -> Result<()> {
    if hub
        .subscribers
        .values()
        .filter(|s| s.board == board && s.client != client)
        .count()
        >= MAX_CLIENTS
        && !hub.subscribers.contains_key(&id)
    {
        return reply(
            out,
            refusal(None, 0, RefusalCode::Busy, "board client capacity reached"),
        );
    }
    hub.subscribers.retain(|other, s| {
        if *other != id && s.board == board && s.client == client {
            let _ = s.out.socket.shutdown(Shutdown::Both);
            false
        } else {
            true
        }
    });
    if hub.boards.get(&board).is_some_and(|a| !a.is_available()) {
        hub.boards.remove(&board);
    }
    if !hub.boards.contains_key(&board) {
        if hub.boards.len() >= MAX_BOARDS {
            return reply(
                out,
                refusal(None, 0, RefusalCode::Busy, "board capacity reached"),
            );
        }
        match Authority::open(&hub.root, board) {
            Ok(a) => {
                hub.boards.insert(board, a);
            }
            Err(e) => return reply(out, refusal(None, 0, RefusalCode::BoardUnavailable, &e)),
        }
    }
    hub.subscribers.insert(
        id,
        Subscriber {
            board,
            client,
            out: Arc::clone(out),
        },
    );
    let authority = hub.boards.get(&board).ok_or("board unavailable")?;
    reply(
        out,
        Message::Snapshot {
            board,
            revision: authority.revision(),
            source_high_water: authority.source_high_water(),
            document: authority.record().clone(),
            sources: authority.sources().to_vec(),
            clients: hub
                .subscribers
                .values()
                .filter(|s| s.board == board)
                .count() as u32,
        },
    )
}
fn apply(
    hub: &mut Hub,
    id: usize,
    operation: WireId,
    base: u64,
    command: Option<(tack_shared::CommandDto, Vec<tack_shared::SourceBinding>)>,
    redo: bool,
    out: &Out,
) -> Result<()> {
    let Some(subscriber) = hub.subscribers.get(&id) else {
        return reply(
            out,
            refusal(Some(operation), 0, RefusalCode::Protocol, "join required"),
        );
    };
    let (board, client) = (subscriber.board, subscriber.client);
    let authority = hub.boards.get_mut(&board).ok_or("board unavailable")?;
    // Exact duplicate delivery acknowledges its original receipt only to its sender.
    if let Some(change) = authority.receipt(client, operation) {
        return reply(out, accepted(board, change));
    }
    let result = match command {
        Some((command, sources)) => authority.edit(client, operation, base, command, sources),
        None => authority.history(client, operation, base, redo),
    };
    match result {
        Ok(change) => {
            let bytes = Arc::new(frame(&accepted(board, change))?);
            hub.subscribers.retain(|_, subscriber| {
                subscriber.board != board || subscriber.out.send(Arc::clone(&bytes))
            });
            Ok(())
        }
        Err(e) => {
            if !authority.is_available() {
                hub.subscribers.retain(|_, s| {
                    if s.board == board {
                        let _ = s.out.socket.shutdown(Shutdown::Both);
                        false
                    } else {
                        true
                    }
                });
                return Err("authority published but directory durability uncertain; reconnect reloads validated authority".into());
            }
            let code = if base != authority.revision() {
                RefusalCode::StaleRevision
            } else if e.starts_with("undo conflict") {
                RefusalCode::Conflict
            } else {
                RefusalCode::InvalidOperation
            };
            reply(
                out,
                refusal(Some(operation), authority.revision(), code, &e),
            )
        }
    }
}
fn accepted(board: WireId, change: Change) -> Message {
    Message::Accepted {
        board,
        revision: change.revision,
        client: change.client,
        operation: change.operation,
        command: change.command,
        sources: change.sources,
    }
}
fn bounded_reason(reason: &str) -> String {
    {
        let mut out = String::new();
        for c in reason.chars().filter(|c| !c.is_control()) {
            if out.len() + c.len_utf8() > 256 {
                break;
            }
            out.push(c);
        }
        out
    }
}
pub(super) fn refusal(
    operation: Option<WireId>,
    revision: u64,
    code: RefusalCode,
    reason: &str,
) -> Message {
    Message::Refused {
        operation,
        revision,
        code,
        reason: bounded_reason(reason),
    }
}

#[cfg(test)]
mod cleanup_tests {
    use super::discard_pending;
    use std::fs;

    #[test]
    fn pending_cleanup_preserves_unknown_and_nonregular_paths()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = std::env::temp_dir().join(format!(
            "tack-pending-cleanup-{}",
            tack_storage::new_document_id()?.value()
        ));
        let boards = root.join("boards");
        fs::create_dir_all(&boards)?;
        let recognized = boards.join("00000000000000000000000000000001.pending");
        fs::write(&recognized, b"interrupted")?;
        discard_pending(&boards)?;
        assert!(!recognized.exists());

        let unknown = boards.join("owner-notes.pending");
        fs::write(&unknown, b"preserve")?;
        assert!(discard_pending(&boards).is_err());
        assert_eq!(fs::read(&unknown)?, b"preserve");
        fs::remove_file(unknown)?;

        fs::create_dir(&recognized)?;
        assert!(discard_pending(&boards).is_err());
        assert!(recognized.is_dir());
        fs::remove_dir(&recognized)?;

        #[cfg(unix)]
        {
            let target = root.join("preserved-owner-file");
            fs::write(&target, b"keep target")?;
            std::os::unix::fs::symlink(&target, &recognized)?;
            assert!(discard_pending(&boards).is_err());
            assert!(fs::symlink_metadata(&recognized)?.file_type().is_symlink());
            assert_eq!(fs::read(target)?, b"keep target");
        }
        fs::remove_dir_all(root)?;
        Ok(())
    }
}
