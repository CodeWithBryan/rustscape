//! Replays the everyday item handler corpus (fixtures/global-items2) through
//! `world::global_items2`. Every service answers from the case the way the
//! oracle's recording adapters do (oracle/global-items2/adapters.py and
//! Support.java); every recorded call (registrations included), result,
//! error, printed stack trace, the attributes, the tracked tasks and the
//! three generators' raw states after each operation are compared.
use rs09_server::world::drop_tables::WeightBasedTable;
use rs09_server::world::global_items2::{
    self as gi, Anim, Container, Definition, Dialogue, Diary, Error, Expression, Gfx, Host,
    ItemArg, Kind, LogLevel, Object, QueueStrength, Result, Speaker, Statics, Store, Task, Value,
    gloves::{self, BrawlingGloves, ManagerRef},
    random::{JavaGenerators, Random},
    registry::{self, Class, ComponentPlugin, Interaction, InterfaceFn, OptionPlugin, Registrar},
    tables::{self, BirdNest},
};
use rs09_server::world::interaction_listeners::Kind as IntType;
use serde_json::{Map, Value as Json, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};

const CORPUS: &str = include_str!("../../../fixtures/global-items2/oracle.jsonl");
const PROVENANCE: &str = include_str!("../../../fixtures/global-items2/provenance.json");

type N = String;
type C = String;

/// What the oracle tracks by identity: pulses, scripts, close events and dialogue files.
#[derive(Clone)]
enum Tracked {
    Task(Task<N, C>),
    Dialogue(Dialogue<N>),
}

fn int(v: &Json) -> i32 {
    v.as_i64().unwrap() as i32
}

#[derive(Clone)]
struct NodeData {
    kind: Kind,
    id: i32,
    name: Option<String>,
    location: Option<[i32; 3]>,
    amount: i32,
    slot: i32,
    charge: i32,
    attrs: BTreeMap<String, Value<N>>,
}
impl NodeData {
    fn new(kind: Kind) -> Self {
        Self {
            kind,
            id: 0,
            name: None,
            location: None,
            amount: 1,
            slot: -1,
            charge: 1000,
            attrs: BTreeMap::new(),
        }
    }
}

fn location(v: &Json) -> Option<[i32; 3]> {
    v.as_array().map(|a| [int(&a[0]), int(&a[1]), int(&a[2])])
}
fn coord(l: Option<[i32; 3]>) -> Json {
    l.map_or(Json::Null, |l| json!(l))
}
fn anim(a: Anim) -> Json {
    json!([a.id, a.delay, a.priority.name()])
}
fn gfx(g: Gfx) -> Json {
    json!([g.id, g.height, g.delay])
}

static NEXT_OBJECT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(1 << 40);
fn fresh() -> usize {
    NEXT_OBJECT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

const MANAGER: &str =
    "content.global.handlers.item.equipment.brawling_gloves.BrawlingGlovesManager";

struct World {
    nodes: BTreeMap<String, NodeData>,
    definitions: Vec<Definition>,
    examines: HashMap<i32, Option<String>>,
    values: Map<String, Json>,
    answers: Map<String, Json>,
    answer_calls: HashMap<String, usize>,
    generators: JavaGenerators,
    statics: Statics<C>,
    tracked: Vec<Tracked>,
    created: usize,
    // registrations
    shared: HashMap<Interaction, usize>,
    shared_interfaces: HashMap<InterfaceFn, usize>,
    object_labels: HashMap<usize, String>,
    next_label: usize,
    interactions: Vec<Interaction>,
    interfaces: Vec<InterfaceFn>,
    item_handlers: BTreeMap<String, (usize, OptionPlugin)>,
    npc_handlers: BTreeMap<String, (usize, OptionPlugin)>,
    components: BTreeMap<i32, ComponentPlugin>,
    login_listeners: Vec<Class>,
    persist_players: Vec<Class>,
    /// Managers that were given a label stay alive (their identity is their
    /// address).
    kept: Vec<ManagerRef<N>>,
    // recording
    hooks: Vec<Json>,
    events: Vec<Json>,
    calls: HashMap<String, i64>,
    op_calls: i64,
    fail_at: i64,
    stderr: Vec<Json>,
}

impl World {
    fn new(case: &Json) -> Self {
        let seed = |k: &str, d: i64| case["seeds"].get(k).and_then(Json::as_i64).unwrap_or(d);
        let mut w = Self {
            nodes: BTreeMap::new(),
            definitions: case["definitions"]
                .as_array()
                .map(|defs| {
                    defs.iter()
                        .map(|d| Definition {
                            id: int(&d["id"]),
                            name: d["name"].as_str().map(str::to_owned),
                            equipment_slot: d.get("slot").map(int),
                        })
                        .collect()
                })
                .unwrap_or_default(),
            examines: case["definitions"]
                .as_array()
                .map(|defs| {
                    defs.iter()
                        .map(|d| (int(&d["id"]), d["examine"].as_str().map(str::to_owned)))
                        .collect()
                })
                .unwrap_or_default(),
            values: Map::new(),
            answers: Map::new(),
            answer_calls: HashMap::new(),
            generators: JavaGenerators::new(
                seed("random", 530),
                seed("shuffle", 531),
                seed("tlr", 532),
            ),
            statics: Statics::default(),
            tracked: Vec::new(),
            created: 0,
            shared: HashMap::new(),
            shared_interfaces: HashMap::new(),
            object_labels: HashMap::new(),
            next_label: 0,
            interactions: Vec::new(),
            interfaces: Vec::new(),
            item_handlers: BTreeMap::new(),
            npc_handlers: BTreeMap::new(),
            components: BTreeMap::new(),
            login_listeners: Vec::new(),
            persist_players: Vec::new(),
            kept: Vec::new(),
            hooks: case["hooks"].as_array().cloned().unwrap_or_default(),
            events: Vec::new(),
            calls: HashMap::new(),
            op_calls: 0,
            fail_at: -1,
            stderr: Vec::new(),
        };
        w.set(&json!({"nodes": {"p": {"kind": "player", "name": "tracer", "location": [3222, 3218, 0]}}}));
        w.set(case);
        w
    }

    fn set(&mut self, v: &Json) {
        if let Some(ns) = v.get("nodes").and_then(Json::as_object) {
            for (k, spec) in ns {
                let node = self.nodes.entry(k.clone()).or_insert_with(|| {
                    NodeData::new(match spec["kind"].as_str().unwrap() {
                        "player" => Kind::Player,
                        "npc" => Kind::Npc,
                        "scenery" => Kind::Scenery,
                        "item" => Kind::Item,
                        other => panic!("{other}"),
                    })
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
                if let Some(s) = spec.get("slot") {
                    node.slot = int(s);
                }
                if let Some(c) = spec.get("charge") {
                    node.charge = int(c);
                }
            }
        }
        if let Some(attrs) = v.get("attributes").and_then(Json::as_object) {
            for (k, map) in attrs {
                for (a, value) in map.as_object().unwrap() {
                    let node = self.nodes.get_mut(k).unwrap();
                    match value {
                        Json::Null => {
                            node.attrs.remove(a);
                        }
                        Json::Bool(b) => {
                            node.attrs.insert(a.clone(), Value::Bool(*b));
                        }
                        Json::String(s) => {
                            node.attrs.insert(a.clone(), Value::Text(s.clone()));
                        }
                        Json::Number(n) => {
                            node.attrs
                                .insert(a.clone(), Value::Int(n.as_i64().unwrap() as i32));
                        }
                        other => panic!("attribute {other}"),
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
    /// Support.node: a node key, or null.
    fn answer_node(&mut self, name: &str, fallback: Json) -> Option<N> {
        self.answer(name, fallback)
            .as_str()
            .filter(|k| self.nodes.contains_key(*k))
            .map(str::to_owned)
    }
    /// Support.items: a list of keys, or a list of such lists one per call.
    fn answer_items(&mut self, name: &str) -> Vec<Option<N>> {
        let mut list = self.answers.get(name).cloned().unwrap_or(json!([]));
        if list.as_array().unwrap().first().is_some_and(Json::is_array) {
            let i = self.answer_calls.entry(name.to_owned()).or_insert(0);
            *i += 1;
            let all = list.as_array().unwrap().clone();
            list = all[(*i - 1).min(all.len() - 1)].clone();
        }
        list.as_array()
            .unwrap()
            .iter()
            .map(|k| {
                k.as_str()
                    .filter(|k| self.nodes.contains_key(*k))
                    .map(str::to_owned)
            })
            .collect()
    }
    fn data(&self, n: &N) -> &NodeData {
        &self.nodes[n]
    }
    fn track(&mut self, t: Tracked) -> usize {
        self.tracked.push(t);
        self.tracked.len() - 1
    }
    fn label(&mut self, identity: usize, class: &str) -> String {
        if let Some(l) = self.object_labels.get(&identity) {
            return l.clone();
        }
        self.next_label += 1;
        let l = format!("{class}#{}", self.next_label);
        self.object_labels.insert(identity, l.clone());
        l
    }
    fn attr_out(&mut self, v: &Value<N>) -> Json {
        match v {
            Value::Null => Json::Null,
            Value::Int(i) => json!(i),
            Value::Long(l) => json!({ "long": l }),
            Value::Double(d) => json!({ "double": d }),
            Value::Bool(b) => json!(b),
            Value::Text(t) => json!(t),
            Value::Location(l) => json!({ "location": l }),
            Value::Node(n) => json!(n),
            Value::Items(items) => json!(items),
            Value::Object(Object::Handler { plugin, identity }) => {
                json!(self.label(*identity, plugin.class()))
            }
            Value::Object(Object::GloveManager(m)) => {
                if !self.kept.iter().any(|k| k == m) {
                    self.kept.push(m.clone());
                }
                json!(self.label(m.identity(), MANAGER))
            }
        }
    }
    fn definitions_walk(&self) -> Vec<Definition> {
        let ids: Vec<i32> = self.definitions.iter().map(|d| d.id).collect();
        tables::java_hash_order(&ids)
            .into_iter()
            .map(|id| {
                self.definitions
                    .iter()
                    .find(|d| d.id == id)
                    .unwrap()
                    .clone()
            })
            .collect()
    }
    fn item_name_of(&mut self, id: i32) -> Option<String> {
        match self.definitions.iter().find(|d| d.id == id) {
            Some(d) => d.name.clone(),
            None => self
                .answer(&format!("itemName:{id}"), Json::Null)
                .as_str()
                .map(str::to_owned),
        }
    }
}

fn int_kind(kind: IntType) -> &'static str {
    match kind {
        IntType::Item => "ITEM",
        IntType::Scenery => "SCENERY",
        IntType::Npc => "NPC",
        IntType::GroundItem => "GROUNDITEM",
        IntType::Player => "PLAYER",
    }
}

// ---------------------------------------------------------------------- the registrar

impl Registrar for World {
    type FunctionRef = (usize, Interaction);
    type InterfaceRef = (usize, InterfaceFn);
    type HandlerRef = (usize, OptionPlugin);
    type ObjectRef = (usize, Class);
    type ComponentRef = (usize, ComponentPlugin);
    fn function(&mut self, f: Interaction) -> Self::FunctionRef {
        if f.shared() {
            return (*self.shared.entry(f).or_insert_with(fresh), f);
        }
        (fresh(), f)
    }
    fn interface_function(&mut self, f: InterfaceFn) -> Self::InterfaceRef {
        if f.shared() {
            return (*self.shared_interfaces.entry(f).or_insert_with(fresh), f);
        }
        (fresh(), f)
    }
    fn handler_object(&mut self, handler: OptionPlugin) -> Self::HandlerRef {
        (fresh(), handler)
    }
    fn content_object(&mut self, class: Class) -> Self::ObjectRef {
        (fresh(), class)
    }
    fn component_object(&mut self, plugin: ComponentPlugin) -> Self::ComponentRef {
        (fresh(), plugin)
    }
    fn put_npc_handler(&mut self, id: i32, key: &str, handler: &Self::HandlerRef) -> Result<()> {
        let label = self.label(handler.0, handler.1.class());
        self.emit(json!(["npcHandler", id, key, label]))?;
        self.npc_handlers.insert(format!("{id}:{key}"), *handler);
        Ok(())
    }
    fn component_plugin(&mut self, id: i32, plugin: &Self::ComponentRef) -> Result<()> {
        let label = self.label(plugin.0, plugin.1.class());
        self.emit(json!(["componentPlugin", id, label]))?;
        self.components.insert(id, plugin.1);
        Ok(())
    }
    fn define_plugin(&mut self, plugin: &Self::ComponentRef) -> Result<()> {
        let label = self.label(plugin.0, plugin.1.class());
        self.emit(json!(["definePlugin", label]))
    }
    fn diango_items(&mut self) -> &mut Vec<i32> {
        &mut self.statics.diango_items
    }
    fn on(
        &mut self,
        ids: &[i32],
        kind: IntType,
        options: &[&str],
        f: &Self::FunctionRef,
    ) -> Result<()> {
        let label = self.label(f.0, &f.1.class());
        self.emit(json!(["on", ids, int_kind(kind), options, label]))?;
        self.interactions.push(f.1);
        Ok(())
    }
    fn on_use_with(
        &mut self,
        kind: IntType,
        used: &[i32],
        _array: bool,
        with: &[i32],
        f: &Self::FunctionRef,
    ) -> Result<()> {
        let label = self.label(f.0, &f.1.class());
        self.emit(json!(["onUseWith", int_kind(kind), used, with, label]))?;
        self.interactions.push(f.1);
        Ok(())
    }
    fn on_equip(&mut self, id: i32, f: &Self::FunctionRef) -> Result<()> {
        let label = self.label(f.0, &f.1.class());
        self.emit(json!(["onEquip", id, label]))?;
        self.interactions.push(f.1);
        Ok(())
    }
    fn on_interface(&mut self, id: i32, f: &Self::InterfaceRef) -> Result<()> {
        let label = self.label(f.0, &f.1.class());
        self.emit(json!(["onInterface", id, label]))?;
        self.interfaces.push(f.1);
        Ok(())
    }
    fn on_interface_close(&mut self, id: i32, f: &Self::InterfaceRef) -> Result<()> {
        let label = self.label(f.0, &f.1.class());
        self.emit(json!(["onInterfaceClose", id, label]))?;
        self.interfaces.push(f.1);
        Ok(())
    }
    fn put_item_handler(&mut self, id: i32, key: &str, handler: &Self::HandlerRef) -> Result<()> {
        let label = self.label(handler.0, handler.1.class());
        self.emit(json!(["itemHandler", id, key, label]))?;
        self.item_handlers.insert(format!("{id}:{key}"), *handler);
        Ok(())
    }
    fn login_listener(&mut self, object: &Self::ObjectRef) -> Result<()> {
        let label = self.label(object.0, object.1.name());
        self.emit(json!(["loginListener", label]))?;
        self.login_listeners.push(object.1);
        Ok(())
    }
    fn persist_player(&mut self, object: &Self::ObjectRef) -> Result<()> {
        let label = self.label(object.0, object.1.name());
        self.emit(json!(["persistPlayer", label]))?;
        self.persist_players.push(object.1);
        Ok(())
    }
    fn definitions(&mut self) -> Vec<Definition> {
        self.definitions_walk()
    }
    fn lowercase_default(&self, text: &str) -> String {
        rs09_game_core::java_lower::lowercase_root(text)
    }
    fn random(&mut self) -> &mut dyn Random {
        &mut self.generators
    }
    fn casket_table(&mut self) -> &mut Option<WeightBasedTable> {
        &mut self.statics.casket
    }
}

// ---------------------------------------------------------------------- the services

fn item_json(item: &ItemArg<N>) -> Json {
    match item {
        ItemArg::Item(n) => json!(n),
        ItemArg::Id(i) => json!(i),
    }
}

impl Host for World {
    type Node = N;
    type Callback = C;

    fn kind(&self, node: &N) -> Kind {
        self.data(node).kind
    }
    fn id(&mut self, node: &N) -> Result<i32> {
        Ok(self.data(node).id)
    }
    fn name(&mut self, node: &N) -> Result<Option<String>> {
        Ok(self.data(node).name.clone())
    }
    fn amount(&mut self, item: &N) -> Result<i32> {
        Ok(self.data(item).amount)
    }
    fn slot(&mut self, item: &N) -> Result<i32> {
        Ok(self.data(item).slot)
    }
    fn charge(&mut self, item: &N) -> Result<i32> {
        Ok(self.data(item).charge)
    }
    fn set_charge(&mut self, item: &N, charge: i32) -> Result<()> {
        self.emit(json!(["setCharge", item, charge]))?;
        self.nodes.get_mut(item).unwrap().charge = charge;
        Ok(())
    }
    fn location(&mut self, node: &N) -> Result<Option<[i32; 3]>> {
        Ok(self.data(node).location)
    }
    fn lowercase_default(&self, text: &str) -> String {
        rs09_game_core::java_lower::lowercase_root(text)
    }
    fn lowercase_root(&self, text: &str) -> String {
        rs09_game_core::java_lower::lowercase_root(text)
    }
    fn item_name(&mut self, id: i32) -> Option<String> {
        self.item_name_of(id)
    }
    fn definitions(&mut self) -> Vec<Definition> {
        self.definitions_walk()
    }
    fn new_item(&mut self, id: i32, amount: i32) -> Result<N> {
        self.emit(json!(["newItem", id, amount]))?;
        self.created += 1;
        let key = format!("new:{id}:{}", self.created);
        let mut data = NodeData::new(Kind::Item);
        data.id = id;
        data.name = self.item_name_of(id);
        data.amount = amount;
        self.nodes.insert(key.clone(), data);
        Ok(key)
    }
    fn constant_item(&mut self, id: i32, amount: i32) -> N {
        let key = format!("const:{id}");
        if !self.nodes.contains_key(&key) {
            let mut data = NodeData::new(Kind::Item);
            data.id = id;
            data.name = self.item_name_of(id);
            data.amount = amount;
            self.nodes.insert(key.clone(), data);
        }
        key
    }
    fn random(&mut self) -> &mut dyn Random {
        &mut self.generators
    }
    fn statics(&mut self) -> &mut Statics<C> {
        &mut self.statics
    }
    fn get_attribute(&mut self, entity: &N, key: &str) -> Result<Option<Value<N>>> {
        self.emit(json!(["getAttribute", key]))?;
        Ok(self
            .data(entity)
            .attrs
            .get(&key.replace("/save:", ""))
            .cloned()
            .filter(|v| *v != Value::Null))
    }
    fn set_attribute(&mut self, entity: &N, key: &str, value: Value<N>) -> Result<()> {
        let out = self.attr_out(&value);
        self.emit(json!(["setAttribute", key, out]))?;
        let key = key.strip_prefix("/save:").unwrap_or(key).to_owned();
        self.nodes.get_mut(entity).unwrap().attrs.insert(key, value);
        Ok(())
    }
    fn remove_attribute(&mut self, entity: &N, key: &str) -> Result<()> {
        self.emit(json!(["removeAttribute", key]))?;
        self.nodes
            .get_mut(entity)
            .unwrap()
            .attrs
            .remove(&key.replace("/save:", ""));
        Ok(())
    }
    fn send_message(&mut self, _: &N, text: &str) -> Result<()> {
        self.emit(json!(["message", text]))
    }
    fn debug(&mut self, _: &N, text: &str) -> Result<()> {
        self.emit(json!(["debug", text]))
    }
    fn item_message(&mut self, _: &N, item: &N, lines: &[&str]) -> Result<()> {
        self.emit(json!(["itemMessage", item, lines]))
    }
    fn item_dialogue(&mut self, _: &N, item: ItemArg<N>, text: &str) -> Result<()> {
        self.emit(json!(["itemDialogue", item_json(&item), text]))
    }
    fn player_dialogue(
        &mut self,
        _: &N,
        text: &str,
        expression: Expression,
        hide: bool,
    ) -> Result<()> {
        self.emit(json!(["playerDialogue", text, expression.name(), hide]))
    }
    fn dialogue_lines(&mut self, _: &N, lines: &[&str]) -> Result<()> {
        self.emit(json!(["dialogueLines", lines]))
    }
    fn send_dialogue(&mut self, _: &N, text: &str) -> Result<()> {
        self.emit(json!(["sendDialogue", text]))
    }
    fn open_interface(&mut self, _: &N, id: i32) -> Result<()> {
        self.emit(json!(["openInterface", id]))
    }
    fn interface_text(&mut self, _: &N, text: &str, iface: i32, child: i32) -> Result<()> {
        self.emit(json!(["interfaceText", text, iface, child]))
    }
    fn close_all_interfaces(&mut self, _: &N) -> Result<()> {
        self.emit(json!(["closeAllInterfaces"]))
    }
    fn open_component(&mut self, _: &N, id: i32, close: Option<Task<N, C>>) -> Result<()> {
        let n = close.map_or(-1, |t| self.track(Tracked::Task(t)) as i64);
        self.emit(json!(["openComponent", id, n]))
    }
    fn open_default_tabs(&mut self, _: &N) -> Result<()> {
        self.emit(json!(["openDefaultTabs"]))
    }
    fn remove_tabs(&mut self, _: &N, tabs: &[i32]) -> Result<()> {
        self.emit(json!(["removeTabs", tabs]))
    }
    fn has_requirement(&mut self, _: &N, quest: &str, message: bool) -> Result<bool> {
        self.emit(json!(["hasRequirement", quest, message]))?;
        Ok(self.answer_bool(&format!("hasRequirement:{quest}"), true))
    }
    fn quests(&mut self) -> Result<Vec<i32>> {
        self.emit(json!(["getQuests"]))?;
        let list = self
            .answers
            .get("questPoints")
            .cloned()
            .unwrap_or(json!([1, 2, 3]));
        Ok(list.as_array().unwrap().iter().map(int).collect())
    }
    fn quest_points(&mut self, _: &N) -> Result<i32> {
        self.emit(json!(["questPoints"]))?;
        Ok(self.answer_int("points", 6))
    }
    fn lock(&mut self, _: &N, ticks: i32) -> Result<()> {
        self.emit(json!(["lock", ticks]))
    }
    fn unlock(&mut self, _: &N) -> Result<()> {
        self.emit(json!(["unlock"]))
    }
    fn lock_interactions(&mut self, _: &N, ticks: i32) -> Result<()> {
        self.emit(json!(["lockInteractions", ticks]))
    }
    fn stop_walk(&mut self, _: &N) -> Result<()> {
        self.emit(json!(["stopWalk"]))
    }
    fn pulse_run(&mut self, _: &N, pulse: Task<N, C>, delay: i32) -> Result<()> {
        let n = self.track(Tracked::Task(pulse));
        self.emit(json!(["pulseRun", n, delay]))
    }
    fn submit_world(&mut self, pulse: Task<N, C>, delay: i32) -> Result<()> {
        let n = self.track(Tracked::Task(pulse));
        self.emit(json!(["submitWorld", n, delay]))
    }
    fn animate(&mut self, _: &N, animation: Anim) -> Result<bool> {
        self.emit(json!(["animate", anim(animation)]))?;
        Ok(true)
    }
    fn visualize(&mut self, _: &N, animation: Anim, graphics: Gfx) -> Result<bool> {
        self.emit(json!(["visualize", anim(animation), gfx(graphics)]))?;
        Ok(true)
    }
    fn chat(&mut self, _: &N, text: &str) -> Result<()> {
        self.emit(json!(["chat", text]))
    }
    fn transform_npc(&mut self, _: &N, id: i32) -> Result<()> {
        self.emit(json!(["transformNpc", id]))
    }
    fn play_audio(
        &mut self,
        _: &N,
        id: i32,
        delay: i32,
        loops: i32,
        location: Option<[i32; 3]>,
        radius: i32,
    ) -> Result<()> {
        self.emit(json!(["audio", id, delay, loops, coord(location), radius]))
    }
    fn play_jingle(&mut self, _: &N, id: i32) -> Result<()> {
        self.emit(json!(["jingle", id]))
    }
    fn container_add(&mut self, _: &N, c: Container, item: &N) -> Result<bool> {
        let name = format!("{}.add", c.name());
        self.emit(json!([name, item]))?;
        Ok(self.answer_bool(&name, true))
    }
    fn container_add_all(&mut self, _: &N, c: Container, items: &[Option<N>]) -> Result<bool> {
        let name = format!("{}.addAll", c.name());
        self.emit(json!([name, items]))?;
        Ok(self.answer_bool(&name, true))
    }
    fn container_remove(&mut self, _: &N, c: Container, item: Option<&N>) -> Result<bool> {
        let name = format!("{}.remove", c.name());
        self.emit(json!([name, item]))?;
        Ok(self.answer_bool(&name, true))
    }
    fn container_remove_slot(
        &mut self,
        _: &N,
        c: Container,
        item: &N,
        slot: i32,
        fire: bool,
    ) -> Result<bool> {
        let name = format!("{}.removeSlot", c.name());
        self.emit(json!([name, item, slot, fire]))?;
        Ok(self.answer_bool(&name, true))
    }
    fn container_free_slots(&mut self, _: &N, c: Container) -> Result<i32> {
        let name = format!("{}.freeSlots", c.name());
        self.emit(json!([name]))?;
        Ok(self.answer_int(&name, 28))
    }
    fn container_replace(&mut self, _: &N, c: Container, item: &N, slot: i32) -> Result<Option<N>> {
        let name = format!("{}.replace", c.name());
        self.emit(json!([name, item, slot]))?;
        Ok(self.answer_node(&name, Json::Null))
    }
    fn container_get(&mut self, _: &N, c: Container, slot: i32) -> Result<Option<N>> {
        let name = format!("{}.get", c.name());
        self.emit(json!([name, slot]))?;
        Ok(self.answer_node(&name, Json::Null))
    }
    fn container_to_array(&mut self, _: &N, c: Container) -> Result<Vec<Option<N>>> {
        let name = format!("{}.toArray", c.name());
        self.emit(json!([name]))?;
        Ok(self.answer_items(&name))
    }
    fn remove_item(&mut self, _: &N, item: ItemArg<N>, store: Store) -> Result<bool> {
        self.emit(json!(["removeItem", item_json(&item), store.name()]))?;
        Ok(self.answer_bool("removeItem", true))
    }
    fn replace_slot(
        &mut self,
        _: &N,
        slot: i32,
        item: &N,
        current: Option<&N>,
        store: Store,
    ) -> Result<Option<N>> {
        self.emit(json!(["replaceSlot", slot, item, current, store.name()]))?;
        Ok(self.answer_node("replaceSlot", json!(current)))
    }
    fn add_item(&mut self, _: &N, id: i32, amount: i32, store: Store) -> Result<bool> {
        self.emit(json!(["addItem", id, amount, store.name()]))?;
        Ok(self.answer_bool("addItem", true))
    }
    fn add_item_or_drop(&mut self, _: &N, id: i32, amount: i32) -> Result<()> {
        self.emit(json!(["addItemOrDrop", id, amount]))
    }
    fn amount_in_inventory(&mut self, _: &N, id: i32) -> Result<i32> {
        self.emit(json!(["amountInInventory", id]))?;
        Ok(self.answer_int("amountInInventory", 0))
    }
    fn produce_ground_item(&mut self, _: &N, id: i32) -> Result<()> {
        self.emit(json!(["produceGroundItem", id]))
    }
    fn ground_item(&mut self, _: &N, item: &N) -> Result<()> {
        self.emit(json!(["groundItem", item]))
    }
    fn replace_scenery(
        &mut self,
        scenery: &N,
        with: i32,
        ticks: i32,
        location: Option<[i32; 3]>,
    ) -> Result<()> {
        self.emit(json!([
            "replaceScenery",
            scenery,
            with,
            ticks,
            coord(location)
        ]))
    }
    fn get_diary(&mut self, _: &N, diary: Diary) -> Result<()> {
        self.emit(json!(["getDiary", diary.name()]))
    }
    fn diary_complete(&mut self, _: &N, diary: Diary, level: i32, task: i32) -> Result<bool> {
        self.emit(json!(["diaryComplete", diary.name(), level, task]))?;
        Ok(self.answer_bool("diaryComplete", false))
    }
    fn diary_update(
        &mut self,
        _: &N,
        diary: Diary,
        level: i32,
        task: i32,
        complete: bool,
    ) -> Result<()> {
        self.emit(json!(["diaryUpdate", diary.name(), level, task, complete]))
    }
    fn static_level(&mut self, _: &N, skill: i32) -> Result<i32> {
        self.emit(json!(["staticLevel", skill]))?;
        Ok(self.answer_int("staticLevel", 1))
    }
    fn add_experience(&mut self, _: &N, skill: i32, experience: f64) -> Result<()> {
        self.emit(json!(["addExperience", skill, experience]))
    }
    fn wilderness_level(&mut self, _: &N) -> Result<i32> {
        self.emit(json!(["wildernessLevel"]))?;
        Ok(self.answer_int("wildernessLevel", 0))
    }
    fn log(&mut self, class: &str, level: LogLevel, message: &str) -> Result<()> {
        self.emit(json!(["log", class, level.name(), message]))
    }
    fn stack_trace(&mut self, exception: &str) {
        self.stderr.push(json!(exception));
    }
    fn run_spade_listener(&mut self, _: &N, callback: &C) -> Result<()> {
        self.emit(json!(["spadeListener", callback]))
    }
    fn run_dig_action(&mut self, _: &N, action: &C) -> Result<()> {
        self.emit(json!(["digAction", action]))
    }
    fn username(&mut self, _: &N) -> Result<String> {
        self.emit(json!(["username"]))?;
        Ok(self
            .values
            .get("username")
            .and_then(Json::as_str)
            .unwrap_or("Tracer")
            .to_owned())
    }
    fn open_dialogue(&mut self, _: &N, dialogue: Dialogue<N>) -> Result<()> {
        let n = self.track(Tracked::Dialogue(dialogue));
        self.emit(json!(["openDialogue", n, []]))
    }
    fn send_dialogues(
        &mut self,
        _: &N,
        speaker: Speaker<N>,
        expression: Option<Expression>,
        lines: &[String],
    ) -> Result<()> {
        let speaker = match speaker {
            Speaker::Node(n) => json!(n),
            Speaker::Id(i) => json!(i),
        };
        self.emit(json!([
            "sendDialogues",
            speaker,
            expression.map(Expression::name),
            lines
        ]))
    }
    fn add_topic(&mut self, _: &N, _: Expression, text: &str, _: i32) -> Result<()> {
        self.emit(json!(["activeTopic", text]))
    }
    fn clear_topics(&mut self, _: &N) -> Result<()> {
        self.emit(json!(["clearTopics"]))
    }
    fn send_options(&mut self, _: &N, title: &str, options: &[&str]) -> Result<()> {
        self.emit(json!(["sendOptions", title, options]))
    }
    fn interpreter_close(&mut self, _: &N) -> Result<bool> {
        self.emit(json!(["interpreterClose"]))?;
        Ok(true)
    }
    fn slayer_master(&mut self, _: &N) -> Result<Option<N>> {
        self.emit(json!(["slayerMaster"]))?;
        Ok(self.answer_node("slayerMaster", json!("master")))
    }
    fn slayer_tip(&mut self, _: &N) -> Result<Vec<String>> {
        self.emit(json!(["slayerTip"]))?;
        let tips = self
            .answers
            .get("slayerTip")
            .cloned()
            .unwrap_or(json!(["Tip one.", "Tip two."]));
        Ok(tips
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t.as_str().unwrap().to_owned())
            .collect())
    }
    fn has_slayer_task(&mut self, _: &N) -> Result<bool> {
        self.emit(json!(["hasSlayerTask"]))?;
        Ok(self.answer_bool("hasSlayerTask", true))
    }
    fn slayer_task_name(&mut self, _: &N) -> Result<String> {
        self.emit(json!(["slayerTaskName"]))?;
        Ok(self
            .answer("slayerTaskName", json!("goblin"))
            .as_str()
            .unwrap()
            .to_owned())
    }
    fn slayer_kills_remaining(&mut self, _: &N) -> Result<i32> {
        self.emit(json!(["slayerKillsRemaining"]))?;
        Ok(self.answer_int("slayerKillsRemaining", 12))
    }
    fn slayer_task_flags(&mut self, _: &N) -> Result<i32> {
        self.emit(json!(["slayerTaskFlags"]))?;
        Ok(self.answer_int("slayerTaskFlags", 0))
    }
    fn set_varp(&mut self, _: &N, index: i32, value: i32, save: bool) -> Result<()> {
        self.emit(json!(["setVarp", index, value, save]))
    }
    fn queue_script(
        &mut self,
        _: &N,
        delay: i32,
        strength: QueueStrength,
        persist: bool,
        script: Task<N, C>,
    ) -> Result<()> {
        let n = self.track(Tracked::Task(script));
        self.emit(json!(["queueScript", delay, strength.name(), persist, n]))
    }
    fn stop_executing(&mut self, _: &N) -> Result<bool> {
        self.emit(json!(["stopExecuting"]))?;
        Ok(self.answer_bool("stopExecuting", true))
    }
    fn animation_duration(&mut self, id: i32) -> Result<i32> {
        self.emit(json!(["animationDuration", id]))?;
        Ok(self.answer_int(&format!("duration:{id}"), 1))
    }
    fn projectile_location(&mut self, entity: &N) -> Result<Option<[i32; 3]>> {
        self.emit(json!(["projectileLocation", entity]))?;
        Ok(location(
            &self.answer("projectileLocation", json!([3222, 3218, 0])),
        ))
    }
    fn random_location(
        &mut self,
        main: [i32; 3],
        radius: i32,
        reachable: bool,
    ) -> Result<Option<[i32; 3]>> {
        self.emit(json!(["randomLocation", main, radius, reachable]))?;
        Ok(location(
            &self.answer("randomLocation", json!([3223, 3219, 0])),
        ))
    }
    fn face_location(&mut self, _: &N, at: [i32; 3]) -> Result<()> {
        self.emit(json!(["faceLocation", at]))
    }
    fn spawn_projectile(
        &mut self,
        source: [i32; 3],
        dest: [i32; 3],
        projectile: i32,
        start: i32,
        end: i32,
        delay: i32,
        speed: i32,
        angle: i32,
    ) -> Result<()> {
        self.emit(json!([
            "projectile",
            source,
            dest,
            projectile,
            start,
            end,
            delay,
            speed,
            angle
        ]))
    }
    fn opened_component(&mut self, _: &N) -> Result<Option<i32>> {
        self.emit(json!(["getOpened"]))?;
        Ok(self.answer("opened", Json::Null).as_i64().map(|i| i as i32))
    }
    fn component_close(&mut self, _: &N, id: i32) -> Result<()> {
        self.emit(json!(["componentClose", id]))
    }
    fn close_interface(&mut self, _: &N) -> Result<bool> {
        self.emit(json!(["closeInterface"]))?;
        Ok(true)
    }
    fn container_contains(&mut self, _: &N, c: Container, item: &N) -> Result<bool> {
        let name = format!("{}.containsItem", c.name());
        self.emit(json!([name, item]))?;
        Ok(self.answer_bool(&name, false))
    }
    fn storage_contains(&mut self, _: &N, family: &str, id: i32) -> Result<bool> {
        self.emit(json!(["storageContains", family, id]))?;
        Ok(self.answer_bool(&format!("storageContains:{id}"), false))
    }
    fn in_inventory(&mut self, _: &N, id: i32, amount: i32) -> Result<bool> {
        self.emit(json!(["inInventory", id, amount]))?;
        Ok(self.answer_bool("inInventory", false))
    }
    fn in_equipment(&mut self, _: &N, id: i32, amount: i32) -> Result<bool> {
        self.emit(json!(["inEquipment", id, amount]))?;
        Ok(self.answer_bool("inEquipment", false))
    }
    fn generate_items(
        &mut self,
        _: &N,
        items: &[Option<N>],
        options: &[&str],
        iface: i32,
        child: i32,
        x: i32,
        y: i32,
    ) -> Result<i32> {
        self.emit(json!(["generateItems", items, options, iface, child, x, y]))?;
        Ok(self.answer_int("generateItems", 0))
    }
    fn item_examine(&mut self, id: i32) -> Option<String> {
        self.examines.get(&id).cloned().flatten()
    }
}

// ---------------------------------------------------------------------- operations

fn node_of(w: &World, v: &Json) -> Option<N> {
    v.as_str()
        .filter(|k| w.nodes.contains_key(*k))
        .map(str::to_owned)
}
fn at(op: &Json) -> [i32; 3] {
    [int(&op["x"]), int(&op["y"]), int(&op["z"])]
}

fn call(w: &mut World, op: &Json, node: Option<N>) -> Result<Json> {
    let p = "p".to_owned();
    match op["name"].as_str().unwrap() {
        "spadeRegister" => {
            gi::dig::register_listener(w, at(op), op["tag"].as_str().unwrap().to_owned());
            Ok(Json::Null)
        }
        "spadeRun" => Ok(json!(gi::dig::run_listener(w, at(op), &p)?)),
        "digRegister" => Ok(json!(gi::dig::register(
            w,
            at(op),
            op["tag"].as_str().unwrap().to_owned()
        )?)),
        "dig" => Ok(json!(gi::dig::dig(w, &p)?)),
        "nestDrop" => {
            gi::everyday::drop_nest(w, &p)?;
            Ok(Json::Null)
        }
        "nestRandom" => {
            let wyson = op["wyson"] == json!(true);
            Ok(gi::everyday::random_nest(Host::random(w), wyson)
                .map_or(Json::Null, |n| json!(n.name())))
        }
        "nestFor" => {
            let node = node.ok_or(Error::Null)?;
            if w.kind(&node) != Kind::Item {
                return Err(Error::Cast);
            }
            Ok(BirdNest::for_nest(w.id(&node)?).map_or(Json::Null, |n| json!(n.name())))
        }
        "nestSearch" => {
            let nest = BirdNest::ALL
                .into_iter()
                .find(|n| n.name() == op["nest"])
                .unwrap();
            gi::everyday::search(w, nest, &p, &node.ok_or(Error::Null)?)?;
            Ok(Json::Null)
        }
        "fogUpdate" => Ok(json!(gloves::fog_update(w, &p, int(&op["charges"]))?)),
        "bgRegister" => {
            let m = gloves::instance(w, &p)?;
            match op.get("charges") {
                Some(c) => gloves::register_with(&m, int(&op["id"]), int(c)),
                None => gloves::register(w, &m, int(&op["id"])),
            }
            Ok(Json::Null)
        }
        "bgUpdate" => {
            let m = gloves::instance(w, &p)?;
            gloves::update_charges(w, &m, int(&op["glove"]), int(&op["charges"]))?;
            Ok(Json::Null)
        }
        "bgBonus" => {
            let m = gloves::instance(w, &p)?;
            Ok(json!({ "double": gloves::experience_bonus(w, &m)? }))
        }
        "bgCharges" => {
            let m = gloves::instance(w, &p)?;
            let mut entries = m.snapshot().glove_charges.walk();
            entries.sort();
            let text: Vec<String> = entries.iter().map(|(k, v)| format!("{k}={v}")).collect();
            Ok(json!(format!("{{{}}}", text.join(", "))))
        }
        "bgForId" => {
            Ok(BrawlingGloves::for_id(int(&op["value"])).map_or(Json::Null, |g| json!(g.name())))
        }
        "bgForIndicator" => Ok(BrawlingGloves::for_indicator(int(&op["value"]))
            .map_or(Json::Null, |g| json!(g.name()))),
        "bgForSkill" => Ok(
            BrawlingGloves::for_skill(int(&op["value"])).map_or(Json::Null, |g| json!(g.name()))
        ),
        "bgName" => {
            let glove = BrawlingGloves::ALL
                .into_iter()
                .find(|g| g.name() == op["glove"])
                .unwrap();
            Ok(json!(w.item_name(glove.id())))
        }
        "tlrInt" => Ok(json!(w.generators.thread_int(int(&op["bound"])))),
        "tlrDouble" => {
            let max = op["max"].as_f64().unwrap();
            // The oracle records the raw bits: decimal text need not parse back exactly here.
            Ok(
                json!({ "long": w.generators.thread_double(0.0, max).map_err(|_| Error::Argument)?.to_bits() as i64 }),
            )
        }
        "random" => Ok(json!(gi::random::random(
            &mut w.generators,
            int(&op["bound"])
        ))),
        "randomRange" => Ok(json!(gi::random::random_range(
            &mut w.generators,
            int(&op["a"]),
            int(&op["b"])
        ))),
        other => panic!("call {other}"),
    }
}

fn saved(v: &Json) -> Json {
    v.clone()
}

fn run(w: &mut World, op: &Json) -> Result<Json> {
    let p = "p".to_owned();
    let node = node_of(w, &op["node"]);
    match op["op"].as_str().unwrap() {
        "set" => {
            w.set(op);
            Ok(Json::Null)
        }
        "define" => {
            let class = Class::named(op["class"].as_str().unwrap()).unwrap();
            registry::define(w, class)?;
            Ok(Json::Null)
        }
        "interact" => {
            let n = op["n"].as_u64().unwrap() as usize;
            let Some(f) = w.interactions.get(n).copied() else {
                return Ok(json!("none"));
            };
            let r = match f.kind() {
                rs09_server::world::interaction_listeners::FunctionKind::UseWith => {
                    let used = node_of(w, &op["used"]).ok_or(Error::Null)?;
                    let with = node_of(w, &op["with"]);
                    registry::run_interaction(w, f, &p, &used, with.as_ref())?
                }
                _ => registry::run_interaction(w, f, &p, &node.ok_or(Error::Null)?, None)?,
            };
            Ok(json!(r))
        }
        "button" => {
            let n = op["n"].as_u64().unwrap() as usize;
            let Some(f) = w.interfaces.get(n).copied() else {
                return Ok(json!("none"));
            };
            let button = op.get("button").map(int);
            Ok(json!(registry::run_interface(w, f, &p, button)?))
        }
        "handler" => {
            let Some((identity, h)) = w.item_handlers.get(op["via"].as_str().unwrap()).copied()
            else {
                return Ok(json!("none"));
            };
            let option = op["option"].as_str().map(str::to_owned);
            Ok(json!(registry::run_handler(
                w,
                h,
                identity,
                &p,
                node.as_ref(),
                option.as_deref()
            )?))
        }
        "npcHandler" => {
            let Some((identity, h)) = w.npc_handlers.get(op["via"].as_str().unwrap()).copied()
            else {
                return Ok(json!("none"));
            };
            let option = op["option"].as_str().map(str::to_owned);
            Ok(json!(registry::run_handler(
                w,
                h,
                identity,
                &p,
                node.as_ref(),
                option.as_deref()
            )?))
        }
        "component" => {
            let Some(c) = w.components.get(&int(&op["id"])).copied() else {
                return Ok(json!("none"));
            };
            Ok(json!(registry::run_component(
                w,
                c,
                &p,
                int(&op["opcode"]),
                int(&op["button"]),
                int(&op["slot"])
            )?))
        }
        "dialogue" => {
            let n = op["n"].as_u64().unwrap() as usize;
            let Some(Tracked::Dialogue(mut d)) = w.tracked.get(n).cloned() else {
                return Ok(json!("none"));
            };
            let r =
                registry::run_dialogue(w, &mut d, &p, int(&op["component"]), int(&op["button"]));
            w.tracked[n] = Tracked::Dialogue(d);
            r?;
            Ok(Json::Null)
        }
        "stage" => {
            let n = op["n"].as_u64().unwrap() as usize;
            if let Tracked::Dialogue(d) = &mut w.tracked[n] {
                d.set_stage(int(&op["value"]));
            }
            Ok(Json::Null)
        }
        "props" => {
            let Some((_, h)) = w.item_handlers.get(op["via"].as_str().unwrap()).copied() else {
                return Ok(json!("none"));
            };
            Ok(json!([h.is_walk(), h.is_walk_for(), h.is_delayed()]))
        }
        "select" => {
            let Some((_, h)) = w.item_handlers.get(op["via"].as_str().unwrap()).copied() else {
                return Ok(json!("none"));
            };
            registry::run_selection(w, h, int(&op["skill"]), &p)?;
            Ok(Json::Null)
        }
        "task" => {
            let n = op["n"].as_u64().unwrap() as usize;
            let Some(Tracked::Task(mut t)) = w.tracked.get(n).cloned() else {
                return Ok(json!("none"));
            };
            let r = registry::run_task(w, &mut t);
            w.tracked[n] = Tracked::Task(t);
            Ok(json!(r?))
        }
        "login" => {
            let n = op["n"].as_u64().unwrap() as usize;
            assert_eq!(w.login_listeners[n], Class::BrawlingGloves);
            gloves::login(w, &p)?;
            Ok(Json::Null)
        }
        "save" => {
            let n = op["n"].as_u64().unwrap() as usize;
            assert_eq!(w.persist_players[n], Class::BrawlingGloves);
            let mut save = Map::new();
            gloves::save_player(w, &p, &mut save)?;
            Ok(saved(&Json::Object(save)))
        }
        "parse" => {
            let n = op["n"].as_u64().unwrap() as usize;
            assert_eq!(w.persist_players[n], Class::BrawlingGloves);
            let data = op["data"].as_object().unwrap().clone();
            gloves::parse_player(w, &p, &data)?;
            Ok(Json::Null)
        }
        "call" => call(w, op, node),
        other => panic!("op {other}"),
    }
}

fn error_name(e: &Error) -> &'static str {
    match e {
        Error::Null => "Null",
        Error::Cast => "Cast",
        Error::Index => "Index",
        Error::Number => "Number",
        Error::NoSuchElement => "NoSuchElement",
        Error::Argument => "Argument",
        Error::State => "State",
        Error::Binding(_) => "Binding",
    }
}

fn snapshot(w: &mut World, result: Json, error: Option<&Error>) -> Json {
    let mut attrs = Map::new();
    let keys: Vec<String> = w.nodes.keys().cloned().collect();
    for k in keys {
        let n = w.nodes[&k].clone();
        if !matches!(n.kind, Kind::Player | Kind::Npc) || n.attrs.is_empty() {
            continue;
        }
        let mut m = Map::new();
        for (a, v) in &n.attrs {
            let out = w.attr_out(v);
            m.insert(a.clone(), out);
        }
        attrs.insert(k.clone(), Json::Object(m));
    }
    let stages: Vec<Json> = w
        .tracked
        .iter()
        .map(|t| match t {
            Tracked::Dialogue(d) => json!(d.stage()),
            Tracked::Task(_) => Json::Null,
        })
        .collect();
    json!({
        "attributes": attrs,
        "draws": w.generators.states(),
        "error": error.map(error_name),
        "events": w.events,
        "result": result,
        "stages": stages,
        "stderr": w.stderr,
    })
}

fn replay(case: &Json) -> Vec<Json> {
    let mut w = World::new(case);
    let mut out = Vec::new();
    for op in case["ops"].as_array().unwrap() {
        w.events.clear();
        w.stderr.clear();
        w.op_calls = 0;
        w.fail_at = op.get("failAt").map_or(-1, |v| v.as_i64().unwrap());
        let r = run(&mut w, op);
        let snap = match r {
            Ok(v) => snapshot(&mut w, v, None),
            Err(e) => snapshot(&mut w, Json::Null, Some(&e)),
        };
        out.push(snap);
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
    assert!(cases > 900);
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
    assert_eq!(classes, Class::ALL[..Class::ALL.len() - 1].to_vec());
    // The casket's table drew its insertion positions from RANDOM.
    let last = case["expected"].as_array().unwrap().last().unwrap();
    assert_eq!(json!(w.generators.states()), last["draws"]);
}

/// The native registrar installs the typed lambdas into the native
/// InteractionListeners and InterfaceListeners registries and collects the
/// item handlers and content objects.
#[test]
fn native_registrar_installs_into_the_native_registries() {
    use rs09_server::world::interaction_listeners as il;
    use rs09_server::world::interface_listeners as ifl;
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
    let interactions = il::Registry::default();
    let interfaces = ifl::Registry::default();
    let mut random = JavaGenerators::new(530, 531, 532);
    let mut native =
        registry::Native::new(&interactions, &interfaces, &Root, Vec::new(), &mut random);
    let classes = registry::register_scanned(&mut native).unwrap();
    assert_eq!(classes.len(), Class::ALL.len() - 1);
    assert!(native.casket.is_some());
    assert_eq!(native.item_handlers.len(), 3 + 23 + 4 + 6 + 1 + 1);
    assert_eq!(native.npc_handlers.len(), 1);
    assert_eq!(
        native.components,
        vec![
            (468, ComponentPlugin::DiangoReclaim),
            (659, ComponentPlugin::SnowGlobe)
        ]
    );
    assert_eq!(native.diango_items.len(), 38);
    assert_eq!(native.login_listeners, vec![Class::BrawlingGloves]);
    assert_eq!(native.persist_players, vec![Class::BrawlingGloves]);
    let text = |s: &str| -> il::Text { s.encode_utf16().collect() };
    let empty = interactions
        .get(&Root, 1933, 0, &text("empty bowl"))
        .unwrap();
    assert_eq!(*empty.value::<Interaction>().unwrap(), Interaction::Empty);
    let blamish = interactions.get(&Root, 1582, 0, &text("empty")).unwrap();
    assert!(
        blamish.same(&empty),
        "blamish oil is a finished potion: one lambda object"
    );
    let first = interactions.get_equip(3751, true).unwrap();
    assert_eq!(
        *first.value::<Interaction>().unwrap(),
        Interaction::QuestEquip(0)
    );
    let morph = interactions.get_equip(7927, true).unwrap();
    assert!(morph.same(&interactions.get_equip(6583, true).unwrap()));
    assert!(interfaces.get(375, None).unwrap().is_some());
    assert!(interfaces.get_close(375).unwrap().is_some());
    let inspected = interactions.inspect();
    let use_with = &inspected
        .iter()
        .find(|(n, _)| *n == "useWithListeners")
        .unwrap()
        .1;
    let use_of = |key: &str| -> Interaction {
        let (_, entry) = use_with
            .iter()
            .find(|(k, _)| String::from_utf16_lossy(k) == key)
            .unwrap();
        match entry {
            il::Entry::Function(f) => *f.value::<Interaction>().unwrap(),
            _ => panic!("{key} is no function"),
        }
    };
    assert_eq!(use_of("1925:2645:1"), Interaction::Sand);
    assert_eq!(use_of("5339:878:1"), Interaction::Water);
    assert_eq!(use_of("432:2079:1"), Interaction::ChestKey);
    let (_, _, lamp) = native
        .item_handlers
        .iter()
        .find(|(id, _, _)| *id == 2528)
        .unwrap();
    assert_eq!(
        lamp.read().value::<OptionPlugin>(),
        Some(&OptionPlugin::Lamp)
    );
}

/// WaterVessel.getInputs() is the JDK HashMap walk the oracle recorded, and
/// Kotlin's `random()` and RandomFunction's draws are the JDK algorithms.
#[test]
fn hash_walks_and_generators() {
    assert_eq!(
        tables::WaterVessel::input_keys(),
        vec![
            1825, 1923, 1827, 1925, 229, 1829, 1831, 7688, 6667, 1935, 5331, 5333, 5334, 5335,
            5336, 5337, 5338, 5339, 1980
        ]
    );
    let mut g = JavaGenerators::new(530, 531, 532);
    assert_eq!(g.states(), [25214903423, 25214903422, 532]);
    g.thread_int(3);
    assert_ne!(g.states()[2], 532);
}
