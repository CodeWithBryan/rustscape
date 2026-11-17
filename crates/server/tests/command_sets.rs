//! Replays the command-sets corpus (fixtures/command-sets) through
//! `world::commands::sets` (Misc, Spawn, Stats), the native `::update`,
//! `::cancelupdate` and `::shutdown` bodies and `world::system_manager`.
//! Every service answers from the case the way the oracle's recording
//! adapters do (oracle/command-sets/adapters.py and Support.java), and the
//! attribute store keeps GameAttributes' key rules; every recorded call,
//! result, error and the state after each operation is compared.
use rs09_server::auth::account::{Account, UserAccountInfo};
use rs09_server::player::details::Rights;
use rs09_server::world::command_packets::{self, AnimatedObject, Camera, CameraType};
use rs09_server::world::commands::{
    self, Body, Command, Container, ContentBody, InputType, ItemRef, LegacyPlugin, Loc, NativeBody,
    Privilege, Registry, TeleportType,
    deferred::{InputHandler, InputValue, Job as CommandJob, Pulse as CommandPulse, Task},
    registration,
    sets::{
        self, Actor, Book, Chunk, Direction, Handler, Input, IronmanMode, Job, Level, Lock, Npc,
        Offer, PageSet, Scenery, SetBody, Timer, Value,
    },
};
use rs09_server::world::system_manager::{self, Manager, Persist, State};
use serde_json::{Map, Value as Json, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::{Arc, Mutex};

const CORPUS: &str = include_str!("../../../fixtures/command-sets/oracle.jsonl");
const PROVENANCE: &str = include_str!("../../../fixtures/command-sets/provenance.json");

type P = String;
type V = Value<P>;

fn int(v: &Json) -> i32 {
    v.as_i64().unwrap() as i32
}
fn location(v: &Json) -> Option<Loc> {
    v.as_array().map(|a| [int(&a[0]), int(&a[1]), int(&a[2])])
}
fn coord(l: Option<Loc>) -> Json {
    l.map_or(Json::Null, |l| json!(l))
}
fn binding() -> commands::Error {
    commands::Error::Binding(None)
}

/// `GameAttributes`: the raw map, the saved-key list and its key rules.
#[derive(Default)]
struct Attributes {
    map: HashMap<String, Option<V>>,
    saved: Vec<String>,
}
impl Attributes {
    fn set(&mut self, key: &str, value: Option<V>) {
        let mut key = key.to_owned();
        if let Some(rest) = key.strip_prefix("/save:") {
            key = rest.to_owned();
            if !self.saved.contains(&key) {
                self.saved.push(key.clone());
            }
        }
        self.map.insert(key, value);
    }
    fn get(&self, key: &str, fail: Option<V>) -> Option<V> {
        let key = key.replace("/save:", "");
        match self.map.get(&key) {
            Some(Some(v)) => Some(v.clone()),
            _ => fail,
        }
    }
    fn get_raw(&self, key: &str) -> Option<V> {
        self.map.get(&key.replace("/save:", "")).cloned().flatten()
    }
    fn remove(&mut self, key: &str) {
        if let Some(i) = self.saved.iter().position(|k| k == key) {
            self.saved.remove(i);
        }
        self.map.remove(key);
    }
}

struct PlayerData {
    username: String,
    account: Account,
    location: Option<Loc>,
    direction: Direction,
    last_scene: Option<Loc>,
    version: i32,
    test_stage: i32,
    read_plaques: Vec<bool>,
    boss_counters: Vec<i32>,
    barrows: i32,
    diaries: Vec<Vec<i32>>,
    packet_count: i32,
    attributes: Attributes,
}

struct NpcData {
    key: String,
    id: i32,
    location: Option<Loc>,
    direction: Direction,
    walks: bool,
}

#[derive(Clone)]
enum Pending {
    Command,
    Set(Input<P>),
}

struct World {
    values: Map<String, Json>,
    answers: Map<String, Json>,
    answer_calls: HashMap<String, usize>,
    players: BTreeMap<String, PlayerData>,
    accounts: Vec<(String, Account)>,
    npcs: Vec<NpcData>,
    sceneries: u32,
    object_keys: Vec<String>,
    events: Vec<Json>,
    op_calls: i64,
    fail_at: i64,
    clock_reads: usize,
    pulses: usize,
    inputs: Vec<Pending>,
    jobs: Vec<Job<P>>,
    books: Vec<(P, Book)>,
    manager: Option<Manager>,
    worker_started: bool,
    websocket: bool,
    dirs: BTreeSet<String>,
}

fn value_json(v: Option<&V>) -> Json {
    match v {
        None => Json::Null,
        Some(Value::Int(i)) => json!({"int": i}),
        Some(Value::Long(i)) => json!({"long": i}),
        Some(Value::Bool(b)) => json!({"bool": b}),
        Some(Value::Text(t)) => json!({"text": t}),
        Some(Value::Rights(r)) => json!({"rights": sets::misc::rights_name(*r)}),
        Some(Value::List(l)) => json!({"list": l.lock().unwrap().clone()}),
        Some(Value::Player(p)) => json!({"player": p}),
    }
}

fn attribute(spec: &Json) -> Option<V> {
    let m = spec.as_object()?;
    Some(if let Some(v) = m.get("int") {
        Value::Int(int(v))
    } else if let Some(v) = m.get("long") {
        Value::Long(v.as_i64().unwrap())
    } else if let Some(v) = m.get("bool") {
        Value::Bool(v.as_bool().unwrap())
    } else if let Some(v) = m.get("text") {
        Value::Text(v.as_str().unwrap().into())
    } else if let Some(v) = m.get("rights") {
        Value::Rights(match v.as_str().unwrap() {
            "REGULAR_PLAYER" => Rights::RegularPlayer,
            "PLAYER_MODERATOR" => Rights::PlayerModerator,
            _ => Rights::Administrator,
        })
    } else if let Some(v) = m.get("list") {
        Value::List(Arc::new(Mutex::new(
            v.as_array()
                .unwrap()
                .iter()
                .map(|s| s.as_str().unwrap().to_owned())
                .collect(),
        )))
    } else if let Some(v) = m.get("player") {
        Value::Player(v.as_str().unwrap().into())
    } else {
        panic!("attribute {spec}")
    })
}

/// `String.valueOf` of a recorded argument, for answer keys.
fn java_text(v: &Json) -> String {
    match v {
        Json::Null => "null".into(),
        Json::String(s) => s.clone(),
        Json::Array(a) => format!(
            "[{}]",
            a.iter().map(java_text).collect::<Vec<_>>().join(", ")
        ),
        other => other.to_string(),
    }
}

fn direction(name: &str) -> Direction {
    *Direction::ALL.iter().find(|d| d.name() == name).unwrap()
}

impl World {
    fn new(case: &Json) -> Self {
        let mut accounts = Vec::new();
        for (key, a) in case["accounts"].as_object().unwrap() {
            let mut info = UserAccountInfo::create_default(0);
            if let Some(v) = a.get("username") {
                info.username = v.as_str().unwrap().into();
            }
            if let Some(v) = a.get("password") {
                info.password = v.as_str().unwrap().into();
            }
            if let Some(v) = a.get("rights") {
                info.rights = int(v);
            }
            if let Some(v) = a.get("credits") {
                info.credits = int(v);
            }
            accounts.push((key.clone(), info.shared()));
        }
        let ints = |v: &Json| -> Vec<i32> {
            v.as_array()
                .map(|a| a.iter().map(int).collect())
                .unwrap_or_default()
        };
        let mut players = BTreeMap::new();
        for (key, p) in case["players"].as_object().unwrap() {
            let account = accounts
                .iter()
                .find(|(k, _)| Some(k.as_str()) == p["account"].as_str())
                .map(|(_, a)| a.clone())
                .unwrap();
            let mut attributes = Attributes::default();
            if let Some(m) = p.get("attributes").and_then(Json::as_object) {
                for (k, v) in m {
                    attributes.set(k, attribute(v));
                }
            }
            players.insert(
                key.clone(),
                PlayerData {
                    username: p["username"].as_str().unwrap().into(),
                    account,
                    location: location(&p["location"]),
                    direction: p["direction"].as_str().map_or(Direction::South, direction),
                    last_scene: location(&p["lastSceneGraph"]),
                    version: p["version"].as_i64().unwrap_or(0) as i32,
                    test_stage: p["testStage"].as_i64().unwrap_or(0) as i32,
                    read_plaques: p["readPlaques"]
                        .as_array()
                        .map(|a| a.iter().map(|b| b == &json!(true)).collect())
                        .unwrap_or_default(),
                    boss_counters: ints(&p["bossCounters"]),
                    barrows: p["barrowsLoots"].as_i64().unwrap_or(0) as i32,
                    diaries: p["diaries"]
                        .as_array()
                        .map(|a| a.iter().map(ints).collect())
                        .unwrap_or_default(),
                    packet_count: p["packetCount"].as_i64().unwrap_or(0) as i32,
                    attributes,
                },
            );
        }
        let values = case["values"].as_object().cloned().unwrap_or_default();
        let dirs = values
            .get("existingDirs")
            .and_then(Json::as_array)
            .map(|a| a.iter().map(|d| d.as_str().unwrap().to_owned()).collect())
            .unwrap_or_default();
        Self {
            worker_started: values.get("workerStarted") != Some(&json!(false)),
            websocket: values.get("websocket") == Some(&json!(true)),
            values,
            answers: case["answers"].as_object().cloned().unwrap_or_default(),
            answer_calls: HashMap::new(),
            players,
            accounts,
            npcs: Vec::new(),
            sceneries: 0,
            object_keys: Vec::new(),
            events: Vec::new(),
            op_calls: 0,
            fail_at: -1,
            clock_reads: 0,
            pulses: 0,
            inputs: Vec::new(),
            jobs: Vec::new(),
            books: Vec::new(),
            manager: Some(Manager::new()),
            dirs,
        }
    }
}

impl World {
    fn note(&mut self, event: Json) {
        self.events.push(event);
    }
    fn emit(&mut self, event: Json) -> Result<(), ()> {
        self.events.push(event);
        self.op_calls += 1;
        if self.op_calls == self.fail_at {
            Err(())
        } else {
            Ok(())
        }
    }
    /// `Support.answer`: the longest argument prefix the case names.
    fn answer(&mut self, name: &str, args: &[Json]) -> Option<Json> {
        for k in (0..=args.len()).rev() {
            let mut key = name.to_owned();
            for a in &args[..k] {
                key.push(':');
                key.push_str(&java_text(a));
            }
            if let Some(v) = self.answers.get(&key).cloned() {
                if let Json::Array(list) = &v {
                    let calls = self.answer_calls.entry(key).or_default();
                    let i = *calls;
                    *calls += 1;
                    return Some(list[i.min(list.len() - 1)].clone());
                }
                return Some(v);
            }
        }
        None
    }
    /// `Support.ask`: the call, then its answer.
    fn ask(&mut self, name: &str, args: Vec<Json>) -> commands::Result<Option<Json>> {
        let mut event = vec![json!(name)];
        event.extend(args.iter().cloned());
        self.emit(Json::Array(event)).map_err(|_| binding())?;
        Ok(self.answer(name, &args))
    }
    fn ask_bool(&mut self, name: &str, args: Vec<Json>, fallback: bool) -> commands::Result<bool> {
        Ok(self.ask(name, args)?.map_or(fallback, |v| v == json!(true)))
    }
    fn ask_int(&mut self, name: &str, args: Vec<Json>, fallback: i64) -> commands::Result<i64> {
        Ok(self
            .ask(name, args)?
            .map_or(fallback, |v| v.as_i64().unwrap()))
    }
    fn ask_text(
        &mut self,
        name: &str,
        args: Vec<Json>,
        fallback: String,
    ) -> commands::Result<String> {
        Ok(self
            .ask(name, args)?
            .map_or(fallback, |v| v.as_str().unwrap().to_owned()))
    }
    fn list(&self, name: &str) -> Vec<Json> {
        self.values
            .get(name)
            .and_then(Json::as_array)
            .cloned()
            .unwrap_or_default()
    }
    fn number(&self, name: &str, fallback: i32) -> i32 {
        self.values.get(name).map_or(fallback, int)
    }
    fn tick(&mut self) -> i64 {
        self.note(json!(["clock"]));
        let clock = self.list("clock");
        if clock.is_empty() {
            return 0;
        }
        let i = self.clock_reads.min(clock.len() - 1);
        self.clock_reads += 1;
        clock[i].as_i64().unwrap()
    }
    fn npc_key(&self, npc: Npc) -> String {
        self.npcs[npc.0 as usize].key.clone()
    }
    fn actor(&self, actor: &Actor<P>) -> Json {
        match actor {
            Actor::Player(p) => json!(p),
            Actor::Npc(n) => json!(self.npc_key(*n)),
        }
    }
    fn placed(&mut self, row: &Json) -> Option<Scenery> {
        let r = row.as_array()?;
        let id = int(&r[0]);
        self.object_keys.push(format!("placed{id}"));
        Some(Scenery {
            handle: (self.object_keys.len() - 1) as u32,
            id,
            location: location(&r[1]),
            kind: int(&r[2]),
            rotation: int(&r[3]),
            constructed: r[4] == json!(true),
            renderable: r[5] == json!(true),
            active: r[6] == json!(true),
        })
    }
    fn offers(&self, name: &str) -> Vec<Offer> {
        self.list(name)
            .iter()
            .map(|r| Offer {
                item: int(&r[0]),
                amount: int(&r[1]),
                offered_value: int(&r[2]),
                sell: r[3] == json!(true),
                amount_left: int(&r[4]),
            })
            .collect()
    }
    fn state(&self) -> Json {
        let accounts: Map<String, Json> = self
            .accounts
            .iter()
            .map(|(k, a)| {
                let a = a.lock().unwrap();
                (k.clone(), json!([a.rights, a.credits]))
            })
            .collect();
        let players: Map<String, Json> = self
            .players
            .iter()
            .map(|(k, p)| {
                let attrs: Map<String, Json> = p
                    .attributes
                    .map
                    .iter()
                    .map(|(k, v)| (k.clone(), value_json(v.as_ref())))
                    .collect();
                (
                    k.clone(),
                    json!([
                        attrs,
                        p.attributes.saved,
                        [p.version, p.test_stage, p.read_plaques]
                    ]),
                )
            })
            .collect();
        let m = self.manager.as_ref().unwrap();
        let dirs: Vec<bool> = self
            .list("cleanDirs")
            .iter()
            .map(|d| self.dirs.contains(d.as_str().unwrap()))
            .collect();
        json!({
            "accounts": accounts,
            "players": players,
            "system": [m.state().name(), m.updater.delay(), m.updater.ticks_passed(), m.updater.is_running(),
                       m.updater.create_backup, self.worker_started],
            "pending": [self.pulses, self.inputs.len(), self.jobs.len(), self.books.len()],
            "dirs": dirs,
        })
    }
    fn with_manager<T>(&mut self, f: impl FnOnce(&mut Manager, &mut World) -> T) -> T {
        let mut manager = self.manager.take().expect("the manager is not re-entered");
        let out = f(&mut manager, self);
        self.manager = Some(manager);
        out
    }
}

macro_rules! emit {
    ($self:ident, $($e:tt)*) => {
        $self.emit(json!([$($e)*])).map_err(|_| binding())?
    };
}

macro_rules! unported {
    ($($name:ident($($arg:ty),*) -> $ret:ty;)*) => {
        $(fn $name(&mut self, $(_: $arg),*) -> $ret {
            unreachable!(concat!(stringify!($name), " is not reached by these sets"))
        })*
    };
}

impl commands::Host for World {
    type Player = P;
    fn player_commands(&mut self) -> bool {
        self.values["playerCommands"] == json!(true)
    }
    fn i_am_a_cheater(&mut self) -> bool {
        self.values["cheater"] == json!(true)
    }
    fn dev_mode(&mut self) -> Option<bool> {
        self.values["settings"]
            .as_object()
            .map(|s| s["devMode"] == json!(true))
    }
    fn beta(&mut self) -> Option<bool> {
        self.values["settings"]
            .as_object()
            .map(|s| s["beta"] == json!(true))
    }
    fn home_location(&mut self) -> Option<Loc> {
        location(&self.values["home"])
    }
    fn data_path(&mut self) -> Option<String> {
        self.values["dataPath"].as_str().map(str::to_owned)
    }
    fn now(&mut self) -> i64 {
        self.tick()
    }
    fn account(&mut self, player: &P) -> Account {
        self.players[player].account.clone()
    }
    fn username(&mut self, player: &P) -> String {
        self.players[player].username.clone()
    }
    fn location(&mut self, player: &P) -> commands::Result<Option<Loc>> {
        Ok(self.players[player].location)
    }
    fn message(&mut self, player: &P, text: &str) -> commands::Result<()> {
        emit!(self, "message", player, text);
        Ok(())
    }
    fn api_message(&mut self, player: &P, text: &str) -> commands::Result<()> {
        emit!(self, "apiMessage", player, text);
        Ok(())
    }
    fn send_chat(&mut self, player: &P, text: &str) -> commands::Result<()> {
        emit!(self, "chat", player, text);
        Ok(())
    }
    fn dialogue(&mut self, player: &P, text: &str) -> commands::Result<()> {
        emit!(self, "dialogue", player, text);
        Ok(())
    }
    fn input_dialogue(
        &mut self,
        player: &P,
        kind: InputType,
        prompt: &str,
        handler: InputHandler<P>,
    ) -> commands::Result<()> {
        emit!(self, "input", player, kind.name(), prompt);
        let _ = handler;
        self.inputs.push(Pending::Command);
        Ok(())
    }
    fn zone_parse_command(
        &mut self,
        player: &P,
        name: &str,
        arguments: &[String],
    ) -> commands::Result<bool> {
        self.ask_bool(
            "zone",
            vec![json!(player), json!(name), json!(arguments)],
            false,
        )
    }
    fn content_command(
        &mut self,
        body: ContentBody,
        player: &P,
        args: &[String],
    ) -> commands::Result<()> {
        emit!(self, "content", body.class, body.lambda, player, args);
        Ok(())
    }
    fn player_by_name(&mut self, name: Option<&str>) -> commands::Result<Option<P>> {
        let key = self.ask("byName", vec![json!(name)])?;
        Ok(key
            .and_then(|k| k.as_str().map(str::to_owned))
            .filter(|k| self.players.contains_key(k)))
    }
    fn clear(&mut self, player: &P) -> commands::Result<()> {
        emit!(self, "clear", player);
        Ok(())
    }
    /// `SystemManager.getUpdater().setCountdown(ticks)`: the native updater.
    fn set_update_countdown(&mut self, seconds: i32) -> commands::Result<()> {
        self.manager
            .as_mut()
            .unwrap()
            .updater
            .set_countdown(seconds);
        Ok(())
    }
    /// `SystemManager.flag(UPDATING)`: the native manager.
    fn flag_updating(&mut self) -> commands::Result<()> {
        self.with_manager(|m, w| m.flag(w, State::Updating))
    }
    /// `SystemManager.getUpdater().cancel()`: the native manager.
    fn cancel_update(&mut self) -> commands::Result<()> {
        self.with_manager(|m, w| m.cancel(w))
    }
    /// `exitProcess(status)`: the oracle's security manager turns
    /// System.exit into an outcome; a world runs the shutdown hook
    /// (`Manager::shutdown_hook`) and exits.
    fn exit_process(&mut self, status: i32) -> commands::Result<()> {
        Err(commands::Error::Binding(Some(format!("exit {status}"))))
    }
    fn add(&mut self, player: &P, c: Container, id: i32, amount: i32) -> commands::Result<bool> {
        self.ask_bool(
            "add",
            vec![json!(player), json!(c.name()), json!(id), json!(amount)],
            true,
        )
    }
    fn sets(&mut self) -> commands::Result<&mut dyn sets::Host<Player = P>> {
        Ok(self)
    }
    unported! {
        castle_wars_destination() -> Loc;
        run_task(&P, i32, i32, Task<P>) -> commands::Result<()>;
        monitor_log(&P, &str) -> commands::Result<()>;
        legacy_parse(LegacyPlugin, &P, &str, &[String]) -> commands::Result<bool>;
        attribute(&P, &str) -> commands::Result<Option<commands::Value>>;
        api_attribute(&P, &str, commands::Value) -> commands::Result<Option<commands::Value>>;
        set_attribute(&P, &str, Option<commands::Value>) -> commands::Result<()>;
        teleport(&P, Loc, TeleportType) -> commands::Result<bool>;
        set_teleport_location(&P, Option<Loc>) -> commands::Result<()>;
        username_taken(&str) -> commands::Result<bool>;
        account_info(Option<&str>) -> commands::Result<Account>;
        account_update(&Account) -> commands::Result<()>;
        usernames_with_ip(&str) -> commands::Result<Vec<Option<String>>>;
        check_password(&P, &str) -> commands::Result<bool>;
        update_password(&str, &str) -> commands::Result<()>;
        store_add_to_list(&str, &str, &str) -> commands::Result<()>;
        submit_pulse(CommandPulse<P>) -> commands::Result<()>;
        launch(CommandJob<P>) -> commands::Result<()>;
        file_lines(Option<&str>, &str) -> commands::Result<Option<Vec<String>>>;
        item_definitions_size() -> commands::Result<i32>;
        item_name(i32) -> commands::Result<String>;
        note_change(i32) -> commands::Result<i32>;
        is_full(&P, Container) -> commands::Result<bool>;
        remove(&P, Container, i32, i32) -> commands::Result<bool>;
        item_amount(&P, Container, i32, i32) -> commands::Result<Option<i32>>;
        amount(&P, Container, i32) -> commands::Result<i32>;
        amount_item(&P, Container, i32) -> commands::Result<i32>;
        slots(&P, Container) -> commands::Result<Vec<Option<ItemRef>>>;
        get(&P, Container, i32) -> commands::Result<Option<ItemRef>>;
        set_charge(&P, Container, &ItemRef, i32) -> commands::Result<()>;
        replace(&P, Container, i32, i32) -> commands::Result<()>;
        charged_item(i32) -> commands::Result<bool>;
        charged_for_charge(i32, i32) -> commands::Result<i32>;
        charged_charge(i32) -> commands::Result<Option<i32>>;
    }
}

fn lock_name(lock: Lock) -> &'static str {
    match lock {
        Lock::Movement => "movementLocked",
        Lock::Teleport => "teleportLocked",
        Lock::Interaction => "interactionLocked",
    }
}

fn opt_int(v: Option<Json>) -> Option<i32> {
    v.filter(|v| !v.is_null()).map(|v| int(&v))
}

impl sets::Host for World {
    fn bank_size(&mut self) -> i32 {
        self.number("bankSize", 496)
    }
    fn new_player_announcement(&mut self) -> bool {
        self.values.get("newPlayerAnnouncement") == Some(&json!(true))
    }
    fn online_players(&mut self) -> Vec<P> {
        self.list("online")
            .iter()
            .filter_map(|k| k.as_str().map(str::to_owned))
            .filter(|k| self.players.contains_key(k))
            .collect()
    }
    fn direction(&mut self, player: &P) -> Direction {
        self.players[player].direction
    }
    fn version(&mut self, player: &P) -> i32 {
        self.players[player].version
    }
    fn last_scene_graph(&mut self, player: &P) -> Option<Loc> {
        self.players[player].last_scene
    }
    fn diaries(&mut self, player: &P) -> Vec<Vec<i32>> {
        self.players[player].diaries.clone()
    }
    fn test_stage(&mut self, player: &P) -> i32 {
        self.players[player].test_stage
    }
    fn set_test_stage(&mut self, player: &P, stage: i32) {
        self.players.get_mut(player).unwrap().test_stage = stage;
    }
    fn read_plaques(&mut self, player: &P) -> Vec<bool> {
        self.players[player].read_plaques.clone()
    }
    fn set_read_plaque(&mut self, player: &P, index: usize, read: bool) {
        self.players.get_mut(player).unwrap().read_plaques[index] = read;
    }
    fn boss_counters(&mut self, player: &P) -> Vec<i32> {
        self.players[player].boss_counters.clone()
    }
    fn barrows_loots(&mut self, player: &P) -> i32 {
        self.players[player].barrows
    }
    fn entity_attribute(
        &mut self,
        player: &P,
        key: &str,
        fail: Option<V>,
    ) -> commands::Result<Option<V>> {
        emit!(self, "getAttribute", player, key, value_json(fail.as_ref()));
        let attributes = &self.players[player].attributes;
        Ok(match fail {
            None => attributes.get_raw(key),
            Some(fail) => attributes.get(key, Some(fail)),
        })
    }
    fn entity_set_attribute(
        &mut self,
        player: &P,
        key: &str,
        value: Option<V>,
    ) -> commands::Result<()> {
        emit!(
            self,
            "setAttribute",
            player,
            key,
            value_json(value.as_ref())
        );
        self.players
            .get_mut(player)
            .unwrap()
            .attributes
            .set(key, value);
        Ok(())
    }
    fn entity_remove_attribute(&mut self, player: &P, key: &str) -> commands::Result<()> {
        emit!(self, "removeAttribute", player, key);
        self.players.get_mut(player).unwrap().attributes.remove(key);
        Ok(())
    }
    fn has_attribute(&mut self, player: &P, key: &str) -> commands::Result<bool> {
        emit!(self, "hasAttribute", player, key);
        Ok(self.players[player].attributes.map.contains_key(key))
    }
    fn dispatch_message(&mut self, player: &P, text: &str) -> commands::Result<()> {
        emit!(self, "packetMessage", player, text);
        Ok(())
    }
    fn send_string(
        &mut self,
        player: &P,
        text: &str,
        interface: i32,
        child: i32,
    ) -> commands::Result<()> {
        emit!(self, "sendString", player, text, interface, child);
        Ok(())
    }
    fn send_varc(&mut self, player: &P, index: i16, value: i32) -> commands::Result<()> {
        emit!(self, "varc", player, index, value);
        Ok(())
    }
    fn send_interface_config(
        &mut self,
        player: &P,
        interface: i32,
        child: i32,
        hidden: bool,
    ) -> commands::Result<()> {
        emit!(self, "interfaceConfig", player, interface, child, hidden);
        Ok(())
    }
    fn interface_opened(&mut self, player: &P) -> commands::Result<bool> {
        self.ask_bool("isOpened", vec![json!(player)], false)
    }
    fn opened_component(&mut self, player: &P) -> commands::Result<Option<i32>> {
        Ok(opt_int(self.ask("opened", vec![json!(player)])?))
    }
    fn open_component(&mut self, player: &P, id: i32) -> commands::Result<()> {
        emit!(self, "open", player, id);
        Ok(())
    }
    fn close_interfaces(&mut self, player: &P) -> commands::Result<bool> {
        self.ask_bool("close", vec![json!(player)], true)
    }
    fn close_dialogue(&mut self, player: &P) -> commands::Result<bool> {
        self.ask_bool("closeDialogue", vec![json!(player)], true)
    }
    fn locked(&mut self, player: &P, lock: Lock) -> commands::Result<bool> {
        self.ask_bool(lock_name(lock), vec![json!(player)], false)
    }
    fn is_artificial(&mut self, player: &P) -> commands::Result<bool> {
        self.ask_bool("artificial", vec![json!(player)], false)
    }
    fn swing_handler(&mut self, player: &P, swing: bool) -> commands::Result<Handler> {
        let label = self.ask_text(
            "swingHandler",
            vec![json!(player), json!(swing)],
            "melee".into(),
        )?;
        Ok(match label.as_str() {
            "range" => Handler::Range,
            "magic" => Handler::Magic,
            "ma_melee" => Handler::MagicalMelee,
            "ma_ranged" => Handler::MagicalRanged,
            "ra_melee" => Handler::RangedMelee,
            _ => Handler::Melee,
        })
    }
    fn level(&mut self, actor: &Actor<P>, skill: i32) -> commands::Result<i32> {
        let key = self.actor(actor);
        Ok(self.ask_int("level", vec![key, json!(skill)], 1)? as i32)
    }
    fn set_version(&mut self, player: &P, version: i32) -> commands::Result<()> {
        emit!(self, "setVersion", player, version);
        self.players.get_mut(player).unwrap().version = version;
        Ok(())
    }
    fn unlock(&mut self, player: &P) -> commands::Result<()> {
        emit!(self, "unlock", player);
        Ok(())
    }
    fn ip_address(&mut self, player: &P) -> commands::Result<String> {
        self.ask_text("ipAddress", vec![json!(player)], "127.0.0.1".into())
    }
    fn comp_name(&mut self, player: &P) -> commands::Result<String> {
        self.ask_text("compName", vec![json!(player)], "pc".into())
    }
    fn finish_task(
        &mut self,
        player: &P,
        diary: i32,
        level: i32,
        task: i32,
    ) -> commands::Result<()> {
        emit!(self, "finishTask", player, diary, level, task);
        Ok(())
    }
    fn quest_points(&mut self, player: &P) -> commands::Result<i32> {
        Ok(self.ask_int("questPoints", vec![json!(player)], 0)? as i32)
    }
    fn ironman_mode(&mut self, player: &P) -> commands::Result<IronmanMode> {
        let name = self.ask_text("ironmanMode", vec![json!(player)], "NONE".into())?;
        Ok(match name.as_str() {
            "STANDARD" => IronmanMode::Standard,
            "HARDCORE" => IronmanMode::Hardcore,
            "ULTIMATE" => IronmanMode::Ultimate,
            _ => IronmanMode::None,
        })
    }
    fn slayer_completed_tasks(&mut self, player: &P) -> commands::Result<i32> {
        Ok(self.ask_int("slayerCompleted", vec![json!(player)], 0)? as i32)
    }
    fn completed_clues(&mut self, player: &P) -> commands::Result<Vec<i32>> {
        emit!(self, "clues", player);
        Ok(self
            .list(&format!("clues:{player}"))
            .iter()
            .map(int)
            .collect())
    }
    fn stat_kills(&mut self, player: &P, npcs: &[i32]) -> commands::Result<i64> {
        self.ask_int("kills", vec![json!(player), json!(npcs)], 0)
    }
    fn stat_rare_drops(&mut self, player: &P, item: i32) -> commands::Result<i64> {
        self.ask_int("rareDrops", vec![json!(player), json!(item)], 0)
    }
    fn container_clear(&mut self, player: &P, c: Container) -> commands::Result<()> {
        emit!(self, "clear", player, c.name());
        Ok(())
    }
    fn container_refresh(&mut self, player: &P, c: Container) -> commands::Result<()> {
        emit!(self, "refresh", player, c.name());
        Ok(())
    }
    fn container_update(&mut self, player: &P, c: Container) -> commands::Result<()> {
        emit!(self, "update", player, c.name());
        Ok(())
    }
    fn maximum_add(
        &mut self,
        player: &P,
        c: Container,
        id: i32,
        amount: i32,
    ) -> commands::Result<i32> {
        let args = vec![json!(player), json!(c.name()), json!(id), json!(amount)];
        Ok(self.ask_int("maximumAdd", args, i64::from(i32::MAX))? as i32)
    }
    fn send_news(&mut self, text: &str, icon: i32, color: &str) -> commands::Result<()> {
        emit!(self, "news", text, icon, color);
        Ok(())
    }
    fn item_definition_name(&mut self, id: i32) -> String {
        self.answer("itemDefName", &[json!(id)])
            .map_or(format!("item {id}"), |v| v.as_str().unwrap().into())
    }
    fn add_item_or_drop(&mut self, player: &P, id: i32, amount: i32) -> commands::Result<()> {
        emit!(self, "addItemOrDrop", player, id, amount);
        Ok(())
    }
    fn set_varp(&mut self, player: &P, index: i32, value: i32, save: bool) -> commands::Result<()> {
        emit!(self, "setVarp", player, index, value, save);
        Ok(())
    }
    fn set_varbit(&mut self, player: &P, id: i32, value: i32, save: bool) -> commands::Result<()> {
        emit!(self, "setVarbit", player, id, value, save);
        Ok(())
    }
    fn get_varbit(&mut self, player: &P, id: i32) -> commands::Result<i32> {
        Ok(self.ask_int("getVarbit", vec![json!(player), json!(id)], 0)? as i32)
    }
    fn start_timer(&mut self, player: &P, timer: Timer) -> commands::Result<()> {
        emit!(self, "timer", player, timer.simple_name());
        Ok(())
    }
    fn patches(&mut self, player: &P) -> commands::Result<i32> {
        emit!(self, "patches", player);
        Ok(self.number("patches", 0))
    }
    fn set_next_growth(&mut self, player: &P, patch: i32, value: i64) -> commands::Result<()> {
        emit!(self, "nextGrowth", player, patch, value);
        Ok(())
    }
    fn set_disease_mod(&mut self, player: &P, patch: i32, value: i32) -> commands::Result<()> {
        emit!(self, "diseaseMod", player, patch, value);
        Ok(())
    }
    fn crop_run(&mut self, player: &P) -> commands::Result<bool> {
        self.ask_bool("cropRun", vec![json!(player)], true)
    }
    fn bins(&mut self, player: &P) -> commands::Result<i32> {
        emit!(self, "bins", player);
        Ok(self.number("bins", 0))
    }
    fn bin_finished(&mut self, player: &P, bin: i32) -> commands::Result<bool> {
        self.ask_bool("binFinished", vec![json!(player), json!(bin)], false)
    }
    fn bin_closed(&mut self, player: &P, bin: i32) -> commands::Result<bool> {
        self.ask_bool("binClosed", vec![json!(player), json!(bin)], false)
    }
    fn bin_finish(&mut self, player: &P, bin: i32) -> commands::Result<()> {
        emit!(self, "binFinish", player, bin);
        Ok(())
    }
    fn bin_reset(&mut self, player: &P, bin: i32) -> commands::Result<()> {
        emit!(self, "binReset", player, bin);
        Ok(())
    }
    fn vats(&mut self, player: &P) -> commands::Result<i32> {
        emit!(self, "vats", player);
        Ok(self.number("vats", 0))
    }
    fn brew(&mut self, player: &P, vat: i32, force: [bool; 4]) -> commands::Result<()> {
        emit!(
            self, "brew", player, vat, force[0], force[1], force[2], force[3]
        );
        Ok(())
    }
    fn log(&mut self, origin: &str, level: Level, message: &str) -> commands::Result<()> {
        emit!(self, "log", origin, level.name(), message);
        Ok(())
    }
    fn input(
        &mut self,
        player: &P,
        kind: InputType,
        prompt: &str,
        handler: Input<P>,
    ) -> commands::Result<()> {
        emit!(self, "input", player, kind.name(), prompt);
        self.inputs.push(Pending::Set(handler));
        Ok(())
    }
    fn launch_job(&mut self, job: Job<P>) -> commands::Result<()> {
        emit!(self, "launch");
        self.jobs.push(job);
        Ok(())
    }
    fn start_cutscene(&mut self, player: &P, name: &str) -> commands::Result<()> {
        emit!(self, "cutscene", name, player);
        Ok(())
    }
    fn permadeath(&mut self, player: &P) -> commands::Result<()> {
        emit!(self, "permadeath", player);
        Ok(())
    }
    fn trawler_loot(
        &mut self,
        player: &P,
        level: i32,
        rolls: i32,
        skip: bool,
    ) -> commands::Result<()> {
        emit!(self, "trawlerLoot", player, level, rolls, skip);
        Ok(())
    }
    fn rules_and_info(&mut self, player: &P) -> commands::Result<i32> {
        Ok(self.ask_int("rulesText", vec![json!(player)], 0)? as i32)
    }
    fn open_book(&mut self, player: &P, book: i32, callback: Book) -> commands::Result<()> {
        emit!(self, "openBook", player, book);
        self.books.push((player.clone(), callback));
        Ok(())
    }
    fn page_setup(
        &mut self,
        player: &P,
        book: i32,
        title: &str,
        contents: &[PageSet],
        pagination: bool,
    ) -> commands::Result<()> {
        let pages: Vec<Json> = contents
            .iter()
            .map(|set| {
                Json::Array(
                    set.iter()
                        .map(|page| {
                            json!(
                                page.iter()
                                    .map(|l| json!([l.message, l.child]))
                                    .collect::<Vec<_>>()
                            )
                        })
                        .collect(),
                )
            })
            .collect();
        emit!(self, "pageSetup", player, book, title, pages, pagination);
        Ok(())
    }
    fn private_message(&mut self, player: &P, target: &str, message: &str) -> commands::Result<()> {
        emit!(self, "privateMessage", player, target, message);
        Ok(())
    }
    fn bot_script(&mut self, name: &str) -> commands::Result<Option<String>> {
        Ok(self
            .ask("botLookup", vec![json!(name)])?
            .and_then(|v| v.as_str().map(str::to_owned)))
    }
    fn valid_offers(&mut self) -> commands::Result<Vec<Offer>> {
        emit!(self, "validOffers");
        Ok(self.offers("validOffers"))
    }
    fn bot_offers(&mut self) -> commands::Result<Vec<Offer>> {
        emit!(self, "botOffers");
        Ok(self.offers("botOffers"))
    }
    fn cache_item_definitions_size(&mut self) -> commands::Result<i32> {
        Ok(self.ask_int("itemDefinitionsSize", vec![], 0)? as i32)
    }
    fn scenery_definition(&mut self, id: i32) -> commands::Result<()> {
        emit!(self, "sceneryDef", id);
        Ok(())
    }
    fn scenery_varbit_id(&mut self, id: i32) -> commands::Result<i32> {
        Ok(self.ask_int("sceneryVarbit", vec![json!(id)], -1)? as i32)
    }
    fn scenery_config_file(&mut self, id: i32) -> commands::Result<Option<i32>> {
        Ok(opt_int(self.ask("sceneryConfigFile", vec![json!(id)])?))
    }
    fn varbit_for_object(&mut self, id: i32) -> commands::Result<Option<i32>> {
        Ok(match self.ask("varbitForObject", vec![json!(id)])? {
            None => Some(id),
            other => opt_int(other),
        })
    }
    fn varbit_for_id(&mut self, id: i32) -> commands::Result<Option<i32>> {
        Ok(match self.ask("varbitForId", vec![json!(id)])? {
            None => Some(id),
            other => opt_int(other),
        })
    }
    fn varbit_varp(&mut self, varbit: i32) -> commands::Result<i32> {
        Ok(self.ask_int("varbitVarp", vec![json!(varbit)], 0)? as i32)
    }
    fn varbit_start_bit(&mut self, varbit: i32) -> commands::Result<i32> {
        Ok(self.ask_int("varbitStart", vec![json!(varbit)], 0)? as i32)
    }
    fn varbit_string(&mut self, varbit: i32) -> commands::Result<String> {
        self.ask_text(
            "varbitText",
            vec![json!(varbit)],
            format!("varbit {varbit}"),
        )
    }
    fn npc_definitions(&mut self) -> commands::Result<Vec<Option<sets::NpcDefinition>>> {
        emit!(self, "npcDefinitions");
        Ok(self
            .list("npcDefinitions")
            .iter()
            .map(|row| {
                row.as_array().map(|r| {
                    (
                        int(&r[0]),
                        r[1].as_array().map(|c| c.iter().map(int).collect()),
                    )
                })
            })
            .collect())
    }
    fn region(&mut self, region: i32) -> commands::Result<bool> {
        emit!(self, "region", region);
        Ok(self.answer("dynamic", &[json!(region)]) == Some(json!(true)))
    }
    fn region_active(&mut self, region: i32) -> commands::Result<bool> {
        self.ask_bool("regionActive", vec![json!(region)], false)
    }
    fn teleport_permitted(&mut self, [x, y, z]: Loc) -> commands::Result<bool> {
        self.ask_bool(
            "teleportPermitted",
            vec![json!(x), json!(y), json!(z)],
            true,
        )
    }
    fn clipping_flag(&mut self, [x, y, z]: Loc) -> commands::Result<i32> {
        let args = vec![json!(x), json!(y), json!(z), json!(false)];
        Ok(self.ask_int("clippingFlag", args, 0)? as i32)
    }
    fn is_landscape(&mut self, [x, y, z]: Loc) -> commands::Result<bool> {
        self.ask_bool("landscape", vec![json!(x), json!(y), json!(z)], false)
    }
    fn region_object(&mut self, [x, y, z]: Loc) -> commands::Result<Option<Scenery>> {
        let row = self.ask("object", vec![json!(x), json!(y), json!(z)])?;
        Ok(row.and_then(|r| self.placed(&r)))
    }
    fn region_chunk(&mut self, [x, y, z]: Loc) -> commands::Result<Chunk> {
        emit!(self, "chunk", x, y, z);
        self.object_keys.push(format!("{},{},{z}", x >> 3, y >> 3));
        Ok(Chunk((self.object_keys.len() - 1) as u32))
    }
    fn chunk_static_objects(&mut self, chunk: Chunk, cx: i32, cy: i32) -> Vec<Option<Scenery>> {
        let at = self.object_keys[chunk.0 as usize].clone();
        let mut out = vec![None, None, None, None];
        for row in self.list(&format!("statObjects:{at}")) {
            let r = row.as_array().unwrap().clone();
            if int(&r[0]) == cx && int(&r[1]) == cy {
                out[int(&r[2]) as usize] = self.placed(&r[3]);
            }
        }
        out
    }
    fn chunk_objects(
        &mut self,
        chunk: Chunk,
        cx: i32,
        cy: i32,
    ) -> commands::Result<Vec<Option<Scenery>>> {
        let at = self.object_keys[chunk.0 as usize].clone();
        emit!(self, "chunkObjects", at, cx, cy);
        let rows = self.list(&format!("chunkObjects:{at}"));
        Ok(rows.iter().map(|r| self.placed(r)).collect())
    }
    fn new_scenery(
        &mut self,
        id: i32,
        location: Option<Loc>,
        kind: i32,
        rotation: i32,
    ) -> commands::Result<Scenery> {
        emit!(self, "newScenery", id, coord(location), kind, rotation);
        let rotation = if rotation < 0 { rotation + 4 } else { rotation };
        let kind = if id < 1 { 22 } else { kind };
        self.object_keys.push(format!("obj{}", self.sceneries));
        self.sceneries += 1;
        let handle = (self.object_keys.len() - 1) as u32;
        emit!(self, "sceneryDef", id);
        Ok(Scenery {
            handle,
            id,
            location,
            kind,
            rotation,
            constructed: false,
            renderable: true,
            active: true,
        })
    }
    fn scenery_add(&mut self, scenery: &Scenery) -> commands::Result<()> {
        let key = self.object_keys[scenery.handle as usize].clone();
        emit!(self, "sceneryAdd", key);
        Ok(())
    }
    fn scenery_remove(&mut self, scenery: &Scenery) -> commands::Result<bool> {
        let key = self.object_keys[scenery.handle as usize].clone();
        self.ask_bool("sceneryRemove", vec![json!(key), json!(scenery.id)], true)
    }
    fn new_npc(&mut self, id: i32, location: Option<Loc>) -> commands::Result<Npc> {
        let key = format!("npc{}", self.npcs.len());
        self.npcs.push(NpcData {
            key: key.clone(),
            id,
            location,
            direction: Direction::South,
            walks: false,
        });
        emit!(self, "newNpc", key, id, coord(location));
        Ok(Npc((self.npcs.len() - 1) as u32))
    }
    fn npc_init_config(&mut self, npc: Npc) -> commands::Result<()> {
        let key = self.npc_key(npc);
        emit!(self, "initConfig", key);
        Ok(())
    }
    fn npc_name(&mut self, npc: Npc) -> commands::Result<String> {
        let (key, id) = (self.npc_key(npc), self.npcs[npc.0 as usize].id);
        self.ask_text("npcName", vec![json!(key), json!(id)], format!("npc {id}"))
    }
    fn npc_bonuses(&mut self, npc: Npc) -> commands::Result<Vec<i32>> {
        let key = self.npc_key(npc);
        emit!(self, "bonuses", key);
        Ok(self
            .list(&format!("bonuses:{key}"))
            .iter()
            .map(int)
            .collect())
    }
    fn npc_set_attack_style(&mut self, npc: Npc, style: i32, bonus: i32) -> commands::Result<()> {
        let key = self.npc_key(npc);
        emit!(self, "attackStyle", key, style, bonus);
        Ok(())
    }
    fn npc_set_attribute(&mut self, npc: Npc, name: &str, value: bool) -> commands::Result<()> {
        let key = self.npc_key(npc);
        emit!(self, "setAttribute", key, name, {"bool": value});
        Ok(())
    }
    fn npc_set_respawn(&mut self, npc: Npc, respawn: bool) -> commands::Result<()> {
        let key = self.npc_key(npc);
        emit!(self, "setRespawn", key, respawn);
        Ok(())
    }
    fn npc_set_direction(&mut self, npc: Npc, direction: Direction) -> commands::Result<()> {
        let key = self.npc_key(npc);
        emit!(self, "setDirection", key, direction.name());
        self.npcs[npc.0 as usize].direction = direction;
        Ok(())
    }
    fn npc_init(&mut self, npc: Npc) -> commands::Result<()> {
        let key = self.npc_key(npc);
        emit!(self, "init", key);
        Ok(())
    }
    fn npc_set_walks(&mut self, npc: Npc, walks: bool) -> commands::Result<()> {
        let key = self.npc_key(npc);
        emit!(self, "setWalks", key, walks);
        self.npcs[npc.0 as usize].walks = walks;
        Ok(())
    }
    fn npc_location(&mut self, npc: Npc) -> Option<Loc> {
        let key = self.npc_key(npc);
        match self.answer("npcLocation", &[json!(key)]) {
            Some(v) => location(&v),
            None => self.npcs[npc.0 as usize].location,
        }
    }
    fn npc_direction(&mut self, npc: Npc) -> Direction {
        self.npcs[npc.0 as usize].direction
    }
    fn npc_walks(&mut self, npc: Npc) -> bool {
        self.npcs[npc.0 as usize].walks
    }
    fn calculate_accuracy(&mut self, handler: Handler, entity: &Actor<P>) -> commands::Result<i32> {
        let key = self.actor(entity);
        Ok(self.ask_int("accuracy", vec![json!(handler.simple_name()), key], 0)? as i32)
    }
    fn calculate_defence(
        &mut self,
        handler: Handler,
        victim: &Actor<P>,
        attacker: &Actor<P>,
    ) -> commands::Result<i32> {
        let (v, a) = (self.actor(victim), self.actor(attacker));
        Ok(self.ask_int("defence", vec![json!(handler.simple_name()), v, a], 0)? as i32)
    }
    fn calculate_hit(
        &mut self,
        handler: Handler,
        entity: &Actor<P>,
        victim: &Actor<P>,
        modifier: f64,
    ) -> commands::Result<i32> {
        let (e, v) = (self.actor(entity), self.actor(victim));
        let m = rs09_game_core::java_double::format(modifier);
        Ok(self.ask_int("hit", vec![json!(handler.simple_name()), e, v, json!(m)], 0)? as i32)
    }
    fn println(&mut self, text: &str) {
        self.note(json!(["println", text]));
    }
    fn set_clipboard(&mut self, _: &str) -> commands::Result<()> {
        // The corpus runs headless.
        Err(sets::headless())
    }
    fn clock(&mut self) -> i64 {
        self.tick()
    }
}

impl system_manager::Host for World {
    type Player = P;
    fn worker_started(&mut self) -> bool {
        self.worker_started
    }
    fn set_worker_started(&mut self, started: bool) -> commands::Result<()> {
        emit!(self, "workerSetStarted", started);
        self.worker_started = started;
        Ok(())
    }
    fn worker_start(&mut self) -> commands::Result<()> {
        emit!(self, "workerStart");
        self.worker_started = true;
        Ok(())
    }
    fn worker_stop(&mut self) -> commands::Result<()> {
        emit!(self, "workerStop");
        Ok(())
    }
    fn players(&mut self) -> Vec<P> {
        sets::Host::online_players(self)
    }
    fn send_system_update(&mut self, player: &P, time: i32) -> commands::Result<()> {
        emit!(self, "systemUpdate", player, time);
        Ok(())
    }
    fn submit_update(&mut self, pulse: &rs09_server::pulses::Handle) -> commands::Result<()> {
        let delay = pulse.read().delay;
        emit!(self, "submit", system_manager::UPDATE_CLASS, delay);
        self.pulses += 1;
        Ok(())
    }
    fn start_update_thread(&mut self, _: &rs09_server::pulses::Handle) -> commands::Result<()> {
        unreachable!("no case runs the update without the tick worker")
    }
    fn log(&mut self, origin: &str, message: &str) -> commands::Result<()> {
        emit!(self, "log", origin, "INFO", message);
        Ok(())
    }
    fn data_path(&mut self) -> Option<String> {
        commands::Host::data_path(self)
    }
    fn set_server_running(&mut self, running: bool) -> commands::Result<()> {
        emit!(self, "serverRunning", running);
        Ok(())
    }
    fn clear_all_bots(&mut self) -> commands::Result<()> {
        emit!(self, "clearBots");
        Ok(())
    }
    fn terminate_reactor(&mut self) -> commands::Result<()> {
        emit!(self, "reactorTerminate");
        Ok(())
    }
    fn websocket_present(&mut self) -> bool {
        self.websocket
    }
    fn stop_websocket(&mut self, timeout: i32, reason: &str) -> commands::Result<()> {
        emit!(self, "websocketStop", timeout, reason);
        Ok(())
    }
    fn clear_websocket(&mut self) -> commands::Result<()> {
        emit!(self, "websocketSet", false);
        self.websocket = false;
        Ok(())
    }
    fn is_artificial(&mut self, player: &P) -> commands::Result<bool> {
        sets::Host::is_artificial(self, player)
    }
    fn save_details(&mut self, player: &P) -> commands::Result<()> {
        emit!(self, "saveDetails", player);
        Ok(())
    }
    fn clear_player(&mut self, player: &P) -> commands::Result<()> {
        emit!(self, "clear", player);
        Ok(())
    }
    fn shutdown_listeners(&mut self) -> Vec<String> {
        self.list("shutdownListeners")
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect()
    }
    fn shutdown_listener(&mut self, listener: &str) -> commands::Result<()> {
        emit!(self, "shutdownListener", listener);
        Ok(())
    }
    fn monitor_flush(&mut self) -> commands::Result<()> {
        emit!(self, "monitorFlush");
        Ok(())
    }
    fn world_persists(&mut self) -> Vec<Persist> {
        self.list("worldPersists")
            .iter()
            .map(|v| {
                let name = v.as_str().unwrap().to_owned();
                Persist {
                    server_store: name == "ServerStore",
                    name,
                }
            })
            .collect()
    }
    fn persist_save(&mut self, store: &Persist) -> commands::Result<()> {
        emit!(self, "persistSave", store.name);
        Ok(())
    }
    fn absolute_path(&mut self, directory: &str) -> String {
        format!("<work>/{directory}")
    }
    fn is_directory(&mut self, directory: &str) -> bool {
        self.dirs.contains(directory)
    }
    fn mkdirs(&mut self, directory: &str) {
        self.dirs.insert(directory.to_owned());
    }
    fn now(&mut self) -> i64 {
        self.tick()
    }
    fn queue_empty(&mut self) -> commands::Result<bool> {
        self.ask_bool("queueEmpty", vec![], true)
    }
    fn queue_update(&mut self) -> commands::Result<()> {
        emit!(self, "queueUpdate");
        Ok(())
    }
    fn queue_clear(&mut self) -> commands::Result<()> {
        emit!(self, "queueClear");
        Ok(())
    }
    fn sleep(&mut self, _: i64) {}
    fn print_stack_trace(&mut self, error: &commands::Error) {
        let e = error_json(error);
        self.note(json!(["printStackTrace", e]));
    }
}

fn error_json(e: &commands::Error) -> Json {
    match e {
        commands::Error::Index(m) | commands::Error::NumberFormat(m) => json!([e.category(), m]),
        commands::Error::Binding(Some(m)) if m == sets::HEADLESS => json!(["Headless", null]),
        commands::Error::Binding(Some(m)) if m.starts_with("exit ") => {
            json!(["Exit", m[5..].parse::<i64>().unwrap()])
        }
        _ => json!([e.category(), null]),
    }
}

/// The registry the corpus compiles: the four sets in the scan's order,
/// then the case's stand-ins.
fn behaviour_registry(case: &Json) -> Registry {
    let mut registry = registration::register_only(&[
        "core.game.system.command.sets.MiscCommandSet",
        "core.game.system.command.sets.SpawnCommandSet",
        "core.game.system.command.sets.StatsCommandSet",
        "core.game.system.command.sets.SystemCommandSet",
    ]);
    for row in case["extra"].as_array().unwrap() {
        let class: &'static str = Box::leak(row[2].as_str().unwrap().to_owned().into_boxed_str());
        let body = ContentBody {
            class,
            lambda: int(&row[3]) as u16,
        };
        registry.register(Command {
            name: row[0].as_str().unwrap().into(),
            privilege: match row[1].as_str().unwrap() {
                "STANDARD" => Privilege::Standard,
                "MODERATOR" => Privilege::Moderator,
                _ => Privilege::Admin,
            },
            usage: row[4].as_str().unwrap().into(),
            description: row[5].as_str().unwrap().into(),
            body: Body::Content(body),
            origin: body,
        });
    }
    registry
}

fn state_of(name: &str) -> State {
    match name {
        "ACTIVE" => State::Active,
        "UPDATING" => State::Updating,
        "PRIVATE" => State::Private,
        _ => State::Terminated,
    }
}

fn run_op(world: &mut World, registry: &Registry, op: &Json) -> Result<Json, Json> {
    let player = op["player"].as_str().map(str::to_owned);
    let ce = |e: commands::Error| error_json(&e);
    match op["op"].as_str().unwrap() {
        "parse" => commands::parse(
            world,
            registry,
            player.as_ref(),
            op["line"].as_str().unwrap(),
        )
        .map(Json::Bool)
        .map_err(ce),
        "input" => {
            let handler = world.inputs[op["index"].as_u64().unwrap() as usize].clone();
            let value = match &op["value"] {
                Json::String(s) => InputValue::Text(s.clone()),
                v => InputValue::Int(int(v)),
            };
            match handler {
                Pending::Set(h) => sets::misc::run_input(world, &h, &value).map_err(ce)?,
                Pending::Command => unreachable!("no case answers a SystemCommandSet prompt"),
            }
            Ok(Json::Null)
        }
        "job" => {
            let job = world.jobs[op["index"].as_u64().unwrap() as usize].clone();
            sets::misc::run_job(world, &job).map_err(ce)?;
            Ok(Json::Null)
        }
        "book" => {
            let (_, book) = world.books[op["index"].as_u64().unwrap() as usize].clone();
            let p = player.unwrap();
            let (page, button) = (int(&op["page"]), int(&op["button"]));
            match book {
                Book::Stats => sets::stats::display(world, &p, page, button),
                Book::Ge { title, contents } => {
                    sets::misc::ge_display(world, &p, &title, &contents)
                }
            }
            .map(Json::Bool)
            .map_err(ce)
        }
        "flag" => {
            let state = state_of(op["state"].as_str().unwrap());
            world.with_manager(|m, w| m.flag(w, state)).map_err(ce)?;
            Ok(Json::Null)
        }
        "countdown" => {
            let ticks = int(&op["ticks"]);
            world.manager.as_mut().unwrap().updater.set_countdown(ticks);
            Ok(Json::Null)
        }
        "backup" => {
            world.manager.as_mut().unwrap().updater.create_backup = op["value"] == json!(true);
            Ok(Json::Null)
        }
        "cancel" => {
            world.with_manager(|m, w| m.cancel(w)).map_err(ce)?;
            Ok(Json::Null)
        }
        "notify" => {
            world.with_manager(|m, w| m.notify_players(w));
            Ok(Json::Null)
        }
        "update" => Ok(Json::Bool(world.with_manager(|m, w| m.update(w)))),
        "hook" => {
            world.with_manager(|m, w| m.shutdown_hook(w)).map_err(ce)?;
            Ok(Json::Null)
        }
        "terminate" => {
            world.with_manager(|m, w| m.terminate(w)).map_err(ce)?;
            Ok(Json::Null)
        }
        "save" => {
            let directory = op["directory"].as_str().unwrap().to_owned();
            world
                .with_manager(|m, w| m.save(w, &directory))
                .map_err(ce)?;
            Ok(Json::Null)
        }
        "query" => {
            let m = world.manager.as_ref().unwrap();
            Ok(json!([
                m.is_active(),
                m.is_updating(),
                m.is_private(),
                m.is_terminated(),
                m.updater.create_backup
            ]))
        }
        "config" => {
            let config = &world.manager.as_ref().unwrap().config;
            Ok(match op["what"].as_str().unwrap() {
                "validLogin" => json!(config.valid_login()),
                "doubleExp" => json!(config.is_double_exp()),
                "split" => json!(
                    config
                        .split(op["data"].as_str().unwrap(), op["regex"].as_str().unwrap())
                        .map_err(ce)?
                ),
                "beta" => json!(config.is_beta_user(op["name"].as_str().unwrap())),
                "get" => Json::Null,
                "getOr" => op["fail"].clone(),
                "sizes" => {
                    let (a, b) = config.sizes();
                    json!([a, b])
                }
                other => panic!("config {other}"),
            })
        }
        "packet" => packet(world, player.as_ref().unwrap(), op).map_err(ce),
        other => panic!("op {other}"),
    }
}

/// The `write` a session records: opcode, header, payload.
fn written(frame: &rs09_protocol::game::Frame, header: &str) -> Json {
    let hex: String = frame.payload.iter().map(|b| format!("{b:02x}")).collect();
    json!([frame.opcode, header, hex])
}

/// The interface manager's `getPacketCount(1)`.
fn packet_count(world: &mut World, player: &P) -> i32 {
    let data = world.players.get_mut(player).unwrap();
    let count = data.packet_count;
    data.packet_count = count.wrapping_add(1);
    count
}

fn packet(world: &mut World, player: &P, op: &Json) -> commands::Result<Json> {
    let last = world.players[player].last_scene;
    let frame = match op["kind"].as_str().unwrap() {
        "systemUpdate" => command_packets::system_update(int(&op["time"])),
        "camera" => {
            let kind = match op["type"].as_str().unwrap() {
                "POSITION" => CameraType::Position,
                "ROTATION" => CameraType::Rotation,
                "SET" => CameraType::Set,
                "SHAKE" => CameraType::Shake,
                _ => CameraType::Reset,
            };
            let counter = packet_count(world, player);
            let camera = Camera {
                kind,
                x: int(&op["x"]),
                y: int(&op["y"]),
                height: int(&op["height"]),
                speed: int(&op["speed"]),
                zoom_speed: int(&op["zoomSpeed"]),
            };
            command_packets::camera(camera, counter, last)?
        }
        "csConfig" => {
            let counter = packet_count(world, player);
            command_packets::cs_config(counter, int(&op["id"]), int(&op["value"]))
        }
        "areaPosition" => {
            command_packets::update_area_position(location(&op["location"]).unwrap(), last)?
        }
        "chunkBuffer" => {
            let frame =
                command_packets::chunk_update_header(location(&op["location"]).unwrap(), last)?;
            return Ok(written(&frame, "SHORT"));
        }
        "animateObject" => {
            let row = op["object"].as_array().unwrap();
            let object = AnimatedObject {
                location: location(&row[1]).unwrap(),
                kind: int(&row[2]),
                rotation: int(&row[3]),
            };
            command_packets::animate_object(object, int(&op["animation"]), last)?
        }
        other => panic!("packet {other}"),
    };
    let record = written(&frame, "NORMAL");
    world
        .emit(json!(["write", player, record]))
        .map_err(|_| binding())?;
    Ok(Json::Null)
}

fn corpus() -> Vec<Json> {
    CORPUS
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

#[test]
fn fixture_matches_provenance() {
    let provenance: Json = serde_json::from_str(PROVENANCE).unwrap();
    let digest = format!("{:x}", Sha256::digest(CORPUS.as_bytes()));
    assert_eq!(provenance["fixtureSha256"], json!(digest));
    assert_eq!(provenance["caseCount"], json!(CORPUS.lines().count()));
}

#[test]
fn replays_the_original_cases() {
    let mut failures = Vec::new();
    let rows = corpus();
    for (number, row) in rows.iter().enumerate() {
        let registry = behaviour_registry(row);
        let mut world = World::new(row);
        for (index, (op, expected)) in row["ops"]
            .as_array()
            .unwrap()
            .iter()
            .zip(row["expected"].as_array().unwrap())
            .enumerate()
        {
            world.events.clear();
            world.op_calls = 0;
            world.fail_at = op.get("failAt").map_or(-1, |v| v.as_i64().unwrap());
            let (result, error) = match run_op(&mut world, &registry, op) {
                Ok(result) => (result, Json::Null),
                Err(error) => (Json::Null, error),
            };
            let actual = json!({"events": world.events, "error": error, "result": result, "state": world.state()});
            if actual != *expected {
                failures.push(format!(
                    "case {number} {} op {index} {op}\n  expected {expected}\n  actual   {actual}",
                    row["name"]
                ));
                break;
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} cases differ:\n{}",
        failures.len(),
        rows.len(),
        failures
            .iter()
            .take(8)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn every_define_of_the_three_sets_is_native() {
    let registry = registration::register_all(&registration::pinned_boot_active);
    let mut counts = BTreeMap::new();
    for command in registry.commands() {
        let class = command.origin.class.rsplit('.').next().unwrap();
        if let Body::Native(NativeBody::Set(body)) = command.body {
            let expected = match class {
                "MiscCommandSet" => matches!(body, SetBody::Misc(_)),
                "SpawnCommandSet" => matches!(body, SetBody::Spawn(_)),
                "StatsCommandSet" => body == SetBody::Stats,
                _ => false,
            };
            assert!(expected, "{} from {class}", command.name);
            *counts.entry(class).or_insert(0) += 1;
        } else {
            assert!(
                !["MiscCommandSet", "SpawnCommandSet", "StatsCommandSet"].contains(&class),
                "{} is still a content body",
                command.name
            );
        }
    }
    assert_eq!(
        counts.into_iter().collect::<Vec<_>>(),
        vec![
            ("MiscCommandSet", 49),
            ("SpawnCommandSet", 7),
            ("StatsCommandSet", 1)
        ]
    );
}

#[test]
fn hit_chance_rounding_and_chunking() {
    assert_eq!(sets::round_up_4(0.98305).unwrap(), 0.9831);
    assert_eq!(sets::round_up_4(1.0e-7).unwrap(), 1.0e-4);
    assert_eq!(sets::round_up_4(-0.12341).unwrap(), -0.1235);
    assert_eq!(sets::round_up_4(0.5).unwrap(), 0.5);
    assert!(sets::round_up_4(f64::NEG_INFINITY).is_err());
    assert_eq!(sets::chunk_text("abc", 70), vec!["abc"]);
    for chunk in sets::chunk_text(&"word ".repeat(30), 70) {
        assert!(chunk.len() <= 70);
    }
}

#[test]
fn system_config_split() {
    let config = system_manager::Config::default();
    assert_eq!(
        config.split("a,b,,c,,", ",").unwrap(),
        vec!["a", "b", "", "c"]
    );
    assert_eq!(config.split(",", ",").unwrap(), Vec::<String>::new());
    assert_eq!(config.split(" x ", " ").unwrap(), vec!["x"]);
    assert!(config.split("a.b", ".").is_err());
}
