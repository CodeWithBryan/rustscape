//! The client side of the JS5 update connection: `rt4.Js5NetQueue` and
//! `client.js5Connect`, behind a socket trait so the rules can be scripted.
//!
//! Nothing here blocks or owns a clock: the caller passes the monotonic time
//! in milliseconds, calls [`Js5Connection::network_loop`] once per client
//! cycle (20 ms in the original) and reads [`Js5Connection::fatal`].
//!
//! Replies are reassembled by [`rs09_protocol::js5::ResponseDecoder`], fed
//! exactly the read sizes the Java asks for. Where that differs from the Java
//! on malformed streams is listed on [`Js5NetQueue::tick`].
use rs09_protocol::js5::{self, Request, ResponseDecoder};
use std::{
    collections::VecDeque,
    io,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

/// `Js5NetQueue.read` refuses the 21st request of either priority.
pub const QUEUE_LIMIT: usize = 20;
/// The build `client.js5Connect` announces after opcode 15.
pub const REVISION: u32 = 530;
/// A connection that delivers nothing for this long is dropped and reopened.
pub const LATENCY_LIMIT_MS: i64 = 30_000;
/// Replies longer than this are treated as a broken stream. The Java has no
/// limit and allocates whatever length the header carries.
pub const DEFAULT_MAX_PAYLOAD: usize = 16 * 1024 * 1024;

/// `rt4.BufferedSocket` as the update connection uses it.
pub trait Js5Socket: Send {
    /// Reports a write that failed after [`Js5Socket::write`] returned.
    fn check_error(&mut self) -> io::Result<()>;
    /// Queues bytes for sending; must not wait for the peer.
    fn write(&mut self, bytes: &[u8]) -> io::Result<()>;
    /// Bytes [`Js5Socket::read`] can deliver now. Like `InputStream.available`
    /// this stays zero after the peer has closed; the 30 second rule in
    /// [`Js5NetQueue::tick`] is what notices a dead connection.
    fn available(&mut self) -> io::Result<usize>;
    /// Fills the slice; callers never ask for more than `available` reported.
    fn read(&mut self, destination: &mut [u8]) -> io::Result<()>;
    /// `BufferedSocket.read()`: one byte, waiting up to the socket's 30 second
    /// timeout, or -1 at the end of the stream.
    fn read_blocking(&mut self) -> io::Result<i32>;
    fn close(&mut self);
    /// Whether [`Js5Socket::read_blocking`] may wait for a byte. A socket
    /// that cannot (a browser's, whose thread must return to the page) says
    /// `false`; the handshake reply is then read once it has arrived, under
    /// the 30 second limit the original applies outside loading.
    fn can_wait(&self) -> bool {
        true
    }
}

struct NetState {
    data: Option<Arc<Vec<u8>>>,
    received: usize,
    expected: usize,
}
struct NetInner {
    request: Request,
    trailer_len: u8,
    incomplete: AtomicBool,
    state: Mutex<NetState>,
}
/// `rt4.Js5NetRequest`: shared between the queue and whoever asked.
#[derive(Clone)]
pub struct NetRequest(Arc<NetInner>);
impl NetRequest {
    fn new(request: Request, trailer_len: u8) -> Self {
        Self(Arc::new(NetInner {
            request,
            trailer_len,
            incomplete: AtomicBool::new(true),
            state: Mutex::new(NetState {
                data: None,
                received: 0,
                expected: 0,
            }),
        }))
    }
    pub fn request(&self) -> Request {
        self.0.request
    }
    pub fn urgent(&self) -> bool {
        self.0.request.urgent
    }
    pub fn incomplete(&self) -> bool {
        self.0.incomplete.load(Ordering::Acquire)
    }
    /// Container bytes received over container length; the trailer is not sent.
    pub fn percentage_complete(&self) -> i32 {
        let state = self.0.state.lock().expect("request state");
        (state.received * 100)
            .checked_div(state.expected)
            .map_or(0, |percentage| percentage as i32)
    }
    /// The container followed by `trailer_len` bytes the server never sends
    /// (zero until [`NetRequest::stamp_trailer`]). `None` while incomplete,
    /// where the Java throws.
    pub fn data(&self) -> Option<Arc<Vec<u8>>> {
        if self.incomplete() {
            return None;
        }
        self.0.state.lock().expect("request state").data.clone()
    }
    /// Writes the low 16 bits of a version into the last two bytes, as
    /// `Js5CachedResourceProvider` does before storing a group.
    pub fn stamp_trailer(&self, version: i32) -> Option<Arc<Vec<u8>>> {
        let mut state = self.0.state.lock().expect("request state");
        let data = state.data.as_mut()?;
        let bytes = Arc::make_mut(data);
        let length = bytes.len();
        if length < 2 {
            return None;
        }
        bytes[length - 2] = (version >> 8) as u8;
        bytes[length - 1] = version as u8;
        Some(Arc::clone(data))
    }
    fn same(&self, other: &NetRequest) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

/// `Js5NetQueue.read` on a full queue; the Java throws `RuntimeException`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QueueFull;

/// `rt4.Js5NetQueue`.
pub struct Js5NetQueue {
    socket: Option<Box<dyn Js5Socket>>,
    previous_loop: u64,
    latency: i64,
    current: Option<NetRequest>,
    /// A reply whose container is only its five-byte header. The Java
    /// completes it on the next loop pass that finds bytes available.
    held: Option<(NetRequest, Vec<u8>)>,
    pending_urgent: VecDeque<NetRequest>,
    in_flight_urgent: VecDeque<NetRequest>,
    pending_prefetch: VecDeque<NetRequest>,
    in_flight_prefetch: VecDeque<NetRequest>,
    encryption_key: u8,
    /// Consecutive failures; reset by the first group that passes its CRC.
    pub errors: i32,
    /// 0, a server handshake code, -1 after a CRC failure, -2 after an I/O
    /// error, or 1000/1001/1002 from the connect step.
    pub response: i32,
    /// Reply bytes read from the server since this queue was made.
    pub bytes_received: u64,
    /// Replies completed since this queue was made.
    pub replies_received: u64,
    decoder: ResponseDecoder,
    max_payload: usize,
    random: Box<dyn FnMut() -> f64 + Send>,
}
impl Default for Js5NetQueue {
    fn default() -> Self {
        Self::new()
    }
}
impl Js5NetQueue {
    pub fn new() -> Self {
        // `Math.random` for the XOR key. Its quality does not matter: the key
        // only makes a retried download differ on the wire.
        #[cfg(not(target_arch = "wasm32"))]
        let mut seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0x9e37_79b9_7f4a_7c15, |elapsed| elapsed.as_nanos() as u64)
            | 1;
        // No clock without the page: the host seeds it (`set_random`).
        #[cfg(target_arch = "wasm32")]
        let mut seed = 0x9e37_79b9_7f4a_7c15u64;
        Self::with_random(Box::new(move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed >> 11) as f64 / (1u64 << 53) as f64
        }))
    }
    /// `random` stands in for `Math.random()`: values in `[0, 1)`.
    pub fn with_random(random: Box<dyn FnMut() -> f64 + Send>) -> Self {
        Self {
            socket: None,
            previous_loop: 0,
            latency: 0,
            current: None,
            held: None,
            pending_urgent: VecDeque::new(),
            in_flight_urgent: VecDeque::new(),
            pending_prefetch: VecDeque::new(),
            in_flight_prefetch: VecDeque::new(),
            encryption_key: 0,
            errors: 0,
            response: 0,
            bytes_received: 0,
            replies_received: 0,
            decoder: ResponseDecoder::new(0, DEFAULT_MAX_PAYLOAD),
            max_payload: DEFAULT_MAX_PAYLOAD,
            random,
        }
    }
    pub fn set_max_payload(&mut self, max_payload: usize) {
        self.max_payload = max_payload;
    }
    /// Replaces the stand-in for `Math.random()` that picks the XOR key.
    pub fn set_random(&mut self, random: Box<dyn FnMut() -> f64 + Send>) {
        self.random = random;
    }
    pub fn encryption_key(&self) -> u8 {
        self.encryption_key
    }
    pub fn connected(&self) -> bool {
        self.socket.is_some()
    }
    pub fn urgent_request_count(&self) -> usize {
        self.pending_urgent.len() + self.in_flight_urgent.len()
    }
    pub fn prefetch_request_count(&self) -> usize {
        self.pending_prefetch.len() + self.in_flight_prefetch.len()
    }
    pub fn is_urgent_request_queue_full(&self) -> bool {
        self.urgent_request_count() >= QUEUE_LIMIT
    }
    pub fn is_prefetch_request_queue_full(&self) -> bool {
        self.prefetch_request_count() >= QUEUE_LIMIT
    }
    /// Requests sent and not yet answered, urgent first. For diagnostics.
    pub fn in_flight(&self) -> Vec<Request> {
        self.in_flight_urgent
            .iter()
            .chain(&self.in_flight_prefetch)
            .map(NetRequest::request)
            .collect()
    }

    /// `Js5NetQueue.read`: queues a request for the next [`Self::tick`].
    /// `trailer_len` bytes are reserved after the container for the caller.
    pub fn read(
        &mut self,
        archive: u8,
        trailer_len: u8,
        group: u16,
        urgent: bool,
    ) -> Result<NetRequest, QueueFull> {
        let request = NetRequest::new(
            Request {
                archive,
                group,
                urgent,
            },
            trailer_len,
        );
        if urgent {
            if self.urgent_request_count() >= QUEUE_LIMIT {
                return Err(QueueFull);
            }
            self.pending_urgent.push_back(request.clone());
        } else if self.prefetch_request_count() < QUEUE_LIMIT {
            self.pending_prefetch.push_back(request.clone());
        } else {
            return Err(QueueFull);
        }
        Ok(request)
    }

    fn idle(&self) -> bool {
        self.urgent_request_count() == 0 && self.prefetch_request_count() == 0
    }
    fn fail(&mut self, response: Option<i32>) {
        if let Some(mut socket) = self.socket.take() {
            socket.close();
        }
        self.errors += 1;
        if let Some(response) = response {
            self.response = response;
        }
    }
    fn control(&mut self, message: [u8; 4]) {
        let Some(socket) = self.socket.as_mut() else {
            return;
        };
        if socket.write(&message).is_err() {
            self.fail(Some(-2));
        }
    }

    /// `Js5NetQueue.loop`. Returns `false` when there is no connection and
    /// requests are waiting, which is the caller's cue to connect.
    ///
    /// Per call: accumulated silence is checked against 30 seconds (each call
    /// counts at most 200 ms), every pending request is written (urgent
    /// first, opcode 1; then prefetch, opcode 0) and at most 100 reads are
    /// made, each for the rest of a header, one block marker or the rest of a
    /// 512-byte block. Any I/O error closes the socket, sets `response` to -2
    /// and counts an error; the unanswered requests stay queued.
    ///
    /// Malformed streams, where this is stricter than the Java: a block
    /// marker other than 255 is an I/O error here (the Java forgets the
    /// reply and treats the byte as the start of the next header, which then
    /// fails to match), and a reply longer than the payload limit is an I/O
    /// error (the Java allocates it, or dies on a negative length).
    pub fn tick(&mut self, now_ms: u64) -> bool {
        if self.socket.is_some() {
            let elapsed = (now_ms.saturating_sub(self.previous_loop)).min(200) as i64;
            self.previous_loop = now_ms;
            self.latency += elapsed;
            if self.latency > LATENCY_LIMIT_MS
                && let Some(mut socket) = self.socket.take()
            {
                socket.close();
            }
        }
        let Some(mut socket) = self.socket.take() else {
            return self.idle();
        };
        match self.pump(socket.as_mut()) {
            Ok(()) => {
                self.socket = Some(socket);
                true
            }
            Err(_) => {
                socket.close();
                self.response = -2;
                self.errors += 1;
                self.idle()
            }
        }
    }

    fn pump(&mut self, socket: &mut dyn Js5Socket) -> io::Result<()> {
        socket.check_error()?;
        // A request moves to the in-flight list only once its write succeeded.
        while let Some(request) = self.pending_urgent.front() {
            socket.write(&request.request().encode())?;
            let request = self.pending_urgent.pop_front().expect("front");
            self.in_flight_urgent.push_back(request);
        }
        while let Some(request) = self.pending_prefetch.front() {
            socket.write(&request.request().encode())?;
            let request = self.pending_prefetch.pop_front().expect("front");
            self.in_flight_prefetch.push_back(request);
        }
        let mut scratch = [0u8; 512];
        for _ in 0..100 {
            let available = socket.available()?;
            if available == 0 {
                break;
            }
            self.latency = 0;
            if let Some((request, container)) = self.held.take() {
                self.complete(request, container);
                continue;
            }
            let count = self.decoder.wanted().min(available);
            socket.read(&mut scratch[..count])?;
            self.bytes_received += count as u64;
            let awaiting_header = self.decoder.partial().is_none();
            let response = self
                .decoder
                .feed(&scratch[..count])
                .map_err(io::Error::other)?
                .pop();
            if awaiting_header {
                let announced = self
                    .decoder
                    .partial()
                    .map(|(request, ..)| request)
                    .or(response.as_ref().map(|response| response.request));
                if let Some(announced) = announced {
                    let list = if announced.urgent {
                        &self.in_flight_urgent
                    } else {
                        &self.in_flight_prefetch
                    };
                    let matched = list
                        .iter()
                        .find(|request| request.request() == announced)
                        .ok_or_else(|| io::Error::other("reply matches no request in flight"))?;
                    self.current = Some(matched.clone());
                }
            }
            if let (Some(current), Some((_, received, expected))) =
                (&self.current, self.decoder.partial())
            {
                let mut state = current.0.state.lock().expect("request state");
                state.received = received;
                state.expected = expected;
            }
            if let Some(response) = response {
                let request = self
                    .current
                    .take()
                    .ok_or_else(|| io::Error::other("reply completed without a header"))?;
                if response.container.len() == 5 {
                    let mut state = request.0.state.lock().expect("request state");
                    state.received = 5;
                    state.expected = 5;
                    drop(state);
                    self.held = Some((request, response.container));
                } else {
                    self.complete(request, response.container);
                }
            }
        }
        Ok(())
    }

    fn complete(&mut self, request: NetRequest, mut container: Vec<u8>) {
        let length = container.len();
        container.resize(length + usize::from(request.0.trailer_len), 0);
        {
            let mut state = request.0.state.lock().expect("request state");
            state.received = length;
            state.expected = length;
            state.data = Some(Arc::new(container));
        }
        let list = if request.urgent() {
            &mut self.in_flight_urgent
        } else {
            &mut self.in_flight_prefetch
        };
        if let Some(position) = list.iter().position(|other| other.same(&request)) {
            list.remove(position);
        }
        request.0.incomplete.store(false, Ordering::Release);
        self.replies_received += 1;
    }

    /// `Js5NetQueue.start`: adopts a socket whose handshake was accepted.
    /// Sends `6 0 0 3`, then the logged-in (2) or logged-out (3) notice,
    /// returns every unanswered request to the pending lists behind the ones
    /// already waiting, and, when a key is set, sends `4 key 0 0`.
    pub fn start(&mut self, logged_in: bool, socket: Box<dyn Js5Socket>, now_ms: u64) {
        if let Some(mut old) = self.socket.take() {
            old.close();
        }
        self.socket = Some(socket);
        self.control([6, 0, 0, 3]);
        self.write_logged_in(logged_in);
        self.decoder = ResponseDecoder::new(self.encryption_key, self.max_payload);
        self.current = None;
        self.held = None;
        self.pending_urgent.extend(self.in_flight_urgent.drain(..));
        self.pending_prefetch
            .extend(self.in_flight_prefetch.drain(..));
        if self.encryption_key != 0 {
            self.control([4, self.encryption_key, 0, 0]);
        }
        self.latency = 0;
        self.previous_loop = now_ms;
    }
    /// Opcode 2 when a player is in the game, 3 on the title screen.
    pub fn write_logged_in(&mut self, logged_in: bool) {
        self.control([if logged_in { 2 } else { 3 }, 0, 0, 0]);
    }
    /// `Js5NetQueue.drop`: asks the server to close (opcode 7).
    pub fn drop_connection(&mut self) {
        self.control([7, 0, 0, 0]);
    }
    /// `Js5NetQueue.rekey`: a reply failed its checksum. Closes the socket,
    /// picks a new XOR key in 1..=255 for the next connection, counts an
    /// error and sets `response` to -1.
    pub fn rekey(&mut self) {
        if let Some(mut socket) = self.socket.take() {
            socket.close();
        }
        self.response = -1;
        self.encryption_key = ((self.random)() * 255.0 + 1.0) as u8;
        self.errors += 1;
    }
    /// `Js5NetQueue.quit`: closes the socket; the queue still believes it is
    /// connected until 30 seconds of silence, as in the Java.
    pub fn quit(&mut self) {
        if let Some(socket) = self.socket.as_mut() {
            socket.close();
        }
    }
}

/// What `client.error` is called with before the client stops (state 1000).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FatalError {
    /// Two failures and the server answered the handshake with 6.
    OutOfDate,
    /// Four failures, the last a checksum mismatch.
    Crc,
    /// Four failures while loading, the last a 7 or 9 from the server.
    Full,
    /// Four failures while loading, the last any other positive code.
    Connect,
    /// Four failures while loading, the last an I/O error.
    Io,
}
impl FatalError {
    /// The original error string.
    pub fn message(self) -> &'static str {
        match self {
            Self::OutOfDate => "js5connect_outofdate",
            Self::Crc => "js5crc",
            Self::Full => "js5connect_full",
            Self::Connect => "js5connect",
            Self::Io => "js5io",
        }
    }
}

/// `SignLink.openSocket` result.
pub enum OpenStatus {
    Pending,
    Ready(Box<dyn Js5Socket>),
    Failed,
}
pub trait SocketRequest: Send {
    fn poll(&mut self) -> OpenStatus;
}
/// Starts opening a connection to the update server on the given port.
pub trait SocketOpener: Send {
    fn open(&mut self, port: u16) -> Box<dyn SocketRequest>;
}

/// `client.js5NetworkLoop` and `client.js5Connect` with the fields they use.
pub struct Js5Connection {
    pub queue: Js5NetQueue,
    opener: Box<dyn SocketOpener>,
    connect_state: u8,
    socket_request: Option<Box<dyn SocketRequest>>,
    socket: Option<Box<dyn Js5Socket>>,
    connect_time: u64,
    previous_errors: i32,
    /// Client cycles to wait before the next attempt.
    connect_delay: i32,
    pub default_port: u16,
    pub alternate_port: u16,
    /// The port the next attempt uses; swapped after every failure.
    pub port: u16,
    /// The original `gameState`: 0 loading, 5 loading behind the title
    /// screen, 10 title screen, 25/28 building a map, 30 in game, 40
    /// reconnecting, 1000 stopped. Set by the caller; set to 1000 here when a
    /// fatal error is raised.
    pub game_state: i32,
    pub fatal: Option<FatalError>,
}
impl Js5Connection {
    pub fn new(opener: Box<dyn SocketOpener>, default_port: u16, alternate_port: u16) -> Self {
        Self {
            queue: Js5NetQueue::new(),
            opener,
            connect_state: 0,
            socket_request: None,
            socket: None,
            connect_time: 0,
            previous_errors: 0,
            connect_delay: 0,
            default_port,
            alternate_port,
            port: default_port,
            game_state: 0,
            fatal: None,
        }
    }
    pub fn connect_delay(&self) -> i32 {
        self.connect_delay
    }
    /// `client.setGameState`, as far as this connection is concerned: the
    /// server is told when the client moves between the title screen (states
    /// 5, 10 and 28) and everything else.
    pub fn set_game_state(&mut self, state: i32) {
        let title = |state: i32| state == 5 || state == 10 || state == 28;
        if title(self.game_state) != title(state) {
            self.queue.write_logged_in(!title(state));
        }
        self.game_state = state;
    }
    /// One client cycle of update-server I/O. Does nothing once stopped.
    pub fn network_loop(&mut self, now_ms: u64) {
        if self.game_state == 1000 {
            return;
        }
        if !self.queue.tick(now_ms) {
            self.connect(now_ms);
        }
    }
    fn stop(&mut self, error: FatalError) {
        self.fatal = Some(error);
        self.game_state = 1000;
    }
    fn set_response(&mut self, response: i32) {
        self.queue.errors += 1;
        self.socket_request = None;
        self.queue.response = response;
        self.socket = None;
        self.connect_state = 0;
    }
    /// `client.js5Connect`. After each new failure the wait before the next
    /// attempt is `250 * (errors - 1)` cycles, at most 3000, and the port
    /// alternates between the default and the alternate one.
    fn connect(&mut self, now_ms: u64) {
        let errors = self.queue.errors;
        let response = self.queue.response;
        if self.previous_errors < errors {
            self.connect_delay = 5 * 50 * (errors - 1);
            self.port = if self.default_port == self.port {
                self.alternate_port
            } else {
                self.default_port
            };
            if self.connect_delay > 3000 {
                self.connect_delay = 3000;
            }
            if errors >= 2 && response == 6 {
                return self.stop(FatalError::OutOfDate);
            }
            if errors >= 4 && response == -1 {
                return self.stop(FatalError::Crc);
            }
            if errors >= 4 && (self.game_state == 0 || self.game_state == 5) {
                return self.stop(if response == 7 || response == 9 {
                    FatalError::Full
                } else if response > 0 {
                    FatalError::Connect
                } else {
                    FatalError::Io
                });
            }
        }
        self.previous_errors = errors;
        if self.connect_delay > 0 {
            self.connect_delay -= 1;
            return;
        }
        if self.handshake(now_ms).is_err() {
            self.set_response(1002);
        }
    }
    fn handshake(&mut self, now_ms: u64) -> io::Result<()> {
        if self.connect_state == 0 {
            self.socket_request = Some(self.opener.open(self.port));
            self.connect_state += 1;
        }
        if self.connect_state == 1 {
            let status = self
                .socket_request
                .as_mut()
                .map_or(OpenStatus::Failed, |request| request.poll());
            match status {
                OpenStatus::Failed => {
                    self.set_response(1000);
                    return Ok(());
                }
                OpenStatus::Ready(socket) => {
                    self.socket = Some(socket);
                    self.connect_state += 1;
                }
                OpenStatus::Pending => {}
            }
        }
        if self.connect_state == 2 {
            let socket = self.socket.as_mut().ok_or(io::ErrorKind::NotConnected)?;
            socket.write(&js5::handshake(REVISION))?;
            self.connect_state += 1;
            self.connect_time = now_ms;
        }
        if self.connect_state == 3 {
            let socket = self.socket.as_mut().ok_or(io::ErrorKind::NotConnected)?;
            // While loading the original waits here for the reply.
            let waits = (self.game_state == 0 || self.game_state == 5) && socket.can_wait();
            if waits || socket.available()? > 0 {
                let response = socket.read_blocking()?;
                if response != 0 {
                    self.set_response(response);
                    return Ok(());
                }
                self.connect_state += 1;
            } else if now_ms.saturating_sub(self.connect_time) > 30_000 {
                self.set_response(1001);
                return Ok(());
            }
        }
        if self.connect_state == 4 {
            let logged_out = matches!(self.game_state, 5 | 10 | 28);
            let socket = self.socket.take().ok_or(io::ErrorKind::NotConnected)?;
            self.queue.start(!logged_out, socket, now_ms);
            self.socket_request = None;
            self.connect_state = 0;
        }
        Ok(())
    }
}

/// A scripted socket for tests of this module and of the code built on it.
pub mod scripted {
    use super::{Js5Socket, OpenStatus, SocketOpener, SocketRequest};
    use rs09_protocol::{
        checksum::crc32,
        js5::{Request, encode_archive, encode_master, master_payload},
    };
    use std::{
        collections::{BTreeMap, BTreeSet, VecDeque},
        io,
        sync::{Arc, Mutex},
    };

    #[derive(Default)]
    pub struct Wire {
        /// Everything the client wrote, in order.
        pub written: Vec<u8>,
        /// Bytes the client has not read yet.
        pub incoming: VecDeque<u8>,
        /// At most this many bytes are reported available per call.
        pub chunk: Option<usize>,
        pub closed: bool,
        pub fail_writes: bool,
        pub fail_check: bool,
        pub fail_available: bool,
        /// Handed out by `read_blocking` before `incoming` is consulted.
        pub handshake: VecDeque<i32>,
    }
    /// One end of a scripted connection; clones share the same wire.
    #[derive(Clone, Default)]
    pub struct ScriptedSocket(pub Arc<Mutex<Wire>>);
    impl ScriptedSocket {
        pub fn new() -> Self {
            Self::default()
        }
        pub fn wire(&self) -> std::sync::MutexGuard<'_, Wire> {
            self.0.lock().expect("scripted wire")
        }
        pub fn push(&self, bytes: &[u8]) {
            self.wire().incoming.extend(bytes);
        }
        pub fn take_written(&self) -> Vec<u8> {
            std::mem::take(&mut self.wire().written)
        }
    }
    impl Js5Socket for ScriptedSocket {
        fn check_error(&mut self) -> io::Result<()> {
            if std::mem::take(&mut self.wire().fail_check) {
                return Err(io::Error::other("scripted write error"));
            }
            Ok(())
        }
        fn write(&mut self, bytes: &[u8]) -> io::Result<()> {
            let mut wire = self.wire();
            if wire.fail_writes {
                return Err(io::Error::other("scripted write failure"));
            }
            wire.written.extend_from_slice(bytes);
            Ok(())
        }
        fn available(&mut self) -> io::Result<usize> {
            let wire = self.wire();
            if wire.fail_available {
                return Err(io::Error::other("scripted read failure"));
            }
            Ok(wire
                .chunk
                .map_or(wire.incoming.len(), |chunk| chunk.min(wire.incoming.len())))
        }
        fn read(&mut self, destination: &mut [u8]) -> io::Result<()> {
            let mut wire = self.wire();
            if wire.incoming.len() < destination.len() {
                return Err(io::ErrorKind::UnexpectedEof.into());
            }
            for byte in destination {
                *byte = wire.incoming.pop_front().expect("length checked");
            }
            Ok(())
        }
        fn read_blocking(&mut self) -> io::Result<i32> {
            let mut wire = self.wire();
            if let Some(value) = wire.handshake.pop_front() {
                return Ok(value);
            }
            match wire.incoming.pop_front() {
                Some(byte) => Ok(i32::from(byte)),
                None => Err(io::ErrorKind::TimedOut.into()),
            }
        }
        fn close(&mut self) {
            self.wire().closed = true;
        }
    }

    /// What the next `open` call yields.
    pub enum Next {
        Socket(ScriptedSocket),
        Failure,
        Pending,
    }
    #[derive(Default)]
    pub struct OpenerScript {
        pub next: VecDeque<Next>,
        /// The port of every attempt, in order.
        pub ports: Vec<u16>,
    }
    #[derive(Clone, Default)]
    pub struct ScriptedOpener(pub Arc<Mutex<OpenerScript>>);
    struct Opening(Option<Next>);
    impl SocketRequest for Opening {
        fn poll(&mut self) -> OpenStatus {
            match self.0.take() {
                Some(Next::Socket(socket)) => OpenStatus::Ready(Box::new(socket)),
                Some(Next::Pending) => {
                    self.0 = Some(Next::Pending);
                    OpenStatus::Pending
                }
                Some(Next::Failure) | None => OpenStatus::Failed,
            }
        }
    }
    impl SocketOpener for ScriptedOpener {
        fn open(&mut self, port: u16) -> Box<dyn SocketRequest> {
            let mut script = self.0.lock().expect("opener script");
            script.ports.push(port);
            Box::new(Opening(script.next.pop_front()))
        }
    }

    /// An in-memory update server for tests. It answers through the Rust
    /// server's reply formatter ([`rs09_protocol::js5::encode_archive`] and
    /// [`rs09_protocol::js5::encode_master`], both checked against the
    /// original server on a JVM), so replies have the pinned server's shape:
    /// groups are obfuscated with the client's key, the master index is not.
    /// Every group it serves holds one file and is stored uncompressed.
    pub struct FakeServer {
        archive_count: usize,
        indexes: BTreeMap<u8, Vec<u8>>,
        versions: BTreeMap<u8, i32>,
        groups: BTreeMap<(u8, u16), Vec<u8>>,
        inbox: Vec<u8>,
        fresh: bool,
        /// The XOR key the client last announced on this connection.
        pub key: u8,
        pub connections: usize,
        /// Every request seen: archive, group, urgent.
        pub requested: Vec<(u8, u16, bool)>,
        pub urgent_requests: usize,
        pub prefetch_requests: usize,
        /// Most prefetch requests received in one `serve` call.
        pub peak_outstanding: usize,
        /// The next reply for each of these has one byte flipped.
        pub corrupt_once: BTreeSet<(u8, u16)>,
        /// Requests for these are never answered.
        pub silent: BTreeSet<(u8, u16)>,
        /// Control messages seen (opcodes 2, 3, 6 and 7), in order.
        pub controls: Vec<u8>,
    }
    fn container(payload: &[u8]) -> Vec<u8> {
        let mut bytes = vec![0];
        bytes.extend((payload.len() as u32).to_be_bytes());
        bytes.extend_from_slice(payload);
        bytes
    }
    impl FakeServer {
        /// A server whose master index has `archive_count` entries.
        pub fn new(archive_count: usize) -> Self {
            Self {
                archive_count,
                indexes: BTreeMap::new(),
                versions: BTreeMap::new(),
                groups: BTreeMap::new(),
                inbox: Vec::new(),
                fresh: true,
                key: 0,
                connections: 1,
                requested: Vec::new(),
                urgent_requests: 0,
                prefetch_requests: 0,
                peak_outstanding: 0,
                corrupt_once: BTreeSet::new(),
                silent: BTreeSet::new(),
                controls: Vec::new(),
            }
        }
        /// Groups are `(id, payload, version)`, IDs ascending.
        pub fn add_archive(&mut self, archive: u8, version: i32, groups: &[(u32, Vec<u8>, i32)]) {
            let named: Vec<_> = groups
                .iter()
                .map(|(id, payload, version)| (*id, None, payload.clone(), *version))
                .collect();
            self.add(archive, version, &named);
        }
        /// As [`Self::add_archive`], with a name per group.
        pub fn add_named_archive(
            &mut self,
            archive: u8,
            version: i32,
            groups: &[(u32, &str, Vec<u8>, i32)],
        ) {
            let named: Vec<_> = groups
                .iter()
                .map(|(id, name, payload, version)| (*id, Some(*name), payload.clone(), *version))
                .collect();
            self.add(archive, version, &named);
        }
        fn add(&mut self, archive: u8, version: i32, groups: &[(u32, Option<&str>, Vec<u8>, i32)]) {
            let names = groups.iter().any(|group| group.1.is_some());
            let hash = |name: Option<&str>| {
                name.map_or(-1i32, |name| {
                    name.bytes().fold(0i32, |hash, byte| {
                        hash.wrapping_mul(31).wrapping_add(i32::from(byte))
                    })
                })
            };
            let mut index = vec![6];
            index.extend(version.to_be_bytes());
            index.push(u8::from(names));
            index.extend((groups.len() as u16).to_be_bytes());
            let mut last = 0;
            for group in groups {
                index.extend(((group.0 - last) as u16).to_be_bytes());
                last = group.0;
            }
            if names {
                for group in groups {
                    index.extend(hash(group.1).to_be_bytes());
                }
            }
            for group in groups {
                let packed = container(&group.2);
                index.extend(crc32(&packed).to_be_bytes());
                self.groups.insert((archive, group.0 as u16), packed);
            }
            for group in groups {
                index.extend(group.3.to_be_bytes());
            }
            for _ in groups {
                index.extend(1u16.to_be_bytes());
            }
            for _ in groups {
                index.extend(0u16.to_be_bytes());
            }
            if names {
                for _ in groups {
                    index.extend(0i32.to_be_bytes()); // the file named ""
                }
            }
            self.indexes.insert(archive, container(&index));
            self.versions.insert(archive, version);
        }
        /// The CRC and version the master index carries for an archive.
        pub fn master_entry(&self, archive: u8) -> (u32, i32) {
            match self.indexes.get(&archive) {
                Some(index) => (crc32(index), self.versions[&archive]),
                None => (if archive == 24 { 609_698_396 } else { 0 }, 0),
            }
        }
        /// An archive's index container, as the server stores it.
        pub fn index_container(&self, archive: u8) -> Vec<u8> {
            self.indexes[&archive].clone()
        }
        /// A group's container without its version trailer.
        pub fn group_container(&self, archive: u8, group: u16) -> Vec<u8> {
            self.groups[&(archive, group)].clone()
        }
        pub fn reset_counters(&mut self) {
            self.requested.clear();
            self.urgent_requests = 0;
            self.prefetch_requests = 0;
            self.peak_outstanding = 0;
        }
        /// The client opened a new connection: the key starts at zero again.
        pub fn reconnected(&mut self) {
            self.connections += 1;
            self.key = 0;
            self.inbox.clear();
            self.fresh = true;
        }
        /// Reads what the client wrote and queues the replies.
        pub fn serve(&mut self, socket: &ScriptedSocket) {
            self.inbox.extend(socket.take_written());
            if self.fresh && self.inbox.first() == Some(&15) {
                if self.inbox.len() < 5 {
                    return;
                }
                self.inbox.drain(..5);
            }
            if !self.inbox.is_empty() {
                self.fresh = false;
            }
            let mut prefetch = 0;
            while self.inbox.len() >= 4 {
                let message: Vec<u8> = self.inbox.drain(..4).collect();
                let group = u16::from_be_bytes([message[2], message[3]]);
                let request = Request {
                    archive: message[1],
                    group,
                    urgent: message[0] == 1,
                };
                match message[0] {
                    0 | 1 => {}
                    4 => {
                        self.key = message[1];
                        continue;
                    }
                    other => {
                        self.controls.push(other);
                        continue;
                    }
                }
                self.requested
                    .push((request.archive, group, request.urgent));
                if request.urgent {
                    self.urgent_requests += 1;
                } else {
                    self.prefetch_requests += 1;
                    prefetch += 1;
                }
                if self.silent.contains(&(request.archive, group)) {
                    continue;
                }
                let mut reply = if (request.archive, group) == (255, 255) {
                    let entries: Vec<_> = (0..self.archive_count)
                        .map(|archive| {
                            self.indexes
                                .contains_key(&(archive as u8))
                                .then(|| self.master_entry(archive as u8))
                        })
                        .collect();
                    encode_master(&master_payload(&entries), 1 << 24).expect("master reply")
                } else if request.archive == 255 {
                    match self.indexes.get(&(group as u8)) {
                        Some(index) => {
                            encode_archive(request, self.key, index, 1 << 24).expect("index reply")
                        }
                        None => continue,
                    }
                } else {
                    match self.groups.get(&(request.archive, group)) {
                        Some(packed) => {
                            // Stored with a trailer, which is not transmitted.
                            let mut stored = packed.clone();
                            stored.extend([0xee, 0xee]);
                            encode_archive(request, self.key, &stored, 1 << 24)
                                .expect("group reply")
                        }
                        None => continue,
                    }
                };
                if self.corrupt_once.remove(&(request.archive, group)) {
                    let last = reply.len() - 1;
                    reply[last] ^= 0x5a;
                }
                socket.push(&reply);
            }
            self.peak_outstanding = self.peak_outstanding.max(prefetch);
        }
    }
}

#[cfg(test)]
mod tests {
    //! Transcription tests: each asserts a rule read off `Js5NetQueue.java`
    //! or `client.js5Connect`, not behaviour observed from a running JVM.
    //! The `fixtures/network` vectors used further down are JVM output, but
    //! of the original *server's* formatter.
    use super::{scripted::*, *};
    use rs09_protocol::js5::encode_archive;

    fn queue_with_socket() -> (Js5NetQueue, ScriptedSocket) {
        let mut queue = Js5NetQueue::with_random(Box::new(|| 0.5));
        let socket = ScriptedSocket::new();
        queue.start(true, Box::new(socket.clone()), 0);
        (queue, socket)
    }
    fn reply(archive: u8, group: u16, urgent: bool, key: u8, packed: &[u8]) -> Vec<u8> {
        encode_archive(
            Request {
                archive,
                group,
                urgent,
            },
            key,
            packed,
            1 << 24,
        )
        .unwrap()
    }
    fn container(compression: u8, payload: &[u8]) -> Vec<u8> {
        let mut packed = vec![compression];
        packed.extend((payload.len() as u32).to_be_bytes());
        if compression != 0 {
            packed.extend(7u32.to_be_bytes());
        }
        packed.extend_from_slice(payload);
        packed.extend([0x12, 0x34]); // version trailer, never transmitted
        packed
    }

    #[test]
    fn each_priority_takes_twenty_requests() {
        let mut queue = Js5NetQueue::new();
        for group in 0..20 {
            assert!(queue.read(3, 0, group, true).is_ok());
            assert!(queue.read(3, 0, group, false).is_ok());
        }
        assert!(queue.is_urgent_request_queue_full());
        assert!(queue.is_prefetch_request_queue_full());
        assert_eq!(queue.read(3, 0, 20, true).err(), Some(QueueFull));
        assert_eq!(queue.read(3, 0, 20, false).err(), Some(QueueFull));
        assert_eq!(queue.urgent_request_count(), 20);
        assert_eq!(queue.prefetch_request_count(), 20);
        // Without a socket the loop only reports that work is waiting.
        assert!(!queue.tick(0));
        assert!(Js5NetQueue::new().tick(0));
    }

    #[test]
    fn start_announces_itself_and_requests_go_out_urgent_first() {
        let (mut queue, socket) = queue_with_socket();
        assert_eq!(socket.take_written(), [6, 0, 0, 3, 2, 0, 0, 0]);
        queue.read(7, 2, 0x1234, false).unwrap();
        queue.read(255, 0, 255, true).unwrap();
        queue.read(5, 2, 9, false).unwrap();
        queue.read(255, 0, 3, true).unwrap();
        assert!(queue.tick(1));
        assert_eq!(
            socket.take_written(),
            [
                1, 255, 0, 255, 1, 255, 0, 3, // urgent, in request order
                0, 7, 0x12, 0x34, 0, 5, 0, 9, // then prefetch
            ]
        );
        assert_eq!(queue.in_flight().len(), 4);
        // Nothing is written twice.
        assert!(queue.tick(2));
        assert!(socket.take_written().is_empty());
        queue.write_logged_in(false);
        queue.drop_connection();
        assert_eq!(socket.take_written(), [3, 0, 0, 0, 7, 0, 0, 0]);
    }

    #[test]
    fn replies_are_matched_reassembled_and_given_a_blank_trailer() {
        let (mut queue, socket) = queue_with_socket();
        let small = container(0, &[1, 2, 3]);
        let large_payload: Vec<u8> = (0..2000u32).map(|i| (i * 7) as u8).collect();
        let large = container(2, &large_payload);
        let a = queue.read(4, 2, 1, true).unwrap();
        let b = queue.read(4, 2, 2, false).unwrap();
        queue.tick(1);
        // Answered out of order, split into awkward pieces.
        socket.push(&reply(4, 2, false, 0, &large));
        socket.push(&reply(4, 1, true, 0, &small));
        socket.wire().chunk = Some(37);
        assert!(a.incomplete() && b.incomplete());
        assert_eq!(a.data(), None);
        let mut now = 1;
        while a.incomplete() {
            now += 1;
            assert!(queue.tick(now));
            assert!(now < 100);
        }
        let mut expected = large[..large.len() - 2].to_vec();
        expected.extend([0, 0]);
        assert_eq!(*b.data().unwrap(), expected);
        assert_eq!(*a.data().unwrap(), [0, 0, 0, 0, 3, 1, 2, 3, 0, 0]);
        assert_eq!(
            (a.percentage_complete(), b.percentage_complete()),
            (100, 100)
        );
        assert!(queue.in_flight().is_empty());
        assert!(queue.tick(now + 1));
        // The version the index names goes into the two spare bytes.
        let stamped = a.stamp_trailer(0x1_abcd).unwrap();
        assert_eq!(stamped[8..], [0xab, 0xcd]);
        assert_eq!(a.data().unwrap()[8..], [0xab, 0xcd]);
    }

    #[test]
    fn one_loop_makes_at_most_one_hundred_reads() {
        let (mut queue, socket) = queue_with_socket();
        let payload = vec![9u8; 60_000];
        let packed = container(0, &payload);
        let request = queue.read(0, 2, 0, false).unwrap();
        queue.tick(1);
        socket.push(&reply(0, 0, false, 0, &packed));
        let total = socket.wire().incoming.len();
        queue.tick(2);
        // Read 1 is the header, read 2 the 504 bytes left in the first block;
        // the other 98 alternate one marker byte and 511 payload bytes.
        let consumed = total - socket.wire().incoming.len();
        assert_eq!(consumed, 8 + 504 + 49 * (1 + 511));
        assert_eq!(
            request.percentage_complete(),
            (5 + 504 + 49 * 511) * 100 / (60_000 + 5)
        );
        // One byte at a time, each read is a loop iteration.
        socket.wire().chunk = Some(1);
        let before = socket.wire().incoming.len();
        queue.tick(3);
        assert_eq!(before - socket.wire().incoming.len(), 100);
    }

    #[test]
    fn an_empty_container_completes_on_the_next_available_byte() {
        let (mut queue, socket) = queue_with_socket();
        let empty = queue.read(1, 2, 0, true).unwrap();
        let next = queue.read(1, 2, 1, true).unwrap();
        queue.tick(1);
        socket.push(&reply(1, 0, true, 0, &[0, 0, 0, 0, 0]));
        queue.tick(2);
        assert!(empty.incomplete());
        assert_eq!(empty.percentage_complete(), 100);
        queue.tick(3);
        assert!(empty.incomplete());
        socket.push(&reply(1, 1, true, 0, &container(0, &[5])));
        queue.tick(4);
        assert_eq!(*empty.data().unwrap(), [0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(*next.data().unwrap(), [0, 0, 0, 0, 1, 5, 0, 0]);
    }

    #[test]
    fn a_reply_nobody_asked_for_is_an_io_error_and_requests_survive_it() {
        let (mut queue, socket) = queue_with_socket();
        let wanted = queue.read(2, 2, 5, true).unwrap();
        let prefetch = queue.read(2, 2, 6, false).unwrap();
        queue.tick(1);
        socket.take_written();
        // Right group, wrong priority bit.
        socket.push(&reply(2, 5, false, 0, &container(0, &[1])));
        assert!(!queue.tick(2));
        assert_eq!((queue.errors, queue.response), (1, -2));
        assert!(socket.wire().closed && !queue.connected());
        assert!(wanted.incomplete() && prefetch.incomplete());
        assert!(!queue.tick(3));
        // A new request made while disconnected goes out before the retries.
        let late = queue.read(2, 2, 7, true).unwrap();
        let second = ScriptedSocket::new();
        queue.start(false, Box::new(second.clone()), 10);
        assert!(queue.tick(11));
        assert_eq!(
            second.take_written(),
            [
                6, 0, 0, 3, 3, 0, 0, 0, // no key message: the key is still zero
                1, 2, 0, 7, 1, 2, 0, 5, 0, 2, 0, 6,
            ]
        );
        second.push(&reply(2, 5, true, 0, &container(0, &[1])));
        queue.tick(12);
        assert_eq!(*wanted.data().unwrap(), [0, 0, 0, 0, 1, 1, 0, 0]);
        assert!(late.incomplete());
    }

    #[test]
    fn a_failed_write_leaves_the_request_pending() {
        let (mut queue, socket) = queue_with_socket();
        queue.read(2, 2, 5, true).unwrap();
        socket.wire().fail_writes = true;
        assert!(!queue.tick(1));
        assert_eq!((queue.errors, queue.response), (1, -2));
        assert!(queue.in_flight().is_empty());
        assert_eq!(queue.urgent_request_count(), 1);
        // A write error reported later surfaces through check_error.
        let (mut queue, socket) = queue_with_socket();
        socket.wire().fail_check = true;
        assert!(queue.tick(1));
        assert_eq!((queue.errors, queue.response), (1, -2));
        // The control messages count errors the same way.
        let (mut queue, socket) = queue_with_socket();
        socket.wire().fail_writes = true;
        queue.write_logged_in(true);
        assert_eq!((queue.errors, queue.response), (1, -2));
        assert!(!queue.connected());
        queue.drop_connection(); // no socket: nothing happens
        assert_eq!(queue.errors, 1);
    }

    #[test]
    fn thirty_seconds_of_silence_close_the_connection_without_an_error() {
        let (mut queue, socket) = queue_with_socket();
        let request = queue.read(2, 2, 5, true).unwrap();
        // Each loop counts at most 200 ms however long it really took.
        let mut now = 0;
        for _ in 0..150 {
            now += 1000;
            assert!(queue.tick(now));
        }
        assert!(queue.connected());
        now += 200;
        assert!(!queue.tick(now));
        assert!(socket.wire().closed);
        assert_eq!((queue.errors, queue.response), (0, 0));
        assert!(request.incomplete());
        // Any arriving byte restarts the count.
        let (mut queue, socket) = queue_with_socket();
        queue.read(2, 2, 5, true).unwrap();
        for step in 1..=149 {
            queue.tick(step * 200);
        }
        socket.push(&[2]);
        for step in 150..=298 {
            assert!(queue.tick(step * 200));
        }
        assert!(queue.connected());
    }

    #[test]
    fn a_checksum_failure_rekeys_and_the_next_connection_is_obfuscated() {
        let (mut queue, socket) = queue_with_socket();
        let request = queue.read(9, 2, 1, true).unwrap();
        queue.tick(1);
        queue.rekey();
        assert_eq!((queue.errors, queue.response), (1, -1));
        assert!(socket.wire().closed);
        // (byte) (0.5 * 255.0 + 1.0)
        assert_eq!(queue.encryption_key(), 128);
        let second = ScriptedSocket::new();
        queue.start(true, Box::new(second.clone()), 5);
        assert_eq!(
            second.take_written(),
            [6, 0, 0, 3, 2, 0, 0, 0, 4, 128, 0, 0]
        );
        queue.tick(6);
        assert_eq!(second.take_written(), [1, 9, 0, 1]);
        let packed = container(1, &[7; 700]);
        second.push(&reply(9, 1, true, 128, &packed));
        queue.tick(7);
        assert_eq!(
            request.data().unwrap()[..packed.len() - 2],
            packed[..packed.len() - 2]
        );
        // Every value Math.random can return gives a key in 1..=255.
        for random in [0.0, 0.003, 0.999_999_9] {
            let mut queue = Js5NetQueue::with_random(Box::new(move || random));
            queue.rekey();
            assert!(queue.encryption_key() >= 1);
        }
    }

    #[test]
    fn malformed_replies_are_io_errors() {
        // A block marker that is not 255.
        let (mut queue, socket) = queue_with_socket();
        queue.read(0, 2, 0, true).unwrap();
        queue.tick(1);
        let mut bytes = reply(0, 0, true, 0, &container(0, &[3; 600]));
        bytes[512] = 0;
        socket.push(&bytes);
        assert!(!queue.tick(2));
        assert_eq!((queue.errors, queue.response), (1, -2));
        // A length beyond the limit, and a negative one.
        for length in [[0x7f, 0xff, 0xff, 0xff], [0xff, 0xff, 0xff, 0xff]] {
            let (mut queue, socket) = queue_with_socket();
            queue.read(0, 2, 0, true).unwrap();
            queue.tick(1);
            socket.push(&[0, 0, 0, 0]);
            socket.push(&length);
            assert!(!queue.tick(2));
            assert_eq!((queue.errors, queue.response), (1, -2));
        }
        let (mut queue, socket) = queue_with_socket();
        socket.wire().fail_available = true;
        assert!(queue.tick(1));
        assert_eq!((queue.errors, queue.response), (1, -2));
    }

    fn fixture() -> Vec<serde_json::Value> {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/network/oracle.jsonl"
        );
        std::fs::read_to_string(path)
            .expect("fixtures/network/oracle.jsonl")
            .lines()
            .map(|line| serde_json::from_str(line).expect("fixture line"))
            .collect()
    }
    fn hex(value: &serde_json::Value) -> Vec<u8> {
        let text = value.as_str().expect("hex string");
        (0..text.len() / 2)
            .map(|i| u8::from_str_radix(&text[i * 2..i * 2 + 2], 16).expect("hex"))
            .collect()
    }

    /// The original server's `Cache.getArchiveData` output (JVM fixture) is
    /// what this queue has to turn back into the stored container.
    #[test]
    fn the_original_servers_replies_reassemble_to_the_stored_container() {
        let (mut clean, mut padded) = (0, 0);
        for case in fixture()
            .iter()
            .filter(|case| case["kind"] == "archive_reply")
        {
            let archive = case["archive"].as_u64().unwrap() as u8;
            let group = case["group"].as_u64().unwrap() as u16;
            let urgent = case["urgent"].as_bool().unwrap();
            let key = case["key"].as_i64().unwrap() as u8;
            let packed = hex(&case["input"]);
            let wire = hex(&case["expected"]);
            let length = u32::from_be_bytes(packed[1..5].try_into().unwrap()) as usize;
            let container_length = length + if packed[0] == 0 { 5 } else { 9 };
            // The server pads a compressed group that has no version trailer
            // with two bytes the client does not expect.
            let stray = archive != 255 && packed[0] != 0 && packed.len() - length == 9;
            for chunk in [1usize, 7, 512, usize::MAX] {
                let mut queue =
                    Js5NetQueue::with_random(Box::new(move || (f64::from(key) - 0.5) / 255.0));
                if key != 0 {
                    queue.rekey();
                    queue.errors = 0;
                    assert_eq!(queue.encryption_key(), key);
                }
                let socket = ScriptedSocket::new();
                socket.wire().chunk = Some(chunk);
                queue.start(true, Box::new(socket.clone()), 0);
                let request = queue.read(archive, 2, group, urgent).unwrap();
                let follower = queue.read(archive, 2, group ^ 1, urgent).unwrap();
                queue.tick(0);
                socket.push(&wire);
                // A second reply shows whether the stream is still aligned.
                socket.push(&reply(archive, group ^ 1, urgent, key, &[0, 0, 0, 0, 1, 9]));
                let mut alive = true;
                for now in 1..40_000 {
                    alive = queue.tick(now);
                    if !alive || !follower.incomplete() {
                        break;
                    }
                }
                let data = request.data().expect("first reply completes");
                assert_eq!(data[..container_length], packed[..container_length]);
                assert_eq!(data[container_length..], [0, 0]);
                if stray {
                    // Two stray zero bytes shift the next header: no match.
                    assert!(!alive && follower.incomplete());
                    assert_eq!((queue.errors, queue.response), (1, -2));
                    padded += 1;
                } else {
                    assert_eq!(*follower.data().unwrap(), [0, 0, 0, 0, 1, 9, 0, 0]);
                    assert_eq!(queue.errors, 0);
                    clean += 1;
                }
            }
        }
        assert_eq!((clean, padded), (4 * 960, 4 * 192));
    }

    /// `JS5WriteEvent.getReferenceData` (JVM fixture): the master index reply
    /// is never obfuscated and always claims urgent priority.
    #[test]
    fn the_original_servers_master_reply_is_readable_only_without_a_key() {
        let mut cases = 0;
        for case in fixture()
            .iter()
            .filter(|case| case["kind"] == "master_reply")
        {
            let payload = hex(&case["input"]);
            let wire = hex(&case["expected"]);
            let (mut queue, socket) = queue_with_socket();
            let master = queue.read(255, 0, 255, true).unwrap();
            queue.tick(0);
            socket.push(&wire);
            let mut alive = true;
            for now in 1..2000 {
                alive &= queue.tick(now);
                if !master.incomplete() {
                    break;
                }
            }
            // The server counts its block from 10, not 8, so its first marker
            // comes two bytes early: only a master index of at most 502
            // bytes (62 archives) survives the trip.
            if payload.len() <= 502 {
                let data = master.data().expect("master index");
                assert_eq!(data[0], 0);
                assert_eq!(data[1..5], (payload.len() as u32).to_be_bytes());
                assert_eq!(data[5..], payload);
                assert!(alive);
            } else {
                assert!(master.data().is_none_or(|data| data[5..] != payload));
            }
            // With a key set, the unobfuscated reply matches no request.
            let mut keyed = Js5NetQueue::with_random(Box::new(|| 0.5));
            keyed.rekey();
            keyed.errors = 0;
            let socket = ScriptedSocket::new();
            keyed.start(true, Box::new(socket.clone()), 0);
            let master = keyed.read(255, 0, 255, true).unwrap();
            keyed.tick(0);
            socket.push(&wire);
            assert!(!keyed.tick(1));
            assert!(master.incomplete());
            assert_eq!((keyed.errors, keyed.response), (1, -2));
            cases += 1;
        }
        assert_eq!(cases, 28);
        let bounds = fixture()
            .into_iter()
            .find(|case| case["kind"] == "client_queue_bounds")
            .expect("queue bounds case");
        assert_eq!(bounds["urgent"], QUEUE_LIMIT);
        assert_eq!(bounds["prefetch"], QUEUE_LIMIT);
    }

    fn dialer() -> (Js5Connection, ScriptedOpener) {
        let opener = ScriptedOpener::default();
        let mut connection = Js5Connection::new(Box::new(opener.clone()), 43594, 443);
        connection.queue = Js5NetQueue::with_random(Box::new(|| 0.5));
        (connection, opener)
    }
    fn offer(opener: &ScriptedOpener, next: Next) {
        opener.0.lock().unwrap().next.push_back(next);
    }
    fn accepted() -> ScriptedSocket {
        let socket = ScriptedSocket::new();
        socket.wire().handshake.push_back(0);
        socket
    }

    #[test]
    fn the_handshake_is_opcode_15_and_the_build_and_zero_accepts_it() {
        let (mut connection, opener) = dialer();
        // Idle: no requests, so no connection is opened.
        connection.network_loop(0);
        assert!(opener.0.lock().unwrap().ports.is_empty());
        let request = connection.queue.read(255, 0, 255, true).unwrap();
        let socket = accepted();
        offer(&opener, Next::Socket(socket.clone()));
        connection.network_loop(20);
        // While loading (state 0) the client reports itself as logged in.
        assert_eq!(
            socket.take_written(),
            [15, 0, 0, 2, 18, 6, 0, 0, 3, 2, 0, 0, 0]
        );
        assert!(connection.queue.connected());
        connection.network_loop(40);
        assert_eq!(socket.take_written(), [1, 255, 0, 255]);
        assert!(request.incomplete());
        assert_eq!(opener.0.lock().unwrap().ports, [43594]);
        // Entering and leaving the title screen is reported once each way.
        connection.set_game_state(5);
        connection.set_game_state(10);
        connection.set_game_state(30);
        assert_eq!(socket.take_written(), [3, 0, 0, 0, 2, 0, 0, 0]);
    }

    #[test]
    fn a_connection_made_on_the_title_screen_says_logged_out() {
        for (state, notice) in [(5, 3), (10, 3), (28, 3), (0, 2), (25, 2), (30, 2), (40, 2)] {
            let (mut connection, opener) = dialer();
            connection.game_state = state;
            connection.queue.read(255, 0, 255, true).unwrap();
            let socket = accepted();
            // Outside the loading states the reply is only read once a byte
            // has arrived.
            socket.push(&[0]);
            socket.wire().handshake.clear();
            offer(&opener, Next::Socket(socket.clone()));
            connection.network_loop(0);
            assert_eq!(socket.take_written()[5..], [6, 0, 0, 3, notice, 0, 0, 0]);
        }
    }

    #[test]
    fn failures_back_off_by_250_cycles_each_and_alternate_the_port() {
        let (mut connection, opener) = dialer();
        connection.game_state = 30; // in game: no fatal error for I/O trouble
        connection.queue.read(255, 0, 255, true).unwrap();
        let mut attempts_at = Vec::new();
        let mut cycle = 0u64;
        while attempts_at.len() < 16 {
            let before = opener.0.lock().unwrap().ports.len();
            connection.network_loop(cycle * 20);
            if opener.0.lock().unwrap().ports.len() > before {
                attempts_at.push(cycle);
            }
            cycle += 1;
        }
        // The first failure is only noticed on the next cycle; from then on
        // each attempt follows the previous by its delay plus that cycle.
        let gaps: Vec<u64> = attempts_at.windows(2).map(|w| w[1] - w[0]).collect();
        assert_eq!(
            gaps,
            [
                1, 251, 501, 751, 1001, 1251, 1501, 1751, 2001, 2251, 2501, 2751, 3001, 3001, 3001
            ]
        );
        assert_eq!(connection.queue.response, 1000);
        assert_eq!(connection.fatal, None);
        let ports = opener.0.lock().unwrap().ports.clone();
        assert_eq!(ports[..5], [43594, 443, 43594, 443, 43594]);
    }

    #[test]
    fn the_fatal_rules() {
        // Out of date: two failures with handshake code 6, in any state.
        let (mut connection, opener) = dialer();
        connection.game_state = 30;
        connection.queue.read(255, 0, 255, true).unwrap();
        for _ in 0..2 {
            let socket = ScriptedSocket::new();
            socket.push(&[6]);
            offer(&opener, Next::Socket(socket));
        }
        for cycle in 0..400 {
            connection.network_loop(cycle * 20);
        }
        assert_eq!(connection.fatal, Some(FatalError::OutOfDate));
        assert_eq!(connection.game_state, 1000);
        assert_eq!(connection.queue.errors, 2);
        assert_eq!(FatalError::OutOfDate.message(), "js5connect_outofdate");

        // While loading, the fourth failure stops the client; which message
        // depends on the last response.
        for (code, expected) in [
            (7, FatalError::Full),
            (9, FatalError::Full),
            (5, FatalError::Connect),
            (-1, FatalError::Crc), // the server closed before answering
        ] {
            for state in [0, 5] {
                let (mut connection, opener) = dialer();
                connection.game_state = state;
                connection.queue.read(255, 0, 255, true).unwrap();
                for _ in 0..4 {
                    let socket = ScriptedSocket::new();
                    socket.wire().handshake.push_back(code);
                    offer(&opener, Next::Socket(socket));
                }
                for cycle in 0..2000 {
                    connection.network_loop(cycle * 20);
                }
                assert_eq!(connection.fatal, Some(expected), "code {code}");
                assert_eq!(connection.queue.errors, 4);
                assert_eq!(opener.0.lock().unwrap().ports.len(), 4);
            }
        }
        // No server at all: response 1000 is positive, so "js5connect".
        let (mut connection, _opener) = dialer();
        connection.queue.read(255, 0, 255, true).unwrap();
        for cycle in 0..2000 {
            connection.network_loop(cycle * 20);
        }
        assert_eq!(connection.fatal, Some(FatalError::Connect));
        // Four stream errors while loading: "js5io".
        let (mut connection, opener) = dialer();
        connection.queue.read(3, 2, 1, true).unwrap();
        for _ in 0..4 {
            let socket = accepted();
            socket.push(&[9, 9, 9, 9, 9, 9, 9, 9]);
            offer(&opener, Next::Socket(socket));
        }
        for cycle in 0..2000 {
            connection.network_loop(cycle * 20);
        }
        assert_eq!(connection.fatal, Some(FatalError::Io));
        assert_eq!(connection.queue.response, -2);
        // Four checksum failures stop the client in any state.
        let (mut connection, opener) = dialer();
        connection.game_state = 30;
        connection.queue.read(3, 2, 1, true).unwrap();
        for _ in 0..4 {
            let socket = accepted();
            socket.push(&[0]);
            socket.wire().handshake.clear();
            offer(&opener, Next::Socket(socket));
        }
        for cycle in 0..4000u64 {
            connection.network_loop(cycle * 20);
            if connection.queue.connected() && connection.queue.errors < 4 {
                connection.queue.rekey();
            }
        }
        assert_eq!(connection.fatal, Some(FatalError::Crc));
        assert_eq!(FatalError::Crc.message(), "js5crc");
    }

    #[test]
    fn handshake_trouble_sets_the_original_response_codes() {
        // Outside the loading states an unanswered handshake times out after
        // 30 seconds with 1001.
        let (mut connection, opener) = dialer();
        connection.game_state = 30;
        connection.queue.read(255, 0, 255, true).unwrap();
        let silent = ScriptedSocket::new();
        offer(&opener, Next::Socket(silent.clone()));
        connection.network_loop(0);
        assert_eq!(silent.take_written(), [15, 0, 0, 2, 18]);
        connection.network_loop(30_000);
        assert_eq!(connection.queue.errors, 0);
        connection.network_loop(30_001);
        assert_eq!(
            (connection.queue.errors, connection.queue.response),
            (1, 1001)
        );
        // A socket that is still opening is polled again next cycle.
        let (mut connection, opener) = dialer();
        connection.queue.read(255, 0, 255, true).unwrap();
        offer(&opener, Next::Pending);
        for cycle in 0..5 {
            connection.network_loop(cycle * 20);
        }
        assert_eq!(opener.0.lock().unwrap().ports.len(), 1);
        assert_eq!(connection.queue.errors, 0);
        // An I/O error anywhere in the step is 1002.
        let (mut connection, opener) = dialer();
        connection.queue.read(255, 0, 255, true).unwrap();
        let broken = ScriptedSocket::new();
        broken.wire().fail_writes = true;
        offer(&opener, Next::Socket(broken));
        connection.network_loop(0);
        assert_eq!(
            (connection.queue.errors, connection.queue.response),
            (1, 1002)
        );
        // While loading, a reply that never comes is the socket's own
        // timeout: also an I/O error.
        let (mut connection, opener) = dialer();
        connection.queue.read(255, 0, 255, true).unwrap();
        offer(&opener, Next::Socket(ScriptedSocket::new()));
        connection.network_loop(0);
        assert_eq!(
            (connection.queue.errors, connection.queue.response),
            (1, 1002)
        );
    }
}
