//! The title screen's login over real sockets: the script commands
//! (`login_flow::Commands`) on a [`LoginFlow`] behind the lifecycle
//! ([`FlowSeam`]), and the hand-over of its socket to the game connection
//! (`login_seam::game_connection`, `resumed_connection`), against a scripted
//! peer on a loopback port.
//!
//! The peer is not the game server. It answers the world-list request with
//! the shared encoder, reads the login block with the shared server-side
//! decoder, answers 2 (with player details and a rebuild packet), 15 or a
//! refusal, sends packets, reads what the client writes and drops the
//! connection on cue. What this establishes is that the pieces fit: the
//! order of the lifecycle's effects, which login block goes out when, and
//! that packets flow both ways over the handed-over socket. The expected
//! effect orders are those of `tests/lifecycle_live.rs`, which were derived
//! from the Java by hand (a transcription test, not a JVM differential).
#![cfg(not(target_arch = "wasm32"))]
use rs09_client::{
    lifecycle::{Effect, GameState, Lifecycle, LogoutReason},
    live::Connection,
    login_flow::{Commands, Endpoints, JsonConfig, LoginFlow, TransportDialer},
    login_seam::{self, FlowSeam},
};
use rs09_protocol::{
    game::Frame,
    lobby,
    login::{self, RsaKey},
};
use std::{
    cell::RefCell,
    io::{Read, Write},
    net::TcpListener,
    rc::Rc,
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

/// What the peer does with one accepted login connection.
enum Step {
    /// Answer the login with reply 2, eleven detail bytes and REBUILD_NORMAL.
    Accept,
    /// Answer the login with reply 15.
    Resume,
    /// Answer the login with this reply code and close.
    Refuse(u8),
    /// Write raw bytes when released.
    Send(Vec<u8>),
    /// Read this many bytes from the client and report them.
    Expect(usize),
    /// Close the socket when released.
    Drop,
}

/// What the peer saw of a connection.
#[derive(Debug, PartialEq)]
enum Seen {
    /// A world-list request with this checksum.
    WorldList(i32),
    /// A login block: its type byte (16 login, 18 reconnect), its tracking
    /// id and the canvas size it reports.
    Login(i32, i32, [i32; 2]),
    /// Bytes the client wrote in the game.
    Bytes(Vec<u8>),
}

/// One thread per run. A connection that opens with 255 is a world-list
/// request and uses up no script; any other must be a login and takes the
/// next list of steps.
fn peer(listener: TcpListener, logins: Vec<Vec<Step>>) -> (mpsc::Sender<()>, mpsc::Receiver<Seen>) {
    let (go, wait) = mpsc::channel::<()>();
    let (seen, reports) = mpsc::channel();
    thread::spawn(move || {
        let mut logins = logins.into_iter();
        loop {
            let (mut stream, _) = listener.accept().unwrap();
            stream.set_nodelay(true).unwrap();
            let mut first = [0];
            stream.read_exact(&mut first).unwrap();
            if first[0] == 255 {
                let mut stamp = [0; 4];
                stream.read_exact(&mut stamp).unwrap();
                let stamp = i32::from_be_bytes(stamp);
                seen.send(Seen::WorldList(stamp)).unwrap();
                let worlds = [
                    lobby::World::default(),
                    lobby::World {
                        id: 2,
                        activity: b"Second".to_vec(),
                        players: 7,
                        ..lobby::World::default()
                    },
                ];
                stream
                    .write_all(&lobby::world_list(&worlds, 1, stamp).unwrap())
                    .unwrap();
                continue;
            }
            assert_eq!(first[0], 14, "a login handshake");
            let mut name_hash = [0];
            stream.read_exact(&mut name_hash).unwrap();
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
            assert_eq!(info.username, "title");
            assert_eq!(info.password, "secret");
            seen.send(Seen::Login(
                info.opcode,
                info.packet_count,
                [info.screen_width, info.screen_height],
            ))
            .unwrap();
            let Some(steps) = logins.next() else {
                return;
            };
            for step in steps {
                match step {
                    Step::Accept => {
                        // staff 2, blackmarks 0, underage, chat consent, ad
                        // consent, quick chat, mouse recorder, index 300,
                        // member, members' map; then REBUILD_NORMAL with a
                        // three-byte payload and one more packet behind it.
                        let mut bytes = vec![2, 2, 0, 0, 0, 0, 0, 1, 1, 44, 1, 1];
                        bytes.extend_from_slice(&[162, 0, 3, 9, 9, 9]);
                        bytes.push(153);
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
                    Step::Expect(count) => {
                        let mut bytes = vec![0; count];
                        stream.read_exact(&mut bytes).unwrap();
                        seen.send(Seen::Bytes(bytes)).unwrap();
                    }
                    Step::Drop => {
                        if wait.recv().is_err() {
                            return;
                        }
                        break;
                    }
                }
            }
            if logins.len() == 0 {
                // The listener closes here: later attempts are refused.
                return;
            }
        }
    });
    (go, reports)
}

struct Client {
    life: Lifecycle,
    flow: Rc<RefCell<LoginFlow<TransportDialer>>>,
    seam: FlowSeam<TransportDialer>,
    connection: Option<Connection>,
    previous: Option<Connection>,
    /// Every effect and every game packet, in the order the client saw them.
    log: Vec<String>,
    started: Instant,
}
impl Client {
    fn new(port: u16) -> Self {
        // A launch endpoint as `config.json` would give it: one address for
        // the world list and for world 1.
        let json = JsonConfig {
            ip_management: "127.0.0.1".into(),
            ip_address: "127.0.0.1".into(),
            world: 1,
            server_port: i32::from(port) - 1,
            wl_port: i32::from(port),
        };
        let dialer = TransportDialer {
            endpoint: Box::new(|host, port| format!("{host}:{port}")),
        };
        let mut seed = 0.0;
        let flow = Rc::new(RefCell::new(LoginFlow::new(
            dialer,
            Endpoints::standalone("unused.invalid", Some(json)),
            Box::new(move || {
                seed += 0.001;
                seed
            }),
        )));
        let mut seam = FlowSeam::new(Rc::clone(&flow));
        seam.env.canvas = [1024, 700];
        seam.env.window_mode = 2;
        Self {
            life: Lifecycle::new(),
            flow,
            seam,
            connection: None,
            previous: None,
            log: vec![],
            started: Instant::now(),
        }
    }
    fn now_ms(&self) -> i64 {
        self.started.elapsed().as_millis() as i64 + 100_000
    }
    fn effects(&mut self) {
        for effect in self.life.take_effects() {
            match &effect {
                Effect::ResetSession(world) => {
                    self.log.push(format!("reset {}", world.player_index));
                    let entered = self.seam.take_entered().expect("the login's first packet");
                    self.connection = Some(
                        login_seam::game_connection(&mut self.flow.borrow_mut(), &entered).unwrap(),
                    );
                }
                Effect::Reconnect => {
                    self.log.push("reconnect".into());
                    let reconnected = self.seam.take_reconnected().expect("reply 15");
                    let mut connection =
                        login_seam::resumed_connection(&mut self.flow.borrow_mut(), &reconnected)
                            .unwrap();
                    connection.world = self.previous.as_ref().and_then(|c| c.world.clone());
                    self.connection = Some(connection);
                }
                Effect::SetAsideConnection => {
                    self.log.push("set aside".into());
                    if let Some(connection) = self.connection.as_mut() {
                        connection.stop("Connection lost");
                    }
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
                    self.seam.mirror(*to);
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
        self.seam.cycle_game_state(&mut self.life);
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
                        assert_eq!(frame.payload, [9, 9, 9]);
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
            self.seam.env.verify_id = 77;
            self.life.connection_lost(0, &mut self.seam);
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
    /// CS2 5600 with what the login form holds.
    fn script_login(&mut self) {
        self.flow
            .borrow_mut()
            .op5600(b"title", b"secret", 0)
            .unwrap();
    }
}

#[test]
fn the_title_screen_logs_in_reconnects_and_returns_over_loopback() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (go, seen) = peer(
        listener,
        vec![
            // 1: the first login is refused: invalid username or password.
            vec![Step::Refuse(3)],
            // 2: log in, read a packet from the client, then drop.
            vec![Step::Accept, Step::Expect(1), Step::Drop],
            // 3: the reconnect resumes with reply 15; a packet, then drop.
            vec![Step::Resume, Step::Send(vec![153]), Step::Drop],
            // 4: the next reconnect is answered like a fresh login, then the
            // server logs the player out.
            vec![Step::Accept, Step::Send(vec![86])],
            // 5: a later login is accepted, then dropped for good.
            vec![Step::Accept, Step::Drop],
        ],
    );
    let next = || seen.recv_timeout(Duration::from_secs(30)).unwrap();
    let mut client = Client::new(port);

    // client.mainLoad state 135: the world list, before the title screen.
    let started = Instant::now();
    loop {
        use rs09_client::login_flow::WorldListStartup;
        let now = client.now_ms();
        match client.flow.borrow_mut().startup_world_list(now) {
            WorldListStartup::Loading => {}
            WorldListStartup::Loaded => break,
            other => panic!("world list: {other:?}"),
        }
        assert!(started.elapsed() < Duration::from_secs(30), "world list");
        thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(next(), Seen::WorldList(0));
    // InterfaceList.resetToLoginScreen: game state 10.
    client.seam.mirror(GameState::Title);
    // The world-select scripts walk the list with 6501 and 6502.
    {
        let mut flow = client.flow.borrow_mut();
        let first = flow.op6501().expect("a first world");
        assert_eq!((first.id, first.players), (1, 0));
        assert_eq!(first.activity, b"2009Scape Classic");
        assert_eq!(first.country_name, b"Anywhere, USA");
        let second = flow.op6502().expect("a second world");
        assert_eq!((second.id, second.players), (2, 7));
        assert_eq!(second.activity, b"Second");
        assert!(flow.op6502().is_none());
        assert_eq!(flow.op6506(2).map(|world| world.players), Some(7));
        assert!(flow.op6506(3).is_none());
        assert_eq!(flow.world_id(), 1, "Player.worldId is the launch's world");
        assert_eq!(flow.op5607(), -2);
    }

    // Nothing typed: reply 3 at once, no connection (LoginManager.startLogin).
    client.flow.borrow_mut().op5600(b"", b"", 0).unwrap();
    client.cycle();
    assert_eq!(client.flow.borrow().op5607(), 3);
    assert!(!client.life.logging_in());
    client.flow.borrow_mut().op5602();
    assert_eq!(client.flow.borrow().op5607(), -2);

    // A refused login leaves its reply for the scripts and the title stays.
    client.script_login();
    assert_eq!(client.flow.borrow().op5607(), -3);
    client.run_until("the refusal", |c| c.flow.borrow().op5607() != -3);
    assert_eq!(next(), Seen::Login(16, 0, [1024, 700]));
    assert_eq!(client.flow.borrow().op5607(), 3);
    assert_eq!(client.life.reply, 3);
    assert_eq!(client.life.state(), GameState::Title);
    assert!(!client.life.logging_in());
    assert!(client.log.is_empty(), "{:?}", client.log);
    // While a reply stands the form's commands still work.
    client.flow.borrow_mut().op5602();

    // Title login: type 16.
    client.script_login();
    client.run_until("the login", |c| c.life.state() == GameState::InGame);
    assert_eq!(next(), Seen::Login(16, 0, [1024, 700]));
    assert_eq!(
        client.log,
        [
            "reset 300",
            "window status",
            "packet 162",
            "packet 153",
            "10 -> 25",
            "send 20",
            "25 -> 30",
            "send 110"
        ]
    );
    assert_eq!(client.life.self_id, 300);
    assert_eq!(client.flow.borrow().game_state, 30);
    // In the game the login commands do nothing.
    client.script_login();
    assert_eq!(client.flow.borrow().op5607(), 2);
    assert!(client.seam.preferences_sent);
    // The handed-over socket carries the client's packets.
    client
        .connection
        .as_mut()
        .unwrap()
        .send(&Frame {
            opcode: 93,
            payload: vec![],
        })
        .unwrap();
    assert_eq!(next(), Seen::Bytes(vec![93]));
    client.log.clear();

    // The socket drops: game state 40 and a type 18 login carrying the
    // tracking id, answered with 15.
    go.send(()).unwrap();
    client.run_until("the reconnect", |c| c.log.iter().any(|l| l == "40 -> 30"));
    client.cycle();
    assert_eq!(next(), Seen::Login(18, 77, [1024, 700]));
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
    assert_eq!(
        client
            .connection
            .as_ref()
            .unwrap()
            .world
            .as_ref()
            .map(|world| world.player_index),
        Some(300),
        "reply 15 keeps the player details of the session it resumes"
    );
    client.log.clear();
    go.send(()).unwrap();
    client.run_until("a packet after the resume", |c| {
        c.log.iter().any(|l| l == "packet 153")
    });
    client.log.clear();

    // The socket drops again: the reconnect login is answered with 2, a
    // full login; then the server's LOGOUT returns to the title screen.
    go.send(()).unwrap();
    client.run_until("the second reconnect", |c| {
        c.log.iter().any(|l| l == "25 -> 30")
    });
    assert_eq!(next(), Seen::Login(18, 77, [1024, 700]));
    assert_eq!(
        client.log,
        [
            "set aside",
            "30 -> 40",
            "reset 300",
            "window status",
            "packet 162",
            "packet 153",
            "40 -> 25",
            "send 20",
            "25 -> 30",
            "send 110"
        ]
    );
    client.log.clear();
    go.send(()).unwrap();
    client.run_until("the logout", |c| c.life.state() == GameState::Title);
    assert_eq!(
        client.log,
        [
            "packet 86",
            &format!("logout {:?}", LogoutReason::Server),
            "30 -> 10"
        ]
    );
    assert_eq!(client.flow.borrow().game_state, 10);
    assert!(client.connection.is_none());
    client.log.clear();

    // Back at the title screen the form logs in again; the socket then
    // drops with nobody listening, and both reconnect attempts fail.
    // (LoginManager.processLogout zeroed Protocol.verifyId.)
    client.seam.env.verify_id = 0;
    client.script_login();
    client.run_until("the second login", |c| c.life.state() == GameState::InGame);
    assert_eq!(next(), Seen::Login(16, 0, [1024, 700]));
    client.log.clear();
    go.send(()).unwrap();
    client.run_until("the failed reconnect", |c| {
        c.life.state() == GameState::Title
    });
    assert_eq!(
        client.log,
        [
            "set aside",
            "30 -> 40",
            &format!("logout {:?}", LogoutReason::ReconnectFailed(-4)),
            "40 -> 10"
        ]
    );
    assert_eq!(client.flow.borrow().op5607(), -4);
    assert_eq!(client.flow.borrow().game_state, 10);
}
