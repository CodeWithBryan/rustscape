//! The update connection over a transport that hands over whole binary
//! messages and can never wait: a browser `WebSocket`.
//!
//! The original applet opens a plain socket to the game server; a page cannot,
//! so the same byte stream travels in WebSocket binary messages (the Rust
//! server accepts the JS5 protocol on its WebSocket listener exactly as on
//! TCP). Message boundaries carry no meaning: [`StreamSocket`] joins whatever
//! arrives into the stream [`crate::js5_client::Js5NetQueue`] reads, and each
//! write goes out as one message.
//!
//! Where this differs from `rt4.BufferedSocket`:
//! * Nothing blocks. [`Js5Socket::can_wait`] is `false`, so the handshake
//!   reply is read when it has arrived instead of being waited for.
//! * A transport that reports failure makes the next `available` an I/O
//!   error at once; a TCP socket would report it on a later read.
use crate::js5_client::{Js5Socket, OpenStatus, SocketOpener, SocketRequest};
use std::{collections::VecDeque, io};

/// Received bytes held here until the queue asks for them, as for TCP.
const READ_AHEAD: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PortState {
    Connecting,
    Open,
    /// The peer closed the connection.
    Closed,
    /// The connection could not be made or broke.
    Failed,
}

/// What the page's glue offers for one connection.
pub trait MessagePort: Send {
    fn state(&mut self) -> PortState;
    /// Moves bytes that have arrived into `destination` and returns how many;
    /// zero when none are waiting. Messages may be split and joined anywhere.
    fn receive(&mut self, destination: &mut [u8]) -> usize;
    /// Sends one binary message. `false` when the transport refuses it
    /// (closed, or too much already waiting to go out).
    fn send(&mut self, bytes: &[u8]) -> bool;
    fn close(&mut self);
}

pub struct StreamSocket<P: MessagePort> {
    port: P,
    incoming: VecDeque<u8>,
    write_error: bool,
    closed: bool,
}
impl<P: MessagePort> StreamSocket<P> {
    pub fn new(port: P) -> Self {
        Self {
            port,
            incoming: VecDeque::with_capacity(READ_AHEAD),
            write_error: false,
            closed: false,
        }
    }
    fn fill(&mut self) {
        let mut buffer = [0u8; 16 * 1024];
        while self.incoming.len() < READ_AHEAD {
            let room = (READ_AHEAD - self.incoming.len()).min(buffer.len());
            let count = self.port.receive(&mut buffer[..room]).min(room);
            if count == 0 {
                break;
            }
            self.incoming.extend(&buffer[..count]);
        }
    }
}
impl<P: MessagePort> Js5Socket for StreamSocket<P> {
    fn check_error(&mut self) -> io::Result<()> {
        if !self.closed && std::mem::take(&mut self.write_error) {
            return Err(io::Error::other("update server write failed"));
        }
        Ok(())
    }
    fn write(&mut self, bytes: &[u8]) -> io::Result<()> {
        if self.closed {
            return Ok(());
        }
        if std::mem::take(&mut self.write_error) || !self.port.send(bytes) {
            return Err(io::Error::other("update server write failed"));
        }
        Ok(())
    }
    fn available(&mut self) -> io::Result<usize> {
        if self.closed {
            return Ok(0);
        }
        self.fill();
        if self.incoming.is_empty() && self.port.state() == PortState::Failed {
            return Err(io::Error::other("update server connection failed"));
        }
        // After the peer has closed this stays zero, like
        // `InputStream.available`; the 30 second rule notices.
        Ok(self.incoming.len())
    }
    fn read(&mut self, destination: &mut [u8]) -> io::Result<()> {
        if self.closed {
            return Ok(());
        }
        if self.incoming.len() < destination.len() {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        let count = destination.len();
        for (byte, value) in destination.iter_mut().zip(self.incoming.drain(..count)) {
            *byte = value;
        }
        Ok(())
    }
    fn read_blocking(&mut self) -> io::Result<i32> {
        if self.closed {
            return Ok(0);
        }
        self.fill();
        if let Some(byte) = self.incoming.pop_front() {
            return Ok(i32::from(byte));
        }
        match self.port.state() {
            PortState::Closed | PortState::Failed => Ok(-1),
            _ => Err(io::ErrorKind::WouldBlock.into()),
        }
    }
    fn close(&mut self) {
        if !self.closed {
            self.closed = true;
            self.incoming.clear();
            self.port.close();
        }
    }
    fn can_wait(&self) -> bool {
        false
    }
}

struct Opening<P: MessagePort>(Option<P>);
impl<P: MessagePort + 'static> SocketRequest for Opening<P> {
    fn poll(&mut self) -> OpenStatus {
        let Some(port) = self.0.as_mut() else {
            return OpenStatus::Failed;
        };
        match port.state() {
            PortState::Connecting => OpenStatus::Pending,
            PortState::Open => match self.0.take() {
                Some(port) => OpenStatus::Ready(Box::new(StreamSocket::new(port))),
                None => OpenStatus::Failed,
            },
            PortState::Closed | PortState::Failed => {
                if let Some(mut port) = self.0.take() {
                    port.close();
                }
                OpenStatus::Failed
            }
        }
    }
}
impl<P: MessagePort> Drop for Opening<P> {
    fn drop(&mut self) {
        // An attempt abandoned while still connecting must not linger.
        if let Some(port) = self.0.as_mut() {
            port.close();
        }
    }
}

/// Opens message connections; `open` is given the port the connect logic
/// chose and returns `None` when no connection can even be started.
pub struct StreamOpener<P, F> {
    open: F,
    port: std::marker::PhantomData<fn() -> P>,
}
impl<P, F> StreamOpener<P, F>
where
    P: MessagePort + 'static,
    F: FnMut(u16) -> Option<P> + Send,
{
    pub fn new(open: F) -> Self {
        Self {
            open,
            port: std::marker::PhantomData,
        }
    }
}
impl<P, F> SocketOpener for StreamOpener<P, F>
where
    P: MessagePort + 'static,
    F: FnMut(u16) -> Option<P> + Send,
{
    fn open(&mut self, port: u16) -> Box<dyn SocketRequest> {
        Box::new(Opening((self.open)(port)))
    }
}

/// A [`MessagePort`] joined to the scripted wire of
/// [`crate::js5_client::scripted`], for tests here and in the client: what
/// the client sends lands on the wire as written bytes, and what the scripted
/// server pushes comes back in messages of a chosen size.
pub mod scripted {
    use super::{MessagePort, PortState};
    use crate::js5_client::scripted::ScriptedSocket;
    use std::sync::{Arc, Mutex};

    pub struct PipeState {
        pub state: PortState,
        /// Bytes handed over per `receive` call: the message size.
        pub message: usize,
        /// Every message the client sent, in order.
        pub sent: Vec<Vec<u8>>,
        pub refuse_sends: bool,
        pub closed: bool,
        /// Answer the handshake (opcode 15 and the build) with this byte, as
        /// the server does on the same connection.
        pub handshake_reply: Option<u8>,
    }
    #[derive(Clone)]
    pub struct PipePort {
        pub wire: ScriptedSocket,
        pub pipe: Arc<Mutex<PipeState>>,
    }
    impl PipePort {
        pub fn new(wire: ScriptedSocket, message: usize) -> Self {
            Self {
                wire,
                pipe: Arc::new(Mutex::new(PipeState {
                    state: PortState::Open,
                    message: message.max(1),
                    sent: Vec::new(),
                    refuse_sends: false,
                    closed: false,
                    handshake_reply: Some(0),
                })),
            }
        }
        pub fn pipe(&self) -> std::sync::MutexGuard<'_, PipeState> {
            self.pipe.lock().expect("pipe state")
        }
    }
    impl MessagePort for PipePort {
        fn state(&mut self) -> PortState {
            self.pipe().state
        }
        fn receive(&mut self, destination: &mut [u8]) -> usize {
            let message = self.pipe().message;
            let mut wire = self.wire.wire();
            let count = destination.len().min(message).min(wire.incoming.len());
            for byte in &mut destination[..count] {
                *byte = wire.incoming.pop_front().expect("length checked");
            }
            count
        }
        fn send(&mut self, bytes: &[u8]) -> bool {
            let mut pipe = self.pipe();
            if pipe.refuse_sends || pipe.state != PortState::Open {
                return false;
            }
            pipe.sent.push(bytes.to_vec());
            let reply = (bytes.len() == 5 && bytes[0] == 15)
                .then_some(pipe.handshake_reply)
                .flatten();
            drop(pipe);
            let mut wire = self.wire.wire();
            wire.written.extend_from_slice(bytes);
            if let Some(reply) = reply {
                wire.incoming.push_back(reply);
            }
            true
        }
        fn close(&mut self) {
            self.pipe().closed = true;
            self.wire.wire().closed = true;
        }
    }
}

#[cfg(test)]
mod tests {
    //! The adapter has no counterpart in the original; these establish that
    //! the queue and the connect logic behave over it as they do over the
    //! scripted socket, whatever the message sizes.
    use super::{scripted::PipePort, *};
    use crate::js5_client::{
        Js5Connection, Js5NetQueue,
        scripted::{FakeServer, ScriptedSocket},
    };
    use rs09_protocol::js5::{Request, encode_archive};
    use std::sync::{Arc, Mutex};

    fn piped(message: usize) -> (StreamSocket<PipePort>, PipePort) {
        let port = PipePort::new(ScriptedSocket::new(), message);
        (StreamSocket::new(port.clone()), port)
    }

    #[test]
    fn messages_of_any_size_are_one_byte_stream() {
        let bytes: Vec<u8> = (0..5000u32).map(|i| (i * 13) as u8).collect();
        for message in [1, 2, 7, 512, 4999, 100_000] {
            let (mut socket, port) = piped(message);
            port.wire.push(&bytes);
            let mut read = Vec::new();
            // Uneven read sizes on the consuming side as well.
            let mut want = 1;
            loop {
                let available = socket.available().unwrap();
                if available == 0 {
                    break;
                }
                let count = want.min(available);
                let mut piece = vec![0; count];
                socket.read(&mut piece).unwrap();
                read.extend(piece);
                want = want % 37 + 3;
            }
            assert_eq!(read, bytes, "message size {message}");
        }
    }

    #[test]
    fn it_never_waits_and_reports_the_end_like_a_socket() {
        let (mut socket, port) = piped(8);
        assert!(!socket.can_wait());
        assert_eq!(socket.available().unwrap(), 0);
        // Nothing has arrived: the Java would wait here; this must not.
        assert_eq!(
            socket.read_blocking().unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
        port.wire.push(&[7, 9]);
        assert_eq!(socket.read_blocking().unwrap(), 7);
        assert_eq!(socket.available().unwrap(), 1);
        let mut more = [0; 2];
        assert_eq!(
            socket.read(&mut more).unwrap_err().kind(),
            io::ErrorKind::UnexpectedEof
        );
        // The peer closes: what arrived can still be read, then nothing is
        // ever available and a single read reports the end.
        port.pipe().state = PortState::Closed;
        assert_eq!(socket.available().unwrap(), 1);
        assert_eq!(socket.read_blocking().unwrap(), 9);
        assert_eq!(socket.available().unwrap(), 0);
        assert_eq!(socket.read_blocking().unwrap(), -1);
        // Writing to it fails, as a write to a closed socket does.
        assert!(socket.write(&[1, 2, 3, 4]).is_err());
        // A broken transport is an error as soon as it is looked at.
        let (mut socket, port) = piped(8);
        port.wire.push(&[1]);
        port.pipe().state = PortState::Failed;
        assert_eq!(socket.available().unwrap(), 1);
        socket.read(&mut [0]).unwrap();
        assert!(socket.available().is_err());
    }

    #[test]
    fn each_write_is_one_message_and_a_refused_one_is_an_error() {
        let (mut socket, port) = piped(64);
        socket.write(&[1, 2, 0, 9]).unwrap();
        socket.write(&[0, 2, 0, 10]).unwrap();
        assert_eq!(port.pipe().sent, [vec![1, 2, 0, 9], vec![0, 2, 0, 10]]);
        assert_eq!(port.wire.take_written(), [1, 2, 0, 9, 0, 2, 0, 10]);
        socket.check_error().unwrap();
        port.pipe().refuse_sends = true;
        assert!(socket.write(&[3, 0, 0, 0]).is_err());
        // After close nothing is sent and nothing is reported, as for TCP.
        socket.close();
        assert!(port.pipe().closed);
        socket.write(&[7, 0, 0, 0]).unwrap();
        socket.check_error().unwrap();
        assert_eq!(socket.available().unwrap(), 0);
        assert_eq!(port.pipe().sent.len(), 2);
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

    /// The original server's replies (the JVM fixture the TCP path is tested
    /// with) reassemble to the stored container over the adapter too, with
    /// the stream cut into messages of four sizes.
    #[test]
    fn the_original_servers_replies_reassemble_over_messages() {
        let mut cases = 0;
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
            // Groups the server pads (see the report on its quirks) leave two
            // stray bytes behind; the reply itself still arrives whole.
            let stray = archive != 255 && packed[0] != 0 && packed.len() - length == 9;
            for message in [1usize, 5, 513, usize::MAX] {
                let mut queue =
                    Js5NetQueue::with_random(Box::new(move || (f64::from(key) - 0.5) / 255.0));
                if key != 0 {
                    queue.rekey();
                    queue.errors = 0;
                    assert_eq!(queue.encryption_key(), key);
                }
                let (socket, port) = piped(message);
                queue.start(true, Box::new(socket), 0);
                let request = queue.read(archive, 2, group, urgent).unwrap();
                let follower = queue.read(archive, 2, group ^ 1, urgent).unwrap();
                queue.tick(0);
                port.wire.push(&wire);
                let next = Request {
                    archive,
                    group: group ^ 1,
                    urgent,
                };
                port.wire
                    .push(&encode_archive(next, key, &[0, 0, 0, 0, 1, 9], 1 << 24).unwrap());
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
                    assert!(!alive && follower.incomplete());
                } else {
                    assert_eq!(*follower.data().unwrap(), [0, 0, 0, 0, 1, 9, 0, 0]);
                    assert_eq!(queue.errors, 0);
                }
                cases += 1;
            }
        }
        assert_eq!(cases, 4 * 1152);
    }

    /// The pipes the opener made and the ports it was asked for.
    type Opened = (Arc<Mutex<Vec<PipePort>>>, Arc<Mutex<Vec<u16>>>);
    fn dial(message: usize) -> (Js5Connection, Opened) {
        let opened = Arc::new(Mutex::new(Vec::<PipePort>::new()));
        let ports = Arc::new(Mutex::new(Vec::<u16>::new()));
        let (made, asked) = (Arc::clone(&opened), Arc::clone(&ports));
        let opener = StreamOpener::new(move |port: u16| {
            asked.lock().unwrap().push(port);
            let pipe = PipePort::new(ScriptedSocket::new(), message);
            made.lock().unwrap().push(pipe.clone());
            Some(pipe)
        });
        let mut connection = Js5Connection::new(Box::new(opener), 43594, 43594);
        connection.queue = Js5NetQueue::with_random(Box::new(|| 0.5));
        (connection, (opened, ports))
    }

    /// `client.js5Connect` over a socket that cannot wait: the connection
    /// opens when the transport does, the handshake goes out once, and the
    /// queue starts when the reply byte has arrived.
    #[test]
    fn the_handshake_completes_without_waiting() {
        let (mut connection, (opened, ports)) = dial(3);
        let master = connection.queue.read(255, 0, 255, true).unwrap();
        connection.network_loop(0);
        let pipe = opened.lock().unwrap()[0].clone();
        // This pipe is open at once and answers the handshake as it is
        // sent, so a single loop gets all the way through.
        assert!(connection.queue.connected());
        assert_eq!(*ports.lock().unwrap(), [43594]);
        assert_eq!(
            pipe.pipe().sent[..3],
            [vec![15, 0, 0, 2, 18], vec![6, 0, 0, 3], vec![2, 0, 0, 0]]
        );
        connection.network_loop(20);
        assert_eq!(pipe.pipe().sent[3], [1, 255, 0, 255]);
        assert!(master.incomplete());

        // Still connecting, then open with the reply held back.
        let slow = Arc::new(Mutex::new(Vec::<PipePort>::new()));
        let made = Arc::clone(&slow);
        let opener = StreamOpener::new(move |_| {
            let pipe = PipePort::new(ScriptedSocket::new(), 3);
            pipe.pipe().state = PortState::Connecting;
            pipe.pipe().handshake_reply = None;
            made.lock().unwrap().push(pipe.clone());
            Some(pipe)
        });
        let mut connection = Js5Connection::new(Box::new(opener), 43594, 43594);
        connection.queue.read(255, 0, 255, true).unwrap();
        for now in [0, 20, 40] {
            connection.network_loop(now);
        }
        let pipe = slow.lock().unwrap()[0].clone();
        assert!(pipe.pipe().sent.is_empty() && !connection.queue.connected());
        pipe.pipe().state = PortState::Open;
        for now in [60, 80, 100] {
            connection.network_loop(now);
        }
        // Loading (game state 0): the original would block in read(); here
        // the handshake has gone out once and the loop keeps running.
        assert_eq!(pipe.pipe().sent, [vec![15, 0, 0, 2, 18]]);
        assert!(!connection.queue.connected());
        assert_eq!(connection.queue.errors, 0);
        pipe.wire.push(&[0]);
        connection.network_loop(120);
        assert!(connection.queue.connected());
        assert_eq!(pipe.pipe().sent.len(), 3);
    }

    #[test]
    fn refusal_failure_and_silence_set_the_original_response_codes() {
        // The server answers the handshake with 6: out of date.
        let opener = StreamOpener::new(move |_| {
            let pipe = PipePort::new(ScriptedSocket::new(), 1);
            pipe.pipe().handshake_reply = Some(6);
            Some(pipe)
        });
        let mut connection = Js5Connection::new(Box::new(opener), 43594, 43594);
        connection.queue.read(255, 0, 255, true).unwrap();
        connection.network_loop(0);
        assert_eq!((connection.queue.errors, connection.queue.response), (1, 6));
        // A transport that fails to open is response 1000.
        let opener = StreamOpener::new(move |_| {
            let pipe = PipePort::new(ScriptedSocket::new(), 1);
            pipe.pipe().state = PortState::Failed;
            Some(pipe)
        });
        let mut connection = Js5Connection::new(Box::new(opener), 43594, 43594);
        connection.queue.read(255, 0, 255, true).unwrap();
        connection.network_loop(0);
        assert_eq!(
            (connection.queue.errors, connection.queue.response),
            (1, 1000)
        );
        // No transport at all (a bad address) is the same failure.
        let opener = StreamOpener::new(|_| None::<PipePort>);
        let mut connection = Js5Connection::new(Box::new(opener), 43594, 43594);
        connection.queue.read(255, 0, 255, true).unwrap();
        connection.network_loop(0);
        assert_eq!(
            (connection.queue.errors, connection.queue.response),
            (1, 1000)
        );
        // Open, but the handshake is never answered: 1001 after 30 seconds.
        let opener = StreamOpener::new(|_| {
            let pipe = PipePort::new(ScriptedSocket::new(), 1);
            pipe.pipe().handshake_reply = None;
            Some(pipe)
        });
        let mut connection = Js5Connection::new(Box::new(opener), 43594, 43594);
        connection.queue.read(255, 0, 255, true).unwrap();
        connection.network_loop(0);
        connection.network_loop(30_000);
        assert_eq!(connection.queue.errors, 0);
        connection.network_loop(30_001);
        assert_eq!(
            (connection.queue.errors, connection.queue.response),
            (1, 1001)
        );
    }

    /// The scripted update server, reached through the adapter: the master
    /// index and a group arrive whole however the stream is cut.
    #[test]
    fn the_scripted_server_is_served_through_the_adapter() {
        for message in [1usize, 4, 600, usize::MAX] {
            let mut server = FakeServer::new(3);
            let payload: Vec<u8> = (0..3000u32).map(|i| (i * 7 + 1) as u8).collect();
            server.add_archive(2, 77, &[(0, payload.clone(), 5), (4, vec![9; 40], 6)]);
            let (mut connection, (opened, _)) = dial(message);
            let master = connection.queue.read(255, 0, 255, true).unwrap();
            let index = connection.queue.read(255, 0, 2, true).unwrap();
            let group = connection.queue.read(2, 2, 0, false).unwrap();
            let mut now = 0;
            while group.incomplete() || index.incomplete() || master.incomplete() {
                now += 20;
                connection.network_loop(now);
                if let Some(pipe) = opened.lock().unwrap().first() {
                    server.serve(&pipe.wire);
                }
                assert!(now < 400_000, "message size {message}");
            }
            assert_eq!(*index.data().unwrap(), server.index_container(2));
            let mut expected = server.group_container(2, 0);
            expected.extend([0, 0]);
            assert_eq!(*group.data().unwrap(), expected);
            assert_eq!(master.data().unwrap().len(), 5 + 3 * 8);
            assert_eq!(connection.queue.errors, 0);
            assert_eq!(server.controls, [6, 2]);
        }
    }
}
