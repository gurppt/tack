//! Real headless processes; native renderer coverage is retained separately.
use std::{
    fs,
    net::{TcpListener, TcpStream},
    path::PathBuf,
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};
use tack_core::{Document, DocumentId, DocumentLimits, DocumentObject, ObjectId, Transform};
use tack_shared::{
    CommandDto, DocumentRecord, Message, RefusalCode, WireId, read_message, write_message,
};
type R = Result<(), Box<dyn std::error::Error>>;
struct Server {
    child: Child,
    root: PathBuf,
    address: String,
}
impl Server {
    fn start() -> Result<Self, Box<dyn std::error::Error>> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let address = listener.local_addr()?.to_string();
        drop(listener);
        let root = std::env::temp_dir().join(format!(
            "tack-lan-process-{}",
            tack_storage::new_document_id()?.value()
        ));
        let child = spawn(&address, &root)?;
        let server = Self {
            child,
            root,
            address,
        };
        server.connect()?;
        Ok(server)
    }
    fn connect(&self) -> Result<TcpStream, Box<dyn std::error::Error>> {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            match TcpStream::connect(&self.address) {
                Ok(s) => {
                    s.set_read_timeout(Some(Duration::from_secs(5)))?;
                    return Ok(s);
                }
                Err(e) => {
                    if Instant::now() > deadline {
                        return Err(e.into());
                    }
                    thread::sleep(Duration::from_millis(20));
                }
            }
        }
    }
    fn restart(&mut self) -> R {
        self.child.kill()?;
        self.child.wait()?;
        self.child = spawn(&self.address, &self.root)?;
        self.connect()?;
        Ok(())
    }
}
fn spawn(address: &str, root: &PathBuf) -> Result<Child, Box<dyn std::error::Error>> {
    Ok(Command::new(env!("CARGO_BIN_EXE_tack-server"))
        .arg("--listen")
        .arg(address)
        .arg("--root")
        .arg(root)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?)
}
impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = fs::remove_dir_all(&self.root);
    }
}
fn exchange(
    socket: &mut TcpStream,
    message: Message,
) -> Result<Message, Box<dyn std::error::Error>> {
    write_message(socket, &message)?;
    loop {
        let incoming = read_message(socket)?;
        if !matches!(incoming, Message::LeaseSnapshot { .. }) {
            return Ok(incoming);
        }
    }
}
fn rename(name: &str) -> Result<CommandDto, Box<dyn std::error::Error>> {
    Ok(CommandDto::from_command(
        &tack_core::Command::SetFrameName {
            object: ObjectId::new(2)?,
            name: name.into(),
        },
    )?)
}
fn snapshot(message: Message, revision: u64) -> R {
    assert!(matches!(message,Message::Snapshot{revision:r,..} if r==revision));
    Ok(())
}
#[test]
fn three_sessions_bidirectional_conflict_rejoin_restart_dedupe_and_assets() -> R {
    let mut server = Server::start()?;
    let board = WireId::new(1)?;
    let a = WireId::new(10)?;
    let b = WireId::new(11)?;
    let c = WireId::new(12)?;
    let mut document = Document::new(DocumentId::new(1)?, DocumentLimits::default());
    document.apply(tack_core::Command::AddObject {
        object: DocumentObject::frame(
            ObjectId::new(2)?,
            "initial".into(),
            Transform::new([0., 0.], [100., 50.], 0., [false, false])?,
        )?,
        index: 0,
    })?;
    let mut sa = server.connect()?;
    snapshot(
        exchange(
            &mut sa,
            Message::Publish {
                board,
                client: a,
                document: DocumentRecord::from_document(&document)?,
                sources: vec![],
            },
        )?,
        0,
    )?;
    let mut sb = server.connect()?;
    snapshot(
        exchange(
            &mut sb,
            Message::Hello {
                board,
                client: b,
                revision: 0,
            },
        )?,
        0,
    )?;
    let mut sc = server.connect()?;
    snapshot(
        exchange(
            &mut sc,
            Message::Hello {
                board,
                client: c,
                revision: 0,
            },
        )?,
        0,
    )?;
    assert!(matches!(
        read_message(&mut sb)?,
        Message::LeaseSnapshot { .. }
    ));
    assert!(matches!(
        read_message(&mut sc)?,
        Message::LeaseSnapshot { .. }
    ));
    let edit_a = Message::Edit {
        operation: WireId::new(20)?,
        base: 0,
        command: rename("A")?,
        sources: vec![],
    };
    assert!(matches!(
        exchange(&mut sa, edit_a.clone())?,
        Message::Accepted { revision: 1, .. }
    ));
    assert!(matches!(
        read_message(&mut sb)?,
        Message::Accepted { revision: 1, .. }
    ));
    assert!(matches!(
        read_message(&mut sc)?,
        Message::Accepted { revision: 1, .. }
    ));
    assert!(matches!(
        exchange(
            &mut sb,
            Message::Edit {
                operation: WireId::new(21)?,
                base: 0,
                command: rename("stale")?,
                sources: vec![]
            }
        )?,
        Message::Refused {
            code: RefusalCode::Conflict,
            revision: 1,
            ..
        }
    ));
    assert!(matches!(
        exchange(
            &mut sb,
            Message::Edit {
                operation: WireId::new(22)?,
                base: 1,
                command: rename("B")?,
                sources: vec![]
            }
        )?,
        Message::Accepted { revision: 2, .. }
    ));
    assert!(matches!(
        read_message(&mut sa)?,
        Message::Accepted { revision: 2, .. }
    ));
    assert!(matches!(
        read_message(&mut sc)?,
        Message::Accepted { revision: 2, .. }
    ));
    assert!(matches!(
        exchange(
            &mut sa,
            Message::Undo {
                operation: WireId::new(23)?,
                base: 2
            }
        )?,
        Message::Refused {
            code: RefusalCode::Conflict,
            ..
        }
    ));
    // A reconnect supersedes an old half-open socket of that same client identity.
    let mut replacement = server.connect()?;
    snapshot(
        exchange(
            &mut replacement,
            Message::Hello {
                board,
                client: a,
                revision: 0,
            },
        )?,
        2,
    )?;
    assert!(matches!(
        exchange(&mut replacement, edit_a.clone())?,
        Message::Accepted { revision: 1, .. }
    ));
    replacement.set_read_timeout(Some(Duration::from_millis(100)))?;
    assert!(read_message(&mut replacement).is_err());
    replacement.set_read_timeout(Some(Duration::from_secs(5)))?;
    let original = b"same imported content";
    let hash = tack_shared::ContentHash::digest(original);
    let mut transfer = server.connect()?;
    assert!(matches!(
        exchange(
            &mut transfer,
            Message::AssetBegin {
                hash: hash.clone(),
                size: original.len() as u64
            }
        )?,
        Message::AssetStatus { present: false, .. }
    ));
    assert!(
        matches!(exchange(&mut transfer,Message::AssetChunk {hash:hash.clone(),offset:0,bytes:tack_shared::encode_hex(original)})?,Message::AssetProgress {offset,..} if offset==original.len() as u64)
    );
    assert!(
        matches!(exchange(&mut transfer,Message::AssetCommit {hash:hash.clone()})?,Message::AssetReady {size,..} if size==original.len() as u64)
    );
    assert!(matches!(
        exchange(
            &mut transfer,
            Message::AssetBegin {
                hash: hash.clone(),
                size: original.len() as u64
            }
        )?,
        Message::AssetStatus { present: true, .. }
    ));
    assert!(
        matches!(exchange(&mut transfer,Message::AssetGet {hash:hash.clone(),offset:0,length:65_536})?,Message::AssetData {bytes,..} if tack_shared::decode_hex(&bytes,65_536)?==original)
    );
    drop(sa);
    drop(sb);
    drop(sc);
    drop(replacement);
    drop(transfer);
    server.restart()?;
    let mut after = server.connect()?;
    let message = exchange(
        &mut after,
        Message::Hello {
            board,
            client: a,
            revision: 1,
        },
    )?;
    if let Message::Snapshot {
        revision, document, ..
    } = message
    {
        assert_eq!(revision, 2);
        let d = document.to_document()?;
        assert!(
            matches!(d.object(ObjectId::new(2)?).ok_or("frame")?.kind(),tack_core::ObjectKind::Frame(name) if name=="B")
        );
    } else {
        return Err("missing snapshot".into());
    }
    assert!(matches!(
        exchange(&mut after, edit_a)?,
        Message::Accepted { revision: 1, .. }
    ));
    assert_eq!(fs::read_dir(server.root.join("assets"))?.count(), 1);
    Ok(())
}

#[test]
fn three_connections_scoped_edits_atomic_leases_expiry_and_disconnect() -> R {
    let server = Server::start()?;
    let board = WireId::new(100)?;
    let mut document = Document::new(DocumentId::new(100)?, DocumentLimits::default());
    for index in 0..3 {
        document.apply(tack_core::Command::AddObject {
            object: DocumentObject::frame(
                ObjectId::new(200 + index)?,
                "Frame".into(),
                Transform::new([index as f64 * 120., 0.], [100., 50.], 0., [false; 2])?,
            )?,
            index: index as usize,
        })?;
    }
    let mut sockets = [server.connect()?, server.connect()?, server.connect()?];
    for (i, socket) in sockets.iter_mut().enumerate() {
        let message = if i == 0 {
            Message::Publish {
                board,
                client: WireId::new(300)?,
                document: DocumentRecord::from_document(&document)?,
                sources: vec![],
            }
        } else {
            Message::Hello {
                board,
                client: WireId::new(300 + i as u128)?,
                revision: 0,
            }
        };
        snapshot(exchange(socket, message)?, 0)?;
        assert!(matches!(
            read_message(socket)?,
            Message::LeaseSnapshot { .. }
        ));
    }
    let operation = WireId::new(400)?;
    write_message(
        &mut sockets[0],
        &Message::LeaseAcquire {
            operation,
            base: 0,
            objects: vec![WireId::new(200)?],
        },
    )?;
    for socket in &mut sockets {
        assert!(
            matches!(read_message(socket)?, Message::LeaseChanged { operation: Some(id), ttl_ms: 5000, .. } if id == operation)
        );
    }
    assert!(matches!(
        exchange(
            &mut sockets[1],
            Message::LeaseAcquire {
                operation: WireId::new(401)?,
                base: 0,
                objects: vec![WireId::new(201)?, WireId::new(200)?]
            }
        )?,
        Message::LeaseDenied { .. }
    ));
    // The failed multi-object request did not reserve its free member.
    write_message(
        &mut sockets[2],
        &Message::LeaseAcquire {
            operation: WireId::new(402)?,
            base: 0,
            objects: vec![WireId::new(201)?],
        },
    )?;
    for socket in &mut sockets {
        assert!(matches!(
            read_message(socket)?,
            Message::LeaseChanged {
                operation: Some(_),
                ..
            }
        ));
    }
    let transform = |object: u128, x: f64| -> Result<CommandDto, Box<dyn std::error::Error>> {
        Ok(CommandDto::from_command(
            &tack_core::Command::SetTransform {
                object: ObjectId::new(object)?,
                transform: Transform::new([x, 0.], [100., 50.], 0., [false; 2])?,
            },
        )?)
    };
    // Foreign delete/edit cannot circumvent the lease.
    assert!(matches!(
        exchange(
            &mut sockets[1],
            Message::Edit {
                operation: WireId::new(403)?,
                base: 0,
                command: transform(200, 40.)?,
                sources: vec![]
            }
        )?,
        Message::Refused {
            code: RefusalCode::Busy,
            ..
        }
    ));
    for (client, object, x) in [(0, 200, 10.), (2, 201, 130.), (1, 202, 250.)] {
        write_message(
            &mut sockets[client],
            &Message::Edit {
                operation: WireId::new(410 + object)?,
                base: 0,
                command: transform(object, x)?,
                sources: vec![],
            },
        )?;
        for socket in &mut sockets {
            assert!(matches!(read_message(socket)?, Message::Accepted { .. }));
        }
    }
    // A connected but silent owner loses its reservation without idle traffic.
    thread::sleep(Duration::from_millis(5100));
    write_message(
        &mut sockets[1],
        &Message::LeaseAcquire {
            operation: WireId::new(420)?,
            base: 3,
            objects: vec![WireId::new(200)?],
        },
    )?;
    for socket in &mut sockets {
        assert!(matches!(
            read_message(socket)?,
            Message::LeaseChanged {
                operation: None,
                ..
            }
        ));
        assert!(matches!(
            read_message(socket)?,
            Message::LeaseChanged {
                operation: Some(_),
                ..
            }
        ));
    }
    sockets[1].shutdown(std::net::Shutdown::Both)?;
    assert!(matches!(
        read_message(&mut sockets[0])?,
        Message::LeaseChanged {
            operation: None,
            ..
        }
    ));
    assert!(matches!(
        read_message(&mut sockets[2])?,
        Message::LeaseChanged {
            operation: None,
            ..
        }
    ));
    Ok(())
}
