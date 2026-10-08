#![allow(clippy::unwrap_used)]
use super::*;
pub(super) fn context() -> (Context, Receiver<ClientEvent>) {
    let (events, rx) = mpsc::sync_channel(EVENTS);
    (
        Context {
            config: ClientConfig {
                address: "127.0.0.1:1".into(),
                board: WireId::new(1).unwrap(),
                client: WireId::new(2).unwrap(),
                cache_dir: std::env::temp_dir().join("unused-shared-queue-test"),
            },
            events,
            notify: Arc::new(|| {}),
            cancel: Arc::new(AtomicBool::new(false)),
            connected: Arc::new(AtomicBool::new(true)),
            overflow: Arc::new(AtomicBool::new(false)),
            epoch: Arc::new(AtomicU64::new(0)),
            snapshot_pending: Arc::new(AtomicBool::new(false)),
            socket: Arc::new(Mutex::new(None)),
            counters: Arc::default(),
        },
        rx,
    )
}
#[test]
fn bounded_event_overflow_is_explicitly_disconnected_at_drain() {
    let (context, events) = context();
    let (requests, _rx) = mpsc::sync_channel(REQUESTS);
    let (transfers, _rx) = mpsc::sync_channel(REQUESTS);
    let (prepares, _rx) = mpsc::sync_channel(REQUESTS);
    for _ in 0..EVENTS {
        assert!(context.emit(ClientEvent::State(ConnectionState::Connected)));
    }
    assert!(!context.emit(ClientEvent::Error("overflow".into())));
    assert!(!context.connected.load(Ordering::Acquire));
    let client = SharedClient {
        context,
        requests,
        transfers,
        prepares,
        events,
        workers: vec![],
    };
    let drained = client.drain();
    assert_eq!(drained.len(), EVENTS + 2);
    assert!(matches!(
        drained.last(),
        Some(ClientEvent::State(ConnectionState::Disconnected))
    ));
    assert_eq!(client.drain().len(), 0);
    assert!(client.undo(0).is_err());
}
#[test]
fn idle_counters_change_only_for_actual_wire_io() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let peer = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        crate::read_message(&mut stream).unwrap()
    });
    let mut socket = TcpStream::connect(address).unwrap();
    let counters = Arc::new(stats::Counters::default());
    let message = crate::Message::AssetCancel {
        hash: crate::ContentHash::digest(b"original"),
    };
    crate::publish::send(
        &mut stats::Observed {
            socket: &mut socket,
            counters: Arc::clone(&counters),
        },
        &message,
    )
    .unwrap();
    assert_eq!(peer.join().unwrap(), message);
    let receipt = counters.snapshot();
    assert!(receipt.bytes_sent > 0);
    assert_eq!(receipt.messages_sent, 1);
    assert_eq!(receipt.bytes_received, 0);
    assert_eq!(counters.snapshot().bytes_sent, receipt.bytes_sent);
}
