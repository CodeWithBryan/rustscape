//! Account creation end to end on the client side: the title interface
//! (744) of the pinned cache and its own scripts, run by the client's VM,
//! with the client's login state machine ([`LoginFlow`] over real loopback
//! sockets) behind the script commands, against a scripted peer that reads
//! the requests as the reference server does (`tests/support/account_peer.rs`).
//!
//! What the tests establish:
//! - the bytes of the 147, 186 and 36 requests, read back in
//!   `AccountRegister.read` order, carry what was chosen and typed on the
//!   screens (day, month, year, country, name, password, revision 530);
//! - for every reply code each of the three answer scripts (1234, 1236, 1238)
//!   tells apart, the screen shows that script's message, read from the
//!   component the script writes; replies -1 (closed connection) and -4 (no
//!   answer, then the retry fails too) come from the socket, as in the
//!   original; 21 with zero to five suggested names shows each list;
//! - success moves through the panels (date of birth, name, password,
//!   created), and the created screen's button opens the login form with the
//!   new name, from which a login goes out with that name and password and
//!   enters the game.
//!
//! The expected messages are the strings of the cache's scripts, so a test
//! fails if the client shows another; the panel order and which reply goes
//! where are read off the scripts (disassembled by hand, a transcription,
//! not a JVM run). Reply -5 (2000 cycles without a connection) needs a
//! connect that never completes and is not produced here; the JVM trace of
//! `tests/login_flow.rs` covers it at the state-machine level.
#![cfg(not(target_arch = "wasm32"))]
#[path = "support/account_peer.rs"]
mod account_peer;

use account_peer::{Answer, Config, Registry, Request};
use rs09_client::{
    display::{Display, Mode, System},
    login_flow::{
        Endpoints, Environment, Event, JsonConfig, LoginFlow, TransportDialer, WorldListStartup,
    },
    ui::{
        HookArg,
        state::{DrawNode, Key, PointerInput, State},
        vm::Vm,
    },
};
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    net::TcpListener,
    path::Path,
    rc::Rc,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

const TITLE: i32 = 744;
/// Panels (`InterfaceList` components of 744) the create scripts switch.
const MAIN_MENU: i32 = 226;
const LOGIN: i32 = 241;
const INTRO: i32 = 403;
const BIRTH: i32 = 404;
const NAME: i32 = 405;
const NAME_FREE: i32 = 406;
const PASSWORD: i32 = 409;
const CREATED: i32 = 410;
/// The message lines the answer scripts write.
const BIRTH_MESSAGE: i32 = 361;
const NAME_MESSAGE: i32 = 385;
const NAME_HELP: i32 = 386;
const FREE_MESSAGE: i32 = 395;
const PASSWORD_MESSAGE: i32 = 413;
/// The login form's username field.
const LOGIN_USERNAME: i32 = 340;

type Queue = Arc<Mutex<VecDeque<Answer>>>;

/// The title interface with the login flow behind it.
struct Title {
    state: State,
    vm: Vm,
    nodes: Vec<DrawNode>,
    point: [i32; 2],
    down: bool,
    flow: Rc<RefCell<LoginFlow<TransportDialer>>>,
    events: Vec<String>,
    answers: Queue,
    seen: std::sync::mpsc::Receiver<Request>,
    /// When set, every `Math.random()` of the flow returns this.
    random: Rc<Cell<Option<f64>>>,
    /// Script errors are expected (they fail the cycle otherwise).
    errors_allowed: bool,
    _hole: TcpListener,
}

fn cache() -> Option<std::path::PathBuf> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/cache");
    if path.join("main_file_cache.dat2").exists() {
        Some(path)
    } else {
        eprintln!("assets/cache absent: account creation verified nothing");
        None
    }
}

/// A peer whose answers come from `answers`, falling back to the
/// reference server's rules when the queue is empty. It listens on port
/// P; the listener returned with it holds P - 1, the configuration's default
/// port, where connections are accepted by the system and never answered
/// (so a retry there is not another test's peer).
fn peer() -> (account_peer::Peer, Queue, TcpListener) {
    let answers: Queue = Arc::default();
    let queue = Arc::clone(&answers);
    let (hole, listener) = (0..100)
        .find_map(|_| {
            let hole = TcpListener::bind("127.0.0.1:0").ok()?;
            let next = hole.local_addr().ok()?.port().checked_add(1)?;
            Some((hole, TcpListener::bind(("127.0.0.1", next)).ok()?))
        })
        .expect("two neighbouring loopback ports");
    let peer = account_peer::start(
        listener,
        Config {
            forward: None,
            answer: Box::new(move |request, _| match request {
                Request::WorldList(_) => Answer::Reference,
                _ => queue
                    .lock()
                    .unwrap()
                    .pop_front()
                    .unwrap_or(Answer::Reference),
            }),
            registry: Registry::default(),
        },
    );
    (peer, answers, hole)
}

fn flow(port: u16, fixed: Rc<Cell<Option<f64>>>) -> Rc<RefCell<LoginFlow<TransportDialer>>> {
    // One address for the world list and world 1, as config.json gives it.
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
    let mut endpoints = Endpoints::standalone("unused.invalid", Some(json));
    // As the client's lifecycle host does: the original's JS5 connection
    // has set the game host and port before the title shows.
    endpoints.js5_connect();
    let mut seed = 0.0;
    Rc::new(RefCell::new(LoginFlow::new(
        dialer,
        endpoints,
        Box::new(move || {
            seed += 0.001;
            fixed.get().unwrap_or(seed)
        }),
    )))
}

impl Title {
    fn new(cache: &Path) -> Self {
        let (peer, answers, hole) = peer();
        let random = Rc::new(Cell::new(None));
        let flow = flow(peer.address.port(), Rc::clone(&random));
        let started = Instant::now();
        loop {
            let now = 10_000 + started.elapsed().as_millis() as i64;
            match flow.borrow_mut().startup_world_list(now) {
                WorldListStartup::Loaded => break,
                WorldListStartup::Loading => {}
                other => panic!("world list: {other:?}"),
            }
            assert!(started.elapsed() < Duration::from_secs(10));
            thread::sleep(Duration::from_millis(1));
        }
        flow.borrow_mut().set_game_state(10);
        let pack = rs09_client::cache_pack::cache_pack(cache, "account").unwrap();
        let mut vm = Vm::default();
        vm.display = Display::new(
            [1024, 768],
            System::client(vec![Mode {
                width: 1024,
                height: 768,
                bit_depth: 32,
                refresh_rate: 60,
            }]),
        );
        vm.cycle = 100;
        // 2026-11-17: days since launch well past any birth year chosen here.
        vm.now_ms = 1_794_960_000_000;
        let remembered = vm.preferences.remembered_mode;
        vm.display
            .set_window_mode(false, remembered, -1, -1, &mut vm.preferences);
        vm.window_mode = vm.display.window_mode();
        vm.display.effects.clear();
        vm.login_host = Some(Rc::clone(&flow) as _);
        // What command 3318 reads (`Player.worldId`), as the lifecycle host
        // copies it each frame.
        vm.world = flow.borrow().endpoints.world_id;
        let mut state = State::new(pack);
        state.open_root(TITLE).unwrap();
        vm.drain(&mut state);
        let mut title = Self {
            state,
            vm,
            nodes: Vec::new(),
            point: [-1, -1],
            down: false,
            flow,
            events: Vec::new(),
            answers,
            seen: peer.seen,
            random,
            errors_allowed: false,
            _hole: hole,
        };
        title.layout();
        for _ in 0..5 {
            title.cycle(false, &[]);
        }
        // The world-list request of the start-up.
        assert_eq!(title.seen(), Request::WorldList(0));
        title
    }

    fn layout(&mut self) {
        self.nodes = self.state.layout(765, 503).unwrap();
    }

    /// One 20 ms cycle: the interface's hooks and input (`mainUpdate`), then
    /// `CreateManager.loop` and the login loops (`client.mainLoop`, state 10).
    fn cycle(&mut self, clicked: bool, keys: &[[i32; 2]]) {
        let (state, vm) = (&mut self.state, &mut self.vm);
        vm.cycle += 1;
        vm.now_ms += 20;
        vm.poll_updates(state, vm.now_ms).unwrap();
        state.key_events.clear();
        state.key_events.extend_from_slice(keys);
        let pointer = PointerInput {
            point: self.point,
            clicked,
            down: self.down,
            any_down: self.down,
            scroll: 0,
            suppressed: false,
            click: None,
        };
        state.queue_cycle_events(Some(pointer), None);
        vm.drain(state);
        state.finish_input_events();
        assert!(
            self.errors_allowed || state.diagnostics.is_empty(),
            "script errors: {:?}",
            state.diagnostics
        );
        self.layout();
        let events = self.flow.borrow_mut().cycle(&Environment::default());
        for event in events {
            self.events.push(match event {
                Event::PreferencesSent => "preferences".into(),
                Event::Entered(entered) => format!("entered {}", entered.change.is_some()),
                Event::Reconnected(_) => "reconnected".into(),
                Event::LoggedOut { reply, .. } => format!("logged out {reply}"),
            });
        }
    }

    fn node(&self, component: i32, child: i32) -> DrawNode {
        let key = Key {
            id: TITLE << 16 | component,
            child,
        };
        *self
            .nodes
            .iter()
            .find(|node| node.key == key)
            .unwrap_or_else(|| panic!("{component}:{child} is not laid out"))
    }

    fn click_at(&mut self, point: [i32; 2]) {
        self.point = point;
        self.cycle(false, &[]);
        self.down = true;
        self.cycle(true, &[]);
        self.cycle(false, &[]);
        self.down = false;
        self.cycle(false, &[]);
        self.cycle(false, &[]);
    }

    /// Press and release on the middle of a component's visible part.
    fn click(&mut self, component: i32, child: i32) {
        let clip = self.node(component, child).clip;
        self.click_at([clip.x + clip.w / 2, clip.y + clip.h / 2]);
    }

    /// The laid-out component whose click runs `script` first.
    fn click_script(&mut self, script: i32) {
        let node = *self
            .nodes
            .iter()
            .find(|node| {
                let c = &self.state.components[&node.key];
                !c.hidden
                    && c.on_click_repeat.as_ref().and_then(|a| a.first())
                        == Some(&HookArg::Int(script))
            })
            .unwrap_or_else(|| {
                panic!("no visible button runs script {script}: {}", self.buttons())
            });
        let clip = node.clip;
        self.click_at([clip.x + clip.w / 2, clip.y + clip.h / 2]);
    }

    /// Typed characters (`Keyboard.keyTyped`: code -1).
    fn type_text(&mut self, text: &str) {
        for character in text.chars() {
            self.cycle(false, &[[-1, character as i32]]);
        }
    }

    /// A pressed key (`Keyboard.keyPressed`: char -1); 84 Enter, 80 Tab.
    fn press_key(&mut self, code: i32) {
        self.cycle(false, &[[code, -1]]);
    }

    /// The laid-out components with a click hook, for failure messages.
    fn buttons(&self) -> String {
        self.nodes
            .iter()
            .filter_map(|node| {
                let c = &self.state.components[&node.key];
                let hook = c.on_click_repeat.as_ref()?;
                (!c.hidden).then(|| {
                    format!(
                        "{}:{} {:?} {:?}",
                        node.key.id & 0xFFFF,
                        node.key.child,
                        c.text,
                        &hook[..hook.len().min(2)]
                    )
                })
            })
            .collect::<Vec<_>>()
            .join("; ")
    }

    fn text(&self, component: i32) -> String {
        self.state.components[&Key::component(TITLE << 16 | component)]
            .text
            .clone()
    }

    fn shown(&self, component: i32) -> bool {
        !self.state.components[&Key::component(TITLE << 16 | component)].hidden
    }

    /// Cycles (with real time for the sockets) until no request is under
    /// way, then a few more for the polling hook to read the reply.
    /// Returns `CreateManager.reply` as it was when the request ended,
    /// before the screen's polling hook read it (on success the next panel's
    /// opening resets it to -2 at once).
    fn settle(&mut self) -> i32 {
        let started = Instant::now();
        while self.flow.borrow().create.step != 0 || self.flow.borrow().login.step != 0 {
            self.cycle(false, &[]);
            thread::sleep(Duration::from_millis(1));
            assert!(started.elapsed() < Duration::from_secs(30), "no reply");
        }
        let reply = self.reply();
        for _ in 0..3 {
            self.cycle(false, &[]);
        }
        reply
    }

    fn answer(&mut self, answer: Answer) {
        self.answers.lock().unwrap().push_back(answer);
    }

    fn seen(&self) -> Request {
        self.seen
            .recv_timeout(Duration::from_secs(10))
            .expect("a request")
    }

    fn reply(&self) -> i32 {
        self.flow.borrow().create.reply
    }
}

/// Clicks `button` with `answer` queued; returns the request and checks
/// the reply code reached the flow.
#[track_caller]
fn ask(title: &mut Title, button: impl Fn(&mut Title), answer: Answer, reply: i32) -> Request {
    title.answer(answer);
    button(title);
    assert_eq!(title.settle(), reply);
    title.seen()
}

fn join(lines: &[&str]) -> String {
    lines.join("<br>")
}

/// The DOB page's refusals, as script 1234 words them.
fn birth_messages() -> Vec<(Answer, i32, String)> {
    let code = |c: u8| (Answer::Code(c), i32::from(c));
    vec![
        (
            code(3).0,
            3,
            join(&[
                "There was an error contacting the create system.",
                "Please try again.",
            ]),
        ),
        (
            code(7).0,
            7,
            join(&["The server is very busy.", "Please try again in a minute."]),
        ),
        (
            code(9).0,
            9,
            join(&[
                "You cannot create an account at this time.",
                "Please try again later.",
            ]),
        ),
        (
            code(10).0,
            10,
            join(&["The date of birth you entered", "was invalid!"]),
        ),
        (
            code(11).0,
            11,
            join(&["The date of birth you entered", "is in the future!"]),
        ),
        (
            code(12).0,
            12,
            join(&["The date of birth you entered", "was this year!"]),
        ),
        (
            code(13).0,
            13,
            join(&["The date of birth you entered", "was last year!"]),
        ),
        (
            code(14).0,
            14,
            "The country you entered was invalid!".into(),
        ),
        (
            code(20).0,
            20,
            join(&["Unexpected server response.", "Please try again."]),
        ),
        (
            Answer::Close,
            -1,
            join(&["Unexpected server response.", "Please try again."]),
        ),
    ]
}

fn unexpected() -> String {
    join(&["Unexpected server response.", "Please try again."])
}

fn contact_failed() -> String {
    join(&[
        "There was an error contacting the server.",
        "Please try using a different world.",
    ])
}

#[test]
fn every_screen_and_every_reply_of_account_creation() {
    let Some(cache) = cache() else { return };
    let mut t = Title::new(&cache);
    assert!(t.shown(MAIN_MENU));

    // Main menu -> the introduction -> the date of birth.
    t.click_script_arg(1173, 48759187);
    assert!(t.shown(INTRO) && !t.shown(MAIN_MENU));
    t.click(351, -1);
    assert!(t.shown(BIRTH));
    assert_eq!(t.text(BIRTH_MESSAGE), "Please input your date of birth:");

    // Each dropdown opened by its box and its third entry picked: day 3,
    // March, 2007 (the years count down from 2009), Algeria.
    for (dropdown, list, text) in [
        (381, 383, "3"),
        (377, 379, "March"),
        (373, 375, "2007"),
        (369, 371, "Algeria"),
    ] {
        t.click(dropdown, 3);
        assert_eq!(
            t.state.components[&Key {
                id: TITLE << 16 | list,
                child: 2
            }]
                .text,
            text
        );
        t.click(list, 2);
        assert_eq!(
            t.state.components[&Key {
                id: TITLE << 16 | dropdown,
                child: 3
            }]
                .text,
            text
        );
    }
    let varc = |id| t.vm.varcs.get(&id).copied().unwrap_or(0);
    let chosen = [varc(179), varc(180), varc(181), varc(182)];
    assert_eq!(chosen[..3], [3, 2, 2007], "day, month (0-based), year");

    // Every refusal of the date check, then success.
    let next = |t: &mut Title| t.click(365, -1);
    for (answer, reply, message) in birth_messages() {
        let request = ask(&mut t, next, answer, reply);
        assert_eq!(
            request,
            Request::Info {
                day: 3,
                month: 2,
                year: 2007,
                country: chosen[3],
            }
        );
        assert_eq!(t.text(BIRTH_MESSAGE), message, "reply {reply}");
        assert!(t.shown(BIRTH));
    }
    // No answer at all: the read times out (1500 cycles, the model of the
    // 30 s socket timeout), the retry on the default port is not answered
    // either, and the flow gives up with -4.
    t.answer(Answer::Silent);
    next(&mut t);
    assert_eq!(t.settle(), -4);
    t.seen();
    assert_eq!(t.text(BIRTH_MESSAGE), contact_failed());
    ask(&mut t, next, Answer::Code(2), 2);
    assert!(t.shown(NAME) && !t.shown(BIRTH));

    // The name: typed, then checked.
    t.type_text("new_hero");
    assert!(t.text(394).starts_with("new_hero"));
    let check = |t: &mut Title| t.click_script(1235);
    let name_refusals: Vec<(Answer, i32, String)> = vec![
        (
            Answer::Code(3),
            3,
            join(&[
                "There was an error contacting the create system.",
                "Please try again.",
            ]),
        ),
        (
            Answer::Code(7),
            7,
            join(&["The server is very busy.", "Please try again in a minute."]),
        ),
        (
            Answer::Code(9),
            9,
            "You cannot create an account at this time. Please try again later.".into(),
        ),
        (
            Answer::Code(20),
            20,
            join(&["That username is not available.", "Please choose again."]),
        ),
        (
            Answer::Code(22),
            22,
            join(&["That is not a valid username.", "Please choose again."]),
        ),
        (Answer::Code(10), 10, unexpected()),
        (Answer::Close, -1, unexpected()),
    ];
    for (answer, reply, message) in name_refusals {
        let request = ask(&mut t, check, answer, reply);
        assert_eq!(request, Request::Name("new_hero".into()));
        assert_eq!(t.text(NAME_MESSAGE), message, "reply {reply}");
        assert!(t.shown(NAME));
    }
    // 21: taken, with zero to five names offered instead.
    let offered = ["hero_one", "hero_two", "hero3", "hero_4", "hero5"];
    for count in 0..=5 {
        let names: Vec<String> = offered[..count].iter().map(|n| n.to_string()).collect();
        ask(&mut t, check, Answer::Suggest(names), 21);
        assert_eq!(
            t.text(NAME_MESSAGE),
            join(&["That username is not available.", "Please choose again."])
        );
        let title_case = ["Hero One", "Hero Two", "Hero3", "Hero 4", "Hero5"];
        let expected = match count {
            0 => "We could not find any suitable alternative account names to suggest, please choose again.".to_owned(),
            1 => format!(
                "Alternatively, the following username is available:<br><br>{}",
                title_case[0]
            ),
            n => format!(
                "Alternatively, the following usernames are available:<br><br>{}",
                title_case[..n].join("<br>")
            ),
        };
        assert_eq!(t.text(NAME_HELP), expected, "{count} names");
        // 5610 discarded the list.
        assert!(t.flow.borrow().create.suggested_names.is_none());
    }
    ask(&mut t, check, Answer::Code(2), 2);
    assert!(t.shown(NAME_FREE) && !t.shown(NAME));
    assert_eq!(
        t.text(FREE_MESSAGE),
        "Congratulations! The username <col=8c320a>new_hero</col> is currently available."
    );

    // The password page: a mismatch is caught by the script, no request.
    t.click_script_arg(1173, 48759193);
    assert!(t.shown(PASSWORD));
    t.type_text("hunter22");
    t.press_key(80);
    t.type_text("hunter23");
    let create = |t: &mut Title| t.click_script(1237);
    create(&mut t);
    assert_eq!(
        t.text(PASSWORD_MESSAGE),
        "The values in the `Password' and `Confirm Password' boxes are not the same."
    );
    assert_eq!(t.flow.borrow().create.step, 0);
    assert!(t.seen.try_recv().is_err());
    t.press_key(85);
    t.type_text("2");

    let create_refusals: Vec<(u8, String)> = vec![
        (
            3,
            join(&[
                "There was an error contacting the create system.",
                "Please try again.",
            ]),
        ),
        (
            7,
            join(&["The server is very busy.", "Please try again in a minute."]),
        ),
        (
            9,
            join(&[
                "You cannot create an account now.",
                "Please try again later.",
            ]),
        ),
        (
            10,
            join(&[
                "The date of birth you entered was invalid.",
                "Please go back and try again.",
            ]),
        ),
        (
            20,
            join(&[
                "That username is no longer available.",
                "Please go back and choose again.",
            ]),
        ),
        (
            30,
            join(&["Your password was an invalid length.", "Please try again."]),
        ),
        (
            31,
            join(&[
                "Your password contained invalid characters.",
                "Please try again.",
            ]),
        ),
        (
            32,
            join(&["Your password is too easy to guess.", "Please try again."]),
        ),
        (
            33,
            join(&["Your password is too easy to guess.", "Please try again."]),
        ),
        (
            34,
            join(&[
                "Your password is too similar to your username.",
                "Please try again.",
            ]),
        ),
        (
            35,
            join(&[
                "Your password is too similar to your username.",
                "Please try again.",
            ]),
        ),
        (
            36,
            join(&[
                "Your password is too similar to your username.",
                "Please try again.",
            ]),
        ),
        (
            37,
            join(&[
                "RuneScape has been updated!",
                "Please reload this page and try again.",
            ]),
        ),
        (
            38,
            join(&[
                "You cannot create an account at this time.",
                "Please try again later.",
            ]),
        ),
        (22, unexpected()),
    ];
    let expected_create = Request::Create {
        header: 10,
        revision: 530,
        name: "new_hero".into(),
        password: "hunter22".into(),
        affiliate: 0,
        day: 3,
        month: 2,
        year: 2007,
        country: chosen[3],
    };
    for (code, message) in create_refusals {
        let request = ask(&mut t, create, Answer::Code(code), i32::from(code));
        assert_eq!(request, expected_create);
        assert_eq!(t.text(PASSWORD_MESSAGE), message, "reply {code}");
        assert!(t.shown(PASSWORD));
    }
    ask(&mut t, create, Answer::Close, -1);
    assert_eq!(t.text(PASSWORD_MESSAGE), unexpected());

    // Created: the reference server's rules accept it.
    let request = ask(&mut t, create, Answer::Reference, 2);
    assert_eq!(request, expected_create);
    assert!(t.shown(CREATED) && !t.shown(PASSWORD));

    // The created screen's button opens the login form with the new name;
    // the password is typed and Enter logs in.
    t.click_script(1123);
    assert!(t.shown(LOGIN) && !t.shown(CREATED));
    assert!(t.text(LOGIN_USERNAME).starts_with("new_hero"));
    t.type_text("hunter22");
    t.answer(Answer::Code(2));
    t.press_key(84);
    let started = Instant::now();
    while !t.events.iter().any(|e| e.starts_with("entered")) {
        t.cycle(false, &[]);
        thread::sleep(Duration::from_millis(1));
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "{:?} login step {} reply {} user {:?} pass {:?} varc174 {:?} seen {:?} world {} {:?} 547 {:?}",
            t.events,
            t.flow.borrow().login.step,
            t.flow.borrow().login.reply,
            t.text(LOGIN_USERNAME),
            t.text(346),
            t.vm.varcs.get(&174),
            t.seen.try_recv(),
            rs09_client::login_flow::Commands::world_id(&*t.flow.borrow()),
            rs09_client::login_flow::Commands::op6506(&*t.flow.borrow(), 1).map(|e| e.flags),
            t.vm.varcs.get(&547),
        );
    }
    assert_eq!(
        t.seen(),
        Request::Login {
            opcode: 16,
            username: "new_hero".into(),
            password: "hunter22".into(),
        }
    );
}

impl Title {
    /// The button whose click runs `script` with `argument` first.
    fn click_script_arg(&mut self, script: i32, argument: i32) {
        let node = *self
            .nodes
            .iter()
            .find(|node| {
                let c = &self.state.components[&node.key];
                !c.hidden
                    && c.on_click_repeat.as_deref().is_some_and(|a| {
                        a.len() >= 2
                            && a[0] == HookArg::Int(script)
                            && a[1] == HookArg::Int(argument)
                    })
            })
            .unwrap_or_else(|| {
                panic!(
                    "no visible button runs {script}({argument}): {}",
                    self.buttons()
                )
            });
        let clip = node.clip;
        self.click_at([clip.x + clip.w / 2, clip.y + clip.h / 2]);
    }
}

/// Main menu to the password page with the default date of birth and
/// country, the name accepted.
fn to_password_page(t: &mut Title, name: &str) {
    t.click_script_arg(1173, 48759187);
    t.click(351, -1);
    ask(t, |t| t.click(365, -1), Answer::Code(2), 2);
    t.type_text(name);
    ask(t, |t| t.click_script(1235), Answer::Code(2), 2);
    t.click_script_arg(1173, 48759193);
    assert!(t.shown(PASSWORD));
}

/// `CreateManager.createAccount` builds its block in a 129-byte buffer, and
/// `Buffer.rsaenc` writes the length byte and `BigInteger.toByteArray()`
/// back into it. When the encrypted value has its top bit set (the modulus
/// is just over 2^1023, so for about 7% of the random padding values) that
/// is 1 + 129 bytes: the original throws, script 1237 stops after it has
/// shown "Please wait..." and before it installs the reply hook, so the page
/// stays waiting with no request sent; Escape (script 1221) returns to the
/// main menu. The port reproduces the throw as a script error.
#[test]
fn a_create_block_too_long_for_its_buffer_leaves_the_page_waiting() {
    let Some(cache) = cache() else { return };
    let mut t = Title::new(&cache);
    to_password_page(&mut t, "tester");
    t.type_text("secret1");
    t.press_key(80);
    t.type_text("secret1");
    // A padding value for which this block overflows, found the way the
    // flow would meet it.
    let varc = |id| t.vm.varcs.get(&id).copied().unwrap_or(0);
    let (day, month, year, country) = (varc(179), varc(180), varc(181), varc(182));
    let overflowing = (1..1000)
        .map(|n| f64::from(n) / 1000.0)
        .find(|&value| {
            let mut probe = LoginFlow::new(
                TransportDialer {
                    endpoint: Box::new(|host, port| format!("{host}:{port}")),
                },
                Endpoints::standalone("unused.invalid", None),
                Box::new(move || value),
            );
            probe
                .create_account(
                    day,
                    country,
                    month,
                    b"secret1",
                    rs09_protocol::login::encode_name(b"tester"),
                    year,
                    0,
                )
                .is_err()
        })
        .expect("a padding value that overflows");
    t.random.set(Some(overflowing));
    t.errors_allowed = true;
    t.click_script(1237);
    for _ in 0..20 {
        t.cycle(false, &[]);
    }
    assert!(
        t.state
            .diagnostics
            .iter()
            .any(|d| d.contains("5605") && d.contains("does not fit")),
        "{:?}",
        t.state.diagnostics
    );
    assert_eq!(t.flow.borrow().create.step, 0);
    assert!(t.seen.try_recv().is_err(), "no request went out");
    assert_eq!(t.text(428), "Please wait...");
    assert!(!t.shown(427), "Previous stays hidden");
    // Script 1237 removed the button's own hook before the throw.
    assert!(!t.buttons().contains("Int(1237)"), "{}", t.buttons());
    t.press_key(13);
    assert!(t.shown(MAIN_MENU) && !t.shown(PASSWORD));
}

/// The peer's reference rules (`AccountRegister.read`) through the client's
/// own requests: what the reference server would answer to these screens.
#[test]
fn the_reference_server_rules_through_the_client_requests() {
    let Some(cache) = cache() else { return };
    let mut t = Title::new(&cache);
    // Everything from here on is answered by the reference rules.
    t.click_script_arg(1173, 48759187);
    t.click(351, -1);
    ask(&mut t, |t| t.click(365, -1), Answer::Reference, 2);
    // A name of digits only is valid; `encode37` turns other characters
    // into underscores, so the client cannot send an invalid name.
    t.type_text("abc-def");
    let request = ask(&mut t, |t| t.click_script(1235), Answer::Reference, 2);
    assert_eq!(request, Request::Name("abc_def".into()));
    t.click_script_arg(1173, 48759193);
    // Too short (30), the same as the name (33), then created (2).
    for (password, reply) in [("abcd", 30), ("abc_def", 33), ("abcde", 2)] {
        t.type_text(password);
        t.press_key(80);
        t.type_text(password);
        let request = ask(&mut t, |t| t.click_script(1237), Answer::Reference, reply);
        let Request::Create { password: sent, .. } = request else {
            panic!("a create request")
        };
        assert_eq!(sent, password);
        if reply != 2 {
            // Clear both fields (backspace) for the next attempt.
            for _ in 0..password.len() {
                t.press_key(85);
            }
            t.press_key(80);
            for _ in 0..password.len() {
                t.press_key(85);
            }
            t.press_key(80);
        }
    }
    assert!(t.shown(CREATED));
    // The same name again: the name check says 20, the creation 9.
    t.click_script(1123);
    t.press_key(13);
    t.click_script_arg(1173, 48759187);
    t.click(351, -1);
    ask(&mut t, |t| t.click(365, -1), Answer::Reference, 2);
    t.type_text("abc_def");
    ask(&mut t, |t| t.click_script(1235), Answer::Reference, 20);
    assert_eq!(
        t.text(NAME_MESSAGE),
        join(&["That username is not available.", "Please choose again."])
    );
}

/// The reference files the peer and the expectations above were transcribed
/// from (SHA-256 with `\r\n` read as `\n`). The interface scripts are pinned
/// by the cache itself.
#[test]
fn transcribed_reference_sources_are_the_pinned_ones() {
    use sha2::{Digest, Sha256};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (path, expected) in [
        (
            "reference/server/Server/src/main/core/net/registry/AccountRegister.java",
            "e9bd0c0380aabc9107822b8344c7ded3752f52f7bfba400f78680abb089a0170",
        ),
        (
            "reference/server/Server/src/main/core/net/registry/RegistryResponse.java",
            "8f4d00d9768207836d542a69ee3028914ceac38b72462e42cc6255d3486f1875",
        ),
        (
            "reference/server/Server/src/main/core/net/event/HSReadEvent.java",
            "7f7ab4a1c05806ea9c71bb2dcfe1de6520b1038adba48ff3b00c184fd387495f",
        ),
        (
            "reference/server/Server/src/main/core/cache/misc/buffer/ByteBufferUtils.kt",
            "299f5638ca2e1a2550ddfbaea137a5704a1a7c4a27018853cd8fbea598f9b745",
        ),
        (
            "reference/server/Server/src/main/core/net/packet/in/Login.kt",
            "fb591971c7f88cdaac3fbf394db1b797c9f6413b6dc03e288d5c12ef6c5c7aaf",
        ),
        (
            "reference/client/client/src/main/java/rt4/CreateManager.java",
            "8043ab2425813698603301ff4fd8f960b0863eb2424fa706e19b402ac109fedc",
        ),
    ] {
        let Ok(bytes) = std::fs::read(root.join(path)) else {
            eprintln!("{path} absent: not checked");
            continue;
        };
        let text = String::from_utf8(bytes).unwrap().replace("\r\n", "\n");
        assert_eq!(
            format!("{:x}", Sha256::digest(text.as_bytes())),
            expected,
            "{path} changed: review the account-creation transcription"
        );
    }
}
