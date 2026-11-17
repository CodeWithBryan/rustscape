use bytes::Bytes;
use rs09_cache::{
    Limits,
    store::{StorageMode, Store},
};
use rs09_net::{Chunk, Reader, login::LoginPeer};
use rs09_protocol::{
    game::{self, Direction, Frame},
    login::{AuthResponse, LoginRequest, RsaKey},
    profile::RS09_REVISION_530,
};
use rs09_server::{
    auth::{Authenticator, Mode, password::Passwords, storage::InMemoryStorage},
    catalog::Catalog,
    definitions::item::Shared,
    player::Prepared,
    runtime::{Config, Service},
    world::{
        doors::Location,
        player_render::{self, Handle, Player},
        rebuild,
        session::{self, World},
    },
};
use std::{
    path::PathBuf,
    sync::{Arc, atomic::Ordering},
    time::Duration,
};
use tokio::{
    sync::{Notify, mpsc},
    time::timeout,
};

struct Renderer {
    frames: Vec<Frame>,
}
impl player_render::Context for Renderer {
    fn candidates(&mut self, _: Location, _: i32) -> player_render::Result<Vec<Option<Handle>>> {
        Ok(vec![])
    }
    fn masks(
        &mut self,
        _: &Handle,
        _: &Handle,
        _: &mut Vec<u8>,
        _: bool,
        _: bool,
    ) -> player_render::Result<()> {
        panic!("mask not part of transport fixture")
    }
    fn send(&mut self, _: &Handle, f: Frame) -> player_render::Result<()> {
        self.frames.push(f);
        Ok(())
    }
}
impl rebuild::Context for Renderer {
    fn keys(&mut self, id: i32) -> player_render::Result<[i32; 4]> {
        Ok([id, -id, i32::MIN, i32::MAX])
    }
    fn send(&mut self, _: &Handle, f: Frame) -> player_render::Result<()> {
        self.frames.push(f);
        Ok(())
    }
}
fn initial_frames() -> Vec<Frame> {
    let p = Shared::new(Player::new(7, Location::new(3207, 3217, 0), 2000));
    p.write().update_scene = true;
    let mut r = Renderer { frames: vec![] };
    rebuild::send(&mut r, &p).unwrap();
    player_render::render(&mut r, &p).unwrap();
    r.frames
}
struct FixtureWorld {
    ready: Arc<Notify>,
    entered: mpsc::UnboundedSender<String>,
    seen: mpsc::UnboundedSender<Frame>,
}
impl World for FixtureWorld {
    fn is_online(&self, name: &str) -> bool {
        name == "already"
    }
    fn enter(&self, p: Prepared) -> session::Admission<'_> {
        Box::pin(async move {
            self.entered.send(p.login.username.clone()).unwrap();
            self.ready.notified().await;
            let (incoming, mut input) = mpsc::channel(1);
            let (output, outgoing) = mpsc::channel(1);
            let seen = self.seen.clone();
            let rights = p.details.rights().unwrap().ordinal();
            tokio::spawn(async move {
                output.send(initial_frames()).await.unwrap();
                while let Some(frame) = input.recv().await {
                    let _ = seen.send(frame);
                    if output
                        .send(vec![Frame {
                            opcode: 234,
                            payload: vec![100],
                        }])
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
            });
            Ok(session::Joined {
                rights,
                index: 7,
                incoming,
                outgoing,
            })
        })
    }
}
fn service(world: Arc<dyn World>) -> Arc<Service> {
    let limits = Limits::default();
    let folder = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/cache");
    Service::new(
        Arc::new(
            Catalog::new(
                Store::open(&folder, StorageMode::Resident, limits).unwrap(),
                limits,
                128,
                1_000_000,
            )
            .unwrap(),
        ),
        Config {
            authenticator: Some(Arc::new(Authenticator::new(
                Mode::Development {
                    default_admin: false,
                },
                Arc::new(InMemoryStorage::default()),
                Arc::new(Passwords::with_salt_source(4, || Ok([0; 16])).unwrap()),
            ))),
            world: Some(world),
            pending_replies: 1,
            ..Config::default()
        },
        530,
    )
    .unwrap()
}
async fn read_frames(reader: &mut Reader, decoder: &mut game::Decoder, n: usize) -> Vec<Frame> {
    timeout(Duration::from_secs(5), async {
        let mut out = vec![];
        loop {
            while let Some(frame) = decoder.next_frame().unwrap() {
                out.push(frame);
                if out.len() == n {
                    return out;
                }
            }
            match reader.read().await.unwrap() {
                Chunk::Binary(bytes) => decoder.push(&bytes).unwrap(),
                _ => panic!("world closed"),
            }
        }
    })
    .await
    .unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn native_world_admission_holds_success_then_switches_tcp_and_websocket_to_game_frames() {
    for websocket in [false, true] {
        let ready = Arc::new(Notify::new());
        let (entered, mut entries) = mpsc::unbounded_channel();
        let (seen, mut actions) = mpsc::unbounded_channel();
        let service = service(Arc::new(FixtureWorld {
            ready: ready.clone(),
            entered,
            seen,
        }));
        let server = service
            .start(
                "127.0.0.1:0".parse().unwrap(),
                Some("127.0.0.1:0".parse().unwrap()),
            )
            .await
            .unwrap();
        let address = if websocket {
            server.websocket.unwrap()
        } else {
            server.tcp
        };
        let mut peer = LoginPeer::connect(address, websocket, b"worldtest")
            .await
            .unwrap();
        let login = tokio::spawn(async move {
            let reply = peer
                .submit(
                    LoginRequest {
                        username: b"worldtest",
                        password: b"test",
                        ..LoginRequest::default()
                    },
                    RS09_REVISION_530,
                    &RsaKey::reference_public(),
                )
                .await
                .unwrap();
            (peer, reply)
        });
        assert_eq!(
            timeout(Duration::from_secs(5), entries.recv())
                .await
                .unwrap()
                .unwrap(),
            "worldtest"
        );
        assert!(!login.is_finished());
        ready.notify_one();
        let (peer, reply) = timeout(Duration::from_secs(5), login)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(reply.code, 2);
        assert_eq!(reply.world.unwrap().player_index, 7);
        let (mut reader, mut writer, pending) = peer.into_transport();
        let mut decoder = game::Decoder::new(Direction::ServerToClient, 65536, 65535);
        decoder.push(&pending).unwrap();
        assert_eq!(
            read_frames(&mut reader, &mut decoder, 2).await,
            initial_frames()
        );
        let frame = Frame {
            opcode: 22,
            payload: vec![1],
        };
        let mut wire = vec![];
        for _ in 0..8 {
            frame
                .write(Direction::ClientToServer, None, &mut wire)
                .unwrap();
        }
        for part in wire.chunks(3) {
            writer.write(Bytes::copy_from_slice(part)).await.unwrap();
        }
        let returned = read_frames(&mut reader, &mut decoder, 8).await;
        assert!(returned.iter().all(|f| f
            == &Frame {
                opcode: 234,
                payload: vec![100]
            }));
        for _ in 0..8 {
            assert_eq!(
                timeout(Duration::from_secs(5), actions.recv())
                    .await
                    .unwrap()
                    .unwrap(),
                frame
            );
        }
        writer.close(1000, "done").await.unwrap();
        server.shutdown().await.unwrap();
        assert_eq!(service.metrics.active.load(Ordering::Relaxed), 0);
        assert_eq!(service.metrics.queued_bytes.load(Ordering::Relaxed), 0);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn disconnect_cancels_pending_world_admission_and_releases_connection_slots() {
    let ready = Arc::new(Notify::new());
    let (entered, mut entries) = mpsc::unbounded_channel();
    let (seen, _) = mpsc::unbounded_channel();
    let service = service(Arc::new(FixtureWorld {
        ready,
        entered,
        seen,
    }));
    let server = service
        .start("127.0.0.1:0".parse().unwrap(), None)
        .await
        .unwrap();
    let mut peer = LoginPeer::connect(server.tcp, false, b"cancel")
        .await
        .unwrap();
    let login = tokio::spawn(async move {
        peer.submit(
            LoginRequest {
                username: b"cancel",
                password: b"test",
                ..LoginRequest::default()
            },
            RS09_REVISION_530,
            &RsaKey::reference_public(),
        )
        .await
    });
    timeout(Duration::from_secs(5), entries.recv())
        .await
        .unwrap()
        .unwrap();
    login.abort();
    let _ = login.await;
    timeout(Duration::from_secs(5), async {
        while service.metrics.active.load(Ordering::Relaxed) != 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(service.metrics.queued_bytes.load(Ordering::Relaxed), 0);
    server.shutdown().await.unwrap();
}

#[test]
fn handoff_keeps_bytes_coalesced_after_login_request() {
    use rs09_protocol::session::{Decoder, Incoming};
    let mut d = Decoder::new(4096, 32767);
    d.push(&[14, 0, 16, 0, 1, 99, 22, 1, 22, 0]).unwrap();
    assert!(matches!(
        d.next_packet().unwrap(),
        Some(Incoming::LoginHandshake { .. })
    ));
    assert_eq!(
        d.next_packet().unwrap(),
        Some(Incoming::LoginPacket(vec![16, 0, 1, 99]))
    );
    assert_eq!(d.into_buffered(), [22, 1, 22, 0]);
    assert_eq!(AuthResponse::Success as u8, 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn online_world_names_are_rejected_before_account_and_profile_work() {
    let (entered, _) = mpsc::unbounded_channel();
    let (seen, _) = mpsc::unbounded_channel();
    let service = service(Arc::new(FixtureWorld {
        ready: Arc::new(Notify::new()),
        entered,
        seen,
    }));
    let server = service
        .start("127.0.0.1:0".parse().unwrap(), None)
        .await
        .unwrap();
    let mut peer = LoginPeer::connect(server.tcp, false, b"already")
        .await
        .unwrap();
    let reply = peer
        .submit(
            LoginRequest {
                username: b"already",
                password: b"test",
                ..LoginRequest::default()
            },
            RS09_REVISION_530,
            &RsaKey::reference_public(),
        )
        .await
        .unwrap();
    assert_eq!(reply.code, AuthResponse::AlreadyOnline as u8);
    assert!(reply.world.is_none());
    assert_eq!(service.metrics.account_checks.load(Ordering::Relaxed), 0);
    assert_eq!(service.metrics.metadata_prepared.load(Ordering::Relaxed), 0);
    server.shutdown().await.unwrap();
}

struct DeclineWorld(AuthResponse);
impl World for DeclineWorld {
    fn enter(&self, _: Prepared) -> session::Admission<'_> {
        let response = self.0;
        Box::pin(async move { Err(response) })
    }
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rejected_world_admission_uses_original_nonce_detail_and_never_forges_success() {
    for response in [AuthResponse::ErrorLoadingProfile, AuthResponse::Success] {
        let service = service(Arc::new(DeclineWorld(response)));
        let server = service
            .start("127.0.0.1:0".parse().unwrap(), None)
            .await
            .unwrap();
        let mut peer = LoginPeer::connect(server.tcp, false, b"decline")
            .await
            .unwrap();
        let key = peer.server_key;
        let reply = peer
            .submit(
                LoginRequest {
                    username: b"decline",
                    password: b"test",
                    ..LoginRequest::default()
                },
                RS09_REVISION_530,
                &RsaKey::reference_public(),
            )
            .await
            .unwrap();
        assert!(reply.world.is_none());
        if response == AuthResponse::Success {
            assert_eq!(reply.code, AuthResponse::UnexpectedError as u8);
            assert_eq!(reply.detail, None);
        } else {
            assert_eq!(reply.code, response as u8);
            assert_eq!(reply.detail, Some(key as u8));
        }
        server.shutdown().await.unwrap();
    }
}
