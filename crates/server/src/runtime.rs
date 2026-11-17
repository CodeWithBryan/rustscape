//! Bounded cache, account validation and native world-session stream transport.
use crate::{
    auth::{
        Authenticator, Clock,
        admission::{self, Decision},
    },
    catalog::Catalog,
    config::Configuration,
};
use bytes::Bytes;
use rs09_game_core::random::JavaRandom;
use rs09_net::{Chunk, Reader, Writer};
use rs09_protocol::{
    lobby::{self, World},
    login::{self, AuthResponse, RsaKey},
    session::{Decoder, Incoming, login_handshake_reply},
};
use std::{
    io,
    net::SocketAddr,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU8, AtomicU64, AtomicUsize, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::{
    net::TcpListener,
    sync::{OwnedSemaphorePermit, Semaphore, mpsc, watch},
    task::JoinSet,
    time::timeout,
};

#[derive(Clone)]
pub struct Config {
    pub max_connections: usize,
    pub pending_requests: usize,
    pub pending_replies: usize,
    pub outgoing_bytes: usize,
    pub compute_jobs: usize,
    pub auth_jobs: usize,
    pub authenticator: Option<Arc<Authenticator>>,
    pub auth_clock: Arc<Clock>,
    pub startup: Option<Arc<Configuration>>,
    pub world: Option<Arc<dyn crate::world::session::World>>,
    pub max_ingress: usize,
    pub read_timeout: Duration,
    pub write_timeout: Duration,
    pub worlds: Vec<World>,
    pub world_stamp: i32,
    pub login_key: RsaKey,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            max_connections: 2048,
            pending_requests: 40,
            pending_replies: 8,
            outgoing_bytes: 32 * 1024 * 1024,
            compute_jobs: 8,
            auth_jobs: 4,
            authenticator: None,
            auth_clock: Arc::new(wall_clock),
            startup: None,
            world: None,
            max_ingress: 64 * 1024,
            read_timeout: Duration::from_secs(60),
            write_timeout: Duration::from_secs(15),
            worlds: vec![World::default()],
            world_stamp: 1,
            login_key: RsaKey::reference_private(),
        }
    }
}
pub fn wall_clock() -> i64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(time) => time.as_millis() as i64,
        Err(error) => (error.duration().as_nanos().div_ceil(1_000_000) as i64).wrapping_neg(),
    }
}
#[derive(Default)]
pub struct Metrics {
    pub active: AtomicUsize,
    pub accepted: AtomicU64,
    pub rejected: AtomicU64,
    pub requests: AtomicU64,
    pub replies: AtomicU64,
    pub sent_bytes: AtomicU64,
    pub queued_bytes: AtomicUsize,
    pub peak_queued_bytes: AtomicUsize,
    pub failed: AtomicU64,
    pub login_packets: AtomicU64,
    pub account_checks: AtomicU64,
    pub accounts_verified: AtomicU64,
    pub metadata_prepared: AtomicU64,
    pub accounts_rejected: AtomicU64,
    pub auth_waiters: AtomicUsize,
}
pub struct Service {
    pub catalog: Arc<Catalog>,
    pub metrics: Arc<Metrics>,
    config: Config,
    slots: Arc<Semaphore>,
    compute: Arc<Semaphore>,
    authentication: Arc<Semaphore>,
    outgoing: Arc<Semaphore>,
    rng: Mutex<JavaRandom>,
}
pub struct Running {
    pub tcp: SocketAddr,
    pub websocket: Option<SocketAddr>,
    stop: watch::Sender<bool>,
    task: tokio::task::JoinHandle<io::Result<()>>,
    pub service: Arc<Service>,
}
impl Running {
    pub async fn shutdown(self) -> io::Result<()> {
        let _ = self.stop.send(true);
        self.task.await.map_err(io::Error::other)?
    }
}
impl Service {
    pub fn new(catalog: Arc<Catalog>, config: Config, seed: i64) -> io::Result<Arc<Self>> {
        if config.max_connections == 0
            || config.max_connections > Semaphore::MAX_PERMITS
            || config.compute_jobs == 0
            || config.compute_jobs > Semaphore::MAX_PERMITS
            || config.auth_jobs == 0
            || config.auth_jobs > Semaphore::MAX_PERMITS
            || config.compute_jobs.checked_add(config.auth_jobs).is_none()
            || config.pending_requests == 0
            || config.pending_requests > Semaphore::MAX_PERMITS
            || config.pending_replies == 0
            || config.pending_replies > Semaphore::MAX_PERMITS
            || config.outgoing_bytes < 1_010_000
            || config.outgoing_bytes > u32::MAX as usize
            || config.outgoing_bytes > Semaphore::MAX_PERMITS
            || config.max_ingress < 4096
        {
            return Err(io::Error::other("invalid server resource limits"));
        }
        Ok(Arc::new(Self {
            catalog,
            metrics: Arc::new(Metrics::default()),
            slots: Arc::new(Semaphore::new(config.max_connections)),
            compute: Arc::new(Semaphore::new(config.compute_jobs)),
            authentication: Arc::new(Semaphore::new(config.auth_jobs)),
            outgoing: Arc::new(Semaphore::new(config.outgoing_bytes)),
            rng: Mutex::new(JavaRandom::new(seed)),
            config,
        }))
    }
    pub fn startup_configuration(&self) -> Option<&Configuration> {
        self.config.startup.as_deref()
    }
    pub async fn start(
        self: &Arc<Self>,
        tcp: SocketAddr,
        websocket: Option<SocketAddr>,
    ) -> io::Result<Running> {
        let tcp_listener = TcpListener::bind(tcp).await?;
        let tcp = tcp_listener.local_addr()?;
        let ws_listener = if let Some(address) = websocket {
            Some(TcpListener::bind(address).await?)
        } else {
            None
        };
        let websocket = ws_listener
            .as_ref()
            .map(TcpListener::local_addr)
            .transpose()?;
        let (stop, receiver) = watch::channel(false);
        let service = Arc::clone(self);
        let task = tokio::spawn(async move {
            let mut workers = JoinSet::new();
            workers.spawn(Arc::clone(&service).listen(tcp_listener, false, receiver.clone()));
            if let Some(listener) = ws_listener {
                workers.spawn(Arc::clone(&service).listen(listener, true, receiver));
            }
            while let Some(result) = workers.join_next().await {
                result.map_err(io::Error::other)??;
            }
            Ok(())
        });
        Ok(Running {
            tcp,
            websocket,
            stop,
            task,
            service: Arc::clone(self),
        })
    }
    async fn listen(
        self: Arc<Self>,
        listener: TcpListener,
        websocket: bool,
        mut stop: watch::Receiver<bool>,
    ) -> io::Result<()> {
        let mut sessions = JoinSet::new();
        loop {
            tokio::select! {
                changed = stop.changed()=>{let _ = changed; break;},
                Some(_) = sessions.join_next(), if !sessions.is_empty()=>{},
                accepted = listener.accept()=>{
                    let (socket, _) = accepted?;
                    let Ok(slot) = Arc::clone(&self.slots).try_acquire_owned() else {
                        self.metrics.rejected.fetch_add(1, Ordering::Relaxed); continue;
                    };
                    let service = Arc::clone(&self);
                    sessions.spawn(async move {
                        service.metrics.accepted.fetch_add(1, Ordering::Relaxed);
                        service.metrics.active.fetch_add(1, Ordering::Relaxed);
                        let active = Active {metrics: Arc::clone(&service.metrics), _slot: slot};
                        if let Ok(Ok((reader, writer))) = timeout(Duration::from_secs(10),
                            rs09_net::accept(socket, websocket, service.config.max_ingress)).await {
                            if service.connection(reader, writer).await.is_err() {
                                service.metrics.failed.fetch_add(1, Ordering::Relaxed);
                            }
                        }
                        drop(active);
                    });
                }
            }
        }
        sessions.abort_all();
        while sessions.join_next().await.is_some() {}
        Ok(())
    }
    async fn connection(self: &Arc<Self>, reader: Reader, writer: Writer) -> io::Result<()> {
        let (requests, queue) = mpsc::channel(self.config.pending_requests);
        let (replies, output) = mpsc::channel(self.config.pending_replies);
        let key = Arc::new(AtomicU8::new(0));
        tokio::select! {
            result = self.read_loop(reader, requests, replies.clone(), Arc::clone(&key))=>result,
            result = self.compute_loop(queue, replies, key)=>result,
            result = self.write_loop(writer, output)=>result,
        }
    }
    async fn enqueue(
        &self,
        replies: &mpsc::Sender<Envelope>,
        bytes: Bytes,
        close: Option<(u16, String)>,
    ) -> io::Result<()> {
        let weight = bytes.len();
        let permit = timeout(
            self.config.write_timeout,
            Arc::clone(&self.outgoing)
                .acquire_many_owned(u32::try_from(weight).map_err(io::Error::other)?),
        )
        .await
        .map_err(|_| io::Error::other("outgoing byte budget timed out"))?
        .map_err(io::Error::other)?;
        let queued = self
            .metrics
            .queued_bytes
            .fetch_add(weight, Ordering::Relaxed)
            + weight;
        self.metrics
            .peak_queued_bytes
            .fetch_max(queued, Ordering::Relaxed);
        let message = Envelope {
            bytes,
            close,
            _budget: Budget {
                metrics: Arc::clone(&self.metrics),
                weight,
                _permit: permit,
            },
        };
        replies
            .send(message)
            .await
            .map_err(|_| io::ErrorKind::BrokenPipe.into())
    }
    async fn read_loop(
        &self,
        mut reader: Reader,
        requests: mpsc::Sender<rs09_protocol::js5::Request>,
        replies: mpsc::Sender<Envelope>,
        key: Arc<AtomicU8>,
    ) -> io::Result<()> {
        let mut decoder = Decoder::new(self.config.max_ingress, 32767);
        let mut server_key = 0;
        loop {
            match timeout(self.config.read_timeout, reader.read())
                .await
                .map_err(|_| io::Error::other("read timed out"))??
            {
                Chunk::Closed => return Ok(()),
                Chunk::Text => {
                    self.enqueue(
                        &replies,
                        Bytes::new(),
                        Some((1003, "Binary frames only".to_owned())),
                    )
                    .await?;
                    return std::future::pending().await;
                }
                Chunk::Binary(bytes) => decoder.push(&bytes).map_err(io::Error::other)?,
            }
            while let Some(packet) = decoder.next_packet().map_err(io::Error::other)? {
                match packet {
                    Incoming::CacheHandshake { revision } => {
                        if revision != 530 {
                            self.enqueue(
                                &replies,
                                Bytes::new(),
                                Some((1000, "Revision mismatch".to_owned())),
                            )
                            .await?;
                            return std::future::pending().await;
                        }
                        self.enqueue(&replies, Bytes::from_static(&[0]), None)
                            .await?;
                    }
                    Incoming::LoginHandshake { .. } => {
                        let seed = self
                            .rng
                            .lock()
                            .map_err(|_| io::Error::other("handshake RNG poisoned"))?
                            .next_i64();
                        server_key = seed;
                        self.enqueue(
                            &replies,
                            Bytes::copy_from_slice(&login_handshake_reply(seed)),
                            None,
                        )
                        .await?;
                    }
                    Incoming::WorldList { stamp } => {
                        let data =
                            lobby::world_list(&self.config.worlds, self.config.world_stamp, stamp)
                                .map_err(io::Error::other)?;
                        self.enqueue(&replies, data.into(), None).await?;
                    }
                    Incoming::CacheRequest(request) => {
                        self.metrics.requests.fetch_add(1, Ordering::Relaxed);
                        requests
                            .send(request)
                            .await
                            .map_err(|_| io::ErrorKind::BrokenPipe)?;
                    }
                    Incoming::CacheKey {
                        key: value,
                        valid_padding,
                    } => {
                        key.store(value, Ordering::Relaxed);
                        if !valid_padding {
                            self.enqueue(
                                &replies,
                                Bytes::new(),
                                Some((1000, "Invalid cache control".to_owned())),
                            )
                            .await?;
                            return std::future::pending().await;
                        }
                    }
                    Incoming::LoginPacket(packet) => {
                        self.metrics.login_packets.fetch_add(1, Ordering::Relaxed);
                        // RSA shares bounded compute admission with cold cache work.
                        let permit = Arc::clone(&self.compute)
                            .acquire_owned()
                            .await
                            .map_err(io::Error::other)?;
                        let key = self.config.login_key.clone();
                        let world = self.config.world.clone();
                        let parsed = tokio::task::spawn_blocking(move || {
                            let _permit = permit;
                            login::decode(&packet, &key, |name| {
                                world.as_ref().is_some_and(|w| w.is_online(name))
                            })
                        })
                        .await
                        .map_err(io::Error::other)?;
                        let prepared = if parsed.response == AuthResponse::Success {
                            if let Some(auth) = self.config.authenticator.clone() {
                                // Separate admission prevents password/SQL work from
                                // occupying the cache/RSA permit pool or socket workers.
                                self.metrics.auth_waiters.fetch_add(1, Ordering::Relaxed);
                                let waiting = AuthWaiting(Arc::clone(&self.metrics));
                                let permit = Arc::clone(&self.authentication)
                                    .acquire_owned()
                                    .await
                                    .map_err(io::Error::other)?;
                                drop(waiting);
                                let clock = Arc::clone(&self.config.auth_clock);
                                let metrics = Arc::clone(&self.metrics);
                                tokio::task::spawn_blocking(move || {
                                    let _permit = permit;
                                    metrics.account_checks.fetch_add(1, Ordering::Relaxed);
                                    let decision = admission::check(parsed, &auth, &*clock);
                                    match decision {
                                        Decision::Ready(authenticated) => {
                                            metrics
                                                .accounts_verified
                                                .fetch_add(1, Ordering::Relaxed);
                                            match crate::player::prepare(authenticated, clock) {
                                                Ok(prepared) => {
                                                    metrics
                                                        .metadata_prepared
                                                        .fetch_add(1, Ordering::Relaxed);
                                                    Ok(prepared)
                                                }
                                                Err(_) => Err(AuthResponse::UnexpectedError),
                                            }
                                        }
                                        Decision::Reply(response) => {
                                            metrics
                                                .accounts_rejected
                                                .fetch_add(1, Ordering::Relaxed);
                                            Err(response)
                                        }
                                    }
                                })
                                .await
                                .unwrap_or(Err(AuthResponse::UnexpectedError))
                            } else {
                                Err(AuthResponse::LoginServerOffline)
                            }
                        } else {
                            Err(parsed.response)
                        };
                        let mut initial = decoder.into_buffered();
                        let joined = match prepared {
                            Ok(prepared) => {
                                if let Some(world) = &self.config.world {
                                    let mut admission = world.enter(prepared);
                                    loop {
                                        tokio::select! {
                                            result=&mut admission=>break result,
                                            chunk=timeout(self.config.read_timeout,reader.read())=>{
                                                match chunk.map_err(|_|io::Error::other("world admission read timed out"))?? {
                                                    Chunk::Closed=>return Ok(()),
                                                    Chunk::Text=>return Err(io::Error::other("binary world frames required")),
                                                    Chunk::Binary(bytes)=>{
                                                        if initial.len().checked_add(bytes.len()).is_none_or(|n|n>self.config.max_ingress){return Err(io::Error::other("world admission ingress exceeds limit"));}
                                                        initial.extend_from_slice(&bytes);
                                                    }
                                                }
                                            }
                                        }
                                    }
                                } else {
                                    Err(AuthResponse::LoginServerOffline)
                                }
                            }
                            Err(response) => Err(response),
                        };
                        let response = match joined {
                            Ok(joined) => {
                                self.enqueue(
                                    &replies,
                                    login::reply(
                                        AuthResponse::Success,
                                        joined.rights,
                                        joined.index,
                                        0,
                                    )
                                    .into(),
                                    None,
                                )
                                .await?;
                                return self.world_loop(reader, initial, replies, joined).await;
                            }
                            Err(AuthResponse::Success) => AuthResponse::UnexpectedError,
                            Err(response) => response,
                        };
                        self.enqueue(
                            &replies,
                            login::reply(response, 0, 0, server_key).into(),
                            Some((1000, "Login ended".to_owned())),
                        )
                        .await?;
                        return std::future::pending().await;
                    }
                    Incoming::Disconnect => {
                        self.enqueue(
                            &replies,
                            Bytes::new(),
                            Some((1000, "Disconnected".to_owned())),
                        )
                        .await?;
                        return std::future::pending().await;
                    }
                    Incoming::CacheControl { .. } => {}
                }
            }
        }
    }
    async fn world_loop(
        &self,
        mut reader: Reader,
        initial: Vec<u8>,
        replies: mpsc::Sender<Envelope>,
        joined: crate::world::session::Joined,
    ) -> io::Result<()> {
        let crate::world::session::Joined {
            incoming,
            mut outgoing,
            ..
        } = joined;
        // Poll both directions independently: a full world's inbound queue must
        // not prevent draining its bounded outbound queue (and vice versa).
        let ingress = async {
            let mut decoder = rs09_protocol::game::Decoder::new(
                rs09_protocol::game::Direction::ClientToServer,
                self.config.max_ingress,
                32767,
            );
            decoder.push(&initial).map_err(io::Error::other)?;
            loop {
                while let Some(frame) = decoder.next_frame().map_err(io::Error::other)? {
                    timeout(self.config.read_timeout, incoming.send(frame))
                        .await
                        .map_err(|_| io::Error::other("world ingress timed out"))?
                        .map_err(|_| io::Error::other("world input closed"))?;
                }
                match timeout(self.config.read_timeout, reader.read())
                    .await
                    .map_err(|_| io::Error::other("world read timed out"))??
                {
                    Chunk::Binary(bytes) => decoder.push(&bytes).map_err(io::Error::other)?,
                    Chunk::Closed => return Ok(()),
                    Chunk::Text => return Err(io::Error::other("binary world frames required")),
                }
            }
        };
        let egress = async {
            while let Some(frames) = outgoing.recv().await {
                let mut wire = vec![];
                for frame in frames {
                    frame
                        .write(
                            rs09_protocol::game::Direction::ServerToClient,
                            None,
                            &mut wire,
                        )
                        .map_err(io::Error::other)?;
                }
                self.enqueue(&replies, wire.into(), None).await?;
            }
            self.enqueue(
                &replies,
                Bytes::new(),
                Some((1000, "World session ended".into())),
            )
            .await?;
            std::future::pending::<io::Result<()>>().await
        };
        tokio::select! {r=ingress=>r,r=egress=>r}
    }
    async fn compute_loop(
        &self,
        mut requests: mpsc::Receiver<rs09_protocol::js5::Request>,
        replies: mpsc::Sender<Envelope>,
        key: Arc<AtomicU8>,
    ) -> io::Result<()> {
        while let Some(request) = requests.recv().await {
            if let Some(bytes) = self
                .catalog
                .cached_reply(request, key.load(Ordering::Relaxed))
            {
                self.enqueue(&replies, bytes, None).await?;
                continue;
            }
            let permit = Arc::clone(&self.compute)
                .acquire_owned()
                .await
                .map_err(io::Error::other)?;
            let catalog = Arc::clone(&self.catalog);
            let key = Arc::clone(&key);
            let result = tokio::task::spawn_blocking(move || {
                let _permit = permit;
                catalog.reply(request, key.load(Ordering::Relaxed))
            })
            .await
            .map_err(io::Error::other)?
            .map_err(io::Error::other)?;
            if let Some(bytes) = result {
                self.enqueue(&replies, bytes, None).await?;
            }
        }
        Ok(())
    }
    async fn write_loop(
        &self,
        mut writer: Writer,
        mut replies: mpsc::Receiver<Envelope>,
    ) -> io::Result<()> {
        while let Some(envelope) = replies.recv().await {
            if !envelope.bytes.is_empty() {
                timeout(
                    self.config.write_timeout,
                    writer.write(envelope.bytes.clone()),
                )
                .await
                .map_err(|_| io::Error::other("client write timed out"))??;
                self.metrics.replies.fetch_add(1, Ordering::Relaxed);
                self.metrics
                    .sent_bytes
                    .fetch_add(envelope.bytes.len() as u64, Ordering::Relaxed);
            }
            if let Some((code, reason)) = &envelope.close {
                let _ = timeout(self.config.write_timeout, writer.close(*code, reason)).await;
                return Ok(());
            }
        }
        Ok(())
    }
}
struct AuthWaiting(Arc<Metrics>);
impl Drop for AuthWaiting {
    fn drop(&mut self) {
        self.0.auth_waiters.fetch_sub(1, Ordering::Relaxed);
    }
}
struct Active {
    metrics: Arc<Metrics>,
    _slot: OwnedSemaphorePermit,
}
impl Drop for Active {
    fn drop(&mut self) {
        self.metrics.active.fetch_sub(1, Ordering::Relaxed);
    }
}
struct Budget {
    metrics: Arc<Metrics>,
    weight: usize,
    _permit: OwnedSemaphorePermit,
}
impl Drop for Budget {
    fn drop(&mut self) {
        self.metrics
            .queued_bytes
            .fetch_sub(self.weight, Ordering::Relaxed);
    }
}
struct Envelope {
    bytes: Bytes,
    close: Option<(u16, String)>,
    _budget: Budget,
}
