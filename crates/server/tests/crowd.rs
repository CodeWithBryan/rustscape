//! Connection capacity smoke test, not a simulation/player-capacity benchmark.
use rs09_cache::{
    Limits,
    store::{StorageMode, Store},
};
use rs09_net::peer::CachePeer;
use rs09_protocol::js5::Request;
use rs09_server::{
    catalog::Catalog,
    runtime::{Config, Service},
};
use std::{
    path::PathBuf,
    sync::{Arc, atomic::Ordering},
    time::Duration,
};
use tokio::{sync::Barrier, task::JoinSet, time::timeout};

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn five_hundred_tcp_and_websocket_cache_clients_share_frames_and_shutdown_cleanly() {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/cache");
    let limits = Limits::default();
    let store = Store::open(&directory, StorageMode::Resident, limits).unwrap();
    let packed = store.read(3, 5).unwrap().unwrap();
    let length = u32::from_be_bytes(packed[1..5].try_into().unwrap()) as usize
        + if packed[0] == 0 { 5 } else { 9 };
    let expected = Arc::new(packed[..length].to_vec());
    let catalog = Arc::new(Catalog::new(store, limits, 128, 1_000_000).unwrap());
    let config = Config {
        read_timeout: Duration::from_secs(15),
        ..Config::default()
    };
    let service = Service::new(catalog, config.clone(), 530).unwrap();
    let server = service
        .start(
            "127.0.0.1:0".parse().unwrap(),
            Some("127.0.0.1:0".parse().unwrap()),
        )
        .await
        .unwrap();
    let barrier = Arc::new(Barrier::new(501));
    let mut tasks = JoinSet::new();
    for i in 0..500 {
        let websocket = i % 2 != 0;
        let address = if websocket {
            server.websocket.unwrap()
        } else {
            server.tcp
        };
        let barrier = Arc::clone(&barrier);
        let expected = Arc::clone(&expected);
        tasks.spawn(async move {
            timeout(Duration::from_secs(15), async {
                let key = [0, 1, 128, 255][i % 4];
                let mut peer = CachePeer::connect(address, websocket, key, 1_000_000).await?;
                barrier.wait().await;
                for round in 0..4 {
                    let received = peer
                        .fetch(Request {
                            archive: 3,
                            group: 5,
                            urgent: round % 2 == 0,
                        })
                        .await?;
                    assert_eq!(received, *expected);
                }
                peer.close().await
            })
            .await
            .unwrap()
        });
    }
    timeout(Duration::from_secs(15), barrier.wait())
        .await
        .unwrap();
    assert_eq!(service.metrics.active.load(Ordering::Relaxed), 500);
    while let Some(result) = tasks.join_next().await {
        result.unwrap().unwrap();
    }
    assert_eq!(service.metrics.requests.load(Ordering::Relaxed), 2000);
    assert_eq!(
        service.catalog.loads(),
        2,
        "shared urgent/prefetch frames each prepare once"
    );
    assert!(service.metrics.peak_queued_bytes.load(Ordering::Relaxed) <= config.outgoing_bytes);
    server.shutdown().await.unwrap();
    assert_eq!(service.metrics.active.load(Ordering::Relaxed), 0);
    assert_eq!(service.metrics.queued_bytes.load(Ordering::Relaxed), 0);
}
