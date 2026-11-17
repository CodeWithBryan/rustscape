//! Replays the NPC class corpus (fixtures/npc-classes) through
//! `world::npc_classes`. Every service answers from the case the way the
//! oracle's recording adapters do (oracle/npc-classes/adapters.py and
//! Support.java); every recorded call (registrations and NPC construction
//! included), result, error, the attributes and the NPCs' class, id, walks,
//! aggression, direction and idle state after each operation are compared.
use rs09_server::world::npc_classes::{
    self as nc, Abstract, Arg, Class, CombatStyle, Constructor, Diary, Direction, Error, Factory,
    Host, InteractionType, Kind, LogLevel, Result, Spell, Value, banker, classes, idle,
    listeners::{self, Destination, Interaction, Listener, OptionPlugin, Plugin, Registrar},
    registry::{self, Registry},
};
use serde_json::{Map, Value as Json, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};

const CORPUS: &str = include_str!("../../../fixtures/npc-classes/oracle.jsonl");
const PROVENANCE: &str = include_str!("../../../fixtures/npc-classes/provenance.json");

/// The clock the world answers (the oracle records the original's own clock
/// relative to itself, in whole seconds).
const NOW: i64 = 1_700_000_000_000;

type N = usize;

/// An AbstractNPC class of the corpus: one of the port's, or one of the
/// oracle's two probes (a class claiming ids another holds, and a class of
/// another package whose simple name is ShopNPC).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TestClass {
    Real(Class),
    ConflictProbe,
    ShopNamed,
}
impl Abstract for TestClass {
    fn name(&self) -> &'static str {
        match self {
            Self::Real(c) => c.name(),
            Self::ConflictProbe => "oracle.ConflictProbe",
            Self::ShopNamed => "oracle.probe.ShopNPC",
        }
    }
    fn ids(&self) -> Vec<i32> {
        match self {
            Self::Real(c) => Abstract::ids(c),
            Self::ConflictProbe => vec![520, 4999999, 46],
            Self::ShopNamed => vec![4999998, 520],
        }
    }
    fn prototype(&self) -> (i32, Constructor) {
        match self {
            Self::Real(c) => c.prototype(),
            _ => (
                0,
                Constructor {
                    walks: true,
                    aggressive: None,
                },
            ),
        }
    }
    fn construct(&self, location: Option<[i32; 3]>) -> Result<Option<Constructor>> {
        match self {
            Self::Real(c) => c.construct(location),
            Self::ConflictProbe => Ok(Some(Constructor {
                walks: true,
                aggressive: None,
            })),
            Self::ShopNamed => Ok(None),
        }
    }
}

#[derive(Clone, Debug)]
struct NodeData {
    key: Option<String>,
    kind: Kind,
    class_name: String,
    class: Option<Class>,
    id: i32,
    location: Option<[i32; 3]>,
    direction: Option<Direction>,
    walks: bool,
    aggressive: bool,
    radius: i32,
    idle: Option<idle::Idle>,
    attrs: BTreeMap<String, Value>,
}

/// A labelled object, by identity.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum Obj {
    Node(N),
    Function(usize),
    Handler(usize),
    Monk,
}

#[derive(Clone, Debug)]
struct Handler {
    reference: &'static str,
    instance: usize,
}

struct World {
    nodes: Vec<NodeData>,
    by_key: BTreeMap<String, N>,
    values: Map<String, Json>,
    answers: Map<String, Json>,
    answer_calls: HashMap<String, usize>,
    second_bank: bool,
    labels: Vec<(Obj, String)>,
    interactions: Vec<Interaction>,
    destinations: Vec<Destination>,
    handlers: BTreeMap<String, OptionPlugin>,
    handler_objects: usize,
    events: Vec<Json>,
    op_calls: i64,
    fail_at: i64,
}

fn int(v: &Json) -> i32 {
    v.as_i64().unwrap() as i32
}
fn location(v: &Json) -> Option<[i32; 3]> {
    v.as_array()
        .map(|a| nc::location(int(&a[0]), int(&a[1]), int(&a[2])))
}
fn coord(l: Option<[i32; 3]>) -> Json {
    l.map_or(Json::Null, |l| json!(l))
}
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
            } else if let Some(s) = o.get("clockPlusSeconds") {
                Value::Long(NOW + s.as_i64().unwrap() * 1000)
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
        Value::Long(l) if (l - NOW).abs() < 86_400_000 => {
            json!({ "clockPlusSeconds": ((l - NOW) as f64 / 1000.0).round() as i64 })
        }
        Value::Long(l) => json!({ "long": l }),
        Value::Bool(b) => json!(b),
        Value::Text(t) => json!(t),
        Value::Location(l) => json!({ "location": l }),
    }
}
fn kind_named(kind: &str) -> (Kind, &'static str) {
    match kind {
        "player" => (Kind::Player, "core.game.node.entity.player.Player"),
        "npc" => (Kind::Npc, "core.game.node.entity.npc.NPC"),
        "randomNpc" => (Kind::RandomEventNpc, "content.global.ame.RandomEventNPC"),
        "holidayNpc" => (
            Kind::HolidayRandomEventNpc,
            "core.game.worldevents.holiday.HolidayRandomEventNPC",
        ),
        "idle" => (Kind::Idle, "oracle.IdleProbe"),
        "scenery" => (Kind::Scenery, "core.game.node.scenery.Scenery"),
        "item" => (Kind::Item, "core.game.node.item.Item"),
        other => panic!("{other}"),
    }
}

impl World {
    fn new(case: &Json) -> Self {
        assert!(
            case.get("hooks").is_none(),
            "hooks are not used by this corpus"
        );
        let mut w = Self {
            nodes: Vec::new(),
            by_key: BTreeMap::new(),
            values: Map::new(),
            answers: Map::new(),
            answer_calls: HashMap::new(),
            second_bank: true,
            labels: Vec::new(),
            interactions: Vec::new(),
            destinations: Vec::new(),
            handlers: BTreeMap::new(),
            handler_objects: 0,
            events: Vec::new(),
            op_calls: 0,
            fail_at: -1,
        };
        w.set(&json!({"nodes": {"p": {"kind": "player", "location": [3222, 3218, 0]}}}));
        w.set(case);
        w
    }

    fn set(&mut self, v: &Json) {
        if let Some(ns) = v.get("nodes").and_then(Json::as_object) {
            for (k, spec) in ns {
                let n = match self.by_key.get(k) {
                    Some(n) => *n,
                    None => {
                        let (kind, class_name) = kind_named(spec["kind"].as_str().unwrap());
                        // NPC(): no id, no direction; NPC(id): direction south;
                        // IdleProbe: AbstractNPC's walks.
                        let constructed = matches!(
                            kind,
                            Kind::RandomEventNpc | Kind::HolidayRandomEventNpc | Kind::Idle
                        );
                        let data = NodeData {
                            key: Some(k.clone()),
                            kind,
                            class_name: class_name.into(),
                            class: None,
                            id: 0,
                            location: None,
                            direction: constructed.then_some(Direction::South),
                            walks: kind == Kind::Idle,
                            aggressive: false,
                            radius: 11,
                            idle: (kind == Kind::Idle)
                                .then(|| idle::Idle::new(int(&spec["activeId"]))),
                            attrs: BTreeMap::new(),
                        };
                        self.nodes.push(data);
                        let n = self.nodes.len() - 1;
                        self.by_key.insert(k.clone(), n);
                        n
                    }
                };
                let node = &mut self.nodes[n];
                if let Some(id) = spec.get("id") {
                    node.id = int(id);
                }
                if let Some(l) = spec.get("location") {
                    node.location = location(l);
                }
                if let Some(d) = spec.get("direction") {
                    node.direction = d.as_str().map(|d| nc::direction_named(d).unwrap());
                }
                if let Some(s) = node.idle.as_mut() {
                    if let Some(i) = spec.get("idle") {
                        s.idle = i.as_bool().unwrap();
                    }
                    if let Some(t) = spec.get("timeout") {
                        s.timeout = int(t);
                    }
                }
            }
        }
        if let Some(attrs) = v.get("attributes").and_then(Json::as_object) {
            for (k, map) in attrs {
                let n = self.by_key[k];
                for (a, value) in map.as_object().unwrap() {
                    if value.is_null() {
                        self.nodes[n].attrs.remove(a);
                    } else {
                        self.nodes[n].attrs.insert(a.clone(), attr_in(value));
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
        if let Some(b) = v
            .get("constants")
            .and_then(|c| c.get("SECOND_BANK"))
            .and_then(Json::as_bool)
        {
            self.second_bank = b;
        }
    }

    // ---------------------------------------------------------------- recording
    fn emit(&mut self, name: &str, args: Vec<Json>) -> Result<()> {
        let mut row = vec![json!(name)];
        row.extend(args);
        self.events.push(Json::Array(row));
        self.op_calls += 1;
        if self.op_calls == self.fail_at {
            return Err(Error::Binding(None));
        }
        Ok(())
    }
    fn answer(&mut self, name: &str) -> Option<Json> {
        let v = self.answers.get(name)?.clone();
        Some(match v {
            Json::Array(list) => {
                let calls = self.answer_calls.entry(name.into()).or_default();
                let i = *calls;
                *calls += 1;
                list[i.min(list.len() - 1)].clone()
            }
            other => other,
        })
    }
    fn answer_bool(&mut self, name: &str, fallback: bool) -> bool {
        self.answer(name).map_or(fallback, |v| v == json!(true))
    }
    fn answer_int(&mut self, name: &str, fallback: i32) -> i32 {
        self.answer(name).map_or(fallback, |v| int(&v))
    }
    fn answer_node(&mut self, name: &str) -> Option<N> {
        self.answer(name)
            .and_then(|v| v.as_str().map(|k| self.by_key[k]))
    }
    fn key(&self, n: N) -> Json {
        match &self.nodes[n].key {
            Some(k) => json!(k),
            None => json!(self.label_of(&Obj::Node(n))),
        }
    }
    fn okey(&self, n: Option<&N>) -> Json {
        n.map_or(Json::Null, |n| self.key(*n))
    }
    fn label_of(&self, o: &Obj) -> Option<String> {
        self.labels
            .iter()
            .find(|(x, _)| x == o)
            .map(|(_, l)| l.clone())
    }
    fn label(&mut self, o: Obj, class: &str) -> String {
        if let Some(l) = self.label_of(&o) {
            return l;
        }
        let l = format!("{class}#{}", self.labels.len() + 1);
        self.labels.push((o, l.clone()));
        l
    }
    fn label_node(&mut self, n: N) -> String {
        let class = self.nodes[n].class_name.clone();
        self.label(Obj::Node(n), &class)
    }
    fn args(&self, args: &[Arg<N>]) -> Json {
        Json::Array(
            args.iter()
                .map(|a| match a {
                    Arg::Node(n) => self.okey(n.as_ref()),
                    Arg::Int(i) => json!(i),
                    Arg::Bool(b) => json!(b),
                    Arg::Barcrawl(name) => json!(name),
                })
                .collect(),
        )
    }
    fn new_node(
        &mut self,
        class_name: &str,
        class: Option<Class>,
        id: i32,
        l: Option<[i32; 3]>,
        d: Option<Direction>,
    ) -> N {
        self.nodes.push(NodeData {
            key: None,
            kind: Kind::Npc,
            class_name: class_name.into(),
            class,
            id,
            location: l,
            direction: d,
            walks: false,
            aggressive: false,
            radius: 11,
            idle: None,
            attrs: BTreeMap::new(),
        });
        self.nodes.len() - 1
    }
    fn new_handler(&mut self, reference: &'static str) -> Handler {
        self.handler_objects += 1;
        Handler {
            reference,
            instance: self.handler_objects,
        }
    }

    fn snapshot(&self, result: Json, error: Option<&Error>) -> Json {
        let mut attrs = Map::new();
        let mut npcs = Map::new();
        for (k, n) in &self.by_key {
            let node = &self.nodes[*n];
            if matches!(node.kind, Kind::Scenery | Kind::Item) {
                continue;
            }
            if !node.attrs.is_empty() {
                attrs.insert(
                    k.clone(),
                    Json::Object(
                        node.attrs
                            .iter()
                            .map(|(a, v)| (a.clone(), attr_out(v)))
                            .collect(),
                    ),
                );
            }
            if node.kind.is_npc() {
                let mut s = json!({
                    "class": node.class_name,
                    "id": node.id,
                    "walks": node.walks,
                    "aggressive": node.aggressive,
                    "direction": node.direction.map(nc::direction_name),
                });
                if let Some(i) = node.idle {
                    s["idle"] = json!(i.idle);
                    s["timeout"] = json!(i.timeout);
                }
                npcs.insert(k.clone(), s);
            }
        }
        json!({
            "error": error.map(|e| match e {
                Error::Null => "Null",
                Error::Cast => "Cast",
                Error::Index => "Index",
                Error::Binding(_) => "Binding",
            }),
            "result": result,
            "events": self.events,
            "attributes": attrs,
            "npcs": npcs,
        })
    }
}

impl Factory for World {
    type Npc = N;
    type Class = Class;
    fn new_npc(
        &mut self,
        class: Option<Class>,
        id: i32,
        l: Option<[i32; 3]>,
        d: Option<Direction>,
    ) -> Result<N> {
        let name = class.map_or("core.game.node.entity.npc.NPC", Class::name);
        self.emit(
            "newNPC",
            vec![
                json!(name),
                json!(id),
                coord(l),
                json!(d.map(nc::direction_name)),
            ],
        )?;
        Ok(self.new_node(name, class, id, l, d))
    }
    fn set_walks(&mut self, npc: &N, walks: bool) -> Result<()> {
        let k = self.raw_key(*npc);
        self.emit("setWalks", vec![k, json!(walks)])?;
        self.nodes[*npc].walks = walks;
        Ok(())
    }
    fn set_aggressive(&mut self, npc: &N, aggressive: bool) -> Result<()> {
        let k = self.raw_key(*npc);
        self.emit("setAggressive", vec![k, json!(aggressive)])?;
        self.nodes[*npc].aggressive = aggressive;
        Ok(())
    }
    fn log(&mut self, origin: &str, level: LogLevel, message: &str) -> Result<()> {
        self.emit(
            "log",
            vec![json!(origin), json!(level.name()), json!(message)],
        )
    }
}
impl World {
    /// The `key` field itself (null for an NPC no case names).
    fn raw_key(&self, n: N) -> Json {
        json!(self.nodes[n].key)
    }
}

/// The world with the probes' class identity for registry operations.
struct Probing<'a>(&'a mut World);
impl Factory for Probing<'_> {
    type Npc = N;
    type Class = TestClass;
    fn new_npc(
        &mut self,
        class: Option<TestClass>,
        id: i32,
        l: Option<[i32; 3]>,
        d: Option<Direction>,
    ) -> Result<N> {
        let name = class.map_or("core.game.node.entity.npc.NPC", |c| c.name());
        self.0.emit(
            "newNPC",
            vec![
                json!(name),
                json!(id),
                coord(l),
                json!(d.map(nc::direction_name)),
            ],
        )?;
        let real = match class {
            Some(TestClass::Real(c)) => Some(c),
            _ => None,
        };
        Ok(self.0.new_node(name, real, id, l, d))
    }
    fn set_walks(&mut self, npc: &N, walks: bool) -> Result<()> {
        self.0.set_walks(npc, walks)
    }
    fn set_aggressive(&mut self, npc: &N, aggressive: bool) -> Result<()> {
        self.0.set_aggressive(npc, aggressive)
    }
    fn log(&mut self, origin: &str, level: LogLevel, message: &str) -> Result<()> {
        self.0.log(origin, level, message)
    }
}

impl Registrar for World {
    type HandlerRef = (OptionPlugin, usize);
    type Definition = i32;
    fn handler_object(&mut self, handler: OptionPlugin) -> (OptionPlugin, usize) {
        self.handler_objects += 1;
        (handler, self.handler_objects)
    }
    fn on(&mut self, ids: Option<&[i32]>, options: &[&str], f: Interaction) -> Result<()> {
        self.handler_objects += 1;
        let label = self.label(Obj::Function(self.handler_objects), f.class());
        self.emit(
            "on",
            vec![json!(ids), json!("NPC"), json!(options), json!(label)],
        )?;
        self.interactions.push(f);
        Ok(())
    }
    fn set_dest(&mut self, ids: &[i32], options: &[&str], f: Destination) -> Result<()> {
        self.handler_objects += 1;
        let label = self.label(Obj::Function(self.handler_objects), f.class());
        self.emit(
            "setDest",
            vec![json!("NPC"), json!(ids), json!(options), json!(label)],
        )?;
        self.destinations.push(f);
        Ok(())
    }
    fn npc_definition(&mut self, id: i32) -> Result<i32> {
        self.emit("npcDefinition", vec![json!(id)])?;
        Ok(id)
    }
    fn put_npc_handler(
        &mut self,
        definition: &i32,
        key: &str,
        handler: &(OptionPlugin, usize),
    ) -> Result<()> {
        let label = self.label(Obj::Handler(handler.1), handler.0.class());
        self.emit(
            "npcHandler",
            vec![json!(definition), json!(key), json!(label)],
        )?;
        self.handlers
            .insert(format!("npc:{definition}:{key}"), handler.0);
        Ok(())
    }
    fn npc_option_handler(&mut self, name: &str, handler: &(OptionPlugin, usize)) -> Result<()> {
        let label = self.label(Obj::Handler(handler.1), handler.0.class());
        self.emit("npcOptionHandler", vec![json!(name), json!(label)])?;
        self.handlers.insert(format!("option:{name}"), handler.0);
        Ok(())
    }
}

impl Host for World {
    type Handler = Handler;
    type File = &'static str;

    fn kind(&self, node: &N) -> Kind {
        self.nodes[*node].kind
    }
    fn class(&self, npc: &N) -> Option<Class> {
        self.nodes[*npc].class
    }
    fn same(&self, a: &N, b: &N) -> bool {
        a == b
    }
    fn id(&mut self, node: &N) -> Result<i32> {
        Ok(self.nodes[*node].id)
    }
    fn location(&mut self, node: &N) -> Result<Option<[i32; 3]>> {
        Ok(self.nodes[*node].location)
    }
    fn base_init(&mut self, npc: &N) -> Result<()> {
        let k = self.raw_key(*npc);
        self.emit("npcInit", vec![k])
    }
    fn base_tick(&mut self, npc: &N) -> Result<()> {
        let k = self.raw_key(*npc);
        self.emit("npcTick", vec![k])
    }
    fn base_tick_actions(&mut self, npc: &N) -> Result<()> {
        let k = self.raw_key(*npc);
        self.emit("npcTickActions", vec![k])
    }
    fn base_finalize_death(&mut self, npc: &N, killer: Option<&N>) -> Result<()> {
        let args = vec![self.raw_key(*npc), self.okey(killer)];
        self.emit("npcFinalizeDeath", args)
    }
    fn base_on_attack(&mut self, npc: &N, target: Option<&N>) -> Result<()> {
        let args = vec![self.raw_key(*npc), self.okey(target)];
        self.emit("npcOnAttack", args)
    }
    fn base_on_impact(&mut self, npc: &N, entity: Option<&N>) -> Result<()> {
        let args = vec![self.raw_key(*npc), self.okey(entity)];
        self.emit("npcOnImpact", args)
    }
    fn base_walk_radius(&mut self, npc: &N) -> Result<i32> {
        let k = self.raw_key(*npc);
        self.emit("walkRadius", vec![k])?;
        Ok(self.nodes[*npc].radius)
    }
    fn idle(&mut self, npc: &N) -> Result<Option<idle::Idle>> {
        Ok(self.nodes[*npc].idle)
    }
    fn set_idle(&mut self, npc: &N, state: idle::Idle) -> Result<()> {
        self.nodes[*npc].idle = Some(state);
        Ok(())
    }
    fn handler_swing(&mut self, handler: &Handler, entity: &N, victim: Option<&N>) -> Result<i32> {
        let args = vec![
            json!(handler.reference),
            self.key(*entity),
            self.okey(victim),
        ];
        self.emit("baseSwing", args)?;
        Ok(self.answer_int("baseSwing", 0))
    }
    fn base_swing_handler(&mut self, npc: &N, swing: bool) -> Result<Handler> {
        let k = self.raw_key(*npc);
        self.emit("npcSwingHandler", vec![k, json!(swing)])?;
        Ok(self.new_handler("npc"))
    }
    fn swing_handler(&mut self, entity: &N, swing: bool) -> Result<Handler> {
        let k = self.raw_key(*entity);
        if self.nodes[*entity].kind == Kind::Player {
            self.emit("swingHandler", vec![k, json!(swing)])?;
            Ok(self.new_handler("player"))
        } else {
            self.emit("npcSwingHandler", vec![k, json!(swing)])?;
            Ok(self.new_handler("npc"))
        }
    }
    fn can_swing(
        &mut self,
        handler: &Handler,
        entity: &N,
        victim: &N,
    ) -> Result<Option<InteractionType>> {
        let args = vec![
            json!(handler.reference),
            self.key(*entity),
            self.key(*victim),
        ];
        self.emit("canSwing", args)?;
        Ok(self.answer("canSwing").and_then(|v| {
            v.as_str().map(|s| match s {
                "STILL_INTERACT" => InteractionType::StillInteract,
                "MOVE_INTERACT" => InteractionType::MoveInteract,
                "NO_INTERACT" => InteractionType::NoInteract,
                other => panic!("{other}"),
            })
        }))
    }
    fn melee_swing(&mut self, entity: &N, victim: Option<&N>) -> Result<i32> {
        let args = vec![self.key(*entity), self.okey(victim)];
        self.emit("meleeSwing", args)?;
        Ok(self.answer_int("meleeSwing", 4))
    }
    fn transform(&mut self, npc: &N, id: i32) -> Result<()> {
        let k = self.raw_key(*npc);
        self.emit("transform", vec![k, json!(id)])
    }
    fn re_transform(&mut self, npc: &N) -> Result<()> {
        let k = self.raw_key(*npc);
        self.emit("reTransform", vec![k])
    }
    fn is_invisible(&mut self, npc: &N) -> Result<bool> {
        let k = self.raw_key(*npc);
        self.emit("isInvisible", vec![k])?;
        Ok(self.answer_bool("isInvisible", false))
    }
    fn task_level(&mut self, npc: &N) -> Result<Option<i32>> {
        let k = self.raw_key(*npc);
        self.emit("task", vec![k])?;
        Ok(self
            .answer("task")
            .filter(|v| !v.is_null())
            .map(|v| int(&v)))
    }
    fn send_chat(&mut self, entity: &N, text: &str) -> Result<bool> {
        let k = self.raw_key(*entity);
        self.emit("sendChat", vec![k, json!(text)])?;
        Ok(self.answer_bool("sendChat", true))
    }
    fn direction(&mut self, entity: &N) -> Result<Option<Direction>> {
        let k = self.raw_key(*entity);
        self.emit("direction", vec![k])?;
        Ok(self.nodes[*entity].direction)
    }
    fn set_direction(&mut self, entity: &N, direction: Option<Direction>) -> Result<()> {
        let k = self.raw_key(*entity);
        self.emit(
            "setDirection",
            vec![k, json!(direction.map(nc::direction_name))],
        )?;
        self.nodes[*entity].direction = direction;
        Ok(())
    }
    fn face_location(&mut self, entity: &N, l: Option<[i32; 3]>) -> Result<()> {
        let k = self.raw_key(*entity);
        self.emit("faceLocation", vec![k, coord(l)])
    }
    fn in_combat(&mut self, entity: &N) -> Result<bool> {
        let k = self.raw_key(*entity);
        self.emit("inCombat", vec![k.clone()])?;
        Ok(self.answer_bool(&format!("inCombat:{}", k.as_str().unwrap_or("")), false))
    }
    fn animate(&mut self, entity: &N, animation: i32) -> Result<()> {
        let k = self.raw_key(*entity);
        self.emit("animate", vec![k, json!(animation)])
    }
    fn get_attribute(&mut self, entity: &N, key: &str) -> Result<Option<Value>> {
        let k = self.raw_key(*entity);
        self.emit("getAttribute", vec![k, json!(key)])?;
        Ok(self.nodes[*entity].attrs.get(key).cloned())
    }
    fn set_attribute(&mut self, entity: &N, key: &str, value: Value) -> Result<()> {
        let k = self.raw_key(*entity);
        self.emit("setAttribute", vec![k, json!(key), attr_out(&value)])?;
        self.nodes[*entity].attrs.insert(key.into(), value);
        Ok(())
    }
    fn remove_attribute(&mut self, entity: &N, key: &str) -> Result<()> {
        let k = self.raw_key(*entity);
        self.emit("removeAttribute", vec![k, json!(key)])?;
        self.nodes[*entity].attrs.remove(key);
        Ok(())
    }
    fn api_get_attribute(&mut self, entity: &N, key: &str) -> Result<Option<Value>> {
        let k = self.raw_key(*entity);
        self.emit("apiGetAttribute", vec![k, json!(key)])?;
        Ok(self.nodes[*entity].attrs.get(key).cloned())
    }
    fn combat_victim(&mut self, entity: &N) -> Result<Option<N>> {
        let k = self.raw_key(*entity);
        self.emit("victim", vec![k.clone()])?;
        Ok(self.answer_node(&format!("victim:{}", k.as_str().unwrap_or(""))))
    }
    fn is_attacking(&mut self, entity: &N) -> Result<bool> {
        let k = self.raw_key(*entity);
        self.emit("isAttacking", vec![k.clone()])?;
        Ok(self.answer_bool(&format!("isAttacking:{}", k.as_str().unwrap_or("")), false))
    }
    fn set_next_attack(&mut self, entity: &N, ticks: i32) -> Result<()> {
        let k = self.raw_key(*entity);
        self.emit("setNextAttack", vec![k, json!(ticks)])
    }
    fn set_combat_style(&mut self, entity: &N, style: CombatStyle) -> Result<()> {
        let k = self.raw_key(*entity);
        self.emit("setStyle", vec![k, json!(style.name())])
    }
    fn set_combat_flags(&mut self, entity: &N, victim: &N) -> Result<()> {
        let args = vec![self.raw_key(*entity), self.key(*victim)];
        self.emit("setCombatFlags", args)
    }
    fn combat_attack(&mut self, entity: &N, victim: Option<&N>) -> Result<()> {
        let args = vec![self.raw_key(*entity), self.okey(victim)];
        self.emit("combatAttack", args)
    }
    fn modern_spell(&mut self, button: i32) -> Result<Option<Spell>> {
        self.emit("getSpell", vec![json!("MODERN"), json!(button)])?;
        Ok(match self.answer(&format!("getSpell:{button}")) {
            None => Some(Spell::Combat(button)),
            Some(Json::Null) => None,
            Some(Json::String(s)) if s == "other" => Some(Spell::Other),
            Some(v) => Some(Spell::Combat(int(&v))),
        })
    }
    fn set_autocast_spell(&mut self, entity: &N, spell: Option<i32>) -> Result<()> {
        let k = self.raw_key(*entity);
        self.emit("setAutocastSpell", vec![k, json!(spell)])
    }
    fn lifepoints(&mut self, entity: &N) -> Result<i32> {
        let k = self.raw_key(*entity);
        self.emit("lifepoints", vec![k])?;
        Ok(self.answer_int("lifepoints", 10))
    }
    fn maximum_lifepoints(&mut self, entity: &N) -> Result<i32> {
        let k = self.raw_key(*entity);
        self.emit("maximumLifepoints", vec![k])?;
        Ok(self.answer_int("maximumLifepoints", 10))
    }
    fn heal(&mut self, entity: &N, amount: i32) -> Result<()> {
        let k = self.raw_key(*entity);
        self.emit("heal", vec![k, json!(amount)])
    }
    fn level(&mut self, entity: &N, skill: i32) -> Result<i32> {
        let k = self.raw_key(*entity);
        self.emit("level", vec![k, json!(skill)])?;
        Ok(self.answer_int("level", 1))
    }
    fn is_dead(&mut self, entity: &N) -> Result<bool> {
        let k = self.key(*entity);
        self.emit("isDead", vec![k])?;
        Ok(self.answer_bool("isDead", false))
    }
    fn continue_attack(
        &mut self,
        entity: &N,
        target: &N,
        style: Option<CombatStyle>,
        message: bool,
    ) -> Result<bool> {
        let args = vec![
            self.raw_key(*entity),
            self.key(*target),
            json!(style.map(CombatStyle::name)),
            json!(message),
        ];
        self.emit("continueAttack", args)?;
        Ok(self.answer_bool("continueAttack", true))
    }
    fn random(&mut self, bound: i32) -> Result<i32> {
        self.emit("random", vec![json!(bound)])?;
        Ok(self.answer_int("random", 0))
    }
    fn randomize(&mut self, bound: i32) -> Result<i32> {
        self.emit("randomize", vec![json!(bound)])?;
        Ok(self.answer_int("randomize", 0))
    }
    fn current_time_millis(&mut self) -> Result<i64> {
        Ok(NOW)
    }
    fn get_quest(&mut self, player: &N, quest: &str) -> Result<bool> {
        let k = self.raw_key(*player);
        self.emit("getQuest", vec![k, json!(quest)])?;
        Ok(!self.answer_bool("questMissing", false))
    }
    fn quest_started(&mut self, player: &N, quest: &str) -> Result<bool> {
        let k = self.raw_key(*player);
        self.emit("questStarted", vec![json!(quest), k])?;
        Ok(self.answer_bool("questStarted", false))
    }
    fn get_diary(&mut self, player: &N, diary: Diary) -> Result<bool> {
        let k = self.raw_key(*player);
        self.emit("getDiary", vec![k, json!(diary.name())])?;
        Ok(!self.answer_bool("diaryMissing", false))
    }
    fn diary_complete(&mut self, diary: Diary, level: i32, task: i32) -> Result<bool> {
        self.emit(
            "diaryComplete",
            vec![json!(diary.name()), json!(level), json!(task)],
        )?;
        Ok(self.answer_bool("diaryComplete", false))
    }
    fn diary_update(
        &mut self,
        diary: Diary,
        player: &N,
        level: i32,
        index: i32,
        complete: bool,
    ) -> Result<()> {
        let k = self.raw_key(*player);
        self.emit(
            "diaryUpdate",
            vec![
                json!(diary.name()),
                k,
                json!(level),
                json!(index),
                json!(complete),
            ],
        )
    }
    fn ground_item(&mut self, id: i32, amount: i32, l: Option<[i32; 3]>, player: &N) -> Result<()> {
        let args = vec![json!(id), json!(amount), coord(l), self.key(*player)];
        self.emit("groundItem", args)
    }
    fn get_scenery(&mut self, l: [i32; 3]) -> Result<Option<N>> {
        self.emit("getScenery", vec![json!(l)])?;
        Ok(self.answer_node(&format!("scenery:{},{},{}", l[0], l[1], l[2])))
    }
    fn api_message(&mut self, player: &N, text: &str) -> Result<()> {
        let k = self.raw_key(*player);
        self.emit("apiMessage", vec![k, json!(text)])
    }
    fn player_message(&mut self, player: &N, text: &str) -> Result<()> {
        let k = self.raw_key(*player);
        self.emit("playerMessage", vec![k, json!(text)])
    }
    fn packet_message(&mut self, player: &N, text: &str) -> Result<()> {
        let k = self.raw_key(*player);
        self.emit("packetMessage", vec![k, json!(text)])
    }
    fn open_dialogue_id(&mut self, player: &N, id: i32, args: &[Arg<N>]) -> Result<bool> {
        let a = vec![self.raw_key(*player), json!(id), self.args(args)];
        self.emit("openDialogueId", a)?;
        Ok(self.answer_bool("openDialogueId", true))
    }
    fn open_dialogue_name(&mut self, player: &N, name: &str, args: &[Arg<N>]) -> Result<bool> {
        let a = vec![self.raw_key(*player), json!(name), self.args(args)];
        self.emit("openDialogueName", a)?;
        Ok(self.answer_bool("openDialogueName", true))
    }
    fn send_dialogue(&mut self, player: &N, lines: &[&str]) -> Result<()> {
        let k = self.raw_key(*player);
        self.emit("sendDialogue", vec![k, json!(lines)])
    }
    fn gnome_completion_dialogue(&mut self, job: &'static str) -> Result<&'static str> {
        self.emit("gcCompletionDialogue", vec![json!(job)])?;
        Ok("content.minigame.gnomecooking.GCCompletionDialogue")
    }
    fn open_file(&mut self, player: &N, file: &'static str) -> Result<()> {
        let k = self.raw_key(*player);
        self.emit("openFile", vec![k, json!(file), json!([])])
    }
    fn banker_conversation(&mut self, player: &N, npc: &N) -> Result<()> {
        let a = vec![
            self.raw_key(*player),
            json!("content.global.handlers.npc.BankerNPC$BankerDialogueLabellerFile"),
            self.key(*npc),
        ];
        self.emit("labellerOpen", a)
    }
    fn api_open_dialogue(&mut self, player: &N, id: i32, npc: &N) -> Result<()> {
        let a = vec![self.raw_key(*player), json!(id), json!([self.key(*npc)])];
        self.emit("apiOpenDialogue", a)
    }
    fn remove_item(&mut self, player: &N, id: i32, amount: i32) -> Result<bool> {
        let k = self.raw_key(*player);
        self.emit(
            "removeItem",
            vec![k, json!({"item": [id, amount]}), json!("INVENTORY")],
        )?;
        Ok(self.answer_bool("removeItem", false))
    }
    fn open_bank_account(&mut self, player: &N) -> Result<()> {
        let k = self.raw_key(*player);
        self.emit("openBankAccount", vec![k])
    }
    fn open_collection_box(&mut self, player: &N) -> Result<()> {
        let k = self.raw_key(*player);
        self.emit("openCollectionBox", vec![k])
    }
    fn has_seal_of_passage(&mut self, player: &N) -> Result<bool> {
        let k = self.raw_key(*player);
        self.emit("hasSealOfPassage", vec![k])?;
        Ok(self.answer_bool("hasSealOfPassage", false))
    }
    fn random_event_ids(&mut self) -> Result<Vec<i32>> {
        self.emit("randomIDs", vec![])?;
        Ok(self.ints("randomIDs"))
    }
    fn anti_macro_event_npc(&mut self, player: &N) -> Result<Option<N>> {
        let k = self.raw_key(*player);
        self.emit("antiMacroEventNpc", vec![k])?;
        Ok(self.answer_node("antiMacroEventNpc"))
    }
    fn finalized(&mut self, npc: &N) -> Result<bool> {
        let k = self.raw_key(*npc);
        self.emit("finalized", vec![k.clone()])?;
        Ok(self.answer_bool(&format!("finalized:{}", k.as_str().unwrap_or("")), false))
    }
    fn talk_to(&mut self, event: &N, npc: &N) -> Result<()> {
        let a = vec![self.raw_key(*event), self.key(*npc)];
        self.emit("talkTo", a)
    }
    fn holiday_random_ids(&mut self) -> Result<Vec<i32>> {
        self.emit("holidayRandomIDs", vec![])?;
        Ok(self.ints("holidayRandomIDs"))
    }
    fn holiday_event_npc(&mut self, player: &N) -> Result<Option<N>> {
        let k = self.raw_key(*player);
        self.emit("holidayEventNpc", vec![k])?;
        Ok(self.answer_node("holidayEventNpc"))
    }
    fn barcrawl_instance(&mut self, player: &N) -> Result<()> {
        let k = self.raw_key(*player);
        self.emit("barcrawlInstance", vec![k])
    }
    fn barcrawl_finished(&mut self, player: &N) -> Result<bool> {
        let k = self.raw_key(*player);
        self.emit("barcrawlFinished", vec![k])?;
        Ok(self.answer_bool("barcrawlFinished", false))
    }
    fn barcrawl_started(&mut self, player: &N) -> Result<bool> {
        let k = self.raw_key(*player);
        self.emit("barcrawlStarted", vec![k])?;
        Ok(self.answer_bool("barcrawlStarted", false))
    }
    fn barcrawl_completed(&mut self, player: &N, index: i32) -> Result<bool> {
        let k = self.raw_key(*player);
        self.emit("barcrawlCompleted", vec![k, json!(index)])?;
        Ok(self.answer_bool("barcrawlCompleted", false))
    }
}
impl World {
    fn ints(&self, name: &str) -> Vec<i32> {
        self.values
            .get(name)
            .and_then(Json::as_array)
            .map(|a| a.iter().map(int).collect())
            .unwrap_or_default()
    }
}

/// What the replay runs an operation against.
struct Replay {
    world: World,
    registry: Registry<TestClass, N>,
}

fn test_class(name: &str) -> Option<TestClass> {
    match name {
        "oracle.ConflictProbe" => Some(TestClass::ConflictProbe),
        "oracle.probe.ShopNPC" => Some(TestClass::ShopNamed),
        other => Class::named(other).map(TestClass::Real),
    }
}

fn node_of(w: &World, op: &Json, k: &str) -> Option<N> {
    op.get(k).and_then(Json::as_str).map(|k| w.by_key[k])
}

impl Replay {
    fn run(&mut self, op: &Json) -> Result<Json> {
        let w = &mut self.world;
        let name = op["op"].as_str().unwrap();
        let p = w.by_key["p"];
        let node = node_of(w, op, "node");
        Ok(match name {
            "set" => {
                w.set(op);
                Json::Null
            }
            "listeners" => {
                let l = Listener::named(op["class"].as_str().unwrap()).unwrap();
                listeners::define_listeners(w, l)?;
                Json::Null
            }
            "plugin" => {
                let class = op["class"].as_str().unwrap();
                match test_class(class) {
                    Some(tc) => {
                        registry::define(&mut Probing(w), &mut self.registry, tc)?;
                    }
                    None => {
                        let plugin = Plugin::named(class).unwrap();
                        listeners::define_plugin(w, &mut Registry::default(), plugin)?;
                    }
                }
                Json::Null
            }
            "forId" => match self.registry.for_id(int(&op["id"])) {
                Some((_, prototype)) => json!(w.label_node(*prototype)),
                None => Json::Null,
            },
            "create" => {
                let l = location(&op["location"]);
                let d = op
                    .get("direction")
                    .map(|d| nc::direction_named(d.as_str().unwrap()).unwrap());
                let n = match d {
                    Some(d) => registry::create(
                        &mut Probing(w),
                        &self.registry,
                        int(&op["id"]),
                        l,
                        Some(d),
                    )?,
                    None => registry::create(
                        &mut Probing(w),
                        &self.registry,
                        int(&op["id"]),
                        l,
                        Some(Direction::South),
                    )?,
                };
                let label = w.label_node(n);
                let key = op["key"].as_str().unwrap().to_owned();
                w.nodes[n].key = Some(key.clone());
                w.by_key.insert(key, n);
                json!(label)
            }
            "init" => {
                classes::init(w, &node.unwrap())?;
                Json::Null
            }
            "tick" => {
                classes::tick(w, &node.unwrap())?;
                Json::Null
            }
            "tickActions" => {
                classes::tick_actions(w, &node.unwrap())?;
                Json::Null
            }
            "death" => {
                let killer = node_of(w, op, "killer");
                classes::finalize_death(w, &node.unwrap(), killer.as_ref())?;
                Json::Null
            }
            "attacked" => {
                let target = node_of(w, op, "target");
                classes::on_attack(w, &node.unwrap(), target.as_ref())?;
                Json::Null
            }
            "impact" => {
                let entity = node_of(w, op, "entity");
                classes::on_impact(w, &node.unwrap(), entity.as_ref())?;
                Json::Null
            }
            "swing" => {
                let npc = node.unwrap();
                let victim = node_of(w, op, "victim");
                let handler = classes::swing_handler(w, &npc, true)?;
                match handler {
                    classes::Swing::Monk => {
                        let label = w.label(
                            Obj::Monk,
                            "content.global.handlers.npc.MonkNPC$CombatAction",
                        );
                        let r = classes::monk_swing(w, &npc, victim.as_ref())?;
                        json!([label, r])
                    }
                    classes::Swing::Other(h) => {
                        let label = w.label(
                            Obj::Handler(1_000_000 + h.instance),
                            "core.game.node.entity.combat.CombatSwingHandler",
                        );
                        let r = w.handler_swing(&h, &npc, victim.as_ref())?;
                        json!([label, r])
                    }
                }
            }
            "radius" => json!(classes::walk_radius(w, &node.unwrap())?),
            "interact" => {
                let n = op["n"].as_u64().unwrap() as usize;
                let Some(f) = w.interactions.get(n).copied() else {
                    return Ok(json!("none"));
                };
                let who = node_of(w, op, "player").unwrap_or(p);
                json!(listeners::run_interaction(w, f, &who, node.as_ref())?)
            }
            "dest" => {
                let n = op["n"].as_u64().unwrap() as usize;
                let Some(f) = w.destinations.get(n).copied() else {
                    return Ok(json!("none"));
                };
                coord(listeners::run_destination(w, f, node.as_ref())?)
            }
            "handler" => {
                let Some(h) = w.handlers.get(op["via"].as_str().unwrap()).copied() else {
                    return Ok(json!("none"));
                };
                json!(listeners::run_handler(w, h, &p, node.as_ref())?)
            }
            "isWalk" => {
                let Some(h) = w.handlers.get(op["via"].as_str().unwrap()).copied() else {
                    return Ok(json!("none"));
                };
                json!([
                    h.is_walk(),
                    listeners::handler_walks(w, h, &p, node.as_ref())?
                ])
            }
            "idle" => {
                let npc = node.unwrap();
                let other = node_of(w, op, "entity");
                match op["call"].as_str().unwrap() {
                    "isAttackable" => {
                        let style = op["style"].as_str().map(|s| match s {
                            "MELEE" => CombatStyle::Melee,
                            "RANGE" => CombatStyle::Range,
                            "MAGIC" => CombatStyle::Magic,
                            other => panic!("{other}"),
                        });
                        let message = op["message"].as_bool().unwrap_or(false);
                        json!(idle::is_attackable(
                            w,
                            &npc,
                            other.as_ref(),
                            style,
                            message
                        )?)
                    }
                    "inDisturbingRange" => {
                        json!(idle::in_disturbing_range(w, &npc, other.as_ref())?)
                    }
                    "canDisturb" => json!(idle::can_disturb(w, &npc)?),
                    "disturb" => {
                        idle::disturb(w, &npc, other.as_ref())?;
                        Json::Null
                    }
                    "goIdle" => {
                        idle::go_idle(w, &npc)?;
                        Json::Null
                    }
                    other => panic!("{other}"),
                }
            }
            "bank" => json!(banker::attempt_bank(w, &p, node.as_ref())?),
            "collect" => json!(banker::attempt_collect(w, &p, node.as_ref())?),
            "lunar" => json!(banker::check_lunar_isle_restriction(w, &p, &node.unwrap())?),
            "access" => json!(banker::can_access_bank(w, &p, &node.unwrap())?),
            other => panic!("operation {other}"),
        })
    }
}

fn cases() -> Vec<Json> {
    CORPUS
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

#[test]
fn provenance_matches_the_corpus() {
    let p: Json = serde_json::from_str(PROVENANCE).unwrap();
    let digest = format!("{:x}", Sha256::digest(CORPUS.as_bytes()));
    assert_eq!(p["fixtureSha256"], json!(digest));
    assert_eq!(p["caseCount"].as_u64().unwrap() as usize, cases().len());
}

#[test]
fn every_case_replays() {
    let mut failures = Vec::new();
    let all = cases();
    for (number, case) in all.iter().enumerate() {
        let mut replay = Replay {
            world: World::new(case),
            registry: Registry::default(),
        };
        for (op, expected) in case["ops"]
            .as_array()
            .unwrap()
            .iter()
            .zip(case["expected"].as_array().unwrap())
        {
            replay.world.events.clear();
            replay.world.op_calls = 0;
            replay.world.fail_at = op.get("failAt").and_then(Json::as_i64).unwrap_or(-1);
            let (result, error) = match replay.run(op) {
                Ok(r) => (r, None),
                Err(e) => (Json::Null, Some(e)),
            };
            let actual = replay.world.snapshot(result, error.as_ref());
            if &actual != expected {
                failures.push(format!(
                    "case {number} {} / {} op {op}\n  expected {expected}\n  actual   {actual}",
                    case["group"], case["name"]
                ));
                break;
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} cases differ:\n{}",
        failures.len(),
        all.len(),
        failures
            .iter()
            .take(8)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// `register_scanned` over the start-up manifest registers what the oracle's
/// scan-order case registers, in the same order, and the registry holds the
/// same class for every id.
#[test]
fn scanned_registration_matches_the_scan_order_case() {
    let case = cases()
        .into_iter()
        .find(|c| c["name"] == "scan order")
        .unwrap();
    let mut expected_events = Vec::new();
    let mut for_id = Vec::new();
    for (op, step) in case["ops"]
        .as_array()
        .unwrap()
        .iter()
        .zip(case["expected"].as_array().unwrap())
    {
        match op["op"].as_str().unwrap() {
            "listeners" | "plugin" => {
                expected_events.extend(step["events"].as_array().unwrap().iter().cloned())
            }
            "forId" => for_id.push((
                int(&op["id"]),
                step["result"]
                    .as_str()
                    .map(|l| l.split('#').next().unwrap().to_owned()),
            )),
            _ => {}
        }
    }
    let mut w = World::new(&case);
    let mut registry: Registry<Class, N> = Registry::default();
    let done = listeners::register_scanned(&mut w, &mut registry, |_| true).unwrap();
    assert_eq!(done.len(), 14);
    // Labels number objects in the order they are first shown; compare
    // without the numbers for the prototypes the scan never labels.
    let strip = |e: &Json| -> Json {
        let mut e = e.clone();
        if let Some(a) = e.as_array_mut() {
            for v in a.iter_mut() {
                if let Some(s) = v.as_str() {
                    if let Some((class, _)) = s.split_once('#') {
                        *v = json!(class);
                    }
                }
            }
        }
        e
    };
    let actual: Vec<Json> = w.events.iter().map(strip).collect();
    let expected: Vec<Json> = expected_events.iter().map(strip).collect();
    assert_eq!(actual, expected);
    for (id, class) in for_id {
        assert_eq!(
            registry.for_id(id).map(|(c, _)| c.name().to_owned()),
            class,
            "forId({id})"
        );
    }
}

/// `listeners::Native` installs the functions into the native
/// InteractionListeners registry and collects the NPC option handlers.
#[test]
fn native_registrar_installs_the_functions() {
    use rs09_server::world::interaction_listeners::{self as interactions, Casing};
    struct Ascii;
    impl Casing for Ascii {
        fn lowercase_default(&self, s: &[u16]) -> Vec<u16> {
            String::from_utf16_lossy(s)
                .to_lowercase()
                .encode_utf16()
                .collect()
        }
        fn lowercase_root(&self, s: &[u16]) -> Vec<u16> {
            self.lowercase_default(s)
        }
    }
    let registry = interactions::Registry::default();
    let mut w = World::new(&json!({"ops": []}));
    let mut native = listeners::Native {
        interactions: &registry,
        casing: &Ascii,
        factory: &mut w,
        npc_handlers: Vec::new(),
        npc_options: Vec::new(),
    };
    let mut classes: Registry<Class, N> = Registry::default();
    listeners::register_scanned(&mut native, &mut classes, |_| true).unwrap();
    let text = |s: &str| -> Vec<u16> { s.encode_utf16().collect() };
    let npc = interactions::Kind::Npc.ordinal();
    let bank = registry.get(&Ascii, 494, npc, &text("Bank")).unwrap();
    assert_eq!(*bank.value::<Interaction>().unwrap(), Interaction::Bank);
    let talk = registry.get(&Ascii, 494, npc, &text("talk-to")).unwrap();
    assert_eq!(
        *talk.value::<Interaction>().unwrap(),
        Interaction::BankerTalk
    );
    let crawl = registry.get(&Ascii, 3217, npc, &text("talk")).unwrap();
    assert_eq!(
        *crawl.value::<Interaction>().unwrap(),
        Interaction::Barcrawl
    );
    let any = registry.get_generic(&Ascii, &text("talk to"), npc).unwrap();
    assert_eq!(*any.value::<Interaction>().unwrap(), Interaction::Talk);
    let dest = registry
        .destination(&Ascii, npc, 5383, &text("collect"))
        .unwrap();
    assert_eq!(*dest.value::<Destination>().unwrap(), Destination::Banker);
    assert_eq!(native.npc_options.len(), 1);
    assert_eq!(native.npc_options[0].0, "disturb");
    assert_eq!(native.npc_handlers.len(), 1);
    assert_eq!(native.npc_handlers[0].0, 598);
    assert_eq!(native.npc_handlers[0].1, "option:hair-cut");
    assert_eq!(classes.for_id(520).unwrap().0, Class::Shop);
    assert_eq!(classes.len(), 334);
}
