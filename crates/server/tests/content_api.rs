//! Replays fixtures/content-api (the original core/api compiled unchanged on
//! JDK 11 over recording adapters) against world::content_api: every
//! recorded call, result, error, follow-up step and state snapshot.
use rs09_server::{
    definitions::{item::Shared, varbit},
    player::variables::{self, Saved, Values},
    world::{
        content_api::{
            self as api, Anim, Arg, Container, EquipmentSlot, Error, GameTab, Gfx, God, Holder,
            Host, InputType, ItemDef, Kind, Location, Member, Path, Priority, Pulse, PulseBody,
            PulseTarget, QueuedScript, Reply, ReplyKind, Requirement, ScriptBody, SkillHandler,
            Value, api_files as files, items, quests, services, state, ui, vars, visuals, world,
        },
        global_items2::random::ThreadRandom,
        player_walk::{NodeHandle, State},
    },
};
use serde_json::{Value as J, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};

const FIXTURE: &str = include_str!("../../../fixtures/content-api/oracle.jsonl");
const PROVENANCE: &str = include_str!("../../../fixtures/content-api/provenance.json");

type N = String;

#[derive(Clone, Default)]
struct NodeState {
    kind: String,
    id: i32,
    name: Option<String>,
    location: Option<Location>,
    size: i32,
    amount: i32,
    id_hash: i32,
    slot: i32,
    charge: i32,
    rotation: i32,
    male: bool,
    username: String,
    secondary: bool,
    clocks: Vec<i32>,
    invisible: bool,
    persist: bool,
    next_execution: i32,
    script_state: i32,
    severity: i32,
    source: Option<String>,
    ids: Vec<i32>,
    hash: i32,
    withdraw: Vec<Option<String>>,
    records: Option<Vec<Option<String>>>,
    listeners: Vec<String>,
}

struct T {
    nodes: BTreeMap<String, NodeState>,
    items: HashMap<i32, J>,
    varbits: HashMap<i32, [i32; 3]>,
    answers: HashMap<String, Vec<J>>,
    answer_calls: HashMap<String, usize>,
    events: Vec<J>,
    calls: i64,
    fail_at: i64,
    sequence: HashMap<String, i32>,
    ticks: i32,
    dev_mode: Option<Option<bool>>,
    max_pathfind: i32,
    toml: Option<String>,
    shops: HashMap<i32, String>,
    random: ThreadRandom,
    pulses: Vec<(String, Pulse<N>)>,
    scripts: Vec<(String, QueuedScript<N>)>,
    handlers: Vec<(String, SkillHandler<N>)>,
    zones: Vec<(String, N)>,
    quests: HashMap<String, String>,
    values: HashMap<String, (Values, Saved)>,
}

fn int(v: &J) -> i32 {
    v.as_i64().unwrap() as i32
}

fn loc(v: &J) -> Option<Location> {
    let l = v.get("loc")?.as_array()?;
    Some(Location::new(int(&l[0]), int(&l[1]), int(&l[2])))
}

/// Java `String.hashCode`.
fn java_hash(s: &str) -> i32 {
    s.encode_utf16()
        .fold(0i32, |h, c| h.wrapping_mul(31).wrapping_add(c as i32))
}

/// Keys of a json-simple JSONObject (a JDK HashMap<String, _>) in its walk order.
fn java_key_order(keys: &[String]) -> Vec<String> {
    let hashes: Vec<i32> = keys.iter().map(|k| java_hash(k)).collect();
    items::java_hash_order(&hashes)
        .into_iter()
        .map(|i| keys[i].clone())
        .collect()
}

impl T {
    fn new(case: &J, quests: &HashMap<String, String>) -> Self {
        let mut t = T {
            nodes: BTreeMap::new(),
            items: HashMap::new(),
            varbits: HashMap::new(),
            answers: HashMap::new(),
            answer_calls: HashMap::new(),
            events: Vec::new(),
            calls: 0,
            fail_at: case.get("failAt").map_or(0, |v| v.as_i64().unwrap()),
            sequence: HashMap::new(),
            ticks: 0,
            dev_mode: None,
            max_pathfind: 25,
            toml: None,
            shops: HashMap::new(),
            random: ThreadRandom::new(case.get("seed").map_or(532, |v| v.as_i64().unwrap())),
            pulses: Vec::new(),
            scripts: Vec::new(),
            handlers: Vec::new(),
            zones: Vec::new(),
            quests: quests.clone(),
            values: HashMap::new(),
        };
        let defs = &case["definitions"];
        if let Some(items) = defs.get("items").and_then(J::as_object) {
            for (k, v) in items {
                t.items.insert(k.parse().unwrap(), v.clone());
            }
        }
        if let Some(vb) = defs.get("varbits").and_then(J::as_object) {
            for (k, v) in vb {
                let a = v.as_array().unwrap();
                t.varbits
                    .insert(k.parse().unwrap(), [int(&a[0]), int(&a[1]), int(&a[2])]);
            }
        }
        if let Some(a) = case.get("answers").and_then(J::as_object) {
            for (k, v) in a {
                t.answers.insert(
                    k.clone(),
                    v.as_array().cloned().unwrap_or_else(|| vec![v.clone()]),
                );
            }
        }
        for (k, v) in case["nodes"].as_object().unwrap() {
            let kind = v["kind"].as_str().unwrap().to_string();
            let mut n = NodeState {
                kind: kind.clone(),
                size: 1,
                slot: -1,
                charge: 1000,
                clocks: vec![0; 8],
                ..Default::default()
            };
            n.id = v.get("id").map_or(0, int);
            n.location = v.get("location").and_then(loc);
            n.size = v.get("size").map_or(1, int);
            if kind == "item" || (kind == "grounditem" && v.get("id").is_some()) {
                let charge = v.get("charge").map_or(1000, int);
                n.id_hash = (n.id << 16) | charge;
                n.amount = v.get("amount").map_or(1, int);
                n.slot = v.get("slot").map_or(-1, int);
                n.name = t
                    .items
                    .get(&(n.id_hash >> 16 & 0xFFFF))
                    .and_then(|d| d["name"].as_str().map(String::from));
            } else if kind == "grounditem" {
                n.id_hash = (-1 << 16) | 1000;
                n.name = Some("null".into());
            } else {
                n.name = v.get("name").and_then(J::as_str).map(String::from);
            }
            n.rotation = v.get("rotation").map_or(0, int);
            n.charge = if kind == "scenery" {
                v.get("charge").map_or(1000, int)
            } else {
                n.charge
            };
            n.male = v.get("male").is_none_or(|m| m.as_bool().unwrap());
            n.username = v
                .get("username")
                .and_then(J::as_str)
                .unwrap_or(k)
                .to_string();
            n.secondary = v.get("secondary").is_some_and(|s| s.as_bool().unwrap());
            if let Some(c) = v.get("clocks") {
                n.clocks = c.as_array().unwrap().iter().map(int).collect();
            }
            n.invisible = v.get("invisible").is_some_and(|s| s.as_bool().unwrap());
            n.persist = v.get("persist").is_some_and(|s| s.as_bool().unwrap());
            n.next_execution = v.get("nextExecution").map_or(0, int);
            n.script_state = v.get("state").map_or(0, int);
            n.severity = v.get("severity").map_or(0, int);
            n.source = v.get("source").and_then(J::as_str).map(String::from);
            n.ids = v
                .get("ids")
                .map_or(vec![], |a| a.as_array().unwrap().iter().map(int).collect());
            n.hash = v.get("hash").map_or(0, int);
            n.withdraw = v.get("withdraw").map_or(vec![None, None], |a| {
                a.as_array()
                    .unwrap()
                    .iter()
                    .map(|x| x.as_str().map(String::from))
                    .collect()
            });
            n.records = v.get("records").map(|a| {
                a.as_array()
                    .unwrap()
                    .iter()
                    .enumerate()
                    .map(|(i, x)| {
                        if x.is_null() {
                            None
                        } else {
                            Some(format!("record:{i}"))
                        }
                    })
                    .collect()
            });
            n.listeners = v.get("listeners").map_or(vec![], |a| {
                a.as_array()
                    .unwrap()
                    .iter()
                    .map(|x| x.as_str().unwrap().to_string())
                    .collect()
            });
            if kind == "player" {
                let values = Values::default();
                let saved = Saved::default();
                if let Some(vm) = v.get("varps").and_then(J::as_object) {
                    let keys: Vec<String> = vm.keys().cloned().collect();
                    for key in java_key_order(&keys) {
                        values.put(key.parse().unwrap(), Some(int(&vm[&key])));
                    }
                }
                if let Some(sm) = v.get("saved").and_then(J::as_object) {
                    let keys: Vec<String> = sm.keys().cloned().collect();
                    for key in java_key_order(&keys) {
                        saved.put(key.parse().unwrap(), Some(sm[&key].as_bool().unwrap()));
                    }
                }
                t.values.insert(k.clone(), (values, saved));
            }
            t.nodes.insert(k.clone(), n);
        }
        let w = &case["world"];
        t.ticks = w.get("ticks").map_or(0, int);
        if let Some(d) = w.get("devMode") {
            t.dev_mode = Some(d.as_bool());
        }
        t.max_pathfind = w.get("maxPathfind").map_or(25, int);
        t.toml = w.get("toml").and_then(J::as_str).map(String::from);
        if let Some(s) = w.get("shops").and_then(J::as_object) {
            for (k, v) in s {
                t.shops
                    .insert(k.parse().unwrap(), v.as_str().unwrap().to_string());
            }
        }
        t
    }

    fn fresh(&mut self, kind: &str) -> String {
        let n = self.sequence.entry(kind.to_string()).or_insert(0);
        *n += 1;
        format!("{kind}#{n}")
    }

    fn node(&self, k: &str) -> &NodeState {
        self.nodes.get(k).unwrap_or_else(|| panic!("no node {k}"))
    }

    fn record(&mut self, row: Vec<J>) -> api::Result<()> {
        self.events.push(J::Array(row));
        self.calls += 1;
        if self.calls == self.fail_at {
            return Err(Error::Binding(None));
        }
        Ok(())
    }

    fn answer(&mut self, name: &str) -> J {
        let Some(list) = self.answers.get(name) else {
            return J::Null;
        };
        if list.is_empty() {
            return J::Null;
        }
        let c = self.answer_calls.entry(name.to_string()).or_insert(0);
        let v = list[(*c).min(list.len() - 1)].clone();
        *c += 1;
        v
    }

    fn arg(&self, a: &Arg<N>) -> J {
        match a {
            Arg::Null => J::Null,
            Arg::Node(n) => json!(n),
            Arg::Int(i) => json!(i),
            Arg::Bool(b) => json!(b),
            Arg::Text(t) => json!(t),
            Arg::Double(d) => json!({"double": dj(*d)}),
            Arg::Long(l) => json!({"long": l}),
            Arg::Short(s) => json!({"short": s}),
            Arg::Loc(l) => json!({"loc": [l.x(), l.y(), l.z()]}),
            Arg::Enum(e) => json!(e),
            Arg::List(l) => J::Array(l.iter().map(|x| self.arg(x)).collect()),
            Arg::Display(s) => json!(s),
        }
    }

    fn value_of(&self, v: &J) -> Value<N> {
        match v {
            J::Null => Value::Null,
            J::Bool(b) => Value::Bool(*b),
            J::Number(n) if n.is_i64() => Value::Int(n.as_i64().unwrap() as i32),
            J::Number(n) => Value::Double(n.as_f64().unwrap()),
            J::String(s) => Value::Text(s.clone()),
            J::Object(m) if m.contains_key("long") => Value::Long(m["long"].as_i64().unwrap()),
            J::Object(m) if m.contains_key("double") => {
                Value::Double(m["double"].as_f64().unwrap())
            }
            J::Object(m) if m.contains_key("node") => {
                Value::Node(m["node"].as_str().unwrap().to_string())
            }
            J::Object(_) if loc(v).is_some() => Value::Loc(loc(v).unwrap()),
            other => panic!("value {other}"),
        }
    }

    /// The state the stubs change inside a recorded call.
    fn side_effect(&mut self, member: Member, args: &[Arg<N>]) {
        let key = |i: usize| match &args[i] {
            Arg::Node(n) => n.clone(),
            other => panic!("{other:?}"),
        };
        let int_at = |i: usize| match &args[i] {
            Arg::Int(v) => *v,
            other => panic!("{other:?}"),
        };
        match member {
            Member::ItemSetCharge => {
                let (k, c) = (key(0), int_at(1));
                let n = self.nodes.get_mut(&k).unwrap();
                n.id_hash = ((n.id_hash >> 16 & 0xFFFF) << 16) | c;
            }
            Member::ScenerySetCharge => {
                let (k, c) = (key(0), int_at(1));
                self.nodes.get_mut(&k).unwrap().charge = c;
            }
            Member::NpcSetInvisible => {
                let k = key(0);
                self.nodes.get_mut(&k).unwrap().invisible = true;
            }
            Member::PoisonSetSeverity => {
                let (k, s) = (key(0), int_at(1));
                self.nodes.get_mut(&k).unwrap().severity = s;
            }
            _ => {}
        }
    }
}

impl Host for T {
    type Node = N;

    fn call(&mut self, member: Member, args: Vec<Arg<N>>) -> api::Result<Reply<N>> {
        let mut row = vec![json!(member.name())];
        for (index, a) in args.iter().enumerate() {
            let is_callback = matches!(
                member,
                Member::Callback
                    | Member::CallbackBool
                    | Member::CallbackInt
                    | Member::CallbackNode
            );
            match a {
                Arg::Node(n) if index == 0 && is_callback => {
                    row.push(json!(n.strip_prefix("fn:").unwrap_or(n)))
                }
                other => row.push(self.arg(other)),
            }
        }
        // Constructors record before their definition lookup.
        if member == Member::NewItem {
            self.record(row)?;
            let id = match args[0] {
                Arg::Int(i) => i,
                _ => unreachable!(),
            };
            let (amount, charge) = match (&args[1], &args[2]) {
                (Arg::Int(a), Arg::Int(c)) => (*a, *c),
                _ => unreachable!(),
            };
            let Some(def) = self.items.get(&id).cloned() else {
                return Ok(Reply::Node(None));
            };
            let k = self.fresh("item");
            self.nodes.insert(
                k.clone(),
                NodeState {
                    kind: "item".into(),
                    id_hash: (id << 16) | charge,
                    amount,
                    slot: -1,
                    name: def["name"].as_str().map(String::from),
                    clocks: vec![],
                    ..Default::default()
                },
            );
            return Ok(Reply::Node(Some(k)));
        }
        self.record(row)?;
        self.side_effect(member, &args);
        let name = member.name();
        let a = self.answer(name);
        Ok(match member.reply() {
            ReplyKind::Unit => match member {
                Member::NewScenery | Member::NewToml | Member::DynamicRegionNew => unreachable!(),
                _ => Reply::Unit,
            },
            ReplyKind::Bool => Reply::Bool(a.as_bool().unwrap_or(false)),
            ReplyKind::Int => Reply::Int(a.as_i64().map_or(0, |v| v as i32)),
            ReplyKind::Double => Reply::Double(a.as_f64().unwrap_or(0.0)),
            ReplyKind::Text => Reply::Text(a.as_str().map(String::from)),
            ReplyKind::Texts => Reply::Texts(
                a.as_array()
                    .map(|l| l.iter().map(|s| s.as_str().unwrap().to_string()).collect()),
            ),
            ReplyKind::Node => {
                let created = match member {
                    Member::NewScenery => Some("scenery"),
                    Member::NewToml => Some("toml"),
                    Member::DynamicRegionNew => Some("DynamicRegion"),
                    Member::RegionChunkOf => Some("chunk"),
                    _ => None,
                };
                match created {
                    Some(kind) => {
                        let k = self.fresh(kind);
                        let mut n = NodeState {
                            kind: kind.into(),
                            clocks: vec![],
                            ..Default::default()
                        };
                        if member == Member::NewScenery {
                            n.charge = 1000;
                            n.id = match args[0] {
                                Arg::Int(i) => i,
                                _ => 0,
                            };
                        }
                        self.nodes.insert(k.clone(), n);
                        Reply::Node(Some(k))
                    }
                    None => Reply::Node(a.as_str().map(String::from)),
                }
            }
            ReplyKind::Nodes => Reply::Nodes(a.as_array().map_or(vec![], |l| {
                l.iter().map(|x| x.as_str().map(String::from)).collect()
            })),
            ReplyKind::Ints => Reply::Ints(a.as_array().map(|l| l.iter().map(int).collect())),
            ReplyKind::Loc => Reply::Loc(loc(&a)),
            ReplyKind::Value => Reply::Value(if a.is_null() {
                None
            } else {
                Some(self.value_of(&a))
            }),
            ReplyKind::Path => Reply::Path(if a.is_null() {
                None
            } else {
                let handle = self.fresh("path");
                Some(Path {
                    handle,
                    successful: a.get("successful").and_then(J::as_bool).unwrap_or(false),
                    move_near: a.get("moveNear").and_then(J::as_bool).unwrap_or(false),
                })
            }),
            ReplyKind::Borders => Reply::Borders(
                a.as_array()
                    .map(|l| [int(&l[0]), int(&l[1]), int(&l[2]), int(&l[3]), int(&l[4])]),
            ),
            ReplyKind::Evaluation => Reply::Evaluation(if a.is_null() {
                None
            } else {
                let unmet = a["unmet"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|r| match r[0].as_str().unwrap() {
                        "QPCumulative" => Requirement::QpCumulative(int(&r[1])),
                        "QPReq" => Requirement::QpReq(int(&r[1])),
                        "QuestReq" => Requirement::Quest {
                            requirement: r[1].as_str().unwrap().into(),
                            stage: int(&r[2]),
                        },
                        "SkillReq" => Requirement::Skill {
                            skill: int(&r[1]),
                            level: int(&r[2]),
                            soft: r[3].as_bool().unwrap(),
                        },
                        other => panic!("{other}"),
                    })
                    .collect();
                Some((a["met"].as_bool().unwrap(), unmet))
            }),
            ReplyKind::Bits => Reply::Bits(
                a.as_array()
                    .map(|l| (l[0].as_i64().unwrap(), l[1].as_i64().unwrap())),
            ),
            ReplyKind::QuestMap => Reply::QuestMap(a.as_object().map_or(vec![], |m| {
                m.iter()
                    .map(|(k, v)| (k.clone(), v.as_str().unwrap().to_string()))
                    .collect()
            })),
            ReplyKind::Enum => Reply::Enum(a.as_str().map(String::from)),
            ReplyKind::Component => Reply::Component(a.as_i64().map(|v| v as i32)),
            ReplyKind::OptInt => Reply::OptInt(a.as_i64().map(|v| v as i32)),
        })
    }

    fn submit_pulse(&mut self, target: PulseTarget<'_, N>, pulse: Pulse<N>) -> api::Result<N> {
        let label = self.fresh("pulse");
        let row = match target {
            PulseTarget::World => vec![
                json!("PulseRunner.submit"),
                J::Null,
                json!(label),
                json!(pulse.delay),
            ],
            PulseTarget::Entity(e) => vec![
                json!("PulseManager.run"),
                json!(e),
                json!(label),
                json!(pulse.delay),
            ],
        };
        self.pulses.push((label.clone(), pulse));
        self.record(row)?;
        Ok(label)
    }

    fn add_to_queue(&mut self, entity: &N, script: QueuedScript<N>) -> api::Result<()> {
        let label = self.fresh("script");
        let row = vec![
            json!("ScriptProcessor.addToQueue"),
            json!(entity),
            json!(label),
            json!(script.strength.name()),
            json!(script.next_execution),
            json!(script.persist),
        ];
        self.nodes.insert(
            label.clone(),
            NodeState {
                kind: "script".into(),
                persist: script.persist,
                next_execution: script.next_execution,
                clocks: vec![],
                ..Default::default()
            },
        );
        self.scripts.push((label, script));
        self.record(row)
    }

    fn open_skill_dialogue(
        &mut self,
        handler: SkillHandler<N>,
        kind: &'static str,
    ) -> api::Result<()> {
        let label = self.fresh("handler");
        let row = vec![
            json!("SkillDialogueHandler.open"),
            json!(label),
            json!(handler.player),
            json!(kind),
            json!(handler.items),
        ];
        self.handlers.push((label, handler));
        self.record(row)
    }

    fn print_stack_trace(&mut self, error: &Error) -> api::Result<()> {
        let message = match error {
            Error::Argument(m) | Error::State(m) => json!(m),
            _ => J::Null,
        };
        self.events
            .push(json!(["@printStackTrace", error.code(), message]));
        Ok(())
    }

    fn exception_to_string(&mut self, message: &str) -> String {
        format!("java.lang.Exception: {message}\n<frames>")
    }

    fn kind(&self, node: &N) -> Kind {
        if node.starts_with("file:") {
            return Kind::DialogueFile;
        }
        match self.nodes.get(node).map(|n| n.kind.as_str()) {
            Some("player") => Kind::Player,
            Some("npc") => Kind::Npc,
            Some("familiar") => Kind::Familiar,
            Some("bob") => Kind::BurdenBeast,
            Some("item") => Kind::Item,
            Some("grounditem") => Kind::GroundItem,
            Some("scenery") => Kind::Scenery,
            _ => Kind::Other,
        }
    }
    fn id(&self, node: &N) -> i32 {
        let n = self.node(node);
        if n.kind == "item" || n.kind == "grounditem" {
            n.id_hash >> 16 & 0xFFFF
        } else {
            n.id
        }
    }
    fn name(&self, node: &N) -> Option<String> {
        self.node(node).name.clone()
    }
    fn location(&self, node: &N) -> Option<Location> {
        self.node(node).location
    }
    fn size(&self, node: &N) -> i32 {
        self.node(node).size
    }
    fn item_amount(&self, item: &N) -> i32 {
        self.node(item).amount
    }
    fn item_charge(&self, node: &N) -> i32 {
        let n = self.node(node);
        if n.kind == "scenery" {
            n.charge
        } else {
            n.id_hash & 0xFFFF
        }
    }
    fn rotation(&self, scenery: &N) -> i32 {
        self.node(scenery).rotation
    }
    fn item_slot(&self, item: &N) -> i32 {
        self.node(item).slot
    }
    fn item_definition(&self, id: i32) -> Option<ItemDef<N>> {
        let d = self.items.get(&id)?;
        let config = d.get("config");
        let conf = |k: &str| config.and_then(|c| c.get(k)).map(|v| self.value_of(v));
        Some(ItemDef {
            name: d["name"].as_str().map(String::from),
            stackable: d.get("stackable").is_some_and(|v| v.as_bool().unwrap()),
            unnoted: d.get("unnoted").is_none_or(|v| v.as_bool().unwrap()),
            note_id: d.get("noteId").map_or(-1, int),
            equip_slot: conf("equipment_slot"),
            rare: conf("rare_item"),
        })
    }
    fn container(&self, player: &N, which: Holder) -> N {
        let secondary = self.node(player).secondary;
        let name = match which {
            Holder::Inventory => "inventory",
            Holder::Equipment => "equipment",
            Holder::Bank if secondary => "bankSecondary",
            Holder::Bank | Holder::BankPrimary => "bankPrimary",
            Holder::BankSecondary => "bankSecondary",
            Holder::PohProxy => "POHStorageProxyContainer",
        };
        format!("{player}.{name}")
    }
    fn burden_container(&self, beast: &N) -> N {
        format!("{beast}.bob")
    }
    fn is_equipment_container(&self, container: &N) -> bool {
        container.ends_with(".equipment")
    }
    fn is_male(&self, player: &N) -> bool {
        self.node(player).male
    }
    fn username(&self, player: &N) -> String {
        self.node(player).username.clone()
    }
    fn use_secondary_bank(&self, player: &N) -> bool {
        self.node(player).secondary
    }
    fn set_use_secondary_bank(&mut self, player: &N, value: bool) {
        self.nodes.get_mut(player).unwrap().secondary = value;
    }
    fn clock(&self, entity: &N, index: i32) -> api::Result<i32> {
        self.node(entity)
            .clocks
            .get(index as usize)
            .copied()
            .filter(|_| index >= 0)
            .ok_or(Error::Index)
    }
    fn set_clock(&mut self, entity: &N, index: i32, value: i32) -> api::Result<()> {
        let c = self
            .nodes
            .get_mut(entity)
            .unwrap()
            .clocks
            .get_mut(index as usize)
            .filter(|_| index >= 0)
            .ok_or(Error::Index)?;
        *c = value;
        Ok(())
    }
    fn ticks(&self) -> i32 {
        self.ticks
    }
    fn dev_mode(&self) -> Option<bool> {
        self.dev_mode.flatten()
    }
    fn max_pathfind_distance(&self) -> i32 {
        self.max_pathfind
    }
    fn script_persist(&self, script: &N) -> bool {
        self.node(script).persist
    }
    fn script_state(&self, script: &N) -> i32 {
        self.node(script).script_state
    }
    fn set_script_next_execution(&mut self, script: &N, value: i32) {
        self.nodes.get_mut(script).unwrap().next_execution = value;
    }
    fn set_script_state(&mut self, script: &N, value: i32) {
        self.nodes.get_mut(script).unwrap().script_state = value;
    }
    fn script_class(&self, _: &N) -> String {
        "QueuedScript".into()
    }
    fn poison_severity(&self, timer: &N) -> i32 {
        self.node(timer).severity
    }
    fn set_poison_source(&mut self, timer: &N, source: &N) {
        self.nodes.get_mut(timer).unwrap().source = Some(source.clone());
    }
    fn timer_class(&self, timer: &N) -> String {
        match self.node(timer).kind.as_str() {
            "poison" => "Poison",
            "poisonImmunity" => "PoisonImmunity",
            "dragonfireImmunity" => "DragonFireImmunity",
            _ => "RSTimer",
        }
        .into()
    }
    fn set_logout_listener(&mut self, player: &N, key: &str, handler: Option<N>) {
        let l = &mut self.nodes.get_mut(player).unwrap().listeners;
        l.retain(|k| k != key);
        if handler.is_some() {
            l.push(key.into());
        }
    }
    fn shop_by_npc(&self, npc: i32) -> Option<N> {
        self.shops.get(&npc).cloned()
    }
    fn offer_records(&self, player: &N) -> api::Result<Vec<Option<N>>> {
        self.node(player)
            .records
            .clone()
            .ok_or(Error::State("no records".into()))
    }
    fn offer_has_withdraw(&self, offer: &N) -> bool {
        self.node(offer)
            .withdraw
            .first()
            .is_some_and(Option::is_some)
    }
    fn toml_data(&self) -> Option<N> {
        self.toml.clone()
    }
    fn potion_ids(&self, consumable: &N) -> Option<Vec<i32>> {
        let n = self.node(consumable);
        (n.kind == "potion").then(|| n.ids.clone())
    }
    fn identity_hash(&self, node: &N) -> i32 {
        self.node(node).hash
    }
    fn diary_level_names(&self, diary: &str) -> Vec<String> {
        let names: &[&str] = match diary {
            "LUMBRIDGE" => &["Beginner", "Easy", "Medium"],
            _ => &["Easy", "Medium", "Hard"],
        };
        names.iter().map(|s| s.to_string()).collect()
    }
    fn kotlin_random_int(&mut self, bound: i32) -> i32 {
        self.random.next_int(bound)
    }
    fn zone_maps(&mut self) -> &mut Vec<(String, N)> {
        &mut self.zones
    }
    fn quest_name(&self, quest: &str) -> String {
        self.quests[quest].clone()
    }
    fn master_npc(&self, master: &str) -> i32 {
        match master {
            "TURAEL" => 8273,
            "MAZCHNA" => 8274,
            "VANNAKA" => 1597,
            "CHAELDAR" => 1598,
            "DURADEL" => 8275,
            other => panic!("{other}"),
        }
    }
}

// ---------------------------------------------------------------- varps through player::variables

struct Vars<'a> {
    t: &'a mut T,
    owner: String,
}
impl variables::Context for Vars<'_> {
    type Dispatch = ();
    fn values(&mut self, _: &NodeHandle) -> variables::Result<Option<Values>> {
        Ok(Some(self.t.values[&self.owner].0.clone()))
    }
    fn saved(&mut self, _: &NodeHandle) -> variables::Result<Option<Saved>> {
        Ok(Some(self.t.values[&self.owner].1.clone()))
    }
    fn dispatch(&mut self, _: &NodeHandle) -> variables::Result<()> {
        Ok(())
    }
    fn send_varp(&mut self, _: (), id: i32, value: i32) -> variables::Result<()> {
        let o = self.owner.clone();
        self.t
            .record(vec![
                json!("PacketDispatch.sendVarp"),
                json!(o),
                json!(id),
                json!(value),
            ])
            .map_err(|_| variables::Error::Binding("injected".into()))
    }
    fn send_varc(&mut self, _: (), id: i16, value: i32) -> variables::Result<()> {
        let o = self.owner.clone();
        self.t
            .record(vec![
                json!("PacketDispatch.sendVarcUpdate"),
                json!(o),
                json!({"short": id}),
                json!(value),
            ])
            .map_err(|_| variables::Error::Binding("injected".into()))
    }
    fn definition(&mut self, id: i32) -> variables::Result<Option<varbit::Handle>> {
        Ok(self.t.varbits.get(&id).map(|v| {
            Shared::new(varbit::Definition {
                id,
                varp_id: v[0],
                start_bit: v[1],
                end_bit: v[2],
            })
        }))
    }
    fn missing_definition(&mut self, id: i32) -> variables::Result<()> {
        let text = format!(
            "java.lang.Exception: Trying to setVarbit {id}, which doesn't seem to exist.\n<frames>"
        );
        self.t
            .record(vec![
                json!("SystemLogger.processLogEntry"),
                json!("ContentAPI"),
                json!("ERR"),
                json!(text),
            ])
            .map_err(|_| variables::Error::Binding("injected".into()))
    }
}

fn var_error(e: variables::Error) -> Error {
    match e {
        variables::Error::Null => Error::Null,
        variables::Error::Type => Error::Cast,
        variables::Error::Binding(m) => Error::Binding(Some(m)),
        other => Error::Binding(Some(format!("{other:?}"))),
    }
}

// ---------------------------------------------------------------- argument decoding

fn s(a: &[J], i: usize) -> String {
    a[i].as_str().unwrap().to_string()
}
fn st(a: &[J], i: usize) -> &'static str {
    Box::leak(s(a, i).into_boxed_str())
}
fn b(a: &[J], i: usize) -> bool {
    a[i].as_bool().unwrap()
}
fn i(a: &[J], i: usize) -> i32 {
    int(&a[i])
}
fn l(a: &[J], i: usize) -> Location {
    loc(&a[i]).unwrap()
}
fn ints(a: &[J], i: usize) -> Vec<i32> {
    a[i].as_array().unwrap().iter().map(int).collect()
}
fn strs(a: &[J], i: usize) -> Vec<String> {
    a[i].as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect()
}
fn opt(a: &[J], i: usize) -> Option<String> {
    a[i].as_str().map(String::from)
}
fn borders(a: &[J], i: usize) -> [i32; 5] {
    let v = ints(a, i);
    [v[0], v[1], v[2], v[3], v[4]]
}
/// A generic argument: {"int"}, {"node"}, {"str"}, {"bool"}, {"anim"}, {"gfx"}, {"long"}, {"double"}, {"loc"}, {"fn"}, {"dialogueFile"}.
fn any(v: &J) -> Value<N> {
    if v.is_null() {
        return Value::Null;
    }
    let m = v.as_object().unwrap();
    if let Some(x) = m.get("int") {
        Value::Int(int(x))
    } else if let Some(x) = m.get("node") {
        Value::Node(x.as_str().unwrap().into())
    } else if let Some(x) = m.get("str") {
        Value::Text(x.as_str().unwrap().into())
    } else if let Some(x) = m.get("bool") {
        Value::Bool(x.as_bool().unwrap())
    } else if let Some(x) = m.get("long") {
        Value::Long(x.as_i64().unwrap())
    } else if let Some(x) = m.get("double") {
        Value::Double(x.as_f64().unwrap())
    } else if m.contains_key("loc") {
        Value::Loc(loc(v).unwrap())
    } else if let Some(x) = m.get("anim") {
        let p = Priority::ALL
            .into_iter()
            .find(|p| p.name() == x[1].as_str().unwrap())
            .unwrap();
        Value::Anim(Anim::with(int(&x[0]), p))
    } else if let Some(x) = m.get("gfx") {
        Value::Gfx(Gfx {
            id: int(&x[0]),
            height: int(&x[1]),
            delay: int(&x[2]),
        })
    } else if let Some(x) = m.get("fn") {
        Value::Node(format!("fn:{}", x.as_str().unwrap()))
    } else if let Some(x) = m.get("dialogueFile") {
        Value::Node(format!("file:{}", x.as_str().unwrap()))
    } else {
        panic!("any {v}")
    }
}
fn anys(a: &[J], i: usize) -> Vec<Value<N>> {
    a[i].as_array().unwrap().iter().map(any).collect()
}
fn container(a: &[J], i: usize) -> Container {
    match a[i].as_str().unwrap() {
        "INVENTORY" => Container::Inventory,
        "BANK" => Container::Bank,
        "EQUIPMENT" => Container::Equipment,
        "BoB" => Container::BoB,
        other => panic!("{other}"),
    }
}
fn fnode(name: &str) -> N {
    format!("fn:{name}")
}

// ---------------------------------------------------------------- results

fn value_json(v: &Value<N>) -> J {
    match v {
        Value::Null => J::Null,
        Value::Int(i) => json!(i),
        Value::Long(l) => json!({"long": l}),
        Value::Double(d) => json!({"double": dj(*d)}),
        Value::Bool(b) => json!(b),
        Value::Text(t) => json!(t),
        Value::Loc(l) => json!({"loc": [l.x(), l.y(), l.z()]}),
        Value::Node(n) => json!(n),
        Value::Anim(a) => json!(["anim", a.id, a.priority.name(), a.object]),
        Value::Gfx(g) => json!(["gfx", g.id, g.height, g.delay]),
        Value::Enum(e) => json!(e),
    }
}
/// A double as the fixture holds it: Java's text, parsed as the fixture's
/// numbers are.
fn dj(d: f64) -> J {
    serde_json::from_str(&files::java_double_string(d)).unwrap()
}
fn lj(l: Location) -> J {
    json!({"loc": [l.x(), l.y(), l.z()]})
}
fn unit_r(r: api::Result<()>) -> api::Result<J> {
    r.map(|_| J::Null)
}
fn bool_r(r: api::Result<bool>) -> api::Result<J> {
    r.map(|v| json!(v))
}
fn int_r(r: api::Result<i32>) -> api::Result<J> {
    r.map(|v| json!(v))
}

fn varp_call(t: &mut T, fn_name: &str, a: &[J]) -> api::Result<J> {
    let mut walk = State::default();
    let node = walk.add(
        1,
        rs09_server::world::doors::Location::new(3200, 3200, 0),
        [1, 2, 3, 4],
    );
    let handle = walk.retain_node(node).unwrap();
    let owner = s(a, 0);
    let mut ctx = Vars { t, owner };
    let r = match fn_name {
        "getVarp" => vars::get_varp(&mut ctx, &handle, i(a, 1)).map(|v| json!(v)),
        "getVarbit" => vars::get_varbit(&mut ctx, &handle, i(a, 1)).map(|v| json!(v)),
        "getVarbit(definition)" => {
            let def = variables::Context::definition(&mut ctx, i(a, 1)).unwrap();
            vars::get_varbit_definition(&mut ctx, &handle, def.as_ref()).map(|v| json!(v))
        }
        "setVarp" => vars::set_varp(&mut ctx, &handle, i(a, 1), i(a, 2), false).map(|_| J::Null),
        "setVarp(save)" => {
            vars::set_varp(&mut ctx, &handle, i(a, 1), i(a, 2), b(a, 3)).map(|_| J::Null)
        }
        "saveVarp" => vars::save_varp(&mut ctx, &handle, i(a, 1)).map(|_| J::Null),
        "unsaveVarp" => vars::unsave_varp(&mut ctx, &handle, i(a, 1)).map(|_| J::Null),
        "setVarbit" => {
            vars::set_varbit(&mut ctx, &handle, i(a, 1), i(a, 2), false).map(|_| J::Null)
        }
        "setVarbit(save)" => {
            vars::set_varbit(&mut ctx, &handle, i(a, 1), i(a, 2), b(a, 3)).map(|_| J::Null)
        }
        "setVarbit(definition)" => {
            let def = variables::Context::definition(&mut ctx, i(a, 1)).unwrap();
            vars::set_varbit_definition(&mut ctx, &handle, def.as_ref(), i(a, 2), b(a, 3))
                .map(|_| J::Null)
        }
        "setVarc" => vars::set_varc(&mut ctx, &handle, i(a, 1), i(a, 2)).map(|_| J::Null),
        "reinitVarps" => vars::reinit_varps(&mut ctx, &handle).map(|_| J::Null),
        _ => unreachable!(),
    };
    r.map_err(var_error)
}

/// Runs one entry point with the case's arguments.
fn run(t: &mut T, f: &str, a: &[J]) -> api::Result<J> {
    let p = || -> N { s(a, 0) };
    match f {
        // skills
        "getTool" => items::get_tool(t, &p(), b(a, 1)).map(|v| json!(v)),
        "hasLevelDyn" => bool_r(services::has_level_dyn(t, &p(), i(a, 1), i(a, 2))),
        "hasLevelStat" => bool_r(services::has_level_stat(t, &p(), i(a, 1), i(a, 2))),
        "rewardXP" => unit_r(services::reward_xp(
            t,
            &p(),
            i(a, 1),
            a[2].as_f64().unwrap(),
        )),
        "setTempLevel" => unit_r(services::set_temp_level(t, &p(), i(a, 1), i(a, 2))),
        "getStatLevel" => int_r(services::get_stat_level(t, &p(), i(a, 1))),
        "getDynLevel" => int_r(services::get_dyn_level(t, &p(), i(a, 1))),
        "adjustLevel" => unit_r(services::adjust_level(t, &p(), i(a, 1), i(a, 2))),
        "heal" => unit_r(services::heal(t, &p(), i(a, 1))),
        "getMasteredSkillNames" => services::get_mastered_skill_names(t, &p()).map(|v| json!(v)),
        "getFamiliarBoost" => int_r(services::get_familiar_boost(t, &p(), i(a, 1))),
        "modPrayerPoints" => unit_r(services::mod_prayer_points(t, &p(), a[1].as_f64().unwrap())),
        "hasDragonfireShieldProtection" => {
            bool_r(services::has_dragonfire_shield_protection(t, &p(), false))
        }
        "hasDragonfireShieldProtection(wyvern)" => {
            bool_r(services::has_dragonfire_shield_protection(t, &p(), b(a, 1)))
        }
        "calculateDragonfireMaxHit" => int_r(services::calculate_dragonfire_max_hit(
            t,
            &p(),
            i(a, 1),
            false,
            0,
            false,
        )),
        "calculateDragonfireMaxHit(all)" => int_r(services::calculate_dragonfire_max_hit(
            t,
            &p(),
            i(a, 1),
            b(a, 2),
            i(a, 3),
            b(a, 4),
        )),
        // containers
        "amountInInventory" => int_r(items::amount_in_inventory(t, &p(), i(a, 1))),
        "amountInBank" => int_r(items::amount_in_bank(t, &p(), i(a, 1), true)),
        "amountInBank(includeSecondary)" => int_r(items::amount_in_bank(t, &p(), i(a, 1), b(a, 2))),
        "amountInEquipment" => int_r(items::amount_in_equipment(t, &p(), i(a, 1))),
        "inInventory" => bool_r(items::in_inventory(t, &p(), i(a, 1), 1)),
        "inInventory(amount)" => bool_r(items::in_inventory(t, &p(), i(a, 1), i(a, 2))),
        "inBank" => bool_r(items::in_bank(t, &p(), i(a, 1), 1)),
        "inBank(amount)" => bool_r(items::in_bank(t, &p(), i(a, 1), i(a, 2))),
        "inEquipment" => bool_r(items::in_equipment(t, &p(), i(a, 1), 1)),
        "inEquipment(amount)" => bool_r(items::in_equipment(t, &p(), i(a, 1), i(a, 2))),
        "anyInInventory" => bool_r(items::any_in_inventory(t, &p(), &ints(a, 1))),
        "inEquipmentOrInventory" => bool_r(items::in_equipment_or_inventory(t, &p(), i(a, 1), 1)),
        "inEquipmentOrInventory(amount)" => {
            bool_r(items::in_equipment_or_inventory(t, &p(), i(a, 1), i(a, 2)))
        }
        "allInEquipment" => bool_r(items::all_in_equipment(t, &p(), &ints(a, 1))),
        "anyInEquipment" => bool_r(items::any_in_equipment(t, &p(), &ints(a, 1))),
        "getItemFromEquipment" => {
            items::get_item_from_equipment(t, &p(), EquipmentSlot::from_name(&s(a, 1)).unwrap())
                .map(|v| json!(v))
        }
        "hasAnItem(id)" | "hasAnItem(id,flags)" | "hasAnItem(ids,flags)" => {
            let ids = if f == "hasAnItem(ids,flags)" {
                ints(a, 1)
            } else {
                vec![i(a, 1)]
            };
            let (inactive, poh) = if f == "hasAnItem(id)" {
                (false, false)
            } else {
                (b(a, 2), b(a, 3))
            };
            items::has_an_item(t, &p(), &ids, inactive, poh)
                .map(|c| json!(["ContainerisedItem", c.container, c.item_id, c.exists()]))
        }
        "ContainerisedItem.remove" => bool_r(
            items::ContainerisedItem {
                container: opt(a, 0),
                item_id: i(a, 1),
            }
            .remove(t),
        ),
        "hasGodItem" => bool_r(items::has_god_item(
            t,
            &p(),
            God::from_name(&s(a, 1)).unwrap(),
        )),
        "shouldRemoveNothings" => bool_r(items::should_remove_nothings(t, &p())),
        "removeItem" => bool_r(items::remove_item(
            t,
            &p(),
            &any(&a[1]),
            Container::Inventory,
        )),
        "removeItem(container)" => {
            bool_r(items::remove_item(t, &p(), &any(&a[1]), container(a, 2)))
        }
        "removeItemsIfPlayerHasEnough" => bool_r(items::remove_items_if_player_has_enough(
            t,
            &p(),
            &strs(a, 1),
        )),
        "addItem" => bool_r(items::add_item(t, &p(), i(a, 1), 1, Container::Inventory)),
        "addItem(amount)" => bool_r(items::add_item(
            t,
            &p(),
            i(a, 1),
            i(a, 2),
            Container::Inventory,
        )),
        "addItem(container)" => bool_r(items::add_item(t, &p(), i(a, 1), i(a, 2), container(a, 3))),
        "replaceSlot" => {
            items::replace_slot(t, &p(), i(a, 1), &s(a, 2), None, Container::Inventory)
                .map(|v| json!(v))
        }
        "replaceSlot(current)" => items::replace_slot(
            t,
            &p(),
            i(a, 1),
            &s(a, 2),
            opt(a, 3).as_ref(),
            Container::Inventory,
        )
        .map(|v| json!(v)),
        "replaceSlot(container)" => items::replace_slot(
            t,
            &p(),
            i(a, 1),
            &s(a, 2),
            opt(a, 3).as_ref(),
            container(a, 4),
        )
        .map(|v| json!(v)),
        "replaceAllItems" => unit_r(items::replace_all_items(t, &p(), i(a, 1), i(a, 2))),
        "addItemOrDrop" => unit_r(items::add_item_or_drop(t, &p(), i(a, 1), 1)),
        "addItemOrDrop(amount)" => unit_r(items::add_item_or_drop(t, &p(), i(a, 1), i(a, 2))),
        "addItemOrBank" => unit_r(items::add_item_or_bank(t, &p(), i(a, 1), 1)),
        "addItemOrBank(amount)" => unit_r(items::add_item_or_bank(t, &p(), i(a, 1), i(a, 2))),
        "freeSlots" => int_r(items::free_slots(t, &p())),
        "hasSpaceFor" => bool_r(items::has_space_for(t, &p(), &s(a, 1))),
        "removeAll" => bool_r(items::remove_all(
            t,
            &p(),
            &any(&a[1]),
            Container::Inventory,
        )),
        "removeAll(container)" => bool_r(items::remove_all(t, &p(), &any(&a[1]), container(a, 2))),
        "note" => items::note(t, &s(a, 0)).map(|v| json!(v)),
        "unnote" => items::unnote(t, &s(a, 0)).map(|v| json!(v)),
        "equipSlot" => items::equip_slot(t, i(a, 0)).map(|v| json!(v.map(|s| s.name()))),
        "hasSealOfPassage" => bool_r(items::has_seal_of_passage(t, &p())),
        "dumpContainer" => int_r(items::dump_container(t, &p(), &s(a, 1))),
        "dumpBeastOfBurden" => unit_r(items::dump_beast_of_burden(t, &p())),
        "decantContainer" => items::decant_container(t, &s(a, 0)).map(|(r, ad)| json!([r, ad])),
        // visuals
        "getAnimation" => Ok(value_json(&Value::Anim(visuals::get_animation(i(a, 0))))),
        "getAnimationWithPriority" => {
            let pr = Priority::ALL
                .into_iter()
                .find(|x| x.name() == s(a, 1))
                .unwrap();
            Ok(value_json(&Value::Anim(
                visuals::get_animation_with_priority(i(a, 0), pr),
            )))
        }
        "resetAnimator" => unit_r(visuals::reset_animator(t, &p())),
        "renderAnimation" => unit_r(visuals::render_animation(t, &p(), i(a, 1))),
        "resetRenderAnimation" => unit_r(visuals::reset_render_animation(t, &p())),
        "animationDuration" => match any(&a[0]) {
            Value::Anim(an) => int_r(visuals::animation_duration(t, &an)),
            _ => unreachable!(),
        },
        "animationCycles" => int_r(visuals::animation_cycles(t, i(a, 0))),
        "animate" => unit_r(visuals::animate(t, &p(), &any(&a[1]), false)),
        "animate(forced)" => unit_r(visuals::animate(t, &p(), &any(&a[1]), b(a, 2))),
        "visualize" => unit_r(visuals::visualize(t, &p(), &any(&a[1]), &any(&a[2]))),
        "sendGraphics" => unit_r(visuals::send_graphics(t, &any(&a[0]), l(a, 1))),
        "emote" => {
            let id = match s(a, 1).as_str() {
                "YES" => 855,
                "NO" => 856,
                _ => 858,
            };
            unit_r(visuals::emote(t, &p(), &Anim::new(id)))
        }
        "animateScenery(player)" => unit_r(visuals::animate_scenery_for(
            t,
            &p(),
            &s(a, 1),
            i(a, 2),
            false,
        )),
        "animateScenery(player,global)" => unit_r(visuals::animate_scenery_for(
            t,
            &p(),
            &s(a, 1),
            i(a, 2),
            b(a, 3),
        )),
        "animateScenery" => unit_r(visuals::animate_scenery(t, &s(a, 0), i(a, 1))),
        "animateInterface" => unit_r(visuals::animate_interface(
            t,
            &p(),
            i(a, 1),
            i(a, 2),
            i(a, 3),
        )),
        "spawnProjectile(entity)" => unit_r(visuals::spawn_projectile(t, &p(), &s(a, 1), i(a, 2))),
        "spawnProjectile(location)" => unit_r(visuals::spawn_projectile_at(
            t,
            l(a, 0),
            l(a, 1),
            i(a, 2),
            i(a, 3),
            i(a, 4),
            i(a, 5),
            i(a, 6),
            i(a, 7),
        )),
        "playJingle" => unit_r(visuals::play_jingle(t, &p(), i(a, 1))),
        "playAudio" => unit_r(visuals::play_audio(t, &p(), i(a, 1), 0, 1, None, 8)),
        "playAudio(delay,loops)" => unit_r(visuals::play_audio(
            t,
            &p(),
            i(a, 1),
            i(a, 2),
            i(a, 3),
            None,
            8,
        )),
        "playAudio(all)" => unit_r(visuals::play_audio(
            t,
            &p(),
            i(a, 1),
            i(a, 2),
            i(a, 3),
            loc(&a[4]),
            i(a, 5),
        )),
        "playGlobalAudio" => unit_r(visuals::play_global_audio(t, l(a, 0), i(a, 1), 0, 1, 8)),
        "playGlobalAudio(all)" => unit_r(visuals::play_global_audio(
            t,
            l(a, 0),
            i(a, 1),
            i(a, 2),
            i(a, 3),
            i(a, 4),
        )),
        "playHurtAudio" => unit_r(visuals::play_hurt_audio(t, &p(), 0)),
        "playHurtAudio(delay)" => unit_r(visuals::play_hurt_audio(t, &p(), i(a, 1))),
        "impact" => unit_r(visuals::impact(t, &p(), i(a, 1), "NORMAL")),
        "impact(type)" => unit_r(visuals::impact(t, &p(), i(a, 1), st(a, 2))),
        // scenery, ground items, definitions, charges
        "replaceScenery" => unit_r(visuals::replace_scenery(
            t,
            &s(a, 0),
            i(a, 1),
            i(a, 2),
            None,
        )),
        "replaceScenery(location)" => unit_r(visuals::replace_scenery(
            t,
            &s(a, 0),
            i(a, 1),
            i(a, 2),
            loc(&a[3]),
        )),
        "replaceScenery(direction)" => unit_r(visuals::replace_scenery_facing(
            t,
            &s(a, 0),
            i(a, 1),
            i(a, 2),
            files::Direction::from_name(&s(a, 3)).unwrap(),
            None,
        )),
        "replaceScenery(direction,location)" => unit_r(visuals::replace_scenery_facing(
            t,
            &s(a, 0),
            i(a, 1),
            i(a, 2),
            files::Direction::from_name(&s(a, 3)).unwrap(),
            loc(&a[4]),
        )),
        "addScenery(id)" => visuals::add_scenery(t, i(a, 0), l(a, 1), 0, 22).map(|v| json!(v)),
        "addScenery(id,rotation,type)" => {
            visuals::add_scenery(t, i(a, 0), l(a, 1), i(a, 2), i(a, 3)).map(|v| json!(v))
        }
        "addScenery(scenery)" => unit_r(visuals::add_existing_scenery(t, &s(a, 0))),
        "removeScenery" => unit_r(visuals::remove_scenery(t, &s(a, 0))),
        "getScenery(xyz)" => visuals::get_scenery(t, i(a, 0), i(a, 1), i(a, 2)).map(|v| json!(v)),
        "getScenery(location)" => visuals::get_scenery_at(t, l(a, 0)).map(|v| json!(v)),
        "getChildren" => visuals::get_children(t, i(a, 0)).map(|v| json!(v)),
        "sceneryDefinition" => visuals::scenery_definition(t, i(a, 0))
            .map(|id| json!(format!("SceneryDefinition:{id}"))),
        "getItemName" => visuals::get_item_name(t, i(a, 0)).map(|v| json!(v)),
        "itemDefinition" => visuals::item_definition(t, i(a, 0))
            .map(|_| json!(format!("ItemDefinition:{}", i(a, 0)))),
        "hasOption" => bool_r(visuals::has_option(t, &s(a, 0), &s(a, 1))),
        "removeGroundItem" => unit_r(visuals::remove_ground_item(t, &s(a, 0))),
        "isValidGroundItem" => bool_r(visuals::is_valid_ground_item(t, &s(a, 0))),
        "produceGroundItem(player)" => unit_r(visuals::produce_ground_item(t, &p(), i(a, 1))),
        "produceGroundItem(owner)" => {
            visuals::produce_ground_item_at(t, opt(a, 0).as_ref(), i(a, 1), i(a, 2), l(a, 3))
                .map(|v| json!(v))
        }
        "adjustCharge" => unit_r(visuals::adjust_charge(t, &s(a, 0), i(a, 1))),
        "getCharge" => int_r(visuals::get_charge(t, &s(a, 0))),
        "setCharge" => unit_r(visuals::set_charge(t, &s(a, 0), i(a, 1))),
        // world
        "getWorldTicks" => Ok(json!(world::get_world_ticks(t))),
        "poofClear" => unit_r(world::poof_clear(t, &s(a, 0))),
        "findNPC(id)" => world::find_npc(t, i(a, 0)).map(|v| json!(v)),
        "findNPC(location)" => world::find_npc_near(t, l(a, 0), i(a, 1)).map(|v| json!(v)),
        "findLocalNPC" => world::find_local_npc(t, &p(), i(a, 1)).map(|v| json!(v)),
        "findLocalNPCs(location)" => {
            world::find_local_npcs_at(t, l(a, 0), i(a, 1)).map(|v| json!(v))
        }
        "findLocalNPCs(ids)" => {
            world::find_local_npcs(t, &p(), &ints(a, 1), None).map(|v| json!(v))
        }
        "findLocalNPCs(ids,distance)" => {
            world::find_local_npcs(t, &p(), &ints(a, 1), Some(i(a, 2))).map(|v| json!(v))
        }
        "getRegionBorders" => world::get_region_borders(t, i(a, 0)).map(t_arg_borders),
        "location" => Ok(lj(world::location(i(a, 0), i(a, 1), i(a, 2)))),
        "inBorders(borders)" => bool_r(world::in_borders(t, &p(), borders(a, 1))),
        "inBorders(corners)" => bool_r(world::in_corners(
            t,
            &p(),
            i(a, 1),
            i(a, 2),
            i(a, 3),
            i(a, 4),
        )),
        "transformNpc" => unit_r(world::transform_npc(t, &p(), i(a, 1), i(a, 2))),
        "transformNpcWithHpCarryover" => unit_r(world::transform_npc_with_hp_carryover(
            t,
            &p(),
            i(a, 1),
            i(a, 2),
        )),
        "forceWalk" => unit_r(world::force_walk(t, &p(), l(a, 1), &s(a, 2))),
        "truncateLoc" => world::truncate_loc(t, &p(), l(a, 1)).map(|(tr, at)| json!([tr, lj(at)])),
        "forceMove" => unit_r(world::force_move(
            t,
            &p(),
            l(a, 1),
            l(a, 2),
            i(a, 3),
            i(a, 4),
            None,
            819,
            None,
        )),
        "forceMove(all)" => {
            let dir = opt(a, 5).map(|d| files::Direction::from_name(&d).unwrap());
            let cb = opt(a, 7).map(|c| fnode(&c));
            unit_r(world::force_move(
                t,
                &p(),
                l(a, 1),
                l(a, 2),
                i(a, 3),
                i(a, 4),
                dir,
                i(a, 6),
                cb.as_ref(),
            ))
        }
        "stopWalk" => unit_r(world::stop_walk(t, &p())),
        "flee" => unit_r(world::flee(t, &p(), &s(a, 1))),
        "teleport" => bool_r(world::teleport(t, &p(), l(a, 1), "INSTANT")),
        "teleport(type)" => bool_r(world::teleport(t, &p(), l(a, 1), st(a, 2))),
        "face" => unit_r(world::face(t, &p(), &world::Face::Node(s(a, 1)), -1)),
        "face(duration)" => unit_r(world::face(t, &p(), &world::Face::Node(s(a, 1)), i(a, 2))),
        "resetFace" => unit_r(world::reset_face(t, &p())),
        "getPathableRandomLocalCoordinate" => {
            world::get_pathable_random_local_coordinate(t, &p(), i(a, 1), l(a, 2), 3).map(lj)
        }
        "getPathableRandomLocalCoordinate(attempts)" => {
            world::get_pathable_random_local_coordinate(t, &p(), i(a, 1), l(a, 2), i(a, 3)).map(lj)
        }
        "getPathableCardinal" => world::get_pathable_cardinal(t, &p(), l(a, 1)).map(lj),
        "registerMapZone" => unit_r(world::register_map_zone(t, &s(a, 0), borders(a, 1))),
        "addClimbDest" => unit_r(world::add_climb_dest(t, l(a, 0), l(a, 1))),
        "handleDoor" => unit_r(world::handle_door(t, &p(), &s(a, 1))),
        "handleAutowalkDoor" => bool_r(world::handle_autowalk_door(
            t,
            opt(a, 0).as_ref(),
            opt(a, 1).as_ref(),
        )),
        "hasLineOfSight" => bool_r(world::has_line_of_sight(t, &p(), &s(a, 1))),
        "isPlayer" => Ok(json!(world::is_player(t, &s(a, 0)))),
        // interfaces and dialogue
        "openInterface" => unit_r(ui::open_interface(t, &p(), i(a, 1))),
        "openOverlay" => unit_r(ui::open_overlay(t, &p(), i(a, 1))),
        "closeOverlay" => unit_r(ui::close_overlay(t, &p())),
        "openSingleTab" => unit_r(ui::open_single_tab(t, &p(), i(a, 1))),
        "closeInterface" => unit_r(ui::close_interface(t, &p())),
        "closeTabInterface" => unit_r(ui::close_tab_interface(t, &p())),
        "closeAllInterfaces" => unit_r(ui::close_all_interfaces(t, &p())),
        "setInterfaceText" | "sendString" => {
            unit_r(ui::set_interface_text(t, &p(), &s(a, 1), i(a, 2), i(a, 3)))
        }
        "setComponentVisibility" => unit_r(ui::set_component_visibility(
            t,
            &p(),
            i(a, 1),
            i(a, 2),
            b(a, 3),
        )),
        "setTitle" => unit_r(ui::set_title(t, &p(), i(a, 1))),
        "removeTabs" => {
            let tabs: Vec<GameTab> = strs(a, 1)
                .iter()
                .map(|x| GameTab::from_name(x).unwrap())
                .collect();
            unit_r(ui::remove_tabs(t, &p(), &tabs))
        }
        "restoreTabs" => unit_r(ui::restore_tabs(t, &p())),
        "sendPlayerOnInterface" => unit_r(ui::send_player_on_interface(t, &p(), i(a, 1), i(a, 2))),
        "sendAnimationOnInterface" => unit_r(ui::send_animation_on_interface(
            t,
            &p(),
            i(a, 1),
            i(a, 2),
            i(a, 3),
        )),
        "sendItemOnInterface" => unit_r(ui::send_item_on_interface(
            t,
            &p(),
            i(a, 1),
            i(a, 2),
            i(a, 3),
            1,
        )),
        "sendItemOnInterface(amount)" => unit_r(ui::send_item_on_interface(
            t,
            &p(),
            i(a, 1),
            i(a, 2),
            i(a, 3),
            i(a, 4),
        )),
        "sendItemZoomOnInterface" => unit_r(ui::send_item_zoom_on_interface(
            t,
            &p(),
            i(a, 1),
            i(a, 2),
            i(a, 3),
            230,
        )),
        "sendItemZoomOnInterface(zoom)" => unit_r(ui::send_item_zoom_on_interface(
            t,
            &p(),
            i(a, 1),
            i(a, 2),
            i(a, 3),
            i(a, 4),
        )),
        "sendMessage" => unit_r(ui::send_message(t, &p(), &s(a, 1))),
        "sendChat" => unit_r(ui::send_chat(t, &p(), &s(a, 1), -1)),
        "sendChat(delay)" => unit_r(ui::send_chat(t, &p(), &s(a, 1), i(a, 2))),
        "sendNews" => unit_r(ui::send_news(t, &s(a, 0))),
        "sendDialogue" => unit_r(ui::send_dialogue(t, &p(), &s(a, 1))),
        "sendDialogueLines" => unit_r(ui::send_dialogue_lines(t, &p(), &strs(a, 1))),
        "sendDialogueOptions" => unit_r(ui::send_dialogue_options(t, &p(), &s(a, 1), &strs(a, 2))),
        "sendPlayerDialogue" => unit_r(ui::send_player_dialogue(
            t,
            &p(),
            &s(a, 1),
            "NEUTRAL",
            false,
        )),
        "sendPlayerDialogue(all)" => unit_r(ui::send_player_dialogue(
            t,
            &p(),
            &s(a, 1),
            st(a, 2),
            b(a, 3),
        )),
        "sendNPCDialogue" => unit_r(ui::send_npc_dialogue(
            t,
            &p(),
            i(a, 1),
            &s(a, 2),
            "NEUTRAL",
            false,
        )),
        "sendNPCDialogue(all)" => unit_r(ui::send_npc_dialogue(
            t,
            &p(),
            i(a, 1),
            &s(a, 2),
            st(a, 3),
            b(a, 4),
        )),
        "sendNPCDialogueLines" => unit_r(ui::send_npc_dialogue_lines(
            t,
            &p(),
            i(a, 1),
            st(a, 2),
            b(a, 3),
            &strs(a, 4),
        )),
        "sendItemDialogue" => unit_r(ui::send_item_dialogue(t, &p(), &any(&a[1]), &s(a, 2))),
        "sendDoubleItemDialogue" => unit_r(ui::send_double_item_dialogue(
            t,
            &p(),
            &any(&a[1]),
            &any(&a[2]),
            &s(a, 3),
        )),
        "sendInputDialogue(numeric)" => unit_r(ui::send_input_dialogue(
            t,
            &p(),
            b(a, 1),
            &s(a, 2),
            &fnode(&s(a, 3)),
        )),
        "sendInputDialogue(type)" => {
            let kind = InputType::ALL
                .into_iter()
                .find(|k| k.name() == s(a, 1))
                .unwrap();
            unit_r(ui::send_input_dialogue_typed(
                t,
                &p(),
                kind,
                &s(a, 2),
                &fnode(&s(a, 3)),
            ))
        }
        "openDialogue" => unit_r(ui::open_dialogue(t, &p(), &any(&a[1]), &anys(a, 2))),
        "closeDialogue" => unit_r(ui::close_dialogue(t, &p())),
        "addDialogueAction" => unit_r(ui::add_dialogue_action(t, &p(), &fnode(&s(a, 1)))),
        "runcs2" => unit_r(ui::runcs2(t, &p(), i(a, 1), &anys(a, 2))),
        "sendItemSelect" => unit_r(ui::send_item_select(
            t,
            &p(),
            &strs(a, 1),
            false,
            &fnode(&s(a, 2)),
        )),
        "sendItemSelect(keepAlive)" => unit_r(ui::send_item_select(
            t,
            &p(),
            &strs(a, 1),
            b(a, 2),
            &fnode(&s(a, 3)),
        )),
        "sendSkillDialogue" => {
            let spec = &a[1];
            let items = if let Some(it) = spec.get("items") {
                ui::SkillItems::Items(
                    it.as_array()
                        .unwrap()
                        .iter()
                        .map(|x| x.as_str().unwrap().to_string())
                        .collect(),
                )
            } else if let Some(ids) = spec.get("ids") {
                ui::SkillItems::Ids(ids.as_array().unwrap().iter().map(int).collect())
            } else {
                ui::SkillItems::None
            };
            let create = (spec.get("create") == Some(&json!(true))).then(|| fnode("create"));
            let max = (spec.get("max") == Some(&json!(true))).then(|| fnode("max"));
            unit_r(ui::send_skill_dialogue(
                t,
                &p(),
                items,
                create.as_ref(),
                max.as_ref(),
            ))
        }
        "registerLogoutListener" => {
            ui::register_logout_listener(t, &p(), &s(a, 1), &fnode(&s(a, 2)));
            Ok(J::Null)
        }
        "clearLogoutListener" => {
            ui::clear_logout_listener(t, &p(), &s(a, 1));
            Ok(J::Null)
        }
        "registerHintIcon(location)" => {
            unit_r(ui::register_hint_icon_at(t, &p(), l(a, 1), i(a, 2)))
        }
        "registerHintIcon(node)" => unit_r(ui::register_hint_icon(t, &p(), &s(a, 1))),
        "clearHintIcon" => unit_r(ui::clear_hint_icon(t, &p())),
        // attributes, timers, locks
        "getAttribute" => {
            state::get_attribute(t, &p(), &s(a, 1), any(&a[2])).map(|v| value_json(&v))
        }
        "setAttribute" => unit_r(state::set_attribute(t, &p(), &s(a, 1), &any(&a[2]))),
        "removeAttribute" => unit_r(state::remove_attribute(t, &p(), &s(a, 1))),
        "removeAttributes" => {
            let keys = strs(a, 1);
            let refs: Vec<&str> = keys.iter().map(String::as_str).collect();
            unit_r(state::remove_attributes(t, &p(), &refs))
        }
        "registerTimer" => unit_r(state::register_timer(t, &p(), opt(a, 1).as_ref())),
        "getOrStartTimer" => {
            state::get_or_start_timer(t, &p(), &s(a, 1), &anys(a, 2)).map(|v| json!(v))
        }
        "getOrStartTimer<Poison>" => {
            state::get_or_start_timer_of(t, &p(), "Poison", &anys(a, 1)).map(|v| json!(v))
        }
        "spawnTimer" => state::spawn_timer(t, &s(a, 0), &anys(a, 1)).map(|v| json!(v)),
        "spawnTimer<Poison>" => state::spawn_timer_of(t, "Poison", &anys(a, 0)).map(|v| json!(v)),
        "hasTimerActive<Poison>" => bool_r(state::has_timer_active_of(t, &p(), "Poison")),
        "getTimer<Poison>" => state::get_timer_of(t, &p(), "Poison").map(|v| json!(v)),
        "removeTimer<Poison>" => unit_r(state::remove_timer(t, &p(), "Poison")),
        "hasTimerActive" => bool_r(state::has_timer_active(t, &p(), &s(a, 1))),
        "getTimer" => state::get_timer(t, &p(), &s(a, 1)).map(|v| json!(v)),
        "removeTimer(identifier)" => unit_r(state::remove_timer(t, &p(), &s(a, 1))),
        "removeTimer(timer)" => unit_r(state::remove_timer_instance(t, &p(), &s(a, 1))),
        "applyPoison" => unit_r(state::apply_poison(t, &p(), &s(a, 1), i(a, 2))),
        "isPoisoned" => bool_r(state::is_poisoned(t, &p())),
        "curePoison" => unit_r(state::cure_poison(t, &p())),
        "lock" => unit_r(state::lock(t, &p(), i(a, 1))),
        "lockInteractions" => unit_r(state::lock_interactions(t, &p(), i(a, 1))),
        "unlock" => unit_r(state::unlock(t, &p())),
        // varps (player::variables through content_api::vars)
        "getVarp"
        | "getVarbit"
        | "getVarbit(definition)"
        | "setVarp"
        | "setVarp(save)"
        | "saveVarp"
        | "unsaveVarp"
        | "setVarbit"
        | "setVarbit(save)"
        | "setVarbit(definition)"
        | "setVarc"
        | "reinitVarps" => varp_call(t, f, a),
        // pulses and scripts
        "getUsedOption" => state::get_used_option(t, &p()).map(|v| json!(v)),
        "submitWorldPulse" => unit_r(state::submit_world_pulse(
            t,
            Pulse {
                delay: i(a, 1),
                body: PulseBody::Content(fnode(&s(a, 0))),
            },
        )),
        "runWorldTask" => state::run_world_task(t, &fnode(&s(a, 0))).map(|v| json!(v)),
        "submitIndividualPulse" => unit_r(state::submit_individual_pulse(
            t,
            &p(),
            Pulse {
                delay: i(a, 2),
                body: PulseBody::Content(fnode(&s(a, 1))),
            },
        )),
        "runTask" => unit_r(state::run_task(t, &p(), 0, 1, &fnode(&s(a, 1)))),
        "runTask(delay,repeat)" => {
            unit_r(state::run_task(t, &p(), i(a, 1), i(a, 2), &fnode(&s(a, 3))))
        }
        "finishedMoving" => bool_r(state::finished_moving(t, &p())),
        "delayScript" => bool_r(state::delay_script(t, &p(), i(a, 1))),
        "delayEntity" => unit_r(state::delay_entity(t, &p(), i(a, 1))),
        "apRange" => unit_r(state::ap_range(t, &p(), i(a, 1))),
        "animationFinished" => bool_r(state::animation_finished(t, &p())),
        "clearScripts" => bool_r(state::clear_scripts(t, &p())),
        "restartScript" => bool_r(state::restart_script(t, &p())),
        "keepRunning" => bool_r(state::keep_running(t, &p())),
        "stopExecuting" => bool_r(state::stop_executing(t, &p())),
        "queueScript" => unit_r(state::queue_script(
            t,
            &p(),
            1,
            state::QueueStrength::Weak,
            false,
            ScriptBody::Content(fnode(&s(a, 1))),
        )),
        "queueScript(all)" => {
            let strength = [
                state::QueueStrength::Weak,
                state::QueueStrength::Normal,
                state::QueueStrength::Strong,
                state::QueueStrength::Soft,
            ]
            .into_iter()
            .find(|x| x.name() == s(a, 2))
            .unwrap();
            unit_r(state::queue_script(
                t,
                &p(),
                i(a, 1),
                strength,
                b(a, 3),
                ScriptBody::Content(fnode(&s(a, 4))),
            ))
        }
        "delayClock" => bool_r(state::delay_clock(t, &p(), i(a, 1), i(a, 2), false)),
        "delayClock(script)" => bool_r(state::delay_clock(t, &p(), i(a, 1), i(a, 2), b(a, 3))),
        "clockReady" => bool_r(state::clock_ready(t, &p(), i(a, 1))),
        "delayAttack" => unit_r(state::delay_attack(t, &p(), i(a, 1))),
        "stun" => unit_r(state::stun(t, &p(), i(a, 1), true)),
        "stun(message)" => unit_r(state::stun(t, &p(), i(a, 1), b(a, 2))),
        "isStunned" => bool_r(state::is_stunned(t, &p())),
        "setCurrentScriptState" => unit_r(state::set_current_script_state(t, &p(), i(a, 1))),
        // quests
        "getQuestPoints" => int_r(quests::get_quest_points(t, &p())),
        "getQuestStage" => int_r(quests::get_quest_stage(t, &p(), st(a, 1))),
        "setQuestStage" => unit_r(quests::set_quest_stage(t, &p(), st(a, 1), i(a, 2))),
        "isQuestInProgress" => bool_r(quests::is_quest_in_progress(
            t,
            &p(),
            st(a, 1),
            i(a, 2),
            i(a, 3),
        )),
        "isQuestStarted" => bool_r(quests::is_quest_started(t, &p(), st(a, 1))),
        "isQuestComplete" => bool_r(quests::is_quest_complete(t, &p(), st(a, 1))),
        "getQuest" => quests::get_quest(t, &p(), st(a, 1)).map(|v| json!(v)),
        "startQuest" => bool_r(quests::start_quest(t, &p(), st(a, 1))),
        "finishQuest" => unit_r(quests::finish_quest(t, &p(), st(a, 1))),
        "requireQuest" => bool_r(quests::require_quest(t, &p(), st(a, 1), &s(a, 2))),
        "hasRequirement(req)" => bool_r(quests::has_requirement(t, &p(), &s(a, 1), 100, true)),
        "hasRequirement(req,message)" => {
            bool_r(quests::has_requirement(t, &p(), &s(a, 1), 100, b(a, 2)))
        }
        "hasRequirement(quest)" => bool_r(quests::has_quest_requirement(t, &p(), &s(a, 1), true)),
        "hasRequirement(quest,message)" => {
            bool_r(quests::has_quest_requirement(t, &p(), &s(a, 1), b(a, 2)))
        }
        "finishTask" => unit_r(quests::finish_task(t, &p(), st(a, 1), &s(a, 2), i(a, 3))),
        "getDiaryLevelIndex" => int_r(quests::get_diary_level_index(t, &s(a, 0), &s(a, 1))),
        // slayer, ironman, bank, shops, house, credits
        "getSlayerTask" => services::get_slayer_task(t, &p()).map(|v| json!(v)),
        "getSlayerTaskName" => services::get_slayer_task_name(t, &p()).map(|v| json!(v)),
        "getSlayerTaskKillsRemaining" => int_r(services::get_slayer_task_kills_remaining(t, &p())),
        "getSlayerMaster" => services::get_slayer_master(t, &p()).map(|v| json!(v)),
        "getSlayerMasterLocation" => {
            services::get_slayer_master_location(t, &p()).map(|v| json!(v))
        }
        "getSlayerTip" => services::get_slayer_tip(t, &p()).map(|v| json!(v)),
        "getSlayerTaskFlags" => int_r(services::get_slayer_task_flags(t, &p())),
        "getSlayerFlags" => services::get_slayer_flags(t, &p()).map(|_| json!("SlayerFlags")),
        "hasSlayerTask" => bool_r(services::has_slayer_task(t, &p())),
        "hasIronmanRestriction" => bool_r(services::has_ironman_restriction(t, &p(), &s(a, 1))),
        "restrictForIronman" => unit_r(services::restrict_for_ironman_callback(
            t,
            &p(),
            st(a, 1),
            &fnode(&s(a, 2)),
        )),
        "openGrandExchange" => unit_r(services::open_grand_exchange(t, &p())),
        "hasAwaitingGrandExchangeCollections" => {
            bool_r(services::has_awaiting_grand_exchange_collections(t, &p()))
        }
        "openGrandExchangeCollectionBox" => {
            unit_r(services::open_grand_exchange_collection_box(t, &p()))
        }
        "openBankAccount" => unit_r(services::open_bank_account(t, &p())),
        "openDepositBox" => unit_r(services::open_deposit_box(t, &p())),
        "openBankPinSettings" => unit_r(services::open_bank_pin_settings(t, &p())),
        "activateSecondaryBankAccount" => {
            services::activate_secondary_bank_account(t, &p()).map(|v| json!(v.name()))
        }
        "hasActivatedSecondaryBankAccount" => {
            bool_r(services::has_activated_secondary_bank_account(t, &p()))
        }
        "toggleBankAccount" => unit_r(services::toggle_bank_account(t, &p())),
        "isUsingSecondaryBankAccount" => {
            Ok(json!(services::is_using_secondary_bank_account(t, &p())))
        }
        "getBankAccountName" => Ok(json!(services::get_bank_account_name(t, &p(), false))),
        "getBankAccountName(invert)" => {
            Ok(json!(services::get_bank_account_name(t, &p(), b(a, 1))))
        }
        "openNpcShop" => bool_r(services::open_npc_shop(t, &p(), i(a, 1))),
        "hasHouse" => bool_r(services::has_house(t, &p())),
        "storeBookInHouse(id)" => bool_r(services::store_book_in_house(t, &p(), i(a, 1))),
        "storeBookInHouse(node)" => bool_r(services::store_book_node_in_house(t, &p(), &s(a, 1))),
        "getCutscene" => services::get_cutscene(t, &p()).map(|v| json!(v)),
        "getCutsceneStage" => int_r(services::get_cutscene_stage(t, &p())),
        "updateCredits" => bool_r(services::update_credits(t, &p(), i(a, 1))),
        "getCredits" => int_r(services::get_credits(t, &p())),
        "announceIfRare" => unit_r(services::announce_if_rare(t, &p(), &s(a, 1))),
        // logging and configuration
        "log" | "logWithStack" => {
            let level = services::LogLevel::ALL
                .into_iter()
                .find(|x| x.name() == s(a, 0))
                .unwrap();
            if f == "log" {
                unit_r(services::log(t, "Calls", level, &s(a, 1)))
            } else {
                unit_r(services::log_with_stack(t, "Calls", level, &s(a, 1)))
            }
        }
        "exceptionToString" => Ok(json!(services::exception_to_string(t, &s(a, 0)))),
        "getServerConfig" => services::get_server_config(t).map(|v| json!(v)),
        // the other core/api files
        "splitLines" => ui::split_lines(&s(a, 0), 54).map(|v| json!(v)),
        "splitLines(limit)" => ui::split_lines(&s(a, 0), i(a, 1)).map(|v| json!(v)),
        "EquipmentSlot.slotByName" => Ok(json!(
            EquipmentSlot::slot_by_name(&s(a, 0)).map(|x| x.name())
        )),
        "God.validItems" => Ok(json!(God::from_name(&s(a, 0)).unwrap().valid_items())),
        "IfaceSettingsBuilder" => iface(&a[0]).map(|v| json!(v)),
        "asItem" => files::as_item(t, i(a, 0)).map(|v| json!(v)),
        "IntProgression.toIntArray" => {
            files::progression_to_int_array(i(a, 0), i(a, 1), i(a, 2)).map(|v| json!(v))
        }
        "Collection<IntArray>.toIntArray" => {
            let arrays: Vec<Vec<i32>> = a[0]
                .as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_array().unwrap().iter().map(int).collect())
                .collect();
            Ok(json!(files::flatten_int_arrays(&arrays)))
        }
        "IntArray.isLast" | "Collection.isLast" => Ok(json!(files::is_last(&ints(a, 0), &i(a, 1)))),
        "IntArray.getNext" | "Collection.getNext" => {
            Ok(json!(files::get_next(&ints(a, 0), &i(a, 1))))
        }
        "IntArray.isNextLast" | "Collection.isNextLast" => {
            Ok(json!(files::is_next_last(&ints(a, 0), &i(a, 1))))
        }
        "tryPop" => {
            let mut list: std::collections::VecDeque<i32> = ints(a, 0).into();
            let popped = files::try_pop(&mut list, a[1].as_i64().map(|v| v as i32));
            Ok(json!([popped, list]))
        }
        "parseEnumEntry<God>" => Ok(json!(
            files::parse_enum_entry(&God::ALL, God::name, &s(a, 0)).map(God::name)
        )),
        "Event" => Ok(json!(files::EVENTS.to_vec())),
        "Commands.reject" => unit_r(files::reject(t, &p(), &strs(a, 1))),
        "Commands.notify" => unit_r(files::notify(t, "", &p(), &s(a, 1), false)),
        "Commands.notify(log)" => unit_r(files::notify(t, "", &p(), &s(a, 1), b(a, 2))),
        "Commands.define" => unit_r(files::define(
            t,
            &s(a, 0),
            "ADMIN",
            "",
            "",
            &fnode(&s(a, 1)),
        )),
        "Commands.define(all)" => unit_r(files::define(
            t,
            &s(a, 0),
            st(a, 1),
            &s(a, 2),
            &s(a, 3),
            &fnode(&s(a, 4)),
        )),
        "MapArea.zone" => {
            let before = match files::map_area_zone(t, "AreaA") {
                Ok(z) => json!(z),
                Err(_) => json!("Null"),
            };
            files::set_map_area_zone(t, "AreaA", "zoneA".into());
            let after = files::map_area_zone(t, "AreaA")?;
            let mut keys: Vec<String> = t.zone_maps().iter().map(|(k, _)| k.clone()).collect();
            keys.sort();
            Ok(json!([before, after, keys, 0, null, null]))
        }
        "Vector" => {
            let v = &a[0];
            let f = |k: usize| v[k].as_f64().unwrap();
            let x = files::Vector::new(f(0), f(1));
            let w = files::Vector::new(f(2), f(3));
            Ok(json!([
                {"double": dj(x.magnitude())},
                x.normalized().to_string(),
                x.times(2.5).to_string(),
                x.times_int(3).to_string(),
                x.plus(w).to_string(),
                x.minus(w).to_string(),
                x.invert().to_string(),
                lj(x.to_location(int(&v[4]))),
                x.to_direction().name(),
                files::Vector::between_locs(Location::new(1, 2, 0), Location::new(4, -2, 1)).to_string(),
                files::Vector::derive_with_equal_components(f(0)).to_string(),
            ]))
        }
        "regionspec" => {
            let (region, chunks) = match s(a, 0).as_str() {
                "empty" => (
                    files::RegionContract::Empty,
                    vec![files::ChunkContract::Empty],
                ),
                "copy" => (
                    files::RegionContract::Clone(12850),
                    vec![files::ChunkContract::Empty],
                ),
                "using" => {
                    let dynamic = t
                        .call(Member::DynamicRegionCreate, vec![Arg::Int(1)])?
                        .node()?
                        .ok_or(Error::Null)?;
                    (
                        files::RegionContract::Existing(dynamic),
                        vec![files::ChunkContract::Empty],
                    )
                }
                "fill" => {
                    let mut fill = files::FillChunk::with_chunk(Some("chunk".into()));
                    fill.source = Some("src".into());
                    fill.planes = vec![1, 2];
                    fill.condition = Some(fnode("cond"));
                    (
                        files::RegionContract::Existing("dyn".into()),
                        vec![files::ChunkContract::Fill(fill)],
                    )
                }
                "fillDelegate" => {
                    let mut fill = files::FillChunk::with_delegate(fnode("delegate"));
                    fill.source = Some("src".into());
                    fill.condition = Some(fnode("cond"));
                    (
                        files::RegionContract::Existing("dyn".into()),
                        vec![files::ChunkContract::Fill(fill)],
                    )
                }
                "fillLateinit" => {
                    let fill = files::FillChunk::with_chunk(Some("chunk".into()));
                    (
                        files::RegionContract::Existing("dyn".into()),
                        vec![files::ChunkContract::Fill(fill)],
                    )
                }
                other => panic!("{other}"),
            };
            files::build_region(t, &region, &chunks).map(|v| json!(v))
        }
        other => panic!("no dispatch for {other}"),
    }
}

fn t_arg_borders(b: [i32; 5]) -> J {
    json!(["borders", b[0], b[1], b[2], b[3], b[4]])
}

fn iface(steps: &J) -> api::Result<i32> {
    let mut b = files::IfaceSettingsBuilder::default();
    for step in steps.as_array().unwrap() {
        let s = step.as_array().unwrap();
        let n = |k: usize| int(&s[k]);
        b = match s[0].as_str().unwrap() {
            "enableOption" => b.enable_option(n(1))?,
            "enableOptions(ids)" => {
                b.enable_options(&s[1..].iter().map(int).collect::<Vec<_>>())?
            }
            "enableOptions(range)" => b.enable_options_range(n(1), n(2))?,
            "enableOptions(names)" => b.enable_named_options(s.len() - 1)?,
            "enableAllOptions" => b.enable_all_options()?,
            "setUseOnSettings" => b.set_use_on_settings(
                s[1].as_bool().unwrap(),
                s[2].as_bool().unwrap(),
                s[3].as_bool().unwrap(),
                s[4].as_bool().unwrap(),
                s[5].as_bool().unwrap(),
                s[6].as_bool().unwrap(),
            ),
            "setInterfaceEventsDepth" => b.set_interface_events_depth(n(1))?,
            "enableSlotSwitch" => b.enable_slot_switch(),
            "enableUseOption" => b.enable_use_option(),
            "enableExamine" => b.enable_examine(),
            "enableUseOnSelf" => b.enable_use_on_self(),
            "enableNullSlotSwitch" => b.enable_null_slot_switch(),
            other => panic!("{other}"),
        };
    }
    Ok(b.build())
}

fn step(t: &mut T, s: &J) -> api::Result<J> {
    match s["step"].as_str().unwrap() {
        "pulse" => {
            let index = s["index"].as_u64().unwrap() as usize;
            let (label, mut pulse) = t.pulses[index].clone();
            let done = state::run_pulse(t, &mut pulse);
            t.pulses[index].1 = pulse;
            let done = done?;
            t.events.push(json!(["@pulse", label, done]));
            Ok(json!(done))
        }
        "script" => {
            let index = s["index"].as_u64().unwrap() as usize;
            let stage = int(&s["stage"]);
            let (label, mut script) = t.scripts[index].clone();
            let done = state::run_script(t, &mut script, stage);
            t.scripts[index].1 = script;
            let done = done?;
            t.events.push(json!(["@script", label, stage, done]));
            Ok(json!(done))
        }
        "create" => {
            let handler = t.handlers[0].1.clone();
            ui::skill_create(t, &handler, int(&s["amount"]), int(&s["index"])).map(|_| J::Null)
        }
        "getAll" => {
            let handler = t.handlers[0].1.clone();
            ui::skill_get_all(t, &handler, int(&s["index"])).map(|v| json!(v))
        }
        "ticks" => {
            t.ticks = int(&s["ticks"]);
            Ok(J::Null)
        }
        other => panic!("{other}"),
    }
}

fn map_string<T: std::fmt::Display>(m: &HashMap<i32, Option<T>>) -> String {
    let sorted: BTreeMap<i32, String> = m
        .iter()
        .map(|(k, v)| (*k, v.as_ref().map_or("null".into(), |x| x.to_string())))
        .collect();
    let parts: Vec<String> = sorted.iter().map(|(k, v)| format!("{k}={v}")).collect();
    format!("{{{}}}", parts.join(", "))
}

fn snapshot(t: &T) -> J {
    let mut out = serde_json::Map::new();
    for (k, n) in &t.nodes {
        let mut m = serde_json::Map::new();
        let entity = matches!(n.kind.as_str(), "player" | "npc" | "familiar" | "bob");
        if n.kind == "player" {
            let (values, saved) = &t.values[k];
            m.insert("varps".into(), json!(map_string(&values.snapshot())));
            m.insert("saved".into(), json!(map_string(&saved.snapshot())));
            let mut l = n.listeners.clone();
            l.sort();
            m.insert("listeners".into(), json!(format!("[{}]", l.join(", "))));
            m.insert("secondary".into(), json!(n.secondary));
        }
        if entity {
            m.insert("clocks".into(), json!(n.clocks));
        }
        if entity && n.kind != "player" {
            m.insert("invisible".into(), json!(n.invisible));
        }
        if n.kind == "item" || n.kind == "grounditem" {
            m.insert(
                "item".into(),
                json!([n.id_hash >> 16 & 0xFFFF, n.amount, n.id_hash & 0xFFFF]),
            );
        }
        if n.kind == "scenery" {
            m.insert("charge".into(), json!(n.charge));
        }
        if n.kind == "script" {
            m.insert("script".into(), json!([n.next_execution, n.script_state]));
        }
        if n.kind == "poison" {
            m.insert("poison".into(), json!([n.severity, n.source]));
        }
        if !m.is_empty() {
            out.insert(k.clone(), J::Object(m));
        }
    }
    out.insert("@tlr".into(), json!({"long": t.random.seed}));
    J::Object(out)
}

fn replay(case: &J, quests: &HashMap<String, String>) -> J {
    let mut t = T::new(case, quests);
    let f = case["fn"].as_str().unwrap();
    let args = case["args"].as_array().unwrap().clone();
    let outcome = run(&mut t, f, &args);
    let (result, error) = match outcome {
        Ok(v) => (v, J::Null),
        Err(e) => (J::Null, json!(e.code())),
    };
    let mut steps = Vec::new();
    if error.is_null() {
        for s in case
            .get("steps")
            .and_then(J::as_array)
            .cloned()
            .unwrap_or_default()
        {
            match step(&mut t, &s) {
                Ok(v) => steps.push(json!([v, null])),
                Err(e) => {
                    steps.push(json!([null, e.code()]));
                    break;
                }
            }
        }
    }
    json!({"events": t.events, "result": result, "error": error, "steps": steps, "state": snapshot(&t)})
}

#[test]
fn fixture_is_pinned() {
    let p: J = serde_json::from_str(PROVENANCE).unwrap();
    let digest = format!("{:x}", Sha256::digest(FIXTURE.as_bytes()));
    assert_eq!(p["fixtureSha256"], json!(digest));
    assert_eq!(p["caseCount"], json!(FIXTURE.lines().count()));
}

#[test]
fn quest_requirements_match_the_original_table() {
    let p: J = serde_json::from_str(PROVENANCE).unwrap();
    let table: Vec<(String, String)> = p["questRequirements"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            (
                r[0].as_str().unwrap().to_string(),
                r[1].as_str().unwrap().to_string(),
            )
        })
        .collect();
    let native: Vec<(String, String)> = quests::QUEST_REQUIREMENTS
        .iter()
        .map(|(a, b)| (a.to_string(), b.to_string()))
        .collect();
    assert_eq!(native, table);
}

#[test]
fn every_member_name_is_recorded_by_the_corpus() {
    let mut seen = std::collections::HashSet::new();
    for line in FIXTURE.lines() {
        let row: J = serde_json::from_str(line).unwrap();
        for e in row["expected"]["events"].as_array().unwrap() {
            seen.insert(e[0].as_str().unwrap().to_string());
        }
    }
    let unreached: Vec<&str> = Member::ALL
        .iter()
        .map(|m| m.name())
        .filter(|n| !seen.contains(*n))
        .collect();
    // Members only reached through paths the corpus does not drive.
    let allowed: [&str; 0] = [];
    assert!(
        unreached
            .iter()
            .all(|n| allowed.contains(n) || seen.contains(*n)),
        "{unreached:?}"
    );
}

#[test]
fn replays_the_original_core_api() {
    let p: J = serde_json::from_str(PROVENANCE).unwrap();
    let quests: HashMap<String, String> = p["quests"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            (
                r[0].as_str().unwrap().to_string(),
                r[1].as_str().unwrap().to_string(),
            )
        })
        .collect();
    let mut failures = Vec::new();
    let mut count = 0;
    for line in FIXTURE.lines() {
        let case: J = serde_json::from_str(line).unwrap();
        let expected = case["expected"].clone();
        let got = replay(&case, &quests);
        count += 1;
        if got != expected {
            failures.push(format!(
                "{}\n  expected {}\n  got      {}",
                case["name"], expected, got
            ));
        }
    }
    if let Ok(path) = std::env::var("CAPI_DUMP") {
        std::fs::write(path, failures.join("\n")).unwrap();
    }
    assert!(
        failures.is_empty(),
        "{} of {count} cases differ:\n{}",
        failures.len(),
        failures
            .iter()
            .take(8)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
    assert_eq!(count, 1635);
}
