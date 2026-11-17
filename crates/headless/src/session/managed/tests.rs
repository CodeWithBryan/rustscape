use super::*;
use rs09_protocol::game::updates;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::{net::TcpListener, task::yield_now};

async fn fixture(websocket: bool, pending: &[u8]) -> (ManagedSession, Reader, Writer) {
    let (session, reader, writer) = raw_fixture(websocket, pending).await;
    (session.into_managed(), reader, writer)
}
async fn raw_fixture(websocket: bool, pending: &[u8]) -> (Session, Reader, Writer) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (client, accepted) = tokio::join!(
        rs09_net::connect(listener.local_addr().unwrap(), websocket, BYTE_LIMIT),
        async {
            let (socket, _) = listener.accept().await.unwrap();
            rs09_net::accept(socket, websocket, BYTE_LIMIT)
                .await
                .unwrap()
        }
    );
    let (reader, writer) = client.unwrap();
    let (peer_reader, peer_writer) = accepted;
    let mut decoder = Decoder::new(Direction::ServerToClient, BYTE_LIMIT, 65535);
    decoder.push(pending).unwrap();
    let session = Session {
        metadata: WorldLogin {
            staff_level: 0,
            blackmarks: 0,
            underage: false,
            parental_chat_consent: false,
            parental_ad_consent: false,
            quick_chat: false,
            mouse_recording: false,
            player_index: 7,
            player_member: false,
            map_members: false,
        },
        reader,
        writer,
        decoder,
    };
    (session, peer_reader, peer_writer)
}
fn wire(frame: &Frame) -> Vec<u8> {
    let mut wire = vec![];
    frame
        .write(Direction::ServerToClient, None, &mut wire)
        .unwrap();
    wire
}
async fn terminal(session: &mut ManagedSession) -> Status {
    timeout(Duration::from_secs(3), async {
        while session.status() == Status::Connected {
            session.sender.status.changed().await.unwrap();
        }
        session.status()
    })
    .await
    .unwrap()
}
fn unhex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[test]
fn original_liveness_blocks_match_counters_wrapping_timeouts_and_failed_prefixes() {
    let corpus: Value = serde_json::from_str(include_str!(
        "../../../../../fixtures/headless-lifecycle/oracle.json"
    ))
    .unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let lock: Value =
        serde_json::from_slice(&std::fs::read(root.join("upstream.lock.json")).unwrap()).unwrap();
    assert_eq!(corpus["upstream"], lock["client"]);
    let source_path = root.join("reference/client/client/src/main/java/rt4/Protocol.java");
    // Portable tests need no reference checkout/JVM. The oracle command always
    // checks the actual Git pin/source and reproduces all fixture bytes.
    if source_path.exists() {
        let source = std::fs::read(source_path).unwrap();
        let normalized: Vec<_> = source.into_iter().filter(|b| *b != b'\r').collect();
        assert_eq!(
            corpus["sourceSha256"],
            format!("{:x}", Sha256::digest(normalized))
        );
    }
    assert_eq!(corpus["cases"].as_array().unwrap().len(), 136);
    for (index, case) in corpus["cases"].as_array().unwrap().iter().enumerate() {
        let mut cycles = Cycles {
            read: case["read"].as_i64().unwrap() as i32,
            write: case["write"].as_i64().unwrap() as i32,
        };
        let mut closed = false;
        let mut socket = "ok";
        let mut pending = vec![];
        let mut sent = vec![];
        let mut states = vec![];
        for op in case["ops"].as_array().unwrap() {
            match op[0].as_str().unwrap() {
                "R" => cycles.received(),
                "P" => pending.extend(unhex(op[1].as_str().unwrap())),
                "S" => socket = op[1].as_str().unwrap(),
                "T" => {
                    for _ in 0..op[1].as_u64().unwrap() {
                        if closed {
                            break;
                        }
                        match cycles.tick() {
                            Err(CloseReason::ReadTimeout) => {
                                closed = true;
                                continue;
                            }
                            Ok(true) => pending.push(93),
                            Ok(false) => {}
                            Err(reason) => panic!("unexpected {reason:?}"),
                        }
                        if socket != "null" && !pending.is_empty() {
                            if socket == "fail" {
                                closed = true;
                            } else {
                                sent.append(&mut pending);
                                cycles.written();
                            }
                        }
                    }
                }
                op => panic!("unknown {op}"),
            }
            states.push(json!([
                cycles.read,
                cycles.write,
                closed,
                hex(&sent),
                hex(&pending)
            ]));
        }
        assert_eq!(json!(states), case["expected"], "original scenario {index}");
    }
}

#[tokio::test]
async fn tcp_and_websocket_keepalive_without_consumer_and_preserve_fragmented_frames() {
    for websocket in [false, true] {
        let (mut session, mut reader, mut writer) = fixture(websocket, &[]).await;
        assert_eq!(session.metadata().player_index, 7);
        let sender = session.sender();
        // No next_frame call is active during the complete idle keepalive interval.
        let Chunk::Binary(bytes) = timeout(Duration::from_secs(3), reader.read())
            .await
            .unwrap()
            .unwrap()
        else {
            panic!("closed")
        };
        assert_eq!(&bytes[..], &[93]);
        assert_eq!(sender.stats().keepalives_written, 1);
        let frame = updates::synth_sound(2582, 1, 0);
        let bytes = wire(&frame);
        writer.write(bytes[..2].to_vec().into()).await.unwrap();
        assert!(
            timeout(Duration::from_millis(25), session.next_frame())
                .await
                .is_err()
        );
        writer.write(bytes[2..].to_vec().into()).await.unwrap();
        assert_eq!(session.next_frame().await.unwrap(), frame);
        assert!(
            sender
                .send_frame(&Frame {
                    opcode: 254,
                    payload: vec![]
                })
                .await
                .is_err()
        );
        let expected = sender.walk([3207, 3216], false).await.unwrap();
        let Chunk::Binary(bytes) = timeout(Duration::from_secs(1), reader.read())
            .await
            .unwrap()
            .unwrap()
        else {
            panic!("closed")
        };
        assert_eq!(&bytes[..], expected);
        assert_eq!(sender.stats().commands_written, 1);
        session.close().await.unwrap();
        assert_eq!(sender.status(), Status::Closed(CloseReason::Requested));
        assert!(sender.walk([1, 1], false).await.is_err());
    }
}

#[tokio::test]
async fn logout_preserves_complete_pending_frames_and_terminal_reason() {
    let first = updates::run_energy(42.0);
    let logout = Frame {
        opcode: 86,
        payload: vec![],
    };
    let pending = [wire(&first), wire(&logout)].concat();
    let (mut session, _reader, _writer) = fixture(false, &pending).await;
    assert_eq!(session.next_frame().await.unwrap(), first);
    assert_eq!(session.next_frame().await.unwrap(), logout);
    assert_eq!(
        session.next_frame().await.unwrap_err().kind(),
        io::ErrorKind::UnexpectedEof
    );
    assert_eq!(session.status(), Status::Closed(CloseReason::ServerLogout));
}

#[tokio::test]
async fn slow_consumer_overflow_preserves_exact_bounded_prefix() {
    let frame = updates::run_energy(42.0);
    let pending = wire(&frame).repeat(FRAME_LIMIT + 1);
    let (mut session, _reader, _writer) = fixture(false, &pending).await;
    assert_eq!(
        terminal(&mut session).await,
        Status::Closed(CloseReason::ObservationOverflow)
    );
    for _ in 0..FRAME_LIMIT {
        assert_eq!(session.next_frame().await.unwrap(), frame);
    }
    assert!(session.next_frame().await.is_err());
    assert_eq!(session.stats().frames_received, FRAME_LIMIT as u64);
}

#[tokio::test]
async fn payload_budget_closes_before_frame_count_limit() {
    let frame = Frame {
        opcode: 225,
        payload: vec![9; 65535],
    };
    let (mut session, _reader, mut writer) = fixture(false, &[]).await;
    writer.write(wire(&frame).repeat(16).into()).await.unwrap();
    assert_eq!(
        terminal(&mut session).await,
        Status::Closed(CloseReason::ObservationOverflow)
    );
    assert_eq!(session.stats().frames_received, 15);
    for _ in 0..15 {
        assert_eq!(session.next_frame().await.unwrap(), frame);
    }
    assert!(session.next_frame().await.is_err());
}

#[tokio::test(start_paused = true)]
async fn incomplete_packet_does_not_reset_original_receive_timeout() {
    let first = updates::run_energy(42.0);
    let pending = [wire(&first), vec![225, 0, 4, 1]].concat();
    let (mut session, _reader, _writer) = fixture(false, &pending).await;
    // Receiving this complete packet proves the driver's clock has started.
    assert_eq!(session.next_frame().await.unwrap(), first);
    for _ in 0..750 {
        tokio::time::advance(CYCLE).await;
        for _ in 0..4 {
            yield_now().await;
        }
    }
    assert_eq!(session.status(), Status::Connected);
    tokio::time::advance(CYCLE).await;
    for _ in 0..4 {
        yield_now().await;
    }
    assert_eq!(session.status(), Status::Closed(CloseReason::ReadTimeout));
    assert_eq!(
        session.next_frame().await.unwrap_err().kind(),
        io::ErrorKind::TimedOut
    );
}

#[tokio::test]
async fn dropping_connection_owner_retires_cloned_command_senders() {
    let (session, _reader, _writer) = fixture(false, &[]).await;
    let mut sender = session.sender();
    drop(session);
    timeout(Duration::from_secs(1), async {
        while sender.status() == Status::Connected {
            sender.status.changed().await.unwrap();
        }
    })
    .await
    .unwrap();
    assert_eq!(sender.status(), Status::Closed(CloseReason::Requested));
    assert!(sender.walk([3207, 3216], false).await.is_err());
}

#[tokio::test]
async fn stalled_write_does_not_block_incoming_frames_and_has_a_deadline() {
    struct Stalled {
        socket: Writer,
        entered: Arc<tokio::sync::Notify>,
    }
    impl Output for Stalled {
        async fn publish(&mut self, _wire: Vec<u8>) -> io::Result<()> {
            self.entered.notify_one();
            std::future::pending().await
        }
        async fn close(&mut self) -> io::Result<()> {
            self.socket.close(1000, "controlled stall complete").await
        }
    }
    let (raw, _unread_peer, mut writer) = raw_fixture(false, &[]).await;
    let entered = Arc::new(tokio::sync::Notify::new());
    let mut session = ManagedSession::drive(
        raw.metadata,
        raw.reader,
        Stalled {
            socket: raw.writer,
            entered: entered.clone(),
        },
        raw.decoder,
    );
    let (reply, mut result) = oneshot::channel();
    // Controlled pending write future; incoming packets still use a real socket.
    session
        .sender
        .commands
        .send(Write {
            wire: vec![93],
            reply: Some(reply),
            _budget: None,
        })
        .await
        .unwrap();
    timeout(Duration::from_secs(1), entered.notified())
        .await
        .unwrap();
    let incoming = updates::run_energy(42.0);
    writer.write(wire(&incoming).into()).await.unwrap();
    assert_eq!(
        timeout(Duration::from_secs(1), session.next_frame())
            .await
            .unwrap()
            .unwrap(),
        incoming
    );
    assert!(matches!(
        result.try_recv(),
        Err(oneshot::error::TryRecvError::Empty)
    ));
    tokio::time::pause();
    tokio::time::advance(WRITE_TIMEOUT + CYCLE).await;
    assert_eq!(
        timeout(CYCLE, result)
            .await
            .unwrap()
            .unwrap()
            .unwrap_err()
            .kind(),
        io::ErrorKind::TimedOut
    );
    assert_eq!(
        terminal(&mut session).await,
        Status::Closed(CloseReason::WriteTimeout)
    );
    tokio::time::resume();
}
