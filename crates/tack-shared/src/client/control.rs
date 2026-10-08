use super::*;
use crate::{Message, RefusalCode};
use std::sync::atomic::AtomicU64;

pub(super) fn run(context: Context, requests: Receiver<Request>, sender: SyncSender<Request>) {
    let mut stream = None;
    let mut reader = None;
    establish(&context, &sender, &mut stream, &mut reader, false);
    while let Ok(request) = requests.recv() {
        if context.cancel.load(Ordering::Relaxed) || matches!(request, Request::Stop) {
            break;
        }
        if matches!(request, Request::Reconnect) {
            context.state(ConnectionState::Reconnecting);
            context.shutdown();
            if let Some(reader) = reader.take() {
                let _ = reader.join();
            }
            stream = None;
            establish(&context, &sender, &mut stream, &mut reader, true);
            continue;
        }
        let ticket = match &request {
            Request::Edit {
                epoch,
                operation,
                base,
                ..
            }
            | Request::Undo {
                epoch,
                operation,
                base,
            }
            | Request::Redo {
                epoch,
                operation,
                base,
            } => Some((*epoch, *operation, *base)),
            _ => None,
        };
        if let Some((epoch, operation, base)) = ticket
            && epoch != context.epoch.load(Ordering::Acquire)
        {
            context.emit(ClientEvent::Refused {
                operation: Some(operation),
                revision: base,
                reason: "obsolete pre-reconnect operation discarded; not replayed".into(),
            });
            continue;
        }
        let message = match request {
            Request::Edit {
                epoch: _,
                operation,
                base,
                command,
                sources,
            } => Message::Edit {
                operation,
                base,
                command,
                sources,
            },
            Request::Undo {
                operation, base, ..
            } => Message::Undo { operation, base },
            Request::Redo {
                operation, base, ..
            } => Message::Redo { operation, base },
            Request::Rejoin => {
                context.state(ConnectionState::Reconnecting);
                Message::Hello {
                    board: context.config.board,
                    client: context.config.client,
                    revision: 0,
                }
            }
            _ => continue,
        };
        if !matches!(message, Message::Hello { .. }) && !context.connected.load(Ordering::Acquire) {
            context.emit(ClientEvent::Error(
                "operation not sent: shared board disconnected".into(),
            ));
            continue;
        }
        let result = stream
            .as_mut()
            .ok_or(Error::Invalid("shared connection unavailable"))
            .and_then(|stream| {
                crate::publish::send(
                    &mut stats::Observed {
                        socket: stream,
                        counters: Arc::clone(&context.counters),
                    },
                    &message,
                )
            });
        if let Err(error) = result {
            context.emit(ClientEvent::Error(error.to_string()));
            context.state(ConnectionState::Disconnected);
            context.shutdown();
        }
    }
    context.shutdown();
    if let Some(reader) = reader {
        let _ = reader.join();
    }
}
fn establish(
    context: &Context,
    sender: &SyncSender<Request>,
    stream: &mut Option<TcpStream>,
    reader: &mut Option<thread::JoinHandle<()>>,
    reconnect: bool,
) {
    context.state(if reconnect {
        ConnectionState::Reconnecting
    } else {
        ConnectionState::Connecting
    });
    let result = (|| -> Result<()> {
        crate::cache::owned_directory(&context.config.cache_dir)?;
        let mut writer = crate::publish::connect(&context.config.address, false)?;
        if context.cancel.load(Ordering::Relaxed) {
            return Err(Error::Invalid("shared client closed"));
        }
        crate::publish::send(
            &mut stats::Observed {
                socket: &mut writer,
                counters: Arc::clone(&context.counters),
            },
            &Message::Hello {
                board: context.config.board,
                client: context.config.client,
                revision: 0,
            },
        )?;
        let incoming = writer.try_clone()?;
        let shutdown = writer.try_clone()?;
        *context
            .socket
            .lock()
            .map_err(|_| Error::Invalid("connection state lock"))? = Some(shutdown);
        let state = context.clone();
        let sender = sender.clone();
        let handle = thread::Builder::new()
            .name("tack-shared-reader".into())
            .spawn(move || receive(state, incoming, sender))?;
        *stream = Some(writer);
        *reader = Some(handle);
        Ok(())
    })();
    if let Err(error) = result
        && !context.cancel.load(Ordering::Relaxed)
    {
        context.emit(ClientEvent::Error(error.to_string()));
        context.state(ConnectionState::ServerUnavailable);
    }
}
fn receive(context: Context, mut stream: TcpStream, requests: SyncSender<Request>) {
    let revision = AtomicU64::new(0);
    loop {
        if context.cancel.load(Ordering::Relaxed) {
            break;
        }
        let result = crate::publish::receive(&mut stats::Observed {
            socket: &mut stream,
            counters: Arc::clone(&context.counters),
        })
        .and_then(|message| consume(&context, &revision, &requests, message));
        if let Err(error) = result {
            if !context.cancel.load(Ordering::Relaxed) {
                context.emit(ClientEvent::Error(error.to_string()));
                context.state(if matches!(error, Error::Version(_)) {
                    ConnectionState::Refused
                } else {
                    ConnectionState::Disconnected
                });
            }
            context.shutdown();
            break;
        }
    }
}
fn consume(
    context: &Context,
    current: &AtomicU64,
    requests: &SyncSender<Request>,
    message: Message,
) -> Result<()> {
    match message {
        Message::Snapshot {
            board,
            revision,
            source_high_water,
            document,
            sources,
            clients,
        } => {
            if board != context.config.board {
                return Err(Error::Invalid("snapshot board mismatch"));
            }
            if context.snapshot_pending.load(Ordering::Acquire) {
                return Err(Error::Invalid(
                    "previous authoritative snapshot not consumed; reconnect required",
                ));
            }
            let document = document.to_document()?;
            if document.id().value() != board.value() {
                return Err(Error::Invalid("snapshot document identity mismatch"));
            }
            let (path, board) =
                crate::cache::snapshot_view(&context.config.cache_dir, &document, &sources)?;
            current.store(revision, Ordering::Release);
            context.snapshot_pending.store(true, Ordering::Release);
            if !context.emit(ClientEvent::Snapshot {
                revision,
                source_high_water,
                document,
                path,
                board,
                sources,
                clients,
            }) {
                context.snapshot_pending.store(false, Ordering::Release);
                return Err(Error::Invalid("shared event queue congested"));
            }
            context.state(ConnectionState::Connected);
        }
        Message::Accepted {
            board,
            revision,
            client,
            operation,
            command,
            sources,
        } => {
            if board != context.config.board {
                return Err(Error::Invalid("accepted board mismatch"));
            }
            let previous = current.load(Ordering::Acquire);
            if revision <= previous {
                return Ok(());
            } // Deduplicated receipt, never apply the durable edit twice.
            if revision
                != previous
                    .checked_add(1)
                    .ok_or(Error::Invalid("revision overflow"))?
            {
                return Err(Error::Invalid("authority revision gap; reconnect required"));
            }
            command.to_command()?;
            for source in &sources {
                source.validate()?;
            }
            current.store(revision, Ordering::Release);
            if !context.emit(ClientEvent::Accepted {
                revision,
                client,
                operation,
                command,
                sources,
            }) {
                return Err(Error::Invalid("shared event queue congested"));
            }
        }
        Message::Refused {
            operation,
            revision,
            code,
            reason,
        } => {
            context.emit(ClientEvent::Refused {
                operation,
                revision,
                reason,
            });
            match code {
                RefusalCode::VersionMismatch => context.state(ConnectionState::Refused),
                RefusalCode::BoardUnavailable => context.state(ConnectionState::BoardUnavailable),
                RefusalCode::StaleRevision => {
                    context.connected.store(false, Ordering::Release);
                    bounded_send(requests, Request::Rejoin)?;
                }
                _ => {}
            }
        }
        _ => return Err(Error::Invalid("unexpected control message")),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    fn accepted(revision: u64) -> Message {
        Message::Accepted {
            board: WireId::new(1).unwrap(),
            revision,
            client: WireId::new(2).unwrap(),
            operation: WireId::new(3).unwrap(),
            command: crate::CommandDto::from_command(&tack_core::Command::SetTransform {
                object: tack_core::ObjectId::new(9).unwrap(),
                transform: tack_core::Transform::new([0., 0.], [10., 10.], 0., [false; 2]).unwrap(),
            })
            .unwrap(),
            sources: vec![],
        }
    }
    #[test]
    fn duplicate_authority_receipt_is_not_applied_twice_and_gap_requires_rejoin() {
        let (context, events) = super::super::tests::context();
        let (requests, _rx) = mpsc::sync_channel(REQUESTS);
        let revision = AtomicU64::new(10);
        consume(&context, &revision, &requests, accepted(10)).unwrap();
        assert!(events.try_recv().is_err());
        consume(&context, &revision, &requests, accepted(11)).unwrap();
        assert!(matches!(
            events.try_recv(),
            Ok(ClientEvent::Accepted { revision: 11, .. })
        ));
        consume(&context, &revision, &requests, accepted(11)).unwrap();
        assert!(events.try_recv().is_err());
        assert!(consume(&context, &revision, &requests, accepted(13)).is_err());
    }
    #[test]
    fn stale_refusal_disables_edits_and_requests_an_authoritative_snapshot() {
        let (context, events) = super::super::tests::context();
        let (requests, rx) = mpsc::sync_channel(REQUESTS);
        consume(
            &context,
            &AtomicU64::new(10),
            &requests,
            Message::Refused {
                operation: Some(WireId::new(3).unwrap()),
                revision: 11,
                code: RefusalCode::StaleRevision,
                reason: "base revision is stale".into(),
            },
        )
        .unwrap();
        assert!(!context.connected.load(Ordering::Acquire));
        assert!(matches!(
            events.try_recv(),
            Ok(ClientEvent::Refused { revision: 11, .. })
        ));
        assert!(matches!(rx.try_recv(), Ok(Request::Rejoin)));
    }
}
