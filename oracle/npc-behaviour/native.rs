//! Native replay of the npc-behaviour corpus. The world here mirrors the
//! oracle's recording stand-ins (oracle/npc-behaviour/adapters.py): every
//! outside call records itself and answers from the case. The ported code is
//! `world::npc_behaviour`, the existing `world::npc` tick kernel it joins, and
//! `entity_regions::is_tolerated` over a real native region's tolerance map.
#![allow(dead_code)]
use rs09_game_core::random::JavaRandom;
use rs09_server::{
    definitions::item::Shared,
    world::{
        chunk::HandleChunk,
        combat_swing::{self as cs, BattleState, CombatStyle, Kind},
        doors::Location,
        drop_tables::RolledItem,
        entity_regions::{self, Kind as RegionKind},
        npc::{self, Event, Query},
        npc_behaviour::{
            self as nb, Ai, Behaviour, Hitsplat, Host, NpcClass, PlainNpc, Registry, Rights,
            SlayerTask,
            aggression::{self, AggressiveBehaviour, DefaultAggression, Handler, Wilderness},
            combat, lifecycle, tick,
        },
        player_walk::{NodeHandle, State as Nodes},
        region,
    },
};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

type R<T> = cs::Result<T>;

#[derive(Clone, Debug)]
enum Attr {
    Bool(bool),
    Int(i32),
    Long(i64),
    Entity(usize),
}

struct Ent {
    kind: Kind,
    id: i32,
    name: String,
    location: Option<Location>,
    attrs: BTreeMap<String, Attr>,
    attacking: bool,
    victim: Option<usize>,
    next_attack: i32,
    last_sent: i32,
    last_received: i32,
    style: Option<CombatStyle>,
    ai: Option<Ai<World>>,
    brain: Option<npc::State>,
    node: NodeHandle,
}

pub struct World {
    case: Value,
    events: Vec<Value>,
    calls: i64,
    fail_at: i64,
    ticks: i32,
    now: i64,
    next_pulse: usize,
    consumed: HashMap<String, usize>,
    random: JavaRandom,
    math: JavaRandom,
    ents: Vec<Ent>,
    regions: HashMap<i32, Region>,
    registry: Registry<World>,
    nodes: Nodes,
    /// Registered recording behaviours and their tags.
    tags: Vec<(Arc<dyn Behaviour<World>>, String)>,
    /// The one WILDERNESS instance cases install.
    wilderness: Arc<dyn AggressiveBehaviour<World>>,
}

fn same_arc<T: ?Sized>(a: &Arc<T>, b: &Arc<T>) -> bool {
    std::ptr::eq(Arc::as_ptr(a) as *const (), Arc::as_ptr(b) as *const ())
}

#[derive(Clone)]
pub struct Region {
    id: i32,
    handle: region::Handle,
    dynamic: bool,
}

impl cs::Types for World {
    type Entity = usize;
    type Item = ();
    type Spell = ();
}

fn int(v: &Value) -> i32 {
    v.as_i64().unwrap() as i32
}

fn coord(l: &Option<Location>) -> Value {
    match l {
        Some(l) => json!(l.coordinates()),
        None => Value::Null,
    }
}

fn loc(v: &Value) -> Option<Location> {
    let a = v.as_array()?;
    Some(Location::new(int(&a[0]), int(&a[1]), int(&a[2])))
}

fn style_name(s: CombatStyle) -> &'static str {
    match s {
        CombatStyle::Melee => "MELEE",
        CombatStyle::Range => "RANGE",
        CombatStyle::Magic => "MAGIC",
    }
}

fn style(v: &Value) -> Option<CombatStyle> {
    match v.as_str()? {
        "MELEE" => Some(CombatStyle::Melee),
        "RANGE" => Some(CombatStyle::Range),
        "MAGIC" => Some(CombatStyle::Magic),
        other => panic!("style {other}"),
    }
}

fn opt(e: Option<&usize>) -> Value {
    e.map_or(Value::Null, |e| json!(e))
}

fn class(e: &cs::Error) -> &'static str {
    match e {
        cs::Error::Null => "Null",
        cs::Error::Index => "Index",
        cs::Error::Binding(_) => "Binding",
    }
}

fn from_npc(e: npc::Error) -> cs::Error {
    match e {
        npc::Error::Null => cs::Error::Null,
        npc::Error::Index => cs::Error::Index,
        npc::Error::Binding(t) => cs::Error::Binding(t),
    }
}

const TICK_KEYS: [&str; 4] = [
    "no-spawn-return",
    "return-to-spawn",
    "return-to-spawn-pulse",
    "lastForceTalk",
];

impl World {
    fn emit(&mut self, name: &str, args: Vec<Value>) -> R<()> {
        let mut row = vec![json!(name)];
        row.extend(args);
        self.events.push(Value::Array(row));
        self.calls += 1;
        if self.calls == self.fail_at {
            return Err(cs::Error::Binding("injected".into()));
        }
        Ok(())
    }

    fn answer(&mut self, key: &str) -> Option<Value> {
        let value = self.case["answers"].get(key)?.clone();
        if let Value::Array(list) = value {
            let at = self.consumed.entry(key.to_owned()).or_insert(0);
            *at += 1;
            return list.get(*at - 1).cloned();
        }
        Some(value)
    }

    fn flag(&mut self, name: &str, e: usize, fallback: bool) -> R<bool> {
        self.emit(name, vec![json!(e)])?;
        Ok(match self.answer(&format!("{name}:{e}")) {
            Some(v) => v == json!(true),
            None => fallback,
        })
    }

    fn number(&mut self, name: &str, e: usize, fallback: i32) -> R<i32> {
        self.emit(name, vec![json!(e)])?;
        Ok(self
            .answer(&format!("{name}:{e}"))
            .map_or(fallback, |v| int(&v)))
    }

    fn players(&self) -> Vec<usize> {
        (0..self.ents.len())
            .filter(|i| self.ents[*i].kind == Kind::Player)
            .collect()
    }

    fn region(&mut self, id: i32) -> Region {
        if let Some(r) = self.regions.get(&id) {
            return r.clone();
        }
        let spec = self.case["regions"][id.to_string()].clone();
        let handle = Shared::new(region::Region::new(id >> 8, id & 0xff));
        if let Some(map) = spec["tolerances"].as_object() {
            let tolerances = handle.read().tolerances.clone();
            for (name, value) in map {
                tolerances.write().insert(name.clone(), value.as_i64());
            }
        }
        let r = Region {
            id,
            handle,
            dynamic: spec["dynamic"] == json!(true),
        };
        self.regions.insert(id, r.clone());
        r
    }

    fn attr(&mut self, e: usize, key: &str) -> R<Option<Attr>> {
        if !TICK_KEYS.contains(&key) {
            self.emit("get_attribute", vec![json!(e), json!(key)])?;
        }
        Ok(self.ents[e].attrs.get(key).cloned())
    }

    fn sticky(&self) -> bool {
        self.case["sticky"] != json!(false)
    }

    fn new(case: &Value) -> Self {
        let mut w = World {
            case: case.clone(),
            events: vec![],
            calls: 0,
            fail_at: -1,
            ticks: case.get("ticks").map_or(100, int),
            now: case
                .get("now")
                .map_or(1_000_000_000, |v| v.as_i64().unwrap()),
            next_pulse: 0,
            consumed: HashMap::new(),
            random: JavaRandom::new(case.get("seed").map_or(0, |v| v.as_i64().unwrap())),
            math: JavaRandom::new(case.get("mathSeed").map_or(0, |v| v.as_i64().unwrap())),
            ents: vec![],
            regions: HashMap::new(),
            registry: Registry::default(),
            nodes: Nodes::default(),
            tags: vec![],
            wilderness: Arc::new(Wilderness),
        };
        for b in case["behaviours"].as_array().into_iter().flatten() {
            let ids: Vec<i32> = b["ids"].as_array().unwrap().iter().map(int).collect();
            w.register(&ids, b["tag"].as_str().unwrap());
        }
        let specs = case["entities"].as_array().unwrap().clone();
        for (index, spec) in specs.iter().enumerate() {
            let kind = match spec["kind"].as_str().unwrap_or("npc") {
                "player" => Kind::Player,
                "familiar" => Kind::Familiar,
                _ => Kind::Npc,
            };
            let location = loc(spec.get("location").unwrap_or(&json!([3200, 3200, 0])));
            let id = spec.get("id").map_or(1, int);
            let node = w
                .nodes
                .add_npc(index as i32, Location::new(0, 0, 0), [1, 2, 3, 4]);
            let node = w.nodes.retain_node(node).unwrap();
            let (ai, brain) = if kind.is_npc() {
                let class: Arc<dyn NpcClass<World>> =
                    if spec["class"] == json!("recording") && kind == Kind::Npc {
                        Arc::new(RecordingClass)
                    } else {
                        Arc::new(PlainNpc)
                    };
                let brain = npc::State::new(location.clone().unwrap());
                (Some(Ai::new(w.registry.for_id(id), class)), Some(brain))
            } else {
                (None, None)
            };
            w.ents.push(Ent {
                kind,
                id,
                name: spec["name"].as_str().unwrap_or("player").into(),
                // An NPC's location lives in its world::npc fields; apply sets a player's.
                location: None,
                attrs: BTreeMap::new(),
                attacking: false,
                victim: None,
                next_attack: 0,
                last_sent: 0,
                last_received: 0,
                style: Some(CombatStyle::Melee),
                ai,
                brain,
                node,
            });
        }
        for (index, spec) in specs.iter().enumerate() {
            w.apply(index, spec);
        }
        w
    }

    fn apply(&mut self, e: usize, m: &Value) {
        let m = m.as_object().unwrap();
        if let Some(v) = m.get("location") {
            let l = loc(v);
            if let Some(b) = &self.ents[e].brain {
                b.fields.write().location = l;
            } else {
                self.ents[e].location = l;
            }
        }
        if let Some(attrs) = m.get("attrs").and_then(Value::as_object) {
            for (key, v) in attrs {
                let value = match v {
                    Value::Null => None,
                    Value::Bool(b) => Some(Attr::Bool(*b)),
                    Value::Number(n) if key == "combat-time" => {
                        Some(Attr::Long(n.as_i64().unwrap()))
                    }
                    Value::Number(n) => Some(Attr::Int(n.as_i64().unwrap() as i32)),
                    Value::String(s) => Some(Attr::Entity(s[1..].parse().unwrap())),
                    other => panic!("attribute {other}"),
                };
                match value {
                    Some(v) => {
                        self.ents[e].attrs.insert(key.clone(), v);
                    }
                    None => {
                        self.ents[e].attrs.remove(key);
                    }
                }
            }
        }
        let ent = &mut self.ents[e];
        if let Some(v) = m.get("attacking") {
            ent.attacking = v == &json!(true);
        }
        if let Some(v) = m.get("victim") {
            ent.victim = v.as_u64().map(|v| v as usize);
        }
        if let Some(v) = m.get("nextAttack") {
            ent.next_attack = int(v);
        }
        if let Some(v) = m.get("lastSent") {
            ent.last_sent = int(v);
        }
        if let Some(v) = m.get("lastReceived") {
            ent.last_received = int(v);
        }
        if let Some(v) = m.get("style") {
            ent.style = style(v);
        }
        if let (Some(v), Some(ai)) = (m.get("invisible"), ent.ai.as_mut()) {
            ai.invisible = v == &json!(true);
        }
        let Some(brain) = ent.brain.clone() else {
            return;
        };
        {
            let mut f = brain.fields.write();
            if let Some(v) = m.get("walks") {
                f.walks = v == &json!(true);
            }
            if let Some(v) = m.get("aggressive") {
                f.aggressive = v == &json!(true);
            }
            if let Some(v) = m.get("radius") {
                f.radius = int(v);
            }
            if let Some(v) = m.get("respawn") {
                f.respawn_tick = int(v);
            }
            if let Some(v) = m.get("respawning") {
                f.respawning = v == &json!(true);
            }
            if let Some(v) = m.get("spawn") {
                f.spawn = loc(v);
            }
            if let Some(v) = m.get("forceTalk") {
                f.force_talk = v.as_str().map(|s| s.encode_utf16().collect());
            }
            if let Some(v) = m.get("neverWalks") {
                f.never_walks = v == &json!(true);
            }
            if let Some(v) = m.get("returning") {
                f.returning = v == &json!(true);
            }
            if let Some(v) = m.get("returnPulse") {
                f.return_pulse = v.as_u64().map(|v| v as usize);
            }
        }
        if let Some(v) = m.get("task") {
            ent.ai.as_mut().unwrap().task = v.as_i64().map(|l| SlayerTask {
                level_req: l as i32,
            });
        }
        if let Some(h) = m.get("handler") {
            if h.is_null() {
                self.ents[e].ai.as_mut().unwrap().handler = None;
                brain.fields.write().has_handler = false;
            } else {
                let behaviour: Arc<dyn AggressiveBehaviour<World>> =
                    if h["behaviour"] == json!("wilderness") {
                        self.wilderness.clone()
                    } else {
                        Arc::new(DefaultAggression)
                    };
                let handler = Handler::new(self, e, behaviour);
                {
                    let mut f = handler.0.write();
                    if let Some(v) = h.get("radius") {
                        f.radius = int(v);
                    }
                    if let Some(v) = h.get("pause") {
                        f.pause_ticks = int(v);
                    }
                    if let Some(v) = h.get("switching") {
                        f.target_switching = v == &json!(true);
                    }
                    if let Some(v) = h.get("chance") {
                        f.chance_ratio = int(v);
                    }
                    if let Some(v) = h.get("allowTolerance") {
                        f.allow_tolerance = v == &json!(true);
                    }
                }
                self.ents[e].ai.as_mut().unwrap().handler = Some(handler);
                brain.fields.write().has_handler = true;
            }
        }
    }

    fn snapshot(&self) -> Value {
        let mut list = vec![];
        for (i, e) in self.ents.iter().enumerate() {
            let mut s = Map::new();
            let mut attrs = Map::new();
            for (k, v) in &e.attrs {
                attrs.insert(
                    k.clone(),
                    match v {
                        Attr::Bool(b) => json!(b),
                        Attr::Int(n) => json!(n),
                        Attr::Long(n) => json!(n),
                        Attr::Entity(n) => json!(format!("e{n}")),
                    },
                );
            }
            s.insert("attrs".into(), Value::Object(attrs));
            s.insert("attacking".into(), json!(e.attacking));
            s.insert("victim".into(), opt(e.victim.as_ref()));
            s.insert("nextAttack".into(), json!(e.next_attack));
            s.insert("lastSent".into(), json!(e.last_sent));
            s.insert("lastReceived".into(), json!(e.last_received));
            s.insert("location".into(), coord(&Host::location(self, &i)));
            if let Some(b) = &e.brain {
                let f = b.fields.read();
                s.insert("walkRadius".into(), json!(f.radius));
                s.insert("nextWalk".into(), json!(f.next_walk));
                s.insert("respawn".into(), json!(f.respawn_tick));
                s.insert("respawning".into(), json!(f.respawning));
                s.insert("aggressive".into(), json!(f.aggressive));
                s.insert("returning".into(), json!(f.returning));
                s.insert("returnPulse".into(), opt(f.return_pulse.as_ref()));
                s.insert("lastTalk".into(), json!(f.last_force_talk));
                let ai = e.ai.as_ref().unwrap();
                s.insert("invisible".into(), json!(ai.invisible));
                s.insert(
                    "handler".into(),
                    match &ai.handler {
                        None => Value::Null,
                        Some(h) => {
                            let f = h.0.read();
                            let wilderness = same_arc(&f.behaviour, &self.wilderness);
                            json!({
                                "radius": f.radius,
                                "pause": f.pause_ticks,
                                "switching": f.target_switching,
                                "chance": f.chance_ratio,
                                "tolerance": f.tolerance_length,
                                "allowTolerance": f.allow_tolerance,
                                "behaviour": if wilderness { "wilderness" } else { "default" },
                            })
                        }
                    },
                );
                s.insert("behaviour".into(), json!(self.behaviour_tag(&ai.behaviour)));
            }
            list.push(Value::Object(s));
        }
        json!({"ticks": self.ticks, "now": self.now, "nextPulse": self.next_pulse, "entities": list})
    }

    fn behaviour_tag(&self, b: &Arc<dyn Behaviour<World>>) -> String {
        self.tags
            .iter()
            .find(|(r, _)| same_arc(r, b))
            .map_or_else(|| "default".into(), |(_, tag)| tag.clone())
    }

    fn register(&mut self, ids: &[i32], tag: &str) {
        let b: Arc<dyn Behaviour<World>> = Arc::new(Recording { tag: tag.into() });
        self.tags.push((b.clone(), tag.into()));
        self.registry.register(ids, b);
    }
}
/// A registered content behaviour: every hook records and answers from the case.
struct Recording {
    tag: String,
}

fn script_state(w: &mut World, state: &mut BattleState<World>) {
    if let Some(Value::Object(m)) = w.answer("behaviour_state") {
        if let Some(v) = m.get("estimated") {
            state.estimated_hit = int(v);
        }
        if let Some(v) = m.get("secondary") {
            state.secondary_hit = int(v);
        }
    }
}

impl Recording {
    fn hook(&self, w: &mut World, name: &str, args: Vec<Value>) -> R<Option<Value>> {
        let mut row = vec![json!(self.tag), json!(name)];
        row.extend(args);
        w.emit("behaviour_hook", row)?;
        Ok(w.answer(&format!("behaviour_hook:{name}")))
    }
}

impl Behaviour<World> for Recording {
    fn tick(&self, w: &mut World, npc: &usize) -> R<bool> {
        Ok(self.hook(w, "tick", vec![json!(npc)])? != Some(json!(false)))
    }
    fn before_damage_received(
        &self,
        w: &mut World,
        npc: &usize,
        attacker: &usize,
        state: &mut BattleState<World>,
    ) -> R<()> {
        self.hook(w, "beforeDamageReceived", vec![json!(npc), json!(attacker)])?;
        script_state(w, state);
        Ok(())
    }
    fn after_damage_received(
        &self,
        w: &mut World,
        npc: &usize,
        attacker: &usize,
        state: &mut BattleState<World>,
    ) -> R<()> {
        self.hook(w, "afterDamageReceived", vec![json!(npc), json!(attacker)])?;
        script_state(w, state);
        Ok(())
    }
    fn before_attack_finalized(
        &self,
        w: &mut World,
        npc: &usize,
        victim: &usize,
        _: &mut BattleState<World>,
    ) -> R<()> {
        self.hook(w, "beforeAttackFinalized", vec![json!(npc), json!(victim)])
            .map(drop)
    }
    fn on_removal(&self, w: &mut World, npc: &usize) -> R<()> {
        self.hook(w, "onRemoval", vec![json!(npc)]).map(drop)
    }
    fn on_creation(&self, w: &mut World, npc: &usize) -> R<()> {
        self.hook(w, "onCreation", vec![json!(npc)]).map(drop)
    }
    fn on_respawn(&self, w: &mut World, npc: &usize) -> R<()> {
        self.hook(w, "onRespawn", vec![json!(npc)]).map(drop)
    }
    fn on_death_started(&self, w: &mut World, npc: &usize, killer: &usize) -> R<()> {
        self.hook(w, "onDeathStarted", vec![json!(npc), json!(killer)])
            .map(drop)
    }
    fn on_death_finished(&self, w: &mut World, npc: &usize, killer: &usize) -> R<()> {
        self.hook(w, "onDeathFinished", vec![json!(npc), json!(killer)])
            .map(drop)
    }
    fn on_drop_table_rolled(
        &self,
        w: &mut World,
        npc: &usize,
        killer: &usize,
        _: &mut Vec<RolledItem>,
    ) -> R<()> {
        self.hook(w, "onDropTableRolled", vec![json!(npc), json!(killer)])
            .map(drop)
    }
    fn can_be_attacked_by(
        &self,
        w: &mut World,
        npc: &usize,
        attacker: &usize,
        style: CombatStyle,
        message: bool,
    ) -> R<bool> {
        Ok(self.hook(
            w,
            "canBeAttackedBy",
            vec![
                json!(npc),
                json!(attacker),
                json!(style_name(style)),
                json!(message),
            ],
        )? != Some(json!(false)))
    }
    fn should_ignore_multi_restrictions(
        &self,
        w: &mut World,
        npc: &usize,
        victim: &usize,
    ) -> R<bool> {
        Ok(self.hook(
            w,
            "shouldIgnoreMultiRestrictions",
            vec![json!(npc), json!(victim)],
        )? == Some(json!(true)))
    }
    fn swing_handler_override(&self, w: &mut World, npc: &usize, original: String) -> R<String> {
        let answer = self.hook(
            w,
            "getSwingHandlerOverride",
            vec![json!(npc), json!(original)],
        )?;
        Ok(match answer {
            Some(Value::String(s)) => s,
            _ => original,
        })
    }
    fn xp_multiplier(&self, w: &mut World, npc: &usize, attacker: &usize) -> R<f64> {
        self.hook(w, "getXpMultiplier", vec![json!(npc), json!(attacker)])?;
        Ok(2.5)
    }
}

/// The oracle's RecordingNpc: a content subclass.
struct RecordingClass;

fn class_hook(
    w: &mut World,
    name: &str,
    npc: usize,
    other: Option<usize>,
    fallback: bool,
) -> R<bool> {
    w.emit(
        "class_hook",
        vec![json!(name), json!(npc), opt(other.as_ref())],
    )?;
    Ok(match w.answer(&format!("class_hook:{name}")) {
        Some(v) => v == json!(true),
        None => fallback,
    })
}

impl NpcClass<World> for RecordingClass {
    fn can_select_target(&self, w: &mut World, npc: &usize, target: &usize) -> R<bool> {
        class_hook(w, "canSelectTarget", *npc, Some(*target), true)
    }
    fn is_ignore_attack_restrictions(&self, w: &mut World, npc: &usize, victim: &usize) -> R<bool> {
        class_hook(w, "isIgnoreAttackRestrictions", *npc, Some(*victim), false)
    }
    fn on_respawn(&self, w: &mut World, npc: &usize) -> R<()> {
        class_hook(w, "onRespawn", *npc, None, false).map(drop)
    }
}

/// `entity_regions::Context` for `Region.isTolerated`: only the clock and
/// the player's name are reached.
struct Tolerance<'a> {
    w: &'a mut World,
    player: usize,
}

impl entity_regions::Context for Tolerance<'_> {
    fn chunk(&mut self, _: Location) -> entity_regions::Result<HandleChunk> {
        Err(entity_regions::Error::Null)
    }
    fn region(&mut self, _: Location) -> entity_regions::Result<region::Handle> {
        Err(entity_regions::Error::Null)
    }
    fn kind(&mut self, _: &NodeHandle) -> entity_regions::Result<RegionKind> {
        Err(entity_regions::Error::Null)
    }
    fn location(&mut self, _: &NodeHandle) -> entity_regions::Result<Option<Location>> {
        Err(entity_regions::Error::Null)
    }
    fn as_player(&mut self, _: &NodeHandle) -> entity_regions::Result<NodeHandle> {
        Err(entity_regions::Error::Null)
    }
    fn name(&mut self, _: &NodeHandle) -> entity_regions::Result<String> {
        let p = self.player;
        self.w
            .emit("name", vec![json!(p)])
            .map_err(|_| entity_regions::Error::Binding("injected".into()))?;
        Ok(self.w.ents[p].name.clone())
    }
    fn flag_active(&mut self, _: region::Handle) -> entity_regions::Result<()> {
        Err(entity_regions::Error::Null)
    }
    fn check_inactive(&mut self, _: region::Handle) -> entity_regions::Result<()> {
        Err(entity_regions::Error::Null)
    }
    fn footprint(&mut self, _: &NodeHandle) -> entity_regions::Result<Option<Location>> {
        Err(entity_regions::Error::Null)
    }
    fn zone_update(&mut self, _: &NodeHandle, _: Option<Location>) -> entity_regions::Result<()> {
        Err(entity_regions::Error::Null)
    }
    fn unknown_entity(&mut self, _: &NodeHandle) -> entity_regions::Result<()> {
        Err(entity_regions::Error::Null)
    }
    fn now_millis(&mut self) -> entity_regions::Result<i64> {
        self.w
            .current_time_millis()
            .map_err(|_| entity_regions::Error::Binding("injected".into()))
    }
}

impl Host for World {
    type Handler = String;
    type Region = Region;

    fn kind(&self, e: &usize) -> Kind {
        self.ents[*e].kind
    }
    fn same(&self, a: &usize, b: &usize) -> bool {
        a == b
    }
    fn ai(&mut self, npc: &usize) -> Option<&mut Ai<Self>> {
        self.ents[*npc].ai.as_mut()
    }
    fn brain(&self, npc: &usize) -> Option<npc::State> {
        self.ents[*npc].brain.clone()
    }
    fn location(&self, e: &usize) -> Option<Location> {
        match &self.ents[*e].brain {
            Some(b) => b.fields.read().location.clone(),
            None => self.ents[*e].location.clone(),
        }
    }
    fn set_location(&mut self, npc: &usize, location: Option<Location>) {
        self.ents[*npc]
            .brain
            .as_ref()
            .unwrap()
            .fields
            .write()
            .location = location;
    }
    fn style_handler(&self, style: CombatStyle) -> String {
        style_name(style).into()
    }
    fn region_dynamic(&self, region: &Region) -> bool {
        region.dynamic
    }
    fn max_players(&self) -> i32 {
        2000
    }

    fn ticks(&mut self) -> R<i32> {
        self.emit("ticks", vec![])?;
        Ok(self.ticks)
    }
    fn current_time_millis(&mut self) -> R<i64> {
        self.emit("current_time_millis", vec![])?;
        Ok(self.now)
    }
    fn next_int(&mut self, bound: i32) -> R<i32> {
        let v = self.random.next_bound(bound).unwrap();
        self.emit("next_int", vec![json!(bound), json!(v)])?;
        Ok(v)
    }
    fn math_random(&mut self) -> R<f64> {
        let v = self.math.next_double();
        self.emit("math_random", vec![json!(format!("{:x}", v.to_bits()))])?;
        Ok(v)
    }
    fn is_active(&mut self, e: &usize) -> R<bool> {
        self.flag("is_active", *e, true)
    }
    fn is_dead(&mut self, e: &usize) -> R<bool> {
        self.flag("is_dead", *e, false)
    }
    fn interaction_locked(&mut self, e: &usize) -> R<bool> {
        self.flag("interaction_locked", *e, false)
    }
    fn attribute_bool(&mut self, e: &usize, key: &str, default: bool) -> R<bool> {
        Ok(match self.attr(*e, key)? {
            None => default,
            Some(Attr::Bool(b)) => b,
            Some(other) => panic!("attribute type {other:?}"),
        })
    }
    fn attribute_long(&mut self, e: &usize, key: &str, default: i64) -> R<i64> {
        Ok(match self.attr(*e, key)? {
            None => default,
            Some(Attr::Long(n)) => n,
            Some(other) => panic!("attribute type {other:?}"),
        })
    }
    fn attribute_entity(&mut self, e: &usize, key: &str) -> R<Option<usize>> {
        Ok(match self.attr(*e, key)? {
            None => None,
            Some(Attr::Entity(n)) => Some(n),
            Some(other) => panic!("attribute type {other:?}"),
        })
    }
    fn set_attribute_entity(&mut self, e: &usize, key: &str, value: &usize) -> R<()> {
        self.ents[*e].attrs.insert(key.into(), Attr::Entity(*value));
        self.emit("set_attribute", vec![json!(e), json!(key)])
    }
    fn remove_attribute(&mut self, e: &usize, key: &str) -> R<()> {
        self.ents[*e].attrs.remove(key);
        self.emit("remove_attribute", vec![json!(e), json!(key)])
    }
    fn config_bool(&mut self, npc: &usize, key: &str, default: bool) -> R<bool> {
        self.emit("config", vec![json!(npc), json!(key)])?;
        Ok(match self.answer(&format!("config:{npc}:{key}")) {
            None | Some(Value::Null) => default,
            Some(v) => v == json!(true),
        })
    }
    fn config_int(&mut self, npc: &usize, key: &str, default: i32) -> R<i32> {
        self.emit("config", vec![json!(npc), json!(key)])?;
        Ok(match self.answer(&format!("config:{npc}:{key}")) {
            None | Some(Value::Null) => default,
            Some(v) => int(&v),
        })
    }
    fn definition_combat_level(&mut self, npc: &usize) -> R<i32> {
        self.number("definition_combat_level", *npc, 1)
    }
    fn has_action(&mut self, npc: &usize, action: &str) -> R<bool> {
        self.emit("has_action", vec![json!(npc), json!(action)])?;
        Ok(self
            .answer(&format!("has_action:{npc}:{action}"))
            .is_none_or(|v| v == json!(true)))
    }
    fn shown_npc(&mut self, npc: &usize, player: &usize) -> R<usize> {
        self.emit("shown_npc", vec![json!(npc), json!(player)])?;
        Ok(self
            .answer(&format!("shown_npc:{npc}"))
            .and_then(|v| v.as_u64())
            .map_or(*npc, |v| v as usize))
    }
    fn is_attacking(&mut self, e: &usize) -> R<bool> {
        self.emit("is_attacking", vec![json!(e)])?;
        Ok(self.ents[*e].attacking)
    }
    fn combat_victim(&mut self, e: &usize) -> R<Option<usize>> {
        self.emit("combat_victim", vec![json!(e)])?;
        Ok(self.ents[*e].victim)
    }
    fn combat_set_victim(&mut self, e: &usize, victim: &usize) -> R<()> {
        self.emit("combat_set_victim", vec![json!(e), json!(victim)])?;
        self.ents[*e].victim = Some(*victim);
        Ok(())
    }
    fn combat_attack(&mut self, e: &usize, victim: Option<&usize>) -> R<()> {
        self.emit("combat_attack", vec![json!(e), opt(victim)])?;
        if let Some(v) = victim
            && self.sticky()
        {
            self.ents[*e].attacking = true;
            self.ents[*e].victim = Some(*v);
        }
        Ok(())
    }
    fn combat_stop(&mut self, e: &usize) -> R<()> {
        self.emit("combat_stop", vec![json!(e)])?;
        self.ents[*e].attacking = false;
        self.ents[*e].victim = None;
        Ok(())
    }
    fn next_attack(&mut self, e: &usize) -> R<i32> {
        self.emit("next_attack", vec![json!(e)])?;
        Ok(self.ents[*e].next_attack)
    }
    fn combat_style(&mut self, e: &usize) -> R<Option<CombatStyle>> {
        self.emit("combat_style", vec![json!(e)])?;
        Ok(self.ents[*e].style)
    }
    fn set_last_received_attack(&mut self, e: &usize, tick: i32) -> R<()> {
        self.emit("set_last_received_attack", vec![json!(e), json!(tick)])?;
        self.ents[*e].last_received = tick;
        Ok(())
    }
    fn last_sent_attack(&mut self, e: &usize) -> R<i32> {
        self.emit("last_sent_attack", vec![json!(e)])?;
        Ok(self.ents[*e].last_sent)
    }
    fn set_last_sent_attack(&mut self, e: &usize, tick: i32) -> R<()> {
        self.emit("set_last_sent_attack", vec![json!(e), json!(tick)])?;
        self.ents[*e].last_sent = tick;
        Ok(())
    }
    fn face(&mut self, e: &usize, target: Option<&usize>) -> R<()> {
        self.emit("face", vec![json!(e), opt(target)])
    }
    fn is_multi_zone(&mut self, e: &usize) -> R<bool> {
        self.flag("is_multi_zone", *e, false)
    }
    fn current_combat_level(&mut self, e: &usize) -> R<i32> {
        self.number("current_combat_level", *e, 3)
    }
    fn is_retaliating(&mut self, e: &usize) -> R<bool> {
        self.flag("is_retaliating", *e, true)
    }
    fn set_npc_walkable(&mut self, npc: &usize, value: bool) -> R<()> {
        self.emit("set_npc_walkable", vec![json!(npc), json!(value)])
    }
    fn set_combat_time_out(&mut self, npc: &usize, value: i32) -> R<()> {
        self.emit("set_combat_time_out", vec![json!(npc), json!(value)])
    }
    fn local_players(&mut self, location: Location, radius: i32) -> R<Vec<usize>> {
        self.emit(
            "local_players",
            vec![json!(location.coordinates()), json!(radius)],
        )?;
        Ok(match self.answer("local_players") {
            None | Some(Value::Null) => self.players(),
            Some(v) => v
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap() as usize)
                .collect(),
        })
    }
    fn wilderness_in_zone(&mut self, e: &usize) -> R<bool> {
        self.flag("wilderness_in_zone", *e, false)
    }
    fn region_for_id(&mut self, region_id: i32) -> R<Region> {
        self.emit("region_for_id", vec![json!(region_id)])?;
        Ok(self.region(region_id))
    }
    fn is_tolerated(&mut self, region: &Region, player: &usize) -> R<bool> {
        let node = self.ents[*player].node.clone();
        let handle = region.handle.clone();
        let mut ctx = Tolerance {
            w: self,
            player: *player,
        };
        entity_regions::is_tolerated(&mut ctx, &handle, &node).map_err(|e| match e {
            entity_regions::Error::Null => cs::Error::Null,
            entity_regions::Error::Binding(t) => cs::Error::Binding(t),
            other => cs::Error::Binding(format!("{other:?}")),
        })
    }
    fn rights(&mut self, p: &usize) -> R<Option<Rights>> {
        self.emit("rights", vec![json!(p)])?;
        Ok(
            match self
                .answer(&format!("rights:{p}"))
                .unwrap_or(json!("REGULAR_PLAYER"))
            {
                Value::Null => None,
                Value::String(s) if s == "ADMINISTRATOR" => Some(Rights::Administrator),
                Value::String(s) if s == "PLAYER_MODERATOR" => Some(Rights::PlayerModerator),
                _ => Some(Rights::RegularPlayer),
            },
        )
    }
    fn player_invisible(&mut self, p: &usize) -> R<bool> {
        self.flag("player_invisible", *p, false)
    }
    fn skull_wilderness(&mut self, p: &usize) -> R<bool> {
        self.flag("skull_wilderness", *p, false)
    }
    fn wilderness_protection(&mut self, p: &usize) -> R<bool> {
        self.flag("wilderness_protection", *p, false)
    }
    fn settings(&mut self) -> R<bool> {
        self.emit("settings", vec![])?;
        Ok(self.answer("settings").is_none_or(|v| v == json!(true)))
    }
    fn wild_pvp_enabled(&mut self) -> R<bool> {
        self.emit("wild_pvp_enabled", vec![])?;
        Ok(self.answer("wild_pvp_enabled") == Some(json!(true)))
    }
    fn zone_continue_attack(
        &mut self,
        e: &usize,
        target: &usize,
        style: Option<CombatStyle>,
        message: bool,
    ) -> R<bool> {
        self.emit(
            "zone_continue_attack",
            vec![
                json!(e),
                json!(target),
                json!(style.map(style_name)),
                json!(message),
            ],
        )?;
        Ok(self
            .answer(&format!("zone_continue_attack:{e}"))
            .is_none_or(|v| v == json!(true)))
    }
    fn zone_is_pvpable(
        &mut self,
        owner: &usize,
        attacker: &usize,
        victim: &usize,
        style: Option<CombatStyle>,
        message: bool,
    ) -> R<bool> {
        self.emit(
            "zone_is_pvpable",
            vec![
                json!(owner),
                json!(attacker),
                json!(victim),
                json!(style.map(style_name)),
                json!(message),
            ],
        )?;
        Ok(self.answer(&format!("zone_is_pvpable:{owner}")) == Some(json!(true)))
    }
    fn level(&mut self, e: &usize, slot: i32) -> R<i32> {
        self.emit("level", vec![json!(e), json!(slot)])?;
        Ok(self.answer(&format!("level:{e}")).map_or(1, |v| int(&v)))
    }
    fn familiar_owner(&mut self, f: &usize) -> R<Option<usize>> {
        self.emit("familiar_owner", vec![json!(f)])?;
        Ok(self
            .answer(&format!("familiar_owner:{f}"))
            .and_then(|v| v.as_u64())
            .map(|v| v as usize))
    }
    fn send_message(&mut self, p: &usize, text: &str) -> R<()> {
        self.emit("send_message", vec![json!(p), json!(text)])
    }
    fn path_find(&mut self, mover: Option<&usize>, destination: Option<Location>) -> R<()> {
        self.emit("path_find", vec![opt(mover), coord(&destination)])
    }
    fn path_walk(&mut self, mover: Option<&usize>) -> R<()> {
        self.emit("path_walk", vec![opt(mover)])
    }
    fn move_step(&mut self, e: &usize) -> R<()> {
        self.emit("move_step", vec![json!(e)])
    }
    fn walking_has_path(&mut self, e: &usize) -> R<bool> {
        self.flag("walking_has_path", *e, false)
    }
    fn pulse_manager_is_moving(&mut self, e: &usize) -> R<bool> {
        self.flag("pulse_manager_is_moving", *e, false)
    }
    fn ring_id(&mut self, p: &usize) -> R<i32> {
        self.emit("equipment_new", vec![json!(p), json!(12)])?;
        Ok(self
            .answer(&format!("equipment_new:{p}"))
            .map_or(0, |v| int(&v)))
    }
    fn send_chat(&mut self, e: &usize, text: &str) -> R<()> {
        self.emit("send_chat", vec![json!(e), json!(text)])
    }
    fn manual_hit(&mut self, target: &usize, source: &usize, hit: i32, kind: Hitsplat) -> R<()> {
        assert_eq!(kind, Hitsplat::Normal);
        self.emit(
            "manual_hit",
            vec![json!(target), json!(source), json!(hit), json!("NORMAL")],
        )
    }
    fn handle_recoil_effect(&mut self, p: &usize, attacker: Option<&usize>, hit: i32) -> R<()> {
        self.emit(
            "handle_recoil_effect",
            vec![json!(p), opt(attacker), json!(hit)],
        )
    }
    fn check_armour_degrades(&mut self, p: &usize) -> R<()> {
        self.emit("check_armour_degrades", vec![json!(p)])
    }
    fn walking_reset(&mut self, e: &usize) -> R<()> {
        self.emit("walking_reset", vec![json!(e)])
    }
    fn pulse_manager_clear(&mut self, e: &usize) -> R<()> {
        self.emit("pulse_manager_clear", vec![json!(e)])
    }
    fn masks_reset(&mut self, e: &usize) -> R<()> {
        self.emit("masks_reset", vec![json!(e)])
    }
    fn chunk_remove(&mut self, npc: &usize, location: Location) -> R<()> {
        self.emit(
            "chunk_remove",
            vec![json!(npc), json!(location.coordinates())],
        )
    }
    fn chunk_add(&mut self, npc: &usize, location: Location) -> R<()> {
        self.emit("chunk_add", vec![json!(npc), json!(location.coordinates())])
    }
    fn return_arrival(&mut self, npc: &usize) -> R<bool> {
        let brain = self.ents[*npc].brain.clone().unwrap();
        let mut ctx = TickCtx { w: self, npc: *npc };
        brain.return_arrival(&mut ctx).map_err(from_npc)
    }
    fn pulse_stop(&mut self, npc: &usize, pulse: usize) -> R<()> {
        self.emit("pulse_stop", vec![json!(npc), json!(pulse)])
    }
    fn remove_renderable(&mut self, npc: &usize) -> R<()> {
        self.emit("remove_renderable", vec![json!(npc)])
    }
    fn npc_clear(&mut self, npc: &usize) -> R<()> {
        self.emit("npc_clear", vec![json!(npc)])
    }
    fn skills_restore(&mut self, e: &usize) -> R<()> {
        self.emit("skills_restore", vec![json!(e)])
    }
    fn remove_timer(&mut self, e: &usize, name: &str) -> R<()> {
        self.emit("remove_timer", vec![json!(e), json!(name)])
    }
    fn clipping_flag(&mut self, z: i32, x: i32, y: i32) -> R<i32> {
        self.emit("clipping_flag", vec![json!(z), json!(x), json!(y)])?;
        Ok(self.answer("clipping_flag").map_or(0, |v| int(&v)))
    }
    fn other_clipping_flag(&mut self, _: u32, _: i32, _: i32, _: i32) -> R<i32> {
        Err(cs::Error::Binding(
            "no content supplier in this corpus".into(),
        ))
    }
}

/// `world::npc::Context` for the subject NPC: this module's queries and
/// events go to `npc_behaviour::tick`, the rest record like the stand-ins.
struct TickCtx<'a> {
    w: &'a mut World,
    npc: usize,
}

impl TickCtx<'_> {
    fn emit(&mut self, name: &str, args: Vec<Value>) -> npc::Result<()> {
        self.w
            .emit(name, args)
            .map_err(|e| npc::Error::Binding(format!("{e:?}")))
    }
    fn flag(&mut self, name: &str, fallback: bool) -> npc::Result<bool> {
        let npc = self.npc;
        self.w
            .flag(name, npc, fallback)
            .map_err(|e| npc::Error::Binding(format!("{e:?}")))
    }
}

impl npc::Context for TickCtx<'_> {
    fn ticks(&mut self, _: &npc::State) -> npc::Result<i32> {
        self.w.ticks().map_err(tick::npc_error)
    }
    fn region_active(&mut self, _: &npc::State, location: Location) -> npc::Result<bool> {
        let id = aggression::region_id(location);
        self.emit("region_for_id", vec![json!(id)])?;
        self.emit("region_active", vec![json!(id)])?;
        Ok(self
            .w
            .answer(&format!("region_active:{id}"))
            .is_none_or(|v| v == json!(true)))
    }
    fn query(&mut self, _: &npc::State, query: Query) -> npc::Result<bool> {
        let npc = self.npc;
        if let Some(answer) = tick::query(self.w, &npc, query).map_err(tick::npc_error)? {
            return Ok(answer);
        }
        match query {
            Query::MovementLocked => self.flag("movement_locked", false),
            Query::PulseRunning => self.flag("has_pulse_running", false),
            Query::InCombat => self.flag("is_in_combat", false),
            Query::WaterNpc => {
                self.emit("water_npc", vec![json!(npc)])?;
                Ok(self.w.answer(&format!("water_npc:{npc}")) == Some(json!(true)))
            }
            other => panic!("unexpected query {other:?}"),
        }
    }
    fn event(&mut self, _: &npc::State, event: Event) -> npc::Result<()> {
        let npc = self.npc;
        if tick::event(self.w, &npc, &event).map_err(tick::npc_error)? {
            return Ok(());
        }
        match event {
            Event::Teleport(l) => self.emit("teleport", vec![json!(npc), coord(&l)]),
            Event::EntityTick => self.emit("entity_tick", vec![json!(npc)]),
            Event::LockMovement(n) => self.emit("lock_movement", vec![json!(npc), json!(n)]),
            Event::UnlockMovement => self.emit("unlock_movement", vec![json!(npc)]),
            Event::ImpactDisabled(n) => self.emit("impact_disabled", vec![json!(npc), json!(n)]),
            Event::Attribute(key) => match key.strip_prefix("remove:") {
                Some(key) => self.emit("remove_attribute", vec![json!(npc), json!(key)]),
                None => self.emit("set_attribute", vec![json!(npc), json!(key)]),
            },
            Event::SubmitReturn(id) => self.emit("submit_return", vec![json!(id)]),
            Event::StopCombat => self.w.combat_stop(&npc).map_err(tick::npc_error),
            Event::Chat(text) => self.emit(
                "send_chat",
                vec![
                    json!(npc),
                    json!(text.map(|t| String::from_utf16(&t).unwrap())),
                ],
            ),
            other => panic!("unexpected event {other:?}"),
        }
    }
    fn pulse_running(&mut self, _: &npc::State, pulse: usize) -> npc::Result<bool> {
        self.emit("return_running", vec![json!(pulse)])?;
        Ok(self
            .w
            .answer(&format!("return_running:{pulse}"))
            .is_none_or(|v| v == json!(true)))
    }
    fn return_pulse(
        &mut self,
        _: &npc::State,
        destination: Option<Location>,
    ) -> npc::Result<usize> {
        self.w.next_pulse += 1;
        let id = self.w.next_pulse;
        self.emit("construct_return", vec![json!(id), coord(&destination)])?;
        Ok(id)
    }
    fn dialogue_active(&mut self, _: &npc::State, _: usize) -> npc::Result<bool> {
        panic!("no dialogue player in this corpus")
    }
    fn dialogue_chatbox(&mut self, _: &npc::State, _: usize) -> npc::Result<bool> {
        panic!("no dialogue player in this corpus")
    }
    fn random_bool(&mut self, _: &npc::State) -> npc::Result<bool> {
        let b = self.w.random.next_bool();
        self.emit("next_bool", vec![json!(b)])?;
        Ok(b)
    }
    fn randomize(&mut self, _: &npc::State, bound: i32) -> npc::Result<i32> {
        if bound < 1 {
            return Ok(0);
        }
        self.w.next_int(bound).map_err(tick::npc_error)
    }
    fn random_range(&mut self, s: &npc::State, min: i32, max: i32) -> npc::Result<i32> {
        let n = max.wrapping_sub(min).wrapping_abs();
        Ok(min.min(max).wrapping_add(self.randomize(s, n)?))
    }
    fn can_move(&mut self, _: &npc::State, l: Option<Location>) -> npc::Result<bool> {
        let npc = self.npc;
        self.emit("can_move", vec![json!(npc), coord(&l)])?;
        Ok(self
            .w
            .answer(&format!("can_move:{npc}"))
            .is_none_or(|v| v == json!(true)))
    }
    fn walk(&mut self, _: &npc::State, l: Option<Location>, water: bool) -> npc::Result<()> {
        let npc = self.npc;
        self.emit("wander", vec![json!(npc), coord(&l), json!(water)])
    }
}

fn aggressive(name: &Value) -> Arc<dyn AggressiveBehaviour<World>> {
    if name == &json!("wilderness") {
        Arc::new(Wilderness)
    } else {
        Arc::new(DefaultAggression)
    }
}

fn battle(state: &BattleState<World>) -> Value {
    json!({"estimated": state.estimated_hit, "secondary": state.secondary_hit, "poison": state.poison_damage})
}

fn battle_state(m: &Value) -> BattleState<World> {
    let mut s = BattleState::new(
        m["attacker"].as_u64().map(|v| v as usize),
        m["victim"].as_u64().map(|v| v as usize),
    );
    if let Some(v) = m.get("estimated") {
        s.estimated_hit = int(v);
    }
    if let Some(v) = m.get("secondary") {
        s.secondary_hit = int(v);
    }
    if let Some(v) = m.get("poison") {
        s.poison_damage = int(v);
    }
    s.style = style(&m["style"]);
    s
}

fn handler(w: &mut World, e: usize) -> R<Handler<World>> {
    w.ents[e]
        .ai
        .as_ref()
        .unwrap()
        .handler
        .clone()
        .ok_or(cs::Error::Null)
}

fn hook(w: &mut World, e: usize, op: &Value) -> R<Value> {
    let b = nb::behaviour(w, &e)?;
    let other = op["other"].as_u64().map(|v| v as usize);
    let need = || other.ok_or(cs::Error::Null);
    Ok(match op["name"].as_str().unwrap() {
        "tick" => json!(b.tick(w, &e)?),
        "canBeAttackedBy" => {
            let o = need()?;
            let s = style(&op["style"]).ok_or(cs::Error::Null)?;
            json!(b.can_be_attacked_by(w, &e, &o, s, op["message"] == json!(true))?)
        }
        "shouldIgnoreMultiRestrictions" => {
            json!(b.should_ignore_multi_restrictions(w, &e, &need()?)?)
        }
        "getXpMultiplier" => json!(format!("{:x}", b.xp_multiplier(w, &e, &need()?)?.to_bits())),
        "getPathfinderOverride" => match b.pathfinder_override(w, &e)? {
            Some(_) => json!("override"),
            None => Value::Null,
        },
        "clip" => {
            let supplier = b.clipping_supplier(w, &e)?.ok_or(cs::Error::Null)?;
            json!(supplier.clipping_flag(w, int(&op["z"]), int(&op["x"]), int(&op["y"]))?)
        }
        "onCreation" => {
            b.on_creation(w, &e)?;
            Value::Null
        }
        "onRemoval" => {
            b.on_removal(w, &e)?;
            Value::Null
        }
        "onRespawn" => {
            b.on_respawn(w, &e)?;
            Value::Null
        }
        "onDeathStarted" => {
            b.on_death_started(w, &e, &need()?)?;
            Value::Null
        }
        "onDeathFinished" => {
            b.on_death_finished(w, &e, &need()?)?;
            Value::Null
        }
        "beforeAttackFinalized" => {
            let mut s = battle_state(op.get("state").unwrap_or(&json!({})));
            b.before_attack_finalized(w, &e, &need()?, &mut s)?;
            Value::Null
        }
        "onDropTableRolled" => {
            b.on_drop_table_rolled(w, &e, &need()?, &mut vec![])?;
            Value::Null
        }
        other => panic!("hook {other}"),
    })
}

fn run(w: &mut World, op: &Value) -> R<Value> {
    let e = op.get("entity").map_or(0, |v| v.as_u64().unwrap() as usize);
    let idx = |k: &str| op[k].as_u64().unwrap() as usize;
    let message = op["message"] == json!(true);
    Ok(match op["op"].as_str().unwrap() {
        "select" => json!(handler(w, e)?.select_target(w)?),
        "tick" => {
            let brain = w.ents[e].brain.clone().unwrap();
            brain.tick(&mut TickCtx { w, npc: e }).map_err(from_npc)?;
            Value::Null
        }
        "actions" => {
            let brain = w.ents[e].brain.clone().unwrap();
            brain
                .actions(&mut TickCtx { w, npc: e })
                .map_err(from_npc)?;
            Value::Null
        }
        "advance" => {
            let n = op.get("ticks").map_or(1, int);
            w.ticks = w.ticks.wrapping_add(n);
            w.now += op
                .get("millis")
                .map_or(600 * i64::from(n), |v| v.as_i64().unwrap());
            Value::Null
        }
        "set" => {
            w.apply(e, &op["fields"]);
            Value::Null
        }
        "attackable" => json!(combat::is_attackable(
            w,
            &idx("victim"),
            &idx("attacker"),
            style(&op["style"]),
            message
        )?),
        "continue" => {
            let target = idx("target");
            json!(match w.ents[e].kind {
                Kind::Player =>
                    combat::player_continue_attack(w, &e, &target, style(&op["style"]), message)?,
                Kind::Npc | Kind::Familiar =>
                    combat::npc_continue_attack(w, &e, &target, style(&op["style"]), message)?,
                Kind::Other => true,
            })
        }
        "check_impact" => {
            let mut s = battle_state(&op["state"]);
            combat::check_impact(w, &e, &mut s)?;
            battle(&s)
        }
        "on_impact" => {
            let mut s = battle_state(&op["state"]);
            let attacker = op["attacker"].as_u64().map(|v| v as usize);
            combat::on_impact(w, &e, attacker.as_ref(), &mut s)?;
            battle(&s)
        }
        "send_impact" => {
            combat::send_impact(w, &e)?;
            Value::Null
        }
        "in_combat" => json!(combat::in_combat(w, &e)?),
        "invisible" => json!(combat::is_invisible(w, &e)?),
        "hidden" => json!(combat::npc_is_hidden(w, &e, &idx("player"))?),
        "swing_handler" => json!(combat::npc_swing_handler(w, &e)?),
        "ignore_multi" => json!(combat::is_ignore_multi_boundaries(w, &e, &idx("victim"))?),
        "inactivity" => {
            lifecycle::on_region_inactivity(w, &e)?;
            Value::Null
        }
        "restore" => {
            lifecycle::full_restore(w, &e)?;
            Value::Null
        }
        "default_behavior" => {
            aggression::set_default_behaviour(w, &e)?;
            Value::Null
        }
        "boss" => {
            aggression::configure_boss_data(w, &e)?;
            Value::Null
        }
        "can_select" => {
            json!(aggressive(&op["behaviour"]).can_select_target(w, &e, &idx("target"))?)
        }
        "priority" => json!(DefaultAggression.priority_flag(w, &idx("target"))?),
        "possible" => {
            json!(aggressive(&op["behaviour"]).possible_targets(w, &e, int(&op["radius"]))?)
        }
        "logical" => {
            let targets: Vec<usize> = op["targets"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap() as usize)
                .collect();
            json!(DefaultAggression.logical_target(w, &e, &targets)?)
        }
        "allow_tolerance" => json!(handler(w, e)?.is_allow_tolerance(w)?),
        "pause" => {
            handler(w, e)?.set_pause_ticks(w, int(&op["ticks"]))?;
            Value::Null
        }
        "remove_tolerance" => {
            handler(w, e)?.remove_tolerance(int(&op["index"]))?;
            Value::Null
        }
        "hook" => hook(w, e, op)?,
        "register" => {
            let ids: Vec<i32> = op["ids"].as_array().unwrap().iter().map(int).collect();
            w.register(&ids, op["tag"].as_str().unwrap());
            Value::Null
        }
        "for_id" => {
            let b = w.registry.for_id(int(&op["id"]));
            json!(w.behaviour_tag(&b))
        }
        "refresh_behavior" => {
            let id = w.ents[e].id;
            let b = w.registry.for_id(id);
            w.ents[e].ai.as_mut().unwrap().behaviour = b.clone();
            json!(w.behaviour_tag(&b))
        }
        other => panic!("op {other}"),
    })
}

/// Replays one case and returns its expected rows.
pub fn evaluate(case: &Value) -> Value {
    let input = &case["input"];
    let mut w = World::new(input);
    let mut rows = vec![];
    for op in input["ops"].as_array().unwrap() {
        w.events.clear();
        w.calls = 0;
        w.fail_at = op.get("failAt").map_or(-1, |v| v.as_i64().unwrap());
        let (result, error) = match run(&mut w, op) {
            Ok(v) => (v, Value::Null),
            Err(e) => (Value::Null, json!(class(&e))),
        };
        w.fail_at = -1;
        rows.push(json!({
            "result": result,
            "error": error,
            "events": w.events.clone(),
            "state": w.snapshot(),
        }));
    }
    Value::Array(rows)
}
