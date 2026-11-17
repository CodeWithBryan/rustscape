//! The session lifecycle over real sockets: this client's own connection
//! code (`LiveDial` -> `live::Connection` -> `Session` -> `Transport`) against
//! a scripted peer on a loopback port.
//!
//! The peer is not the game server. It speaks just enough of the login
//! exchange to exercise the client: it reads the login block with the shared
//! server-side decoder, answers 2 (with player details and a rebuild packet)
//! or 15, sends packets, and drops the connection on cue. What this
//! establishes is the client's behaviour on a dropped socket, a reconnect
//! login and a LOGOUT packet; it says nothing about what a server answers.
#![cfg(not(target_arch = "wasm32"))]
use rs09_client::lifecycle::{
    Credentials, Effect, GameState, Lifecycle, LiveDial, LoginLoop, LogoutReason,
};
use rs09_protocol::login::{self, RsaKey};
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

/// What the peer does with one accepted connection.
enum Step {
    /// Answer the login with reply 2, eleven detail bytes and REBUILD_NORMAL.
    Accept,
    /// Answer the login with reply 15.
    Resume,
    /// Answer the login with this reply code and close.
    Refuse(u8),
    /// Write raw bytes.
    Send(Vec<u8>),
    /// Close the socket.
    Drop,
}

/// Reads the handshake and the login block; returns the block's type byte
/// (16 login, 18 reconnect) and the tracking id it carries.
fn read_login(stream: &mut TcpStream) -> (i32, i32) {
    let mut handshake = [0; 2];
    stream.read_exact(&mut handshake).unwrap();
    assert_eq!(handshake[0], 14);
    let mut reply = vec![0];
    reply.extend_from_slice(&0x0102_0304_0506_0708i64.to_be_bytes());
    stream.write_all(&reply).unwrap();
    let mut header = [0; 3];
    stream.read_exact(&mut header).unwrap();
    let mut block = header.to_vec();
    block.resize(
        3 + usize::from(u16::from_be_bytes([header[1], header[2]])),
        0,
    );
    stream.read_exact(&mut block[3..]).unwrap();
    let outcome = login::decode(&block, &RsaKey::reference_private(), |_| false);
    let info = outcome.info.expect("a well-formed login block");
    assert_eq!(info.username, "lifecycle");
    (info.opcode, info.packet_count)
}

/// One thread per run: for each scripted connection, accept, read the login
/// and carry out the steps as the test releases them.
fn peer(
    listener: TcpListener,
    connections: Vec<Vec<Step>>,
) -> (mpsc::Sender<()>, mpsc::Receiver<(i32, i32)>) {
    let (go, wait) = mpsc::channel::<()>();
    let (seen, logins) = mpsc::channel();
    thread::spawn(move || {
        for steps in connections {
            let (mut stream, _) = listener.accept().unwrap();
            stream.set_nodelay(true).unwrap();
            let login = read_login(&mut stream);
            seen.send(login).unwrap();
            for step in steps {
                match step {
                    Step::Accept => {
                        // staff 2, blackmarks 0, underage, chat consent, ad
                        // consent, quick chat, mouse recorder, index 300,
                        // member, members' map.
                        let mut bytes = vec![2, 2, 0, 0, 0, 0, 0, 1, 1, 44, 1, 1];
                        bytes.extend_from_slice(&[162, 0, 3, 9, 9, 9]);
                        stream.write_all(&bytes).unwrap();
                    }
                    Step::Resume => stream.write_all(&[15]).unwrap(),
                    Step::Refuse(code) => {
                        stream.write_all(&[code]).unwrap();
                        break;
                    }
                    Step::Send(bytes) => {
                        if wait.recv().is_err() {
                            return;
                        }
                        stream.write_all(&bytes).unwrap();
                    }
                    Step::Drop => {
                        if wait.recv().is_err() {
                            return;
                        }
                        break;
                    }
                }
            }
        }
        // The listener closes here: later connection attempts are refused.
    });
    (go, logins)
}

struct Client {
    life: Lifecycle,
    login: LoginLoop<LiveDial>,
    connection: Option<rs09_client::live::Connection>,
    previous: Option<rs09_client::live::Connection>,
    /// Every effect and every game packet, in the order the client saw them.
    log: Vec<String>,
}
impl Client {
    fn new(endpoint: String) -> Self {
        let credentials = Credentials::new(endpoint, b"lifecycle".to_vec(), b"secret".to_vec());
        let mut seed = 0;
        let dial = LiveDial::new(
            credentials,
            Box::new(move || {
                seed += 1;
                seed
            }),
        );
        Self {
            life: Lifecycle::new(),
            login: LoginLoop::new(dial),
            connection: None,
            previous: None,
            log: vec![],
        }
    }
    fn effects(&mut self) {
        for effect in self.life.take_effects() {
            match &effect {
                Effect::ResetSession(world) => {
                    self.log.push(format!("reset {}", world.player_index));
                    self.connection = self.login.dial.take_connection();
                    assert!(self.connection.is_some());
                }
                Effect::Reconnect => {
                    self.log.push("reconnect".into());
                    let mut connection = self.login.dial.take_connection().unwrap();
                    connection.world = self.previous.as_ref().and_then(|c| c.world.clone());
                    self.connection = Some(connection);
                }
                Effect::SetAsideConnection => {
                    self.log.push("set aside".into());
                    self.previous = self.connection.take();
                }
                Effect::ProcessLogout(reason) => {
                    self.log.push(format!("logout {reason:?}"));
                    if let Some(mut connection) = self.connection.take() {
                        connection.stop("Logged out");
                    }
                }
                Effect::State { from, to } => {
                    self.log.push(format!("{} -> {}", from.code(), to.code()));
                    if *to != GameState::ConnectionLost {
                        self.previous = None;
                    }
                }
                Effect::WindowStatus => self.log.push("window status".into()),
                Effect::Send(frame) => self.log.push(format!("send {}", frame.opcode)),
                Effect::Report(text) => self.log.push(text.clone()),
            }
        }
    }
    /// One 20 ms client cycle (run faster than real time), as the
    /// application's loop does it.
    fn cycle(&mut self) {
        self.life.cycle(&mut self.login);
        self.effects();
        if let Some(connection) = &mut self.connection {
            let frames = connection.poll().unwrap_or_else(|e| {
                connection.stop(e);
                vec![]
            });
            for frame in frames {
                self.life.packet_read(frame.opcode);
                self.log.push(format!("packet {}", frame.opcode));
                match frame.opcode {
                    162 => {
                        self.life.rebuild_packet(400, 400);
                    }
                    86 => self.life.logout_packet(),
                    _ => {}
                }
                if self.life.stopping() {
                    break;
                }
            }
            let _ = connection.tick();
        }
        if matches!(self.life.state(), GameState::InGame | GameState::Loading)
            && !self.life.stopping()
            && self.connection.as_ref().is_none_or(|c| c.closed())
        {
            self.login.dial.verify_id = 77;
            self.life.connection_lost(0, &mut self.login);
        }
        self.effects();
        if self.life.state() == GameState::Loading {
            self.life.rebuild_finished();
            self.effects();
        }
    }
    fn run_until(&mut self, what: &str, mut done: impl FnMut(&Self) -> bool) {
        let started = Instant::now();
        while !done(self) {
            assert!(
                started.elapsed() < Duration::from_secs(30),
                "timed out waiting for {what}; log: {:?}",
                self.log
            );
            self.cycle();
            thread::sleep(Duration::from_millis(2));
        }
    }
}

#[test]
fn drop_reconnect_logout_and_a_failed_reconnect_over_loopback() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = listener.local_addr().unwrap().to_string();
    let (go, logins) = peer(
        listener,
        vec![
            // 1: log in, then the socket drops.
            vec![Step::Accept, Step::Drop],
            // 2: the reconnect resumes with reply 15; a packet, then drop.
            vec![Step::Resume, Step::Send(vec![153]), Step::Drop],
            // 3: the next reconnect is answered like a fresh login, then the
            // server logs the player out.
            vec![Step::Accept, Step::Send(vec![86])],
            // 4: a later login is accepted, then dropped for good.
            vec![Step::Accept, Step::Drop],
        ],
    );
    let mut client = Client::new(endpoint);

    // Title login: type 16.
    client.life.start_login(&mut client.login);
    client.run_until("the first login", |c| c.life.state() == GameState::InGame);
    assert_eq!(
        logins.recv_timeout(Duration::from_secs(30)).unwrap(),
        (16, 0)
    );
    assert_eq!(
        client.log,
        [
            "reset 300",
            "window status",
            "packet 162",
            "10 -> 25",
            "send 20",
            "25 -> 30",
            "send 110"
        ]
    );
    assert_eq!(client.life.self_id, 300);
    client.log.clear();

    // The socket drops: game state 40 and a type 18 login carrying the
    // tracking id, answered with 15.
    go.send(()).unwrap();
    client.run_until("the reconnect", |c| c.log.iter().any(|l| l == "40 -> 30"));
    client.cycle();
    assert_eq!(
        logins.recv_timeout(Duration::from_secs(30)).unwrap(),
        (18, 77)
    );
    assert_eq!(
        client.log,
        [
            "set aside",
            "30 -> 40",
            "reconnect",
            "40 -> 30",
            "window status"
        ]
    );
    // The resumed connection has the first login's player details and
    // carries game packets.
    assert_eq!(
        client
            .connection
            .as_ref()
            .unwrap()
            .world
            .as_ref()
            .unwrap()
            .player_index,
        300
    );
    client.log.clear();
    go.send(()).unwrap();
    client.run_until("a packet on the resumed connection", |c| {
        c.log.iter().any(|line| line == "packet 153")
    });
    client.log.clear();

    // It drops again; this time the server answers 2: a whole new session.
    go.send(()).unwrap();
    client.run_until("the second reconnect", |c| {
        c.log.iter().any(|l| l == "send 110")
    });
    assert_eq!(
        logins.recv_timeout(Duration::from_secs(30)).unwrap(),
        (18, 77)
    );
    assert_eq!(
        client.log,
        [
            "set aside",
            "30 -> 40",
            "reset 300",
            "window status",
            "packet 162",
            "40 -> 25",
            "send 20",
            "25 -> 30",
            "send 110"
        ]
    );
    client.log.clear();

    // LOGOUT: processLogout and the title screen; nothing logs in again.
    go.send(()).unwrap();
    client.run_until("the logout", |c| c.life.state() == GameState::Title);
    assert_eq!(client.log, ["packet 86", "logout Server", "30 -> 10"]);
    assert!(client.connection.is_none());
    for _ in 0..20 {
        client.cycle();
    }
    assert!(logins.try_recv().is_err());
    client.log.clear();

    // A new title login, then the server goes away entirely: the reconnect
    // fails to connect twice (reply -4) and the client logs out.
    client.life.start_login(&mut client.login);
    client.run_until("the second login", |c| c.life.state() == GameState::InGame);
    assert_eq!(
        logins.recv_timeout(Duration::from_secs(30)).unwrap(),
        (16, 77)
    );
    client.log.clear();
    go.send(()).unwrap();
    client.run_until("the failed reconnect", |c| {
        c.life.state() == GameState::Title
    });
    assert_eq!(
        client.log,
        [
            "set aside".to_owned(),
            "30 -> 40".to_owned(),
            format!("logout {:?}", LogoutReason::ReconnectFailed(-4)),
            "40 -> 10".to_owned()
        ]
    );
    assert_eq!(client.life.reply, -4);
}

#[test]
fn a_refused_login_reports_its_reply_code() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = listener.local_addr().unwrap().to_string();
    let (_go, logins) = peer(listener, vec![vec![Step::Refuse(3)]]);
    let mut client = Client::new(endpoint);
    client.life.start_login(&mut client.login);
    client.run_until("the refusal", |c| !c.life.logging_in());
    assert_eq!(
        logins.recv_timeout(Duration::from_secs(30)).unwrap(),
        (16, 0)
    );
    assert_eq!(
        (client.life.state(), client.life.reply),
        (GameState::Title, 3)
    );
    assert!(client.log.is_empty() && client.connection.is_none());
}
