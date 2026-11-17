//! Login into the world (world::login) against the world-trace login traces.
//!
//! The scripted input of each world-trace login scenario (new-character,
//! two-characters, returning-character) is driven through the native login:
//! the recorded login block is decoded by `rs09_protocol::login::decode`,
//! authenticated by the native development authenticator, prepared by
//! `player::prepare` and admitted by `world::login::admission::read` over the
//! native repository, social library and canonical player; the login pulse is
//! run by `world::login::pulse::run` at the tick the original ran it. Every
//! byte the original queued for the session during those ticks is compared,
//! in order, with what the native login wrote.
//!
//! The binding below is a test binding. Where the original reaches a system
//! with a native library (containers and their listeners, skills, Settings,
//! variables, the interface manager, interaction options, the rebuild writer,
//! the save file) it calls that library on the canonical player. Where it
//! reaches content or a system that is not ported, the stand-in replays the
//! packets the original queued from that call, identified by the call stacks
//! of fixtures/world-login (one origin per packet), advances the interface
//! packet counter by the counter-carrying packets among them and puts the
//! varps they carry into the canonical variable store (what setVarp does
//! besides sending). Every replayed segment is pinned below by name and size:
//! a gap that closes, or a new difference, fails the test.
use rs09_game_core::java_json::{self, Json};
use rs09_protocol::{
    buffer::Writer,
    game::{Direction, Frame, updates},
    login::{self as codec, AuthResponse, RsaKey},
};
use rs09_server::{
    auth::{
        Authenticator, Clock, Mode,
        admission::{self, Decision},
        password::Passwords,
        storage::InMemoryStorage,
    },
    config::Configuration,
    definitions::item::{ItemDefinition, Registry as Items, Shared},
    inventory::{
        self,
        container::{Container, ContainerType, Event, Factory, SortType},
        container_listener::Handle as ListenerHandle,
        equipment,
        item::Handle as ItemHandle,
        listener::Listener as InventoryListener,
        player_binding::Capture,
    },
    player::{
        self as players,
        attributes::AttributeValue,
        core_data::CoreData,
        details::{ClientInfo, PlayerDetails},
        owned::{
            Player,
            profile::{Environment, Fresh, Profile},
        },
        persisted::{
            host::{Remaining, World},
            music, quests,
        },
        save::{self, Level as SaveLevel, Parser},
        settings::{self, Settings},
        skills::{self, Effect, EntityKind, Skills},
        variables,
    },
    timers,
    world::{
        appearance,
        doors::Location,
        interaction_options, interfaces,
        login::{self, Constants, Level, Write},
        player_render, player_walk,
        player_walk::{Node, NodeHandle, State},
        prepared_masks::PreparedMasks,
        rebuild,
        repository::{self, LogLevel, Repository, SessionObject},
        social,
    },
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashMap},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicI64, AtomicUsize, Ordering},
    },
};

/// The tick driver against the same scenarios, on this binding.
#[path = "world_login/tick_driver.rs"]
mod tick_driver;

const TRACE: &str = include_str!("../../../fixtures/world-trace/trace.jsonl");
const TRACE_PROVENANCE: &str = include_str!("../../../fixtures/world-trace/provenance.json");
const ORIGINS: &str = include_str!("../../../fixtures/world-login/origins.jsonl");
const PROVENANCE: &str = include_str!("../../../fixtures/world-login/provenance.json");
const DEFAULT_CONF: &str = include_str!("../../../fixtures/config/default.conf");
const XTEAS: &str = include_str!("../../../fixtures/startup-loaders/xteas.json");
const INTERFACE_CONFIGS: &str = include_str!("../../../fixtures/interfaces/interface_configs.json");
const CANONICAL: &str = include_str!("../../../fixtures/canonical-player/oracle.jsonl");

/// Opcodes whose writers take `InterfaceManager.getPacketCount(1)`.
const COUNTER_OPCODES: [u8; 16] = [
    145, 155, 149, 21, 171, 165, 115, 36, 130, 73, 66, 132, 50, 119, 65, 69,
];
/// Varbit 357 (the spell book) as the pinned cache defines it: varp 439,
/// bits 0..1. Checked against the cache by `pinned_varbit_matches_the_cache`
/// when the cache is present.
const SPELL_BOOK_VARBIT: (i32, i32, i32, i32) = (439, 357, 0, 1);

thread_local! {
    /// The clock of the scenario the test thread drives: tests run in
    /// parallel and each drives its own scenarios.
    static NOW: AtomicI64 = const { AtomicI64::new(0) };
}
fn now() -> i64 {
    NOW.with(|n| n.load(Ordering::SeqCst))
}
static RUN: AtomicUsize = AtomicUsize::new(0);

fn sha(data: &[u8]) -> String {
    format!("{:x}", Sha256::digest(data))
}
fn unhex(hex: &str) -> Vec<u8> {
    (0..hex.len() / 2)
        .map(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap())
        .collect()
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

// ------------------------------------------------------------------ fixture

/// One entry the original handed to a session, with the stack that queued it.
#[derive(Clone, Debug)]
struct Original {
    hex: String,
    op: Option<u8>,
    origin: Option<Vec<String>>,
}
impl Original {
    fn has(&self, marker: &str) -> bool {
        self.origin
            .as_ref()
            .is_some_and(|frames| frames.iter().any(|f| f.contains(marker)))
    }
}

struct Record {
    value: Value,
    out: BTreeMap<String, Vec<Original>>,
}
struct Scenario {
    name: String,
    records: Vec<Record>,
}

fn scenarios() -> Vec<Scenario> {
    let lines: Vec<Value> = ORIGINS
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let stacks: Vec<Vec<String>> =
        serde_json::from_value(lines.last().unwrap()["stacks"].clone()).unwrap();
    let mut origins: HashMap<(String, i64), Value> = HashMap::new();
    let mut current = String::new();
    for line in &lines {
        match line["t"].as_str().unwrap() {
            "scenario" => current = line["name"].as_str().unwrap().to_owned(),
            "record" => {
                origins.insert(
                    (current.clone(), line["i"].as_i64().unwrap()),
                    line["origins"].clone(),
                );
            }
            _ => {}
        }
    }
    let mut out: Vec<Scenario> = Vec::new();
    for line in TRACE.lines() {
        let value: Value = serde_json::from_str(line).unwrap();
        if value["t"] == "scenario" {
            out.push(Scenario {
                name: value["name"].as_str().unwrap().to_owned(),
                records: Vec::new(),
            });
            continue;
        }
        let scenario = out.last_mut().unwrap();
        let mut records = BTreeMap::new();
        if let Some(sessions) = value["out"].as_object() {
            for (session, entries) in sessions {
                let origin = origins.get(&(scenario.name.clone(), value["i"].as_i64().unwrap()));
                let list = entries
                    .as_array()
                    .unwrap()
                    .iter()
                    .enumerate()
                    .map(|(n, e)| Original {
                        hex: e["hex"].as_str().unwrap().to_owned(),
                        op: e["op"].as_u64().map(|v| v as u8),
                        origin: origin
                            .and_then(|o| o[session][n].as_u64())
                            .map(|i| stacks[i as usize].clone()),
                    })
                    .collect();
                records.insert(session.clone(), list);
            }
        }
        scenario.records.push(Record {
            value,
            out: records,
        });
    }
    out
}

/// The pinned inputs this test reads, checked against the world-trace provenance.
#[test]
fn inputs_are_the_world_trace_inputs_and_the_origins_cover_the_trace() {
    let trace: Value = serde_json::from_str(TRACE_PROVENANCE).unwrap();
    assert_eq!(trace["fixtureSha256"], sha(TRACE.as_bytes()));
    let data = &trace["data"];
    assert_eq!(
        data["data/configs/xteas.json"],
        sha(XTEAS.replace('\r', "").as_bytes())
    );
    assert_eq!(
        data["data/configs/interface_configs.json"],
        sha(INTERFACE_CONFIGS.replace('\r', "").as_bytes())
    );
    assert_eq!(
        trace["config"]["shippedSha256"],
        sha(DEFAULT_CONF.replace('\r', "").as_bytes())
    );
    assert_eq!(trace["config"]["usedSha256"], sha(used_conf().as_bytes()));
    let origins: Value = serde_json::from_str(PROVENANCE).unwrap();
    assert_eq!(origins["fixtureSha256"], sha(ORIGINS.as_bytes()));
    assert_eq!(
        origins["worldTrace"]["fixtureSha256"],
        sha(TRACE.as_bytes())
    );
    assert_eq!(origins["worldTrace"]["recordsIdentical"], true);
    assert_eq!(origins["determinism"]["identical"], true);
    // Every recorded entry of the three login scenarios has an origin.
    let mut entries = 0;
    for scenario in scenarios().iter().filter(|s| s.name != "boot") {
        for record in &scenario.records {
            for list in record.out.values() {
                entries += list.len();
            }
        }
    }
    assert_eq!(origins["counts"]["entries"], entries);
}

fn used_conf() -> String {
    let text = DEFAULT_CONF.replace('\r', "");
    let replaced = text.replacen("enable_bots = true", "enable_bots = false", 1);
    assert_ne!(text, replaced);
    replaced
}

// ------------------------------------------------------------------ binding

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct P(usize);

#[derive(Clone, Debug, PartialEq, Eq)]
struct Out {
    bytes: Vec<u8>,
    /// None: written by native code. Some(marker): a replayed original entry.
    replay: Option<String>,
}

struct Canonical {
    player: Player,
    owner: NodeHandle,
    render: player_render::Handle,
    client: ClientInfo,
    weapon: Option<interfaces::Handle>,
    friends: Option<interfaces::Handle>,
    options: interaction_options::Handle,
}

struct Slot {
    name: String,
    uid: i32,
    artificial: bool,
    session: Option<usize>,
    index: i32,
    active: bool,
    playing: bool,
    location: Location,
    canonical: Option<Canonical>,
}

struct Session {
    id: String,
    active: bool,
    object: Option<P>,
    game: bool,
    username: Option<String>,
    direct: Vec<Out>,
    published: bool,
}

/// What the binding answers from, and records.
struct Inner {
    state: State,
    frames: BTreeMap<Node, Vec<Frame>>,
    items: Arc<Items>,
    factory: Factory,
    registry: interfaces::Registry,
    options: interaction_options::Constants,
    xteas: HashMap<i32, [i32; 4]>,
    constants: Constants,
    world: World,
    hooks: usize,
    players: Vec<Slot>,
    sessions: Vec<Session>,
    queue: Vec<(usize, Out)>,
    /// The original's queued entries (stack origins) of the tick being compared, per session.
    expected: HashMap<usize, Vec<Original>>,
    replays: Vec<(usize, String, usize)>,
    calls: Vec<String>,
    pending: Vec<login::pulse::LoginPulse<P, Parser, usize>>,
    ticks: i32,
    current: P,
    save_path: String,
    clock: Arc<Clock>,
    /// ServerConstants.START_LOCATION.
    start: Location,
}

struct Binding {
    inner: Inner,
    social: social::Social<P>,
}

fn native<E: std::fmt::Debug>(e: E) -> login::Error {
    login::Error::Exception(format!("{e:?}"))
}

impl Inner {
    fn slot(&self, p: &P) -> &Slot {
        &self.players[p.0]
    }
    fn canonical(&self, p: &P) -> &Canonical {
        self.players[p.0]
            .canonical
            .as_ref()
            .expect("a canonical player")
    }
    fn session_of(&self, p: &P) -> usize {
        self.players[p.0].session.expect("a session")
    }
    fn queue_native(&mut self, p: &P, frame: Frame) {
        let session = self.session_of(p);
        let mut bytes = Vec::new();
        frame
            .write(Direction::ServerToClient, None, &mut bytes)
            .unwrap();
        self.queue.push((
            session,
            Out {
                bytes,
                replay: None,
            },
        ));
    }
    fn direct_native(&mut self, session: usize, frame: Frame) {
        let mut bytes = Vec::new();
        frame
            .write(Direction::ServerToClient, None, &mut bytes)
            .unwrap();
        self.sessions[session].direct.push(Out {
            bytes,
            replay: None,
        });
    }
    /// Moves what the canonical services queued for the player's node into the session queue.
    fn drain(&mut self, p: &P) {
        let node = self.canonical(p).owner.node;
        if let Some(frames) = self.frames.remove(&node) {
            for frame in frames {
                self.queue_native(p, frame);
            }
        }
    }
    /// Replays the original's queued entries at the session's current queue
    /// position whose stack contains `marker`.
    fn replay(&mut self, p: &P, marker: &str) -> usize {
        let session = self.session_of(p);
        let position = self.queue.iter().filter(|(s, _)| *s == session).count();
        let expected = self.expected.get(&session).cloned().unwrap_or_default();
        let mut n = 0;
        while let Some(entry) = expected.get(position + n) {
            if !entry.has(marker) {
                break;
            }
            let bytes = unhex(&entry.hex);
            let op = entry.op.unwrap_or(0);
            if COUNTER_OPCODES.contains(&op) {
                self.canonical(p).player.manager().packet_count(1);
            }
            if op == 60 || op == 226 {
                let (id, value) = decode_varp(&bytes);
                let values = self.canonical(p).player.variables().values().unwrap();
                values.put(id, Some(value));
            }
            self.queue.push((
                session,
                Out {
                    bytes,
                    replay: Some(marker.to_owned()),
                },
            ));
            n += 1;
        }
        self.replays.push((session, marker.to_owned(), n));
        n
    }
    fn variables<T>(
        &mut self,
        p: &P,
        operation: impl FnOnce(
            &mut players::owned::VariableServices<'_, Assets>,
            &NodeHandle,
        ) -> variables::Result<T>,
    ) -> login::Result<T> {
        let player = self.canonical(p).player.clone();
        let owner = self.canonical(p).owner.clone();
        let result = player
            .apply_variables(&mut self.state, &mut self.frames, &mut Assets, |ctx| {
                operation(ctx, &owner)
            })
            .map_err(native);
        self.drain(p);
        result
    }
    fn text(&mut self, p: &P, text: &str, interface: i32, child: i32) -> login::Result<()> {
        let counter = self.canonical(p).player.manager().packet_count(1);
        let frame =
            updates::interface_text(interface.wrapping_shl(16) | child, text.as_bytes(), counter)
                .map_err(native)?;
        self.queue_native(p, frame);
        Ok(())
    }
    /// PacketDispatch.sendMessage.
    fn message(&mut self, p: &P, text: &str) -> login::Result<()> {
        let filtered = self
            .canonical(p)
            .player
            .attributes()
            .get("chat_filter")
            .is_some();
        if filtered && !text.contains("<col=CC6600>") && !text.contains("<col=FFFF00>") {
            return Ok(());
        }
        assert!(
            text.encode_utf16().count() <= 255,
            "message truncation is not exercised"
        );
        let mut w = Writer::new();
        w.pjstr(text.as_bytes()).map_err(native)?;
        self.queue_native(
            p,
            Frame {
                opcode: 70,
                payload: w.into_vec(),
            },
        );
        Ok(())
    }
    fn with_profile<T>(&self, p: &P, f: impl FnOnce(&mut Profile) -> T) -> login::Result<T> {
        self.canonical(p).player.with_profile(f).map_err(native)
    }
    fn environment_run<T>(
        &mut self,
        p: &P,
        f: impl FnOnce(&Player, &State, &mut Environment<'_, BaseTimers, Rest>) -> T,
    ) -> (T, Vec<String>) {
        let player = self.canonical(p).player.clone();
        let mut timers = BaseTimers;
        let mut rest = Rest {
            hooks: self.hooks,
            ticks: self.ticks,
            calls: Vec::new(),
        };
        let clock = self.clock.clone();
        let mut env = Environment {
            clock: &*clock,
            timers: &mut timers,
            services: &mut rest,
            world: &mut self.world,
            attribute_path: "data/players/attributes/",
        };
        let result = f(&player, &self.state, &mut env);
        (result, rest.calls)
    }
}

/// The varbit archive is not opened: the one varbit login sets is pinned.
struct Assets;
impl rs09_server::definitions::varbit::Assets for Assets {
    fn file(
        &mut self,
        _: &rs09_server::definitions::varbit::Registry,
        group: u32,
        file: u32,
    ) -> rs09_server::definitions::varbit::Result<Option<Vec<u8>>> {
        Err(rs09_server::definitions::varbit::Error::Binding(format!(
            "varbit {group}/{file} is not pinned in this test"
        )))
    }
}

/// Config (60) and its large form (226): `p2add(id)` with `p1neg(value)`, or
/// `p4(value)` with `p2add(id)`.
fn decode_varp(bytes: &[u8]) -> (i32, i32) {
    let id = |hi: u8, lo: u8| (i32::from(hi) << 8) | i32::from(lo.wrapping_sub(128));
    match bytes[0] {
        60 => (
            id(bytes[1], bytes[2]),
            i32::from((0u8.wrapping_sub(bytes[3])) as i8),
        ),
        226 => (
            id(bytes[5], bytes[6]),
            i32::from_be_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]),
        ),
        other => panic!("not a varp opcode: {other}"),
    }
}

/// InteractionOption (44), as the original writes it.
fn interaction_option(index: i32, name: &str) -> Frame {
    let mut w = Writer::new();
    // putLEShortA(-1): the low byte plus 128, then the high byte.
    let value: i32 = std::hint::black_box(-1);
    w.p1(value.wrapping_add(128));
    w.p1(value >> 8);
    let remove = name == "null";
    w.p1(if remove {
        0
    } else if index == 0 {
        1
    } else {
        0
    });
    w.p1(index + 1);
    w.pjstr(name.as_bytes()).unwrap();
    Frame {
        opcode: 44,
        payload: w.into_vec(),
    }
}

// ---------------------------------------------------- repository::Host

const NOT_REACHED: &str = "not reached in the world-trace login scenarios";

impl repository::Host<P> for Binding {
    type Session = usize;
    type Clan = ();
    type Location = ();
    fn uid(&mut self, p: &P) -> i32 {
        self.inner.slot(p).uid
    }
    fn name(&mut self, p: &P) -> Option<String> {
        Some(self.inner.slot(p).name.clone())
    }
    fn details_username(&mut self, p: &P) -> Option<String> {
        Some(self.inner.slot(p).name.clone())
    }
    fn set_index(&mut self, p: &P, index: i32) {
        self.inner.players[p.0].index = index;
        if let Some(c) = &self.inner.players[p.0].canonical {
            c.render.write().index = index;
            let node = c.owner.node;
            self.inner.state.actor_mut(node).unwrap().wire_index = index;
        }
    }
    fn is_artificial(&mut self, p: &P) -> bool {
        self.inner.slot(p).artificial
    }
    fn afk_logout(&mut self, _: &P) -> Option<bool> {
        None
    }
    fn set_playing(&mut self, p: &P, playing: bool) {
        self.inner.players[p.0].playing = playing;
    }
    fn session(&mut self, p: &P) -> Option<usize> {
        self.inner.slot(p).session
    }
    fn ticks(&mut self) -> i32 {
        self.inner.ticks
    }
    fn global_chat_enabled(&mut self) -> bool {
        panic!("{NOT_REACHED}")
    }
    fn to_lower_case(&mut self, text: &str) -> String {
        rs09_game_core::java_lower::lowercase_root(text)
    }
    fn session_active(&mut self, s: &usize) -> bool {
        self.inner.sessions[*s].active
    }
    fn set_session_active(&mut self, s: &usize, active: bool) {
        self.inner.sessions[*s].active = active;
    }
    fn session_has_key(&mut self, _: &usize) -> bool {
        false
    }
    fn key_cancel(&mut self, _: &usize) -> repository::Result<()> {
        Ok(())
    }
    fn socket_close(&mut self, _: &usize) -> repository::Result<()> {
        Ok(())
    }
    fn session_object(&mut self, s: &usize) -> SessionObject<P> {
        self.inner.sessions[*s]
            .object
            .map_or(SessionObject::Null, SessionObject::Player)
    }
    fn set_session_object(&mut self, s: &usize, p: Option<P>) {
        self.inner.sessions[*s].object = p;
    }
    fn session_username(&mut self, s: &usize) -> Option<String> {
        self.inner.sessions[*s].username.clone()
    }
    fn player_clear(&mut self, _: &Repository<P>, _: &P) -> repository::Result<()> {
        Err(repository::Error::Throwable(NOT_REACHED.into()))
    }
    fn renderable_players_remove(&mut self, _: &P) -> repository::Result<bool> {
        Err(repository::Error::Throwable(NOT_REACHED.into()))
    }
    fn in_combat(&mut self, _: &P) -> repository::Result<bool> {
        Err(repository::Error::Throwable(NOT_REACHED.into()))
    }
    fn lifepoints(&mut self, _: &P) -> repository::Result<i32> {
        Err(repository::Error::Throwable(NOT_REACHED.into()))
    }
    fn is_dead(&mut self, _: &P) -> repository::Result<bool> {
        Err(repository::Error::Throwable(NOT_REACHED.into()))
    }
    fn is_teleporting(&mut self, _: &P) -> repository::Result<bool> {
        Err(repository::Error::Throwable(NOT_REACHED.into()))
    }
    fn has_soft_script(&mut self, _: &P) -> repository::Result<bool> {
        Err(repository::Error::Throwable(NOT_REACHED.into()))
    }
    fn logout_packet(&mut self, _: &P) -> repository::Result<()> {
        Err(repository::Error::Throwable(NOT_REACHED.into()))
    }
    fn world_logout_listeners(&mut self, _: &P) -> repository::Result<()> {
        Err(repository::Error::Throwable(NOT_REACHED.into()))
    }
    fn walking_queue_reset(&mut self, _: &P) -> repository::Result<()> {
        Err(repository::Error::Throwable(NOT_REACHED.into()))
    }
    fn player_logout_listeners(&mut self, _: &P) -> repository::Result<()> {
        Err(repository::Error::Throwable(NOT_REACHED.into()))
    }
    fn has_familiar(&mut self, _: &P) -> repository::Result<bool> {
        Err(repository::Error::Throwable(NOT_REACHED.into()))
    }
    fn familiar_clear(&mut self, _: &P) -> repository::Result<()> {
        Err(repository::Error::Throwable(NOT_REACHED.into()))
    }
    fn interface_close(&mut self, _: &P) -> repository::Result<()> {
        Err(repository::Error::Throwable(NOT_REACHED.into()))
    }
    fn interface_close_single_tab(&mut self, _: &P) -> repository::Result<()> {
        Err(repository::Error::Throwable(NOT_REACHED.into()))
    }
    fn entity_clear(&mut self, _: &P) -> repository::Result<()> {
        Err(repository::Error::Throwable(NOT_REACHED.into()))
    }
    fn zone_monitor_clear(&mut self, _: &P) -> repository::Result<()> {
        Err(repository::Error::Throwable(NOT_REACHED.into()))
    }
    fn house_leave(&mut self, _: &P) -> repository::Result<()> {
        Err(repository::Error::Throwable(NOT_REACHED.into()))
    }
    fn publish_player_status(&mut self, _: &str, _: i32, _: bool) -> repository::Result<()> {
        Err(repository::Error::Throwable(NOT_REACHED.into()))
    }
    fn clan(&mut self, _: &P) -> repository::Result<Option<()>> {
        Err(repository::Error::Throwable(NOT_REACHED.into()))
    }
    fn world_id(&mut self) -> repository::Result<i32> {
        Ok(self.inner.constants.world_id)
    }
    fn clan_owner(&mut self, _: &()) -> repository::Result<Option<String>> {
        Err(repository::Error::Throwable(NOT_REACHED.into()))
    }
    fn publish_clan_leave(&mut self, _: &str, _: i32, _: &str) -> repository::Result<()> {
        Err(repository::Error::Throwable(NOT_REACHED.into()))
    }
    fn wealth_update(&mut self, _: &P, _: bool) -> repository::Result<()> {
        Err(repository::Error::Throwable(NOT_REACHED.into()))
    }
    fn clan_leave(&mut self, _: &(), _: &P, _: bool) -> repository::Result<()> {
        Err(repository::Error::Throwable(NOT_REACHED.into()))
    }
    fn details_save(&mut self, _: &P) -> repository::Result<()> {
        Err(repository::Error::Throwable(NOT_REACHED.into()))
    }
    fn execute_sql(
        &mut self,
        _: rs09_server::world::disconnection::SaveTask<P>,
    ) -> repository::Result<()> {
        Err(repository::Error::Throwable(NOT_REACHED.into()))
    }
    fn set_thread_name(&mut self, _: &str) {}
    fn save_immediately(&mut self, _: &P) -> repository::Result<()> {
        Err(repository::Error::Throwable(NOT_REACHED.into()))
    }
    fn log(&mut self, _: LogLevel, message: &str) -> repository::Result<()> {
        self.inner.calls.push(format!("log {message}"));
        Ok(())
    }
    fn print_stack_trace(&mut self, error: &repository::Error) {
        self.inner.calls.push(format!("printStackTrace {error:?}"));
    }
    fn is_revenant(&mut self, _: &P) -> bool {
        panic!("{NOT_REACHED}")
    }
    fn set_renderable(&mut self, _: &P, _: bool) {
        panic!("{NOT_REACHED}")
    }
    fn npc_id(&mut self, _: &P) -> i32 {
        panic!("{NOT_REACHED}")
    }
    fn region_chunk_npcs(&mut self, _: &()) -> repository::Result<Vec<P>> {
        Err(repository::Error::Throwable(NOT_REACHED.into()))
    }
    fn npc_at(&mut self, _: &P, _: &()) -> bool {
        panic!("{NOT_REACHED}")
    }
    fn ge_news_muted(&mut self, _: &P) -> repository::Result<bool> {
        Err(repository::Error::Throwable(NOT_REACHED.into()))
    }
    fn send_message(&mut self, _: &P, _: &str) -> repository::Result<()> {
        Err(repository::Error::Throwable(NOT_REACHED.into()))
    }
}

// ---------------------------------------------------- login::Host

impl login::Host<P> for Binding {
    type Ui = Inner;
    type Parser = Parser;
    type Social = Inner;
    fn now_millis(&mut self) -> i64 {
        now()
    }
    fn constants(&mut self) -> Constants {
        self.inner.constants.clone()
    }
    fn random_seed(&mut self) -> i64 {
        panic!("{NOT_REACHED}: the lobby")
    }
    fn session_address(&mut self, _: &usize) -> Option<String> {
        // TraceSession.getRemoteAddress, as the original's own test session answers.
        Some("127.0.0.1".into())
    }
    fn server_key(&mut self, _: &usize) -> i64 {
        panic!("{NOT_REACHED}: reply 21")
    }
    fn write_bytes(&mut self, s: &usize, bytes: Vec<u8>, _: Write) -> login::Result<()> {
        self.inner.sessions[*s].direct.push(Out {
            bytes,
            replay: None,
        });
        Ok(())
    }
    fn set_game_producer(&mut self, s: &usize) {
        self.inner.sessions[*s].game = true;
    }
    fn set_last_ping(&mut self, _: &usize, millis: i64) {
        self.inner.calls.push(format!("lastPing {millis}"));
    }
    fn flagged_ips(&mut self) -> login::Result<Vec<String>> {
        // ServerStore is not ported: an empty "flagged-ips" list, as a world
        // without that archive answers.
        self.inner
            .calls
            .push("ServerStore flagged-ips (stand-in: empty)".into());
        Ok(Vec::new())
    }
    fn post_player_alert(&mut self, _: &str, _: &str) -> login::Result<()> {
        Err(login::Error::Throwable(NOT_REACHED.into()))
    }
    fn create_player(&mut self, details: PlayerDetails) -> login::Result<P> {
        let inner = &mut self.inner;
        let name = details.account.lock().unwrap().username.clone();
        let uid = details.account.lock().unwrap().uid;
        let start = inner.start.clone();
        let node = inner.state.add(0, start.clone(), [1, 2, 3, 4]);
        let owner = inner.state.retain_node(node).map_err(native)?;
        let mut inventory = inner
            .factory
            .create(28, ContainerType::Default, SortType::Id)
            .map_err(native)?;
        inventory.register(Some(ListenerHandle::new(InventoryListener::new(Some(
            owner.clone(),
        )))));
        let equipment = inner
            .factory
            .create(14, ContainerType::Default, SortType::Id)
            .map_err(native)?;
        let core = Capture::new(
            owner.clone(),
            inventory,
            equipment,
            Settings::default(),
            inner.factory.null_item.clone(),
        );
        let render = Shared::new(player_render::Player::new(0, start.clone(), 2000));
        let masks = PreparedMasks::new(
            render.clone(),
            Shared::new(appearance::Data::naked(name.clone(), 0)),
        );
        let player = Player::new(
            inner.state.identity(),
            core,
            inner.items.clone(),
            interfaces::Manager::default(),
            Shared::new(false),
            masks,
        )
        .map_err(native)?;
        inner
            .state
            .bind_player_fields(player.clone())
            .map_err(native)?;
        let profile = Profile::new(
            details,
            Fresh {
                world: &inner.world,
                ticks: inner.ticks,
            },
            &inner.factory,
        )
        .map_err(native)?;
        player.bind_profile(profile).map_err(native)?;
        let session = inner
            .sessions
            .iter()
            .position(|s| s.username.as_deref() == Some(name.as_str()));
        inner.players.push(Slot {
            name,
            uid,
            artificial: false,
            session,
            index: 0,
            active: false,
            playing: false,
            location: start,
            canonical: Some(Canonical {
                player,
                owner,
                render,
                client: ClientInfo {
                    display_mode: 0,
                    window_mode: 2,
                    screen_width: 765,
                    screen_height: 503,
                },
                weapon: None,
                friends: None,
                options: interaction_options::Table::new(Some(node)),
            }),
        });
        Ok(P(inner.players.len() - 1))
    }
    fn with_details<R>(
        &mut self,
        p: &P,
        f: impl FnOnce(&mut PlayerDetails) -> R,
    ) -> login::Result<R> {
        self.inner
            .with_profile(p, |profile| f(&mut profile.details))
    }
    fn log_ip(&mut self, _: &P, ip: Option<&str>) -> login::Result<()> {
        self.inner
            .calls
            .push(format!("PlayerMonitor.log IP_LOG {ip:?} (not ported)"));
        Ok(())
    }
    fn account_limit_exception(&mut self, _: &str, _: Option<&str>) -> login::Result<Option<i64>> {
        Err(login::Error::Throwable(NOT_REACHED.into()))
    }
    fn daily_accounts(&mut self, _: Option<&str>) -> login::Result<Vec<String>> {
        Err(login::Error::Throwable(NOT_REACHED.into()))
    }
    fn add_daily_account(&mut self, _: Option<&str>, _: &str) -> login::Result<()> {
        Err(login::Error::Throwable(NOT_REACHED.into()))
    }
    fn print_stack(&mut self, error: &login::Error) {
        self.inner.calls.push(format!("printStackTrace {error:?}"));
    }
    fn is_updating(&mut self) -> bool {
        // SystemManager is not ported; the trace's system state is PRIVATE.
        false
    }
    fn parse_save(&mut self, p: &P) -> login::Result<Option<Parser>> {
        let location = self.inner.slot(p).location.clone();
        let save_path = self.inner.save_path.clone();
        let (outcome, calls) = self.inner.environment_run(p, |player, state, env| {
            let mut parser = Parser::default();
            let outcome = player.load_file(state, Some(&location), &mut parser, &save_path, env);
            (outcome, parser)
        });
        self.inner.calls.extend(calls);
        let (outcome, parser) = outcome;
        let outcome = outcome.map_err(native)?;
        if let Some(location) = outcome.location {
            self.inner.players[p.0].location = location.clone();
            self.inner.canonical(p).render.write().location = Some(location);
        }
        match outcome.result {
            Ok(_) => Ok(Some(parser)),
            Err(error) => {
                self.inner
                    .calls
                    .push(format!("PlayerParser.parse printed {error:?}"));
                Ok(None)
            }
        }
    }
    fn submit_login_pulse(
        &mut self,
        pulse: login::pulse::LoginPulse<P, Parser, usize>,
    ) -> login::Result<()> {
        self.inner.pending.push(pulse);
        Ok(())
    }
    fn social(&mut self) -> (&mut social::Social<P>, &mut Inner) {
        (&mut self.social, &mut self.inner)
    }
    fn saved_spawn_location(
        &mut self,
        p: &P,
        default: Option<Location>,
    ) -> login::Result<Option<Location>> {
        match self
            .inner
            .canonical(p)
            .player
            .attributes()
            .get("/save:spawnLocation")
        {
            None => Ok(default),
            Some(_) => panic!("{NOT_REACHED}: a saved spawn location"),
        }
    }
    fn set_spawn_location(&mut self, p: &P, location: Option<Location>) -> login::Result<()> {
        self.inner
            .with_profile(p, |profile| profile.properties.spawn_location = location)
    }
    fn login_listener(&mut self, listener: &str, p: &P) -> login::Result<()> {
        self.inner.replay(p, &format!("{listener}.login:"));
        Ok(())
    }
    fn run_content_hooks(&mut self, p: &P, parser: &Parser) -> login::Result<()> {
        let location = self.inner.slot(p).location.clone();
        let (result, calls) = self.inner.environment_run(p, |player, state, env| {
            player.run_content_hooks(state, Some(&location), parser, env)
        });
        self.inner.calls.extend(calls);
        result.map_err(native)?.map_err(native)
    }
    fn init_reconnect(&mut self, _: &P) -> login::Result<()> {
        Err(login::Error::Throwable(NOT_REACHED.into()))
    }
    fn set_active(&mut self, p: &P, active: bool) {
        self.inner.players[p.0].active = active;
    }
    fn add_auto_timers(&mut self, _: &P) -> login::Result<()> {
        self.inner
            .calls
            .push("TimerRegistry.addAutoTimers (timer content not ported)".into());
        Ok(())
    }
    fn update_scene_graph(&mut self, p: &P, _: bool) -> login::Result<()> {
        struct Keys<'a> {
            xteas: &'a HashMap<i32, [i32; 4]>,
            frames: Vec<Frame>,
        }
        impl rebuild::Context for Keys<'_> {
            fn keys(&mut self, id: i32) -> player_render::Result<[i32; 4]> {
                Ok(self.xteas.get(&id).copied().unwrap_or([0; 4]))
            }
            fn send(
                &mut self,
                _: &player_render::Handle,
                frame: Frame,
            ) -> player_render::Result<()> {
                self.frames.push(frame);
                Ok(())
            }
        }
        let render = self.inner.canonical(p).render.clone();
        let mut keys = Keys {
            xteas: &self.inner.xteas,
            frames: Vec::new(),
        };
        rebuild::send(&mut keys, &render).map_err(native)?;
        let session = self.inner.session_of(p);
        for frame in keys.frames {
            self.inner.direct_native(session, frame);
        }
        Ok(())
    }
    fn reinit_varps(&mut self, p: &P) -> login::Result<()> {
        self.inner
            .variables(p, |ctx, owner| variables::reinit(ctx, Some(owner)))
    }
    fn attribute_bool(&mut self, p: &P, key: &str, default: bool) -> login::Result<bool> {
        self.inner
            .canonical(p)
            .player
            .attributes()
            .boolean(key, default)
            .map_err(native)
    }
    fn login_type_not_reconnect(&mut self, p: &P) -> login::Result<bool> {
        match self
            .inner
            .canonical(p)
            .player
            .attributes()
            .get("login_type")
        {
            None => Ok(true),
            Some(_) => panic!("{NOT_REACHED}: a login_type attribute"),
        }
    }
    fn set_attribute_bool(&mut self, p: &P, key: &str, value: bool) -> login::Result<()> {
        self.inner
            .canonical(p)
            .player
            .attributes()
            .set_boolean(key, value);
        Ok(())
    }
    fn log_line(&mut self, origin: &str, level: Level, message: &str) -> login::Result<()> {
        self.inner
            .calls
            .push(format!("log {origin} {level:?} {message}"));
        Ok(())
    }
    fn index(&mut self, p: &P) -> i32 {
        self.inner.slot(p).index
    }
    fn ui(&mut self, p: &P) -> login::Result<(&mut Inner, interfaces::Manager)> {
        self.inner.current = *p;
        let manager = self.inner.canonical(p).player.manager();
        Ok((&mut self.inner, manager))
    }
    fn dispatch_message(&mut self, p: &P, text: &str) -> login::Result<()> {
        self.inner.message(p, text)
    }
    fn set_interface_text(
        &mut self,
        p: &P,
        text: &str,
        interface: i32,
        child: i32,
    ) -> login::Result<()> {
        self.inner.text(p, text, interface, child)
    }
    fn send_last_login_info(&mut self, _: &P) -> login::Result<()> {
        Err(login::Error::Throwable(NOT_REACHED.into()))
    }
    fn inventory_refresh(&mut self, p: &P) -> login::Result<()> {
        let inner = &mut self.inner;
        let player = inner.canonical(p).player.clone();
        player
            .apply(
                &mut inner.state,
                &mut inner.frames,
                now,
                |inventory, definitions, ctx| inventory.refresh(None, definitions, ctx),
            )
            .map_err(native)?;
        inner.drain(p);
        Ok(())
    }
    fn equipment_refresh(&mut self, p: &P) -> login::Result<()> {
        let owner = self.inner.canonical(p).owner.clone();
        let mut container = self.inner.canonical(p).player.equipment();
        let items = self.inner.items.clone();
        let mut ctx = Equipment {
            inner: &mut self.inner,
            p: *p,
        };
        equipment::Listener::new(Some(owner))
            .refresh(&mut ctx, &mut container, &items)
            .map_err(native)
    }
    fn skills_refresh(&mut self, p: &P) -> login::Result<()> {
        let mut skills = self.inner.with_profile(p, |profile| {
            std::mem::replace(&mut profile.skills, placeholder_skills())
        })?;
        let result = skills.refresh(&mut SkillsContext {
            inner: &mut self.inner,
            p: *p,
        });
        self.inner
            .with_profile(p, |profile| profile.skills = skills)?;
        result.map_err(native)
    }
    fn skills_configure(&mut self, p: &P) -> login::Result<()> {
        let mut skills = self.inner.with_profile(p, |profile| {
            std::mem::replace(&mut profile.skills, placeholder_skills())
        })?;
        let result = skills.update_combat_level(&mut SkillsContext {
            inner: &mut self.inner,
            p: *p,
        });
        self.inner
            .with_profile(p, |profile| profile.skills = skills)?;
        result.map(|_| ()).map_err(native)
    }
    fn settings_update(&mut self, p: &P) -> login::Result<()> {
        let settings = self.inner.canonical(p).player.settings();
        settings
            .update(&mut SettingsContext {
                inner: &mut self.inner,
                p: *p,
            })
            .map_err(native)
    }
    fn interaction_set_default(&mut self, p: &P) -> login::Result<()> {
        let table = self.inner.canonical(p).options.clone();
        interaction_options::set_default(
            &mut OptionsContext {
                inner: &mut self.inner,
                p: *p,
            },
            &table,
        )
        .map_err(native)
    }
    fn send_run_energy(&mut self, p: &P) -> login::Result<()> {
        let energy = self
            .inner
            .canonical(p)
            .player
            .settings()
            .fields
            .read()
            .run_energy;
        self.inner.queue_native(p, updates::run_energy(energy));
        Ok(())
    }
    fn familiar_login(&mut self, p: &P) -> login::Result<()> {
        self.inner.replay(p, "FamiliarManager.login");
        Ok(())
    }
    fn quest_synchronize_tab(&mut self, p: &P) -> login::Result<()> {
        self.inner.replay(p, "QuestRepository.syncronizeTab");
        Ok(())
    }
    fn emote_refresh(&mut self, p: &P) -> login::Result<()> {
        self.inner.replay(p, "EmoteManager.refresh");
        Ok(())
    }
    fn login_plugins(&mut self) -> Vec<String> {
        boot()["listeners"]["loginPlugins"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_owned())
            .collect()
    }
    fn login_plugin(&mut self, plugin: &str, p: &P) -> login::Result<()> {
        self.inner.replay(p, plugin);
        Ok(())
    }
    fn appearance_sync(&mut self, _: &P) -> login::Result<()> {
        self.inner
            .calls
            .push("Appearance.sync (prepared masks not joined here)".into());
        Ok(())
    }
    fn renderable_players_add(&mut self, p: &P) -> login::Result<()> {
        self.inner.calls.push(format!(
            "UpdateSequence.renderablePlayers.add {}",
            self.inner.slot(p).name
        ));
        Ok(())
    }
    fn region_move(&mut self, p: &P) -> login::Result<()> {
        // RegionManager.move: entity regions and zones are libraries not
        // joined here; ZoneMonitor's music zones queue packets.
        self.inner.replay(p, "RegionManager.move");
        Ok(())
    }
    fn music_init(&mut self, p: &P) -> login::Result<()> {
        self.inner.replay(p, "MusicPlayer.init");
        Ok(())
    }
    fn update_appearance(&mut self, _: &P) -> login::Result<()> {
        self.inner
            .calls
            .push("updateAppearance (update masks not joined here)".into());
        Ok(())
    }
    fn set_update_scene_graph_flag(&mut self, p: &P, value: bool) -> login::Result<()> {
        self.inner.canonical(p).render.write().update_scene = value;
        Ok(())
    }
    fn test_stage(&mut self, p: &P) -> login::Result<i32> {
        self.inner
            .with_profile(p, |profile| profile.saved.global.player_test_stage)
    }
    fn safety_first_unlocked(&mut self, _: &P) -> login::Result<bool> {
        Err(login::Error::Throwable(NOT_REACHED.into()))
    }
    fn unlock_safety_first(&mut self, _: &P) -> login::Result<()> {
        Err(login::Error::Throwable(NOT_REACHED.into()))
    }
    fn attack_style_index(&mut self, p: &P) -> login::Result<i32> {
        Ok(self
            .inner
            .canonical(p)
            .player
            .settings()
            .fields
            .read()
            .attack_style_index)
    }
    fn equipment_items(&mut self, p: &P) -> login::Result<Vec<Option<i32>>> {
        let items = self.inner.canonical(p).player.equipment().items();
        let items = items.read();
        Ok(items
            .iter()
            .map(|item| item.as_ref().map(|i| i.read().id()))
            .collect())
    }
    fn rerun_equip_hooks(&mut self, _: &P, _: usize, _: i32) -> login::Result<()> {
        Err(login::Error::Throwable(NOT_REACHED.into()))
    }
    fn spell_book(&mut self, p: &P) -> login::Result<i32> {
        self.inner.with_profile(p, |profile| profile.spell_book)
    }
    fn player_message(&mut self, _: &P, _: &str) -> login::Result<()> {
        Err(login::Error::Throwable(NOT_REACHED.into()))
    }
    fn has_requirement(&mut self, _: &P, quest: &str, message: bool) -> login::Result<bool> {
        // QuestRequirements is content and not ported. These characters have
        // started no quest; the original wrote no varbit 3909 for them.
        self.inner.calls.push(format!(
            "hasRequirement {quest} {message} (stand-in: false)"
        ));
        Ok(false)
    }
    fn set_spell_book(&mut self, _: &P, _: i32) -> login::Result<()> {
        Err(login::Error::Throwable(NOT_REACHED.into()))
    }
    fn set_varbit(&mut self, p: &P, id: i32, value: i32, save: bool) -> login::Result<()> {
        self.inner.variables(p, |ctx, owner| {
            variables::set_varbit(ctx, Some(owner), id, value, save)
        })
    }
    fn rules_and_info(&mut self, _: &P) -> login::Result<()> {
        Err(login::Error::Throwable(NOT_REACHED.into()))
    }
    fn set_last_login(&mut self, _: &P, _: i64) -> login::Result<()> {
        Err(login::Error::Throwable(NOT_REACHED.into()))
    }
}

fn placeholder_skills() -> Skills {
    struct Empty;
    impl skills::Context for Empty {
        fn kind(&self) -> EntityKind {
            EntityKind::Player
        }
        fn version(&self) -> i32 {
            6
        }
        fn tick(&mut self) -> i32 {
            0
        }
        fn world_tick(&mut self) -> i32 {
            0
        }
        fn combat_level(&self) -> i32 {
            3
        }
        fn summoning_combat_level(&self) -> i32 {
            0
        }
        fn npc_combat_level(&mut self) -> skills::Result<i32> {
            Err(skills::Error::Binding)
        }
        fn damage_spong_active(&mut self) -> skills::Result<bool> {
            Err(skills::Error::Binding)
        }
        fn assisted_level(&mut self, _: i32) -> skills::Result<Option<i32>> {
            Err(skills::Error::Binding)
        }
        fn translate_experience(
            &mut self,
            _: &mut Skills,
            _: i32,
            _: f64,
            _: f64,
        ) -> skills::Result<bool> {
            Err(skills::Error::Binding)
        }
        fn brawling_present(&mut self) -> skills::Result<bool> {
            Err(skills::Error::Binding)
        }
        fn brawling_for_skill(&mut self, _: i32) -> skills::Result<Option<i32>> {
            Err(skills::Error::Binding)
        }
        fn equipment_contains(&mut self, _: i32) -> skills::Result<bool> {
            Err(skills::Error::Binding)
        }
        fn brawling_bonus(&mut self) -> skills::Result<f64> {
            Err(skills::Error::Binding)
        }
        fn tutorial_complete(&mut self) -> skills::Result<bool> {
            Err(skills::Error::Binding)
        }
        fn artificial(&mut self) -> skills::Result<bool> {
            Err(skills::Error::Binding)
        }
        fn username(&mut self) -> skills::Result<String> {
            Err(skills::Error::Binding)
        }
        fn permadeath_present(&self) -> bool {
            false
        }
        fn effect(&mut self, _: &mut Skills, _: Effect) -> skills::Result<()> {
            Ok(())
        }
    }
    Skills::new(&mut Empty).unwrap()
}

// ---------------------------------------------------- interfaces::Context (the Ui)

impl interfaces::Context for Inner {
    fn validate_manager(&mut self, manager: &interfaces::Manager) -> interfaces::Result<()> {
        let owner = self.canonical(&self.current.clone()).owner.clone();
        if manager.player().as_ref() != Some(&owner) {
            return Err(interfaces::Error::Binding(
                "another player's interface manager".into(),
            ));
        }
        Ok(())
    }
    fn component(&mut self, id: i32) -> interfaces::Result<interfaces::Handle> {
        Ok(self.registry.component(id))
    }
    fn resizable(&mut self) -> interfaces::Result<bool> {
        let p = self.current;
        Ok(self.canonical(&p).client.is_resizable())
    }
    fn send(&mut self, frame: Frame) -> interfaces::Result<()> {
        let p = self.current;
        self.queue_native(&p, frame);
        Ok(())
    }
    fn log(&mut self, message: String) -> interfaces::Result<()> {
        self.calls.push(format!("interfaces log {message}"));
        Ok(())
    }
    fn listener_open(&mut self, _: &interfaces::Handle) -> interfaces::Result<()> {
        let p = self.current;
        self.replay(&p, "InterfaceListeners.runOpen");
        Ok(())
    }
    fn listener_close(&mut self, _: &interfaces::Handle) -> interfaces::Result<bool> {
        Err(interfaces::Error::Binding(NOT_REACHED.into()))
    }
    fn plugin_open(
        &mut self,
        _: interfaces::Plugin,
        _: &interfaces::Handle,
    ) -> interfaces::Result<()> {
        Err(interfaces::Error::Binding(NOT_REACHED.into()))
    }
    fn close_event(
        &mut self,
        _: interfaces::CloseEvent,
        _: &interfaces::Handle,
    ) -> interfaces::Result<bool> {
        Err(interfaces::Error::Binding(NOT_REACHED.into()))
    }
    fn open_component(
        &mut self,
        m: &interfaces::Manager,
        c: &interfaces::Handle,
    ) -> interfaces::Result<()> {
        let p = self.current;
        let weapon = self
            .canonical(&p)
            .weapon
            .as_ref()
            .is_some_and(|w| w.same(c));
        let friends = self
            .canonical(&p)
            .friends
            .as_ref()
            .is_some_and(|f| f.same(c));
        if weapon {
            // WeaponInterface.open: core, not ported.
            self.replay(&p, "WeaponInterface.open");
            return Ok(());
        }
        if friends {
            // FriendsListInterface.open: content.
            self.replay(&p, "FriendsListInterface.open");
            return Ok(());
        }
        interfaces::component_open(self, m, c)
    }
    fn dispatch_open(&mut self, _: interfaces::Handle) -> interfaces::Result<()> {
        Err(interfaces::Error::Binding(NOT_REACHED.into()))
    }
    fn set_varp(&mut self, id: i32, value: i32) -> interfaces::Result<()> {
        let p = self.current;
        self.variables(&p, |ctx, owner| {
            variables::set_varp(ctx, Some(owner), id, value, false)
        })
        .map_err(|e| interfaces::Error::Binding(e.to_string()))
    }
    fn weapon_extension(&mut self) -> interfaces::Result<Option<interfaces::Handle>> {
        let p = self.current;
        Ok(self.canonical(&p).weapon.clone())
    }
    fn create_weapon(&mut self) -> interfaces::Result<interfaces::Handle> {
        let handle = self.registry.component(92);
        handle.write().weapon = true;
        Ok(handle)
    }
    fn add_weapon(&mut self, component: interfaces::Handle) -> interfaces::Result<()> {
        let p = self.current;
        self.players[p.0].canonical.as_mut().unwrap().weapon = Some(component);
        Ok(())
    }
    fn spellbook(&mut self) -> interfaces::Result<i32> {
        let p = self.current;
        self.with_profile(&p, |profile| profile.spell_book)
            .map_err(|e| interfaces::Error::Binding(e.to_string()))
    }
    fn has_familiar(&mut self) -> interfaces::Result<bool> {
        // FamiliarManager is not ported; these characters have no familiar.
        Ok(false)
    }
    fn friends_component(&mut self) -> interfaces::Result<interfaces::Handle> {
        let p = self.current;
        let handle = self.registry.component(550);
        self.players[p.0].canonical.as_mut().unwrap().friends = Some(handle.clone());
        Ok(handle)
    }
    fn autocast(&mut self) -> interfaces::Result<Option<usize>> {
        let p = self.current;
        let autocast = self
            .with_profile(&p, |profile| profile.persisted.spells.autocast.is_some())
            .map_err(|e| interfaces::Error::Binding(e.to_string()))?;
        if autocast {
            return Err(interfaces::Error::Binding(NOT_REACHED.into()));
        }
        Ok(None)
    }
    fn auto_spell_id(&mut self, _: &interfaces::Handle, _: usize) -> interfaces::Result<i32> {
        Err(interfaces::Error::Binding(NOT_REACHED.into()))
    }
    fn select_auto_spell(&mut self, _: &interfaces::Handle, _: i32) -> interfaces::Result<()> {
        Err(interfaces::Error::Binding(NOT_REACHED.into()))
    }
    fn window_mode(&mut self) -> interfaces::Result<i32> {
        Err(interfaces::Error::Binding(NOT_REACHED.into()))
    }
    fn set_window_mode(&mut self, _: i32) -> interfaces::Result<()> {
        Err(interfaces::Error::Binding(NOT_REACHED.into()))
    }
    fn tutorial_complete(&mut self) -> interfaces::Result<bool> {
        Err(interfaces::Error::Binding(NOT_REACHED.into()))
    }
    fn tutorial_tabs(&mut self) -> interfaces::Result<()> {
        Err(interfaces::Error::Binding(NOT_REACHED.into()))
    }
    fn has_runscript(&mut self) -> interfaces::Result<bool> {
        let p = self.current;
        Ok(self
            .canonical(&p)
            .player
            .attributes()
            .get("runscript")
            .is_some())
    }
}

// ---------------------------------------------------- social::Host

impl social::Host<P> for Inner {
    fn name(&mut self, p: &P) -> String {
        self.slot(p).name.clone()
    }
    fn with_details<R>(&mut self, p: &P, f: impl FnOnce(&mut PlayerDetails) -> R) -> R {
        self.with_profile(p, |profile| f(&mut profile.details))
            .unwrap()
    }
    fn private_chat_setting(&mut self, _: &P) -> i32 {
        panic!("{NOT_REACHED}")
    }
    fn is_ai(&mut self, p: &P) -> bool {
        self.slot(p).artificial
    }
    fn is_artificial(&mut self, p: &P) -> bool {
        self.slot(p).artificial
    }
    fn has_session(&mut self, p: &P) -> bool {
        self.slot(p).session.is_some()
    }
    fn is_resizable(&mut self, _: &P) -> bool {
        panic!("{NOT_REACHED}")
    }
    fn world_id(&mut self) -> i32 {
        self.constants.world_id
    }
    fn world_name(&mut self) -> String {
        panic!("{NOT_REACHED}")
    }
    fn dev_mode(&mut self) -> bool {
        panic!("{NOT_REACHED}")
    }
    fn enable_default_clan(&mut self) -> bool {
        panic!("{NOT_REACHED}")
    }
    fn server_name(&mut self) -> String {
        self.constants.server_name.clone()
    }
    fn ms_secret_key(&mut self) -> String {
        panic!("{NOT_REACHED}")
    }
    fn global_chat_enabled(&mut self) -> bool {
        panic!("{NOT_REACHED}")
    }
    fn ms_enabled(&mut self) -> bool {
        false
    }
    fn ms_session(&mut self) -> bool {
        false
    }
    fn player_by_name(&mut self, _: &str) -> Option<P> {
        panic!("{NOT_REACHED}")
    }
    fn player_names(&mut self) -> Vec<String> {
        panic!("{NOT_REACHED}")
    }
    fn players(&mut self) -> Vec<P> {
        panic!("{NOT_REACHED}")
    }
    fn current_time_millis(&mut self) -> i64 {
        now()
    }
    fn random_seed(&mut self) -> i64 {
        panic!("{NOT_REACHED}")
    }
    fn player_message(&mut self, _: &P, _: &str) -> social::Result<()> {
        panic!("{NOT_REACHED}")
    }
    fn dispatch_message(&mut self, _: &P, _: &str) -> social::Result<()> {
        panic!("{NOT_REACHED}")
    }
    fn api_message(&mut self, _: &P, _: &str) -> social::Result<()> {
        panic!("{NOT_REACHED}")
    }
    fn send_string(&mut self, _: &P, _: &str, _: i32, _: i32) -> social::Result<()> {
        panic!("{NOT_REACHED}")
    }
    fn open_component(&mut self, _: &P, _: i32) -> social::Result<()> {
        panic!("{NOT_REACHED}")
    }
    fn get_attribute(&mut self, _: &P, _: &str) -> social::Result<Option<social::Value>> {
        panic!("{NOT_REACHED}")
    }
    fn set_attribute(&mut self, _: &P, _: &str, _: social::Value) -> social::Result<()> {
        panic!("{NOT_REACHED}")
    }
    fn is_locked(&mut self, _: &P, _: &str) -> social::Result<bool> {
        panic!("{NOT_REACHED}")
    }
    fn lock(&mut self, _: &P, _: &str, _: i32) -> social::Result<()> {
        panic!("{NOT_REACHED}")
    }
    fn register_chat(
        &mut self,
        _: &P,
        _: &social::ChatMessage<P>,
        _: bool,
    ) -> social::Result<bool> {
        panic!("{NOT_REACHED}")
    }
    fn cipher(&mut self, _: &P) -> social::Result<i32> {
        panic!("{NOT_REACHED}")
    }
    fn write(&mut self, _: &P, _: social::Written) -> social::Result<()> {
        panic!("{NOT_REACHED}")
    }
    fn ms_write(&mut self, _: social::Written) -> social::Result<()> {
        panic!("{NOT_REACHED}")
    }
    fn varp(&mut self, _: &P, _: i32, _: i32, _: bool) -> social::Result<()> {
        panic!("{NOT_REACHED}")
    }
    fn submit_pulse(&mut self, _: social::PulseId, _: i32) -> social::Result<()> {
        panic!("{NOT_REACHED}")
    }
    fn stop_pulse(&mut self, _: social::PulseId) -> social::Result<()> {
        panic!("{NOT_REACHED}")
    }
    fn log(&mut self, _: &str, _: social::Level, _: &str) -> social::Result<()> {
        panic!("{NOT_REACHED}")
    }
    fn log_ms(&mut self, _: &str) -> social::Result<()> {
        panic!("{NOT_REACHED}")
    }
    fn log_chat(&mut self, _: &P, _: &str, _: &str) -> social::Result<()> {
        panic!("{NOT_REACHED}")
    }
    fn log_private_chat(&mut self, _: &P, _: &str, _: &str) -> social::Result<()> {
        panic!("{NOT_REACHED}")
    }
    fn account_info(
        &mut self,
        _: &str,
    ) -> social::Result<rs09_server::auth::account::UserAccountInfo> {
        panic!("{NOT_REACHED}")
    }
    fn online_friends(&mut self, _: &str) -> social::Result<Vec<String>> {
        panic!("{NOT_REACHED}")
    }
    fn account_update(
        &mut self,
        _: &rs09_server::auth::account::UserAccountInfo,
    ) -> social::Result<()> {
        panic!("{NOT_REACHED}")
    }
    fn create_account(
        &mut self,
        _: &rs09_server::auth::account::UserAccountInfo,
    ) -> social::Result<bool> {
        panic!("{NOT_REACHED}")
    }
    fn define_command(&mut self, _: &str, _: &str, _: &str, _: &str) -> social::Result<()> {
        panic!("{NOT_REACHED}")
    }
    fn update_chat_settings(&mut self, _: &P, _: i32, _: i32, _: i32) -> social::Result<()> {
        panic!("{NOT_REACHED}")
    }
    fn ai_incoming_chat(&mut self, _: &P, _: i32, _: &str, _: &str) -> social::Result<()> {
        panic!("{NOT_REACHED}")
    }
    fn management_take(&mut self, _: &social::Event) {
        panic!("{NOT_REACHED}")
    }
}

// ---------------------------------------------------- library contexts

/// skills::Context for Skills.refresh and Skills.configure: SkillLevel is
/// written as the original writes it; LevelUp.sendFlashingIcons is replayed.
struct SkillsContext<'a> {
    inner: &'a mut Inner,
    p: P,
}
impl skills::Context for SkillsContext<'_> {
    fn kind(&self) -> EntityKind {
        EntityKind::Player
    }
    fn version(&self) -> i32 {
        6
    }
    fn tick(&mut self) -> i32 {
        self.inner.ticks
    }
    fn world_tick(&mut self) -> i32 {
        self.inner.ticks
    }
    fn combat_level(&self) -> i32 {
        self.inner
            .with_profile(&self.p, |p| p.properties.combat_level)
            .unwrap()
    }
    fn summoning_combat_level(&self) -> i32 {
        self.inner
            .with_profile(&self.p, |p| p.summoning_combat_level)
            .unwrap()
    }
    fn npc_combat_level(&mut self) -> skills::Result<i32> {
        Err(skills::Error::Binding)
    }
    fn damage_spong_active(&mut self) -> skills::Result<bool> {
        Err(skills::Error::Binding)
    }
    fn assisted_level(&mut self, _: i32) -> skills::Result<Option<i32>> {
        Err(skills::Error::Binding)
    }
    fn translate_experience(
        &mut self,
        _: &mut Skills,
        _: i32,
        _: f64,
        _: f64,
    ) -> skills::Result<bool> {
        Err(skills::Error::Binding)
    }
    fn brawling_present(&mut self) -> skills::Result<bool> {
        Err(skills::Error::Binding)
    }
    fn brawling_for_skill(&mut self, _: i32) -> skills::Result<Option<i32>> {
        Err(skills::Error::Binding)
    }
    fn equipment_contains(&mut self, _: i32) -> skills::Result<bool> {
        Err(skills::Error::Binding)
    }
    fn brawling_bonus(&mut self) -> skills::Result<f64> {
        Err(skills::Error::Binding)
    }
    fn tutorial_complete(&mut self) -> skills::Result<bool> {
        Err(skills::Error::Binding)
    }
    fn artificial(&mut self) -> skills::Result<bool> {
        Ok(false)
    }
    fn username(&mut self) -> skills::Result<String> {
        Err(skills::Error::Binding)
    }
    fn permadeath_present(&self) -> bool {
        false
    }
    fn effect(&mut self, state: &mut Skills, effect: Effect) -> skills::Result<()> {
        match effect {
            Effect::Packet(skill) => {
                // SkillLevel: prayer points rounded up, lifepoints, or the
                // current level without assistance.
                let level = match skill {
                    5 => state.prayer_points.ceil() as i32,
                    3 => state.lifepoints,
                    _ => state.level(skill as i32, true, self)?,
                };
                let frame = updates::skill_level(skill as i32, level, state.experience[skill]);
                self.inner.queue_native(&self.p, frame);
                Ok(())
            }
            Effect::FlashingIcons(_) => {
                // LevelUp.sendFlashingIcons: core, not ported.
                self.inner.replay(&self.p, "LevelUp.sendFlashingIcons");
                Ok(())
            }
            Effect::Combat(level) => {
                self.inner
                    .with_profile(&self.p, |p| p.properties.combat_level = level)
                    .unwrap();
                Ok(())
            }
            Effect::SummoningCombat(level) => {
                self.inner
                    .with_profile(&self.p, |p| p.summoning_combat_level = level)
                    .unwrap();
                Ok(())
            }
            other => panic!("{NOT_REACHED}: skills effect {other:?}"),
        }
    }
}

/// settings::Context for Settings.update: varps through the canonical
/// variable store, the chat settings written at once, run energy queued.
struct SettingsContext<'a> {
    inner: &'a mut Inner,
    p: P,
}
impl settings::Context for SettingsContext<'_> {
    fn varp(&mut self, _: &Settings, id: i32, value: i32) -> settings::Result<()> {
        self.inner
            .variables(&self.p, |ctx, owner| {
                variables::set_varp(ctx, Some(owner), id, value, false)
            })
            .map_err(|_| settings::Error::Binding)
    }
    fn session(&mut self, _: &Settings) -> settings::Result<Option<usize>> {
        Ok(self.inner.slot(&self.p).session)
    }
    fn send(&mut self, _: &Settings, session: Option<usize>, frame: Frame) -> settings::Result<()> {
        self.inner
            .direct_native(session.ok_or(settings::Error::Null)?, frame);
        Ok(())
    }
    fn dispatcher(&mut self, _: &Settings) -> settings::Result<Option<usize>> {
        Ok(Some(0))
    }
    fn run_energy(&mut self, state: &Settings, _: Option<usize>) -> settings::Result<()> {
        let energy = state.fields.read().run_energy;
        self.inner
            .queue_native(&self.p, updates::run_energy(energy));
        Ok(())
    }
    fn message(&mut self, _: &Settings, _: Option<usize>, _: &str) -> settings::Result<()> {
        Err(settings::Error::Binding)
    }
    fn string(
        &mut self,
        _: &Settings,
        _: Option<usize>,
        _: String,
        _: i32,
        _: i32,
    ) -> settings::Result<()> {
        Err(settings::Error::Binding)
    }
    fn properties(&mut self, _: &Settings) -> settings::Result<Option<usize>> {
        Ok(Some(0))
    }
    fn retaliating(&mut self, _: &Settings, _: Option<usize>) -> settings::Result<bool> {
        self.inner
            .with_profile(&self.p, |p| p.properties.retaliating)
            .map_err(|_| settings::Error::Binding)
    }
    fn set_retaliating(&mut self, _: &Settings, _: Option<usize>, _: bool) -> settings::Result<()> {
        Err(settings::Error::Binding)
    }
    fn ironman(&mut self, _: &Settings) -> settings::Result<bool> {
        Err(settings::Error::Binding)
    }
    fn container(&mut self, _: &Settings, _: settings::ContainerKind) -> settings::Result<usize> {
        Err(settings::Error::Binding)
    }
    fn item(&mut self, _: &Settings, _: usize, _: i32) -> settings::Result<Option<usize>> {
        Err(settings::Error::Binding)
    }
    fn weight(&mut self, _: &Settings, _: usize) -> settings::Result<f64> {
        Err(settings::Error::Binding)
    }
}

/// settings::Context for Settings.updateWeight over the canonical containers.
struct WeightContext<'a> {
    inner: &'a mut Inner,
    p: P,
    item: Option<ItemHandle>,
}
impl settings::Context for WeightContext<'_> {
    fn varp(&mut self, _: &Settings, _: i32, _: i32) -> settings::Result<()> {
        Err(settings::Error::Binding)
    }
    fn session(&mut self, _: &Settings) -> settings::Result<Option<usize>> {
        Err(settings::Error::Binding)
    }
    fn send(&mut self, _: &Settings, _: Option<usize>, _: Frame) -> settings::Result<()> {
        Err(settings::Error::Binding)
    }
    fn dispatcher(&mut self, _: &Settings) -> settings::Result<Option<usize>> {
        Ok(Some(0))
    }
    fn run_energy(&mut self, _: &Settings, _: Option<usize>) -> settings::Result<()> {
        Err(settings::Error::Binding)
    }
    fn message(&mut self, _: &Settings, _: Option<usize>, _: &str) -> settings::Result<()> {
        Err(settings::Error::Binding)
    }
    fn string(
        &mut self,
        _: &Settings,
        _: Option<usize>,
        text: String,
        interface: i32,
        child: i32,
    ) -> settings::Result<()> {
        self.inner
            .text(&self.p, &text, interface, child)
            .map_err(|_| settings::Error::Binding)
    }
    fn properties(&mut self, _: &Settings) -> settings::Result<Option<usize>> {
        Err(settings::Error::Binding)
    }
    fn retaliating(&mut self, _: &Settings, _: Option<usize>) -> settings::Result<bool> {
        Err(settings::Error::Binding)
    }
    fn set_retaliating(&mut self, _: &Settings, _: Option<usize>, _: bool) -> settings::Result<()> {
        Err(settings::Error::Binding)
    }
    fn ironman(&mut self, _: &Settings) -> settings::Result<bool> {
        Err(settings::Error::Binding)
    }
    fn container(
        &mut self,
        _: &Settings,
        kind: settings::ContainerKind,
    ) -> settings::Result<usize> {
        Ok(match kind {
            settings::ContainerKind::Inventory => 0,
            settings::ContainerKind::Equipment => 1,
        })
    }
    fn item(
        &mut self,
        _: &Settings,
        container: usize,
        slot: i32,
    ) -> settings::Result<Option<usize>> {
        let player = &self.inner.canonical(&self.p).player;
        self.item = match container {
            0 => player.inventory().get(slot),
            1 => player.equipment().get(slot),
            _ => return Err(settings::Error::Binding),
        };
        Ok(self.item.as_ref().map(|_| 0))
    }
    fn weight(&mut self, _: &Settings, _: usize) -> settings::Result<f64> {
        Err(settings::Error::Binding)
    }
}

/// equipment::Context for EquipmentListener.refresh: WeaponInterface's update
/// is replayed; the container packet and the weight are native.
struct Equipment<'a> {
    inner: &'a mut Inner,
    p: P,
}
impl inventory::container::Context for Equipment<'_> {
    fn update(
        &mut self,
        _: ListenerHandle,
        _: &mut Container,
        _: Event,
        _: &Items,
    ) -> inventory::Result<()> {
        Err(inventory::Error::Binding(NOT_REACHED.into()))
    }
    fn refresh(
        &mut self,
        _: ListenerHandle,
        _: &mut Container,
        _: &Items,
    ) -> inventory::Result<()> {
        Err(inventory::Error::Binding(NOT_REACHED.into()))
    }
    fn ground(
        &mut self,
        _: &mut Container,
        _: Option<ItemHandle>,
        _: &Items,
    ) -> inventory::Result<()> {
        Err(inventory::Error::Binding(NOT_REACHED.into()))
    }
}
fn binding<E: std::fmt::Debug>(e: E) -> inventory::Error {
    inventory::Error::Binding(format!("{e:?}"))
}
impl equipment::Context for Equipment<'_> {
    type Dispatch = ();
    type Properties = ();
    type Weapon = ();
    type CombatPulse = ();
    type Settings = Settings;
    type Appearance = ();
    type Interfaces = interfaces::Manager;
    fn inventory(&mut self, _: &NodeHandle) -> inventory::Result<Container> {
        Ok(self.inner.canonical(&self.p).player.inventory())
    }
    fn equipment(&mut self, _: &NodeHandle) -> inventory::Result<Container> {
        Ok(self.inner.canonical(&self.p).player.equipment())
    }
    fn requirements(
        &mut self,
        _: rs09_server::definitions::item::Handle,
        _: Option<&NodeHandle>,
        _: bool,
        _: bool,
    ) -> inventory::Result<bool> {
        Err(inventory::Error::Binding(NOT_REACHED.into()))
    }
    fn dispatch(&mut self, _: &NodeHandle) -> inventory::Result<()> {
        Ok(())
    }
    fn message(&mut self, _: (), _: &str) -> inventory::Result<()> {
        Err(inventory::Error::Binding(NOT_REACHED.into()))
    }
    fn text(
        &mut self,
        _: (),
        text: Option<String>,
        interface: i32,
        child: i32,
    ) -> inventory::Result<()> {
        self.inner
            .text(
                &self.p,
                &text.unwrap_or_else(|| "null".into()),
                interface,
                child,
            )
            .map_err(binding)
    }
    fn unequip_plugin(
        &mut self,
        _: rs09_server::definitions::plugin::Handle,
        _: &NodeHandle,
        _: ItemHandle,
    ) -> inventory::Result<rs09_server::definitions::property::Property> {
        Err(inventory::Error::Binding(NOT_REACHED.into()))
    }
    fn interaction(
        &mut self,
        _: i32,
        _: &NodeHandle,
        _: ItemHandle,
        _: bool,
    ) -> inventory::Result<bool> {
        Err(inventory::Error::Binding(NOT_REACHED.into()))
    }
    fn send(&mut self, packet: equipment::Packet, _: &mut Container) -> inventory::Result<()> {
        let null = self.inner.factory.null_item.clone();
        let frame = packet.frame(&null)?;
        self.inner.queue_native(&self.p, frame);
        Ok(())
    }
    fn properties(&mut self, _: &NodeHandle) -> inventory::Result<()> {
        Ok(())
    }
    fn attack_speed(&mut self, _: (), speed: i32) -> inventory::Result<()> {
        self.inner
            .with_profile(&self.p, |p| p.properties.attack_speed = speed)
            .map_err(binding)
    }
    fn extension(&mut self, _: &NodeHandle) -> inventory::Result<Option<()>> {
        Ok(self.inner.canonical(&self.p).weapon.as_ref().map(|_| ()))
    }
    fn update_weapon(&mut self, _: ()) -> inventory::Result<()> {
        // WeaponInterface.updateInterface: core, not ported.
        self.inner.replay(&self.p, "WeaponInterface");
        Ok(())
    }
    fn defence_animation(&mut self, _: ()) -> inventory::Result<()> {
        self.inner
            .calls
            .push("Properties.updateDefenceAnimation (not ported)".into());
        Ok(())
    }
    fn cape_perks(&mut self, _: &NodeHandle, _: &mut Container, _: usize) -> inventory::Result<()> {
        Err(inventory::Error::Binding(NOT_REACHED.into()))
    }
    fn attribute_bool(
        &mut self,
        _: &NodeHandle,
        key: &str,
        default: bool,
    ) -> inventory::Result<bool> {
        self.inner
            .canonical(&self.p)
            .player
            .attributes()
            .boolean(key, default)
            .map_err(binding)
    }
    fn remove_attribute(&mut self, _: &NodeHandle, _: &str) -> inventory::Result<()> {
        Err(inventory::Error::Binding(NOT_REACHED.into()))
    }
    fn combat_pulse(&mut self, _: ()) -> inventory::Result<()> {
        Err(inventory::Error::Binding(NOT_REACHED.into()))
    }
    fn clear_combat_handler(&mut self, _: ()) -> inventory::Result<()> {
        Err(inventory::Error::Binding(NOT_REACHED.into()))
    }
    fn settings(&mut self, _: &NodeHandle) -> inventory::Result<Settings> {
        Ok(self.inner.canonical(&self.p).player.settings())
    }
    fn special_toggled(&mut self, _: Settings) -> inventory::Result<bool> {
        Err(inventory::Error::Binding(NOT_REACHED.into()))
    }
    fn set_varp(&mut self, _: &NodeHandle, _: i32, _: i32) -> inventory::Result<()> {
        Err(inventory::Error::Binding(NOT_REACHED.into()))
    }
    fn appearance(&mut self, _: &NodeHandle) -> inventory::Result<()> {
        Ok(())
    }
    fn set_animations(&mut self, _: ()) -> inventory::Result<()> {
        self.inner
            .calls
            .push("Appearance.setAnimations (not joined here)".into());
        Ok(())
    }
    fn update_appearance(&mut self, _: &NodeHandle) -> inventory::Result<()> {
        self.inner
            .calls
            .push("updateAppearance (not joined here)".into());
        Ok(())
    }
    fn update_weight(&mut self, settings: Settings) -> inventory::Result<()> {
        let mut ctx = WeightContext {
            inner: &mut *self.inner,
            p: self.p,
            item: None,
        };
        settings.update_weight(&mut ctx).map_err(binding)
    }
    fn perk(&mut self, _: &NodeHandle, _: equipment::Perk) -> inventory::Result<bool> {
        Err(inventory::Error::Binding(NOT_REACHED.into()))
    }
    fn set_bonuses(&mut self, _: (), bonuses: Shared<Vec<i32>>) -> inventory::Result<()> {
        let bonuses = bonuses.read().clone();
        self.inner
            .with_profile(&self.p, |p| p.properties.bonuses = bonuses)
            .map_err(binding)
    }
    fn bonuses(&mut self, _: ()) -> inventory::Result<Shared<Vec<i32>>> {
        let bonuses = self
            .inner
            .with_profile(&self.p, |p| p.properties.bonuses.clone())
            .map_err(binding)?;
        Ok(Shared::new(bonuses))
    }
    fn interfaces(&mut self, _: &NodeHandle) -> inventory::Result<interfaces::Manager> {
        Ok(self.inner.canonical(&self.p).player.manager())
    }
    fn has_main(&mut self, manager: interfaces::Manager, id: i32) -> inventory::Result<bool> {
        Ok(interfaces::has_main_component(&manager, id))
    }
}

/// interaction_options::Context for InteractPlugin.setDefault on a player.
struct OptionsContext<'a> {
    inner: &'a mut Inner,
    p: P,
}
impl interaction_options::Context for OptionsContext<'_> {
    type Definition = ();
    fn kind(&self, node: Option<Node>) -> interaction_options::Kind {
        if node == Some(self.inner.canonical(&self.p).owner.node) {
            interaction_options::Kind::Player
        } else {
            interaction_options::Kind::Other
        }
    }
    fn node_id(&mut self, _: Node) -> interaction_options::Result<i32> {
        Err(interaction_options::Error::Binding(NOT_REACHED.into()))
    }
    fn item_has_location(&mut self, _: Node) -> interaction_options::Result<bool> {
        Err(interaction_options::Error::Binding(NOT_REACHED.into()))
    }
    fn definition(&mut self, _: Node) -> interaction_options::Result<Option<()>> {
        Err(interaction_options::Error::Binding(NOT_REACHED.into()))
    }
    fn names(
        &mut self,
        _: &(),
        _: interaction_options::NamesKind,
    ) -> interaction_options::Result<Option<interaction_options::Names>> {
        Err(interaction_options::Error::Binding(NOT_REACHED.into()))
    }
    fn lowercase(&mut self, name: &str) -> interaction_options::Result<String> {
        Ok(rs09_game_core::java_lower::lowercase_root(name))
    }
    fn option_handler(
        &mut self,
        _: interaction_options::Kind,
        _: i32,
        _: &str,
    ) -> interaction_options::Result<Option<usize>> {
        Err(interaction_options::Error::Binding(NOT_REACHED.into()))
    }
    fn send_option(
        &mut self,
        _: Node,
        index: i32,
        name: Option<String>,
    ) -> interaction_options::Result<()> {
        let name = name.unwrap_or_else(|| "null".into());
        self.inner
            .queue_native(&self.p, interaction_option(index, &name));
        Ok(())
    }
    fn node_string(&mut self, _: Option<Node>) -> interaction_options::Result<String> {
        Err(interaction_options::Error::Binding(NOT_REACHED.into()))
    }
    fn constants(&self) -> &interaction_options::Constants {
        &self.inner.options
    }
}

// ---------------------------------------------------- save-file services

/// The services that stay outside the canonical player during a load, as in
/// tests/canonical_player.rs: content hooks and house/familiar parsing are
/// recorded, not run (not ported).
struct Rest {
    hooks: usize,
    ticks: i32,
    calls: Vec<String>,
}
impl Remaining for Rest {
    fn log(&mut self, _: SaveLevel, message: &str) {
        self.calls.push(format!("save log {message}"));
    }
    fn retaliating(&mut self) -> save::Result<bool> {
        unreachable!("answered by the canonical player")
    }
    fn attribute_set(&mut self, key: &str, _: &AttributeValue) -> save::Result<()> {
        self.calls.push(format!("AttributeSetEvent {key}"));
        Ok(())
    }
    fn content_hooks(&mut self) -> usize {
        self.hooks
    }
    fn save_content(&mut self, hook: usize, _: &mut Json) -> save::Result<()> {
        self.calls.push(format!(
            "content hook {hook} save (not ported: writes nothing)"
        ));
        Ok(())
    }
    fn parse_content(&mut self, hook: usize, _: &Json) -> save::Result<()> {
        self.calls
            .push(format!("content hook {hook} parse (not ported)"));
        Ok(())
    }
    fn parse_familiars(&mut self, _: &Json) -> save::Result<()> {
        self.calls.push("FamiliarManager.parse (not ported)".into());
        Ok(())
    }
    fn parse_house(&mut self, _: &Json) -> save::Result<()> {
        self.calls.push("HouseManager.parse (not ported)".into());
        Ok(())
    }
}
impl inventory::container::Context for Rest {
    fn update(
        &mut self,
        _: ListenerHandle,
        _: &mut Container,
        _: Event,
        _: &Items,
    ) -> inventory::Result<()> {
        self.calls.push("container update during load".into());
        Ok(())
    }
    fn refresh(
        &mut self,
        _: ListenerHandle,
        _: &mut Container,
        _: &Items,
    ) -> inventory::Result<()> {
        self.calls.push("container refresh during load".into());
        Ok(())
    }
    fn ground(
        &mut self,
        _: &mut Container,
        _: Option<ItemHandle>,
        _: &Items,
    ) -> inventory::Result<()> {
        unreachable!()
    }
}
impl skills::Context for Rest {
    fn kind(&self) -> EntityKind {
        EntityKind::Player
    }
    fn version(&self) -> i32 {
        6
    }
    fn tick(&mut self) -> i32 {
        self.ticks
    }
    fn world_tick(&mut self) -> i32 {
        self.ticks
    }
    fn combat_level(&self) -> i32 {
        unreachable!("answered by the canonical player")
    }
    fn summoning_combat_level(&self) -> i32 {
        unreachable!("answered by the canonical player")
    }
    fn npc_combat_level(&mut self) -> skills::Result<i32> {
        unreachable!()
    }
    fn damage_spong_active(&mut self) -> skills::Result<bool> {
        unreachable!()
    }
    fn assisted_level(&mut self, _: i32) -> skills::Result<Option<i32>> {
        unreachable!()
    }
    fn translate_experience(
        &mut self,
        _: &mut Skills,
        _: i32,
        _: f64,
        _: f64,
    ) -> skills::Result<bool> {
        unreachable!()
    }
    fn brawling_present(&mut self) -> skills::Result<bool> {
        unreachable!()
    }
    fn brawling_for_skill(&mut self, _: i32) -> skills::Result<Option<i32>> {
        unreachable!()
    }
    fn equipment_contains(&mut self, _: i32) -> skills::Result<bool> {
        unreachable!()
    }
    fn brawling_bonus(&mut self) -> skills::Result<f64> {
        unreachable!()
    }
    fn tutorial_complete(&mut self) -> skills::Result<bool> {
        unreachable!()
    }
    fn artificial(&mut self) -> skills::Result<bool> {
        unreachable!()
    }
    fn username(&mut self) -> skills::Result<String> {
        unreachable!()
    }
    fn permadeath_present(&self) -> bool {
        unreachable!("the parser supplies the permadeath attribute")
    }
    fn effect(&mut self, _: &mut Skills, effect: Effect) -> skills::Result<()> {
        self.calls
            .push(format!("skills effect during load {effect:?}"));
        Ok(())
    }
}
impl settings::Context for Rest {
    fn varp(&mut self, _: &Settings, _: i32, _: i32) -> settings::Result<()> {
        unreachable!()
    }
    fn session(&mut self, _: &Settings) -> settings::Result<Option<usize>> {
        unreachable!()
    }
    fn send(&mut self, _: &Settings, _: Option<usize>, _: Frame) -> settings::Result<()> {
        unreachable!()
    }
    fn dispatcher(&mut self, _: &Settings) -> settings::Result<Option<usize>> {
        unreachable!()
    }
    fn run_energy(&mut self, _: &Settings, _: Option<usize>) -> settings::Result<()> {
        unreachable!()
    }
    fn message(&mut self, _: &Settings, _: Option<usize>, _: &str) -> settings::Result<()> {
        unreachable!()
    }
    fn string(
        &mut self,
        _: &Settings,
        _: Option<usize>,
        _: String,
        _: i32,
        _: i32,
    ) -> settings::Result<()> {
        unreachable!()
    }
    fn properties(&mut self, _: &Settings) -> settings::Result<Option<usize>> {
        unreachable!("answered by the canonical player")
    }
    fn retaliating(&mut self, _: &Settings, _: Option<usize>) -> settings::Result<bool> {
        unreachable!("answered by the canonical player")
    }
    fn set_retaliating(&mut self, _: &Settings, _: Option<usize>, _: bool) -> settings::Result<()> {
        unreachable!("answered by the canonical player")
    }
    fn ironman(&mut self, _: &Settings) -> settings::Result<bool> {
        unreachable!("answered by the canonical player")
    }
    fn container(&mut self, _: &Settings, _: settings::ContainerKind) -> settings::Result<usize> {
        unreachable!()
    }
    fn item(&mut self, _: &Settings, _: usize, _: i32) -> settings::Result<Option<usize>> {
        unreachable!()
    }
    fn weight(&mut self, _: &Settings, _: usize) -> settings::Result<f64> {
        unreachable!()
    }
}

/// Timer content is not ported: the stand-in of tests/canonical_player.rs.
struct BaseTimers;
impl timers::Context<CoreData> for BaseTimers {
    fn is_player(&self, _: &CoreData) -> bool {
        true
    }
    fn modal_open(&mut self, _: &mut CoreData) -> timers::Result<bool> {
        Ok(false)
    }
    fn script_delay(&mut self, _: &mut CoreData) -> timers::Result<i32> {
        Ok(0)
    }
    fn ticks(&mut self, _: &mut CoreData) -> timers::Result<i32> {
        Ok(0)
    }
    fn entity_name(&mut self, _: &mut CoreData) -> timers::Result<Option<String>> {
        Ok(None)
    }
    fn before_register(
        &mut self,
        _: &timers::Handle,
        _: &mut timers::Manager,
        _: &mut CoreData,
    ) -> timers::Result<()> {
        Ok(())
    }
    fn on_register(
        &mut self,
        _: &timers::Handle,
        _: &mut timers::Manager,
        _: &mut CoreData,
    ) -> timers::Result<()> {
        Ok(())
    }
    fn on_removal(
        &mut self,
        _: &timers::Handle,
        _: &mut timers::Manager,
        _: &mut CoreData,
    ) -> timers::Result<()> {
        Ok(())
    }
    fn run(
        &mut self,
        _: &timers::Handle,
        _: &mut timers::Manager,
        _: &mut CoreData,
    ) -> timers::Result<bool> {
        Ok(true)
    }
    fn initial_delay(
        &mut self,
        _: &timers::Handle,
        _: &mut timers::Manager,
        _: &mut CoreData,
    ) -> timers::Result<i32> {
        Ok(0)
    }
    fn save(
        &mut self,
        timer: &timers::Handle,
        root: &mut rs09_server::definitions::item_config::json::Json,
        _: &mut timers::Manager,
        state: &mut CoreData,
    ) -> timers::Result<()> {
        if timer.read().class_name == "AntiMacro" {
            let left = timer.read().next_execution.wrapping_sub(0);
            root.put(
                "ticksRemaining",
                rs09_server::definitions::item_config::json::Json::Text(left.to_string()),
            )
            .map_err(|_| timers::Error::Type)?;
            return Ok(());
        }
        timers::save_base(timer, root, state, self)
    }
    fn parse(
        &mut self,
        timer: &timers::Handle,
        data: &rs09_server::definitions::item_config::json::Json,
        _: &mut timers::Manager,
        _: &mut CoreData,
    ) -> timers::Result<()> {
        if timer.read().class_name == "AntiMacro" {
            let text = data.get("ticksRemaining").map(|v| v.text());
            let interval = text
                .and_then(|t| rs09_game_core::java_string::parse_i32(&t))
                .unwrap_or(0);
            let mut timer = timer.write();
            timer.run_interval = interval;
            timer.next_execution = interval;
            return Ok(());
        }
        timers::parse_base(timer, data)
    }
    fn resolve(
        &mut self,
        identifier: &str,
        _: &mut timers::Manager,
        _: &mut CoreData,
    ) -> timers::Result<Option<timers::Handle>> {
        let class = if identifier == "antimacro" {
            "AntiMacro"
        } else {
            "RSTimer"
        };
        Ok(Some(Shared::new(timers::Timer::new(
            0,
            identifier.to_owned(),
            class.into(),
            true,
        ))))
    }
    fn run_failure(&mut self, _: &str, _: Option<&str>, _: &mut CoreData) -> timers::Result<()> {
        Ok(())
    }
    fn unknown_timer(&mut self, _: &str, _: &mut CoreData) -> timers::Result<()> {
        Ok(())
    }
    fn exception(&mut self, _: &timers::Error, _: &mut CoreData) -> timers::Result<()> {
        Ok(())
    }
}

// ------------------------------------------------------------------ driver

fn boot() -> Value {
    serde_json::from_str(TRACE.lines().nth(1).unwrap()).unwrap()
}

fn world_inputs() -> World {
    let record: Value = CANONICAL
        .lines()
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .find(|r| r["t"] == "world")
        .unwrap();
    let int = |v: &Value| v.as_i64().unwrap() as i32;
    let mut world = World::default();
    for row in record["quests"].as_array().unwrap() {
        world.quests.register(
            int(&row[0]),
            quests::Quest {
                name: row[1].as_str().unwrap().to_owned(),
                index: int(&row[2]),
                button: int(&row[3]),
                points: int(&row[4]),
            },
        );
    }
    for row in record["songs"].as_array().unwrap() {
        world.songs.songs.insert(
            int(&row[0]),
            music::Song {
                name: row[1].as_str().unwrap().to_owned(),
                index: int(&row[2]),
            },
        );
    }
    world
}

fn xteas() -> HashMap<i32, [i32; 4]> {
    let data = java_json::parse(XTEAS).unwrap();
    let Some(Json::Array(rows)) = data.get("xteas").cloned() else {
        panic!("xteas.json has no xteas array")
    };
    let mut out = HashMap::new();
    for row in rows {
        let region =
            rs09_game_core::java_string::parse_i32(&row.get("regionId").unwrap().text()).unwrap();
        let keys: Vec<i32> = row
            .get("keys")
            .unwrap()
            .text()
            .split(',')
            .map(|k| rs09_game_core::java_string::parse_i32(k).unwrap())
            .collect();
        out.insert(region, [keys[0], keys[1], keys[2], keys[3]]);
    }
    out
}

/// What one native login wrote, against what the original queued.
#[derive(Debug)]
struct Compared {
    scenario: String,
    session: String,
    tick: i64,
    native: usize,
    /// Natively written entries by the original call site they match.
    sites: BTreeMap<String, usize>,
    replayed: Vec<(String, usize)>,
    /// The not-ported calls the native login made, whether or not the
    /// original queued anything from them.
    stand_ins: usize,
    mismatch: Option<String>,
    /// The canonical player's save text right after the login tick.
    save: String,
}

struct Run {
    compared: Vec<Compared>,
    events: Vec<(String, Vec<String>)>,
    players: Vec<(String, i64, Value, Value)>,
}

fn configuration() -> (Configuration, Constants, Location) {
    let mut config = Configuration::default();
    config.apply(&used_conf(), '/', None).unwrap();
    let c = config.constants();
    let loc = |key: &str| {
        let v: Vec<i32> = serde_json::from_value(c[key].clone()).unwrap();
        Location::new(v[0], v[1], v[2])
    };
    let world = config.world().unwrap();
    let constants = Constants {
        server_name: c["SERVER_NAME"].as_str().unwrap().to_owned(),
        world_id: world.world_id(),
        home_location: Some(loc("HOME_LOCATION")),
        daily_account_limit: c["DAILY_ACCOUNT_LIMIT"].as_i64().unwrap() as i32,
        config_path: c["CONFIG_PATH"].as_str().map(str::to_owned),
        rules_and_info_enabled: c["RULES_AND_INFO_ENABLED"].as_bool().unwrap(),
        message_model: world.values()["message_model"].as_i64().unwrap() as i32,
        message_string: world.values()["message_string"]
            .as_str()
            .unwrap()
            .to_owned(),
    };
    let start = loc("START_LOCATION");
    (config, constants, start)
}

fn run(scenario: &Scenario, seed: &[(String, Vec<u8>)]) -> Run {
    let (config, constants, start) = configuration();
    let header_clock = 1_689_163_200_000i64;
    NOW.with(|n| n.store(header_clock, Ordering::SeqCst));
    let directory = std::env::temp_dir().join(format!(
        "rs09-world-login-{}-{}-{}",
        std::process::id(),
        RUN.fetch_add(1, Ordering::SeqCst),
        scenario.name
    ));
    std::fs::create_dir_all(directory.join("data/players")).unwrap();
    for (path, bytes) in seed {
        std::fs::write(directory.join(path), bytes).unwrap();
    }
    let save_path = format!(
        "{}/data/players/",
        directory.to_string_lossy().replace('\\', "/")
    );
    let items = Arc::new(Items::default());
    let mut registry = interfaces::Registry::default();
    registry.load(INTERFACE_CONFIGS).unwrap();
    let state = State::default();
    let (varp, id, start_bit, end_bit) = SPELL_BOOK_VARBIT;
    state
        .identity()
        .varbits()
        .create(varp, id, start_bit, end_bit);
    let clock: Arc<Clock> = Arc::new(now);
    let mut binding = Binding {
        inner: Inner {
            state,
            frames: BTreeMap::new(),
            factory: Factory::new(&items),
            items,
            registry,
            options: interaction_options::Constants::default(),
            xteas: xteas(),
            constants,
            world: world_inputs(),
            hooks: boot()["listeners"]["persistPlayer"]
                .as_array()
                .unwrap()
                .len(),
            players: Vec::new(),
            sessions: Vec::new(),
            queue: Vec::new(),
            expected: HashMap::new(),
            replays: Vec::new(),
            calls: Vec::new(),
            pending: Vec::new(),
            ticks: 0,
            current: P(0),
            save_path,
            clock: clock.clone(),
            start,
        },
        social: social::Social::new(),
    };
    let repository: Repository<P> =
        Repository::new(repository::MAX_PLAYERS, repository::MAX_NPCS).unwrap();
    let auth_settings = config.auth_settings();
    let authenticator = Authenticator::new(
        Mode::Development {
            default_admin: auth_settings.default_admin,
        },
        Arc::new(InMemoryStorage::default()),
        Arc::new(Passwords::with_salt_source(4, || Ok([0; 16])).unwrap()),
    );
    let listeners: Vec<String> = boot()["listeners"]["login"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_owned())
        .collect();
    let key = RsaKey::reference_private();
    let mut out = Run {
        compared: Vec::new(),
        events: Vec::new(),
        players: Vec::new(),
    };
    // The login tick of each session: the tick its pulse runs.
    let mut login_ticks: HashMap<usize, i64> = HashMap::new();
    for record in &scenario.records {
        let v = &record.value;
        match v["t"].as_str().unwrap() {
            "boot" | "end" => continue,
            _ => {}
        }
        let op = v["op"].as_str().unwrap();
        match op {
            "connect" => {
                binding.inner.sessions.push(Session {
                    id: v["session"].as_str().unwrap().to_owned(),
                    active: true,
                    object: None,
                    game: false,
                    username: None,
                    direct: Vec::new(),
                    published: false,
                });
            }
            "recv" => {
                let session = binding
                    .inner
                    .sessions
                    .iter()
                    .position(|s| s.id == v["session"])
                    .unwrap();
                let bytes = unhex(v["hex"].as_str().unwrap());
                if bytes[0] == 14 {
                    // The handshake reply carries the original's server key,
                    // drawn from Math.random at its generator position: the
                    // transport's, not compared here.
                    let entries = &record.out[&binding.inner.sessions[session].id];
                    assert_eq!(entries.len(), 1);
                    continue;
                }
                let parsed = codec::decode(&bytes, &key, |name| {
                    repository
                        .get_player_by_name(&mut binding, Some(name))
                        .is_some()
                });
                assert_eq!(parsed.response, AuthResponse::Success);
                let name = parsed.info.as_ref().unwrap().username.clone();
                binding.inner.sessions[session].username = Some(name.clone());
                let decided = admission::check(parsed, &authenticator, &*clock);
                let Decision::Ready(authenticated) = decided else {
                    panic!("authentication refused")
                };
                let prepared = players::prepare(authenticated, clock.clone()).unwrap();
                assert_eq!(prepared.client.window_mode, 2);
                let opcode = prepared.login.opcode;
                let before = binding.social.pending().count();
                let admitted = login::admission::read(
                    &mut binding,
                    &repository,
                    &session,
                    prepared.details,
                    opcode,
                )
                .unwrap();
                assert!(
                    matches!(admitted, login::admission::Admitted::Submitted(_)),
                    "{admitted:?}"
                );
                binding.inner.sessions[session].published = true;
                // What the recv wrote: the original wrote nothing either.
                assert!(binding.inner.sessions[session].direct.is_empty());
                assert!(!record.out.contains_key(&binding.inner.sessions[session].id));
                let events: Vec<String> = binding
                    .social
                    .pending()
                    .skip(before)
                    .map(event_name)
                    .collect();
                let original: Vec<String> =
                    serde_json::from_value(v["handoffs"]["events"].clone()).unwrap();
                out.events.push((name, events.clone()));
                assert_eq!(events, original[..events.len()]);
                compare_players(&binding, v, &mut out);
            }
            "tick" => {
                NOW.with(|n| n.fetch_add(600, Ordering::SeqCst));
                binding.inner.ticks = v["tick"].as_i64().unwrap() as i32 - 1;
                let due = std::mem::take(&mut binding.inner.pending);
                if due.is_empty() {
                    // Not a login tick. Content (not ported) added artificial
                    // players on the first tick; their slots are taken from the trace.
                    add_artificial_players(&mut binding, &repository, v);
                    continue;
                }
                binding.inner.queue.clear();
                binding.inner.expected.clear();
                for pulse in &due {
                    let session = binding.inner.session_of(&pulse.player);
                    let id = binding.inner.sessions[session].id.clone();
                    let originals = record.out.get(&id).cloned().unwrap_or_default();
                    binding.inner.expected.insert(
                        session,
                        originals
                            .into_iter()
                            .filter(|o| o.origin.is_some())
                            .collect(),
                    );
                    login_ticks.insert(session, v["tick"].as_i64().unwrap());
                    // The management events login_events published were
                    // handled by ManagementEvents' handlers before this tick
                    // (not joined here): their packets are replayed first.
                    let p = pulse.player;
                    binding.inner.replay(&p, "ManagementEvents.handleEvent");
                }
                for pulse in &due {
                    let done =
                        login::pulse::run(&mut binding, &repository, &listeners, pulse).unwrap();
                    assert!(done);
                }
                for (session, _) in login_ticks.clone() {
                    if !due
                        .iter()
                        .any(|p| binding.inner.session_of(&p.player) == session)
                    {
                        continue;
                    }
                    let id = binding.inner.sessions[session].id.clone();
                    let originals = record.out.get(&id).cloned().unwrap_or_default();
                    let mut native: Vec<Out> =
                        std::mem::take(&mut binding.inner.sessions[session].direct);
                    native.extend(
                        binding
                            .inner
                            .queue
                            .iter()
                            .filter(|(s, _)| *s == session)
                            .map(|(_, o)| o.clone()),
                    );
                    let mismatch = first_mismatch(&native, &originals);
                    let mut sites = BTreeMap::new();
                    for (mine, original) in native.iter().zip(&originals) {
                        if mine.replay.is_none() {
                            *sites.entry(site(original)).or_insert(0) += 1;
                        }
                    }
                    let p = due
                        .iter()
                        .find(|p| binding.inner.session_of(&p.player) == session)
                        .unwrap()
                        .player;
                    let location = binding.inner.slot(&p).location.clone();
                    let (save, _) = binding.inner.environment_run(&p, |player, state, env| {
                        player.save_text(state, Some(&location), env)
                    });
                    let save = save.unwrap();
                    let replayed = binding
                        .inner
                        .replays
                        .iter()
                        .filter(|(s, _, _)| *s == session)
                        .map(|(_, m, n)| (m.clone(), *n))
                        .collect::<Vec<_>>();
                    out.compared.push(Compared {
                        scenario: scenario.name.clone(),
                        session: id,
                        tick: v["tick"].as_i64().unwrap(),
                        native: native.iter().filter(|o| o.replay.is_none()).count(),
                        sites,
                        stand_ins: replayed.len(),
                        replayed: replayed.into_iter().filter(|(_, n)| *n > 0).collect(),
                        mismatch,
                        save,
                    });
                    binding.inner.replays.retain(|(s, _, _)| *s != session);
                }
                binding.inner.queue.clear();
                compare_players(&binding, v, &mut out);
            }
            "close" => break,
            other => panic!("{other}"),
        }
    }
    let _ = std::fs::remove_dir_all(&directory);
    out
}

fn event_name(event: &social::Event) -> String {
    let text = format!("{event:?}");
    text.split([' ', '{', '(']).next().unwrap().to_owned()
}

fn add_artificial_players(binding: &mut Binding, repository: &Repository<P>, v: &Value) {
    for row in v["players"].as_array().unwrap() {
        if !row["artificial"].as_bool().unwrap() {
            continue;
        }
        let name = row["name"].as_str().unwrap().to_owned();
        if binding.inner.players.iter().any(|s| s.name == name) {
            continue;
        }
        let l = row["location"].as_array().unwrap();
        binding.inner.players.push(Slot {
            name,
            // Not in the uid map; addPlayer's scan compares it. AIPlayer sets
            // its own uid field, not the account's: the account's default.
            uid: 0,
            artificial: true,
            session: None,
            index: 0,
            active: true,
            playing: true,
            location: Location::new(
                l[0].as_i64().unwrap() as i32,
                l[1].as_i64().unwrap() as i32,
                l[2].as_i64().unwrap() as i32,
            ),
            canonical: None,
        });
        let p = P(binding.inner.players.len() - 1);
        // AIPlayer's constructor adds itself to Repository.players directly
        // (NodeList.add), not through addPlayer: no uid or name map entry.
        let index = repository.players().add(p).expect("a free slot");
        binding.inner.players[p.0].index = index;
        assert_eq!(index, row["slot"].as_i64().unwrap() as i32);
    }
}

/// The repository after a step, as the trace records `players`, for the
/// characters (artificial players are stand-ins taken from the trace).
fn compare_players(binding: &Binding, v: &Value, out: &mut Run) {
    let native: Vec<Value> = binding
        .inner
        .players
        .iter()
        .filter(|s| !s.artificial)
        .map(|s| {
            serde_json::json!({
                "slot": s.index, "index": s.index, "name": s.name,
                "location": s.location.coordinates(), "playing": s.playing, "active": s.active,
            })
        })
        .collect();
    let original: Vec<Value> = v["players"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| !r["artificial"].as_bool().unwrap())
        .map(|r| {
            serde_json::json!({
                "slot": r["slot"], "index": r["index"], "name": r["name"],
                "location": r["location"], "playing": r["playing"], "active": r["active"],
            })
        })
        .collect();
    out.players.push((
        format!("{} {}", v["op"].as_str().unwrap(), v["i"]),
        v["tick"].as_i64().unwrap(),
        Value::Array(native),
        Value::Array(original),
    ));
}

/// The original call site an entry came from: the login frame nearest the
/// queue (LoginConfiguration, LoginParser, Player.init) and the frame above it.
fn site(original: &Original) -> String {
    let Some(frames) = &original.origin else {
        return "direct (written at once)".into();
    };
    let at = frames.iter().position(|f| {
        f.contains("LoginConfiguration.") || f.contains("LoginParser$") || f.contains("Player.init")
    });
    match at {
        Some(0) | None => frames.first().cloned().unwrap_or_default(),
        Some(i) => format!("{} < {}", frames[i - 1], frames[i]),
    }
}

fn first_mismatch(native: &[Out], originals: &[Original]) -> Option<String> {
    for (n, original) in originals.iter().enumerate() {
        let Some(mine) = native.get(n) else {
            return Some(format!(
                "native ends at {n}; original continues with op {:?} {}",
                original.op, original.hex
            ));
        };
        if hex(&mine.bytes) != original.hex {
            return Some(format!(
                "entry {n}: native {} ({:?}), original {} op {:?} from {:?}",
                hex(&mine.bytes),
                mine.replay,
                original.hex,
                original.op,
                original
                    .origin
                    .as_ref()
                    .map(|f| f.iter().take(4).cloned().collect::<Vec<_>>())
            ));
        }
    }
    if native.len() > originals.len() {
        return Some(format!(
            "native has {} more entries",
            native.len() - originals.len()
        ));
    }
    None
}

fn seed_for(scenarios: &[Scenario], name: &str) -> Vec<(String, Vec<u8>)> {
    let source = scenarios.iter().find(|s| s.name == name).unwrap();
    let mut out = Vec::new();
    for record in &source.records {
        for f in record.value["files"].as_array().into_iter().flatten() {
            if let Some(h) = f["hex"].as_str() {
                out.push((f["path"].as_str().unwrap().to_owned(), unhex(h)));
            }
        }
    }
    out
}

/// What the native login of one character leaves to stand-ins: the
/// segments of the original's queued packets replayed for calls that are not
/// ported, by marker and size, in order (calls that queued nothing omitted).
type Segments = &'static [(&'static str, usize)];
const COMMON_HEAD: Segments = &[("ManagementEvents.handleEvent", 2)];
const COMMON_TAIL: Segments = &[
    ("content.minigame.puropuro.CropCircleController.login:", 1),
    ("WeaponInterface.open", 9),
    ("FriendsListInterface.open", 2),
    ("WeaponInterface", 6),
    ("LevelUp.sendFlashingIcons", 1),
    ("FamiliarManager.login", 3),
    ("WeaponInterface.open", 9),
    ("QuestRepository.syncronizeTab", 104),
];
/// (scenario, session, login tick, native entries, stand-in calls, emotes,
/// region move, extra login listener, save keys that differ from the
/// original's save at logout).
#[allow(clippy::type_complexity)]
const PINNED: &[(&str, &str, i64, usize, usize, usize, usize, bool, &[&str])] = &[
    (
        "new-character",
        "traceone",
        1,
        206,
        61,
        6,
        23,
        false,
        NEW_SAVE,
    ),
    (
        "two-characters",
        "traceone",
        1,
        206,
        61,
        6,
        23,
        false,
        NEW_SAVE,
    ),
    (
        "two-characters",
        "tracetwo",
        6,
        206,
        61,
        6,
        23,
        true,
        NEW_SAVE,
    ),
    (
        "returning-character",
        "traceone",
        1,
        206,
        61,
        3,
        2,
        false,
        RETURNING_SAVE,
    ),
];
const NEW_SAVE: &[&str] = &[
    "attributes",
    "barCrawl",
    "core_data",
    "emoteData",
    "ge-history",
    "globalData",
    "slayer",
    "treasureTrails",
    "unlockedMusic",
];
const RETURNING_SAVE: &[&str] = &[
    "barCrawl",
    "core_data",
    "ge-history",
    "slayer",
    "treasureTrails",
];

fn expected_segments(emotes: usize, region: usize, shooting_star: bool) -> Vec<(String, usize)> {
    let mut out: Vec<(String, usize)> = COMMON_HEAD
        .iter()
        .map(|(m, n)| (m.to_string(), *n))
        .collect();
    if shooting_star {
        out.push((
            "content.global.activity.shootingstar.ShootingStarPlugin.login:".into(),
            1,
        ));
    }
    out.extend(COMMON_TAIL.iter().map(|(m, n)| (m.to_string(), *n)));
    out.push(("EmoteManager.refresh".into(), emotes));
    out.push((
        "content.global.handlers.player.LoginValidationPlugin".into(),
        1,
    ));
    out.push(("RegionManager.move".into(), region));
    out.push(("MusicPlayer.init".into(), 22));
    out
}

/// Natively written entries by the original call site they match, the same
/// for every login of the corpus (206).
const NATIVE_SITES: &[(&str, usize)] = &[
    ("direct (written at once)", 3),
    (
        "core.api.ContentAPIKt.reinitVarps:1478 < core.game.node.entity.player.info.login.LoginParser$initialize$1.pulse:50",
        116,
    ),
    (
        "core.game.node.entity.skill.Skills.refresh:375 < core.game.node.entity.player.info.login.LoginConfiguration.config:239",
        24,
    ),
    (
        "core.game.node.entity.player.link.InterfaceManager.openDefaultTabs:409 < core.game.node.entity.player.info.login.LoginConfiguration.sendGameConfiguration:200",
        11,
    ),
    (
        "core.game.node.entity.player.link.InterfaceManager.openDefaultTabs:409 < core.game.node.entity.player.info.login.LoginConfiguration.config:245",
        11,
    ),
    (
        "core.game.node.entity.player.link.InterfaceManager.openChatbox:486 < core.game.node.entity.player.info.login.LoginConfiguration.sendGameConfiguration:199",
        4,
    ),
    (
        "core.game.node.Node.getInteraction:251 < core.game.node.entity.player.info.login.LoginConfiguration.config:242",
        3,
    ),
    (
        "core.game.container.Container.refresh:493 < core.game.node.entity.player.info.login.LoginConfiguration.config:237",
        2,
    ),
    (
        "core.game.container.Container.refresh:493 < core.game.node.entity.player.info.login.LoginConfiguration.config:238",
        2,
    ),
];

#[test]
fn native_login_writes_what_the_original_queued_during_the_login_ticks() {
    let scenarios = scenarios();
    let mut compared = 0;
    for name in ["new-character", "two-characters", "returning-character"] {
        let scenario = scenarios.iter().find(|s| s.name == name).unwrap();
        let seed = if name == "returning-character" {
            seed_for(&scenarios, "new-character")
        } else {
            Vec::new()
        };
        let run = run(scenario, &seed);
        let saves = seed_for(&scenarios, name);
        for c in &run.compared {
            eprintln!(
                "{} {} tick {}: {} native, {} replayed in {} stand-in calls {:?}; mismatch {:?}",
                c.scenario,
                c.session,
                c.tick,
                c.native,
                c.replayed.iter().map(|(_, n)| n).sum::<usize>(),
                c.stand_ins,
                c.replayed,
                c.mismatch
            );
            // Every byte, in order, against the original.
            assert!(
                c.mismatch.is_none(),
                "{} {}: {:?}",
                c.scenario,
                c.session,
                c.mismatch
            );
            let pin = PINNED
                .iter()
                .find(|p| p.0 == c.scenario && p.1 == c.session)
                .unwrap_or_else(|| panic!("no pin for {} {}", c.scenario, c.session));
            assert_eq!(c.tick, pin.2);
            assert_eq!(
                c.native, pin.3,
                "{} {}: natively written entries",
                c.scenario, c.session
            );
            assert_eq!(
                c.stand_ins, pin.4,
                "{} {}: stand-in calls",
                c.scenario, c.session
            );
            assert_eq!(
                c.replayed,
                expected_segments(pin.5, pin.6, pin.7),
                "{} {}",
                c.scenario,
                c.session
            );
            for (site, n) in NATIVE_SITES {
                assert_eq!(c.sites.get(*site), Some(n), "{site}");
            }
            let singles = c.sites.values().filter(|n| **n == 1).count();
            assert_eq!(c.sites.len(), NATIVE_SITES.len() + singles);
            assert_eq!(
                c.sites.values().sum::<usize>(),
                NATIVE_SITES.iter().map(|(_, n)| n).sum::<usize>() + singles
            );
            // The single sites: Settings.update (15), the windows pane, the
            // spell book's tab (twice) and update (two), the welcome, run
            // energy, both config texts, the five info bars, the interface config.
            assert_eq!(
                singles,
                15 + 1 + 2 + 2 + 1 + 1 + 2 + 5 + 1,
                "{:#?}",
                c.sites
            );
            let original = saves
                .iter()
                .find(|(path, _)| path.ends_with(&format!("/{}.json", c.session)))
                .map(|(_, b)| String::from_utf8(b.clone()).unwrap())
                .unwrap();
            let differing = differing_keys(&c.save, &original);
            if std::env::var_os("RS09_WORLD_LOGIN_VERBOSE").is_some() {
                let a: Value = serde_json::from_str(&c.save).unwrap();
                let b: Value = serde_json::from_str(&original).unwrap();
                for key in &differing {
                    if let (Some(x), Some(y)) = (a[key].as_object(), b[key].as_object()) {
                        for (sub, value) in y {
                            if x.get(sub) != Some(value) {
                                eprintln!(
                                    "      {key}.{sub}: native {:?} original {}",
                                    x.get(sub),
                                    value
                                );
                            }
                        }
                    } else {
                        eprintln!(
                            "      {key}: native {}\n      {key}: original {}",
                            a[key], b[key]
                        );
                    }
                }
            }
            assert_eq!(differing, pin.8, "{} {}: save keys", c.scenario, c.session);
            compared += 1;
        }
        // The repository after each recv and login tick: slot, index, name,
        // location, playing, active of every character.
        assert!(!run.players.is_empty());
        for (step, tick, native, original) in &run.players {
            assert_eq!(native, original, "{name} {step} tick {tick}");
        }
        // The management events login_events published, against the types the
        // original handled after the recv (its SendContactInfo is published by
        // the RequestContactInfo handler, which is not joined here).
        for (_, events) in &run.events {
            assert_eq!(events, &["PlayerStatusUpdate", "RequestContactInfo"]);
        }
    }
    assert_eq!(compared, 4);
}
/// The top-level save keys whose values differ.
fn differing_keys(native: &str, original: &str) -> Vec<String> {
    let a: Value = serde_json::from_str(native).unwrap();
    let b: Value = serde_json::from_str(original).unwrap();
    let (a, b) = (a.as_object().unwrap(), b.as_object().unwrap());
    let mut keys: Vec<String> = a
        .keys()
        .chain(b.keys())
        .filter(|k| a.get(*k) != b.get(*k))
        .cloned()
        .collect();
    keys.sort();
    keys.dedup();
    keys
}
/// The pinned spell-book varbit against the pinned cache, when it is present
/// (`cargo xtask cache-fetch`); without it this compares nothing.
#[test]
fn pinned_varbit_matches_the_cache() {
    use rs09_cache::{
        Limits,
        archive::Archive,
        store::{StorageMode, Store},
    };
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let directory = root.join("assets/cache");
    if !directory.join("main_file_cache.idx22").is_file() {
        eprintln!("SKIPPED: assets/cache is absent; the pinned varbit is not checked");
        return;
    }
    let store = Store::open(&directory, StorageMode::File, Limits::default()).unwrap();
    let mut archive = Arc::new(Archive::open(store, 22, None, Limits::default()).unwrap());
    let registry = rs09_server::definitions::varbit::Registry::default();
    let (varp, id, start, end) = SPELL_BOOK_VARBIT;
    let definition = registry.for_id(id, &mut archive).unwrap();
    let definition = definition.read();
    assert_eq!(
        (
            definition.varp_id,
            definition.id,
            definition.start_bit,
            definition.end_bit
        ),
        (varp, id, start, end)
    );
}
/// Unused helpers kept for the item registry shape the binding needs.
#[allow(dead_code)]
fn define(definitions: &mut Items, id: i32) {
    definitions.insert(
        id,
        Shared::new(ItemDefinition {
            id,
            ..Default::default()
        }),
    );
}
#[allow(dead_code)]
fn unused(_: PathBuf, _: player_walk::Node) {}
