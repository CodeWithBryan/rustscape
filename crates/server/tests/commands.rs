//! Replays the commands corpus (fixtures/commands) through `world::commands`,
//! `world::quick_chat` and `world::abuse_report`, and checks the native
//! registration against the registry the original's boot leaves
//! (fixtures/commands/registry.json). Every service answers from the case
//! the way the oracle's recording adapters do (oracle/commands/adapters.py
//! and Support.java); every recorded call, result, error and the account
//! state after each operation is compared.
use rs09_server::auth::account::{Account, UserAccountInfo};
use rs09_server::player::details::Rights;
use rs09_server::world::abuse_report;
use rs09_server::world::commands::{
    self, Body, Command, Container, ContentBody, InputType, ItemRef, LegacyPlugin, LegacySet, Loc,
    Privilege, Registry, TeleportType, Value,
    deferred::{self, InputHandler, InputValue, Job, Pulse, Task},
    kotlin, registration,
};
use rs09_server::world::quick_chat::{self, ChatPulse, Level};
use rs09_server::world::social::{ChatMessage, Event};
use serde_json::{Map, Value as Json, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};

const CORPUS: &str = include_str!("../../../fixtures/commands/oracle.jsonl");
const PROVENANCE: &str = include_str!("../../../fixtures/commands/provenance.json");
const REGISTRY: &str = include_str!("../../../fixtures/commands/registry.json");
const WORLD_TRACE: &str = include_str!("../../../fixtures/world-trace/trace.jsonl");

type P = String;

fn lock<T>(mutex: &std::sync::Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn int(v: &Json) -> i32 {
    v.as_i64().unwrap() as i32
}

#[derive(Clone)]
struct PlayerData {
    name: String,
    username: String,
    account: Account,
    location: Option<Loc>,
    attributes: HashMap<String, Option<Value>>,
}

enum Pending {
    Command(Pulse<P>),
    Chat(ChatPulse<P>),
}

struct World {
    values: Map<String, Json>,
    answers: Map<String, Json>,
    answer_calls: HashMap<String, usize>,
    players: BTreeMap<String, PlayerData>,
    accounts: Vec<(String, Account)>,
    files: Map<String, Json>,
    events: Vec<Json>,
    op_calls: i64,
    fail_at: i64,
    clock_reads: usize,
    pulses: Vec<Pending>,
    tasks: Vec<Task<P>>,
    inputs: Vec<InputHandler<P>>,
    jobs: Vec<Job<P>>,
}

fn location(v: &Json) -> Option<Loc> {
    v.as_array().map(|a| [int(&a[0]), int(&a[1]), int(&a[2])])
}
fn coord(l: Option<Loc>) -> Json {
    l.map_or(Json::Null, |l| json!(l))
}
fn attribute(spec: &Json) -> Option<Value> {
    let m = spec.as_object()?;
    Some(if let Some(v) = m.get("int") {
        Value::Int(int(v))
    } else if let Some(v) = m.get("float") {
        Value::Float(kotlin::to_float_or_null(v.as_str().unwrap()).unwrap())
    } else if let Some(v) = m.get("bool") {
        Value::Bool(v.as_bool().unwrap())
    } else if let Some(v) = m.get("text") {
        Value::Text(v.as_str().unwrap().to_owned())
    } else if let Some(v) = m.get("rights") {
        Value::Other(v.as_str().unwrap().to_owned())
    } else {
        panic!("attribute {spec}")
    })
}
/// `Support.value`: the recorded form of an attribute value.
fn value_json(v: Option<&Value>) -> Json {
    match v {
        None => Json::Null,
        Some(Value::Int(i)) => json!({"int": i}),
        Some(Value::Float(f)) => json!({"float": kotlin::float_to_string(*f)}),
        Some(Value::Bool(b)) => json!({"bool": b}),
        Some(Value::Text(t)) => json!({"text": t}),
        Some(Value::Other(o)) => json!({"rights": o}),
    }
}
fn binding() -> commands::Error {
    commands::Error::Binding(None)
}

impl World {
    fn new(case: &Json) -> Self {
        let mut accounts = Vec::new();
        if let Some(specs) = case["accounts"].as_object() {
            for (key, a) in specs {
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
                if let Some(v) = a.get("muteEndTime") {
                    info.mute_end_time = v.as_i64().unwrap();
                }
                if let Some(v) = a.get("banEndTime") {
                    info.ban_end_time = v.as_i64().unwrap();
                }
                accounts.push((key.clone(), info.shared()));
            }
        }
        let mut players = BTreeMap::new();
        for (key, p) in case["players"].as_object().unwrap() {
            let account = accounts
                .iter()
                .find(|(k, _)| Some(k.as_str()) == p["account"].as_str())
                .map(|(_, a)| a.clone())
                .expect("player account");
            let attributes = p
                .get("attributes")
                .and_then(Json::as_object)
                .map(|m| m.iter().map(|(k, v)| (k.clone(), attribute(v))).collect())
                .unwrap_or_default();
            players.insert(
                key.clone(),
                PlayerData {
                    name: p["name"].as_str().unwrap().into(),
                    username: p["username"].as_str().unwrap().into(),
                    account,
                    location: location(&p["location"]),
                    attributes,
                },
            );
        }
        Self {
            values: case["values"].as_object().cloned().unwrap_or_default(),
            answers: case["answers"].as_object().cloned().unwrap_or_default(),
            answer_calls: HashMap::new(),
            players,
            accounts,
            files: case["files"].as_object().cloned().unwrap_or_default(),
            events: Vec::new(),
            op_calls: 0,
            fail_at: -1,
            clock_reads: 0,
            pulses: Vec::new(),
            tasks: Vec::new(),
            inputs: Vec::new(),
            jobs: Vec::new(),
        }
    }

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
    fn answer(&mut self, name: &str) -> Option<Json> {
        let v = self.answers.get(name)?.clone();
        if let Json::Array(list) = &v
            && !name.starts_with("slots:")
            && !name.starts_with("ips:")
            && !name.starts_with("file:")
        {
            let calls = self.answer_calls.entry(name.into()).or_default();
            let i = *calls;
            *calls += 1;
            return Some(list[i.min(list.len() - 1)].clone());
        }
        Some(v)
    }
    fn answer_bool(&mut self, name: &str, fallback: bool) -> bool {
        self.answer(name)
            .map_or(fallback, |v| v.as_bool() == Some(true))
    }
    fn answer_int(&mut self, name: &str, fallback: i32) -> i32 {
        self.answer(name).map_or(fallback, |v| int(&v))
    }
    fn account_key(&self, info: &Account) -> Json {
        self.accounts
            .iter()
            .find(|(_, a)| std::sync::Arc::ptr_eq(a, info))
            .map_or(Json::Null, |(k, _)| json!(k))
    }
    fn slots(&self, player: &str, container: Container) -> Vec<Option<ItemRef>> {
        self.answers
            .get(&format!("slots:{player}:{}", container.name()))
            .and_then(Json::as_array)
            .map(|l| {
                l.iter()
                    .enumerate()
                    .map(|(i, s)| {
                        s.as_array().map(|s| ItemRef {
                            id: int(&s[0]),
                            slot: i as i32,
                            charge: int(&s[1]),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
    fn state(&self) -> Json {
        let accounts: Map<String, Json> = self
            .accounts
            .iter()
            .map(|(k, a)| {
                let a = lock(a);
                (
                    k.clone(),
                    json!([
                        a.rights,
                        a.credits,
                        a.ban_end_time,
                        a.mute_end_time,
                        a.password
                    ]),
                )
            })
            .collect();
        json!({"accounts": accounts, "pulses": self.pulses.len(), "tasks": self.tasks.len(),
               "inputs": self.inputs.len(), "jobs": self.jobs.len()})
    }
}

macro_rules! emit {
    ($self:ident, $($e:tt)*) => {
        $self.emit(json!([$($e)*])).map_err(|_| binding())?
    };
}
macro_rules! emit_qc {
    ($self:ident, $($e:tt)*) => {
        $self
            .emit(json!([$($e)*]))
            .map_err(|_| quick_chat::Error::Binding(None))?
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
    fn castle_wars_destination(&mut self) -> Loc {
        location(&self.values["castleWars"]).unwrap()
    }
    fn data_path(&mut self) -> Option<String> {
        self.values["dataPath"].as_str().map(str::to_owned)
    }
    fn now(&mut self) -> i64 {
        self.note(json!(["clock"]));
        let clock = self.values["clock"].as_array().cloned().unwrap_or_default();
        if clock.is_empty() {
            return 0;
        }
        let i = self.clock_reads.min(clock.len() - 1);
        self.clock_reads += 1;
        clock[i].as_i64().unwrap()
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
        self.inputs.push(handler);
        Ok(())
    }
    fn run_task(
        &mut self,
        player: &P,
        delay: i32,
        repeat: i32,
        task: Task<P>,
    ) -> commands::Result<()> {
        emit!(self, "runTask", player, delay, repeat);
        self.tasks.push(task);
        Ok(())
    }
    fn monitor_log(&mut self, player: &P, line: &str) -> commands::Result<()> {
        emit!(self, "monitor", player, "COMMAND", line);
        Ok(())
    }
    fn zone_parse_command(
        &mut self,
        player: &P,
        name: &str,
        arguments: &[String],
    ) -> commands::Result<bool> {
        emit!(self, "zone", player, name, arguments);
        Ok(self.answer_bool("zone", false))
    }
    fn legacy_parse(
        &mut self,
        plugin: LegacyPlugin,
        player: &P,
        name: &str,
        arguments: &[String],
    ) -> commands::Result<bool> {
        emit!(self, "legacy", plugin, player, name, arguments);
        Ok(self.answer_bool(&format!("legacy:{plugin}"), false))
    }
    fn content_command(
        &mut self,
        body: ContentBody,
        player: &P,
        args: &[String],
    ) -> commands::Result<()> {
        emit!(self, "content", body.class, body.lambda, player, args);
        if self.answer_bool(
            &format!(
                "contentReject:{}$defineCommands${}",
                body.class, body.lambda
            ),
            false,
        ) {
            return Err(commands::Error::IllegalState(None));
        }
        Ok(())
    }
    fn player_by_name(&mut self, name: Option<&str>) -> commands::Result<Option<P>> {
        emit!(self, "byName", name);
        let key = self.answer(&format!("byName:{}", name.unwrap_or("null")));
        Ok(key
            .and_then(|k| k.as_str().map(str::to_owned))
            .filter(|k| self.players.contains_key(k)))
    }
    fn clear(&mut self, player: &P) -> commands::Result<()> {
        emit!(self, "clear", player);
        Ok(())
    }
    fn attribute(&mut self, player: &P, key: &str) -> commands::Result<Option<Value>> {
        emit!(self, "getAttribute", player, key);
        Ok(self.players[player].attributes.get(key).cloned().flatten())
    }
    fn api_attribute(
        &mut self,
        player: &P,
        key: &str,
        default: Value,
    ) -> commands::Result<Option<Value>> {
        emit!(
            self,
            "apiGetAttribute",
            player,
            key,
            value_json(Some(&default))
        );
        Ok(match self.players[player].attributes.get(key) {
            Some(stored) => stored.clone(),
            None => Some(default),
        })
    }
    fn set_attribute(
        &mut self,
        player: &P,
        key: &str,
        value: Option<Value>,
    ) -> commands::Result<()> {
        emit!(
            self,
            "apiSetAttribute",
            player,
            key,
            value_json(value.as_ref())
        );
        Ok(())
    }
    fn teleport(
        &mut self,
        player: &P,
        location: Loc,
        kind: TeleportType,
    ) -> commands::Result<bool> {
        let kind = match kind {
            TeleportType::Normal => "NORMAL",
        };
        emit!(self, "teleport", player, location, kind);
        Ok(self.answer_bool("teleport", true))
    }
    fn set_teleport_location(&mut self, player: &P, location: Option<Loc>) -> commands::Result<()> {
        emit!(self, "teleportLocation", player, coord(location));
        Ok(())
    }
    fn username_taken(&mut self, name: &str) -> commands::Result<bool> {
        emit!(self, "taken", name);
        Ok(self.answer_bool(&format!("taken:{name}"), false))
    }
    fn account_info(&mut self, name: Option<&str>) -> commands::Result<Account> {
        let name = name.ok_or(commands::Error::Null)?;
        emit!(self, "accountInfo", name);
        match self
            .answer(&format!("info:{name}"))
            .and_then(|k| k.as_str().map(str::to_owned))
        {
            Some(key) => Ok(self
                .accounts
                .iter()
                .find(|(k, _)| *k == key)
                .unwrap()
                .1
                .clone()),
            None => {
                // The storage answers a fresh `UserAccountInfo.createDefault()`,
                // which reads the clock.
                let now = commands::Host::now(self);
                let fresh = UserAccountInfo::create_default(now).shared();
                let key = format!("new:{name}:{}", self.accounts.len());
                self.accounts.push((key, fresh.clone()));
                Ok(fresh)
            }
        }
    }
    fn account_update(&mut self, info: &Account) -> commands::Result<()> {
        let key = self.account_key(info);
        emit!(self, "accountUpdate", key);
        Ok(())
    }
    fn usernames_with_ip(&mut self, ip: &str) -> commands::Result<Vec<Option<String>>> {
        emit!(self, "ips", ip);
        Ok(self
            .answers
            .get(&format!("ips:{ip}"))
            .and_then(Json::as_array)
            .map(|l| l.iter().map(|v| v.as_str().map(str::to_owned)).collect())
            .unwrap_or_default())
    }
    fn check_password(&mut self, player: &P, password: &str) -> commands::Result<bool> {
        emit!(self, "checkPassword", player, password);
        Ok(self.answer_bool("checkPassword", false))
    }
    fn update_password(&mut self, username: &str, password: &str) -> commands::Result<()> {
        emit!(self, "updatePassword", username, password);
        Ok(())
    }
    fn store_add_to_list(&mut self, archive: &str, key: &str, value: &str) -> commands::Result<()> {
        emit!(self, "store", archive, key, value);
        Ok(())
    }
    fn submit_pulse(&mut self, pulse: Pulse<P>) -> commands::Result<()> {
        emit!(self, "submit", pulse.label(), pulse.delay());
        self.pulses.push(Pending::Command(pulse));
        Ok(())
    }
    fn launch(&mut self, job: Job<P>) -> commands::Result<()> {
        emit!(self, "launch");
        self.jobs.push(job);
        Ok(())
    }
    fn file_lines(
        &mut self,
        directory: Option<&str>,
        name: &str,
    ) -> commands::Result<Option<Vec<String>>> {
        if directory != Some("data") {
            return Ok(None);
        }
        Ok(self
            .files
            .get(name)
            .and_then(Json::as_str)
            .map(|t| t.lines().map(str::to_owned).collect()))
    }
    fn set_update_countdown(&mut self, seconds: i32) -> commands::Result<()> {
        emit!(self, "countdown", seconds);
        Ok(())
    }
    fn flag_updating(&mut self) -> commands::Result<()> {
        emit!(self, "flag", "UPDATING");
        Ok(())
    }
    fn cancel_update(&mut self) -> commands::Result<()> {
        emit!(self, "cancelUpdate");
        Ok(())
    }
    fn exit_process(&mut self, _status: i32) -> commands::Result<()> {
        panic!("no case drives ::shutdown")
    }
    fn item_definitions_size(&mut self) -> commands::Result<i32> {
        emit!(self, "itemDefs");
        Ok(self.answer_int("itemDefs", 0))
    }
    fn item_name(&mut self, id: i32) -> commands::Result<String> {
        emit!(self, "itemName", id);
        Ok(match self.answer(&format!("itemName:{id}")) {
            None => format!("item {id}"),
            Some(Json::Null) => "null".into(),
            Some(v) => v.as_str().unwrap().into(),
        })
    }
    fn note_change(&mut self, id: i32) -> commands::Result<i32> {
        emit!(self, "noteChange", id);
        Ok(self.answer_int(&format!("noteChange:{id}"), id))
    }
    fn is_full(&mut self, player: &P, container: Container) -> commands::Result<bool> {
        emit!(self, "isFull", player, container.name());
        Ok(self.answer_bool(&format!("isFull:{player}:{}", container.name()), false))
    }
    fn add(
        &mut self,
        player: &P,
        container: Container,
        id: i32,
        amount: i32,
    ) -> commands::Result<bool> {
        emit!(self, "add", player, container.name(), id, amount);
        Ok(self.answer_bool("add", true))
    }
    fn remove(
        &mut self,
        player: &P,
        container: Container,
        id: i32,
        amount: i32,
    ) -> commands::Result<bool> {
        emit!(self, "remove", player, container.name(), id, amount);
        Ok(self.answer_bool("remove", true))
    }
    fn item_amount(
        &mut self,
        player: &P,
        container: Container,
        id: i32,
        amount: i32,
    ) -> commands::Result<Option<i32>> {
        emit!(self, "getItem", player, container.name(), id, amount);
        Ok(self
            .answer(&format!("getItem:{player}:{}:{id}", container.name()))
            .filter(|v| !v.is_null())
            .map(|v| int(&v)))
    }
    fn amount(&mut self, player: &P, container: Container, id: i32) -> commands::Result<i32> {
        emit!(self, "amount", player, container.name(), id);
        Ok(self.answer_int(&format!("amount:{player}:{}:{id}", container.name()), 0))
    }
    fn amount_item(&mut self, player: &P, container: Container, id: i32) -> commands::Result<i32> {
        emit!(self, "amountItem", player, container.name(), id);
        Ok(self.answer_int(&format!("amountItem:{player}:{}:{id}", container.name()), 0))
    }
    fn slots(
        &mut self,
        player: &P,
        container: Container,
    ) -> commands::Result<Vec<Option<ItemRef>>> {
        emit!(self, "slots", player, container.name());
        Ok(World::slots(self, player, container))
    }
    fn get(
        &mut self,
        player: &P,
        container: Container,
        slot: i32,
    ) -> commands::Result<Option<ItemRef>> {
        emit!(self, "get", player, container.name(), slot);
        let slots = World::slots(self, player, container);
        Ok(usize::try_from(slot)
            .ok()
            .and_then(|s| slots.get(s).cloned())
            .flatten())
    }
    fn set_charge(
        &mut self,
        player: &P,
        container: Container,
        item: &ItemRef,
        charge: i32,
    ) -> commands::Result<()> {
        emit!(
            self,
            "setCharge",
            player,
            container.name(),
            item.slot,
            charge
        );
        Ok(())
    }
    fn replace(
        &mut self,
        player: &P,
        container: Container,
        id: i32,
        slot: i32,
    ) -> commands::Result<()> {
        emit!(self, "replace", player, container.name(), id, slot);
        Ok(())
    }
    fn charged_item(&mut self, id: i32) -> commands::Result<bool> {
        emit!(self, "chargedItem", id);
        Ok(self.answer_bool(&format!("chargedItem:{id}"), false))
    }
    fn charged_for_charge(&mut self, id: i32, charge: i32) -> commands::Result<i32> {
        emit!(self, "forCharge", id, charge);
        Ok(self.answer_int(&format!("forCharge:{id}:{charge}"), id))
    }
    fn charged_charge(&mut self, id: i32) -> commands::Result<Option<i32>> {
        emit!(self, "chargedCharge", id);
        Ok(self
            .answer(&format!("chargedCharge:{id}"))
            .filter(|v| !v.is_null())
            .map(|v| int(&v)))
    }
}

impl abuse_report::Host for World {
    fn name(&mut self, player: &P) -> String {
        self.players[player].name.clone()
    }
    fn packet_message(&mut self, player: &P, text: &str) -> commands::Result<()> {
        emit!(self, "packetMessage", player, text);
        Ok(())
    }
    fn post_player_alert(&mut self, player: &str, kind: &str) -> commands::Result<()> {
        emit!(self, "discord", player, kind);
        Ok(())
    }
}

impl quick_chat::Host for World {
    type Player = P;
    fn qc_file(&mut self, index: i32) -> quick_chat::Result<Option<Vec<u8>>> {
        emit_qc!(self, "qcFile", 24, 1, index);
        Ok(self.answer(&format!("qcFile:{index}")).and_then(|v| {
            v.as_str().map(|hex| {
                (0..hex.len() / 2)
                    .map(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap())
                    .collect()
            })
        }))
    }
    fn data_map_string(&mut self, map: i32, key: i32) -> quick_chat::Result<Option<String>> {
        emit_qc!(self, "dataMap", map, key);
        match self.answer(&format!("dataMap:{map}:{key}")) {
            None | Some(Json::Null) => Ok(None),
            Some(Json::String(s)) => Ok(Some(s)),
            Some(_) => Err(quick_chat::Error::Cast),
        }
    }
    fn item_name(&mut self, id: i32) -> quick_chat::Result<Option<String>> {
        emit_qc!(self, "itemDef", id);
        if !self.answer_bool(&format!("itemDefExists:{id}"), true) {
            return Err(quick_chat::Error::Null);
        }
        Ok(self
            .answer(&format!("itemDefName:{id}"))
            .and_then(|v| v.as_str().map(str::to_owned)))
    }
    fn experience(&mut self, player: &P, slot: i32) -> quick_chat::Result<f64> {
        emit_qc!(self, "experience", player, slot);
        Ok(match self.answer(&format!("experience:{player}:{slot}")) {
            None => 0.0,
            Some(Json::String(s)) => rs09_game_core::java_double::parse(&s).unwrap(),
            Some(v) => v.as_f64().unwrap(),
        })
    }
    fn static_level(&mut self, player: &P, slot: i32) -> quick_chat::Result<i32> {
        emit_qc!(self, "staticLevel", player, slot);
        Ok(self.answer_int(&format!("staticLevel:{player}:{slot}"), 1))
    }
    fn slayer_amount(&mut self, player: &P) -> quick_chat::Result<i32> {
        emit_qc!(self, "slayerAmount", player);
        Ok(self.answer_int(&format!("slayerAmount:{player}"), 0))
    }
    fn slayer_task_name(&mut self, player: &P) -> quick_chat::Result<Option<String>> {
        emit_qc!(self, "slayerTask", player);
        Ok(self
            .answer(&format!("slayerTask:{player}"))
            .and_then(|v| v.as_str().map(str::to_owned)))
    }
    fn set_attribute(&mut self, player: &P, key: &str, value: i32) -> quick_chat::Result<()> {
        emit_qc!(self, "setAttribute", player, key, {"int": value});
        Ok(())
    }
    fn name(&mut self, player: &P) -> String {
        self.players[player].name.clone()
    }
    fn account(&mut self, player: &P) -> Account {
        self.players[player].account.clone()
    }
    fn clan_owner(&mut self, player: &P) -> quick_chat::Result<Option<String>> {
        Ok(self
            .answer(&format!("clan:{player}"))
            .and_then(|v| v.as_str().map(str::to_owned)))
    }
    /// `Rights.getChatIcon`: the rights (read first), then the
    /// `visible_rank` attribute with them as its default.
    fn chat_icon(&mut self, player: &P) -> quick_chat::Result<i32> {
        let ordinal = lock(&self.players[player].account).rights;
        let rights = Rights::from_ordinal(ordinal).map_err(|_| {
            quick_chat::Error::Index(format!("Index {ordinal} out of bounds for length 3"))
        })?;
        emit_qc!(self, "getAttribute", player, "visible_rank");
        let shown = match self.players[player].attributes.get("visible_rank") {
            None => Some(rights.ordinal()),
            Some(None) => None,
            Some(Some(Value::Other(name))) => Some(
                ["REGULAR_PLAYER", "PLAYER_MODERATOR", "ADMINISTRATOR"]
                    .iter()
                    .position(|n| n == name)
                    .unwrap() as i32,
            ),
            Some(Some(_)) => return Err(quick_chat::Error::Cast),
        };
        Ok(shown.filter(|o| *o != 0).unwrap_or(0))
    }
    fn publish(&mut self, event: Event) -> quick_chat::Result<()> {
        let Event::ClanMessage {
            sender,
            clan_name,
            message,
            rank,
        } = event
        else {
            panic!("quick chat publishes clan messages only")
        };
        emit_qc!(self, "publish", sender, clan_name, message, rank);
        Ok(())
    }
    fn submit(&mut self, pulse: ChatPulse<P>) -> quick_chat::Result<()> {
        emit_qc!(self, "submit", ChatPulse::<P>::LABEL, ChatPulse::<P>::DELAY);
        self.pulses.push(Pending::Chat(pulse));
        Ok(())
    }
    fn register_chat(&mut self, player: &P, message: &ChatMessage<P>) -> quick_chat::Result<()> {
        emit_qc!(
            self,
            "registerChat",
            player,
            "Chat",
            message.player,
            message.text,
            message.effects,
            message.num_chars,
            message.chat_icon,
            message.is_quick_chat
        );
        Ok(())
    }
    fn log(&mut self, origin: &str, level: Level, message: &str) -> quick_chat::Result<()> {
        let level = match level {
            Level::Fine => "FINE",
            Level::Warn => "WARN",
        };
        emit_qc!(self, "log", origin, level, message);
        Ok(())
    }
    fn dev_mode(&mut self) -> bool {
        commands::Host::dev_mode(self) == Some(true)
    }
}

fn command_error(e: &commands::Error) -> Json {
    match e {
        commands::Error::Index(m) | commands::Error::NumberFormat(m) => json!([e.category(), m]),
        _ => json!([e.category(), null]),
    }
}
fn qc_error(e: &quick_chat::Error) -> Json {
    match e {
        quick_chat::Error::Index(m) => json!([e.category(), m]),
        _ => json!([e.category(), null]),
    }
}

fn unhex(text: &str) -> Vec<u8> {
    (0..text.len() / 2)
        .map(|i| u8::from_str_radix(&text[i * 2..i * 2 + 2], 16).unwrap())
        .collect()
}

/// The registry the behaviour corpus compiles: the four sets in the scan's
/// order, the case's content stand-ins, the three legacy plugins.
fn behaviour_registry(case: &Json) -> Registry {
    let mut registry = registration::register_only(&[
        "core.game.system.command.sets.ModerationCommandSet",
        "core.game.system.command.sets.PlayerAttributeCommandSet",
        "core.game.system.command.sets.SystemCommandSet",
        "core.game.system.command.sets.TeleportCommandSet",
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
            usage: String::new(),
            description: String::new(),
            body: Body::Content(body),
            origin: body,
        });
    }
    for (plugin, sets) in registration::LEGACY {
        for set in *sets {
            registry.link(*set, plugin);
        }
    }
    registry
}

/// One operation: its result, or the error it stopped with.
fn run_op(world: &mut World, registry: &Registry, op: &Json) -> Result<Json, Json> {
    let player = op["player"].as_str().map(str::to_owned);
    let ce = |e: commands::Error| command_error(&e);
    let qe = |e: quick_chat::Error| qc_error(&e);
    match op["op"].as_str().unwrap() {
        "command" => {
            commands::process(
                world,
                registry,
                player.as_ref().unwrap(),
                op["line"].as_str().unwrap(),
            )
            .map_err(ce)?;
            Ok(Json::Null)
        }
        "parse" => commands::parse(
            world,
            registry,
            player.as_ref(),
            op["line"].as_str().unwrap(),
        )
        .map(Json::Bool)
        .map_err(ce),
        "packet" => {
            let p = player.unwrap();
            let payload = unhex(op["hex"].as_str().unwrap());
            match int(&op["opcode"]) {
                44 => match commands::decode(&payload) {
                    commands::Decoded::NoProcess => {
                        world.note(json!(["decoded", ["NoProcess"]]));
                        Ok(Json::Null)
                    }
                    commands::Decoded::Underflow => {
                        commands::Host::message(
                            world,
                            &p,
                            "Something went wrong there! Please try again.",
                        )
                        .map_err(ce)?;
                        world.note(json!(["decoded", ["NoProcess"]]));
                        Ok(Json::Null)
                    }
                    commands::Decoded::Command(line) => {
                        world.note(json!(["decoded", ["Command", line]]));
                        commands::process(world, registry, &p, &line).map_err(ce)?;
                        Ok(Json::Null)
                    }
                },
                99 => match abuse_report::decode(&payload) {
                    Err(message) => {
                        world.note(json!(["decoded", ["DecodingError", message]]));
                        Ok(Json::Null)
                    }
                    Ok(report) => {
                        world.note(json!([
                            "decoded",
                            ["ReportAbuse", report.target, report.rule, report.mod_mute]
                        ]));
                        abuse_report::process(world, registry, &p, &report).map_err(ce)?;
                        Ok(Json::Null)
                    }
                },
                167 => {
                    let (logs, decoded) = quick_chat::decode(&payload);
                    for (level, line) in logs {
                        quick_chat::Host::log(world, quick_chat::DECODER_ORIGIN, level, &line)
                            .map_err(qe)?;
                    }
                    match decoded {
                        Err(e) => {
                            world.note(json!([
                                "decoded",
                                [
                                    "DecodingError",
                                    format!(
                                        "Error decoding opcode 167/QUICKCHAT: {}",
                                        e.java_string()
                                    )
                                ]
                            ]));
                            Ok(Json::Null)
                        }
                        Ok(chat) => {
                            world.note(json!([
                                "decoded",
                                [
                                    "QuickChat",
                                    chat.index_a,
                                    chat.index_b,
                                    chat.for_clan,
                                    chat.multiplier,
                                    chat.offset,
                                    chat.kind.name()
                                ]
                            ]));
                            quick_chat::send(world, &p, &chat).map_err(qe)?;
                            Ok(Json::Null)
                        }
                    }
                }
                other => panic!("opcode {other}"),
            }
        }
        "qcHandler" => {
            quick_chat::handler(
                world,
                player.as_ref().unwrap(),
                &unhex(op["hex"].as_str().unwrap()),
            )
            .map_err(qe)?;
            Ok(Json::Null)
        }
        "pulse" => {
            let index = op["index"].as_u64().unwrap() as usize;
            let length = world.pulses.len();
            let Some(pending) = world.pulses.get(index) else {
                return Err(json!([
                    "Index",
                    format!("Index {index} out of bounds for length {length}")
                ]));
            };
            match pending {
                Pending::Command(p) => {
                    let p = p.clone();
                    deferred::run_pulse(world, &p).map(Json::Bool).map_err(ce)
                }
                Pending::Chat(p) => {
                    let p = p.clone();
                    quick_chat::run_pulse(world, &p).map(Json::Bool).map_err(qe)
                }
            }
        }
        "task" => {
            let task = world.tasks[op["index"].as_u64().unwrap() as usize].clone();
            deferred::run_task(world, &task).map_err(ce)?;
            Ok(Json::Null)
        }
        "input" => {
            let handler = world.inputs[op["index"].as_u64().unwrap() as usize].clone();
            let value = match &op["value"] {
                Json::String(s) => InputValue::Text(s.clone()),
                v => InputValue::Int(int(v)),
            };
            deferred::run_input(world, &handler, &value).map_err(ce)?;
            Ok(Json::Null)
        }
        "job" => {
            let job = world.jobs[op["index"].as_u64().unwrap() as usize].clone();
            deferred::run_job(world, registry, &job).map_err(ce)?;
            Ok(Json::Null)
        }
        "names" => {
            let mut out: Vec<Json> = registry
                .commands()
                .iter()
                .map(|c| json!([c.name, c.privilege.name()]))
                .collect();
            out.push(json!([registry.plugins(LegacySet::Administrator).len()]));
            Ok(Json::Array(out))
        }
        "pages" => Ok(json!(registry.page_indices(int(&op["rights"])))),
        other => panic!("op {other}"),
    }
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
    let registry = format!("{:x}", Sha256::digest(REGISTRY.as_bytes()));
    assert_eq!(provenance["registry"]["fixtureSha256"], json!(registry));
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
            .take(12)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn registration_matches_the_booted_original() {
    let fixture: Json = serde_json::from_str(REGISTRY).unwrap();
    let registry = registration::register_all(&registration::pinned_boot_active);
    let native: Vec<Json> = registry
        .names()
        .iter()
        .map(|name| {
            let c = registry.get(name).unwrap();
            json!([
                name,
                c.name,
                c.privilege.name(),
                c.usage,
                c.description,
                format!("{}$defineCommands${}", c.origin.class, c.origin.lambda)
            ])
        })
        .collect();
    assert_eq!(Json::Array(native), fixture["registry"]);
    for set in LegacySet::ALL {
        let plugins: Vec<&str> = registry.plugins(set).to_vec();
        assert_eq!(
            json!(plugins),
            fixture["legacySets"][set.name()],
            "{}",
            set.name()
        );
    }
    let pages: Vec<Vec<i32>> = (0..=3).map(|r| registry.page_indices(r)).collect();
    assert_eq!(json!(pages), fixture["pageIndices"]);
    // The world-trace boot record pins the same name set (sorted digest).
    let boot: Json = serde_json::from_str(WORLD_TRACE.lines().nth(1).unwrap()).unwrap();
    let pinned = &boot["registries"]["core.game.system.command.CommandMapping"]["mapping"];
    let mut sorted = registry.names();
    sorted.sort();
    assert_eq!(pinned["size"], json!(sorted.len()));
    assert_eq!(
        pinned["keysSha256"],
        json!(format!(
            "{:x}",
            Sha256::digest(sorted.join("\n").as_bytes())
        ))
    );
}

#[test]
fn ported_bodies_are_the_ported_sets() {
    let registry = registration::register_all(&registration::pinned_boot_active);
    let mut native = 0;
    for command in registry.commands() {
        if let Body::Native(_) = command.body {
            native += 1;
            assert!(
                [
                    "core.game.system.command.sets.ModerationCommandSet",
                    "core.game.system.command.sets.SystemCommandSet",
                    "core.game.system.command.sets.TeleportCommandSet",
                    "core.game.system.command.sets.PlayerAttributeCommandSet",
                ]
                .contains(&command.origin.class)
            );
        }
    }
    // Moderation 7 (its getattribute is registered over), System 10,
    // Teleport 5, PlayerAttribute 2.
    assert_eq!(native, 24);
    // An active EasterEvent adds its one command.
    let easter = registration::register_all(&|class| class.ends_with("EasterEvent"));
    assert_eq!(easter.len(), registry.len() + 1);
    assert!(easter.get("eggspawntest").is_some());
}

#[test]
fn kotlin_conversions() {
    assert_eq!(kotlin::to_int_or_null("+"), None);
    assert_eq!(kotlin::to_int_or_null("-2147483648"), Some(i32::MIN));
    assert_eq!(kotlin::to_int_or_null("2147483648"), None);
    assert_eq!(kotlin::float_to_string(1.5), "1.5");
    assert_eq!(kotlin::float_to_string(1000.0), "1000.0");
    assert_eq!(kotlin::float_to_string(1.0e7), "1.0E7");
    assert_eq!(kotlin::float_to_string(1.0e-4), "1.0E-4");
    assert_eq!(kotlin::float_to_string(0.001), "0.001");
    assert_eq!(
        commands::decode(b"KICK Bob\0"),
        commands::Decoded::Command("kick bob".into())
    );
}
