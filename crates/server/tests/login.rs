use bytes::{Bytes, BytesMut};
use rs09_cache::{
    Limits,
    store::{StorageMode, Store},
};
use rs09_game_core::random::JavaRandom;
use rs09_net::{Chunk, Reader, Writer, login::LoginPeer, peer::CachePeer};
use rs09_protocol::{
    js5::Request,
    login::{self, AuthResponse, LoginRequest, RsaKey},
    profile::RS09_REVISION_530,
};
use rs09_server::{
    catalog::Catalog,
    runtime::{Config, Service},
};
use serde_json::Value;
use std::{
    path::PathBuf,
    sync::{Arc, atomic::Ordering},
    time::Duration,
};
use tokio::{net::TcpListener, time::timeout};

fn service(compute_jobs: usize) -> Arc<Service> {
    let limits = Limits::default();
    let folder = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/cache");
    let store = Store::open(&folder, StorageMode::Resident, limits).unwrap();
    Service::new(
        Arc::new(Catalog::new(store, limits, 128, 1_000_000).unwrap()),
        Config {
            compute_jobs,
            ..Config::default()
        },
        530,
    )
    .unwrap()
}
async fn take(reader: &mut Reader, pending: &mut BytesMut, count: usize) -> Bytes {
    while pending.len() < count {
        match timeout(Duration::from_secs(5), reader.read())
            .await
            .unwrap()
            .unwrap()
        {
            Chunk::Binary(bytes) => pending.extend_from_slice(&bytes),
            _ => panic!("missing binary reply"),
        }
    }
    pending.split_to(count).freeze()
}
fn packet(case: &Value) -> Vec<u8> {
    let hex = case["packet"].as_str().unwrap();
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn original_login_corpus_gets_exact_parser_responses_on_tcp_and_websocket() {
    let service = service(2);
    let server = service
        .start(
            "127.0.0.1:0".parse().unwrap(),
            Some("127.0.0.1:0".parse().unwrap()),
        )
        .await
        .unwrap();
    let mut attempts = 0;
    for websocket in [false, true] {
        let address = if websocket {
            server.websocket.unwrap()
        } else {
            server.tcp
        };
        for case in include_str!("../../../fixtures/login/oracle.jsonl")
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).unwrap())
        {
            if !["client_frame", "decode"].contains(&case["kind"].as_str().unwrap())
                || case["online"] == true
            {
                continue;
            }
            let packet = packet(&case);
            if packet.len() < 3
                || i16::from_be_bytes([packet[1], packet[2]]) as isize != packet.len() as isize - 3
            {
                continue;
            }
            let (mut reader, mut writer) = rs09_net::connect(address, websocket, 64 * 1024)
                .await
                .unwrap();
            let mut pending = BytesMut::new();
            writer.write(Bytes::from_static(&[14])).await.unwrap();
            writer.write(Bytes::from_static(&[0])).await.unwrap();
            assert_eq!(take(&mut reader, &mut pending, 9).await[0], 0);
            // Exercise fragmentation independently of TCP/WS message boundaries.
            for chunk in packet.chunks(7) {
                writer.write(Bytes::copy_from_slice(chunk)).await.unwrap();
            }
            let code = case["expected"]["response"].as_u64().unwrap() as u8;
            let code = if code == 2 { 8 } else { code };
            assert_eq!(
                take(&mut reader, &mut pending, 1).await[0],
                code,
                "case {}, ws={websocket}",
                case["id"]
            );
            assert!(matches!(
                timeout(Duration::from_secs(5), reader.read())
                    .await
                    .unwrap()
                    .unwrap(),
                Chunk::Closed
            ));
            attempts += 1;
        }
    }
    assert!(attempts > 600);
    server.shutdown().await.unwrap();
    assert_eq!(
        service.metrics.login_packets.load(Ordering::Relaxed),
        attempts
    );
    assert_eq!(service.metrics.active.load(Ordering::Relaxed), 0);
    assert_eq!(service.metrics.queued_bytes.load(Ordering::Relaxed), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn native_login_peer_uses_the_server_nonce_and_reports_authentication_unavailable() {
    let service = service(2);
    let server = service
        .start(
            "127.0.0.1:0".parse().unwrap(),
            Some("127.0.0.1:0".parse().unwrap()),
        )
        .await
        .unwrap();
    let mut rng = JavaRandom::new(530);
    for websocket in [false, true] {
        let address = if websocket {
            server.websocket.unwrap()
        } else {
            server.tcp
        };
        let mut peer = LoginPeer::connect(address, websocket, b"Rust_Client")
            .await
            .unwrap();
        assert_eq!(peer.server_key, rng.next_i64());
        let request = LoginRequest {
            username: b"Rust_Client",
            password: b"test_password",
            isaac_seed: [1, 2, 3, 4],
            ..LoginRequest::default()
        };
        let reply = peer
            .submit(request, RS09_REVISION_530, &RsaKey::reference_public())
            .await
            .unwrap();
        assert_eq!(reply.code, AuthResponse::LoginServerOffline as u8);
        assert!(reply.world.is_none());
        assert!(reply.detail.is_none());
        assert!(
            peer.submit(
                LoginRequest::default(),
                RS09_REVISION_530,
                &RsaKey::reference_public()
            )
            .await
            .is_err()
        );
    }
    server.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn shared_peer_retains_game_bytes_after_fragmented_login_and_world_metadata() {
    for websocket in [false, true] {
        // Keep the listener owned while starting the mock to avoid a bind-port race.
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        // Pass the owned listener to the responder below.
        let task = tokio::spawn(async move {
            let (socket, _) = listener.accept().await.unwrap();
            let (mut reader, mut writer) = rs09_net::accept(socket, websocket, 64 * 1024)
                .await
                .unwrap();
            serve_reply(&mut reader, &mut writer).await;
        });
        let mut peer = LoginPeer::connect(address, websocket, b"Rust_Client")
            .await
            .unwrap();
        let reply = peer
            .submit(
                LoginRequest {
                    username: b"Rust_Client",
                    password: b"password",
                    isaac_seed: [1, 2, 3, 4],
                    ..LoginRequest::default()
                },
                RS09_REVISION_530,
                &RsaKey::reference_public(),
            )
            .await
            .unwrap();
        let world = reply.world.unwrap();
        assert_eq!(world.staff_level, 2);
        assert_eq!(world.player_index, 2047);
        assert!(world.parental_ad_consent);
        assert!(world.player_member);
        assert!(world.map_members);
        let (mut reader, _writer, pending) = peer.into_transport();
        let mut pending = BytesMut::from(pending.as_ref());
        assert_eq!(
            take(&mut reader, &mut pending, 5).await.as_ref(),
            &[1, 0, 2, 0xaa, 0xbb]
        );
        task.await.unwrap();
    }
}

async fn serve_reply(reader: &mut Reader, writer: &mut Writer) {
    let mut pending = BytesMut::new();
    assert_eq!(
        take(reader, &mut pending, 2).await.as_ref(),
        &login::handshake(b"Rust_Client")
    );
    let nonce = 0x12345678abcdef01u64 as i64;
    for byte in rs09_protocol::session::login_handshake_reply(nonce) {
        writer.write(Bytes::copy_from_slice(&[byte])).await.unwrap();
    }
    let prefix = take(reader, &mut pending, 3).await;
    let count = u16::from_be_bytes([prefix[1], prefix[2]]) as usize;
    let mut packet = prefix.to_vec();
    packet.extend_from_slice(&take(reader, &mut pending, count).await);
    let decoded = login::decode(&packet, &RsaKey::reference_private(), |_| false);
    assert_eq!(decoded.response, AuthResponse::Success);
    assert_eq!(decoded.info.as_ref().unwrap().username, "rust_client");
    assert_eq!(
        decoded.info.unwrap().isaac_seed,
        Some([1, 2, (nonce >> 32) as i32, nonce as i32])
    );
    let mut reply = login::reply(AuthResponse::Success, 2, 2047, nonce);
    reply.extend([1, 0, 2, 0xaa, 0xbb]);
    writer.write(reply.into()).await.unwrap();
    writer.close(1000, "done").await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn rsa_login_burst_does_not_block_cached_io_and_shutdown_releases_sessions() {
    let service = service(1);
    let server = service
        .start("127.0.0.1:0".parse().unwrap(), None)
        .await
        .unwrap();
    let mut tasks = tokio::task::JoinSet::new();
    for _ in 0..64 {
        let address = server.tcp;
        tasks.spawn(async move {
            let mut peer = LoginPeer::connect(address, false, b"Rust_Client")
                .await
                .unwrap();
            assert_eq!(
                peer.submit(
                    LoginRequest {
                        username: b"Rust_Client",
                        password: b"password",
                        ..LoginRequest::default()
                    },
                    RS09_REVISION_530,
                    &RsaKey::reference_public()
                )
                .await
                .unwrap()
                .code,
                8
            );
        });
    }
    timeout(Duration::from_secs(3), async {
        while service.metrics.login_packets.load(Ordering::Relaxed) == 0 {
            tokio::task::yield_now().await;
        }
        let mut peer = CachePeer::connect(server.tcp, false, 0, 1_000_000)
            .await
            .unwrap();
        assert!(
            !peer
                .fetch(Request {
                    archive: 255,
                    group: 255,
                    urgent: true
                })
                .await
                .unwrap()
                .is_empty()
        );
        peer.close().await.unwrap();
    })
    .await
    .unwrap();
    timeout(Duration::from_secs(10), async {
        while let Some(result) = tasks.join_next().await {
            result.unwrap();
        }
    })
    .await
    .unwrap();
    server.shutdown().await.unwrap();
    assert_eq!(service.metrics.login_packets.load(Ordering::Relaxed), 64);
    assert_eq!(service.metrics.active.load(Ordering::Relaxed), 0);
    assert_eq!(service.metrics.queued_bytes.load(Ordering::Relaxed), 0);
}
