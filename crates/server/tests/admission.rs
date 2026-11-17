use rs09_cache::{
    Limits,
    store::{StorageMode, Store},
};
use rs09_net::{login::LoginPeer, peer::CachePeer};
use rs09_protocol::{
    js5::Request,
    login::{AuthResponse, LoginRequest, RsaKey},
    profile::RS09_REVISION_530,
};
use rs09_server::{
    auth::{
        Authenticator, Clock, Mode,
        account::UserAccountInfo,
        password::{PasswordCodec, Passwords},
        storage::{AccountStorage, InMemoryStorage},
    },
    catalog::Catalog,
    runtime::{Config, Service},
};
use std::{
    path::PathBuf,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicI64, AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::time::timeout;
fn service(auth: Arc<Authenticator>, clock: Arc<Clock>, jobs: usize) -> Arc<Service> {
    let limits = Limits::default();
    let cache = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/cache");
    Service::new(
        Arc::new(
            Catalog::new(
                Store::open(&cache, StorageMode::Resident, limits).unwrap(),
                limits,
                128,
                1_000_000,
            )
            .unwrap(),
        ),
        Config {
            authenticator: Some(auth),
            auth_clock: clock,
            auth_jobs: jobs,
            ..Default::default()
        },
        530,
    )
    .unwrap()
}
async fn submit(
    address: std::net::SocketAddr,
    ws: bool,
    name: &[u8],
    password: &[u8],
    revision: u32,
) -> u8 {
    let mut peer = LoginPeer::connect(address, ws, name).await.unwrap();
    let mut profile = RS09_REVISION_530;
    profile.revision = revision;
    let reply = peer
        .submit(
            LoginRequest {
                username: name,
                password,
                isaac_seed: [1, 2, 3, 4],
                ..Default::default()
            },
            profile,
            &RsaKey::reference_public(),
        )
        .await
        .unwrap();
    assert!(reply.world.is_none());
    reply.code
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn original_account_decisions_reach_tcp_and_websocket_peers_after_packet_validation() {
    let storage = Arc::new(InMemoryStorage::default());
    let codec = Arc::new(Passwords::with_salt_source(4, || Ok([0; 16])).unwrap());
    let hash = codec.hash("password").unwrap();
    for (name, ban, online) in [
        ("alice", 0, false),
        ("banned", 1001, false),
        ("online", 0, true),
        ("broken", 0, false),
        ("badcontacts", 0, false),
        ("badreqs", 0, false),
    ] {
        let info = UserAccountInfo::create_default(1000).shared();
        {
            let mut info = info.lock().unwrap();
            info.username = name.into();
            info.password = if name == "broken" {
                "invalid hash".into()
            } else {
                hash.clone()
            };
            info.ban_end_time = ban;
            info.online = online;
            if name == "badcontacts" {
                info.contacts = "{friend,bad}".into();
            }
            if name == "badreqs" {
                info.clan_reqs = "1,0,bad,9".into();
            }
        }
        storage.store(info).unwrap();
    }
    let service = service(
        Arc::new(Authenticator::new(Mode::Production, storage, codec)),
        Arc::new(|| 1000),
        2,
    );
    let server = service
        .start(
            "127.0.0.1:0".parse().unwrap(),
            Some("127.0.0.1:0".parse().unwrap()),
        )
        .await
        .unwrap();
    for ws in [false, true] {
        let address = if ws {
            server.websocket.unwrap()
        } else {
            server.tcp
        };
        assert_eq!(
            submit(address, ws, b"alice", b"password", 531).await,
            AuthResponse::Updated as u8
        );
        for (name, password, response) in [
            ("missing", "password", AuthResponse::InvalidCredentials),
            ("alice", "wrong", AuthResponse::InvalidCredentials),
            ("banned", "password", AuthResponse::AccountDisabled),
            ("online", "password", AuthResponse::AlreadyOnline),
            ("broken", "password", AuthResponse::CouldNotLogin),
            ("alice", "password", AuthResponse::LoginServerOffline),
            ("badcontacts", "password", AuthResponse::UnexpectedError),
            ("badreqs", "password", AuthResponse::UnexpectedError),
        ] {
            assert_eq!(
                submit(address, ws, name.as_bytes(), password.as_bytes(), 530).await,
                response as u8,
                "{name}, ws={ws}"
            );
        }
    }
    assert_eq!(service.metrics.account_checks.load(Ordering::Relaxed), 16);
    assert_eq!(service.metrics.accounts_verified.load(Ordering::Relaxed), 6);
    assert_eq!(service.metrics.metadata_prepared.load(Ordering::Relaxed), 2);
    assert_eq!(
        service.metrics.accounts_rejected.load(Ordering::Relaxed),
        10
    );
    server.shutdown().await.unwrap();
    assert_eq!(service.metrics.active.load(Ordering::Relaxed), 0);
}

#[cfg(windows)]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn configured_windows_executable_applies_backend_policy_and_explicit_listener_overrides() {
    use std::{
        io::{BufRead, BufReader, Write},
        os::windows::process::CommandExt,
        process::{Child, Command, Stdio},
    };
    struct OwnedChild(Child);
    impl Drop for OwnedChild {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    struct TempFile(PathBuf);
    impl Drop for TempFile {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    std::fs::create_dir_all(root.join("runs")).unwrap();
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).unwrap();
    let path = root.join("runs").join(format!(
        "admission-cli-{:032x}.conf",
        u128::from_le_bytes(bytes)
    ));
    let config = TempFile(path);
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&config.0)
        .unwrap();
    file.write_all(
        include_str!("../../../fixtures/config/default.conf")
            .replace("use_auth = false", "use_auth = true")
            .as_bytes(),
    )
    .unwrap();
    drop(file);
    let mut child = OwnedChild(
        Command::new(env!("CARGO_BIN_EXE_rs09-server"))
            .current_dir(&root)
            .arg("--config")
            .arg(&config.0)
            .arg("--cache")
            .arg(root.join("fixtures/cache"))
            .args(["--tcp", "127.0.0.1:0", "--websocket", "127.0.0.1:0"])
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .creation_flags(0x08000000)
            .spawn()
            .unwrap(),
    );
    let output = child.0.stdout.take().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    let reader = std::thread::spawn(move || {
        let (mut tcp, mut ws, mut sent) = (None, None, false);
        for line in BufReader::new(output).lines() {
            let Ok(line) = line else {
                break;
            };
            if let Some((_, address)) = line.split_once("listening on TCP ") {
                tcp = address.parse::<std::net::SocketAddr>().ok();
            }
            if let Some((_, address)) = line.split_once("Binary WebSocket listener: ws://") {
                ws = address
                    .trim_end_matches('/')
                    .parse::<std::net::SocketAddr>()
                    .ok();
            }
            if !sent {
                if let (Some(tcp), Some(ws)) = (tcp, ws) {
                    let _ = tx.send((tcp, ws));
                    sent = true;
                }
            }
        }
    });
    let (tcp, ws) = rx.recv_timeout(Duration::from_secs(10)).unwrap();
    for (address, websocket) in [(tcp, false), (ws, true)] {
        assert_eq!(
            timeout(
                Duration::from_secs(5),
                submit(address, websocket, b"missing", b"password", 530)
            )
            .await
            .unwrap(),
            AuthResponse::InvalidCredentials as u8
        );
    }
    drop(child);
    reader.join().unwrap();
    drop(config);
}
struct Gate {
    released: Mutex<bool>,
    changed: Condvar,
    started: AtomicUsize,
    time: Arc<AtomicI64>,
}
impl Gate {
    fn release(&self) {
        *self.released.lock().unwrap() = true;
        self.changed.notify_all();
    }
}
struct Release(Arc<Gate>);
impl Drop for Release {
    fn drop(&mut self) {
        self.0.release();
    }
}
impl PasswordCodec for Gate {
    fn hash(&self, _: &str) -> rs09_server::auth::Result<String> {
        unreachable!()
    }
    fn verify(&self, _: &str, _: &str) -> rs09_server::auth::Result<bool> {
        self.started.fetch_add(1, Ordering::Relaxed);
        let mut released = self.released.lock().unwrap();
        while !*released {
            released = self.changed.wait(released).unwrap();
        }
        self.time.fetch_add(20, Ordering::Relaxed);
        Ok(true)
    }
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn blocked_account_work_preserves_cache_io_and_keeps_its_permit_after_session_shutdown() {
    let time = Arc::new(AtomicI64::new(1000));
    let gate = Arc::new(Gate {
        released: Mutex::new(false),
        changed: Condvar::new(),
        started: AtomicUsize::new(0),
        time: time.clone(),
    });
    let release = Release(gate.clone());
    let storage = Arc::new(InMemoryStorage::default());
    let info = UserAccountInfo::create_default(1000).shared();
    {
        let mut info = info.lock().unwrap();
        info.username = "alice".into();
        info.password = "present".into();
        info.ban_end_time = 1010;
    }
    storage.store(info).unwrap();
    let service = service(
        Arc::new(Authenticator::new(Mode::Production, storage, gate.clone())),
        Arc::new(move || time.load(Ordering::Relaxed)),
        1,
    );
    let server = service
        .start("127.0.0.1:0".parse().unwrap(), None)
        .await
        .unwrap();
    let addr = server.tcp;
    let first = tokio::spawn(async move { submit(addr, false, b"alice", b"password", 530).await });
    timeout(Duration::from_secs(5), async {
        while gate.started.load(Ordering::Relaxed) == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let mut cache = CachePeer::connect(server.tcp, false, 0, 1_000_000)
        .await
        .unwrap();
    timeout(
        Duration::from_secs(2),
        cache.fetch(Request {
            archive: 3,
            group: 5,
            urgent: true,
        }),
    )
    .await
    .unwrap()
    .unwrap();
    server.shutdown().await.unwrap();
    first.abort();
    let _ = first.await;
    let restarted = service
        .start("127.0.0.1:0".parse().unwrap(), None)
        .await
        .unwrap();
    let addr = restarted.tcp;
    let second = tokio::spawn(async move { submit(addr, false, b"alice", b"password", 530).await });
    timeout(Duration::from_secs(5), async {
        while service.metrics.auth_waiters.load(Ordering::Relaxed) == 0
            && service.metrics.account_checks.load(Ordering::Relaxed) == 1
        {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        service.metrics.account_checks.load(Ordering::Relaxed),
        1,
        "canceling a session must not release a running job's permit"
    );
    gate.release();
    assert_eq!(
        timeout(Duration::from_secs(5), second)
            .await
            .unwrap()
            .unwrap(),
        AuthResponse::LoginServerOffline as u8,
        "ban expires during verification"
    );
    drop(release);
    restarted.shutdown().await.unwrap();
    assert_eq!(service.metrics.auth_waiters.load(Ordering::Relaxed), 0);
}
