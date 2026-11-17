//! Replays the global handler corpus (fixtures/global-handlers) through
//! `world::global_handlers`. Every service answers from the case the way the
//! oracle's recording adapters do (oracle/global-handlers/adapters.py and
//! Support.java); every recorded call (registrations included), result,
//! error and the attributes and values after each operation are compared.
use rs09_server::definitions::item::Shared;
use rs09_server::player::details::Rights;
use rs09_server::world::global_handlers::{
    self as gh, ClanField, ClanRank, DialogueArg, Error, Expression, Extension, Host, Input,
    IronmanMode, Kind, LogLevel, ObjectKind, Outcome, PrayerType, PulseType, Quest, Result,
    ScriptArg, Song, SpellBook, Store, Task, TeleportType, Toggle, Value,
    registry::{
        self, Class, ComponentPlugin, Interaction, Listener, LoginPlugin, OptionPlugin, Registrar,
    },
};
use rs09_server::world::interaction_listeners::Kind as IntType;
use rs09_server::world::interface_listeners::{self, Button, Callback};
use rs09_server::world::player_managers::diary_data::TYPES as DIARIES;
use rs09_server::world::trade::RequestType;
use serde_json::{Map, Value as Json, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};

const CORPUS: &str = include_str!("../../../fixtures/global-handlers/oracle.jsonl");
const PROVENANCE: &str = include_str!("../../../fixtures/global-handlers/provenance.json");

type N = String;
type T = Task<N, ()>;

/// What the oracle tracks by identity: deferred bodies and the climb dialogue.
#[derive(Clone)]
enum Tracked {
    Task(T),
    Dialogue(gh::climb::ClimbDialogue<N>),
}

fn int(v: &Json) -> i32 {
    v.as_i64().unwrap() as i32
}

/// An object attribute: a Kotlin function or a plugin.
#[derive(Clone, Debug, PartialEq)]
enum Object {
    Callback(String),
    Plugin(String),
}

#[derive(Clone)]
struct NodeData {
    kind: Kind,
    id: i32,
    name: Option<String>,
    location: Option<[i32; 3]>,
    amount: i32,
    attrs: BTreeMap<String, Value<Object>>,
    rotation: i32,
    size: [i32; 2],
    walking_flag: i32,
    options: Option<Vec<Option<String>>>,
}

fn location(v: &Json) -> Option<[i32; 3]> {
    v.as_array().map(|a| [int(&a[0]), int(&a[1]), int(&a[2])])
}
fn coord(l: Option<[i32; 3]>) -> Json {
    l.map_or(Json::Null, |l| json!(l))
}

fn attr_in(v: &Json) -> Value<Object> {
    match v {
        Json::Bool(b) => Value::Bool(*b),
        Json::String(s) => Value::Text(s.clone()),
        Json::Number(n) => Value::Int(n.as_i64().unwrap() as i32),
        Json::Object(o) => {
            if let Some(c) = o.get("callback") {
                Value::Object(Object::Callback(c.as_str().unwrap().into()))
            } else if let Some(p) = o.get("plugin") {
                Value::Object(Object::Plugin(p.as_str().unwrap().into()))
            } else if let Some(l) = o.get("location") {
                Value::Location(location(l).unwrap())
            } else if let Some(l) = o.get("long") {
                Value::Long(l.as_i64().unwrap())
            } else {
                panic!("attribute {v}")
            }
        }
        _ => panic!("attribute {v}"),
    }
}
fn attr_out(v: &Value<Object>) -> Json {
    match v {
        Value::Int(i) => json!(i),
        Value::Long(l) => json!({ "long": l }),
        Value::Bool(b) => json!(b),
        Value::Text(t) => json!(t),
        Value::Location(l) => json!({ "location": l }),
        Value::Object(Object::Callback(c)) => json!({ "callback": c }),
        Value::Object(Object::Plugin(p)) => json!({ "plugin": p }),
    }
}

/// What the test registrar registered, by object identity.
#[derive(Default)]
struct Labels {
    next: usize,
}

struct World {
    nodes: BTreeMap<String, NodeData>,
    containers: BTreeMap<String, Vec<Option<String>>>,
    values: Map<String, Json>,
    answers: Map<String, Json>,
    answer_calls: HashMap<String, usize>,
    songs: Vec<Song>,
    unlocked: BTreeMap<i32, Song>,
    settings: bool,
    server_name: String,
    quick_open: bool,
    note_enabled: bool,
    note_uim: bool,
    now: i64,
    tracked: Vec<Tracked>,
    ladders: gh::climb::Ladders,
    // registrations
    labels: Labels,
    listeners: interface_listeners::Registry,
    callback_labels: Vec<(Callback, String)>,
    components: BTreeMap<i32, (usize, ComponentPlugin)>,
    player_options: BTreeMap<String, (usize, OptionPlugin)>,
    scenery_options: BTreeMap<String, (usize, OptionPlugin)>,
    scenery_handlers: BTreeMap<(i32, String), (usize, OptionPlugin)>,
    interactions: Vec<Interaction>,
    object_labels: HashMap<usize, String>,
    // recording
    hooks: Vec<Json>,
    events: Vec<Json>,
    calls: HashMap<String, i64>,
    op_calls: i64,
    fail_at: i64,
}

impl World {
    fn new(case: &Json) -> Self {
        let mut w = Self {
            nodes: BTreeMap::new(),
            containers: BTreeMap::from([
                ("inventory".to_owned(), vec![None; 28]),
                ("equipment".to_owned(), vec![None; 14]),
                ("kept".to_owned(), vec![None; 4]),
                ("lost".to_owned(), vec![None; 4]),
                ("bob".to_owned(), vec![None; 4]),
            ]),
            values: Map::new(),
            answers: Map::new(),
            answer_calls: HashMap::new(),
            songs: Vec::new(),
            unlocked: BTreeMap::new(),
            settings: true,
            server_name: "2009scape".into(),
            quick_open: false,
            note_enabled: true,
            note_uim: true,
            now: 1_700_000_000_000,
            tracked: Vec::new(),
            ladders: gh::climb::Ladders::original(),
            labels: Labels::default(),
            listeners: interface_listeners::Registry::default(),
            callback_labels: Vec::new(),
            components: BTreeMap::new(),
            player_options: BTreeMap::new(),
            scenery_options: BTreeMap::new(),
            scenery_handlers: BTreeMap::new(),
            interactions: Vec::new(),
            object_labels: HashMap::new(),
            hooks: case["hooks"].as_array().cloned().unwrap_or_default(),
            events: Vec::new(),
            calls: HashMap::new(),
            op_calls: 0,
            fail_at: -1,
        };
        w.set(&json!({"nodes": {"p": {"kind": "player", "name": "tracer", "location": [3222, 3218, 0]}}}));
        w.set(case);
        w
    }

    fn set(&mut self, v: &Json) {
        if let Some(ns) = v.get("nodes").and_then(Json::as_object) {
            for (k, spec) in ns {
                let node = self.nodes.entry(k.clone()).or_insert_with(|| NodeData {
                    kind: match spec["kind"].as_str().unwrap() {
                        "player" => Kind::Player,
                        "bot" => Kind::Bot,
                        "npc" => Kind::Npc,
                        "banker" => Kind::Banker,
                        "scenery" => Kind::Scenery,
                        "item" => Kind::Item,
                        other => panic!("{other}"),
                    },
                    id: 0,
                    name: None,
                    location: None,
                    amount: 1,
                    attrs: BTreeMap::new(),
                    rotation: 0,
                    size: [1, 1],
                    walking_flag: 0,
                    options: Some(Vec::new()),
                });
                if let Some(id) = spec.get("id") {
                    node.id = int(id);
                }
                if let Some(name) = spec.get("name") {
                    node.name = name.as_str().map(str::to_owned);
                }
                if let Some(l) = spec.get("location") {
                    node.location = location(l);
                }
                if let Some(a) = spec.get("amount") {
                    node.amount = int(a);
                }
                if let Some(r) = spec.get("rotation") {
                    node.rotation = int(r);
                }
                if let Some(s) = spec.get("size") {
                    node.size = [int(&s[0]), int(&s[1])];
                }
                if let Some(w) = spec.get("walkingFlag") {
                    node.walking_flag = int(w);
                }
                if let Some(o) = spec.get("options") {
                    node.options = o
                        .as_array()
                        .map(|l| l.iter().map(|x| x.as_str().map(str::to_owned)).collect());
                }
            }
        }
        if let Some(cs) = v.get("containers").and_then(Json::as_object) {
            for (k, slots) in cs {
                let slots = slots.as_array().unwrap();
                let items = slots
                    .iter()
                    .map(|s| {
                        s.as_str()
                            .filter(|k| self.nodes.contains_key(*k))
                            .map(str::to_owned)
                    })
                    .collect();
                self.containers.insert(k.clone(), items);
            }
        }
        if let Some(attrs) = v.get("attributes").and_then(Json::as_object) {
            for (k, map) in attrs {
                let node = self.nodes.get_mut(k).unwrap();
                for (a, value) in map.as_object().unwrap() {
                    if value.is_null() {
                        node.attrs.remove(a);
                    } else {
                        node.attrs.insert(a.clone(), attr_in(value));
                    }
                }
            }
        }
        if let Some(vs) = v.get("values").and_then(Json::as_object) {
            for (k, x) in vs {
                self.values.insert(k.clone(), x.clone());
            }
        }
        if let Some(a) = v.get("answers").and_then(Json::as_object) {
            for (k, x) in a {
                self.answers.insert(k.clone(), x.clone());
            }
        }
        if let Some(songs) = v.get("songs").and_then(Json::as_array) {
            self.songs = songs
                .iter()
                .map(|s| Song {
                    id: int(&s[0]),
                    index: int(&s[1]),
                })
                .collect();
        }
        if let Some(u) = v.get("unlocked").and_then(Json::as_object) {
            self.unlocked = u
                .iter()
                .map(|(k, s)| {
                    (
                        k.parse().unwrap(),
                        Song {
                            id: int(&s[0]),
                            index: int(&s[1]),
                        },
                    )
                })
                .collect();
        }
        if let Some(s) = v.get("settings") {
            self.settings = s == &Json::Bool(true);
        }
        if let Some(k) = v.get("constants").and_then(Json::as_object) {
            if let Some(n) = k.get("SERVER_NAME") {
                self.server_name = n.as_str().unwrap().into();
            }
            if let Some(b) = k.get("BANK_BOOTH_QUICK_OPEN") {
                self.quick_open = b == &Json::Bool(true);
            }
            if let Some(b) = k.get("BANK_BOOTH_NOTE_ENABLED") {
                self.note_enabled = b == &Json::Bool(true);
            }
            if let Some(b) = k.get("BANK_BOOTH_NOTE_UIM") {
                self.note_uim = b == &Json::Bool(true);
            }
        }
    }

    fn emit(&mut self, event: Json) -> Result<()> {
        let name = event[0].as_str().unwrap().to_owned();
        self.events.push(event);
        self.op_calls += 1;
        let count = {
            let c = self.calls.entry(name.clone()).or_insert(0);
            *c += 1;
            *c
        };
        for hook in self.hooks.clone() {
            let at = hook.get("at").map_or(1, |v| v.as_i64().unwrap());
            if hook["event"] == json!(name) && count == at {
                for action in hook["actions"].as_array().unwrap() {
                    if action["op"] == "fail" {
                        return Err(Error::Binding(None));
                    }
                    self.set(action);
                }
            }
        }
        if self.op_calls == self.fail_at {
            return Err(Error::Binding(None));
        }
        Ok(())
    }

    fn answer(&mut self, name: &str, fallback: Json) -> Json {
        let Some(v) = self.answers.get(name).cloned() else {
            return fallback;
        };
        match v {
            Json::Array(list) => {
                let i = self.answer_calls.entry(name.to_owned()).or_insert(0);
                *i += 1;
                list[(*i - 1).min(list.len() - 1)].clone()
            }
            other => other,
        }
    }
    fn answer_bool(&mut self, name: &str, fallback: bool) -> bool {
        self.answer(name, Json::Bool(fallback)) == Json::Bool(true)
    }
    fn answer_int(&mut self, name: &str, fallback: i32) -> i32 {
        int(&self.answer(name, json!(fallback)))
    }
    fn answer_node(&mut self, name: &str, fallback: Json) -> Option<N> {
        self.answer(name, fallback)
            .as_str()
            .filter(|k| self.nodes.contains_key(*k))
            .map(str::to_owned)
    }
    fn data(&self, n: &N) -> &NodeData {
        &self.nodes[n]
    }
    fn track(&mut self, task: T) -> usize {
        self.tracked.push(Tracked::Task(task));
        self.tracked.len() - 1
    }
    fn label(&mut self, identity: usize, class: &str) -> String {
        if let Some(l) = self.object_labels.get(&identity) {
            return l.clone();
        }
        self.labels.next += 1;
        let l = format!("{class}#{}", self.labels.next);
        self.object_labels.insert(identity, l.clone());
        l
    }
    fn callback_label(&mut self, callback: &Callback, class: &str) -> String {
        let l = self.label(callback.identity(), class);
        self.callback_labels.push((callback.clone(), l.clone()));
        l
    }
    fn container(&self, c: &str) -> &Vec<Option<String>> {
        &self.containers[c]
    }
}

// ---------------------------------------------------------------------- the registrar

/// Objects created by the test registrar: identities for labels.
static NEXT_OBJECT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(1 << 40);
fn fresh() -> usize {
    NEXT_OBJECT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

impl Registrar for World {
    type PluginRef = (usize, ComponentPlugin);
    type HandlerRef = (usize, OptionPlugin);
    fn plugin_object(&mut self, plugin: ComponentPlugin) -> Self::PluginRef {
        (fresh(), plugin)
    }
    fn handler_object(&mut self, handler: OptionPlugin) -> Self::HandlerRef {
        (fresh(), handler)
    }
    fn on_button(&mut self, component: i32, button: Option<i32>, listener: Listener) -> Result<()> {
        let callback = Callback::button(listener);
        let label = self.callback_label(&callback, &listener.class());
        self.emit(json!(["ifaceOn", component, button, label]))?;
        self.listeners.add(component, button, callback).unwrap();
        Ok(())
    }
    fn on_open(&mut self, component: i32, listener: Listener) -> Result<()> {
        let callback = Callback::open(listener);
        let label = self.callback_label(&callback, &listener.class());
        self.emit(json!(["ifaceOnOpen", component, label]))?;
        self.listeners.add_open(component, callback).unwrap();
        Ok(())
    }
    fn on_close(&mut self, component: i32, listener: Listener) -> Result<()> {
        let callback = Callback::open(listener);
        let label = self.callback_label(&callback, &listener.class());
        self.emit(json!(["ifaceOnClose", component, label]))?;
        self.listeners.add_close(component, callback).unwrap();
        Ok(())
    }
    fn set_component_plugin(&mut self, id: i32, plugin: &Self::PluginRef) -> Result<()> {
        let label = self.label(plugin.0, plugin.1.class());
        self.emit(json!(["componentPlugin", id, label]))?;
        self.components.insert(id, *plugin);
        Ok(())
    }
    fn set_player_option(&mut self, option: &str, handler: &Self::HandlerRef) -> Result<()> {
        let label = self.label(handler.0, handler.1.class());
        self.emit(json!(["setHandler", option, label]))?;
        self.player_options.insert(option.into(), *handler);
        Ok(())
    }
    fn set_scenery_option(&mut self, option: &str, handler: &Self::HandlerRef) -> Result<()> {
        let label = self.label(handler.0, handler.1.class());
        self.emit(json!(["sceneryOptionHandler", option, label]))?;
        self.scenery_options.insert(option.into(), *handler);
        Ok(())
    }
    fn put_scenery_handler(
        &mut self,
        id: i32,
        key: &str,
        handler: &Self::HandlerRef,
    ) -> Result<()> {
        let label = self.label(handler.0, handler.1.class());
        self.emit(json!(["sceneryHandler", id, key, label]))?;
        self.scenery_handlers.insert((id, key.into()), *handler);
        Ok(())
    }
    fn on(
        &mut self,
        ids: Option<&[i32]>,
        kind: IntType,
        options: &[&str],
        f: Interaction,
    ) -> Result<()> {
        let label = self.label(fresh(), f.class());
        self.emit(json!(["on", ids, gh_kind(kind), options, label]))?;
        self.interactions.push(f);
        Ok(())
    }
    fn define_interaction(
        &mut self,
        kind: IntType,
        ids: &[i32],
        options: &[&str],
        persistent: bool,
        distance: i32,
        f: Interaction,
    ) -> Result<()> {
        let label = self.label(fresh(), f.class());
        self.emit(json!([
            "defineInteraction",
            gh_kind(kind),
            ids,
            options,
            persistent,
            distance,
            label
        ]))?;
        self.interactions.push(f);
        Ok(())
    }
    fn on_use_any_with(&mut self, kind: IntType, ids: &[i32], f: Interaction) -> Result<()> {
        let label = self.label(fresh(), f.class());
        self.emit(json!(["onUseAnyWith", gh_kind(kind), ids, label]))?;
        self.interactions.push(f);
        Ok(())
    }
    fn login_plugin(&mut self, _: LoginPlugin) -> Result<()> {
        // A LOGIN manifest adds the plugin to a list; the oracle records nothing.
        Ok(())
    }
    fn booth_note_enabled(&mut self) -> bool {
        self.note_enabled
    }
}

fn gh_kind(kind: IntType) -> &'static str {
    match kind {
        IntType::Item => "ITEM",
        IntType::Scenery => "SCENERY",
        IntType::Npc => "NPC",
        IntType::GroundItem => "GROUNDITEM",
        IntType::Player => "PLAYER",
    }
}

fn toggle_name(t: Toggle) -> &'static str {
    match t {
        Toggle::Run => "toggleRun",
        Toggle::ChatEffects => "toggleChatEffects",
        Toggle::SplitPrivateChat => "toggleSplitPrivateChat",
        Toggle::MouseButton => "toggleMouseButton",
        Toggle::AcceptAid => "toggleAcceptAid",
        Toggle::Retaliating => "toggleRetaliating",
        Toggle::SpecialBar => "toggleSpecialBar",
    }
}
fn rank_json(r: Option<ClanRank>) -> Json {
    r.map_or(Json::Null, |r| json!(r.name()))
}
fn diary_json(k: Option<usize>) -> Json {
    k.map_or(Json::Null, |k| json!(DIARIES[k].constant))
}

// ---------------------------------------------------------------------- the services

impl Host for World {
    type Node = N;
    type Container = String;
    type Pulse = usize;
    type Weapon = ();
    type Special = String;
    type Operate = ();
    type Quest = String;
    type Diary = Option<usize>;
    type Clan = ();
    type Session = ();
    type Object = Object;

    fn kind(&self, node: &N) -> Kind {
        self.data(node).kind
    }
    fn id(&mut self, node: &N) -> Result<i32> {
        Ok(self.data(node).id)
    }
    fn name(&mut self, node: &N) -> Result<Option<String>> {
        Ok(self.data(node).name.clone())
    }
    fn location(&mut self, node: &N) -> Result<Option<[i32; 3]>> {
        Ok(self.data(node).location)
    }
    fn amount(&mut self, item: &N) -> Result<i32> {
        Ok(self.data(item).amount)
    }
    fn lowercase_default(&self, text: &str) -> String {
        rs09_game_core::java_lower::lowercase_root(text)
    }
    fn server_name(&mut self) -> String {
        self.server_name.clone()
    }
    fn booth_quick_open(&mut self) -> bool {
        self.quick_open
    }
    fn booth_note_enabled(&mut self) -> bool {
        self.note_enabled
    }
    fn booth_note_uim(&mut self) -> bool {
        self.note_uim
    }
    fn ticks(&mut self) -> i32 {
        self.values.get("ticks").map_or(0, int)
    }
    fn current_time_millis(&mut self) -> i64 {
        self.now
    }
    fn world_id(&mut self) -> Result<i32> {
        if !self.settings {
            return Err(Error::Null);
        }
        Ok(self.values.get("worldId").map_or(1, int))
    }

    fn get_attribute(&mut self, e: &N, key: &str) -> Result<Option<Value<Object>>> {
        self.emit(json!(["getAttribute", e, key]))?;
        Ok(self.data(e).attrs.get(key).cloned())
    }
    fn set_attribute(&mut self, e: &N, key: &str, value: Value<Object>) -> Result<()> {
        self.emit(json!(["setAttribute", e, key, attr_out(&value)]))?;
        self.nodes
            .get_mut(e)
            .unwrap()
            .attrs
            .insert(key.into(), value);
        Ok(())
    }
    fn remove_attribute(&mut self, e: &N, key: &str) -> Result<()> {
        self.emit(json!(["removeAttribute", e, key]))?;
        self.nodes.get_mut(e).unwrap().attrs.remove(key);
        Ok(())
    }
    fn attributes_get(&mut self, e: &N, key: &str) -> Result<Option<Value<Object>>> {
        self.emit(json!(["attributes", e]))?;
        Ok(self.data(e).attrs.get(key).cloned())
    }
    fn api_get_attribute(&mut self, e: &N, key: &str) -> Result<Option<Value<Object>>> {
        self.emit(json!(["apiGetAttribute", e, key]))?;
        Ok(self.data(e).attrs.get(key).cloned())
    }
    fn api_set_attribute(&mut self, e: &N, key: &str, value: Value<Object>) -> Result<()> {
        self.emit(json!(["apiSetAttribute", e, key, attr_out(&value)]))?;
        self.nodes
            .get_mut(e)
            .unwrap()
            .attrs
            .insert(key.into(), value);
        Ok(())
    }
    fn api_remove_attribute(&mut self, e: &N, key: &str) -> Result<()> {
        self.emit(json!(["apiRemoveAttribute", e, key]))?;
        self.nodes.get_mut(e).unwrap().attrs.remove(key);
        Ok(())
    }
    fn api_remove_attributes(&mut self, e: &N, keys: &[&str]) -> Result<()> {
        self.emit(json!(["apiRemoveAttributes", e, keys]))?;
        for k in keys {
            self.nodes.get_mut(e).unwrap().attrs.remove(*k);
        }
        Ok(())
    }
    fn object_kind(&mut self, object: &Object) -> ObjectKind {
        match object {
            Object::Callback(_) => ObjectKind::Function,
            Object::Plugin(_) => ObjectKind::Plugin,
        }
    }
    fn selection_callback(&mut self, object: &Object, skill: i32, _: &N) -> Result<()> {
        let Object::Plugin(r) = object else { panic!() };
        self.emit(json!(["selectionCallback", r, skill]))
    }
    fn caller_invoke(&mut self, object: &Object, skill: i32, player: &N) -> Result<()> {
        let Object::Callback(r) = object else {
            panic!()
        };
        self.emit(json!(["callback", r, skill, player]))
    }
    fn item_select_invoke(&mut self, object: &Object, slot: i32, option: i32) -> Result<()> {
        let Object::Callback(r) = object else {
            panic!()
        };
        self.emit(json!(["callback", r, slot, option]))
    }

    fn api_message(&mut self, _: &N, text: &str) -> Result<()> {
        self.emit(json!(["apiMessage", text]))
    }
    fn message(&mut self, _: &N, text: &str) -> Result<()> {
        self.emit(json!(["message", text]))
    }
    fn packet_message(&mut self, _: &N, text: &str) -> Result<()> {
        self.emit(json!(["packetMessage", text]))
    }
    fn packet_message_delay(&mut self, _: &N, text: &str, ticks: i32) -> Result<()> {
        self.emit(json!(["packetMessageDelay", text, ticks]))
    }
    fn packet_string(&mut self, _: &N, text: &str, iface: i32, child: i32) -> Result<()> {
        self.emit(json!(["packetString", text, iface, child]))
    }
    fn set_interface_text(&mut self, _: &N, text: &str, iface: i32, child: i32) -> Result<()> {
        self.emit(json!(["setInterfaceText", text, iface, child]))
    }
    fn dialogue(&mut self, _: &N, text: &str) -> Result<()> {
        self.emit(json!(["dialogue", text]))
    }
    fn npc_dialogue(
        &mut self,
        _: &N,
        npc: i32,
        text: &str,
        e: Expression,
        hide: bool,
    ) -> Result<()> {
        self.emit(json!(["npcDialogue", npc, text, e.name(), hide]))
    }
    fn item_dialogue(&mut self, _: &N, item: i32, text: &str) -> Result<()> {
        self.emit(json!(["itemDialogue", item, text]))
    }
    fn double_item_dialogue(&mut self, _: &N, first: i32, second: i32, text: &str) -> Result<()> {
        self.emit(json!(["doubleItemDialogue", first, second, text]))
    }
    fn open_dialogue(&mut self, _: &N, dialogue: i32, args: Vec<DialogueArg>) -> Result<()> {
        let args: Vec<Json> = args
            .iter()
            .map(|a| match a {
                DialogueArg::NewNpc { id, location } => json!(format!(
                    "new NPC {id} {}",
                    location.map_or("null".to_owned(), |l| format!(
                        "[{}, {}, {}]",
                        l[0], l[1], l[2]
                    ))
                )),
            })
            .collect();
        self.emit(json!(["openDialogue", dialogue, args]))
    }
    fn dialogue_contains(&mut self, id: i32) -> Result<bool> {
        self.emit(json!(["dialogueContains", id]))?;
        Ok(self.answer_bool("dialogueContains", false))
    }
    fn open_interface(&mut self, _: &N, id: i32) -> Result<()> {
        self.emit(json!(["openInterface", id]))
    }
    fn close_interface(&mut self, _: &N) -> Result<()> {
        self.emit(json!(["closeInterface"]))
    }
    fn open_single_tab(&mut self, _: &N, id: i32) -> Result<()> {
        self.emit(json!(["openSingleTab", id]))
    }
    fn close_tab_interface(&mut self, _: &N) -> Result<()> {
        self.emit(json!(["closeTabInterface"]))
    }
    fn open(&mut self, _: &N, id: i32) -> Result<bool> {
        self.emit(json!(["open", id]))?;
        Ok(!self.answer_bool("openNull", false))
    }
    fn set_close_event(&mut self, _: &N, id: i32, event: T) -> Result<()> {
        let n = self.track(event);
        self.emit(json!(["setCloseEvent", id, n]))
    }
    fn opened(&mut self, _: &N) -> Result<bool> {
        self.emit(json!(["opened"]))?;
        Ok(self.answer_bool("opened", false))
    }
    fn close_manager(&mut self, _: &N) -> Result<bool> {
        self.emit(json!(["closeManager"]))?;
        Ok(self.answer_bool("closeManager", true))
    }
    fn close_single_tab(&mut self, _: &N) -> Result<bool> {
        self.emit(json!(["closeSingleTab"]))?;
        Ok(true)
    }
    fn remove_tabs(&mut self, _: &N, tabs: &[i32]) -> Result<()> {
        self.emit(json!(["removeTabs", tabs]))
    }
    fn open_default_tabs(&mut self, _: &N) -> Result<()> {
        self.emit(json!(["openDefaultTabs"]))
    }
    fn open_tab(&mut self, _: &N, slot: i32, id: i32) -> Result<()> {
        self.emit(json!(["openTab", slot, id]))
    }
    fn open_tab_weapon(&mut self, _: &N, _: &()) -> Result<()> {
        self.emit(json!(["openTabWeapon"]))
    }
    fn open_windows_pane(&mut self, _: &N, id: i32, kind: Option<i32>) -> Result<()> {
        self.emit(json!(["openWindowsPane", id, kind]))
    }
    fn is_resizable(&mut self, _: &N) -> Result<bool> {
        self.emit(json!(["isResizable"]))?;
        Ok(self.answer_bool("isResizable", false))
    }
    fn component_open(&mut self, _: &N, id: i32) -> Result<()> {
        self.emit(json!(["componentOpen", id]))
    }
    fn run_script(&mut self, _: &N, id: i32, types: &str, args: Vec<ScriptArg>) -> Result<()> {
        let args: Vec<Json> = args
            .into_iter()
            .map(|a| match a {
                ScriptArg::Int(i) => json!(i),
                ScriptArg::Text(t) => json!(t),
            })
            .collect();
        self.emit(json!(["runScript", id, types, args]))
    }
    fn iface_settings(
        &mut self,
        _: &N,
        hash: i32,
        child: i32,
        iface: i32,
        offset: i32,
        length: i32,
    ) -> Result<()> {
        self.emit(json!(["ifaceSettings", hash, child, iface, offset, length]))
    }
    fn interface_config(&mut self, _: &N, iface: i32, child: i32, hide: bool) -> Result<()> {
        self.emit(json!(["interfaceConfig", iface, child, hide]))
    }
    fn script_configs(
        &mut self,
        _: &N,
        id: i32,
        value: i32,
        kind: &str,
        params: Vec<ScriptArg>,
    ) -> Result<()> {
        let params: Vec<Json> = params
            .into_iter()
            .map(|a| match a {
                ScriptArg::Int(i) => json!(i),
                ScriptArg::Text(t) => json!(t),
            })
            .collect();
        self.emit(json!(["scriptConfigs", id, value, kind, params]))
    }
    fn generate_items(
        &mut self,
        _: &N,
        items: Vec<Option<N>>,
        options: &[&str],
        iface: i32,
        child: i32,
        x: i32,
        y: i32,
        key: Option<i32>,
    ) -> Result<()> {
        self.emit(json!([
            "generateItems",
            items,
            options,
            iface,
            child,
            x,
            y,
            key
        ]))
    }
    fn update_scene_graph(&mut self, _: &N, login: bool) -> Result<()> {
        self.emit(json!(["updateSceneGraph", login]))
    }
    fn log(&mut self, level: LogLevel, text: &str) -> Result<()> {
        self.emit(json!(["log", level.name(), text]))
    }
    fn debug(&mut self, _: &N, text: &str) -> Result<()> {
        self.emit(json!(["debug", text]))
    }

    fn movement_pulse(&mut self, mover: &N, target: Option<&N>, body: T) -> Result<usize> {
        let n = self.track(body);
        self.emit(json!(["movementPulse", mover, target, "FOLLOW_ENTITY", n]))?;
        Ok(n)
    }
    fn new_pulse(&mut self, body: T) -> Result<usize> {
        Ok(self.track(body))
    }
    fn run_pulse(&mut self, _: &N, pulse: &usize, kind: Option<PulseType>) -> Result<()> {
        self.emit(json!(["run", pulse, kind.map(PulseType::name)]))
    }
    fn base_stop(&mut self, _: &usize) -> Result<()> {
        // Pulse.stop: `running = false`, no recorded call.
        Ok(())
    }
    fn movement_stop(&mut self, pulse: &usize) -> Result<()> {
        self.emit(json!(["pulseStop", pulse]))
    }
    fn submit(&mut self, _: i32, _: &[&N], body: T) -> Result<()> {
        let n = self.track(body);
        self.emit(json!(["submit", n]))
    }
    fn run_task(&mut self, _: &N, delay: i32, repeat: i32, body: T) -> Result<()> {
        let n = self.track(body);
        self.emit(json!(["runTask", delay, repeat, n]))
    }
    fn run_world_task(&mut self, body: T) -> Result<()> {
        let n = self.track(body);
        self.emit(json!(["runWorldTask", n]))
    }
    fn input_dialogue(&mut self, _: &N, numeric: bool, prompt: &str, body: T) -> Result<()> {
        let n = self.track(body);
        self.emit(json!(["inputDialogue", numeric, prompt, n]))
    }

    fn attack(&mut self, _: &N, node: Option<&N>) -> Result<()> {
        self.emit(json!(["attack", node]))
    }
    fn face(&mut self, e: &N, target: Option<&N>) -> Result<()> {
        self.emit(json!(["face", e, target]))
    }
    fn face_location(&mut self, e: &N, l: Option<[i32; 3]>) -> Result<()> {
        self.emit(json!(["faceLocation", e, coord(l)]))
    }
    fn in_combat(&mut self, e: &N) -> Result<bool> {
        self.emit(json!(["inCombat", e]))?;
        Ok(self.answer_bool(&format!("inCombat:{e}"), false))
    }
    fn lock_all(&mut self, e: &N) -> Result<()> {
        self.emit(json!(["lockAll", e]))
    }
    fn lock_ticks(&mut self, e: &N, ticks: i32) -> Result<()> {
        self.emit(json!(["lockTicks", e, ticks]))
    }
    fn unlock(&mut self, e: &N) -> Result<()> {
        self.emit(json!(["unlock", e]))
    }
    fn animate_entity(&mut self, e: &N, animation: Option<i32>) -> Result<()> {
        self.emit(json!(["animatePlayer", e, animation]))
    }
    fn api_animate(&mut self, e: &N, animation: i32, forced: bool) -> Result<()> {
        self.emit(json!(["animate", e, animation, forced]))
    }
    fn visualize(&mut self, e: &N, animation: i32, graphics: i32) -> Result<()> {
        self.emit(json!(["visualize", e, animation, graphics]))
    }
    fn lock_key(&mut self, _: &N, key: &str, ticks: i32) -> Result<()> {
        self.emit(json!(["lock", key, ticks]))
    }
    fn is_locked(&mut self, _: &N, key: &str) -> Result<bool> {
        self.emit(json!(["isLocked", key]))?;
        Ok(self.answer_bool("isLocked", false))
    }
    fn interaction_locked(&mut self, _: &N) -> Result<bool> {
        self.emit(json!(["interactionLocked"]))?;
        Ok(self.answer_bool("interactionLocked", false))
    }
    fn movement_locked(&mut self, _: &N) -> Result<bool> {
        self.emit(json!(["movementLocked"]))?;
        Ok(self.answer_bool("movementLocked", false))
    }
    fn teleport(&mut self, _: &N, l: Option<[i32; 3]>, _: TeleportType) -> Result<bool> {
        self.emit(json!(["teleport", coord(l), "TELE_OTHER"]))?;
        Ok(self.answer_bool("teleport", false))
    }
    fn play_audio(&mut self, _: &N, id: i32, delay: i32, loops: i32) -> Result<()> {
        self.emit(json!(["playAudio", id, delay, loops]))
    }

    fn is_artificial(&mut self, _: &N) -> Result<bool> {
        self.emit(json!(["isArtificial"]))?;
        Ok(self.answer_bool("isArtificial", false))
    }
    fn valid_login(&mut self, _: &N) -> Result<bool> {
        self.emit(json!(["validLogin"]))?;
        Ok(self.answer_bool("validLogin", true))
    }
    fn dev_mode(&mut self) -> Result<bool> {
        self.emit(json!(["devMode"]))?;
        Ok(self.answer_bool("devMode", false))
    }
    fn toggle_debug(&mut self, _: &N) -> Result<()> {
        self.emit(json!(["toggleDebug"]))
    }
    fn is_admin(&mut self, _: &N) -> Result<bool> {
        self.emit(json!(["isAdmin"]))?;
        Ok(self.answer_bool("isAdmin", false))
    }
    fn username(&mut self, _: &N) -> Result<String> {
        self.emit(json!(["username"]))?;
        Ok(self
            .values
            .get("username")
            .and_then(Json::as_str)
            .unwrap_or("tracer")
            .into())
    }
    fn rights(&mut self, _: &N) -> Result<Rights> {
        self.emit(json!(["rights"]))?;
        Ok(
            match self
                .answer("rights", json!("REGULAR_PLAYER"))
                .as_str()
                .unwrap()
            {
                "REGULAR_PLAYER" => Rights::RegularPlayer,
                "PLAYER_MODERATOR" => Rights::PlayerModerator,
                _ => Rights::Administrator,
            },
        )
    }
    fn is_teleporting(&mut self, _: &N) -> Result<bool> {
        self.emit(json!(["isTeleporting"]))?;
        Ok(self.answer_bool("isTeleporting", false))
    }
    fn can_logout(&mut self, _: &N) -> Result<bool> {
        self.emit(json!(["canLogout"]))?;
        Ok(self.answer_bool("canLogout", true))
    }
    fn zone_type(&mut self, _: &N) -> Result<i32> {
        self.emit(json!(["zoneType"]))?;
        Ok(self.answer_int("zoneType", 0))
    }
    fn disconnect(&mut self, _: &N) -> Result<()> {
        self.emit(json!(["disconnect"]))
    }
    fn toggle(&mut self, _: &N, toggle: Toggle) -> Result<()> {
        self.emit(json!([toggle_name(toggle)]))
    }
    fn special_toggled(&mut self, _: &N) -> Result<bool> {
        self.emit(json!(["isSpecialToggled"]))?;
        Ok(self.answer_bool("isSpecialToggled", false))
    }
    fn check_restriction(&mut self, _: &N, mode: IronmanMode) -> Result<bool> {
        self.emit(json!(["checkRestriction", mode.name()]))?;
        Ok(self.answer_bool("checkRestriction", false))
    }
    fn is_skulled(&mut self, _: &N) -> Result<bool> {
        self.emit(json!(["isSkulled"]))?;
        Ok(self.answer_bool("isSkulled", false))
    }
    fn prayer_active(&mut self, _: &N, prayer: PrayerType) -> Result<bool> {
        self.emit(json!(["prayerActive", prayer.name()]))?;
        Ok(self.answer_bool("prayerActive", false))
    }
    fn prayer_type(&mut self, button: i32) -> Result<Option<PrayerType>> {
        self.emit(json!(["prayerType", button]))?;
        let name = self.answer("prayerType", Json::Null);
        Ok(name
            .as_str()
            .map(|n| *PrayerType::ALL.iter().find(|p| p.name() == n).unwrap()))
    }
    fn prayer_toggle(&mut self, _: &N, prayer: PrayerType) -> Result<bool> {
        self.emit(json!(["prayerToggle", prayer.name()]))?;
        Ok(self.answer_bool("prayerToggle", false))
    }
    fn has_familiar(&mut self, _: &N) -> Result<bool> {
        self.emit(json!(["hasFamiliar"]))?;
        Ok(self.answer_bool("hasFamiliar", false))
    }
    fn is_burden_beast(&mut self, _: &N) -> Result<bool> {
        // familiarManager.familiar (an answer read), then isBurdenBeast.
        self.answer_bool("beastObject", true);
        self.emit(json!(["isBurdenBeast"]))?;
        Ok(self.answer_bool("isBurdenBeast", false))
    }
    fn beast_container(&mut self, _: &N) -> Result<String> {
        if self.answer_bool("beastObject", true) {
            Ok("bob".into())
        } else {
            Err(Error::Cast)
        }
    }
    fn has_timer(&mut self, _: &N, id: &str) -> Result<bool> {
        self.emit(json!(["getTimer", id]))?;
        Ok(self.answer_bool("getTimer", false))
    }
    fn dragon_slayer_attribute(&mut self, _: &N, name: &str) -> Result<bool> {
        self.emit(json!(["dragonSlayerAttribute", name]))?;
        Ok(self.answer_bool("dragonSlayerAttribute", false))
    }
    fn loot_share_delay(&mut self, _: &N) -> Result<i64> {
        self.emit(json!(["lootShareDelay"]))?;
        Ok(self
            .values
            .get("lootShareDelay")
            .map_or(0, |v| v.as_i64().unwrap()))
    }
    fn set_loot_share_delay(&mut self, _: &N, value: i64) -> Result<()> {
        let d = value - self.now;
        let shown = if d > 86_390_000 && d <= 86_400_000 {
            json!("now+86400000")
        } else {
            json!(value)
        };
        self.emit(json!(["setLootShareDelay", shown.clone()]))?;
        // A delay relative to the clock is kept as that label (the oracle reads its own clock).
        self.values.insert("lootShareDelay".into(), shown);
        Ok(())
    }
    fn loot_share_points(&mut self, _: &N) -> Result<i32> {
        self.emit(json!(["lootSharePoints"]))?;
        Ok(self.values.get("lootSharePoints").map_or(0, int))
    }
    fn set_loot_share_points(&mut self, _: &N, value: i32) -> Result<()> {
        self.emit(json!(["setLootSharePoints", value]))?;
        self.values.insert("lootSharePoints".into(), json!(value));
        Ok(())
    }
    fn set_disable_death_screen(&mut self, _: &N, value: bool) -> Result<()> {
        self.emit(json!(["setDisableDeathScreen", value]))
    }
    fn activity_start(&mut self, _: &N, name: &str, login: bool) -> Result<bool> {
        self.emit(json!(["activityStart", name, login]))?;
        Ok(true)
    }
    fn set_varp(&mut self, _: &N, index: i32, value: i32, save: bool) -> Result<()> {
        self.emit(json!(["setVarp", index, value, save]))
    }
    fn set_varbit(&mut self, _: &N, id: i32, value: i32, save: bool) -> Result<()> {
        self.emit(json!(["setVarbit", id, value, save]))
    }
    fn flashing_icons(&mut self, _: &N, skill: i32) -> Result<()> {
        self.emit(json!(["flashingIcons", skill]))
    }
    fn emote(&mut self, _: &N, button: i32) -> Result<()> {
        self.emit(json!(["emote", button]))
    }
    fn character_design(&mut self, _: &N, button: i32) -> Result<()> {
        self.emit(json!(["characterDesign", button]))
    }
    fn configure_game_world(&mut self, _: &N) -> Result<()> {
        self.emit(json!(["configureGameWorld"]))
    }
    fn random(&mut self, low: i32, high: i32) -> Result<i32> {
        self.emit(json!(["random", low, high]))?;
        Ok(self.answer_int("random", low))
    }

    fn request(&mut self, _: &N, target: &N, kind: RequestType) -> Result<bool> {
        let k = match kind {
            RequestType::Trade => "TRADE",
            RequestType::Assist => "ASSIST",
        };
        self.emit(json!(["request", target, k]))?;
        Ok(self.answer_bool("request", true))
    }
    fn bot_info(&mut self, _: &N, bot: &N) -> Result<()> {
        self.emit(json!(["botInfo", bot]))
    }
    fn assist_session(&mut self, _: &N) -> Result<Option<()>> {
        self.emit(json!(["assistSession"]))?;
        Ok(self.answer_bool("assistSession", false).then_some(()))
    }
    fn assist_player(&mut self, _: &()) -> Result<Option<N>> {
        self.emit(json!(["assistPlayer"]))?;
        Ok(self.answer_node("assistPlayer", json!("p")))
    }
    fn assist_toggle(&mut self, _: &(), index: i32) -> Result<()> {
        self.emit(json!(["assistToggle", index]))
    }
    fn assist_refresh(&mut self, _: &()) -> Result<()> {
        self.emit(json!(["assistRefresh"]))
    }

    fn inventory(&mut self, _: &N) -> Result<String> {
        Ok("inventory".into())
    }
    fn equipment(&mut self, _: &N) -> Result<String> {
        Ok("equipment".into())
    }
    fn death_containers(&mut self, _: &N) -> Result<[String; 2]> {
        self.emit(json!(["deathContainers"]))?;
        Ok(["kept".into(), "lost".into()])
    }
    fn get(&mut self, c: &String, slot: i32) -> Result<Option<N>> {
        self.emit(json!(["get", c, slot]))?;
        let items = self.container(c);
        if slot < 0 || slot as usize >= items.len() {
            return Ok(None);
        }
        Ok(items[slot as usize].clone())
    }
    fn get_new_id(&mut self, c: &String, slot: i32) -> Result<i32> {
        self.emit(json!(["getNew", c, slot]))?;
        let items = self.container(c);
        if slot < 0 || slot as usize >= items.len() {
            return Err(Error::Index);
        }
        Ok(items[slot as usize].clone().map_or(0, |k| self.data(&k).id))
    }
    fn item_count(&mut self, c: &String) -> Result<i32> {
        self.emit(json!(["itemCount", c]))?;
        Ok(self.container(c).iter().filter(|i| i.is_some()).count() as i32)
    }
    fn container_id(&mut self, c: &String, slot: i32) -> Result<i32> {
        self.emit(json!(["containerId", c, slot]))?;
        let items = self.container(c);
        if slot as usize >= items.len() {
            return Ok(-1);
        }
        Ok(items[slot as usize]
            .clone()
            .map_or(-1, |k| self.data(&k).id))
    }
    fn is_empty(&mut self, c: &String) -> Result<bool> {
        self.emit(json!(["isEmpty", c]))?;
        Ok(self.container(c).iter().all(Option::is_none))
    }
    fn to_array(&mut self, c: &String) -> Result<Vec<Option<N>>> {
        self.emit(json!(["toArray", c]))?;
        Ok(self.container(c).clone())
    }
    fn get_amount(&mut self, c: &String, item: &N) -> Result<i32> {
        self.emit(json!(["getAmount", c, item]))?;
        let id = self.data(item).id;
        Ok(self
            .container(c)
            .iter()
            .flatten()
            .filter(|k| self.data(k).id == id)
            .map(|k| self.data(k).amount)
            .fold(0i32, i32::wrapping_add))
    }
    fn refresh(&mut self, c: &String) -> Result<()> {
        self.emit(json!(["refresh", c]))
    }
    fn contain_items(&mut self, c: &String, ids: &[i32]) -> Result<bool> {
        self.emit(json!(["containItems", c, ids]))?;
        Ok(self.answer_bool("containItems", false))
    }
    fn add(&mut self, c: &String, id: i32, amount: i32) -> Result<bool> {
        self.emit(json!(["add", c, id, amount]))?;
        Ok(true)
    }
    fn equipment_update(&mut self, _: &N) -> Result<()> {
        self.emit(json!(["equipmentUpdate"]))
    }
    fn unequip(&mut self, _: &N, slot: i32, item: i32) -> Result<()> {
        self.emit(json!(["unequip", slot, item]))
    }
    fn listeners(&mut self, id: i32, kind: IntType, option: &str, _: &N, node: &N) -> Result<bool> {
        self.emit(json!(["listeners", id, gh_kind(kind), option, node]))?;
        Ok(self.answer_bool(&format!("listeners:{option}"), false))
    }
    fn operate_handler(&mut self, item: &N) -> Result<Option<()>> {
        self.emit(json!(["operateHandler", item]))?;
        Ok(self.answer_bool("operateHandler", false).then_some(()))
    }
    fn operate_handle(&mut self, _: &(), _: &N, item: &N, option: &str) -> Result<bool> {
        self.emit(json!(["operateHandle", item, option]))?;
        Ok(self.answer_bool("operateHandle", false))
    }
    fn examine(&mut self, item: &N) -> Result<Option<String>> {
        self.emit(json!(["examine", item]))?;
        Ok(self
            .answer(&format!("examine:{item}"), json!("examine"))
            .as_str()
            .map(str::to_owned))
    }
    fn inventory_options(&mut self, item: &N) -> Result<Option<Vec<Option<String>>>> {
        self.emit(json!(["inventoryOptions", item]))?;
        Ok(self
            .answers
            .get(&format!("options:{item}"))
            .and_then(Json::as_array)
            .map(|l| l.iter().map(|o| o.as_str().map(str::to_owned)).collect()))
    }
    fn note_change(&mut self, item: &N) -> Result<i32> {
        self.emit(json!(["noteChange", item]))?;
        let id = self.data(item).id;
        Ok(self.answer_int(&format!("noteChange:{item}"), id))
    }
    fn is_unnoted(&mut self, item: &N) -> Result<bool> {
        self.emit(json!(["isUnnoted", item]))?;
        Ok(self.answer_bool(&format!("isUnnoted:{item}"), true))
    }
    fn any_in_equipment(&mut self, _: &N, ids: &[i32]) -> Result<bool> {
        self.emit(json!(["anyInEquipment", ids]))?;
        Ok(self.answer_bool("anyInEquipment", false))
    }
    fn remove_item(&mut self, _: &N, id: i32, amount: i32, store: Store) -> Result<bool> {
        self.emit(json!(["removeItem", id, amount, store.name()]))?;
        Ok(self.answer_bool("removeItem", true))
    }
    fn add_item(&mut self, _: &N, id: i32, amount: i32, store: Store) -> Result<bool> {
        self.emit(json!(["addItem", id, amount, store.name()]))?;
        Ok(self.answer_bool(&format!("addItem:{}", store.name()), true))
    }
    fn amount_in_inventory(&mut self, _: &N, id: i32) -> Result<i32> {
        self.emit(json!(["amountInInventory", id]))?;
        Ok(self.answer_int("amountInInventory", 0))
    }
    fn free_slots(&mut self, _: &N) -> Result<i32> {
        self.emit(json!(["freeSlots"]))?;
        Ok(self.answer_int("freeSlots", 0))
    }
    fn item_name(&mut self, id: i32) -> Result<String> {
        self.emit(json!(["itemName", id]))?;
        Ok(self
            .values
            .get(&format!("itemName:{id}"))
            .and_then(Json::as_str)
            .map_or_else(|| format!("item {id}"), str::to_owned))
    }
    fn has_option(&mut self, node: &N, option: &str) -> Result<bool> {
        self.emit(json!(["hasOption", node, option]))?;
        Ok(self.answer_bool("hasOption", true))
    }

    fn open_bank(&mut self, _: &N) -> Result<()> {
        self.emit(json!(["openBank"]))
    }
    fn open_collection_box(&mut self, _: &N) -> Result<()> {
        self.emit(json!(["openCollectionBox"]))
    }
    fn open_deposit_box(&mut self, _: &N) -> Result<()> {
        self.emit(json!(["openDepositBox"]))
    }
    fn bank_add_item(&mut self, _: &N, slot: i32, amount: i32) -> Result<()> {
        self.emit(json!(["bankAddItem", slot, amount]))
    }
    fn refresh_deposit_box(&mut self, _: &N) -> Result<()> {
        self.emit(json!(["refreshDepositBox"]))
    }
    fn transfer_x(&mut self, _: &N, slot: i32, withdraw: bool, after: Option<T>) -> Result<()> {
        let n = after.map_or(-1, |t| self.track(t) as i64);
        self.emit(json!(["transferX", slot, withdraw, n]))
    }
    fn dump_beast_of_burden(&mut self, _: &N) -> Result<()> {
        self.emit(json!(["dumpBob"]))
    }
    fn lunar_restriction(&mut self, _: &N, node: &N) -> Result<bool> {
        self.emit(json!(["lunarRestriction", node]))?;
        Ok(self.answer_bool("lunarRestriction", false))
    }
    fn find_npc(&mut self, l: [i32; 3]) -> Result<Option<N>> {
        self.emit(json!(["findNpc", l]))?;
        Ok(self.answer_node(&format!("findNpc:{},{},{}", l[0], l[1], l[2]), Json::Null))
    }

    fn weapon_interface(&mut self, _: &N) -> Result<Option<Extension<()>>> {
        self.emit(json!(["weaponInterface"]))?;
        Ok(
            match self.answer("weaponInterface", json!("present")).as_str() {
                Some("STAFF") => Some(Extension::StaffConstant),
                Some("present") => Some(Extension::Weapon(())),
                _ => None,
            },
        )
    }
    fn is_special_bar(&mut self, _: &()) -> Result<bool> {
        self.emit(json!(["isSpecialBar"]))?;
        Ok(self.answer_bool("isSpecialBar", false))
    }
    fn set_attack_style(&mut self, _: &(), button: i32) -> Result<bool> {
        self.emit(json!(["setAttackStyle", button]))?;
        Ok(self.answer_bool("setAttackStyle", false))
    }
    fn open_autocast_select(&mut self, _: &()) -> Result<()> {
        self.emit(json!(["openAutocastSelect"]))
    }
    fn select_auto_spell(&mut self, _: &(), button: i32, adjust: bool, forget: bool) -> Result<()> {
        self.emit(json!(["selectAutoSpell", button, adjust, forget]))
    }
    fn update_weapon_interface(&mut self, _: &()) -> Result<()> {
        self.emit(json!(["updateWeaponInterface"]))
    }
    fn has_autocast_spell(&mut self, _: &N) -> Result<bool> {
        self.emit(json!(["autocastSpell"]))?;
        Ok(self.answer_bool("autocastSpell", false))
    }
    fn special_handler(&mut self, item: i32) -> Result<Option<String>> {
        self.emit(json!(["getSpecial", item]))?;
        Ok(self
            .answer("getSpecial", Json::Null)
            .as_str()
            .map(str::to_owned))
    }
    fn special_instant(&mut self, handler: &String, _: &N) -> Result<bool> {
        self.emit(json!(["fireEvent", handler, "instant_spec"]))?;
        Ok(self.answer("fireEvent", Json::Null) == Json::Bool(true))
    }
    fn combat_victim(&mut self, _: &N) -> Result<Option<N>> {
        self.emit(json!(["victim"]))?;
        Ok(self.answer_node("victim", Json::Null))
    }
    fn special_swing(&mut self, handler: &String, _: &N, victim: Option<&N>) -> Result<()> {
        self.emit(json!(["swing", handler, victim]))
    }
    fn spell_listeners(
        &mut self,
        button: i32,
        kind: i32,
        book: &str,
        _: &N,
        node: Option<&N>,
    ) -> Result<()> {
        self.emit(json!(["spellListeners", button, kind, book, node]))
    }
    fn cast_spell(&mut self, _: &N, book: SpellBook, spell: i32, target: &N) -> Result<bool> {
        self.emit(json!(["castSpell", book.name(), spell, target]))?;
        Ok(self.answer_bool("castSpell", false))
    }
    fn attack_animation(&mut self, _: &N) -> Result<i32> {
        self.emit(json!(["attackAnimation"]))?;
        Ok(self.answer_int("attackAnimation", 422))
    }
    fn combat_level(&mut self, _: &N) -> Result<i32> {
        self.emit(json!(["combatLevel"]))?;
        Ok(self.answer_int("combatLevel", 3))
    }
    fn attack_style(&mut self, _: &N) -> Result<i32> {
        self.emit(json!(["attackStyle"]))?;
        Ok(self.answer_int("attackStyle", 0))
    }
    fn add_experience(&mut self, _: &N, skill: i32, experience: f64) -> Result<()> {
        self.emit(json!(["addExperience", skill, experience]))
    }

    fn diary_open_tab(&mut self, _: &N) -> Result<()> {
        self.emit(json!(["diaryOpenTab"]))
    }
    fn quest_for_button(&mut self, _: &N, button: i32) -> Result<Option<String>> {
        self.emit(json!(["questForButton", button]))?;
        Ok(self
            .answer("questForButton", Json::Null)
            .as_str()
            .map(str::to_owned))
    }
    fn quest_stage(&mut self, quest: &String, _: &N) -> Result<i32> {
        self.emit(json!(["questStage", quest]))?;
        Ok(self.answer_int("questStage", 0))
    }
    fn draw_journal(&mut self, quest: &String, _: &N, stage: i32) -> Result<()> {
        self.emit(json!(["drawJournal", quest, stage]))
    }
    fn quest_requirements(&mut self, _: &N, button: i32) -> Result<()> {
        self.emit(json!(["questRequirements", button]))
    }
    fn quest_sync_tab(&mut self, _: &N) -> Result<()> {
        self.emit(json!(["questSyncTab"]))
    }
    fn quest_complete(&mut self, _: &N, quest: Quest) -> Result<bool> {
        self.emit(json!(["questComplete", quest.name()]))?;
        Ok(self.answer_bool(&format!("questComplete:{}", quest.name()), false))
    }
    fn has_completed_all(&mut self, _: &N) -> Result<bool> {
        self.emit(json!(["hasCompletedAll"]))?;
        Ok(self.answer_bool("hasCompletedAll", false))
    }
    fn diary_for_child(&mut self, child: i32) -> Result<Option<usize>> {
        self.emit(json!(["diaryForChild", child]))?;
        let name = self.answer("diaryForChild", Json::Null);
        Ok(name
            .as_str()
            .map(|n| DIARIES.iter().position(|d| d.constant == n).unwrap()))
    }
    fn get_diary(&mut self, _: &N, kind: Option<usize>) -> Result<Option<Option<usize>>> {
        self.emit(json!(["getDiary", diary_json(kind)]))?;
        Ok(self.answer_bool("getDiary", true).then_some(kind))
    }
    fn diary_open(&mut self, diary: &Option<usize>, _: &N) -> Result<()> {
        self.emit(json!(["diaryOpen", diary_json(*diary)]))
    }
    fn toggle_looping(&mut self, _: &N) -> Result<()> {
        self.emit(json!(["toggleLooping"]))
    }
    fn unlocked(&mut self, _: &N, slot: i32) -> Result<Option<Song>> {
        self.emit(json!(["unlocked", slot]))?;
        Ok(self.unlocked.get(&slot).copied())
    }
    fn play(&mut self, _: &N, song: Option<Song>) -> Result<()> {
        self.emit(json!(["play", song.map(|s| s.id)]))
    }
    fn songs(&mut self) -> Result<Vec<Song>> {
        self.emit(json!(["songs"]))?;
        Ok(self.songs.clone())
    }
    fn unlock_song(&mut self, _: &N, id: i32) -> Result<()> {
        self.emit(json!(["unlock", id]))
    }
    fn clan_open_settings(&mut self, _: &N) -> Result<()> {
        self.emit(json!(["clanOpenSettings"]))
    }
    fn toggle_lootshare(&mut self, _: &N) -> Result<()> {
        self.emit(json!(["toggleLootshare"]))
    }
    fn clan_get(&mut self, owner: Option<&str>, create: bool) -> Result<Option<()>> {
        self.emit(json!(["clanGet", owner, create]))?;
        Ok((!self.answer_bool("clanNull", false)).then_some(()))
    }
    fn clan_name(&mut self, _: &()) -> Result<Option<String>> {
        self.emit(json!(["clanName"]))?;
        Ok(match self.values.get("clanName") {
            None => Some("Chat disabled".into()),
            Some(v) => v.as_str().map(str::to_owned),
        })
    }
    fn clan_set_name(&mut self, _: &(), name: &str) -> Result<()> {
        self.emit(json!(["clanSetName", name]))?;
        self.values.insert("clanName".into(), json!(name));
        Ok(())
    }
    fn clan_rank(&mut self, _: &(), field: ClanField) -> Result<Option<ClanRank>> {
        self.emit(json!(["clanGetReq", field.name()]))?;
        Ok(self
            .values
            .get(&format!("clan:{}", field.name()))
            .and_then(Json::as_str)
            .map(|n| *ClanRank::ALL.iter().find(|r| r.name() == n).unwrap()))
    }
    fn clan_set_rank(&mut self, _: &(), field: ClanField, rank: Option<ClanRank>) -> Result<()> {
        self.emit(json!(["clanSet", field.name(), rank_json(rank)]))?;
        self.values
            .insert(format!("clan:{}", field.name()), rank_json(rank));
        Ok(())
    }
    fn clan_update_settings(&mut self, _: &(), _: &N) -> Result<()> {
        self.emit(json!(["clanUpdateSettings"]))
    }
    fn clan_update(&mut self, _: &()) -> Result<()> {
        self.emit(json!(["clanUpdate"]))
    }
    fn clan_delete(&mut self, _: &()) -> Result<()> {
        self.emit(json!(["clanDelete"]))
    }
    fn comm_clan_name(&mut self, _: &N) -> Result<Option<String>> {
        self.emit(json!(["commClanName"]))?;
        Ok(match self.values.get("commClanName") {
            None => Some(String::new()),
            Some(v) => v.as_str().map(str::to_owned),
        })
    }
    fn comm_set_clan_name(&mut self, _: &N, name: &str) -> Result<()> {
        self.emit(json!(["commSetClanName", name]))?;
        self.values.insert("commClanName".into(), json!(name));
        Ok(())
    }
    fn comm_set_rank(&mut self, _: &N, field: ClanField, rank: Option<ClanRank>) -> Result<()> {
        self.emit(json!(["commSet", field.name(), rank_json(rank)]))
    }
    fn world_communicator_enabled(&mut self) -> Result<bool> {
        self.emit(json!(["wcEnabled"]))?;
        Ok(self.answer_bool("wcEnabled", false))
    }
    fn ms_clan_rename(&mut self, _: &N, name: Option<&str>) -> Result<()> {
        self.emit(json!(["msClanRename", name]))
    }
    fn ms_clan_setting(&mut self, _: &N, kind: i32, rank: Option<ClanRank>) -> Result<()> {
        self.emit(json!(["msClanSetting", kind, rank_json(rank)]))
    }

    fn special_destination(&mut self, at: [i32; 3]) -> Result<Option<[i32; 3]>> {
        Ok(self.ladders.destination(at))
    }
    fn special_ladder(
        &mut self,
        at: [i32; 3],
    ) -> Result<Option<&'static gh::climb::SpecialLadder>> {
        Ok(self.ladders.special(at))
    }
    fn scenery_size(&mut self, s: &N) -> Result<[i32; 2]> {
        Ok(self.data(s).size)
    }
    fn scenery_rotation(&mut self, s: &N) -> Result<i32> {
        Ok(self.data(s).rotation)
    }
    fn scenery_walking_flag(&mut self, s: &N) -> Result<i32> {
        Ok(self.data(s).walking_flag)
    }
    fn scenery_options(&mut self, s: &N) -> Result<Option<Vec<Option<String>>>> {
        Ok(self.data(s).options.clone())
    }
    fn get_object(&mut self, at: [i32; 3]) -> Result<Option<N>> {
        self.emit(json!(["getObject", at]))?;
        Ok(self.answer_node(&format!("object:{},{},{}", at[0], at[1], at[2]), Json::Null))
    }
    fn teleport_permitted(&mut self, at: [i32; 3]) -> Result<bool> {
        self.emit(json!(["teleportPermitted", at]))?;
        Ok(self.answer_bool(&format!("teleport:{},{},{}", at[0], at[1], at[2]), true))
    }
    fn clipping_flag(&mut self, at: [i32; 3]) -> Result<i32> {
        self.emit(json!(["clippingFlag", at]))?;
        Ok(self.answer_int(&format!("clip:{},{},{}", at[0], at[1], at[2]), 0))
    }
    fn set_teleport_location(&mut self, _: &N, at: [i32; 3]) -> Result<()> {
        self.emit(json!(["setTeleportLocation", at]))
    }
    fn finish_task(&mut self, _: &N, kind: usize, level: i32, task: i32) -> Result<()> {
        self.emit(json!(["finishTask", DIARIES[kind].constant, level, task]))
    }
    fn zogre_ladder(&mut self, _: &N) -> Result<()> {
        self.emit(json!(["zogreLadder"]))
    }
    fn send_options(&mut self, _: &N, title: &str, options: &[&str]) -> Result<()> {
        self.emit(json!(["sendOptions", title, options]))
    }
    fn set_dialogue(&mut self, _: &N, dialogue: gh::climb::ClimbDialogue<N>) -> Result<()> {
        self.tracked.push(Tracked::Dialogue(dialogue));
        let n = self.tracked.len() - 1;
        self.emit(json!(["setDialogue", n]))
    }
    fn dialogue_end(&mut self, _: &N) -> Result<()> {
        self.emit(json!(["dialogueEnd"]))
    }
}

// ---------------------------------------------------------------------- operations

fn button(op: &Json) -> Button {
    Button {
        opcode: int(&op["opcode"]),
        button: int(&op["button"]),
        slot: int(&op["slot"]),
        item: int(&op["item"]),
    }
}

fn outcome(o: Outcome) -> Json {
    match o {
        Outcome::Bool(b) => json!(b),
        Outcome::Unit => Json::Null,
    }
}

fn handler_for(w: &World, via: &str) -> Option<(usize, OptionPlugin)> {
    if let Some(name) = via.strip_prefix("option:") {
        return w.player_options.get(name).copied();
    }
    if let Some(name) = via.strip_prefix("scenery:") {
        return w.scenery_options.get(name).copied();
    }
    let mut parts = via.splitn(3, ':');
    parts.next();
    let id: i32 = parts.next().unwrap().parse().unwrap();
    let key = parts.next().unwrap();
    w.scenery_handlers.get(&(id, key.to_owned())).copied()
}

fn run(w: &mut World, op: &Json) -> Result<Json> {
    let p = "p".to_owned();
    let node = op["node"]
        .as_str()
        .filter(|k| w.nodes.contains_key(*k))
        .map(str::to_owned);
    match op["op"].as_str().unwrap() {
        "set" => {
            w.set(op);
            Ok(Json::Null)
        }
        "define" => {
            let class = Class::named(op["class"].as_str().unwrap()).expect("ported class");
            registry::define(w, class)?;
            Ok(Json::Null)
        }
        "button" => {
            let b = button(op);
            let iface = int(&op["iface"]);
            let callback = match w.listeners.get(iface, Some(b.button)).unwrap() {
                Some(c) => Some(c),
                None => w.listeners.get(iface, None).unwrap(),
            };
            let Some(callback) = callback else {
                return Ok(json!("none"));
            };
            let listener = *callback.value::<Listener>().unwrap();
            registry::run_button(w, listener, &p, b).map(|r| json!(r))
        }
        "open" | "close" => {
            let iface = int(&op["iface"]);
            let callback = if op["op"] == "open" {
                w.listeners.get_open(iface).unwrap()
            } else {
                w.listeners.get_close(iface).unwrap()
            };
            let Some(callback) = callback else {
                return Ok(json!("none"));
            };
            let listener = *callback.value::<Listener>().unwrap();
            registry::run_open(w, listener, &p, iface).map(|r| json!(r))
        }
        "plugin" => {
            let Some(&(_, plugin)) = w.components.get(&int(&op["iface"])) else {
                return Ok(json!("none"));
            };
            registry::run_plugin(w, plugin, &p, button(op)).map(|r| json!(r))
        }
        "handler" => {
            let Some((_, handler)) = handler_for(w, op["via"].as_str().unwrap()) else {
                return Ok(json!("none"));
            };
            registry::run_handler(w, handler, &p, node.as_ref(), op["option"].as_str())
                .map(|r| json!(r))
        }
        "props" => {
            let Some((_, h)) = handler_for(w, op["via"].as_str().unwrap()) else {
                return Ok(json!("none"));
            };
            Ok(json!([h.is_walk(), h.is_walk_for(), h.is_delayed()]))
        }
        "destination" => {
            let Some((_, h)) = handler_for(w, op["via"].as_str().unwrap()) else {
                return Ok(json!("none"));
            };
            registry::handler_destination(w, h, node.as_ref()).map(coord)
        }
        "interact" => {
            let n = int(&op["n"]) as usize;
            let Some(&f) = w.interactions.get(n) else {
                return Ok(json!("none"));
            };
            let pick = |k: &str| {
                op[k]
                    .as_str()
                    .filter(|k| w.nodes.contains_key(*k))
                    .map(str::to_owned)
            };
            if f == Interaction::BoothUse {
                let (used, with) = (pick("used"), pick("with"));
                registry::run_interaction(w, f, &p, used.as_ref(), with.as_ref()).map(|r| json!(r))
            } else {
                registry::run_interaction(w, f, &p, node.as_ref(), None).map(|r| json!(r))
            }
        }
        "task" => {
            let n = int(&op["n"]) as usize;
            let Some(Tracked::Task(task)) = w.tracked.get(n).cloned() else {
                return Ok(json!("none"));
            };
            let input = match &op["value"] {
                Json::String(s) => Some(Input::Text(s.clone())),
                Json::Number(i) => Some(Input::Int(i.as_i64().unwrap() as i32)),
                _ => None,
            };
            gh::run_task(w, &task, &p, input.as_ref()).map(outcome)
        }
        "stop" => {
            let n = int(&op["n"]) as usize;
            let Some(Tracked::Task(task)) = w.tracked.get(n).cloned() else {
                return Ok(json!("none"));
            };
            gh::stop_task(w, &task, &n).map(|_| Json::Null)
        }
        "dialogue" => {
            let n = int(&op["n"]) as usize;
            let Some(Tracked::Dialogue(d)) = w.tracked.get(n).cloned() else {
                return Ok(json!("none"));
            };
            gh::climb::climb_dialogue_handle(w, &d, int(&op["button"])).map(|r| json!(r))
        }
        "login" => registry::run_login(w, LoginPlugin::Validation, &p).map(|_| Json::Null),
        "friends" => gh::ifaces::friends_list_open(w, &p).map(|_| Json::Null),
        "rules" => gh::ifaces::rules_open_for(w, &p).map(|_| Json::Null),
        "rules_base" => gh::ifaces::rules_base(w, &p).map(|r| json!(r)),
        "scroll" => {
            let lines: Vec<(String, i32)> = op["lines"]
                .as_array()
                .unwrap()
                .iter()
                .map(|l| (l[0].as_str().unwrap().to_owned(), int(&l[1])))
                .collect();
            gh::ifaces::scroll_setup(w, &p, int(&op["component"]), &lines).map(|_| Json::Null)
        }
        other => panic!("operation {other}"),
    }
}

fn error_name(e: &Error) -> &'static str {
    match e {
        Error::Null => "Null",
        Error::Cast => "Cast",
        Error::Index => "Index",
        Error::Binding(_) => "Binding",
    }
}

fn snapshot(w: &World, result: Json, error: Option<&Error>) -> Json {
    let mut attrs = Map::new();
    for (k, n) in &w.nodes {
        if matches!(n.kind, Kind::Scenery | Kind::Item) || n.attrs.is_empty() {
            continue;
        }
        let m: Map<String, Json> = n
            .attrs
            .iter()
            .map(|(a, v)| (a.clone(), attr_out(v)))
            .collect();
        attrs.insert(k.clone(), Json::Object(m));
    }
    let values = w.values.clone();
    json!({
        "attributes": attrs,
        "error": error.map(error_name),
        "events": w.events,
        "result": result,
        "values": values,
    })
}

fn replay(case: &Json) -> Vec<Json> {
    let mut w = World::new(case);
    let mut out = Vec::new();
    for op in case["ops"].as_array().unwrap() {
        w.events.clear();
        w.op_calls = 0;
        w.fail_at = op.get("failAt").map_or(-1, |v| v.as_i64().unwrap());
        let r = run(&mut w, op);
        out.push(match r {
            Ok(v) => snapshot(&w, v, None),
            Err(e) => snapshot(&w, Json::Null, Some(&e)),
        });
    }
    out
}

#[test]
fn corpus_is_pinned() {
    let provenance: Json = serde_json::from_str(PROVENANCE).unwrap();
    let hash = format!("{:x}", Sha256::digest(CORPUS.as_bytes()));
    assert_eq!(provenance["fixtureSha256"], json!(hash));
    assert_eq!(provenance["caseCount"], json!(CORPUS.lines().count()));
}

#[test]
fn every_case_matches_the_original() {
    let mut failures = Vec::new();
    let mut cases = 0;
    for line in CORPUS.lines() {
        let case: Json = serde_json::from_str(line).unwrap();
        cases += 1;
        let got = replay(&case);
        for (i, (got, want)) in got
            .into_iter()
            .zip(case["expected"].as_array().unwrap())
            .enumerate()
        {
            if &got != want {
                failures.push(format!(
                    "{} | {} | op {i} {}\n  want {}\n  got  {}",
                    case["group"], case["name"], case["ops"][i], want, got
                ));
                break;
            }
        }
    }
    assert!(cases > 1000);
    if !failures.is_empty() {
        panic!(
            "{} of {cases} cases differ:\n{}",
            failures.len(),
            failures
                .iter()
                .take(12)
                .cloned()
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}

/// The start-up manifest order registers exactly what the oracle's
/// scan-order case recorded.
#[test]
fn scanned_registration_matches_the_scan_order_case() {
    let case: Json = CORPUS
        .lines()
        .map(|l| serde_json::from_str::<Json>(l).unwrap())
        .find(|c| c["name"] == "scan order" && c["ops"][0].get("failAt").is_none())
        .unwrap();
    let want: Vec<Json> = case["expected"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|s| s["events"].as_array().unwrap().clone())
        .collect();
    let mut w = World::new(&json!({}));
    let classes = registry::register_scanned(&mut w).unwrap();
    assert_eq!(Json::Array(w.events.clone()), Json::Array(want));
    // LoginValidationPlugin is registered too (as a login plugin).
    assert!(classes.contains(&Class::LoginValidation));
    assert_eq!(classes.len(), Class::ALL.len());
}

/// The native registrar installs the same typed objects into the native
/// registries.
#[test]
fn native_registrar_installs_into_the_native_registries() {
    use rs09_server::definitions::option_handler;
    use rs09_server::world::{interaction_listeners as il, interfaces};
    struct Root;
    impl il::Casing for Root {
        fn lowercase_default(&self, s: &[u16]) -> il::Text {
            rs09_game_core::java_lower::lowercase_root(&String::from_utf16_lossy(s))
                .encode_utf16()
                .collect()
        }
        fn lowercase_root(&self, s: &[u16]) -> il::Text {
            self.lowercase_default(s)
        }
    }
    let listeners = interface_listeners::Registry::default();
    let mut components = interfaces::Registry::default();
    let interactions = il::Registry::default();
    let defaults: option_handler::DefaultHandle = Shared::new(option_handler::Defaults::default());
    let mut native = registry::Native {
        listeners: &listeners,
        components: &mut components,
        interactions: &interactions,
        casing: &Root,
        scenery_defaults: &defaults,
        player_options: Vec::new(),
        scenery_handlers: Vec::new(),
        login_plugins: Vec::new(),
        note_enabled: true,
    };
    let classes = registry::register_scanned(&mut native).unwrap();
    assert_eq!(classes.len(), Class::ALL.len());
    assert_eq!(native.login_plugins, vec![LoginPlugin::Validation]);
    assert_eq!(
        native
            .player_options
            .iter()
            .map(|(n, _)| n.as_str())
            .collect::<Vec<_>>(),
        ["Attack", "Follow"]
    );
    assert_eq!(native.scenery_handlers.len(), 24 + 13);
    // Interface listeners: typed values behind native callbacks.
    let logout = listeners.get(182, None).unwrap().unwrap();
    assert_eq!(*logout.value::<Listener>().unwrap(), Listener::Logout);
    let beast = listeners.get(11, Some(13)).unwrap().unwrap();
    assert_eq!(
        *beast.value::<Listener>().unwrap(),
        Listener::DepositBoxBeast
    );
    assert!(listeners.get_close(667).unwrap().is_some());
    let modern = listeners.get(192, None).unwrap().unwrap();
    assert_eq!(
        *modern.value::<Listener>().unwrap(),
        Listener::Magic(SpellBook::Modern)
    );
    // Component plugins: one object shared by every weapon interface.
    let staff = components.for_id(90).read().plugin.clone().unwrap();
    let fists = components.for_id(92).read().plugin.clone().unwrap();
    assert!(staff.same(&fists));
    assert_eq!(
        *staff.value::<ComponentPlugin>().unwrap(),
        ComponentPlugin::CombatTab
    );
    // Scenery option defaults.
    let search = defaults.read().get(Some("search"));
    assert!(
        search.is_some_and(|h| h.read().value::<OptionPlugin>() == Some(&OptionPlugin::Search))
    );
    // InteractionListeners: the catch-all trade option and the booth metadata.
    let inspected = interactions.inspect();
    let listeners_view = &inspected.iter().find(|(n, _)| *n == "listeners").unwrap().1;
    assert!(
        listeners_view
            .iter()
            .any(|(k, _)| String::from_utf16_lossy(k) == "4:trade with")
    );
    let meta = &inspected
        .iter()
        .find(|(n, _)| *n == "interactions")
        .unwrap()
        .1;
    assert!(
        meta.iter()
            .any(|(k, _)| String::from_utf16_lossy(k) == "1:2213:use-quickly")
    );
}
