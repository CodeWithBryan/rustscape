//! Replays the packet handler corpus (fixtures/packet-handlers) through
//! `world::packet_handlers`. Every service answers from the case the way the
//! oracle's recording adapters do (oracle/packet-handlers/adapters.py and
//! Support.java); every recorded call, result, error and the state after each
//! operation is compared.
use rs09_game_core::java_lower::lowercase_root;
use rs09_server::definitions::{
    item::{self, Shared},
    npc,
    plugin::{self, NativePlugin},
    property::Property,
    scenery,
};
use rs09_server::world::interaction::{OptionData, OptionHandle};
use rs09_server::world::interaction_listeners::Kind;
use rs09_server::world::packet_handlers::{
    Base, Container, Error, Listener, NodeKind, Result,
    actions::{self, ItemAction, NpcAction, PlayerAction},
    component::{self, ComponentAction},
    drop::{self, DestroyAction, DropScript},
    equip::{self, Fired},
    examine, item_option, kind_name,
    slot_switch::{self, SlotSwitch},
    use_with::{self, Body, Event, HandlerData, HandlerList, Registry, UseHandler, UseWith},
};
use rs09_server::world::spells::SpellBook;
use serde_json::{Map, Value as Json, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashMap};

const CORPUS: &str = include_str!("../../../fixtures/packet-handlers/oracle.jsonl");
const PROVENANCE: &str = include_str!("../../../fixtures/packet-handlers/provenance.json");

type N = String;

fn int(v: &Json) -> i32 {
    v.as_i64().unwrap() as i32
}

struct Label(String);
impl NativePlugin for Label {
    fn label(&self) -> &str {
        &self.0
    }
}

#[derive(Clone)]
struct NodeData {
    kind: NodeKind,
    id: i32,
    index: i32,
    name: Option<String>,
    active: bool,
    location: Option<[i32; 3]>,
    interaction: Option<String>,
    text: String,
    shown: Option<String>,
    child: Option<String>,
    spawn: Option<[i32; 3]>,
    config_file: i32,
    amount: i32,
    definition: i32,
    drop: Option<String>,
}

struct Interaction {
    owner: String,
    options: Vec<Option<OptionHandle>>,
}

enum Pulse {
    Item(item_option::Task<N>),
    Use(Body<N>),
}

struct World {
    nodes: BTreeMap<String, NodeData>,
    interactions: BTreeMap<String, Interaction>,
    option_keys: HashMap<usize, String>,
    containers: BTreeMap<String, Vec<Option<String>>>,
    tab_start: Vec<i32>,
    repo_players: HashMap<i32, String>,
    repo_npcs: HashMap<i32, String>,
    player_length: i32,
    npc_length: i32,
    max_players: i32,
    max_npcs: i32,
    object: Option<String>,
    object_at: Option<String>,
    ground: Option<String>,
    flags: Map<String, Json>,
    values: Map<String, Json>,
    answers: Map<String, Json>,
    answer_calls: HashMap<String, usize>,
    attributes: BTreeMap<String, Json>,
    item_defs: BTreeMap<i32, item::Handle>,
    scenery_defs: BTreeMap<i32, scenery::Handle>,
    npc_defs: BTreeMap<i32, npc::Handle>,
    missing: BTreeSet<String>,
    pulses: Vec<Pulse>,
    scripts: Vec<DropScript<N>>,
    actions: Vec<DestroyAction<N>>,
    listeners: Vec<Listener>,
    registry: Registry,
    use_handlers: BTreeMap<String, UseHandler>,
    use_refs: Vec<String>,
    hooks: Vec<Json>,
    events: Vec<Json>,
    calls: HashMap<String, i64>,
    op_calls: i64,
    fail_at: i64,
}

fn location(v: &Json) -> Option<[i32; 3]> {
    v.as_array().map(|a| [int(&a[0]), int(&a[1]), int(&a[2])])
}
fn coord(l: Option<[i32; 3]>) -> Json {
    l.map_or(Json::Null, |l| json!(l))
}
fn text(v: &Json) -> Option<String> {
    v.as_str().map(str::to_owned)
}
fn property(v: &Json) -> Property {
    match v {
        Json::Null => Property::Null,
        Json::Bool(b) => Property::Bool(*b),
        Json::String(s) => Property::Text(s.clone()),
        Json::Number(n) => Property::Int(n.as_i64().unwrap() as i32),
        Json::Object(o) => Property::Plugin(plugin::Handle::new(Label(
            o["plugin"].as_str().unwrap().to_owned(),
        ))),
        Json::Array(_) => panic!("array handler"),
    }
}
fn index_error(index: i32, length: usize) -> Error {
    Error::Index(Some(format!(
        "Index {index} out of bounds for length {length}"
    )))
}

impl World {
    fn new(case: &Json) -> Self {
        let mut w = Self {
            nodes: BTreeMap::new(),
            interactions: BTreeMap::new(),
            option_keys: HashMap::new(),
            containers: BTreeMap::from([
                ("inventory".to_owned(), vec![None; 28]),
                ("bank".to_owned(), vec![None; 496]),
                ("equipment".to_owned(), vec![None; 14]),
            ]),
            tab_start: vec![0; 11],
            repo_players: HashMap::new(),
            repo_npcs: HashMap::new(),
            player_length: 0,
            npc_length: 0,
            max_players: 2000,
            max_npcs: 32000,
            object: None,
            object_at: None,
            ground: None,
            flags: Map::new(),
            values: Map::new(),
            answers: Map::new(),
            answer_calls: HashMap::new(),
            attributes: BTreeMap::new(),
            item_defs: BTreeMap::new(),
            scenery_defs: BTreeMap::new(),
            npc_defs: BTreeMap::new(),
            missing: BTreeSet::new(),
            pulses: Vec::new(),
            scripts: Vec::new(),
            actions: Vec::new(),
            listeners: Vec::new(),
            registry: Registry::default(),
            use_handlers: BTreeMap::new(),
            use_refs: Vec::new(),
            hooks: case["hooks"].as_array().cloned().unwrap_or_default(),
            events: Vec::new(),
            calls: HashMap::new(),
            op_calls: 0,
            fail_at: -1,
        };
        w.set(&json!({"nodes": {"p": {"kind": "player"}}}));
        let p = w.nodes.get_mut("p").unwrap();
        p.location = Some([3222, 3218, 0]);
        p.name = Some("tracer".into());
        w.set(case);
        w.player_length = w.max_players;
        w.npc_length = w.max_npcs;
        w
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

    fn flag(&self, key: &str) -> bool {
        self.flags.get(key) == Some(&Json::Bool(true))
    }
    fn value_int(&self, key: &str, default: i32) -> i32 {
        self.values
            .get(key)
            .filter(|v| !v.is_null())
            .map_or(default, int)
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
    fn node(&self, key: &Option<String>) -> Option<N> {
        key.as_ref()
            .filter(|k| self.nodes.contains_key(*k))
            .cloned()
    }
    fn data(&self, n: &N) -> &NodeData {
        &self.nodes[n]
    }
    fn option_key(&self, o: &OptionHandle) -> String {
        self.option_keys[&o.identity()].clone()
    }
    fn item_def(&mut self, id: i32) -> item::Handle {
        self.item_defs
            .entry(id)
            .or_insert_with(|| Shared::new(item::ItemDefinition::default()))
            .clone()
    }

    fn set_node(&mut self, k: &str, v: &Json) {
        if !self.nodes.contains_key(k) {
            let kind_text = v["kind"].as_str().unwrap();
            let kind = match kind_text {
                "item" => NodeKind::Item,
                "ground" => NodeKind::GroundItem,
                "npc" => NodeKind::Npc,
                "player" => NodeKind::Player,
                "scenery" => NodeKind::Scenery,
                other => panic!("{other}"),
            };
            let definition = v.get("definition").or_else(|| v.get("id")).map_or(0, int);
            self.nodes.insert(
                k.to_owned(),
                NodeData {
                    kind,
                    id: 0,
                    index: 0,
                    name: None,
                    active: true,
                    location: None,
                    interaction: None,
                    text: format!("{kind_text}:{k}"),
                    shown: None,
                    child: None,
                    spawn: None,
                    config_file: -1,
                    amount: 1,
                    definition,
                    drop: None,
                },
            );
        }
        let n = self.nodes.get_mut(k).unwrap();
        let o = v.as_object().unwrap();
        if let Some(x) = o.get("id") {
            n.id = int(x);
        }
        if let Some(x) = o.get("index") {
            n.index = int(x);
        }
        if let Some(x) = o.get("name") {
            n.name = text(x);
        }
        if let Some(x) = o.get("active") {
            n.active = x == &Json::Bool(true);
        }
        if let Some(x) = o.get("location") {
            n.location = location(x);
        }
        if let Some(x) = o.get("interaction") {
            n.interaction = text(x);
        }
        if let Some(x) = o.get("text") {
            n.text = text(x).unwrap();
        }
        if let Some(x) = o.get("shown") {
            n.shown = text(x);
        }
        if let Some(x) = o.get("child") {
            n.child = text(x);
        }
        if let Some(x) = o.get("spawn") {
            n.spawn = location(x);
        }
        if let Some(x) = o.get("configFile") {
            n.config_file = int(x);
        }
        if let Some(x) = o.get("amount") {
            n.amount = int(x);
        }
        if let Some(x) = o.get("definition") {
            n.definition = int(x);
        }
        if let Some(x) = o.get("drop") {
            n.drop = text(x);
        }
    }

    fn set_definitions(&mut self, v: &Json) {
        if let Some(defs) = v.get("itemDefs").and_then(Json::as_object) {
            for (k, d) in defs {
                let def = self.item_def(k.parse().unwrap());
                let mut w = def.write();
                if let Some(x) = d.get("name") {
                    w.name = text(x);
                }
                if let Some(x) = d.get("examine") {
                    w.examine = text(x);
                }
                if let Some(x) = d.get("value") {
                    w.value = int(x);
                }
                if let Some(x) = d.get("model") {
                    w.interface_model_id = int(x);
                }
                if let Some(x) = d.get("unnoted") {
                    w.unnoted = x == &Json::Bool(true);
                }
                for (hk, hv) in d
                    .get("handlers")
                    .and_then(Json::as_object)
                    .into_iter()
                    .flatten()
                {
                    w.handlers.insert(hk.clone(), property(hv));
                }
            }
        }
        if let Some(defs) = v.get("sceneryDefs").and_then(Json::as_object) {
            for (k, d) in defs {
                let def = self
                    .scenery_defs
                    .entry(k.parse().unwrap())
                    .or_insert_with(|| Shared::new(scenery::SceneryDefinition::default()))
                    .clone();
                let mut w = def.write();
                if let Some(x) = d.get("name") {
                    w.name = text(x);
                }
                if let Some(x) = d.get("examine") {
                    w.examine = text(x);
                }
                if let Some(x) = d.get("configFile") {
                    w.config_file_id = int(x);
                }
                for (hk, hv) in d
                    .get("handlers")
                    .and_then(Json::as_object)
                    .into_iter()
                    .flatten()
                {
                    w.handlers.write().insert(hk.clone(), property(hv));
                }
            }
        }
        if let Some(defs) = v.get("npcDefs").and_then(Json::as_object) {
            for (k, d) in defs {
                let def = self
                    .npc_defs
                    .entry(k.parse().unwrap())
                    .or_insert_with(|| Shared::new(npc::NpcDefinition::default()))
                    .clone();
                let mut w = def.write();
                if let Some(x) = d.get("name") {
                    w.name = text(x);
                }
                if let Some(x) = d.get("examine") {
                    w.examine = text(x);
                }
                if let Some(x) = d.get("configFile") {
                    w.config_file_id = int(x);
                }
                for (hk, hv) in d
                    .get("handlers")
                    .and_then(Json::as_object)
                    .into_iter()
                    .flatten()
                {
                    w.handlers.write().insert(hk.clone(), property(hv));
                }
            }
        }
    }

    /// Support.set: the case itself, a "set" operation or a hook action.
    fn set(&mut self, v: &Json) {
        if let Some(nodes) = v.get("nodes").and_then(Json::as_object) {
            for (k, n) in nodes {
                self.set_node(k, n);
            }
        }
        if let Some(interactions) = v.get("interactions").and_then(Json::as_object) {
            for (k, spec) in interactions {
                let owner = spec["owner"].as_str().unwrap().to_owned();
                let mut options = Vec::new();
                for (slot, o) in spec["options"].as_array().unwrap().iter().enumerate() {
                    if o.is_null() {
                        options.push(None);
                        continue;
                    }
                    let index = o
                        .get("index")
                        .filter(|v| !v.is_null())
                        .map_or(slot as i32, int);
                    let mut data = OptionData::new(o.get("name").and_then(text), index);
                    data.handler = o
                        .get("handler")
                        .filter(|v| !v.is_null())
                        .map(|h| if int(h) == 1 { 1 } else { 2 });
                    let handle = Shared::new(data);
                    let key = o.get("key").and_then(text).unwrap_or(format!("{k}.{slot}"));
                    self.option_keys.insert(handle.identity(), key);
                    options.push(Some(handle));
                }
                match self.interactions.get_mut(k) {
                    Some(existing) => existing.options = options,
                    None => {
                        self.interactions
                            .insert(k.clone(), Interaction { owner, options });
                    }
                }
            }
        }
        if let Some(containers) = v.get("containers").and_then(Json::as_object) {
            for (k, spec) in containers {
                let placed: Vec<(usize, Option<String>)> = spec
                    .get("slots")
                    .and_then(Json::as_object)
                    .into_iter()
                    .flatten()
                    .map(|(s, key)| {
                        let key = text(key).filter(|k| self.nodes.contains_key(k));
                        (s.parse::<usize>().unwrap(), key)
                    })
                    .collect();
                let slots = self.containers.get_mut(k).unwrap();
                if let Some(c) = spec.get("capacity") {
                    slots.resize(int(c) as usize, None);
                }
                for (s, key) in placed {
                    slots[s] = key;
                }
                if let Some(t) = spec.get("tabStart").and_then(Json::as_array) {
                    for (i, v) in t.iter().enumerate() {
                        self.tab_start[i] = int(v);
                    }
                }
            }
        }
        for (k, key) in v
            .get("players")
            .and_then(Json::as_object)
            .into_iter()
            .flatten()
        {
            self.repo_players
                .insert(k.parse().unwrap(), text(key).unwrap());
        }
        for (k, key) in v
            .get("npcs")
            .and_then(Json::as_object)
            .into_iter()
            .flatten()
        {
            self.repo_npcs
                .insert(k.parse().unwrap(), text(key).unwrap());
        }
        if let Some(x) = v.get("object") {
            self.object = text(x);
        }
        if let Some(x) = v.get("objectAt") {
            self.object_at = text(x);
        }
        if let Some(x) = v.get("ground") {
            self.ground = text(x);
        }
        for (field, map) in [("flags", 0), ("values", 1), ("answers", 2)] {
            for (k, x) in v.get(field).and_then(Json::as_object).into_iter().flatten() {
                let target = match map {
                    0 => &mut self.flags,
                    1 => &mut self.values,
                    _ => &mut self.answers,
                };
                target.insert(k.clone(), x.clone());
            }
        }
        for (k, x) in v
            .get("attributes")
            .and_then(Json::as_object)
            .into_iter()
            .flatten()
        {
            self.attributes.insert(k.clone(), x.clone());
        }
        for m in v
            .get("missing")
            .and_then(Json::as_array)
            .into_iter()
            .flatten()
        {
            self.missing.insert(text(m).unwrap());
        }
        self.set_definitions(v);
        self.max_players = self.value_int("maxPlayers", 2000);
        self.max_npcs = self.value_int("maxNpcs", 32000);
    }

    fn slots(&mut self, c: Container) -> &mut Vec<Option<String>> {
        self.containers.get_mut(c.name()).unwrap()
    }
    fn remove_from(&mut self, c: Container, item: &N) -> Result<bool> {
        self.emit(json!(["remove", c.name(), item]))?;
        let removed = self.answer_bool(&format!("remove:{}", c.name()), true);
        if removed {
            if let Some(slot) = self.slots(c).iter_mut().find(|s| s.as_ref() == Some(item)) {
                *slot = None;
            }
        }
        Ok(removed)
    }
    fn add_to(&mut self, c: Container, item: &N) -> Result<bool> {
        self.emit(json!(["add", c.name(), item]))?;
        let added = self.answer_bool(&format!("add:{}", c.name()), true);
        if added {
            if let Some(slot) = self.slots(c).iter_mut().find(|s| s.is_none()) {
                *slot = Some(item.clone());
            }
        }
        Ok(added)
    }
    fn typed(&self, key: &Option<String>, kind: NodeKind) -> Result<Option<N>> {
        match self.node(key) {
            Some(n) if self.data(&n).kind != kind => Err(Error::Cast),
            other => Ok(other),
        }
    }
    fn repository(&mut self, name: &str, index: i32) -> Result<Option<N>> {
        self.emit(json!([name, index]))?;
        let (length, map) = if name == "player" {
            (self.player_length, &self.repo_players)
        } else {
            (self.npc_length, &self.repo_npcs)
        };
        if index <= 0 || index >= length {
            return Err(Error::Index(None));
        }
        let key = map.get(&index).cloned();
        Ok(self.node(&key))
    }
}

impl Base for World {
    type Node = N;
    type Interaction = String;
    fn id(&mut self, node: &N) -> Result<i32> {
        Ok(self.data(node).id)
    }
    fn node_kind(&mut self, node: &N) -> NodeKind {
        self.data(node).kind
    }
    fn index(&mut self, node: &N) -> Result<i32> {
        Ok(self.data(node).index)
    }
    fn set_index(&mut self, node: &N, index: i32) -> Result<()> {
        self.nodes.get_mut(node).unwrap().index = index;
        Ok(())
    }
    fn active(&mut self, node: &N) -> Result<bool> {
        Ok(self.data(node).active)
    }
    fn text(&mut self, node: Option<&N>) -> Result<String> {
        Ok(match node {
            None => "null".into(),
            Some(n) => {
                let d = self.data(n);
                if d.kind == NodeKind::Item {
                    format!(
                        "Item id={}, name={}, amount={}",
                        d.id,
                        d.name.as_deref().unwrap_or("null"),
                        d.amount
                    )
                } else {
                    d.text.clone()
                }
            }
        })
    }
    fn lowercase_default(&self, text: &str) -> String {
        lowercase_root(text)
    }
    fn debug(&mut self, _: &N, text: &str) -> Result<()> {
        self.emit(json!(["debug", text]))
    }
    fn message(&mut self, _: &N, text: Option<&str>) -> Result<()> {
        self.emit(json!(["message", text]))
    }
    fn packet_message(&mut self, _: &N, text: &str) -> Result<()> {
        self.emit(json!(["packetMessage", text]))
    }
    fn api_message(&mut self, _: &N, text: &str) -> Result<()> {
        self.emit(json!(["sendMessage", text]))
    }
    fn clear_minimap(&mut self, _: &N) -> Result<()> {
        self.emit(json!(["minimap"]))
    }
    fn reset_scripts(&mut self, _: &N) -> Result<()> {
        self.emit(json!(["scripts"]))
    }
    fn interaction_locked(&mut self, _: &N) -> Result<bool> {
        self.emit(json!(["interactionLocked"]))?;
        Ok(self.flag("interactionLocked"))
    }
    fn movement_locked(&mut self, _: &N) -> Result<bool> {
        self.emit(json!(["movementLocked"]))?;
        Ok(self.flag("movementLocked"))
    }
    fn has_equipment_lock(&mut self, _: &N) -> Result<bool> {
        Ok(self.flag("equipmentLock"))
    }
    fn equipment_locked(&mut self, _: &N) -> Result<bool> {
        self.emit(json!(["equipmentLocked"]))?;
        Ok(self.flag("equipmentLocked"))
    }
    fn equipment_lock_message(&mut self, _: &N) -> Result<Option<String>> {
        Ok(self.values.get("lockMessage").and_then(text))
    }
    fn location(&mut self, node: &N) -> Result<Option<[i32; 3]>> {
        Ok(self.data(node).location)
    }
    fn interaction(&mut self, node: &N) -> Result<Option<String>> {
        self.emit(json!(["interaction", node]))?;
        Ok(self
            .data(node)
            .interaction
            .clone()
            .filter(|k| self.interactions.contains_key(k)))
    }
    fn option(&mut self, interaction: &String, index: i32) -> Result<Option<OptionHandle>> {
        self.emit(json!(["option", interaction, index]))?;
        let options = &self.interactions[interaction].options;
        if index < 0 || index as usize >= options.len() {
            return Err(index_error(index, options.len()));
        }
        Ok(options[index as usize].clone())
    }
    fn handle(&mut self, interaction: &String, _: &N, option: &OptionHandle) -> Result<()> {
        let key = self.option_key(option);
        self.emit(json!(["handle", interaction, key]))
    }
    fn invalid(&mut self, _: &N, node: &N) -> Result<()> {
        self.emit(json!(["invalid", node, "NULL"]))
    }
    fn listeners(
        &mut self,
        id: i32,
        kind: Kind,
        option: Option<&str>,
        _: &N,
        node: &N,
    ) -> Result<bool> {
        let option = option.ok_or(Error::Null)?;
        self.emit(json!(["listeners", id, kind_name(kind), option, node]))?;
        Ok(self.answer_bool("listeners", false))
    }
    fn capacity(&mut self, _: &N, container: Container) -> Result<i32> {
        Ok(self.slots(container).len() as i32)
    }
    fn get(&mut self, _: &N, container: Container, slot: i32) -> Result<Option<N>> {
        self.emit(json!(["get", container.name(), slot]))?;
        let slots = self.slots(container);
        if slot < 0 || slot as usize >= slots.len() {
            return Ok(None);
        }
        Ok(slots[slot as usize].clone())
    }
    fn replace(
        &mut self,
        _: &N,
        container: Container,
        item: Option<&N>,
        slot: i32,
        fire: bool,
    ) -> Result<Option<N>> {
        self.emit(json!(["replace", container.name(), item, slot, fire]))?;
        let slots = self.slots(container);
        if slot < 0 || slot as usize >= slots.len() {
            return Err(index_error(slot, slots.len()));
        }
        Ok(std::mem::replace(&mut slots[slot as usize], item.cloned()))
    }
    fn npc(&mut self, index: i32) -> Result<Option<N>> {
        self.repository("npc", index)
    }
    fn other_player(&mut self, index: i32) -> Result<Option<N>> {
        self.repository("player", index)
    }
    fn shown_npc(&mut self, npc: &N, _: &N) -> Result<N> {
        self.emit(json!(["shown", npc]))?;
        let shown = self.data(npc).shown.clone().or(Some(npc.clone()));
        self.typed(&shown, NodeKind::Npc)?.ok_or(Error::Null)
    }
    fn ground(&mut self, id: i32, l: [i32; 3], _: &N) -> Result<Option<N>> {
        self.emit(json!(["ground", id, l[0], l[1], l[2]]))?;
        let ground = self.ground.clone();
        self.typed(&ground, NodeKind::GroundItem)
    }
    fn max_players(&mut self) -> i32 {
        self.max_players
    }
    fn max_npcs(&mut self) -> i32 {
        self.max_npcs
    }
    fn item_definition_of(&mut self, item: &N) -> Result<item::Handle> {
        let id = self.data(item).definition;
        Ok(self.item_def(id))
    }
    fn audio(&mut self, _: &N, id: i32) -> Result<()> {
        self.emit(json!(["audio", id]))
    }
    fn on(&mut self, kind: Kind, options: &[&str], listener: Listener) -> Result<()> {
        self.listeners.push(listener);
        let n = self.listeners.len() - 1;
        self.emit(json!(["on", kind_name(kind), options, n]))
    }
}

impl actions::NpcContext for World {
    fn spawn_location(&mut self, npc: &N) -> Result<Option<[i32; 3]>> {
        Ok(self.data(npc).spawn)
    }
    fn config_file_id(&mut self, npc: &N) -> Result<i32> {
        Ok(self.data(npc).config_file)
    }
    fn plugin_npc(&mut self, _: &N, npc: &N, option: &OptionHandle) -> Result<bool> {
        let key = self.option_key(option);
        self.emit(json!(["pluginNpc", npc, key]))?;
        Ok(self.answer_bool("pluginNpc", false))
    }
}

impl item_option::Context for World {
    fn owner(&mut self, interaction: &String) -> Result<N> {
        Ok(self.interactions[interaction].owner.clone())
    }
    fn clear_pulses(&mut self, _: &N) -> Result<()> {
        self.emit(json!(["clearPulses"]))
    }
    fn submit(&mut self, task: item_option::Task<N>) -> Result<()> {
        self.pulses.push(Pulse::Item(task));
        let n = self.pulses.len() - 1;
        self.emit(json!(["submit", n]))
    }
    fn zone_interact(&mut self, _: &N, node: &N, option: &OptionHandle) -> Result<bool> {
        let key = self.option_key(option);
        self.emit(json!(["zoneInteract", node, key]))?;
        Ok(self.answer_bool("zoneInteract", false))
    }
    fn handler_handle(&mut self, handler: usize, _: &N, node: &N, option: &str) -> Result<bool> {
        self.emit(json!(["handlerHandle", handler, node, option]))?;
        Ok(self.answer_bool(&format!("handler:{handler}"), false))
    }
    fn handler_name(&mut self, handler: usize) -> Result<String> {
        Ok(if handler == 1 {
            "FirstHandler"
        } else {
            "SecondHandler"
        }
        .into())
    }
    fn log(&mut self, origin: &str, level: &str, message: &str) -> Result<()> {
        self.emit(json!(["log", origin, level, message]))
    }
}

impl use_with::Context for World {
    type Pulse = usize;
    fn object(&mut self, x: i32, y: i32, z: i32, id: i32, shape: i32) -> Result<Option<N>> {
        self.emit(json!(["object", x, y, z, id, shape]))?;
        let object = self.object.clone();
        self.typed(&object, NodeKind::Scenery)
    }
    fn scenery_child(&mut self, scenery: &N, _: &N) -> Result<Option<N>> {
        self.emit(json!(["child", scenery]))?;
        let child = self.data(scenery).child.clone();
        self.typed(&child, NodeKind::Scenery)
    }
    fn zone_use_with(&mut self, _: &N, item: &N, node: &N) -> Result<bool> {
        self.emit(json!(["zoneUseWith", item, node]))?;
        Ok(self.answer_bool("zoneUseWith", false))
    }
    fn listeners_use(&mut self, used: &N, with: &N, kind: Kind, _: &N) -> Result<bool> {
        self.emit(json!(["listenersUse", used, with, kind_name(kind)]))?;
        Ok(self.answer_bool("listenersUse", false))
    }
    fn plugin_use(&mut self, _: &N, event: &Event<N>) -> Result<bool> {
        self.emit(json!(["pluginUse", event.used, event.with]))?;
        Ok(self.answer_bool("pluginUse", false))
    }
    fn handlers(&mut self, key: i32) -> Result<Option<HandlerList>> {
        Ok(self.registry.get(key))
    }
    fn close_interface(&mut self, _: &N) -> Result<()> {
        self.emit(json!(["closeInterface"]))
    }
    fn unhandled(&mut self, _: &N) -> Result<()> {
        self.emit(json!(["unhandled"]))
    }
    fn movement(
        &mut self,
        _: &N,
        destination: &N,
        handler: Option<&UseHandler>,
        body: Body<N>,
    ) -> Result<usize> {
        self.pulses.push(Pulse::Use(body));
        let n = self.pulses.len() - 1;
        let reference = handler.map(|h| self.use_refs[h.read().handler].clone());
        self.emit(json!(["movement", destination, reference, n]))?;
        Ok(n)
    }
    fn delayed(&mut self, _: &Event<N>, body: Body<N>) -> Result<usize> {
        self.pulses.push(Pulse::Use(body));
        Ok(self.pulses.len() - 1)
    }
    fn run_pulse(&mut self, _: &N, pulse: usize) -> Result<()> {
        self.emit(json!(["run", pulse]))
    }
    fn dispatch_use(&mut self, _: &N, used: i32, with: i32) -> Result<()> {
        self.emit(json!(["dispatchUse", used, with]))
    }
    fn use_handle(&mut self, handler: &UseHandler, event: &Event<N>) -> Result<bool> {
        let reference = self.use_refs[handler.read().handler].clone();
        self.emit(json!(["useHandle", reference, event.used, event.with]))?;
        Ok(self.answer_bool(&format!("useHandle:{reference}"), false))
    }
    fn face_location(&mut self, node: &N, from: Option<[i32; 3]>) -> Result<Option<[i32; 3]>> {
        self.emit(json!(["faceLocation", node, coord(from)]))?;
        Ok(self.data(node).location)
    }
    fn face(&mut self, _: &N, location: Option<[i32; 3]>) -> Result<()> {
        self.emit(json!(["face", coord(location)]))
    }
}

impl component::Context for World {
    fn object_at(&mut self, x: i32, y: i32, z: i32) -> Result<Option<N>> {
        self.emit(json!(["objectAt", x, y, z]))?;
        let object = self.object_at.clone();
        self.typed(&object, NodeKind::Scenery)
    }
    fn attribute_int(&mut self, _: &N, key: &str, default: i32) -> Result<i32> {
        self.emit(json!(["attribute", key]))?;
        match self.attributes.get(key) {
            None => Ok(default),
            Some(Json::Number(n)) => Ok(n.as_i64().unwrap() as i32),
            Some(_) => Err(Error::Cast),
        }
    }
    fn ticks(&mut self) -> i32 {
        self.value_int("ticks", 0)
    }
    fn spell_book(&mut self, _: &N) -> Result<i32> {
        self.emit(json!(["spellBook"]))?;
        Ok(self.value_int("spellBook", 192))
    }
    fn spell_listeners(
        &mut self,
        button: i32,
        kind: i32,
        book: &str,
        _: &N,
        node: &N,
    ) -> Result<()> {
        self.emit(json!(["spellListeners", button, kind, book, node]))
    }
    fn cast_spell(&mut self, _: &N, book: SpellBook, spell: i32, target: &N) -> Result<()> {
        self.emit(json!(["castSpell", book.name(), spell, target]))
    }
    fn has_familiar(&mut self, _: &N) -> Result<bool> {
        self.emit(json!(["hasFamiliar"]))?;
        Ok(self.flag("familiar"))
    }
    fn familiar_special(
        &mut self,
        _: &N,
        target: &N,
        iface: i32,
        child: i32,
        item: Option<&N>,
    ) -> Result<()> {
        self.emit(json!(["familiarSpecial", target, iface, child, item]))
    }
}

impl slot_switch::Context for World {
    fn insert_items(&mut self, _: &N) -> Result<bool> {
        Ok(self.flag("insertItems"))
    }
    fn free_slot(&mut self, _: &N, container: Container) -> Result<i32> {
        self.emit(json!(["freeSlot", container.name()]))?;
        Ok(self
            .slots(container)
            .iter()
            .position(Option::is_none)
            .map_or(-1, |i| i as i32))
    }
    fn tab_start_slot(&mut self, _: &N, tab: i32) -> Result<i32> {
        self.tab_start
            .get(tab as usize)
            .copied()
            .ok_or_else(|| index_error(tab, 11))
    }
    fn items_in_tab(&mut self, _: &N, tab: i32) -> Result<i32> {
        self.emit(json!(["itemsInTab", tab]))?;
        Ok(self.answer_int("itemsInTab", 0))
    }
    fn remove(&mut self, _: &N, container: Container, item: &N) -> Result<bool> {
        self.remove_from(container, item)
    }
    fn add(&mut self, _: &N, container: Container, item: &N) -> Result<bool> {
        self.add_to(container, item)
    }
    fn tab_by_item_slot(&mut self, _: &N, slot: i32) -> Result<i32> {
        self.emit(json!(["tabByItemSlot", slot]))?;
        Ok(self.answer_int("tabByItemSlot", 0))
    }
    fn insert(
        &mut self,
        _: &N,
        container: Container,
        from: i32,
        to: i32,
        update: bool,
    ) -> Result<()> {
        self.emit(json!(["insert", container.name(), from, to, update]))?;
        let items = self.slots(container);
        let length = items.len();
        let at = |i: i32| -> Result<usize> {
            if i < 0 || i as usize >= length {
                Err(index_error(i, length))
            } else {
                Ok(i as usize)
            }
        };
        let temp = items[at(from)?].clone();
        if to > from {
            for i in from..to {
                let next = items[at(i + 1)?].clone();
                items[at(i)?] = next;
            }
        } else if from > to {
            let mut i = from;
            while i > to {
                let previous = items[at(i - 1)?].clone();
                items[at(i)?] = previous;
                i -= 1;
            }
        }
        items[at(to)?] = temp;
        Ok(())
    }
    fn increase_tab_start_slots(&mut self, _: &N, tab: i32) -> Result<()> {
        self.emit(json!(["increaseTabStartSlots", tab]))
    }
    fn decrease_tab_start_slots(&mut self, _: &N, tab: i32) -> Result<()> {
        self.emit(json!(["decreaseTabStartSlots", tab]))
    }
    fn set_tab_configurations(&mut self, _: &N) -> Result<()> {
        self.emit(json!(["setTabConfigurations"]))
    }
    fn has_chatbox(&mut self, _: &N) -> Result<bool> {
        self.emit(json!(["hasChatbox"]))?;
        Ok(self.flag("chatbox"))
    }
    fn attribute_bool(&mut self, _: &N, key: &str, default: bool) -> Result<bool> {
        self.emit(json!(["attribute", key]))?;
        match self.attributes.get(key) {
            None => Ok(default),
            Some(Json::Bool(b)) => Ok(*b),
            Some(_) => Err(Error::Cast),
        }
    }
    fn close_chatbox(&mut self, _: &N) -> Result<()> {
        self.emit(json!(["closeChatbox"]))?;
        self.flags.insert("chatbox".into(), Json::Bool(false));
        Ok(())
    }
    fn refresh(&mut self, _: &N, container: Container) -> Result<()> {
        self.emit(json!(["refresh", container.name()]))
    }
}

impl examine::Context for World {
    fn item_definition(&mut self, id: i32) -> Result<Option<item::Handle>> {
        self.emit(json!(["itemDefinition", id]))?;
        Ok((!self.missing.contains(&format!("item:{id}"))).then(|| self.item_def(id)))
    }
    fn scenery_definition(&mut self, id: i32) -> Result<Option<scenery::Handle>> {
        self.emit(json!(["sceneryDefinition", id]))?;
        if self.missing.contains(&format!("scenery:{id}")) {
            return Ok(None);
        }
        Ok(Some(
            self.scenery_defs
                .entry(id)
                .or_insert_with(|| Shared::new(scenery::SceneryDefinition::default()))
                .clone(),
        ))
    }
    fn npc_definition(&mut self, id: i32) -> Result<Option<npc::Handle>> {
        self.emit(json!(["npcDefinition", id]))?;
        if self.missing.contains(&format!("npc:{id}")) {
            return Ok(None);
        }
        Ok(Some(
            self.npc_defs
                .entry(id)
                .or_insert_with(|| Shared::new(npc::NpcDefinition::default()))
                .clone(),
        ))
    }
    fn varbit(&mut self, id: i32) -> Result<Option<i32>> {
        self.emit(json!(["varbit", id]))?;
        let found = self.answer("varbit", json!(id));
        Ok((!found.is_null()).then(|| int(&found)))
    }
    fn music_looping(&mut self, _: &N) -> Result<bool> {
        Ok(self.flag("looping"))
    }
    fn music_replay(&mut self, _: &N) -> Result<()> {
        self.emit(json!(["replay"]))
    }
    fn music_set_playing(&mut self, _: &N, playing: bool) -> Result<()> {
        self.emit(json!(["setPlaying", playing]))?;
        self.flags.insert("playing".into(), Json::Bool(playing));
        Ok(())
    }
}

impl equip::Context for World {
    fn name(&mut self, node: &N) -> Result<Option<String>> {
        Ok(self.data(node).name.clone())
    }
    fn amount(&mut self, item: &N) -> Result<i32> {
        Ok(self.data(item).amount)
    }
    fn equip_listeners(&mut self, id: i32, _: &N, node: &N, equip: bool) -> Result<bool> {
        self.emit(json!(["listenersEquip", id, node, equip]))?;
        Ok(self.answer_bool(&format!("listenersEquip:{equip}"), true))
    }
    fn fire_event(
        &mut self,
        plugin: &plugin::Handle,
        name: &str,
        _: &N,
        item: &N,
    ) -> Result<Fired> {
        let label = plugin.label().to_owned();
        self.emit(json!(["fireEvent", label, name, item]))?;
        Ok(
            match self.answer(&format!("fireEvent:{label}"), Json::Null) {
                Json::Null => Fired::Null,
                Json::Bool(b) => Fired::Bool(b),
                _ => Fired::Other,
            },
        )
    }
    fn equipment_add(
        &mut self,
        _: &N,
        item: &N,
        slot: i32,
        fire: bool,
        from: bool,
    ) -> Result<bool> {
        self.emit(json!(["equipAdd", item, slot, fire, from]))?;
        Ok(self.answer_bool("equipAdd", true))
    }
    fn equipment_remove(&mut self, _: &N, item: &N) -> Result<bool> {
        self.remove_from(Container::Equipment, item)
    }
    fn inventory_add(&mut self, _: &N, item: &N) -> Result<()> {
        self.add_to(Container::Inventory, item).map(|_| ())
    }
    fn maximum_add(&mut self, _: &N, item: &N) -> Result<i32> {
        self.emit(json!(["maximumAdd", item]))?;
        Ok(self.answer_int("maximumAdd", 28))
    }
    fn register_glove(&mut self, _: &N, id: i32) -> Result<()> {
        self.emit(json!(["registerGlove", id]))
    }
    fn dialogue_close(&mut self, _: &N) -> Result<()> {
        self.emit(json!(["dialogueClose"]))
    }
    fn has_autocast(&mut self, _: &N) -> Result<bool> {
        Ok(self.flag("autocast"))
    }
    fn clear_autocast(&mut self, _: &N) -> Result<()> {
        self.emit(json!(["clearAutocast"]))?;
        self.flags.insert("autocast".into(), Json::Bool(false));
        Ok(())
    }
    fn has_weapon_interface(&mut self, _: &N) -> Result<bool> {
        Ok(!self.flag("noWeaponInterface"))
    }
    fn select_auto_spell(&mut self, _: &N, button: i32, adjust: bool, forget: bool) -> Result<()> {
        self.emit(json!(["selectAutoSpell", button, adjust, forget]))
    }
    fn open_autocast_select(&mut self, _: &N) -> Result<()> {
        self.emit(json!(["openAutocastSelect"]))
    }
    fn is_slayer_equipment(&mut self, id: i32) -> Result<bool> {
        self.emit(json!(["isSlayerEq", id]))?;
        Ok(self.answer_bool("isSlayerEq", false))
    }
    fn update_slayer_flags(&mut self, _: &N) -> Result<()> {
        self.emit(json!(["updateSlayerFlags"]))
    }
    fn dispatch_equip(&mut self, _: &N, id: i32, slot: i32) -> Result<()> {
        self.emit(json!(["dispatchEquip", id, slot]))
    }
    fn dispatch_unequip(&mut self, _: &N, id: i32, slot: i32) -> Result<()> {
        self.emit(json!(["dispatchUnequip", id, slot]))
    }
    fn send_string(&mut self, _: &N, text: &str, iface: i32, child: i32) -> Result<()> {
        self.emit(json!(["sendString", text, iface, child]))
    }
}

impl drop::Context for World {
    fn used_option(&mut self, _: &N) -> Result<String> {
        self.emit(json!(["usedOption"]))?;
        Ok(self
            .values
            .get("usedOption")
            .and_then(text)
            .unwrap_or_else(|| "INVALID".into()))
    }
    fn is_pet(&mut self, id: i32) -> Result<bool> {
        self.emit(json!(["pet", id]))?;
        Ok(self.answer_bool("pet", false))
    }
    fn summon(&mut self, _: &N, item: &N, pet: bool, deposit: bool) -> Result<()> {
        self.emit(json!(["summon", item, pet, deposit]))
    }
    fn has_grave_at(&mut self, location: Option<[i32; 3]>) -> Result<bool> {
        self.emit(json!(["grave", coord(location)]))?;
        Ok(self.answer_bool("grave", false))
    }
    fn close_all_interfaces(&mut self, _: &N) -> Result<()> {
        self.emit(json!(["closeAll"]))
    }
    fn queue_soft_script(&mut self, _: &N, script: DropScript<N>) -> Result<()> {
        self.scripts.push(script);
        let n = self.scripts.len() - 1;
        self.emit(json!(["queueScript", "SOFT", n]))
    }
    fn stop_executing(&mut self, _: &N) -> Result<bool> {
        self.emit(json!(["stopExecuting"]))?;
        Ok(self.answer_bool("stopExecuting", true))
    }
    fn dupe_alert(&mut self, _: &N, text: &str) -> Result<()> {
        self.emit(json!(["monitor", "DUPE_ALERT", text]))
    }
    fn player_name(&mut self, player: &N) -> Result<Option<String>> {
        Ok(self.data(player).name.clone())
    }
    fn amount(&mut self, item: &N) -> Result<i32> {
        Ok(self.data(item).amount)
    }
    fn name(&mut self, item: &N) -> Result<Option<String>> {
        Ok(self.data(item).name.clone())
    }
    fn drop_item(&mut self, item: &N) -> Result<N> {
        self.emit(json!(["dropItem", item]))?;
        let drop = self.data(item).drop.clone().or(Some(item.clone()));
        let dropped = self.node(&drop).ok_or(Error::Null)?;
        if !self.data(&dropped).kind.is_item() {
            return Err(Error::Cast);
        }
        Ok(dropped)
    }
    fn ground_create(&mut self, item: &N, location: Option<[i32; 3]>, _: &N) -> Result<()> {
        self.emit(json!(["groundCreate", item, coord(location)]))
    }
    fn world_ticks(&mut self) -> i32 {
        self.value_int("ticks", 0)
    }
    fn set_attribute(&mut self, _: &N, key: &str, value: i32) -> Result<()> {
        self.emit(json!(["setAttribute", key, value]))
    }
    fn save(&mut self, _: &N) -> Result<()> {
        self.emit(json!(["save"]))
    }
    fn send_destroy_item(&mut self, _: &N, id: i32, name: Option<&str>) -> Result<()> {
        self.emit(json!(["destroyDialogue", id, name]))
    }
    fn add_dialogue_action(&mut self, _: &N, action: DestroyAction<N>) -> Result<()> {
        self.actions.push(action);
        let n = self.actions.len() - 1;
        self.emit(json!(["addDialogueAction", n]))
    }
    fn remove_item(&mut self, _: &N, item: &N) -> Result<bool> {
        self.emit(json!(["removeItem", item]))?;
        Ok(self.answer_bool("removeItem", true))
    }
}

fn error_label(e: &Error) -> &'static str {
    match e {
        Error::Null => "Null",
        Error::Index(_) => "Index",
        Error::Cast => "Cast",
        Error::Binding(_) => "Binding",
    }
}

fn i(op: &Json, key: &str) -> i32 {
    int(&op[key])
}

/// One operation of a case, as Support.run does it.
fn run(w: &mut World, op: &Json) -> Result<Json> {
    let p = "p".to_owned();
    let name = op["op"].as_str().unwrap();
    let none = Json::String("none".into());
    match name {
        "set" => w.set(op),
        "item_action" => actions::item_action(
            w,
            &p,
            ItemAction {
                opt_index: i(op, "opt"),
                item_id: i(op, "item"),
                slot: i(op, "slot"),
                iface: i(op, "iface"),
                child: i(op, "child"),
            },
        )?,
        "npc_action" => actions::npc_action(
            w,
            &p,
            NpcAction {
                opt_index: i(op, "opt"),
                npc_index: i(op, "npc"),
            },
        )?,
        "player_action" => actions::player_action(
            w,
            &p,
            PlayerAction {
                opt_index: i(op, "opt"),
                other_index: i(op, "other"),
            },
        )?,
        "use_npc" => use_with::process(
            w,
            &p,
            UseWith::Npc {
                item_id: i(op, "item"),
                npc_index: i(op, "npc"),
                iface: i(op, "iface"),
                slot: i(op, "slot"),
            },
        )?,
        "use_player" => use_with::process(
            w,
            &p,
            UseWith::Player {
                item_id: i(op, "item"),
                other_index: i(op, "other"),
                iface: i(op, "iface"),
                slot: i(op, "slot"),
            },
        )?,
        "use_item" => use_with::process(
            w,
            &p,
            UseWith::Item {
                used_id: i(op, "used"),
                used_with_id: i(op, "with"),
                used_slot: i(op, "usedSlot"),
                used_with_slot: i(op, "withSlot"),
                used_iface: i(op, "usedIface"),
                used_with_iface: i(op, "withIface"),
                used_child: 0,
                used_with_child: 0,
            },
        )?,
        "use_scenery" => use_with::process(
            w,
            &p,
            UseWith::Scenery {
                item_id: i(op, "item"),
                slot: i(op, "slot"),
                scenery_id: i(op, "scenery"),
                x: i(op, "x"),
                y: i(op, "y"),
            },
        )?,
        "use_ground" => use_with::process(
            w,
            &p,
            UseWith::GroundItem {
                used_id: i(op, "used"),
                with_id: i(op, "with"),
                iface: i(op, "iface"),
                child: 0,
                slot: i(op, "slot"),
                x: i(op, "x"),
                y: i(op, "y"),
            },
        )?,
        "comp_ground" => component::process(
            w,
            &p,
            ComponentAction::GroundItem {
                iface: i(op, "iface"),
                child: i(op, "child"),
                slot: 0,
                item_id: i(op, "item"),
                x: i(op, "x"),
                y: i(op, "y"),
            },
        )?,
        "comp_player" => component::process(
            w,
            &p,
            ComponentAction::Player {
                other_index: i(op, "other"),
                iface: i(op, "iface"),
                child: i(op, "child"),
            },
        )?,
        "comp_scenery" => component::process(
            w,
            &p,
            ComponentAction::Scenery {
                iface: i(op, "iface"),
                child: i(op, "child"),
                scenery_id: i(op, "scenery"),
                x: i(op, "x"),
                y: i(op, "y"),
            },
        )?,
        "comp_npc" => component::process(
            w,
            &p,
            ComponentAction::Npc {
                iface: i(op, "iface"),
                child: i(op, "child"),
                npc_index: i(op, "npc"),
            },
        )?,
        "comp_item" => component::process(
            w,
            &p,
            ComponentAction::Item {
                iface: i(op, "iface"),
                child: i(op, "child"),
                item_id: i(op, "item"),
                slot: i(op, "slot"),
            },
        )?,
        "switch_multi" => slot_switch::process(
            w,
            &p,
            SlotSwitch::Multi {
                source_iface: i(op, "srcIface"),
                source_child: 0,
                source_slot: i(op, "src"),
                dest_iface: i(op, "dstIface"),
                dest_child: i(op, "dstChild"),
                dest_slot: i(op, "dst"),
            },
        )?,
        "switch_single" => slot_switch::process(
            w,
            &p,
            SlotSwitch::Single {
                iface: i(op, "iface"),
                child: 0,
                source_slot: i(op, "src"),
                dest_slot: i(op, "dst"),
                is_insert: op["insert"] == Json::Bool(true),
            },
        )?,
        "examine_item" => examine::item_examine(w, &p, i(op, "id"))?,
        "examine_scenery" => examine::scenery_examine(w, &p, i(op, "id"))?,
        "examine_npc" => examine::npc_examine(w, &p, i(op, "id"))?,
        "track" => examine::track_finished(w, &p)?,
        "equip" => {
            let node = op["node"].as_str().unwrap().to_owned();
            equip::handle_equip(w, &p, &node)?;
        }
        "unequip" => equip::unequip(w, &p, i(op, "slot"), i(op, "item"))?,
        "equip_define" => equip::define(w)?,
        "drop_define" => drop::define(w)?,
        "drop" => {
            let node = op["node"].as_str().unwrap().to_owned();
            return Ok(json!(drop::drop(w, &p, &node)?));
        }
        "invoke" => {
            let n = i(op, "n") as usize;
            let node = op["node"].as_str().unwrap().to_owned();
            return match w.listeners.get(n).copied() {
                None => Ok(none),
                Some(Listener::Equip) => {
                    equip::handle_equip(w, &p, &node)?;
                    Ok(json!(true))
                }
                Some(Listener::Drop) => Ok(json!(drop::handle_drop_action(w, &p, &node)?)),
            };
        }
        "use_run" => {
            let event = Event {
                player: p.clone(),
                used: op["used"].as_str().unwrap().to_owned(),
                with: op["with"].as_str().unwrap().to_owned(),
            };
            use_with::run(w, &event)?;
        }
        "use_register" => {
            let reference = op["ref"].as_str().unwrap().to_owned();
            let handler = match w.use_handlers.get(&reference) {
                Some(h) => h.clone(),
                None => {
                    let h = Shared::new(HandlerData {
                        name: op["class"].as_str().unwrap().to_owned(),
                        allowed_nodes: op["allowed"]
                            .as_array()
                            .map(|a| a.iter().map(int).collect()),
                        dynamic: op["dynamic"] == Json::Bool(true),
                        handler: w.use_refs.len(),
                    });
                    w.use_refs.push(reference.clone());
                    w.use_handlers.insert(reference, h.clone());
                    h
                }
            };
            w.registry.add_handler(i(op, "id"), i(op, "type"), &handler);
        }
        "pulse" => {
            let n = i(op, "n") as usize;
            let body = match w.pulses.get(n) {
                None => return Ok(none),
                Some(Pulse::Item(task)) => Err(task.clone()),
                Some(Pulse::Use(body)) => Ok(body.clone()),
            };
            return Ok(json!(match body {
                Err(task) => item_option::pulse(w, &task)?,
                Ok(body) => use_with::pulse(w, &body)?,
            }));
        }
        "script" => {
            let Some(script) = w.scripts.get(i(op, "n") as usize).cloned() else {
                return Ok(none);
            };
            return Ok(json!(drop::script(w, &script)?));
        }
        "dialogue" => {
            let Some(action) = w.actions.get(i(op, "n") as usize).cloned() else {
                return Ok(none);
            };
            drop::destroy_action(w, &action, &p, i(op, "button"))?;
        }
        other => panic!("unknown operation {other}"),
    }
    Ok(Json::Null)
}

fn snapshot(w: &World, result: Json, error: Option<&str>) -> Json {
    let containers: Map<String, Json> = w
        .containers
        .iter()
        .map(|(k, v)| (k.clone(), json!(v)))
        .collect();
    let indices: Map<String, Json> = w
        .nodes
        .iter()
        .map(|(k, n)| (k.clone(), json!(n.index)))
        .collect();
    let mut examines = Map::new();
    for (id, d) in &w.item_defs {
        examines.insert(format!("item:{id}"), json!(d.read().examine));
    }
    for (id, d) in &w.scenery_defs {
        examines.insert(format!("scenery:{id}"), json!(d.read().examine));
    }
    let allowed: Map<String, Json> = w
        .use_handlers
        .iter()
        .map(|(k, h)| (k.clone(), json!(h.read().allowed_nodes)))
        .collect();
    json!({
        "error": error,
        "result": result,
        "events": w.events,
        "containers": containers,
        "indices": indices,
        "examines": examines,
        "allowed": allowed,
        "chatbox": w.flag("chatbox"),
        "autocast": w.flag("autocast"),
    })
}

fn replay(case: &Json) -> Vec<Json> {
    let mut w = World::new(case);
    let mut states = Vec::new();
    for op in case["ops"].as_array().unwrap() {
        w.events.clear();
        w.op_calls = 0;
        w.fail_at = op.get("failAt").map_or(-1, |v| v.as_i64().unwrap());
        let (result, error) = match run(&mut w, op) {
            Ok(v) => (v, None),
            Err(e) => (Json::Null, Some(error_label(&e))),
        };
        states.push(snapshot(&w, result, error));
    }
    states
}

#[test]
fn provenance_pins_the_corpus() {
    let provenance: Json = serde_json::from_str(PROVENANCE).unwrap();
    let digest = format!("{:x}", Sha256::digest(CORPUS.as_bytes()));
    assert_eq!(provenance["fixtureSha256"], json!(digest));
    assert_eq!(
        provenance["caseCount"],
        json!(CORPUS.lines().count()),
        "case count"
    );
}

#[test]
fn original_packet_handlers_match_the_native_port() {
    let mut mismatches = Vec::new();
    for (number, line) in CORPUS.lines().enumerate() {
        let row: Json = serde_json::from_str(line).unwrap();
        let actual = replay(&row);
        let expected = row["expected"].as_array().unwrap();
        for (step, (a, e)) in actual.iter().zip(expected).enumerate() {
            if a != e {
                let fields: Vec<&String> = e
                    .as_object()
                    .unwrap()
                    .keys()
                    .filter(|k| a.get(k.as_str()) != e.get(k.as_str()))
                    .collect();
                let show = |v: &Json| json!({"error": v["error"], "result": v["result"], "events": v["events"]});
                mismatches.push(format!(
                    "case {number} op {step} {} differs in {fields:?}:\n  native   {}\n  original {}",
                    row["ops"][step],
                    show(a),
                    show(e)
                ));
                break;
            }
        }
    }
    assert!(
        mismatches.is_empty(),
        "{} of {} cases differ:\n{}",
        mismatches.len(),
        CORPUS.lines().count(),
        mismatches[..mismatches.len().min(8)].join("\n")
    );
}

/// The driver routes every packet type of this slice to a bound handler,
/// and each decoded `ClientAction` converts to its library's request.
#[test]
fn routed_packets_reach_the_ported_branches() {
    use rs09_protocol::game::actions::ClientAction as A;
    use rs09_server::world::driver::dispatch::{self, Handler, Route};
    let routes = [
        ("ItemAction", Handler::ItemAction),
        ("NpcAction", Handler::NpcAction),
        ("PlayerAction", Handler::PlayerAction),
        ("UseWithNpc", Handler::UseWith),
        ("UseWithPlayer", Handler::UseWith),
        ("UseWithItem", Handler::UseWith),
        ("UseWithScenery", Handler::UseWith),
        ("UseWithGroundItem", Handler::UseWith),
        ("ComponentItemAction", Handler::ComponentUseWith),
        ("ComponentPlayerAction", Handler::ComponentUseWith),
        ("ComponentNpcAction", Handler::ComponentUseWith),
        ("ComponentSceneryAction", Handler::ComponentUseWith),
        ("ComponentGroundItemAction", Handler::ComponentUseWith),
        ("SlotSwitchSingleComponent", Handler::SlotSwitch),
        ("SlotSwitchMultiComponent", Handler::SlotSwitch),
        ("ItemExamine", Handler::Examine),
        ("SceneryExamine", Handler::Examine),
        ("NpcExamine", Handler::Examine),
        ("TrackFinished", Handler::TrackFinished),
    ];
    for (class, handler) in routes {
        assert_eq!(dispatch::route(class), Route::Native(handler), "{class}");
    }
    let item = A::ItemAction {
        opt_index: 1,
        item_id: 1277,
        slot: 3,
        iface: 149,
        child: 0,
    };
    assert_eq!(
        ItemAction::from_action(&item),
        Some(ItemAction {
            opt_index: 1,
            item_id: 1277,
            slot: 3,
            iface: 149,
            child: 0
        })
    );
    assert_eq!(ItemAction::from_action(&A::Ping), None);
    assert_eq!(
        NpcAction::from_action(&A::NpcAction {
            opt_index: 2,
            npc_index: 9
        }),
        Some(NpcAction {
            opt_index: 2,
            npc_index: 9
        })
    );
    assert_eq!(
        PlayerAction::from_action(&A::PlayerAction {
            opt_index: 3,
            other_index: 4
        }),
        Some(PlayerAction {
            opt_index: 3,
            other_index: 4
        })
    );
    let used = A::UseWithItem {
        used_id: 1,
        used_with_id: 2,
        used_slot: 3,
        used_with_slot: 4,
        used_iface: 149,
        used_with_iface: 762,
        used_child: 0,
        used_with_child: 73,
    };
    assert!(matches!(
        UseWith::from_action(&used),
        Some(UseWith::Item {
            used_with_iface: 762,
            used_with_child: 73,
            ..
        })
    ));
    assert!(matches!(
        ComponentAction::from_action(&A::ComponentNpcAction {
            iface: 192,
            child: 1,
            npc_index: 5
        }),
        Some(ComponentAction::Npc { npc_index: 5, .. })
    ));
    assert!(matches!(
        SlotSwitch::from_action(&A::SlotSwitchSingleComponent {
            iface: 149,
            child: 0,
            source_slot: 1,
            dest_slot: 2,
            is_insert: true
        }),
        Some(SlotSwitch::Single {
            is_insert: true,
            ..
        })
    ));
}

/// `TrackFinished` on the native MusicPlayer: looping replays the current
/// track through the music packet; otherwise only `playing` is cleared.
#[test]
fn track_finished_drives_the_native_music_player() {
    use rs09_server::player::persisted::music::{Player as Unlocked, Song, Songs};
    use rs09_server::world::player_tabs::{
        self,
        emotes::Emote,
        music::{Host, MusicPlayer, Playback},
    };
    #[derive(Default)]
    struct Calls(Vec<String>);
    impl Host for Calls {
        fn varp(&mut self, id: i32, value: i32, _: bool) -> player_tabs::Result<()> {
            self.0.push(format!("varp {id} {value}"));
            Ok(())
        }
        fn access_mask(
            &mut self,
            _: i32,
            _: i32,
            _: i32,
            _: i32,
            _: i32,
        ) -> player_tabs::Result<()> {
            self.0.push("mask".into());
            Ok(())
        }
        fn message(&mut self, text: &str) -> player_tabs::Result<()> {
            self.0.push(format!("message {text}"));
            Ok(())
        }
        fn music(&mut self, id: i32, secondary: bool) -> player_tabs::Result<()> {
            self.0.push(format!("music {id} {secondary}"));
            Ok(())
        }
        fn send_string(
            &mut self,
            text: &str,
            interface: i32,
            child: i32,
        ) -> player_tabs::Result<()> {
            self.0.push(format!("string {text} {interface} {child}"));
            Ok(())
        }
        fn is_debug(&mut self) -> player_tabs::Result<bool> {
            Ok(false)
        }
        fn ticks(&mut self) -> player_tabs::Result<i32> {
            Ok(0)
        }
        fn emote_unlocked(&mut self, _: Emote) -> player_tabs::Result<bool> {
            Ok(false)
        }
        fn emote_unlock(&mut self, _: Emote) -> player_tabs::Result<bool> {
            Ok(false)
        }
        fn emote_lock(&mut self, _: Emote) -> player_tabs::Result<bool> {
            Ok(false)
        }
    }
    let songs = Songs {
        songs: BTreeMap::from([(
            76,
            Song {
                name: "Harmony".into(),
                index: 3,
            },
        )]),
    };
    for looping in [true, false] {
        let mut unlocked = Unlocked::default();
        let mut playback = Playback {
            current_music_id: 76,
            playing: true,
            looping,
        };
        let mut calls = Calls::default();
        let mut music = MusicPlayer {
            unlocked: &mut unlocked,
            playback: &mut playback,
            songs: &songs,
        };
        examine::track_finished_native(&mut music, &mut calls).unwrap();
        if looping {
            assert_eq!(calls.0, ["music 76 false", "string Harmony 187 14"]);
            assert!(playback.playing);
        } else {
            assert!(calls.0.is_empty());
            assert!(!playback.playing);
        }
    }
}

/// The examine texts read and cache the native definitions' own fields.
#[test]
fn examine_texts_use_the_native_definitions() {
    let lower = |s: &str| lowercase_root(s);
    let def = Shared::new(item::ItemDefinition {
        name: Some("Apple pie".into()),
        ..Default::default()
    });
    assert_eq!(
        examine::item_examine_text(&def, lower).unwrap(),
        "It's an Apple pie."
    );
    assert_eq!(def.read().examine.as_deref(), Some("It's an Apple pie."));
    def.write().unnoted = false;
    assert_eq!(
        examine::item_examine_text(&def, lower).unwrap(),
        examine::NOTE_EXAMINE
    );
    // The note text stays cached after the item is unnoted again.
    def.write().unnoted = true;
    assert_eq!(
        examine::item_examine_text(&def, lower).unwrap(),
        examine::NOTE_EXAMINE
    );
    let npc = Shared::new(npc::NpcDefinition {
        name: Some("Hans".into()),
        ..Default::default()
    });
    assert_eq!(
        examine::npc_examine_text(&npc, lower).unwrap().as_deref(),
        // isPlusN: an "h" not followed by "e" takes "an".
        Some("It's an Hans.")
    );
    npc.write().name = Some("H".into());
    assert_eq!(examine::npc_examine_text(&npc, lower).unwrap(), None);
}
