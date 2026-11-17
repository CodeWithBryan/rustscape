use rs09_cache::{
    Limits,
    store::{StorageMode, Store},
};
use rs09_net::peer::CachePeer;
use rs09_protocol::js5::{self, Request};
use rs09_server::{
    catalog::Catalog,
    runtime::{Config, Service},
};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{Arc, atomic::Ordering},
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    time::{Instant, timeout},
};

fn directory() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/cache")
}

#[test]
fn impossible_resource_limits_return_errors_before_allocating_channels() {
    let limits = Limits::default();
    let store = Store::open(&directory(), StorageMode::Resident, limits).unwrap();
    let catalog = Arc::new(Catalog::new(store, limits, 128, 1_000_000).unwrap());
    let oversized = tokio::sync::Semaphore::MAX_PERMITS + 1;
    for config in [
        Config {
            max_connections: oversized,
            ..Config::default()
        },
        Config {
            compute_jobs: oversized,
            ..Config::default()
        },
        Config {
            pending_requests: oversized,
            ..Config::default()
        },
        Config {
            pending_replies: oversized,
            ..Config::default()
        },
        Config {
            outgoing_bytes: oversized,
            ..Config::default()
        },
    ] {
        assert!(Service::new(Arc::clone(&catalog), config, 530).is_err());
    }
}
fn service(mut config: Config) -> Arc<Service> {
    let limits = Limits::default();
    let store = Store::open(&directory(), StorageMode::Resident, limits).unwrap();
    let catalog = Arc::new(Catalog::new(store, limits, 128, 1_000_000).unwrap());
    config.read_timeout = Duration::from_secs(3);
    Service::new(catalog, config, 530).unwrap()
}
fn large_service(mut config: Config) -> Arc<Service> {
    let limits = Limits::default();
    let mut data = std::fs::read(directory().join("main_file_cache.dat2")).unwrap();
    let mut indices: BTreeMap<_, _> = [3, 7, 255]
        .into_iter()
        .map(|id| {
            (
                id,
                std::fs::read(directory().join(format!("main_file_cache.idx{id}"))).unwrap(),
            )
        })
        .collect();
    let mut payload = vec![0x5a; 65_000];
    payload[0] = 0;
    payload[1..5].copy_from_slice(&64_995u32.to_be_bytes());
    let start = data.len().div_ceil(520);
    let chunks = payload.len().div_ceil(512);
    data.resize((start + chunks) * 520, 0);
    for (part, chunk) in payload.chunks(512).enumerate() {
        let offset = (start + part) * 520;
        let next = if part + 1 == chunks {
            0
        } else {
            start + part + 1
        };
        data[offset..offset + 2].copy_from_slice(&2u16.to_be_bytes());
        data[offset + 2..offset + 4].copy_from_slice(&(part as u16).to_be_bytes());
        data[offset + 4..offset + 7].copy_from_slice(&(next as u32).to_be_bytes()[1..]);
        data[offset + 7] = 3;
        data[offset + 8..offset + 8 + chunk.len()].copy_from_slice(chunk);
    }
    let index = indices.get_mut(&3).unwrap();
    index[12..15].copy_from_slice(&65_000u32.to_be_bytes()[1..]);
    index[15..18].copy_from_slice(&(start as u32).to_be_bytes()[1..]);
    let store = Store::from_bytes(data, indices, limits);
    config.read_timeout = Duration::from_secs(3);
    Service::new(
        Arc::new(Catalog::new(store, limits, 128, 1_000_000).unwrap()),
        config,
        530,
    )
    .unwrap()
}
fn keys() -> BTreeMap<u16, [u32; 4]> {
    include_str!("../../../fixtures/cache/oracle.jsonl")
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .filter(|case| case["kind"] == "group")
        .map(|case| {
            (
                case["group"].as_u64().unwrap() as u16,
                case["key"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|value| value.as_i64().unwrap() as u32)
                    .collect::<Vec<_>>()
                    .try_into()
                    .unwrap(),
            )
        })
        .collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn tcp_and_websocket_clients_receive_exact_cache_packets_for_every_xor_key() {
    let service = service(Config::default());
    let server = service
        .start(
            "127.0.0.1:0".parse().unwrap(),
            Some("127.0.0.1:0".parse().unwrap()),
        )
        .await
        .unwrap();
    let store = Store::open(&directory(), StorageMode::Resident, Limits::default()).unwrap();
    for websocket in [false, true] {
        let address = if websocket {
            server.websocket.unwrap()
        } else {
            server.tcp
        };
        for key in [0, 1, 128, 255] {
            let mut peer = CachePeer::connect(address, websocket, key, 1_000_000)
                .await
                .unwrap();
            for (&group, _) in keys().iter().take(12) {
                // Group keys decrypt file contents; the wire service transmits packed encrypted bytes.
                let raw = store.read(3, u32::from(group)).unwrap().unwrap();
                for urgent in [true, false] {
                    let actual = timeout(
                        Duration::from_secs(3),
                        peer.fetch(Request {
                            archive: 3,
                            group,
                            urgent,
                        }),
                    )
                    .await
                    .unwrap()
                    .unwrap();
                    let length = u32::from_be_bytes(raw[1..5].try_into().unwrap()) as usize
                        + if raw[0] == 0 { 5 } else { 9 };
                    assert_eq!(actual, raw[..length]);
                }
            }
            peer.close().await.unwrap();
        }
    }
    assert!(service.catalog.retained_bytes() <= 1_000_000);
    server.shutdown().await.unwrap();
    assert_eq!(service.metrics.active.load(Ordering::Relaxed), 0);
    assert_eq!(service.metrics.queued_bytes.load(Ordering::Relaxed), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn fragmented_handshakes_and_coalesced_controls_preserve_wire_order() {
    let service = service(Config::default());
    let server = service
        .start("127.0.0.1:0".parse().unwrap(), None)
        .await
        .unwrap();
    let mut socket = TcpStream::connect(server.tcp).await.unwrap();
    for byte in js5::handshake(530) {
        socket.write_all(&[byte]).await.unwrap();
        tokio::task::yield_now().await;
    }
    let mut response = [255];
    socket.read_exact(&mut response).await.unwrap();
    assert_eq!(response, [0]);
    let first = Request {
        archive: 3,
        group: 2,
        urgent: true,
    };
    let second = Request {
        archive: 3,
        group: 5,
        urgent: false,
    };
    let mut requests = vec![6, 0, 0, 3, 4, 128, 0, 0];
    requests.extend(first.encode());
    requests.extend(second.encode());
    socket.write_all(&requests).await.unwrap();
    let expected_first = service.catalog.reply(first, 128).unwrap().unwrap();
    let expected_second = service.catalog.reply(second, 128).unwrap().unwrap();
    let mut actual = vec![0; expected_first.len() + expected_second.len()];
    timeout(Duration::from_secs(3), socket.read_exact(&mut actual))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        actual,
        [expected_first.as_ref(), expected_second.as_ref()].concat()
    );
    server.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn revision_mismatch_and_text_websocket_frames_close_without_corrupting_other_sessions() {
    let service = service(Config::default());
    let server = service
        .start(
            "127.0.0.1:0".parse().unwrap(),
            Some("127.0.0.1:0".parse().unwrap()),
        )
        .await
        .unwrap();
    let mut wrong = TcpStream::connect(server.tcp).await.unwrap();
    wrong.write_all(&js5::handshake(529)).await.unwrap();
    let mut buffer = [0; 1];
    assert_eq!(
        timeout(Duration::from_secs(3), wrong.read(&mut buffer))
            .await
            .unwrap()
            .unwrap(),
        0
    );
    let (mut socket, _) =
        tokio_tungstenite::connect_async(format!("ws://{}/", server.websocket.unwrap()))
            .await
            .unwrap();
    use futures_util::{SinkExt, StreamExt};
    socket
        .send(tokio_tungstenite::tungstenite::Message::Text(
            "unsupported".into(),
        ))
        .await
        .unwrap();
    let close = timeout(Duration::from_secs(3), socket.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    match close {
        tokio_tungstenite::tungstenite::Message::Close(Some(close)) => {
            assert_eq!(u16::from(close.code), 1003)
        }
        _ => panic!("expected close frame"),
    }
    let mut peer = CachePeer::connect(server.tcp, false, 0, 1_000_000)
        .await
        .unwrap();
    assert!(
        !peer
            .fetch(Request {
                archive: 3,
                group: 5,
                urgent: true
            })
            .await
            .unwrap()
            .is_empty()
    );
    peer.close().await.unwrap();
    server.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn missing_groups_do_not_poison_later_requests_and_master_snapshot_is_stable() {
    let service = service(Config::default());
    let server = service
        .start("127.0.0.1:0".parse().unwrap(), None)
        .await
        .unwrap();
    let mut peer = CachePeer::connect(server.tcp, false, 0, 1_000_000)
        .await
        .unwrap();
    peer.request(Request {
        archive: 3,
        group: 65535,
        urgent: true,
    })
    .await
    .unwrap();
    let request = Request {
        archive: 255,
        group: 255,
        urgent: true,
    };
    let master = peer.fetch(request).await.unwrap();
    assert_eq!(master[0], 0);
    assert_eq!(master.len(), 5 + 4 * 8); // synthetic idx255 has slots 0..3
    assert_eq!(peer.fetch(request).await.unwrap(), master);
    peer.close().await.unwrap();
    server.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn native_cache_peer_enforces_original_inflight_quotas() {
    let service = service(Config::default());
    let server = service
        .start("127.0.0.1:0".parse().unwrap(), None)
        .await
        .unwrap();
    let mut peer = CachePeer::connect(server.tcp, false, 0, 1_000_000)
        .await
        .unwrap();
    for urgent in [true, false] {
        for i in 0..20 {
            peer.request(Request {
                archive: 3,
                group: 60_000 + i,
                urgent,
            })
            .await
            .unwrap();
        }
        assert!(
            peer.request(Request {
                archive: 3,
                group: 61_000,
                urgent
            })
            .await
            .is_err()
        );
    }
    peer.close().await.unwrap();
    server.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn bounded_flood_and_slow_readers_leave_timers_and_fast_clients_responsive() {
    let config = Config {
        max_connections: 256,
        pending_requests: 2,
        pending_replies: 2,
        outgoing_bytes: 1_100_000,
        write_timeout: Duration::from_millis(400),
        ..Config::default()
    };
    let service = large_service(config.clone());
    let server = service
        .start("127.0.0.1:0".parse().unwrap(), None)
        .await
        .unwrap();
    let mut flood = TcpStream::connect(server.tcp).await.unwrap();
    flood.write_all(&js5::handshake(530)).await.unwrap();
    flood.read_u8().await.unwrap();
    let packet = Request {
        archive: 3,
        group: 2,
        urgent: true,
    }
    .encode();
    let bytes: Vec<_> = (0..5000).flat_map(|_| packet).collect();
    let flood_task = tokio::spawn(async move {
        let _ = flood.write_all(&bytes).await;
        tokio::time::sleep(Duration::from_secs(1)).await;
    });
    timeout(Duration::from_secs(1), async {
        while service.metrics.sent_bytes.load(Ordering::Relaxed) < 65_000 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let started = Instant::now();
    for _ in 0..10 {
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    assert!(started.elapsed() < Duration::from_secs(1));
    let result = timeout(Duration::from_secs(3), async {
        let mut peer = CachePeer::connect(server.tcp, false, 0, 1_000_000).await?;
        let bytes = peer
            .fetch(Request {
                archive: 3,
                group: 5,
                urgent: true,
            })
            .await?;
        peer.close().await?;
        Ok::<_, std::io::Error>(bytes)
    })
    .await
    .unwrap()
    .unwrap();
    assert!(!result.is_empty());
    assert!(service.metrics.peak_queued_bytes.load(Ordering::Relaxed) >= 65_000);
    assert!(service.metrics.peak_queued_bytes.load(Ordering::Relaxed) <= config.outgoing_bytes);
    flood_task.abort();
    server.shutdown().await.unwrap();
    assert_eq!(service.metrics.queued_bytes.load(Ordering::Relaxed), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn connection_admission_and_shutdown_release_all_bounded_resources() {
    let config = Config {
        max_connections: 2,
        ..Config::default()
    };
    let service = service(config);
    let server = service
        .start("127.0.0.1:0".parse().unwrap(), None)
        .await
        .unwrap();
    let a = TcpStream::connect(server.tcp).await.unwrap();
    let b = TcpStream::connect(server.tcp).await.unwrap();
    timeout(Duration::from_secs(3), async {
        while service.metrics.active.load(Ordering::Relaxed) != 2 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let mut c = TcpStream::connect(server.tcp).await.unwrap();
    let mut byte = [0];
    assert_eq!(
        timeout(Duration::from_secs(3), c.read(&mut byte))
            .await
            .unwrap()
            .unwrap(),
        0
    );
    assert_eq!(service.metrics.rejected.load(Ordering::Relaxed), 1);
    drop((a, b));
    server.shutdown().await.unwrap();
    assert_eq!(service.metrics.active.load(Ordering::Relaxed), 0);
    assert_eq!(service.metrics.queued_bytes.load(Ordering::Relaxed), 0);
}
