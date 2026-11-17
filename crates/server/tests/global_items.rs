//! Replays the global item handler corpus (fixtures/global-items) through
//! `world::global_items`. Every service answers from the case the way the
//! oracle's recording adapters do (oracle/global-items/adapters.py and
//! Support.java); every recorded call (registrations included), result,
//! error, the attributes, store archives and dialogue stages after each
//! operation are compared.
use rs09_server::world::global_items::{
    self as gi, Altar, Anim, Arg, Dialogue, Diary, Error, Expression, Gfx, Host, ItemArg, Kind,
    LogLevel, MonitorLog, Outcome, PulseType, Quest, QueueStrength, Result, Store, StoreValue,
    Task, TaskInput, TeleportMethod, TeleportType, Value,
    registry::{self, Class, DialoguePlugin, Interaction, OptionPlugin, Registrar},
};
use rs09_server::world::interaction_listeners::Kind as IntType;
use serde_json::{Map, Value as Json, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};

const CORPUS: &str = include_str!("../../../fixtures/global-items/oracle.jsonl");
const PROVENANCE: &str = include_str!("../../../fixtures/global-items/provenance.json");

/// The clock `System.currentTimeMillis()` answers here; the oracle records
/// times near its own clock relative to it.
const NOW: i64 = 1_700_000_000_000;
const NEAR: i64 = 864_000_000;

type N = String;

/// What the oracle tracks by identity: deferred bodies and dialogues.
#[derive(Clone)]
enum Tracked {
    Task(Task<N>),
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
    attrs: BTreeMap<String, Value>,
}

fn location(v: &Json) -> Option<[i32; 3]> {
    v.as_array().map(|a| [int(&a[0]), int(&a[1]), int(&a[2])])
}
fn coord(l: Option<[i32; 3]>) -> Json {
    l.map_or(Json::Null, |l| json!(l))
}

fn long_out(l: i64) -> Json {
    if (l - NOW).abs() < NEAR {
        json!({ "relative": ((l - NOW) as f64 / 1000.0).round() as i64 })
    } else {
        json!({ "long": l })
    }
}

/// Support.typed: a case value with its boxed class.
fn attr_in(v: &Json) -> Value {
    match v {
        Json::Bool(b) => Value::Bool(*b),
        Json::String(s) => Value::Text(s.clone()),
        Json::Number(n) => Value::Int(n.as_i64().unwrap() as i32),
        Json::Object(o) => {
            if let Some(l) = o.get("location") {
                Value::Location(location(l).unwrap())
            } else if let Some(l) = o.get("long") {
                Value::Long(l.as_i64().unwrap())
            } else if let Some(r) = o.get("relative") {
                Value::Long(NOW + r.as_i64().unwrap() * 1000)
            } else {
                panic!("attribute {v}")
            }
        }
        _ => panic!("attribute {v}"),
    }
}
fn attr_out(v: &Value) -> Json {
    match v {
        Value::Int(i) => json!(i),
        Value::Long(l) => long_out(*l),
        Value::Bool(b) => json!(b),
        Value::Text(t) => json!(t),
        Value::Location(l) => json!({ "location": l }),
        Value::Object(()) => panic!("object attribute"),
    }
}
fn store_in(v: &Json) -> StoreValue {
    match v {
        Json::Bool(b) => StoreValue::Bool(*b),
        Json::String(s) => StoreValue::Text(s.clone()),
        Json::Number(n) => StoreValue::Int(n.as_i64().unwrap() as i32),
        Json::Object(o) => {
            if let Some(l) = o.get("long") {
                StoreValue::Long(l.as_i64().unwrap())
            } else if let Some(r) = o.get("relative") {
                StoreValue::Long(NOW + r.as_i64().unwrap() * 1000)
            } else if let Some(d) = o.get("double") {
                StoreValue::Double(d.as_f64().unwrap())
            } else if let Some(f) = o.get("float") {
                StoreValue::Float(f.as_f64().unwrap() as f32)
            } else {
                panic!("store value {v}")
            }
        }
        _ => panic!("store value {v}"),
    }
}
fn store_out(v: &StoreValue) -> Json {
    match v {
        StoreValue::Int(i) => json!(i),
        StoreValue::Long(l) => long_out(*l),
        StoreValue::Double(d) => json!({ "double": d }),
        StoreValue::Float(f) => json!({ "float": *f as f64 }),
        StoreValue::Text(t) => json!(t),
        StoreValue::Bool(b) => json!(b),
    }
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

struct World {
    nodes: BTreeMap<String, NodeData>,
    values: Map<String, Json>,
    answers: Map<String, Json>,
    answer_calls: HashMap<String, usize>,
    stores: BTreeMap<String, BTreeMap<String, StoreValue>>,
    tracked: Vec<Tracked>,
    created: usize,
    // registrations
    shared: HashMap<Interaction, usize>,
    object_labels: HashMap<usize, String>,
    next_label: usize,
    interactions: Vec<Interaction>,
    item_handlers: BTreeMap<String, OptionPlugin>,
    dialogues: BTreeMap<i32, DialoguePlugin>,
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
            values: Map::new(),
            answers: Map::new(),
            answer_calls: HashMap::new(),
            stores: BTreeMap::new(),
            tracked: Vec::new(),
            created: 0,
            shared: HashMap::new(),
            object_labels: HashMap::new(),
            next_label: 0,
            interactions: Vec::new(),
            item_handlers: BTreeMap::new(),
            dialogues: BTreeMap::new(),
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
                        "npc" => Kind::Npc,
                        "scenery" => Kind::Scenery,
                        "item" => Kind::Item,
                        other => panic!("{other}"),
                    },
                    id: 0,
                    name: None,
                    location: None,
                    amount: 1,
                    slot: -1,
                    attrs: BTreeMap::new(),
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
        if let Some(stores) = v.get("stores").and_then(Json::as_object) {
            for (k, entries) in stores {
                let store = self.stores.entry(k.clone()).or_default();
                for (e, value) in entries.as_object().unwrap() {
                    if value.is_null() {
                        store.remove(e);
                    } else {
                        store.insert(e.clone(), store_in(value));
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
    type HandlerRef = (usize, OptionPlugin);
    type DialogueRef = (usize, DialoguePlugin);
    fn function(&mut self, f: Interaction) -> Self::FunctionRef {
        if f.shared() {
            let identity = *self.shared.entry(f).or_insert_with(fresh);
            return (identity, f);
        }
        (fresh(), f)
    }
    fn handler_object(&mut self, handler: OptionPlugin) -> Self::HandlerRef {
        (fresh(), handler)
    }
    fn dialogue_object(&mut self, dialogue: DialoguePlugin) -> Self::DialogueRef {
        (fresh(), dialogue)
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
        used: i32,
        with: &[i32],
        f: &Self::FunctionRef,
    ) -> Result<()> {
        let label = self.label(f.0, &f.1.class());
        self.emit(json!(["onUseWith", int_kind(kind), used, with, label]))?;
        self.interactions.push(f.1);
        Ok(())
    }
    fn put_item_handler(&mut self, id: i32, key: &str, handler: &Self::HandlerRef) -> Result<()> {
        let label = self.label(handler.0, handler.1.class());
        self.emit(json!(["itemHandler", id, key, label]))?;
        self.item_handlers.insert(format!("{id}:{key}"), handler.1);
        Ok(())
    }
    fn add_dialogue(&mut self, id: i32, plugin: &Self::DialogueRef) -> Result<()> {
        let label = self.label(plugin.0, plugin.1.class());
        self.emit(json!(["dialogueAdd", id, label]))?;
        self.dialogues.insert(id, plugin.1);
        Ok(())
    }
}

// ---------------------------------------------------------------------- the services

impl Host for World {
    type Node = N;

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
    fn location(&mut self, node: &N) -> Result<Option<[i32; 3]>> {
        Ok(self.data(node).location)
    }
    fn lowercase_default(&self, text: &str) -> String {
        rs09_game_core::java_lower::lowercase_root(text)
    }
    fn lowercase_root(&self, text: &str) -> String {
        rs09_game_core::java_lower::lowercase_root(text)
    }
    fn ticks(&mut self) -> i32 {
        self.values.get("ticks").map_or(0, int)
    }
    fn current_time_millis(&mut self) -> i64 {
        NOW
    }
    fn home_location(&mut self) -> Option<[i32; 3]> {
        Some([3222, 3218, 0])
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
    fn new_item(&mut self, id: i32, amount: i32) -> Result<N> {
        self.emit(json!(["newItem", id, amount]))?;
        self.created += 1;
        let key = format!("new:{id}:{}", self.created);
        let name = self
            .answer(&format!("itemName:{id}"), Json::Null)
            .as_str()
            .map(str::to_owned);
        self.nodes.insert(
            key.clone(),
            NodeData {
                kind: Kind::Item,
                id,
                name,
                location: None,
                amount,
                slot: -1,
                attrs: BTreeMap::new(),
            },
        );
        Ok(key)
    }
    fn new_npc(&mut self, id: i32) -> Result<()> {
        self.emit(json!(["newNpc", id]))
    }
    fn get_attribute(&mut self, entity: &N, key: &str) -> Result<Option<Value>> {
        self.emit(json!(["getAttribute", key]))?;
        Ok(self
            .data(entity)
            .attrs
            .get(&key.replace("/save:", ""))
            .cloned())
    }
    fn set_attribute(&mut self, entity: &N, key: &str, value: Value) -> Result<()> {
        self.emit(json!(["setAttribute", key, attr_out(&value)]))?;
        let key = key.strip_prefix("/save:").unwrap_or(key).to_owned();
        self.nodes.get_mut(entity).unwrap().attrs.insert(key, value);
        Ok(())
    }
    fn send_message(&mut self, _: &N, text: &str) -> Result<()> {
        self.emit(json!(["message", text]))
    }
    fn send_messages(&mut self, _: &N, texts: &[&str]) -> Result<()> {
        self.emit(json!(["messages", texts]))
    }
    fn send_dialogue(&mut self, _: &N, text: &str) -> Result<()> {
        self.emit(json!(["sendDialogue", text]))
    }
    fn npc_dialogue(
        &mut self,
        _: &N,
        npc: i32,
        message: &str,
        expression: Expression,
        hide: bool,
    ) -> Result<()> {
        self.emit(json!([
            "npcDialogue",
            npc,
            message,
            expression.name(),
            hide
        ]))
    }
    fn open_dialogue(&mut self, _: &N, dialogue: Dialogue<N>) -> Result<()> {
        let n = self.track(Tracked::Dialogue(dialogue));
        self.emit(json!(["openDialogue", n, []]))
    }
    fn open_dialogue_plugin(&mut self, _: &N, key: i32, args: &[Arg]) -> Result<bool> {
        let args: Vec<Json> = args
            .iter()
            .map(|a| match a {
                Arg::Int(i) => json!(i),
                Arg::Text(t) => json!(t),
            })
            .collect();
        self.emit(json!(["openDialoguePlugin", key, args]))?;
        Ok(self.answer_bool("openDialoguePlugin", true))
    }
    fn send_options(&mut self, _: &N, title: &str, options: &[&str]) -> Result<()> {
        self.emit(json!(["sendOptions", title, options]))
    }
    fn interpreter_close(&mut self, _: &N) -> Result<bool> {
        self.emit(json!(["interpreterClose"]))?;
        Ok(true)
    }
    fn add_dialogue_action(&mut self, _: &N, action: Task<N>) -> Result<()> {
        let n = self.track(Tracked::Task(action));
        self.emit(json!(["addDialogueAction", n]))
    }
    fn send_item_select(
        &mut self,
        _: &N,
        options: &[&str],
        keep_alive: bool,
        callback: Task<N>,
    ) -> Result<()> {
        let n = self.track(Tracked::Task(callback));
        self.emit(json!(["sendItemSelect", options, keep_alive, n]))
    }
    fn set_varp(&mut self, _: &N, index: i32, value: i32, save: bool) -> Result<()> {
        self.emit(json!(["setVarp", index, value, save]))
    }
    fn has_requirement(&mut self, _: &N, quest: Quest, message: bool) -> Result<bool> {
        self.emit(json!(["hasRequirement", quest.name(), message]))?;
        Ok(self.answer_bool(&format!("hasRequirement:{}", quest.name()), true))
    }
    fn is_quest_complete(&mut self, _: &N, quest: Quest) -> Result<bool> {
        self.emit(json!(["isQuestComplete", quest.name()]))?;
        Ok(self.answer_bool(&format!("isQuestComplete:{}", quest.name()), true))
    }
    fn has_level_stat(&mut self, _: &N, skill: i32, level: i32) -> Result<bool> {
        self.emit(json!(["hasLevelStat", skill, level]))?;
        Ok(self.answer_bool("hasLevelStat", true))
    }
    fn altar_requirement(&mut self, _: &N, altar: Altar) -> Result<bool> {
        self.emit(json!(["altarRequirement", altar.name()]))?;
        Ok(self.answer_bool("altarRequirement", true))
    }
    fn wilderness_check_teleport(&mut self, _: &N, level: i32) -> Result<bool> {
        self.emit(json!(["checkTeleport", level]))?;
        Ok(self.answer_bool("checkTeleport", true))
    }
    fn zone_teleport(&mut self, _: &N, kind: i32, node: &N) -> Result<bool> {
        self.emit(json!(["zoneTeleport", kind, node]))?;
        Ok(self.answer_bool("zoneTeleport", true))
    }
    fn is_tele_blocked(&mut self, _: &N) -> Result<bool> {
        self.emit(json!(["isTeleBlocked"]))?;
        Ok(self.answer_bool("isTeleBlocked", false))
    }
    fn queue_script(
        &mut self,
        _: &N,
        delay: i32,
        strength: QueueStrength,
        persist: bool,
        script: Task<N>,
    ) -> Result<()> {
        let n = self.track(Tracked::Task(script));
        self.emit(json!(["queueScript", delay, strength.name(), persist, n]))
    }
    fn delay_script(&mut self, _: &N, ticks: i32) -> Result<bool> {
        self.emit(json!(["delayScript", ticks]))?;
        Ok(self.answer_bool("delayScript", false))
    }
    fn stop_executing(&mut self, _: &N) -> Result<bool> {
        self.emit(json!(["stopExecuting"]))?;
        Ok(self.answer_bool("stopExecuting", true))
    }
    fn set_script_delay(&mut self, _: &N, delay: i32) -> Result<()> {
        self.emit(json!(["scriptDelay", delay]))
    }
    fn lock(&mut self, _: &N, ticks: i32) -> Result<()> {
        self.emit(json!(["lock", ticks]))
    }
    fn lock_all(&mut self, _: &N) -> Result<()> {
        self.emit(json!(["lockAll"]))
    }
    fn unlock(&mut self, _: &N) -> Result<()> {
        self.emit(json!(["unlock"]))
    }
    fn pulse_clear(&mut self, _: &N, kind: PulseType) -> Result<()> {
        self.emit(json!(["pulseClear", kind.name()]))
    }
    fn submit_world(&mut self, pulse: Task<N>) -> Result<()> {
        let n = self.track(Tracked::Task(pulse));
        self.emit(json!(["submitWorld", n]))
    }
    fn animate(&mut self, _: &N, animation: Anim) -> Result<bool> {
        self.emit(json!(["animate", anim(animation)]))?;
        Ok(true)
    }
    fn force_animation(&mut self, _: &N, animation: Anim) -> Result<bool> {
        self.emit(json!(["forceAnimation", anim(animation)]))?;
        Ok(true)
    }
    fn visualize(&mut self, _: &N, animation: Anim, graphics: Gfx) -> Result<bool> {
        self.emit(json!(["visualize", anim(animation), gfx(graphics)]))?;
        Ok(true)
    }
    fn graphics(&mut self, _: &N, graphics: Gfx) -> Result<bool> {
        self.emit(json!(["graphics", gfx(graphics)]))?;
        Ok(true)
    }
    fn animation_duration(&mut self, id: i32) -> Result<i32> {
        self.emit(json!(["animationDuration", id]))?;
        Ok(self.answer_int(&format!("duration:{id}"), 1))
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
    fn play_global_audio(
        &mut self,
        location: [i32; 3],
        id: i32,
        delay: i32,
        loops: i32,
        radius: i32,
    ) -> Result<()> {
        self.emit(json!(["globalAudio", location, id, delay, loops, radius]))
    }
    fn face_location(&mut self, _: &N, location: [i32; 3]) -> Result<()> {
        self.emit(json!(["faceLocation", location]))
    }
    fn teleport_send(&mut self, _: &N, location: [i32; 3], kind: TeleportType) -> Result<bool> {
        self.emit(json!(["teleportSend", location, kind.name()]))?;
        Ok(self.answer_bool("teleportSend", true))
    }
    fn set_teleport_location(&mut self, _: &N, location: [i32; 3]) -> Result<()> {
        self.emit(json!(["setTeleportLocation", location]))
    }
    fn set_disabled_ticks(&mut self, _: &N, ticks: i32) -> Result<()> {
        self.emit(json!(["disabledTicks", ticks]))
    }
    fn teleport_event(
        &mut self,
        _: &N,
        kind: TeleportType,
        method: TeleportMethod,
        source: &N,
        location: [i32; 3],
    ) -> Result<()> {
        self.emit(json!([
            "teleportEvent",
            kind.name(),
            method.name(),
            source,
            location
        ]))
    }
    fn close_interface(&mut self, _: &N) -> Result<()> {
        self.emit(json!(["closeInterface"]))
    }
    fn close_all_interfaces(&mut self, _: &N) -> Result<()> {
        self.emit(json!(["closeAllInterfaces"]))
    }
    fn finish_diary_task(&mut self, _: &N, diary: Diary, level: i32, task: i32) -> Result<()> {
        self.emit(json!(["finishTask", diary.name(), level, task]))
    }
    fn remove_item(&mut self, _: &N, item: ItemArg<N>, store: Store) -> Result<bool> {
        let item = match item {
            ItemArg::Item(n) => json!(n),
            ItemArg::Id(i) => json!(i),
        };
        self.emit(json!(["removeItem", item, store.name()]))?;
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
    fn in_inventory(&mut self, _: &N, id: i32, amount: i32) -> Result<bool> {
        self.emit(json!(["inInventory", id, amount]))?;
        Ok(self.answer_bool("inInventory", true))
    }
    fn inventory_remove(&mut self, _: &N, item: &N) -> Result<bool> {
        self.emit(json!(["inventoryRemove", item]))?;
        Ok(self.answer_bool("inventoryRemove", true))
    }
    fn inventory_add(&mut self, _: &N, item: &N) -> Result<bool> {
        self.emit(json!(["inventoryAdd", item]))?;
        Ok(self.answer_bool("inventoryAdd", true))
    }
    fn inventory_get(&mut self, _: &N, slot: i32) -> Result<Option<N>> {
        self.emit(json!(["inventoryGet", slot]))?;
        Ok(self.answer_node("inventoryGet", Json::Null))
    }
    fn log(&mut self, class: &str, level: LogLevel, message: &str) -> Result<()> {
        self.emit(json!(["log", class, level.name(), message]))
    }
    fn monitor_log(&mut self, _: &N, kind: MonitorLog, details: &str) -> Result<()> {
        self.emit(json!(["playerMonitorLog", kind.name(), details]))
    }
    fn slayer_has_task(&mut self, _: &N) -> Result<bool> {
        self.emit(json!(["slayerHasTask"]))?;
        Ok(self.answer_bool("slayerHasTask", true))
    }
    fn slayer_master_npc(&mut self, _: &N) -> Result<Option<i32>> {
        self.emit(json!(["slayerMaster"]))?;
        Ok(self
            .answer("slayerMaster", json!(8273))
            .as_i64()
            .map(|n| n as i32))
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
    fn house_exit_location(&mut self, _: &N) -> Result<Option<[i32; 3]>> {
        self.emit(json!(["houseExitLocation"]))?;
        Ok(location(&self.answer("houseExitLocation", Json::Null)))
    }
    fn house_pre_enter(&mut self, _: &N, building: bool) -> Result<()> {
        self.emit(json!(["housePreEnter", building]))
    }
    fn house_enter_location(&mut self, _: &N) -> Result<Option<[i32; 3]>> {
        self.emit(json!(["houseEnterLocation"]))?;
        Ok(location(
            &self.answer("houseEnterLocation", json!([1950, 5730, 0])),
        ))
    }
    fn house_post_enter(&mut self, _: &N, building: bool) -> Result<()> {
        self.emit(json!(["housePostEnter", building]))
    }
    fn store_archive(&mut self, name: &str) -> Result<()> {
        self.emit(json!(["storeArchive", name]))?;
        self.stores.entry(name.to_owned()).or_default();
        Ok(())
    }
    fn store_get(&mut self, archive: &str, key: &str) -> Result<Option<StoreValue>> {
        self.emit(json!(["storeGet", archive, key]))?;
        Ok(self.stores[archive].get(key).cloned())
    }
    fn store_put(&mut self, archive: &str, key: &str, value: StoreValue) -> Result<()> {
        self.emit(json!(["storePut", archive, key, store_out(&value)]))?;
        self.stores
            .get_mut(archive)
            .unwrap()
            .insert(key.to_owned(), value);
        Ok(())
    }
    fn run_energy(&mut self, _: &N) -> Result<f64> {
        self.emit(json!(["runEnergy"]))?;
        Ok(self.answer("runEnergy", json!(50.0)).as_f64().unwrap())
    }
    fn update_run_energy(&mut self, _: &N, delta: f64) -> Result<()> {
        self.emit(json!(["updateRunEnergy", delta]))
    }
    fn alchemize(&mut self, _: &N, item: &N, high: bool, explorers_ring: bool) -> Result<bool> {
        self.emit(json!(["alchemize", item, high, explorers_ring]))?;
        Ok(self.answer_bool("alchemize", true))
    }
    fn prayer_effect(&mut self, _: &N, base: f64, bonus: f64, round_up: bool) -> Result<()> {
        self.emit(json!(["prayerEffect", base, bonus, round_up]))
    }
}

// ---------------------------------------------------------------------- operations

fn node_of(w: &World, v: &Json) -> Option<N> {
    v.as_str()
        .filter(|k| w.nodes.contains_key(*k))
        .map(str::to_owned)
}

fn outcome(o: Outcome) -> Json {
    match o {
        Outcome::Bool(b) => json!(b),
        Outcome::Unit => Json::Null,
    }
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
                rs09_server::world::interaction_listeners::FunctionKind::On => {
                    registry::run_interaction(w, f, &p, &node.ok_or(Error::Null)?, None)?
                }
                _ => {
                    let used = node_of(w, &op["used"]).ok_or(Error::Null)?;
                    let with = node_of(w, &op["with"]);
                    registry::run_interaction(w, f, &p, &used, with.as_ref())?
                }
            };
            Ok(json!(r))
        }
        "handler" => {
            let Some(h) = w.item_handlers.get(op["via"].as_str().unwrap()).copied() else {
                return Ok(json!("none"));
            };
            let option = op["option"].as_str().map(str::to_owned);
            Ok(json!(registry::run_handler(
                w,
                h,
                &p,
                node.as_ref(),
                option.as_deref()
            )?))
        }
        "props" => {
            let Some(h) = w.item_handlers.get(op["via"].as_str().unwrap()).copied() else {
                return Ok(json!("none"));
            };
            Ok(json!([h.is_walk(), h.is_walk_for(), h.is_delayed()]))
        }
        "dialogueOpen" => {
            let Some(plugin) = w.dialogues.get(&int(&op["key"])).copied() else {
                return Ok(json!("none"));
            };
            let args: Vec<Arg> = op["args"]
                .as_array()
                .unwrap()
                .iter()
                .map(|a| match a {
                    Json::String(s) => Arg::Text(s.clone()),
                    other => Arg::Int(int(other)),
                })
                .collect();
            let (dialogue, opened) = registry::open_dialogue(w, plugin, &p, &args);
            w.track(Tracked::Dialogue(dialogue));
            Ok(json!(opened?))
        }
        "dialogue" => {
            let n = op["n"].as_u64().unwrap() as usize;
            let Some(Tracked::Dialogue(mut d)) = w.tracked.get(n).cloned() else {
                return Ok(json!("none"));
            };
            let r = registry::run_dialogue(w, &mut d, int(&op["component"]), int(&op["button"]));
            w.tracked[n] = Tracked::Dialogue(d);
            Ok(outcome(r?))
        }
        "task" => {
            let n = op["n"].as_u64().unwrap() as usize;
            let Some(Tracked::Task(mut t)) = w.tracked.get(n).cloned() else {
                return Ok(json!("none"));
            };
            let input = match &t {
                Task::FaladorEmote { .. } => TaskInput::Pulse,
                Task::AlchemyAction { .. } => TaskInput::Button(int(&op["button"])),
                Task::AlchemySelect { .. } => TaskInput::Select {
                    slot: int(&op["slot"]),
                    option: int(&op["option"]),
                },
                _ => TaskInput::Stage(int(&op["stage"])),
            };
            let r = registry::run_task(w, &mut t, input);
            w.tracked[n] = Tracked::Task(t);
            Ok(outcome(r?))
        }
        other => panic!("op {other}"),
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
        if !matches!(n.kind, Kind::Player | Kind::Npc) || n.attrs.is_empty() {
            continue;
        }
        let m: Map<String, Json> = n
            .attrs
            .iter()
            .map(|(a, v)| (a.clone(), attr_out(v)))
            .collect();
        attrs.insert(k.clone(), Json::Object(m));
    }
    let stores: Map<String, Json> = w
        .stores
        .iter()
        .map(|(k, s)| {
            (
                k.clone(),
                Json::Object(s.iter().map(|(e, v)| (e.clone(), store_out(v))).collect()),
            )
        })
        .collect();
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
        "error": error.map(error_name),
        "events": w.events,
        "result": result,
        "stages": stages,
        "stores": stores,
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
    assert!(cases > 800);
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
    assert_eq!(classes, Class::ALL.to_vec());
}

/// The native registrar installs the typed lambdas into the native
/// InteractionListeners registry (one object for a non-capturing lambda) and
/// collects the item handlers and dialogue plugins.
#[test]
fn native_registrar_installs_into_the_native_registries() {
    use rs09_server::world::interaction_listeners as il;
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
    let mut native = registry::Native::new(&interactions, &Root);
    let classes = registry::register_scanned(&mut native).unwrap();
    assert_eq!(classes.len(), Class::ALL.len());
    assert_eq!(native.item_handlers.len(), 6 + 4);
    assert_eq!(
        native.dialogue_plugins,
        vec![(3999111, DialoguePlugin::TeleportCrystal)]
    );
    let text = |s: &str| -> il::Text { s.encode_utf16().collect() };
    let rub = interactions.get(&Root, 1712, 0, &text("rub")).unwrap();
    assert_eq!(
        *rub.value::<Interaction>().unwrap(),
        Interaction::JewelleryRub
    );
    let glory = interactions.get(&Root, 1704, 0, &text("rub")).unwrap();
    assert!(glory.same(&rub));
    let first = interactions.get(&Root, 8016, 0, &text("break")).unwrap();
    let last = interactions.get(&Root, 8021, 0, &text("break")).unwrap();
    assert!(
        first.same(&last),
        "the non-capturing break lambda is one object"
    );
    let tab = interactions.get(&Root, 13611, 0, &text("break")).unwrap();
    assert_eq!(*tab.value::<Interaction>().unwrap(), Interaction::TeleTab);
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
    assert_eq!(use_of("4252:5282:1"), Interaction::EctophialRefill);
    assert_eq!(
        use_of("8020:1702:0"),
        Interaction::TabletUse { enchanted: 1704 }
    );
    assert_eq!(use_with.len(), 1 + 24);
    let (_, _, shield) = &native.item_handlers[0];
    assert_eq!(
        shield.read().value::<OptionPlugin>(),
        Some(&OptionPlugin::FaladorShield)
    );
}

/// EnchantedJewellery.idMap's key order is the JDK HashMap walk the oracle
/// recorded.
#[test]
fn id_map_walk_is_the_hash_order() {
    let keys = gi::jewellery::id_map_keys();
    assert_eq!(keys.len(), 50);
    assert_eq!(&keys[..6], &[2560, 2562, 2564, 2566, 2570, 3853]);
}
