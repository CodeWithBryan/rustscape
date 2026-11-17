//! The login, account-creation and world-list state machines against the
//! original client.
//!
//! `login_flow/java_trace.txt` is what the JVM printed while the original
//! `LoginManager` loop methods, `CreateManager` and `WorldList` ran
//! `login_flow/scenarios.txt` over scripted sockets. `matches_the_original`
//! runs the same scenarios through the Rust flow and requires the same
//! trace, line for line: every connection opened, every byte written, every
//! close, and the state after each command and each cycle that changed it.
//!
//! The JVM side ran `LoginManager.clear`, `loopAuto`, `loop`,
//! `continueDelayedLogin` and `startLogin` (sliced unchanged from the
//! reference file), and `CreateManager`, `WorldList`, `Base37`, `JagString`,
//! `Buffer`, `Packet` and `IsaacRandom` whole. Its harness transcribes, rather
//! than runs, the script-command guards, the login-relevant lines of
//! `client.setGameState`, the game-state 40 branch of `client.mainLoop`,
//! `Protocol.handleConnectionLost`, `client.mainInit`'s port arithmetic and
//! the host and port a JS5 connection attempt sets (client.java:1320-1323,
//! command `js5`); a blocking socket read there lasts 1500 scripted cycles
//! before it times out. The harness and its stand-in classes are in
//! `oracle/login-flow/` (`build.ps1` regenerates the trace);
//! `login_flow/sources.txt` records the hashes of the reference files and
//! of the scenario file the trace was generated from.
//!
//! The remaining tests cover what that oracle does not execute: the request
//! bytes against the reference server's decoders, the script-command guards,
//! the game-state bookkeeping and the handover to the in-game session.
use rs09_client::{
    login_flow::{
        Dialer, Endpoints, Entered, Environment, Event, JsonConfig, LoadingProgress, LoginFlow,
        Lost, Opened, ScriptError, Socket, StateChange, WorldListStartup, decode37,
        preferences_to_int, status_text, uid_bytes,
    },
    world_list::{EmptySlot, WorldList, compare_strings},
};
use rs09_protocol::{
    buffer::Reader,
    login::{self, AuthResponse, RsaKey},
    profile::RS09_REVISION_530,
};
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    io,
    rc::Rc,
};

#[derive(Default)]
struct Wire {
    input: VecDeque<u8>,
    eof: bool,
    ioerr: bool,
}
impl Wire {
    fn check(&mut self) -> io::Result<()> {
        if self.ioerr {
            self.ioerr = false;
            return Err(io::Error::other("scripted"));
        }
        Ok(())
    }
}
type Trace = Rc<RefCell<Vec<String>>>;

struct Request {
    status: Cell<i32>,
    wire: RefCell<Option<Rc<RefCell<Wire>>>>,
}
struct Script {
    trace: Trace,
    latest: Rc<RefCell<Option<Rc<Request>>>>,
}
struct ScriptSocket {
    wire: Rc<RefCell<Wire>>,
    trace: Trace,
}
impl Dialer for Script {
    type Socket = ScriptSocket;
    type Request = Rc<Request>;
    fn open(&mut self, host: &str, port: i32) -> Rc<Request> {
        self.trace.borrow_mut().push(format!("open {host} {port}"));
        let request = Rc::new(Request {
            status: Cell::new(0),
            wire: RefCell::new(None),
        });
        *self.latest.borrow_mut() = Some(request.clone());
        request
    }
    fn status(&mut self, request: &mut Rc<Request>) -> Opened<ScriptSocket> {
        match request.status.get() {
            1 => Opened::Ready(ScriptSocket {
                wire: request.wire.borrow().clone().expect("connected wire"),
                trace: self.trace.clone(),
            }),
            2 => Opened::Failed,
            _ => Opened::Pending,
        }
    }
}
impl Socket for ScriptSocket {
    fn available(&mut self) -> io::Result<usize> {
        let mut wire = self.wire.borrow_mut();
        wire.check()?;
        Ok(wire.input.len())
    }
    fn read(&mut self, out: &mut [u8]) -> io::Result<()> {
        let mut wire = self.wire.borrow_mut();
        wire.check()?;
        if wire.input.len() < out.len() {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        for byte in out {
            *byte = wire.input.pop_front().expect("checked length");
        }
        Ok(())
    }
    fn at_end(&mut self) -> bool {
        let wire = self.wire.borrow();
        wire.eof && wire.input.is_empty()
    }
    fn write(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.wire.borrow_mut().check()?;
        self.trace
            .borrow_mut()
            .push(format!("write {}", hex(bytes)));
        Ok(())
    }
    fn close(&mut self) {
        self.trace.borrow_mut().push("close".into());
    }
}

fn hex(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return "-".into();
    }
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn token(token: &str) -> Vec<u8> {
    if token == "-" {
        Vec::new()
    } else if let Some(text) = token.strip_prefix('\'') {
        text.bytes().collect()
    } else {
        (0..token.len() / 2)
            .map(|i| u8::from_str_radix(&token[i * 2..i * 2 + 2], 16).expect("hex token"))
            .collect()
    }
}
fn tokens(parts: &[&str]) -> Vec<u8> {
    parts.iter().flat_map(|part| token(part)).collect()
}

struct Harness {
    flow: LoginFlow<Script>,
    trace: Trace,
    latest: Rc<RefCell<Option<Rc<Request>>>>,
    current: Option<Rc<RefCell<Wire>>>,
    random: Rc<RefCell<VecDeque<f64>>>,
    last_signature: String,
    window_mode: i32,
    size: [i32; 2],
    anti_aliasing: i32,
    uid: [u8; 24],
    settings: Vec<u8>,
    affiliate: i32,
    preferences: i32,
    verify_id: i32,
    checksums: [i32; 28],
    advert: bool,
}
impl Harness {
    fn new(trace: Trace) -> Self {
        let latest = Rc::new(RefCell::new(None));
        let random = Rc::new(RefCell::new(VecDeque::new()));
        let queue = random.clone();
        let flow = LoginFlow::new(
            Script {
                trace: trace.clone(),
                latest: latest.clone(),
            },
            Endpoints::new("host", 0, 1, None, true),
            Box::new(move || queue.borrow_mut().pop_front().unwrap_or(0.0)),
        );
        Self {
            flow,
            trace,
            latest,
            current: None,
            random,
            last_signature: String::new(),
            window_mode: 0,
            size: [765, 503],
            anti_aliasing: 0,
            uid: [0; 24],
            settings: Vec::new(),
            affiliate: 0,
            preferences: 0,
            verify_id: 0,
            checksums: [0; 28],
            advert: false,
        }
    }
    fn say(&self, line: impl Into<String>) {
        self.trace.borrow_mut().push(line.into());
    }
    fn state_line(&self, signature: bool) -> String {
        let f = &self.flow;
        let count = |n: i32| {
            if signature {
                "*".to_owned()
            } else {
                n.to_string()
            }
        };
        format!(
            "st g={} L={}/{}/{}/{} H={}/{}/{}/{} d={} C={}/{}/{}/{} W={}/{}/{}/{} P={}:{}/{}/{} WL={}:{} w={} s={} p={} r={}",
            f.game_state,
            f.login.step,
            count(f.login.loops),
            f.login.errors,
            f.login.reply,
            f.login.hop_step,
            count(f.login.hop_loops),
            f.login.hop_errors,
            f.login.hop_time,
            f.login.disallow_result,
            f.create.step,
            count(f.create.loops),
            f.create.errors,
            f.create.reply,
            f.worlds.step,
            f.worlds.errors,
            i32::from(f.worlds.loaded),
            f.worlds.size,
            f.endpoints.hostname,
            f.endpoints.port,
            f.endpoints.default_port,
            f.endpoints.alternate_port,
            f.endpoints.world_list_hostname,
            f.endpoints.world_list_port,
            f.endpoints.world_id,
            i32::from(f.net.socket.is_some()),
            i32::from(f.net.previous_socket.is_some()),
            i32::from(f.net.request.is_some()),
        )
    }
    fn print_state(&mut self) {
        self.say(self.state_line(false));
        self.last_signature = self.state_line(true);
    }
    fn tick(&mut self, last: bool) {
        let settings = self.settings.clone();
        let env = Environment {
            advert_suppressed: self.advert,
            window_mode: self.window_mode,
            canvas_width: self.size[0],
            canvas_height: self.size[1],
            anti_aliasing: self.anti_aliasing,
            uid: self.uid,
            settings: &settings,
            affiliate: self.affiliate,
            preferences: self.preferences,
            verify_id: self.verify_id,
            checksums: self.checksums,
        };
        for event in self.flow.cycle(&env) {
            match event {
                Event::PreferencesSent => self.say("ev prefs"),
                Event::Entered(e) => self.say(format!(
                    "ev entered staff={} marks={} underage={} chat={} advert={} quick={} mouse={} self={} member={} members={} opcode={} rebuild={}",
                    e.world.staff_level,
                    e.world.blackmarks,
                    e.world.underage,
                    e.world.parental_chat_consent,
                    e.world.parental_ad_consent,
                    e.world.quick_chat,
                    e.world.mouse_recording,
                    e.world.player_index,
                    e.world.player_member,
                    e.world.map_members,
                    e.opcode,
                    hex(&e.rebuild)
                )),
                Event::Reconnected(_) => self.say("ev reconnected"),
                Event::LoggedOut { reply, .. } => self.say(format!("ev loggedout reply={reply}")),
            }
        }
        if self.state_line(true) != self.last_signature || last {
            self.print_state();
        }
    }
    fn describe(&self, prefix: &str, entry: Option<rs09_client::world_list::Entry>, id: bool) {
        match entry {
            None => self.say(format!("{prefix} null")),
            Some(e) => self.say(format!(
                "{prefix}{} {} {} {} {} {}",
                if id {
                    format!(" {}", e.id)
                } else {
                    String::new()
                },
                e.flags,
                hex(&e.activity),
                e.country_flag,
                hex(&e.country_name),
                e.players
            )),
        }
    }
    fn wire(&self) -> Rc<RefCell<Wire>> {
        self.current.clone().expect("a connected socket")
    }
    fn exec(&mut self, c: &[&str]) {
        let int = |i: usize| c[i].parse::<i32>().expect("integer");
        match c[0] {
            "config" => {
                let json = (c.len() > 5).then(|| JsonConfig {
                    ip_management: c[5].into(),
                    ip_address: c[6].into(),
                    world: int(7),
                    server_port: int(8),
                    wl_port: int(9),
                });
                self.flow.endpoints = Endpoints::new("host", int(1), int(2), json, c[3] == "1");
                self.flow.title_scene = c[4] == "1";
            }
            "random" => {
                self.random
                    .borrow_mut()
                    .extend(c[1..].iter().map(|d| d.parse::<f64>().expect("double")));
                return;
            }
            "js5" => self.flow.endpoints.js5_connect(),
            "env" => {
                self.window_mode = int(1);
                self.size = [int(2), int(3)];
                self.anti_aliasing = int(4);
                let uid = token(c[5]);
                self.uid = [0; 24];
                self.uid[..uid.len().min(24)].copy_from_slice(&uid[..uid.len().min(24)]);
                self.settings = token(c[6]);
                self.affiliate = int(7);
                self.preferences = int(8);
                self.verify_id = int(9);
                for (i, crc) in self.checksums.iter_mut().enumerate() {
                    *crc = int(10) + i as i32 * 7919;
                }
                self.advert = c[11] == "1";
                return;
            }
            "state" => self.flow.game_state = int(1),
            "setstate" => {
                self.flow.set_game_state(int(1));
            }
            "login" => {
                if self
                    .flow
                    .script_login(&token(c[1]), &token(c[2]), int(3))
                    .is_err()
                {
                    self.say("err");
                }
            }
            "continue" => self.flow.script_continue_login(),
            "resetlogin" => self.flow.script_reset_login_reply(),
            "checkinfo" => self
                .flow
                .script_check_info([int(1), int(2), int(3), int(4)]),
            "checkname" => {
                if self.flow.script_check_name(&token(c[1])).is_err() {
                    self.say("err");
                }
            }
            "create" => {
                if self
                    .flow
                    .script_create_account(
                        [int(1), int(2), int(3), int(4)],
                        &token(c[5]),
                        &token(c[6]),
                        int(7),
                    )
                    .is_err()
                {
                    self.say("err");
                }
            }
            "resetcreate" => self.flow.script_reset_create_reply(),
            "names" => match self.flow.script_suggested_names() {
                Ok(names) => self.say(format!(
                    "names {}",
                    names.iter().map(|n| hex(n)).collect::<Vec<_>>().join(" ")
                )),
                Err(_) => self.say("names err"),
            },
            "outbound" => {
                self.say(format!("outbound {}", hex(&self.flow.net.outbound)));
                return;
            }
            "fetch" => {
                let now = c[1].parse::<i64>().expect("time");
                let flow = &mut self.flow;
                let value = flow.worlds.fetch(&mut flow.net, &mut flow.endpoints, now);
                self.say(format!("ret {value}"));
            }
            "hop" => {
                let value = self.flow.script_hop_world(int(1));
                self.say(format!("ret {value}"));
            }
            "sort" => {
                if self
                    .flow
                    .worlds
                    .sort(int(1), c[2] == "1", int(3), c[4] == "1")
                    .is_err()
                {
                    self.say("err");
                }
                return;
            }
            "list" => {
                let mut count = 0;
                let mut entry = self.flow.worlds.first_world();
                while entry.is_some() {
                    self.describe("w", entry, true);
                    count += 1;
                    entry = self.flow.worlds.next_world();
                }
                self.say(format!("listed {count}"));
                return;
            }
            "get" => {
                self.describe("g", self.flow.worlds.get(int(1)), false);
                return;
            }
            "connect" => {
                let request = self.latest.borrow().clone().expect("a socket request");
                if c[1] == "ok" {
                    let wire = Rc::new(RefCell::new(Wire::default()));
                    *request.wire.borrow_mut() = Some(wire.clone());
                    request.status.set(1);
                    self.current = Some(wire);
                } else {
                    request.status.set(2);
                }
                return;
            }
            "recv" => self.wire().borrow_mut().input.extend(tokens(&c[1..])),
            "recvlen" => {
                let bytes = tokens(&c[1..]);
                let wire = self.wire();
                let mut wire = wire.borrow_mut();
                wire.input.extend((bytes.len() as u16).to_be_bytes());
                wire.input.extend(bytes);
            }
            "eof" => self.wire().borrow_mut().eof = true,
            "ioerr" => self.wire().borrow_mut().ioerr = true,
            "lost" => match self.flow.connection_lost(int(1), None) {
                Lost::LoggedOut(_) => self.say("lost loggedout"),
                Lost::Reconnecting(_) => self.say("lost reconnecting"),
            },
            "tick" => {
                let count = int(1);
                for i in 0..count {
                    self.tick(i + 1 == count);
                }
                return;
            }
            other => panic!("unknown scenario command {other}"),
        }
        self.print_state();
    }
}

fn run_scenarios(text: &str) -> Vec<String> {
    let trace: Trace = Rc::default();
    let mut harness = Harness::new(trace.clone());
    for line in text.lines().map(str::trim) {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        trace.borrow_mut().push(format!("> {line}"));
        let command: Vec<&str> = line.split(' ').filter(|part| !part.is_empty()).collect();
        if command[0] == "scenario" {
            harness = Harness::new(trace.clone());
        } else {
            harness.exec(&command);
        }
    }
    drop(harness);
    Rc::try_unwrap(trace).expect("sole owner").into_inner()
}

#[test]
fn matches_the_original() {
    let expected: Vec<&str> = include_str!("login_flow/java_trace.txt").lines().collect();
    let actual = run_scenarios(include_str!("login_flow/scenarios.txt"));
    let mut scenario = "";
    for (index, (want, got)) in expected.iter().zip(&actual).enumerate() {
        if let Some(name) = want.strip_prefix("> scenario ") {
            scenario = name;
        }
        assert_eq!(
            got,
            want,
            "trace line {} in scenario {scenario}; preceding lines:\n{}",
            index + 1,
            actual[index.saturating_sub(8)..index].join("\n")
        );
    }
    assert_eq!(actual.len(), expected.len(), "trace length");
    // The fixture is the complete run, not a prefix of it.
    assert_eq!(
        expected
            .iter()
            .filter(|l| l.starts_with("> scenario "))
            .count(),
        32
    );
    assert!(expected.len() > 1500, "{} trace lines", expected.len());
}

// ----- helpers for the Rust-only tests -----

struct Server {
    flow: LoginFlow<Script>,
    trace: Trace,
    latest: Rc<RefCell<Option<Rc<Request>>>>,
    wire: Option<Rc<RefCell<Wire>>>,
}
impl Server {
    fn new(endpoints: Endpoints, random: Vec<f64>) -> Self {
        let trace: Trace = Rc::default();
        let latest = Rc::new(RefCell::new(None));
        let mut random: VecDeque<f64> = random.into();
        let mut flow = LoginFlow::new(
            Script {
                trace: trace.clone(),
                latest: latest.clone(),
            },
            endpoints,
            Box::new(move || random.pop_front().unwrap_or(0.0)),
        );
        flow.game_state = 10;
        Self {
            flow,
            trace,
            latest,
            wire: None,
        }
    }
    fn title() -> Self {
        Self::new(Endpoints::new("host", 0, 1, None, true), vec![])
    }
    fn accept(&mut self) {
        let request = self.latest.borrow().clone().expect("a socket request");
        let wire = Rc::new(RefCell::new(Wire::default()));
        *request.wire.borrow_mut() = Some(wire.clone());
        request.status.set(1);
        self.wire = Some(wire);
    }
    fn send(&mut self, bytes: &[u8]) {
        self.wire
            .as_ref()
            .expect("accepted")
            .borrow_mut()
            .input
            .extend(bytes);
    }
    fn cycle(&mut self) -> Vec<Event> {
        self.flow.cycle(&Environment::default())
    }
    fn writes(&self) -> Vec<Vec<u8>> {
        self.trace
            .borrow()
            .iter()
            .filter_map(|line| line.strip_prefix("write "))
            .map(token)
            .collect()
    }
    fn opened(&self) -> Vec<String> {
        self.trace
            .borrow()
            .iter()
            .filter_map(|line| line.strip_prefix("open "))
            .map(str::to_owned)
            .collect()
    }
    /// Log in with the given reply bytes after the handshake.
    fn enter(&mut self, key: i64, after_key: &[u8]) -> Vec<Event> {
        self.flow.script_login(b"player", b"password", 0).unwrap();
        self.cycle();
        self.accept();
        let mut bytes = vec![0];
        bytes.extend_from_slice(&key.to_be_bytes());
        bytes.extend_from_slice(after_key);
        self.send(&bytes);
        self.cycle()
    }
}
const DETAILS: [u8; 11] = [2, 0, 0, 0, 1, 0, 0, 0, 7, 1, 1];
fn success(payload: &[u8]) -> Vec<u8> {
    let mut bytes = vec![2];
    bytes.extend_from_slice(&DETAILS);
    bytes.push(162);
    bytes.extend_from_slice(&(payload.len() as u16).to_be_bytes());
    bytes.extend_from_slice(payload);
    bytes
}
fn entered(events: Vec<Event>) -> Entered {
    events
        .into_iter()
        .find_map(|event| match event {
            Event::Entered(entered) => Some(*entered),
            _ => None,
        })
        .expect("an Entered event")
}

// ----- request bytes against the reference server's decoders -----

/// The login block is read back by the port of the reference server's
/// `Login.decodeFromBuffer` (itself checked against the Kotlin original by
/// the login oracle).
#[test]
fn the_reference_server_decodes_the_login_block() {
    let mut server = Server::new(
        Endpoints::new("host", 0, 1, None, true),
        vec![0.12345678, 0.87654321],
    );
    let key = 0x0123_4567_89AB_CDEFu64 as i64;
    server.flow.script_login(b"Zezima", b"hunter2", 0).unwrap();
    server.cycle();
    server.accept();
    let mut reply = vec![0];
    reply.extend_from_slice(&key.to_be_bytes());
    server.send(&reply);
    let env = Environment {
        window_mode: 2,
        canvas_width: 1280,
        canvas_height: 720,
        anti_aliasing: 1,
        affiliate: 55,
        preferences: 0x00AB_CDEF,
        verify_id: 321,
        settings: b"cookie",
        checksums: std::array::from_fn(|i| 1000 + i as i32),
        ..Environment::default()
    };
    let events = server.flow.cycle(&env);
    assert_eq!(events, [Event::PreferencesSent]);
    let writes = server.writes();
    assert_eq!(writes[0], login::handshake(b"Zezima"));
    let block = &writes[1];
    assert_eq!(block[0], 16);
    assert_eq!(
        usize::from(u16::from_be_bytes([block[1], block[2]])),
        block.len() - 3
    );
    let outcome = login::decode(block, &RsaKey::reference_private(), |_| false);
    assert_eq!(outcome.response, AuthResponse::Success);
    let info = outcome.info.expect("decoded login");
    assert_eq!(info.opcode, 16);
    assert_eq!(info.username, "zezima");
    assert_eq!(info.password, "hunter2");
    assert_eq!(info.window_mode, 2);
    assert_eq!([info.screen_width, info.screen_height], [1280, 720]);
    assert_eq!(info.display_mode, 1);
    assert_eq!(info.affiliate, 55);
    assert_eq!(info.settings_hash, 0x00AB_CDEF);
    assert_eq!(info.packet_count, 321);
    assert!(!info.show_ads);
    assert_eq!(info.crc_sums[..28], env.checksums);
    assert_eq!(info.crc_sums[28], 0);
    // key[1] is drawn before key[0]; the server key fills the other half.
    let seed = [
        (0.87654321f64 * 9.9999999E7) as i32,
        (0.12345678f64 * 9.9999999E7) as i32,
        0x0123_4567,
        0x89AB_CDEFu32 as i32,
    ];
    assert_eq!(info.isaac_seed, Some(seed));
    assert_eq!(server.flow.login.isaac_seed, seed);
    assert_eq!(server.flow.login.server_key, key);
    assert_eq!(server.flow.name37, login::encode_name(b"Zezima"));
}

fn decrypt(block: &[u8]) -> Vec<u8> {
    RsaKey::reference_private()
        .unframe(&mut Reader::new(block))
        .expect("RSA block")
}

/// Account creation as the reference server's `AccountRegister.read` reads
/// it: opcode, a size byte it skips, then the RSA block.
#[test]
fn the_reference_server_reads_the_create_request() {
    let mut server = Server::new(
        Endpoints::new("host", 0, 1, None, true),
        vec![0.5, 0.1, 0.2, 0.3, 0.4],
    );
    server
        .flow
        .script_create_account([31, 12, 1990, 225], b"New User", b"hunter22", 1234)
        .unwrap();
    let out = server.flow.net.outbound.clone();
    assert_eq!(out[0], 36);
    assert_eq!(usize::from(out[1]), out.len() - 2);
    let plain = decrypt(&out[2..]);
    let mut r = Reader::new(&plain);
    assert_eq!(r.g1().unwrap(), 10);
    assert_eq!(r.g2().unwrap(), 49999);
    assert_eq!(r.g2().unwrap(), 530);
    assert_eq!(r.gjstr().unwrap(), b"new_user");
    assert_eq!(r.g4().unwrap(), 9999999);
    assert_eq!(r.gjstr().unwrap(), b"hunter22");
    assert_eq!(r.g4().unwrap(), 19999999);
    assert_eq!(r.g2().unwrap(), 1234);
    assert_eq!(r.g1().unwrap(), 31);
    assert_eq!(r.g1().unwrap(), 12);
    assert_eq!(r.g4().unwrap(), 29999999);
    assert_eq!(r.g2().unwrap(), 1990);
    assert_eq!(r.g2().unwrap(), 225);
    assert_eq!(r.g4().unwrap(), 39999999);
    assert_eq!(r.remaining(), 0);
    assert_eq!(
        (
            server.flow.create.step,
            server.flow.create.reply,
            server.flow.create.loops,
            server.flow.create.errors
        ),
        (1, -3, 0, 0)
    );
}

/// The name check and the date check as `AccountRegister.read` reads them.
#[test]
fn the_reference_server_reads_the_name_and_date_checks() {
    let mut server = Server::title();
    server.flow.script_check_name(b"Cool Name").unwrap();
    assert_eq!(server.flow.net.outbound, b"\xbacool_name\0");
    server.flow.create.step = 0;
    server.flow.script_check_info([31, 12, 1990, 225]);
    let out = server.flow.net.outbound.clone();
    let mut r = Reader::new(&out);
    assert_eq!(r.g1().unwrap(), 147);
    assert_eq!(r.g1().unwrap(), 31); // day
    assert_eq!(r.g1().unwrap(), 12); // month
    assert_eq!(r.g2().unwrap(), 1990); // year
    assert_eq!(r.g2().unwrap(), 225); // country
    assert_eq!(r.remaining(), 0);
}

/// The world-hop request has no reader in the reference server (its
/// handshake decoder drops opcode 210), so only the layout written by
/// `LoginManager.startLogin` is checked. The pinned key's output does not
/// fit the original's 128-byte buffer (see the `hop` scenarios), so this
/// uses the public exponent 1, which leaves the block readable and short.
#[test]
fn the_hop_request_carries_the_credentials() {
    let mut random: VecDeque<f64> = vec![0.5, 0.25, 0.75].into();
    let trace: Trace = Rc::default();
    let latest = Rc::new(RefCell::new(None));
    let mut flow = LoginFlow::with_profile(
        Script {
            trace: trace.clone(),
            latest: latest.clone(),
        },
        Endpoints::new("host", 0, 1, None, false),
        Box::new(move || random.pop_front().unwrap_or(0.0)),
        RS09_REVISION_530,
        RsaKey::new("1", login::RSA_MODULUS).unwrap(),
    );
    flow.game_state = 10;
    let mut server = Server {
        flow,
        trace,
        latest,
        wire: None,
    };
    server.flow.script_login(b"Zezima", b"hunter2", 0).unwrap();
    let out = server.flow.net.outbound.clone();
    assert_eq!(out[0], 210);
    assert_eq!(usize::from(out[1]), out.len() - 2);
    assert_eq!(usize::from(out[2]), out.len() - 3);
    let plain = out[3..].to_vec();
    let mut r = Reader::new(&plain);
    assert_eq!(r.g1().unwrap(), 10);
    assert_eq!(r.g2().unwrap(), 49999);
    assert_eq!(r.g2().unwrap(), 530);
    assert_eq!(r.g8().unwrap(), login::encode_name(b"Zezima"));
    assert_eq!(r.g4().unwrap(), 24999999);
    assert_eq!(r.gjstr().unwrap(), b"hunter2");
    assert_eq!(r.g4().unwrap(), 74999999);
    assert_eq!(r.remaining(), 0);
    assert_eq!(
        (server.flow.login.hop_step, server.flow.login.reply),
        (1, -3)
    );
}

/// The world-list payload the Rust server sends is the one this client
/// decodes.
#[test]
fn decodes_the_server_world_list() {
    use rs09_protocol::lobby::{World, world_list};
    let worlds = [
        World::default(),
        World {
            id: 2,
            location: 1,
            flags: 4,
            activity: b"-".to_vec(),
            host: b"two.example".to_vec(),
            region: b"Nowhere".to_vec(),
            country: 77,
            players: 65535,
        },
    ];
    let packet = world_list(&worlds, 9, 0).unwrap();
    assert_eq!(packet[0], 0);
    assert_eq!(
        usize::from(u16::from_be_bytes([packet[1], packet[2]])),
        packet.len() - 3
    );
    let mut list = WorldList::default();
    assert!(list.decode(&packet[3..]));
    assert!(list.loaded);
    assert_eq!((list.min_id, list.max_id, list.size), (0, 2, 2));
    assert_eq!(list.checksum, 9);
    let one = list.get(1).unwrap();
    assert_eq!(one.flags, 9);
    assert_eq!(one.activity, b"2009Scape Classic");
    assert_eq!(
        (one.country_flag, &one.country_name[..]),
        (225, &b"Anywhere, USA"[..])
    );
    assert_eq!(one.players, 0);
    let two = list.get(2).unwrap();
    assert_eq!((two.country_flag, two.players), (77, -1));
    assert_eq!(list.world(2).unwrap().hostname, b"two.example");
    assert!(list.world(2).unwrap().is_pvp());
    assert!(list.get(0).is_none() && list.get(3).is_none());
    // An unchanged stamp sends the player counts only.
    let update = world_list(&worlds, 9, 9).unwrap();
    assert!(list.decode(&update[3..]));
    assert_eq!(list.get(2).unwrap().players, -1);
}

// ----- malformed world lists -----

#[test]
fn rejects_malformed_world_lists_and_keeps_the_previous_one() {
    let good: Vec<u8> = [
        &[1, 1, 1, 5, 0][..],
        b"Z\0",
        &[1, 2, 1, 0, 0, 0, 0, 0, 9, 0],
        b"act\0",
        &[0],
        b"host\0",
        &[0, 0, 0, 7, 0, 0, 3],
    ]
    .concat();
    let mut list = WorldList::default();
    assert!(list.decode(&good));
    let before = (list.get(1), list.checksum, list.size);
    assert_eq!(before.0.as_ref().unwrap().players, 3);
    let mut cases: Vec<Vec<u8>> = vec![
        vec![],
        vec![1],
        vec![2, 0],
        vec![0, 0],
        // Country string with version byte 1.
        [&[1, 1, 1, 5, 1][..], b"Z\0"].concat(),
        // max_id below min_id - 1: a negative table.
        [&[1, 1, 0, 9, 3, 0][..], &[0, 0, 0, 0]].concat(),
        // A world outside the table.
        [
            &[1, 1, 1, 5, 0][..],
            b"Z\0",
            &[1, 1, 1, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            &[0, 0, 0, 0],
        ]
        .concat(),
        // A country outside the country table.
        [
            &[1, 1, 1, 5, 0][..],
            b"Z\0",
            &[1, 1, 1, 0, 3, 0, 0, 0, 0, 0, 0, 0, 0],
            &[0, 0, 0, 0, 0, 0, 0],
        ]
        .concat(),
        // A player count for a world outside the table.
        vec![1, 0, 9, 0, 1],
    ];
    for cut in 1..good.len() {
        cases.push(good[..cut].to_vec());
    }
    for case in cases {
        assert!(!list.decode(&case), "{case:?}");
        assert_eq!((list.get(1), list.checksum, list.size), before, "{case:?}");
        assert!(list.loaded);
    }
}

#[test]
fn a_list_with_an_empty_slot_is_not_sorted() {
    // Two entries naming the same world leave one slot empty.
    let data: Vec<u8> = [
        &[1, 1, 1, 5, 0][..],
        b"Z\0",
        &[1, 2, 3],
        &[0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        &[0, 0, 0, 0, 0, 1, 0, 0, 0, 0],
        &[1, 0, 0, 0, 0, 2, 0, 0, 0, 0],
        &[0, 0, 0, 0],
        &[0, 0, 5, 0, 0, 6, 1, 0, 7],
    ]
    .concat();
    let mut list = WorldList::default();
    assert!(list.decode(&data));
    // `fetch` builds the sorted array; do the same through a fetch.
    let mut server = Server::title();
    server.flow.worlds = list;
    let now = 100_000;
    assert_eq!(server.flow.script_fetch_world_list(now), 0);
    server.accept();
    let mut reply = vec![0];
    reply.extend_from_slice(&(data.len() as u16).to_be_bytes());
    reply.extend_from_slice(&data);
    server.send(&reply);
    assert_eq!(server.flow.script_fetch_world_list(now), 1);
    let worlds = &mut server.flow.worlds;
    assert_eq!(worlds.sorted, [Some(0), Some(1), None]);
    let before = worlds.sorted.clone();
    assert_eq!(worlds.sort(1, false, -1, false), Err(EmptySlot));
    assert_eq!(worlds.sorted, before);
    assert_eq!(worlds.first_world().unwrap().players, 6);
    assert_eq!(worlds.next_world().unwrap().players, 7);
    assert!(worlds.next_world().is_none());
    assert_eq!(worlds.world_pos, 3);
    assert!(worlds.next_world().is_none());
    assert_eq!(worlds.world_pos, 3);
}

#[test]
fn collation_expands_ligatures() {
    assert_eq!(compare_strings(b"", b""), 0);
    assert_eq!(compare_strings(b"a", b""), 1);
    assert_eq!(compare_strings(b"", b"a"), -1);
    assert_eq!(compare_strings(b"abc", b"abd"), -1);
    // Case folds to neighbouring weights: lower case sorts after upper.
    assert_eq!(compare_strings(b"a", b"A"), 1);
    assert_eq!(compare_strings(b"B", b"a"), -1);
    // The ae ligature is "ae": same weights, and the same length once the
    // ligature is counted as two letters.
    assert_eq!(compare_strings(b"\xe6", b"ae"), 0);
    assert_eq!(compare_strings(b"ae", b"\xe6"), 0);
    assert_eq!(compare_strings(b"\xe6", b"af"), -1);
    assert_eq!(compare_strings(b"\xe6", b"a"), 1);
    // The sharp s has its own weight, just above "s" and below "t".
    assert_eq!(compare_strings(b"Stra\xdfe", b"Strasse"), 1);
    assert_eq!(compare_strings(b"Stra\xdf", b"Strat"), -1);
}

// ----- script commands -----

#[test]
fn commands_act_only_at_an_idle_title_screen() {
    for state in [0, 5, 25, 28, 30, 40, 1000] {
        let mut server = Server::title();
        server.flow.game_state = state;
        server.flow.script_login(b"a", b"b", 0).unwrap();
        server.flow.script_check_name(b"a").unwrap();
        server.flow.script_check_info([1, 1, 2000, 1]);
        server
            .flow
            .script_create_account([1, 1, 2000, 1], b"a", b"password", 0)
            .unwrap();
        // An invalid name is not even looked at.
        server.flow.script_check_name(b"").unwrap();
        assert_eq!(server.flow.script_fetch_world_list(100_000), 1);
        assert_eq!(server.flow.script_hop_world(1), 0);
        assert_eq!(
            (
                server.flow.login.step,
                server.flow.login.reply,
                server.flow.create.step,
                server.flow.create.reply,
                server.flow.worlds.step
            ),
            (0, -2, 0, -2, 0),
            "state {state}"
        );
        assert!(server.flow.net.outbound.is_empty());
        assert!(server.opened().is_empty());
    }
    // A world-list fetch in progress blocks login and creation, but a login
    // in progress answers the fetch command with 1 without fetching.
    let mut server = Server::title();
    assert_eq!(server.flow.script_fetch_world_list(100_000), 0);
    assert!(!server.flow.idle());
    server.flow.script_login(b"a", b"b", 0).unwrap();
    assert_eq!(server.flow.login.step, 0);
    let mut server = Server::title();
    server.flow.script_login(b"a", b"b", 0).unwrap();
    assert_eq!(server.flow.script_fetch_world_list(100_000), 1);
    assert_eq!(server.flow.worlds.step, 0);
}

#[test]
fn the_reply_getters_start_at_the_original_values() {
    let server = Server::title();
    assert_eq!(server.flow.login.reply, -2);
    assert_eq!(server.flow.login.hop_time, 0);
    assert_eq!(server.flow.create.reply, -2);
    assert_eq!(server.flow.login.disallow_result, -1);
    assert_eq!(server.flow.login.login_type, -1);
    assert_eq!(server.flow.endpoints.world_id, 1);
}

#[test]
fn script_errors_leave_the_state_alone() {
    let mut server = Server::title();
    assert_eq!(
        server.flow.script_suggested_names(),
        Err(ScriptError::NoSuggestions)
    );
    assert_eq!(
        server.flow.script_check_name(b"!!"),
        Err(ScriptError::InvalidName)
    );
    assert_eq!(server.flow.create.step, 0);
    assert_eq!(
        server
            .flow
            .script_create_account([1, 1, 2000, 1], b"name", &[b'p'; 200], 0),
        Err(ScriptError::BlockTooLong)
    );
    assert_eq!(server.flow.create.step, 0);
    // The longest password the 129-byte block holds with this name.
    let longest = 129 - 29 - 5 - 1;
    server
        .flow
        .script_create_account([1, 1, 2000, 1], b"name", &vec![b'p'; longest], 0)
        .unwrap();
    assert_eq!(server.flow.create.step, 1);
}

/// The original arms the world hop before it builds the request, and with
/// the pinned key the request overruns its buffer: the command fails with
/// the hop armed and the shared outbound buffer untouched.
#[test]
fn a_failed_hop_request_leaves_the_hop_armed() {
    let mut server = Server::new(Endpoints::new("host", 0, 1, None, false), vec![]);
    server.flow.net.outbound = vec![1, 2, 3];
    assert_eq!(
        server.flow.script_login(b"name", b"password", 0),
        Err(ScriptError::BlockTooLong)
    );
    assert_eq!(
        (
            server.flow.login.hop_step,
            server.flow.login.reply,
            server.flow.login.step
        ),
        (1, -3, 0)
    );
    assert_eq!(server.flow.username(), b"name");
    assert_eq!(server.flow.net.outbound, [1, 2, 3]);
    server.cycle();
    server.accept();
    server.cycle();
    assert_eq!(server.opened(), ["host 43595"]);
    assert_eq!(server.writes(), [vec![1, 2, 3]]);
}

#[test]
fn base37_names() {
    assert_eq!(decode37(0), None);
    assert_eq!(decode37(-1), None);
    assert_eq!(decode37(37), None);
    assert_eq!(decode37(1).unwrap(), b"a");
    assert_eq!(decode37(6_582_952_005_840_035_281), None);
    assert_eq!(
        decode37(6_582_952_005_840_035_280).unwrap(),
        b"999999999999"
    );
    assert_eq!(
        decode37(login::encode_name(b"Cool Name 1")).unwrap(),
        b"cool_name_1"
    );
}

// ----- the game state -----

#[test]
fn set_game_state_reports_what_the_original_does() {
    let mut server = Server::title();
    server.flow.game_state = 0;
    assert_eq!(server.flow.set_game_state(0), None);
    let change = |from, to| StateChange {
        from,
        to,
        login_cleared: false,
        previous_socket_closed: false,
        loading_reset: false,
        title_background: false,
        title_music: None,
    };
    assert_eq!(
        server.flow.set_game_state(5),
        Some(StateChange {
            title_background: true,
            title_music: Some(true),
            ..change(0, 5)
        })
    );
    assert_eq!(server.flow.set_game_state(10), Some(change(5, 10)));
    assert_eq!(
        server.flow.set_game_state(28),
        Some(StateChange {
            loading_reset: true,
            ..change(10, 28)
        })
    );
    assert_eq!(server.flow.set_game_state(10), Some(change(28, 10)));
    assert_eq!(
        server.flow.set_game_state(25),
        Some(StateChange {
            loading_reset: true,
            title_music: Some(false),
            ..change(10, 25)
        })
    );
    assert_eq!(server.flow.set_game_state(30), Some(change(25, 30)));
    assert_eq!(server.flow.login.step, 0);
    assert_eq!(
        server.flow.set_game_state(40),
        Some(StateChange {
            login_cleared: true,
            ..change(30, 40)
        })
    );
    assert_eq!((server.flow.login.step, server.flow.login.reply), (1, -3));
    assert_eq!(server.flow.set_game_state(30), Some(change(40, 30)));
    assert_eq!(
        server.flow.set_game_state(10),
        Some(StateChange {
            title_music: Some(true),
            ..change(30, 10)
        })
    );
    assert_eq!(
        server.flow.set_game_state(1000),
        Some(StateChange {
            title_music: Some(false),
            ..change(10, 1000)
        })
    );
}

#[test]
fn losing_the_connection_keeps_the_old_socket_until_the_reconnect_ends() {
    let mut server = Server::title();
    let first = entered(server.enter(1, &success(&[])));
    assert_eq!(first.change.map(|c| (c.from, c.to)), Some((10, 25)));
    server.flow.set_game_state(30);
    // The caller hands the game socket back; it is kept, not closed.
    let socket = server.flow.net.socket.take();
    assert!(socket.is_some());
    let closes = |server: &Server| {
        server
            .trace
            .borrow()
            .iter()
            .filter(|line| *line == "close")
            .count()
    };
    assert_eq!(
        server.flow.connection_lost(0, socket),
        Lost::Reconnecting(Some(StateChange {
            from: 30,
            to: 40,
            login_cleared: true,
            previous_socket_closed: false,
            loading_reset: false,
            title_background: false,
            title_music: None,
        }))
    );
    assert!(server.flow.net.previous_socket.is_some());
    assert!(server.flow.net.socket.is_none());
    assert_eq!(closes(&server), 0);
    // Reply 15: the session carries on over the new connection.
    server.cycle();
    server.accept();
    server.send(&[0, 0, 0, 0, 0, 0, 0, 0, 2, 15]);
    let events = server.cycle();
    let Some(Event::Reconnected(resumed)) = events.last() else {
        panic!("{events:?}");
    };
    assert_eq!(resumed.isaac_seed, server.flow.login.isaac_seed);
    assert_eq!(
        resumed.change.map(|c| (c.to, c.previous_socket_closed)),
        Some((30, true))
    );
    assert_eq!(closes(&server), 1);
    assert!(server.flow.net.socket.is_some());
    // The reconnect block is opcode 18.
    assert_eq!(server.writes().last().unwrap()[0], 18);
    // With the logout timer running the client logs out instead.
    let socket = server.flow.net.socket.take();
    server.flow.title_scene = true;
    assert_eq!(
        server.flow.connection_lost(250, socket),
        Lost::LoggedOut(Some(StateChange {
            from: 30,
            to: 28,
            login_cleared: false,
            previous_socket_closed: false,
            loading_reset: true,
            title_background: false,
            title_music: Some(true),
        }))
    );
    assert_eq!(closes(&server), 2);
    assert!(server.flow.net.socket.is_none());
}

#[test]
fn startup_world_list_outcomes() {
    // Loaded.
    let mut server = Server::title();
    server.flow.game_state = 5;
    assert_eq!(
        server.flow.startup_world_list(10_000),
        WorldListStartup::Loading
    );
    assert_eq!(server.opened(), ["host 43595"]);
    server.accept();
    let payload = [
        &[1, 1, 1, 5, 0][..],
        b"Z\0",
        &[1, 1, 1, 0, 0, 0, 0, 0, 0, 0],
        b"-\0",
        &[0],
        b"h\0",
        &[0, 0, 0, 1, 0, 0, 2],
    ]
    .concat();
    let mut reply = vec![0];
    reply.extend_from_slice(&(payload.len() as u16).to_be_bytes());
    reply.extend_from_slice(&payload);
    server.send(&reply);
    assert_eq!(
        server.flow.startup_world_list(10_001),
        WorldListStartup::Loaded
    );
    assert_eq!(server.writes(), [vec![255, 0, 0, 0, 0]]);
    // Full: the fifth refusal in a row with code 7.
    let mut server = Server::title();
    let mut outcome = WorldListStartup::Loading;
    for _ in 0..5 {
        assert_eq!(
            server.flow.startup_world_list(10_000),
            WorldListStartup::Loading
        );
        server.accept();
        server.send(&[7]);
        outcome = server.flow.startup_world_list(10_000);
    }
    assert_eq!(outcome, WorldListStartup::Full);
    assert_eq!(
        server.opened(),
        [
            "host 43595",
            "host 43594",
            "host 43595",
            "host 43594",
            "host 43595"
        ]
    );
    // Failed: five connection failures.
    let mut server = Server::title();
    let mut outcome = WorldListStartup::Loading;
    for _ in 0..5 {
        server.flow.startup_world_list(10_000);
        server
            .latest
            .borrow()
            .as_ref()
            .expect("request")
            .status
            .set(2);
        outcome = server.flow.startup_world_list(10_000);
    }
    assert_eq!(outcome, WorldListStartup::Failed(1001));
    // A decoded answer without a world list is a failure with code 0.
    let mut server = Server::title();
    server.flow.startup_world_list(10_000);
    server.accept();
    server.send(&[0, 0, 2, 1, 0]);
    assert_eq!(
        server.flow.startup_world_list(10_000),
        WorldListStartup::Failed(0)
    );
}

// ----- handover to the game -----

#[test]
fn entered_hands_over_the_connection_and_cipher_state() {
    let mut server = Server::new(Endpoints::new("host", 0, 1, None, true), vec![0.3, 0.6]);
    let payload = [1, 2, 3, 4, 5, 6, 7];
    let e = entered(server.enter(77, &success(&payload)));
    assert_eq!(e.details, DETAILS);
    assert_eq!(e.header, [162, 0, 7]);
    assert_eq!(e.opcode, 162);
    assert_eq!(e.rebuild, payload);
    assert_eq!(e.server_key, 77);
    assert_eq!(e.outbound_opcodes, 0);
    assert_eq!(e.world.staff_level, 2);
    assert_eq!(e.world.player_index, 7);
    assert!(e.world.parental_ad_consent && e.world.player_member && e.world.map_members);
    assert_eq!(e.frame().payload, payload);
    assert_eq!(server.flow.game_state, 25);
    assert_eq!(server.flow.login.reply, 2);
    assert_eq!(server.flow.login.self_id, 7);
    assert!(server.flow.net.socket.is_some());
    // The existing in-game session picks up exactly here.
    let (mut session, events) = e
        .session(server.flow.username(), server.flow.profile(), 765, 503)
        .unwrap();
    assert!(session.in_game());
    assert_eq!(events.len(), 2);
    assert_eq!(
        events[0],
        rs09_client::session::Event::World(e.world.clone())
    );
    assert_eq!(events[1], rs09_client::session::Event::Frame(e.frame()));
    assert!(session.take_output().is_none());
    let more = session.feed(&[234, 100]).unwrap();
    assert_eq!(more.len(), 1);
    // The pinned profile has ISAAC off, so the ciphers pass opcodes through.
    assert_eq!(e.outbound_cipher(RS09_REVISION_530).encode(93), 93);
    let mut decoder = e.decoder(RS09_REVISION_530, 1024, 1024);
    decoder.push(&[234, 100]).unwrap();
    assert_eq!(decoder.next_frame().unwrap().unwrap().opcode, 234);
}

/// With ISAAC on, the handed-over ciphers continue the streams the login
/// used: the continue opcode consumed one outbound key and the first packet
/// one inbound key.
#[test]
fn handed_over_ciphers_continue_the_isaac_streams() {
    use rs09_protocol::isaac::OpcodeCipher;
    let mut profile = RS09_REVISION_530;
    profile.use_isaac = true;
    let trace: Trace = Rc::default();
    let latest = Rc::new(RefCell::new(None));
    let mut random: VecDeque<f64> = vec![0.3, 0.6].into();
    let mut flow = LoginFlow::with_profile(
        Script {
            trace: trace.clone(),
            latest: latest.clone(),
        },
        Endpoints::new("host", 0, 1, None, true),
        Box::new(move || random.pop_front().unwrap_or(0.0)),
        profile,
        RsaKey::reference_public(),
    );
    flow.game_state = 10;
    let mut server = Server {
        flow,
        trace,
        latest,
        wire: None,
    };
    let key = 0x1111_2222_3333_4444i64;
    let seed = [
        (0.6f64 * 9.9999999E7) as i32,
        (0.3f64 * 9.9999999E7) as i32,
        0x1111_2222,
        0x3333_4444,
    ];
    let mut outbound = OpcodeCipher::new(&seed.map(|w| w as u32), true).unwrap();
    let mut inbound = OpcodeCipher::new(&seed.map(|w| w.wrapping_add(50) as u32), true).unwrap();
    // Reply 1, continue, then success.
    assert!(
        server
            .enter(key, &[1])
            .iter()
            .all(|e| *e == Event::PreferencesSent)
    );
    assert_eq!((server.flow.login.step, server.flow.login.reply), (5, 1));
    server.flow.script_continue_login();
    server.cycle();
    let confirm = outbound.encode(17);
    assert_ne!(confirm, 17);
    assert_eq!(server.writes().last().unwrap(), &[confirm]);
    let wire_opcode = 162u8.wrapping_add(inbound.next_key() as u8);
    let mut reply = vec![2];
    reply.extend_from_slice(&DETAILS);
    reply.extend_from_slice(&[wire_opcode, 0, 1, 9]);
    server.send(&reply);
    let e = entered(server.cycle());
    assert_eq!(e.opcode, 162);
    assert_eq!(e.header[0], wire_opcode);
    assert_eq!(e.outbound_opcodes, 1);
    assert_eq!(e.isaac_seed, seed);
    assert_eq!(e.outbound_cipher(profile).encode(93), outbound.encode(93));
    let next = 234u8.wrapping_add(inbound.next_key() as u8);
    let mut decoder = e.decoder(profile, 1024, 1024);
    decoder.push(&[next, 100]).unwrap();
    assert_eq!(decoder.next_frame().unwrap().unwrap().opcode, 234);
    // The replayed session agrees.
    let (mut session, events) = e.session(b"player", profile, 765, 503).unwrap();
    assert_eq!(events.len(), 2);
    let mut inbound_again =
        OpcodeCipher::new(&seed.map(|w| w.wrapping_add(50) as u32), true).unwrap();
    inbound_again.next_key();
    let next = 234u8.wrapping_add(inbound_again.next_key() as u8);
    let frames = session.feed(&[next, 100]).unwrap();
    assert_eq!(
        frames,
        [rs09_client::session::Event::Frame(
            rs09_protocol::game::Frame {
                opcode: 234,
                payload: vec![100]
            }
        )]
    );
}

#[test]
fn the_password_is_kept_for_reconnecting_until_forgotten() {
    let mut server = Server::title();
    entered(server.enter(1, &success(&[])));
    server.flow.set_game_state(30);
    server.flow.connection_lost(0, None);
    server.cycle();
    server.accept();
    server.send(&[0, 0, 0, 0, 0, 0, 0, 0, 9]);
    server.cycle();
    let block = server.writes().last().unwrap().clone();
    let info = login::decode(&block, &RsaKey::reference_private(), |_| false)
        .info
        .unwrap();
    assert_eq!(info.opcode, 18);
    assert_eq!(
        (info.username.as_str(), info.password.as_str()),
        ("player", "password")
    );
    server.flow.forget_password();
    server.flow.clear();
    server.cycle();
    server.accept();
    server.send(&[0, 0, 0, 0, 0, 0, 0, 0, 9]);
    server.cycle();
    let block = server.writes().last().unwrap().clone();
    let info = login::decode(&block, &RsaKey::reference_private(), |_| false)
        .info
        .unwrap();
    assert_eq!(info.password, "");
}

// ----- small ports -----

#[test]
fn uid_file_cases() {
    assert_eq!(uid_bytes(None), [0; 24]);
    assert_eq!(uid_bytes(Some(&[])), [0xFF; 24]);
    assert_eq!(uid_bytes(Some(&[1; 23])), [0xFF; 24]);
    assert_eq!(uid_bytes(Some(&[0; 24])), [0xFF; 24]);
    let mut file = [0u8; 30];
    file[23] = 9;
    file[24] = 7;
    let mut expected = [0u8; 24];
    expected[23] = 9;
    assert_eq!(uid_bytes(Some(&file)), expected);
}

#[test]
fn preferences_pack_like_to_int() {
    let mut p = rs09_client::ui::vm::Preferences::default();
    // Defaults: brightness 3, every detail on, scenery shadows 2, all three
    // volumes non-zero, particles 2.
    let expected = 3
        | 1 << 3
        | 1 << 4
        | 1 << 5
        | 1 << 6
        | 1 << 7
        | 1 << 8
        | 1 << 9
        | 1 << 10
        | 2 << 11
        | 1 << 13
        | 1 << 15
        | 1 << 16
        | 1 << 19
        | 1 << 20
        | 1 << 21
        | 1 << 22
        | 2 << 23;
    assert_eq!(preferences_to_int(&p), expected);
    p.brightness = 4;
    p.stereo = false;
    p.music_volume = 0;
    p.scenery_shadows = 0;
    p.particles = 0;
    p.fog = false;
    assert_eq!(
        preferences_to_int(&p),
        (expected & !3 & !(1 << 19) & !(1 << 21) & !(3 << 11) & !(3 << 23) & !(1 << 16)) | 4
    );
}

#[test]
fn loading_and_connection_lost_text() {
    assert_eq!(status_text(10, None), None);
    assert_eq!(status_text(30, None), None);
    assert_eq!(
        status_text(25, None).as_deref(),
        Some("Loading - please wait.")
    );
    assert_eq!(
        status_text(28, Some(37)).as_deref(),
        Some("Loading - please wait.<br>(37%)")
    );
    assert_eq!(
        status_text(40, None).as_deref(),
        Some("Connection lost.<br>Please wait - attempting to reestablish.")
    );
    let mut progress = LoadingProgress::default();
    assert_eq!(progress.percent(0, 5, 5), None);
    assert_eq!(progress.percent(1, 8, 0), Some(0));
    assert_eq!(progress.percent(1, 6, 0), Some(12));
    assert_eq!(progress.percent(1, 0, 0), Some(50));
    assert_eq!(progress.percent(2, 0, 4), Some(50));
    assert_eq!(progress.percent(2, 0, 1), Some(87));
    assert_eq!(progress.percent(2, 0, 9), Some(50));
    assert_eq!(
        progress,
        LoadingProgress {
            peak_map_files: 8,
            peak_loc_models: 9
        }
    );
}

#[test]
fn standalone_endpoints() {
    let e = Endpoints::standalone("example.org", None);
    assert_eq!(
        (
            e.hostname.as_str(),
            e.port,
            e.default_port,
            e.alternate_port
        ),
        ("example.org", 43595, 43595, 43594)
    );
    assert_eq!(
        (
            e.world_list_port,
            e.world_list_default_port,
            e.world_list_alternate_port
        ),
        (43595, 43595, 43594)
    );
    assert_eq!((e.mode_where, e.world_list_id, e.world_id), (2, 1, 1));
    let live = Endpoints::new("example.org", 0, 7, None, false);
    assert_eq!(
        (live.port, live.alternate_port, live.world_id),
        (43595, 43594, -1)
    );
    let json = Endpoints::standalone(
        "example.org",
        Some(JsonConfig {
            ip_management: "m".into(),
            ip_address: "a".into(),
            world: 3,
            server_port: 5000,
            wl_port: 6000,
        }),
    );
    assert_eq!(
        (
            json.hostname.as_str(),
            json.port,
            json.alternate_port,
            json.world_id
        ),
        ("a", 5000, 5003, 3)
    );
}

/// The object-safe view the script VM uses reaches the same state.
#[test]
fn the_vm_interface_reaches_the_flow() {
    use rs09_client::login_flow::Commands;
    let mut server = Server::title();
    {
        let vm: &mut dyn Commands = &mut server.flow;
        assert_eq!(
            (vm.op5607(), vm.op5608(), vm.op5609(), vm.op5611()),
            (-2, 0, -2, -1)
        );
        assert_eq!(vm.world_id(), 1);
        assert_eq!(vm.op5610(), Err(ScriptError::NoSuggestions));
        assert_eq!(vm.op6500(100_000), 0);
    }
    server.accept();
    let payload = [
        &[1, 1, 1, 5, 0][..],
        b"Z\0",
        &[1, 2, 2, 0, 0, 0, 0, 0, 1, 0],
        b"one\0",
        &[0],
        b"h1\0",
        &[1, 0, 0, 0, 0, 0, 0],
        b"two\0",
        &[0],
        b"h2\0",
        &[0, 0, 0, 1, 0, 0, 9, 1, 0, 4],
    ]
    .concat();
    let mut reply = vec![0];
    reply.extend_from_slice(&(payload.len() as u16).to_be_bytes());
    reply.extend_from_slice(&payload);
    server.send(&reply);
    let vm: &mut dyn Commands = &mut server.flow;
    assert_eq!(vm.op6500(100_000), 1);
    assert_eq!(vm.op6501().map(|w| (w.id, w.players)), Some((1, 9)));
    assert_eq!(vm.op6502().map(|w| (w.id, w.players)), Some((2, 4)));
    assert_eq!(vm.op6502(), None);
    // Players ascending.
    vm.op6507([1, 0, -1, 0]).unwrap();
    assert_eq!(vm.op6501().map(|w| w.id), Some(2));
    let two = vm.op6506(2).unwrap();
    assert_eq!(
        (
            two.flags,
            &two.activity[..],
            two.country_flag,
            &two.country_name[..]
        ),
        (0, &b"two"[..], 5, &b"Z"[..])
    );
    assert_eq!(vm.op6506(3), None);
    assert_eq!(vm.op6503(2), 1);
    assert_eq!(vm.world_id(), 2);
    assert_eq!(vm.op6503(7), 0);
    vm.op5603([1, 2, 1990, 3]);
    assert_eq!(vm.op5609(), -3);
    vm.op5606();
    assert_eq!(vm.op5609(), -3, "a request in progress keeps its reply");
    vm.op5600(b"name", b"password", 0).unwrap();
    assert_eq!(vm.op5607(), -2, "ignored while the date check runs");
    assert_eq!(vm.username_input(), b"");
}
