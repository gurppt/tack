//! Optional blocking TCP workers with bounded, nonblocking UI queues. No idle heartbeat.
mod control;
mod stats;
mod transfers;
use crate::{CommandDto, Error, Result, SourceBinding, WireId};
pub use stats::ClientStats;
use std::{
    net::{Shutdown, TcpStream},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, Receiver, SyncSender, TrySendError},
    },
    thread,
};
use tack_core::{Command, Document, SourceId};
use tack_storage::{Payload, TackFile};

const REQUESTS: usize = 4;
const EVENTS: usize = 8;
#[derive(Clone)]
pub struct ClientConfig {
    pub address: String,
    pub board: WireId,
    pub client: WireId,
    pub cache_dir: PathBuf,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
    Refused,
    ServerUnavailable,
    BoardUnavailable,
}
impl ConnectionState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Disconnected => "Disconnected",
            Self::Connecting => "Connecting",
            Self::Connected => "Connected",
            Self::Reconnecting => "Reconnecting",
            Self::Refused => "Refused",
            Self::ServerUnavailable => "Server unavailable",
            Self::BoardUnavailable => "Board unavailable",
        }
    }
}
pub enum ClientEvent {
    State(ConnectionState),
    Snapshot {
        revision: u64,
        source_high_water: u64,
        document: Document,
        path: PathBuf,
        board: Arc<TackFile>,
        sources: Vec<SourceBinding>,
        clients: u32,
    },
    Accepted {
        revision: u64,
        client: WireId,
        operation: WireId,
        command: CommandDto,
        sources: Vec<SourceBinding>,
    },
    Refused {
        operation: Option<WireId>,
        revision: u64,
        reason: String,
    },
    AssetReady {
        binding: SourceBinding,
        path: PathBuf,
    },
    AssetFailed {
        binding: SourceBinding,
        reason: String,
    },
    AssetEvicted {
        hashes: Vec<crate::ContentHash>,
    },
    Error(String),
}
enum Request {
    Edit {
        epoch: u64,
        operation: WireId,
        base: u64,
        command: CommandDto,
        sources: Vec<SourceBinding>,
    },
    Undo {
        epoch: u64,
        operation: WireId,
        base: u64,
    },
    Redo {
        epoch: u64,
        operation: WireId,
        base: u64,
    },
    Reconnect,
    Rejoin,
    Stop,
}
enum Transfer {
    Asset(SourceBinding),
    Prepare {
        epoch: u64,
        operation: WireId,
        base: u64,
        command: Command,
        originals: Vec<(SourceId, u64, Payload)>,
        path: PathBuf,
    },
}
#[derive(Clone)]
struct Context {
    config: ClientConfig,
    events: SyncSender<ClientEvent>,
    notify: Arc<dyn Fn() + Send + Sync>,
    cancel: Arc<AtomicBool>,
    connected: Arc<AtomicBool>,
    overflow: Arc<AtomicBool>,
    epoch: Arc<AtomicU64>,
    snapshot_pending: Arc<AtomicBool>,
    socket: Arc<Mutex<Option<TcpStream>>>,
    counters: Arc<stats::Counters>,
}
impl Context {
    fn emit(&self, event: ClientEvent) -> bool {
        match self.events.try_send(event) {
            Ok(()) => {
                (self.notify)();
                true
            }
            Err(_) => {
                self.overflow.store(true, Ordering::Release);
                self.epoch.fetch_add(1, Ordering::AcqRel);
                self.connected.store(false, Ordering::Release);
                self.shutdown();
                (self.notify)();
                false
            }
        }
    }
    fn state(&self, state: ConnectionState) {
        if matches!(
            state,
            ConnectionState::Disconnected
                | ConnectionState::Reconnecting
                | ConnectionState::Refused
                | ConnectionState::ServerUnavailable
                | ConnectionState::BoardUnavailable
        ) {
            self.epoch.fetch_add(1, Ordering::AcqRel);
        }
        self.connected
            .store(state == ConnectionState::Connected, Ordering::Release);
        self.emit(ClientEvent::State(state));
    }
    fn shutdown(&self) {
        if let Ok(socket) = self.socket.lock()
            && let Some(socket) = socket.as_ref()
        {
            let _ = socket.shutdown(Shutdown::Both);
        }
    }
}
pub struct SharedClient {
    context: Context,
    requests: SyncSender<Request>,
    transfers: SyncSender<Transfer>,
    prepares: SyncSender<Transfer>,
    events: Receiver<ClientEvent>,
    workers: Vec<thread::JoinHandle<()>>,
}
impl SharedClient {
    pub fn start(config: ClientConfig, notify: impl Fn() + Send + Sync + 'static) -> Result<Self> {
        if config.address.len() > 256 || !config.cache_dir.is_absolute() {
            return Err(Error::Invalid("shared client configuration"));
        }
        let (events_tx, events) = mpsc::sync_channel(EVENTS);
        let (requests, rx) = mpsc::sync_channel(REQUESTS);
        let (transfers, transfer_rx) = mpsc::sync_channel(REQUESTS);
        let (prepares, prepare_rx) = mpsc::sync_channel(REQUESTS);
        let context = Context {
            config,
            events: events_tx,
            notify: Arc::new(notify),
            cancel: Arc::new(AtomicBool::new(false)),
            connected: Arc::new(AtomicBool::new(false)),
            overflow: Arc::new(AtomicBool::new(false)),
            epoch: Arc::new(AtomicU64::new(0)),
            snapshot_pending: Arc::new(AtomicBool::new(false)),
            socket: Arc::new(Mutex::new(None)),
            counters: Arc::default(),
        };
        let control_context = context.clone();
        let control_requests = requests.clone();
        let writer = thread::Builder::new()
            .name("tack-shared-control".into())
            .spawn(move || control::run(control_context, rx, control_requests))?;
        let transfer_context = context.clone();
        let transfer_requests = requests.clone();
        let worker = match thread::Builder::new()
            .name("tack-shared-assets".into())
            .spawn(move || transfers::run(transfer_context, transfer_rx, transfer_requests))
        {
            Ok(worker) => worker,
            Err(error) => {
                context.cancel.store(true, Ordering::Relaxed);
                context.shutdown();
                let _ = requests.try_send(Request::Stop);
                return Err(error.into());
            }
        };
        let prepare_context = context.clone();
        let prepare_requests = requests.clone();
        let prepare_worker = match thread::Builder::new()
            .name("tack-shared-publish".into())
            .spawn(move || transfers::run(prepare_context, prepare_rx, prepare_requests))
        {
            Ok(worker) => worker,
            Err(error) => {
                context.cancel.store(true, Ordering::Relaxed);
                context.shutdown();
                let _ = requests.try_send(Request::Stop);
                return Err(error.into());
            }
        };
        Ok(Self {
            context,
            requests,
            transfers,
            prepares,
            events,
            workers: vec![writer, worker, prepare_worker],
        })
    }
    pub fn connected(&self) -> bool {
        self.context.connected.load(Ordering::Acquire)
    }
    pub fn stats(&self) -> ClientStats {
        self.context.counters.snapshot()
    }
    pub fn cache_dir(&self) -> &std::path::Path {
        &self.context.config.cache_dir
    }
    pub fn drain(&self) -> Vec<ClientEvent> {
        let mut events: Vec<_> = self.events.try_iter().take(EVENTS).collect();
        if events
            .iter()
            .any(|event| matches!(event, ClientEvent::Snapshot { .. }))
        {
            self.context
                .snapshot_pending
                .store(false, Ordering::Release);
        }
        if self.context.overflow.swap(false, Ordering::AcqRel) {
            events.push(ClientEvent::Error(
                "shared event queue congested; disconnected until reconciliation".into(),
            ));
            events.push(ClientEvent::State(ConnectionState::Disconnected));
        }
        events
    }
    fn send(&self, request: Request) -> Result<()> {
        if !self.connected() {
            return Err(Error::Invalid("shared board disconnected/read-only"));
        }
        bounded_send(&self.requests, request)
    }
    pub fn edit(&self, base: u64, command: CommandDto) -> Result<WireId> {
        self.edit_sources(base, command, vec![])
    }
    pub fn edit_sources(
        &self,
        base: u64,
        command: CommandDto,
        sources: Vec<SourceBinding>,
    ) -> Result<WireId> {
        let operation = new_operation()?;
        self.send(Request::Edit {
            epoch: self.context.epoch.load(Ordering::Acquire),
            operation,
            base,
            command,
            sources,
        })?;
        Ok(operation)
    }
    pub fn prepare_edit(
        &self,
        base: u64,
        command: Command,
        originals: Vec<(SourceId, u64, Payload)>,
        path: PathBuf,
    ) -> Result<WireId> {
        if !self.connected() {
            return Err(Error::Invalid("shared board disconnected/read-only"));
        }
        if command.retained_bytes() > 8 * 1024 * 1024 || originals.len() > 4096 {
            return Err(Error::Invalid("shared edit preparation metadata budget"));
        }
        let operation = new_operation()?;
        bounded_send(
            &self.prepares,
            Transfer::Prepare {
                epoch: self.context.epoch.load(Ordering::Acquire),
                operation,
                base,
                command,
                originals,
                path,
            },
        )?;
        Ok(operation)
    }
    pub fn undo(&self, base: u64) -> Result<WireId> {
        let operation = new_operation()?;
        self.send(Request::Undo {
            epoch: self.context.epoch.load(Ordering::Acquire),
            operation,
            base,
        })?;
        Ok(operation)
    }
    pub fn redo(&self, base: u64) -> Result<WireId> {
        let operation = new_operation()?;
        self.send(Request::Redo {
            epoch: self.context.epoch.load(Ordering::Acquire),
            operation,
            base,
        })?;
        Ok(operation)
    }
    pub fn reconnect(&self) -> Result<()> {
        bounded_send(&self.requests, Request::Reconnect)
    }
    pub fn request_asset(&self, binding: SourceBinding) -> Result<()> {
        binding.validate()?;
        if !self.connected() {
            return Err(Error::Invalid(
                "shared source unavailable while disconnected",
            ));
        }
        bounded_send(&self.transfers, Transfer::Asset(binding))
    }
}
fn bounded_send<T>(sender: &SyncSender<T>, value: T) -> Result<()> {
    sender.try_send(value).map_err(|error| match error {
        TrySendError::Full(_) => {
            Error::Invalid("shared worker queue full; operation not submitted")
        }
        TrySendError::Disconnected(_) => Error::Invalid("shared worker stopped"),
    })
}
fn new_operation() -> Result<WireId> {
    WireId::new(tack_storage::new_document_id()?.value())
}
impl Drop for SharedClient {
    fn drop(&mut self) {
        self.context.cancel.store(true, Ordering::Relaxed);
        self.context.connected.store(false, Ordering::Release);
        // No UI-thread join or filesystem cleanup. Shutdown interrupts the blocking reader.
        if let Ok(socket) = self.context.socket.try_lock()
            && let Some(socket) = socket.as_ref()
        {
            let _ = socket.shutdown(Shutdown::Both);
        }
        let _ = self.requests.try_send(Request::Stop);
        let workers = std::mem::take(&mut self.workers);
        let context = self.context.clone();
        let _ = thread::Builder::new()
            .name("tack-shared-close".into())
            .spawn(move || {
                context.shutdown();
                for worker in workers {
                    let _ = worker.join();
                }
            });
    }
}

#[cfg(test)]
mod tests;
