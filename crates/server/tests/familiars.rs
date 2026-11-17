//! Replays the familiar framework corpus (fixtures/familiars) through
//! `world::familiars`. Every service answers from the case the way the
//! oracle's recording stand-ins do (oracle/familiars/adapters.py and
//! Support.java); every recorded call, result, failure class and the
//! snapshot after each operation (the manager with its pet details, the
//! registry, every familiar seen with its NPC's fields, the player) are
//! compared.
use rs09_game_core::java_json;
use rs09_server::world::familiars::{
    self as fm, CombatStyle, Error, FamiliarSpecial, Item, Loc, LogLevel, Manager, NodeKind,
    Origin, PetDetails, PetStore, Pouch, Pulse, Registry, Scroll, Shape, SkillBonus, Spec,
    pets::{self, PET_NPC_IDS, PETS, Pet},
};
use serde_json::{Map, Value as Json, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};

const CORPUS: &str = include_str!("../../../fixtures/familiars/oracle.jsonl");
const PROVENANCE: &str = include_str!("../../../fixtures/familiars/provenance.json");

type N = String;
type Familiar = fm::Familiar<World>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Class {
    Plain,
    Beast,
    Forager,
    Pet,
}
impl Class {
    fn name(self) -> &'static str {
        match self {
            Self::Plain => "oracle.CaseFamiliar",
            Self::Beast => "oracle.CaseBeast",
            Self::Forager => "oracle.CaseForager",
            Self::Pet => "content.global.skill.summoning.pet.Pet",
        }
    }
}

#[derive(Clone, Debug)]
enum Attr {
    Null,
    Int(i32),
    Bool(bool),
    Node(N),
}
impl Attr {
    fn json(&self) -> Json {
        match self {
            Self::Null => Json::Null,
            Self::Int(i) => json!(i),
            Self::Bool(b) => json!(b),
            Self::Node(k) => json!(k),
        }
    }
}

#[derive(Clone, Debug)]
struct NodeState {
    kind: NodeKind,
    id: i32,
    original: i32,
    name: String,
    location: Option<Loc>,
    invisible: bool,
    attributes: BTreeMap<String, Attr>,
}

fn int(v: &Json) -> i32 {
    v.as_i64().unwrap() as i32
}
fn loc(v: &Json) -> Option<Loc> {
    v.as_array().map(|a| [int(&a[0]), int(&a[1]), int(&a[2])])
}
fn coord(l: Option<Loc>) -> Json {
    l.map_or(Json::Null, |l| json!(l))
}
fn bits(d: f64) -> Json {
    json!({ "double": d.to_bits() as i64 })
}
fn item_json(i: Option<&Item>) -> Json {
    i.map_or(Json::Null, |i| json!({ "item": [i.id, i.amount, i.slot] }))
}
fn items_json(items: &[Option<Item>]) -> Json {
    Json::Array(items.iter().map(|i| item_json(i.as_ref())).collect())
}
fn item_of(v: &Json) -> Option<Item> {
    let a = v.as_array()?;
    let mut item = Item::new(int(&a[0]), int(&a[1]));
    if a.len() > 2 {
        item.slot = int(&a[2]);
    }
    Some(item)
}
fn style_name(s: CombatStyle) -> &'static str {
    match s {
        CombatStyle::Melee => "MELEE",
        CombatStyle::Range => "RANGE",
        CombatStyle::Magic => "MAGIC",
    }
}
fn style_of(s: &str) -> CombatStyle {
    match s {
        "MELEE" => CombatStyle::Melee,
        "RANGE" => CombatStyle::Range,
        "MAGIC" => CombatStyle::Magic,
        other => panic!("style {other}"),
    }
}
fn error_name(e: &Error) -> &'static str {
    match e {
        Error::Null => "Null",
        Error::Cast => "Cast",
        Error::Number => "Number",
        Error::Index => "Index",
        Error::Binding(_) => "Binding",
    }
}

struct World {
    input: Json,
    nodes: BTreeMap<N, NodeState>,
    npcs: i32,
    containers: i32,
    states: i32,
    version: i32,
    answers: Map<String, Json>,
    answer_calls: HashMap<String, usize>,
    events: Vec<Json>,
    calls: HashMap<String, i64>,
    op_calls: i64,
    fail_at: i64,
    pulses: Vec<Pulse<World>>,
    close_events: Vec<(N, i32)>,
    seen: Vec<Familiar>,
    labels: Vec<(usize, fm::Details)>,
    managers: BTreeMap<N, Manager<World>>,
    registry: Registry<World>,
}

impl World {
    fn new(case: &Json) -> Self {
        let mut w = Self {
            input: case.clone(),
            nodes: BTreeMap::new(),
            npcs: 0,
            containers: 0,
            states: 0,
            version: case["player"]["version"].as_i64().unwrap_or(2) as i32,
            answers: case["answers"].as_object().cloned().unwrap_or_default(),
            answer_calls: HashMap::new(),
            events: Vec::new(),
            calls: HashMap::new(),
            op_calls: 0,
            fail_at: 0,
            pulses: Vec::new(),
            close_events: Vec::new(),
            seen: Vec::new(),
            labels: Vec::new(),
            managers: BTreeMap::new(),
            registry: Registry::default(),
        };
        let mut player = w.node_state(NodeKind::Player, -1);
        player.location = loc(&case["player"]["location"]).or(Some([3200, 3200, 0]));
        if let Some(attrs) = case["attributes"].as_object() {
            for (k, v) in attrs {
                player.attributes.insert(k.clone(), Self::attr(v));
            }
        }
        w.nodes.insert("p".into(), player);
        if let Some(nodes) = case["nodes"].as_object() {
            for (k, d) in nodes {
                let kind = match d["kind"].as_str() {
                    Some("player") => NodeKind::Player,
                    Some("npc") => NodeKind::Npc,
                    _ => NodeKind::Other,
                };
                let default_id = if kind == NodeKind::Other { 995 } else { 1 };
                let id = d.get("id").map_or(default_id, int);
                let mut s = w.node_state(kind, id);
                s.location = loc(&d["location"]);
                s.invisible = d["invisible"].as_bool().unwrap_or(false);
                if let Some(attrs) = d["attributes"].as_object() {
                    for (a, v) in attrs {
                        s.attributes.insert(a.clone(), Self::attr(v));
                    }
                }
                w.nodes.insert(k.clone(), s);
            }
        }
        w
    }
    fn attr(v: &Json) -> Attr {
        match v {
            Json::Null => Attr::Null,
            Json::Bool(b) => Attr::Bool(*b),
            Json::Number(n) => Attr::Int(n.as_i64().unwrap() as i32),
            Json::Object(m) => Attr::Node(m["node"].as_str().unwrap().into()),
            other => panic!("attribute {other}"),
        }
    }
    fn node_state(&self, kind: NodeKind, id: i32) -> NodeState {
        NodeState {
            kind,
            id,
            original: id,
            name: self.npc_name(id),
            location: None,
            invisible: false,
            attributes: BTreeMap::new(),
        }
    }
    fn npc_name(&self, id: i32) -> String {
        self.input["npcNames"][id.to_string()]
            .as_str()
            .unwrap_or("null")
            .into()
    }
    fn class_spec(&self, id: i32) -> Json {
        let classes = self.input["classes"].as_object().unwrap();
        if let Some(c) = classes.get(&id.to_string()) {
            return c.clone();
        }
        let mut keys: Vec<&String> = classes.keys().collect();
        keys.sort();
        for k in keys {
            if classes[k]["ids"]
                .as_array()
                .unwrap()
                .iter()
                .any(|i| int(i) == id)
            {
                return classes[k].clone();
            }
        }
        panic!("no class for {id}")
    }
    fn item_spec(&self, id: i32) -> Json {
        self.input["items"][id.to_string()].clone()
    }

    // ------------------------------------------------------------ recording
    fn rec(&mut self, name: &str, args: Vec<Json>) -> fm::Result<()> {
        let mut row = vec![json!(name)];
        row.extend(args);
        self.events.push(Json::Array(row));
        self.op_calls += 1;
        let count = {
            let c = self.calls.entry(name.into()).or_insert(0);
            *c += 1;
            *c
        };
        let hooks = self.input["hooks"].as_array().cloned().unwrap_or_default();
        for hook in hooks {
            if hook["event"] == name && count == hook.get("at").and_then(Json::as_i64).unwrap_or(1)
            {
                for action in hook["actions"].as_array().unwrap() {
                    if action["op"] == "fail" {
                        return Err(Error::Binding("hook".into()));
                    }
                    self.op(&action["do"])?;
                }
            }
        }
        if self.op_calls == self.fail_at {
            return Err(Error::Binding("failAt".into()));
        }
        Ok(())
    }
    fn answer(&mut self, name: &str, receiver: Option<&str>, fallback: Json) -> Json {
        let key = match receiver {
            Some(r) if self.answers.contains_key(&format!("{name}@{r}")) => format!("{name}@{r}"),
            _ => name.to_string(),
        };
        let Some(v) = self.answers.get(&key).cloned() else {
            return fallback;
        };
        if let Json::Array(l) = v {
            let i = {
                let c = self.answer_calls.entry(key).or_insert(0);
                *c += 1;
                *c - 1
            };
            return l[i.min(l.len() - 1)].clone();
        }
        v
    }
    fn bool(&mut self, name: &str, receiver: Option<&str>, fallback: bool) -> bool {
        self.answer(name, receiver, json!(fallback)) == json!(true)
    }
    fn integer(&mut self, name: &str, receiver: Option<&str>, fallback: i32) -> i32 {
        int(&self.answer(name, receiver, json!(fallback)))
    }
    fn node(&mut self, k: &N) -> &mut NodeState {
        self.nodes.get_mut(k).unwrap_or_else(|| panic!("node {k}"))
    }

    // ------------------------------------------------------------ objects
    fn kept(&mut self, f: &Familiar) {
        if !self.seen.iter().any(|s| s.same(f)) {
            self.seen.push(f.clone());
        }
    }
    fn create(&mut self, id: i32, owner: Option<N>) -> fm::Result<Familiar> {
        let s = self.class_spec(id);
        let style = s.get("style").map_or(fm::STYLE_DEFENSIVE, int);
        let spec = Spec {
            id,
            ticks: int(&s["ticks"]),
            pouch_id: int(&s["pouch"]),
            special_cost: int(&s["cost"]),
            attack_style: style,
        };
        let (class, shape) = match s["type"].as_str().unwrap() {
            "plain" => (Class::Plain, Shape::Plain),
            "beast" => (
                Class::Beast,
                Shape::Burden {
                    container_size: int(&s["size"]),
                },
            ),
            "forager" => (
                Class::Forager,
                Shape::Forager {
                    items: s["items"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|e| Item::new(int(&e[0]), int(&e[1])))
                        .collect(),
                },
            ),
            other => panic!("{other}"),
        };
        let f = Familiar::new(self, class, owner, spec, shape)?;
        for b in s["boosts"].as_array().cloned().unwrap_or_default() {
            f.add_boost(SkillBonus {
                skill: int(&b[0]),
                bonus: b[1].as_f64().unwrap(),
            });
        }
        Ok(f)
    }
    /// Every familiar object the case can name: the managers' current ones
    /// and those seen.
    fn familiar_of(&self, k: &str) -> Option<Familiar> {
        let current = self.managers.values().filter_map(|m| m.familiar());
        current
            .chain(self.seen.iter().cloned())
            .chain(
                self.registry
                    .ids()
                    .into_iter()
                    .filter_map(|i| self.registry.get(i)),
            )
            .find(|f| f.npc() == k)
    }
    fn manager_of(&mut self, k: &N) -> Manager<World> {
        self.managers
            .entry(k.clone())
            .or_insert_with(|| Manager::new(k.clone(), PetStore::default()))
            .clone()
    }
    fn target(&self, op: &Json) -> fm::Result<Familiar> {
        match op.get("fam").and_then(Json::as_str) {
            None => self
                .managers
                .get("p")
                .and_then(|m| m.familiar())
                .ok_or(Error::Null),
            Some(k) => self.familiar_of(k).ok_or(Error::Cast),
        }
    }

    // ------------------------------------------------------------ operations
    fn op(&mut self, op: &Json) -> fm::Result<Json> {
        let m = self.manager_of(&"p".to_string());
        let name = op["op"].as_str().unwrap();
        let s = |k: &str| op[k].as_str().map(str::to_owned);
        let b = |k: &str| op[k].as_bool().unwrap_or(false);
        let i = |k: &str| int(&op[k]);
        let null = Ok(Json::Null);
        match name {
            "setLocation" => {
                let l = loc(&op["location"]);
                self.node(&s("node").unwrap()).location = l;
                null
            }
            "setInvisible" => {
                let v = b("value");
                self.node(&s("node").unwrap()).invisible = v;
                null
            }
            "setAttribute" => {
                let v = Self::attr(&op["value"]);
                let key = s("key").unwrap();
                self.node(&s("node").unwrap()).attributes.insert(key, v);
                null
            }
            "register" => {
                let owner = s("owner");
                let proto = self.create(i("id"), owner)?;
                let registry = self.registry.clone();
                Ok(registry
                    .new_instance(self, &proto)?
                    .map_or(Json::Null, |f| json!(f.npc())))
            }
            "newFamiliar" => {
                let f = self.create(i("id"), Some(s("owner").unwrap_or("p".into())))?;
                self.kept(&f);
                Ok(json!(f.npc()))
            }
            "construct" => {
                let proto = self.registry.get(i("id")).ok_or(Error::Null)?;
                let owner = s("owner").unwrap_or("p".into());
                let f = fm::Host::construct(self, &proto, &owner, i("id"))?;
                self.kept(&f);
                Ok(json!(f.npc()))
            }
            "newPet" => {
                let d = PetDetails::new(op.get("growth").map_or(0.0, |g| g.as_f64().unwrap()));
                let f =
                    Familiar::new_pet(self, Class::Pet, Some("p".into()), d, i("item"), i("id"))?;
                self.kept(&f);
                Ok(json!(f.npc()))
            }
            "setFamiliar" => {
                let f = match s("fam") {
                    None => None,
                    Some(k) => Some(self.familiar_of(&k).ok_or(Error::Cast)?),
                };
                m.set_familiar(f);
                null
            }
            "parse" => {
                let data = java_json::parse(op["json"].as_str().unwrap()).unwrap();
                m.parse(self, &data)?;
                null
            }
            "login" => m.login(self).and(null),
            "summon" => {
                let delete = if op.get("delete").is_some() {
                    b("delete")
                } else {
                    true
                };
                m.summon(self, item_of(&op["item"]).unwrap(), b("pet"), delete)
                    .and(null)
            }
            "morphPet" => m
                .morph_pet(
                    self,
                    item_of(&op["item"]).unwrap(),
                    b("delete"),
                    loc(&op["location"]),
                    op["hunger"].as_f64().unwrap(),
                    op["growth"].as_f64().unwrap(),
                )
                .and(null),
            "morphFamiliar" => m.morph_familiar(self, loc(&op["location"])).and(null),
            "spawnFamiliar" => m.spawn_familiar(self).and(null),
            "eat" => {
                let f = self.target(op);
                let f = match f {
                    Ok(f) if !f.is_pet() => return Err(Error::Cast),
                    Ok(f) => f,
                    Err(_) => return Err(Error::Null),
                };
                m.eat(self, i("food"), &f).and(null)
            }
            "pickup" => m.pickup(self).and(null),
            "adjustBattleState" => {
                let mut state = fm::Host::battle_state(self, &"p".into(), &s("victim").unwrap())?;
                m.adjust_battle_state(self, &mut state).and(null)
            }
            "getBoost" => Ok(json!(m.get_boost(i("skill")))),
            "hasFamiliar" => Ok(json!(m.has_familiar())),
            "hasPet" => Ok(json!(m.has_pet())),
            "dismiss" => m.dismiss(self).and(null),
            "addDetails" => {
                let d = match s("from") {
                    Some(k) => self.familiar_of(&k).unwrap().pet_details()?,
                    None => PetDetails::new(op.get("growth").map_or(0.0, |g| g.as_f64().unwrap())),
                };
                m.add_details(i("item"), d);
                null
            }
            "removeDetails" => m.remove_details(i("item")).and(null),
            "isOwner" => {
                let f = self.familiar_of(&s("fam").unwrap()).ok_or(Error::Cast)?;
                Ok(json!(m.is_owner(self, &f)?))
            }
            "isUsingSummoning" => Ok(json!(m.is_using_summoning())),
            "setHasPouch" => {
                m.set_has_pouch(b("value"));
                Ok(json!(m.has_pouch()))
            }
            "setCombatLevel" => {
                m.set_summoning_combat_level(i("value"));
                Ok(json!(m.summoning_combat_level()))
            }
            _ => self.familiar_op(op, name),
        }
    }

    fn familiar_op(&mut self, op: &Json, name: &str) -> fm::Result<Json> {
        let s = |k: &str| op[k].as_str().map(str::to_owned);
        let b = |k: &str| op[k].as_bool().unwrap_or(false);
        let i = |k: &str| int(&op[k]);
        let null = Ok(Json::Null);
        match name {
            "petIds" => {
                Familiar::new_pet(
                    self,
                    Class::Pet,
                    Some("p".into()),
                    PetDetails::new(0.0),
                    1555,
                    761,
                )?;
                return Ok(json!(PET_NPC_IDS));
            }
            "petsForId" => return Ok(Pet::for_id(i("item")).map_or(Json::Null, |p| json!(p.name))),
            "petsNpcId" => {
                return Ok(json!(
                    pet_named(&s("pet").unwrap()).npc_id(self, i("item"))?
                ));
            }
            "petsNextStage" => {
                return Ok(json!(
                    pet_named(&s("pet").unwrap()).next_stage_item_id(i("item"))
                ));
            }
            "petsKitten" => return Ok(json!(pet_named(&s("pet").unwrap()).is_kitten(i("item")))),
            "petsGrownCat" => {
                return Ok(json!(pet_named(&s("pet").unwrap()).is_grown_cat(i("item"))));
            }
            "petsHasPet" => return Ok(json!(pets::has_pet(self, &"p".into())?)),
            "petsDump" => {
                return Ok(Json::Array(
                    PETS.iter()
                        .map(|p| {
                            json!([
                                p.name,
                                p.baby_item,
                                p.grown_item,
                                p.overgrown_item,
                                p.baby_npc,
                                p.grown_npc,
                                p.overgrown_npc,
                                bits(p.growth_rate),
                                p.summoning_level,
                                p.food
                            ])
                        })
                        .collect(),
                ));
            }
            "specialTarget" => {
                let sp = FamiliarSpecial::<World>::new(s("node"));
                return Ok(sp.target(self)?.map_or(Json::Null, |k| json!(k)));
            }
            "closeEvent" => {
                let (p, _) = self.close_events[i("index") as usize].clone();
                return Ok(json!(fm::burden::close_event(self, &p)?));
            }
            "pulse" => {
                let pulse = self.pulses[i("index") as usize].clone();
                return Ok(json!(pulse.pulse(self)?));
            }
            "listenerUpdate" => {
                let items: Vec<Option<Item>> = op["items"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(item_of)
                    .collect();
                let slots: Vec<i32> = op["slots"].as_array().unwrap().iter().map(int).collect();
                return fm::burden::listener_update(self, &"p".into(), &items, &slots).and(null);
            }
            _ => {}
        }
        let f = self.target(op);
        let beast = |f: &fm::Result<Familiar>| match f {
            Ok(f) if !f.is_burden_beast() => Err(Error::Cast),
            Ok(f) => Ok(f.clone()),
            Err(_) => Err(Error::Null),
        };
        let forager = |f: &fm::Result<Familiar>| match f {
            Ok(f) if !f.is_forager() => Err(Error::Cast),
            Ok(f) => Ok(f.clone()),
            Err(_) => Err(Error::Null),
        };
        let pet = |f: &fm::Result<Familiar>| match f {
            Ok(f) if !f.is_pet() => Err(Error::Cast),
            Ok(f) => Ok(f.clone()),
            Err(_) => Err(Error::Null),
        };
        match name {
            "isAllowed" => {
                let f = beast(&f)?;
                let owner = s("owner").unwrap_or("p".into());
                return Ok(json!(f.is_allowed(
                    self,
                    &owner,
                    &item_of(&op["item"]).unwrap()
                )?));
            }
            "transfer" => {
                return beast(&f)?
                    .transfer(self, item_of(&op["item"]), i("amount"), b("withdraw"))
                    .and(null);
            }
            "withdrawAll" => return beast(&f)?.withdraw_all(self).and(null),
            "openInterface" => return beast(&f)?.open_interface(self).and(null),
            "listenerRefresh" => {
                let c = beast(&f)?.container()?;
                return fm::burden::listener_refresh(self, &"p".into(), &c).and(null);
            }
            "poisonImmune" => return Ok(json!(beast(&f)?.poison_immune_override().unwrap())),
            "produceItem" => {
                return Ok(json!(
                    forager(&f)?.produce_item(self, &item_of(&op["item"]).unwrap())?
                ));
            }
            "produceRandom" => return Ok(json!(forager(&f)?.produce_random(self)?)),
            "setRandomPassive" => return forager(&f)?.set_random_passive(self).and(null),
            "passiveAction" => {
                forager(&f)?.handle_passive_action();
                return null;
            }
            "growNextStage" => return pet(&f)?.grow_next_stage(self).and(null),
            "setHasWarned" => return pet(&f)?.set_has_warned(i("value")).and(null),
            _ => {}
        }
        let f = f?;
        match name {
            "tick" => f.handle_tick_actions(self).and(null),
            "init" => f.init(self).and(null),
            "initAt" => f.init_at(self, loc(&op["location"]), b("call")).and(null),
            "isAttackable" => Ok(json!(f.is_attackable(
                self,
                &s("entity").unwrap(),
                style_of(&s("style").unwrap()),
                b("message")
            )?)),
            "continueAttack" => Ok(json!(f.continue_attack(
                self,
                &s("entity").unwrap(),
                style_of(&s("style").unwrap()),
                b("message")
            )?)),
            "regionInactivity" => f.on_region_inactivity(self).and(null),
            "swingHandler" => Ok(json!(f.swing_handler(self, b("swing"))?)),
            "setCombatHandler" => {
                f.set_combat_handler(s("handler"));
                null
            }
            "transform" => f.transform(self).and(null),
            "refreshTimer" => {
                f.refresh_timer();
                null
            }
            "specialMove" => {
                let node = s("node");
                let sp = if op.get("iface").is_some() {
                    FamiliarSpecial::with(node, i("iface"), i("component"), item_of(&op["item"]))
                } else {
                    FamiliarSpecial::new(node)
                };
                Ok(json!(f.execute_special_move(self, &sp)?))
            }
            "visualize" => f.visualize_special_move(self).and(null),
            "sendHit" => {
                let gfx = op.get("gfx").map(int);
                f.send_familiar_hit(self, &s("target").unwrap(), i("max"), gfx)
                    .and(null)
            }
            "projectile" => f.projectile(self, &s("target").unwrap(), i("id")).and(null),
            "canAttack" => {
                let t = s("target").unwrap();
                if op.get("message").is_some() {
                    Ok(json!(f.can_attack_with(self, &t, b("message"))?))
                } else {
                    Ok(json!(f.can_attack(self, &t)?))
                }
            }
            "canCombatSpecial" => {
                let message = if op.get("message").is_some() {
                    b("message")
                } else {
                    true
                };
                Ok(json!(f.can_combat_special(
                    self,
                    &s("target").unwrap(),
                    message
                )?))
            }
            "isOwnerAttackable" => Ok(json!(f.is_owner_attackable(self)?)),
            "startFollowing" => f.start_following(self).and(null),
            "finalizeDeath" => f.finalize_death(self).and(null),
            "isCombatFamiliar" => Ok(json!(f.is_combat_familiar())),
            "sendConfiguration" => f.send_configuration(self).and(null),
            "call" => Ok(json!(f.call(self)?)),
            "spawnLocation" => Ok(coord(f.spawn_location(self)?)),
            "dismissFamiliar" => f.dismiss(self).and(null),
            "updateSpecialPoints" => f.update_special_points(self, i("diff")).and(null),
            "isCharged" => Ok(json!(f.is_charged(self)?)),
            "charge" => {
                f.charge();
                null
            }
            "setCharged" => {
                f.set_charged(b("value"));
                null
            }
            "familiarBoost" => Ok(json!(f.get_boost(i("skill")))),
            "isPeaceful" => Ok(json!(f.is_peaceful_familiar()?)),
            "isBurdenBeast" => Ok(json!(f.is_burden_beast())),
            "setOwner" => {
                f.set_owner(s("owner"));
                null
            }
            "getAttackStyle" => Ok(json!(f.attack_style())),
            other => panic!("operation {other}"),
        }
    }

    // ------------------------------------------------------------ snapshot
    fn label(&mut self, d: &fm::Details) -> usize {
        let ptr = std::sync::Arc::as_ptr(d) as usize;
        if let Some(i) = self.labels.iter().position(|(p, _)| *p == ptr) {
            return i + 1;
        }
        self.labels.push((ptr, d.clone()));
        self.labels.len()
    }
    fn details(&mut self, d: &fm::Details) -> Json {
        json!([self.label(d), bits(d.hunger()), bits(d.growth())])
    }
    fn attributes(&self, k: &str) -> Json {
        Json::Object(
            self.nodes[k]
                .attributes
                .iter()
                .map(|(a, v)| (a.clone(), v.json()))
                .collect(),
        )
    }
    fn familiar_state(&mut self, f: &Familiar) -> Json {
        let npc = f.npc();
        let (
            owner,
            ticks,
            max,
            special,
            pouch,
            cost,
            has_pouch,
            handler,
            combat,
            charged,
            boosts,
            style,
            per,
            frac,
            first,
        ) = f.with(|s| {
            (
                s.owner.clone(),
                s.ticks,
                s.maximum_ticks,
                s.special_points,
                s.pouch_id,
                s.special_cost,
                s.pouch.is_some(),
                s.combat_handler.clone(),
                s.combat_familiar,
                s.charged,
                s.boosts.clone(),
                s.attack_style,
                s.points_per_tick,
                s.frac_drain,
                s.first_call,
            )
        });
        let n = self.nodes[&npc].clone();
        let mut out = Map::new();
        out.insert("owner".into(), owner.map_or(Json::Null, |o| json!(o)));
        out.insert("ticks".into(), json!(ticks));
        out.insert("max".into(), json!(max));
        out.insert("special".into(), json!(special));
        out.insert("pouch".into(), json!(pouch));
        out.insert("cost".into(), json!(cost));
        out.insert("hasPouch".into(), json!(has_pouch));
        out.insert("handler".into(), handler.map_or(Json::Null, |h| json!(h)));
        out.insert("combat".into(), json!(combat));
        out.insert("charged".into(), json!(charged));
        out.insert(
            "boosts".into(),
            Json::Array(
                boosts
                    .iter()
                    .map(|b| json!([b.skill, bits(b.bonus)]))
                    .collect(),
            ),
        );
        out.insert("style".into(), json!(style));
        out.insert("perTick".into(), bits(per));
        out.insert("frac".into(), bits(frac));
        out.insert("first".into(), json!(first));
        out.insert(
            "npc".into(),
            json!([
                n.id,
                n.original,
                coord(n.location),
                n.invisible,
                self.attributes(&npc)
            ]),
        );
        let kind = if f.is_pet() {
            "pet"
        } else if f.is_forager() {
            "forager"
        } else if f.is_burden_beast() {
            "beast"
        } else {
            "plain"
        };
        if f.is_burden_beast() {
            out.insert("container".into(), json!(f.container().unwrap()));
            if let Some(delay) = f.passive_delay() {
                let items = f.with(|s| match &s.kind {
                    fm::Kind::Burden(b) => b.forager.as_ref().unwrap().items.clone(),
                    _ => unreachable!(),
                });
                let items: Vec<Option<Item>> = items.into_iter().map(Some).collect();
                out.insert("forager".into(), json!([items_json(&items), delay]));
            }
        }
        if f.is_pet() {
            let d = f.pet_details().unwrap();
            let details = self.details(&d);
            let (item, rate, pet, warned) = f.with(|s| match &s.kind {
                fm::Kind::Pet(p) => (p.item_id, p.growth_rate, p.pet.name, p.has_warned),
                _ => unreachable!(),
            });
            out.insert(
                "pet".into(),
                json!([item, details, bits(rate), pet, warned]),
            );
        }
        out.insert("kind".into(), json!(kind));
        Json::Object(out)
    }
    fn snapshot(&mut self) -> Json {
        let m = self.manager_of(&"p".to_string());
        let current = m.familiar();
        if let Some(c) = &current {
            self.kept(c);
        }
        let mut pets = Vec::new();
        let entries: Vec<(i32, Vec<fm::Details>)> = m
            .pet_details()
            .with(|map| map.iter().map(|(k, v)| (k, v.clone())).collect());
        for (k, list) in entries {
            let list: Vec<Json> = list.iter().map(|d| self.details(d)).collect();
            pets.push(json!([k, list]));
        }
        let manager = json!({
            "familiar": current.map_or(Json::Null, |f| json!(f.npc())),
            "hasPouch": m.has_pouch(),
            "level": m.summoning_combat_level(),
            "pets": pets,
        });
        let mut registry = Vec::new();
        for id in self.registry.ids() {
            let f = self.registry.get(id).unwrap();
            self.kept(&f);
            registry.push(json!([id, f.npc()]));
        }
        let seen = self.seen.clone();
        let familiars: Vec<Json> = seen
            .iter()
            .map(|f| json!([f.npc(), self.familiar_state(f)]))
            .collect();
        json!({
            "manager": manager,
            "registry": registry,
            "familiars": familiars,
            "player": [coord(self.nodes["p"].location), self.attributes("p")],
        })
    }
}

fn pet_named(name: &str) -> &'static Pet {
    PETS.iter().find(|p| p.name == name).unwrap()
}

impl fm::Host for World {
    type Node = N;
    type Container = String;
    type Handler = String;
    type State = String;
    type Class = Class;

    fn location(&mut self, node: &N) -> fm::Result<Option<Loc>> {
        Ok(self.node(node).location)
    }
    fn set_location(&mut self, npc: &N, loc: Option<Loc>) -> fm::Result<()> {
        self.node(npc).location = loc;
        Ok(())
    }
    fn npc_id(&mut self, npc: &N) -> fm::Result<i32> {
        Ok(self.node(npc).id)
    }
    fn original_id(&mut self, npc: &N) -> fm::Result<i32> {
        Ok(self.node(npc).original)
    }
    fn name(&mut self, node: &N) -> fm::Result<Option<String>> {
        Ok(Some(self.node(node).name.clone()))
    }
    fn is_invisible(&mut self, node: &N) -> fm::Result<bool> {
        Ok(self.node(node).invisible)
    }
    fn kind(&mut self, node: &N) -> fm::Result<NodeKind> {
        Ok(self.node(node).kind)
    }
    fn inventory(&mut self, player: &N) -> fm::Result<String> {
        Ok(format!("{player}.inv"))
    }
    fn version(&mut self, _: &N) -> fm::Result<i32> {
        Ok(self.version)
    }
    fn manager(&mut self, player: &N) -> fm::Result<Manager<World>> {
        Ok(self.manager_of(player))
    }
    fn registry(&mut self) -> fm::Result<Registry<World>> {
        Ok(self.registry.clone())
    }
    fn pet_class(&mut self) -> Class {
        Class::Pet
    }

    fn new_npc(&mut self, id: i32) -> fm::Result<N> {
        self.rec("new_npc", vec![json!(id)])?;
        self.npcs += 1;
        let key = format!("npc:{}", self.npcs);
        let state = self.node_state(NodeKind::Npc, id);
        self.nodes.insert(key.clone(), state);
        Ok(key)
    }
    fn npc_definition_name(&mut self, id: i32) -> fm::Result<Option<String>> {
        self.rec("npc_definition_name", vec![json!(id)])?;
        Ok(Some(self.npc_name(id)))
    }
    fn set_invisible(&mut self, node: &N, invisible: bool) -> fm::Result<()> {
        self.rec("set_invisible", vec![json!(node), json!(invisible)])?;
        self.node(node).invisible = invisible;
        Ok(())
    }
    fn npc_init(&mut self, npc: &N) -> fm::Result<()> {
        let at = coord(self.node(npc).location);
        self.rec("npc_init", vec![json!(npc), at])
    }
    fn npc_clear(&mut self, npc: &N) -> fm::Result<()> {
        self.rec("npc_clear", vec![json!(npc)])
    }
    fn npc_transform(&mut self, npc: &N, id: i32) -> fm::Result<()> {
        self.rec("npc_transform", vec![json!(npc), json!(id)])?;
        self.node(npc).id = id;
        Ok(())
    }
    fn pulse_run(
        &mut self,
        node: &N,
        pulse: Pulse<World>,
        mover: &N,
        destination: Option<&N>,
    ) -> fm::Result<()> {
        self.rec(
            "pulse_run",
            vec![
                json!(node),
                json!("follow"),
                json!(mover),
                json!(destination),
                json!("STANDARD"),
            ],
        )?;
        self.pulses.push(pulse);
        Ok(())
    }
    fn pulse_clear(&mut self, node: &N) -> fm::Result<()> {
        self.rec("pulse_clear", vec![json!(node)])
    }
    fn has_pulse_running(&mut self, node: &N) -> fm::Result<bool> {
        self.rec("has_pulse_running", vec![json!(node)])?;
        Ok(self.bool("has_pulse_running", Some(node), false))
    }
    fn submit_pulse(&mut self, delay: i32, nodes: &[N], pulse: Pulse<World>) -> fm::Result<()> {
        self.rec("submit_pulse", vec![json!(delay), json!(nodes)])?;
        self.pulses.push(pulse);
        Ok(())
    }
    fn face(&mut self, node: &N, target: Option<&N>) -> fm::Result<()> {
        self.rec("face", vec![json!(node), json!(target)])
    }
    fn send_chat(&mut self, node: &N, text: &str) -> fm::Result<()> {
        self.rec("send_chat", vec![json!(node), json!(text)])
    }
    fn graphics(&mut self, node: &N, id: i32) -> fm::Result<()> {
        self.rec("graphics", vec![json!(node), json!(id)])
    }
    fn size(&mut self, node: &N) -> fm::Result<i32> {
        self.rec("size", vec![json!(node)])?;
        Ok(self.integer("size", Some(node), 1))
    }
    fn is_renderable(&mut self, node: &N) -> fm::Result<bool> {
        self.rec("is_renderable", vec![json!(node)])?;
        Ok(self.bool("is_renderable", Some(node), true))
    }
    fn walking_update(&mut self, node: &N) -> fm::Result<()> {
        self.rec("walking_update", vec![json!(node)])
    }
    fn masks_prepare(&mut self, node: &N) -> fm::Result<()> {
        self.rec("masks_prepare", vec![json!(node)])
    }
    fn set_teleport_location(&mut self, node: &N, loc: Loc) -> fm::Result<()> {
        self.rec("set_teleport_location", vec![json!(node), json!(loc)])
    }
    fn in_zone(&mut self, node: &N, name: &str) -> fm::Result<bool> {
        self.rec("in_zone", vec![json!(node), json!(name)])?;
        Ok(self.bool("in_zone", Some(node), false))
    }
    fn restricted_followers(&mut self, node: &N) -> fm::Result<bool> {
        self.rec("restricted_followers", vec![json!(node)])?;
        Ok(self.bool("restricted_followers", Some(node), false))
    }
    fn is_locked(&mut self, node: &N, key: &str) -> fm::Result<bool> {
        self.rec("is_locked", vec![json!(node), json!(key)])?;
        Ok(self.bool("is_locked", Some(node), false))
    }
    fn interaction_locked(&mut self, node: &N) -> fm::Result<bool> {
        self.rec("interaction_locked", vec![json!(node)])?;
        Ok(self.bool("interaction_locked", Some(node), false))
    }
    fn attribute_int(&mut self, node: &N, key: &str, default: i32) -> fm::Result<i32> {
        self.rec(
            "get_attribute",
            vec![json!(node), json!(key), json!(default)],
        )?;
        match self.node(node).attributes.get(key) {
            None | Some(Attr::Null) => Ok(default),
            Some(Attr::Int(i)) => Ok(*i),
            Some(_) => Err(Error::Cast),
        }
    }
    fn attribute_bool(&mut self, node: &N, key: &str, default: bool) -> fm::Result<bool> {
        self.rec(
            "get_attribute",
            vec![json!(node), json!(key), json!(default)],
        )?;
        match self.node(node).attributes.get(key) {
            None | Some(Attr::Null) => Ok(default),
            Some(Attr::Bool(b)) => Ok(*b),
            Some(_) => Err(Error::Cast),
        }
    }
    fn attribute_node(&mut self, node: &N, key: &str) -> fm::Result<Option<N>> {
        self.rec("get_attribute", vec![json!(node), json!(key)])?;
        match self.node(node).attributes.get(key) {
            None | Some(Attr::Null) => Ok(None),
            Some(Attr::Node(k)) => Ok(Some(k.clone())),
            Some(_) => Err(Error::Cast),
        }
    }
    fn set_attribute_int(&mut self, node: &N, key: &str, value: i32) -> fm::Result<()> {
        self.rec("set_attribute", vec![json!(node), json!(key), json!(value)])?;
        self.node(node)
            .attributes
            .insert(key.into(), Attr::Int(value));
        Ok(())
    }
    fn increment_attribute(&mut self, node: &N, key: &str) -> fm::Result<()> {
        self.rec("increment_attribute", vec![json!(node), json!(key)])?;
        let v = match self.node(node).attributes.get(key) {
            None | Some(Attr::Null) => 0,
            Some(Attr::Int(i)) => *i,
            Some(_) => return Err(Error::Cast),
        };
        self.node(node)
            .attributes
            .insert(key.into(), Attr::Int(v + 1));
        Ok(())
    }
    fn is_attackable(
        &mut self,
        node: &N,
        attacker: &N,
        style: CombatStyle,
        message: bool,
    ) -> fm::Result<bool> {
        if let Some(f) = self.familiar_of(node) {
            return f.is_attackable(self, attacker, style, message);
        }
        if self.node(node).kind == NodeKind::Npc {
            return self.npc_is_attackable(node, attacker, style, message);
        }
        self.rec(
            "is_attackable",
            vec![
                json!(node),
                json!(attacker),
                json!(style_name(style)),
                json!(message),
            ],
        )?;
        Ok(self.bool("is_attackable", Some(node), true))
    }
    fn npc_is_attackable(
        &mut self,
        npc: &N,
        attacker: &N,
        style: CombatStyle,
        message: bool,
    ) -> fm::Result<bool> {
        self.rec(
            "npc_is_attackable",
            vec![
                json!(npc),
                json!(attacker),
                json!(style_name(style)),
                json!(message),
            ],
        )?;
        Ok(self.bool("npc_is_attackable", Some(npc), true))
    }
    fn npc_swing_handler(&mut self, npc: &N, swing: bool) -> fm::Result<String> {
        self.rec("npc_swing_handler", vec![json!(npc), json!(swing)])?;
        Ok(format!("handler:{npc}"))
    }
    fn combat_attacking(&mut self, node: &N) -> fm::Result<bool> {
        self.rec("combat_attacking", vec![json!(node)])?;
        Ok(self.bool("combat_attacking", Some(node), false))
    }
    fn combat_victim(&mut self, node: &N) -> fm::Result<Option<N>> {
        self.rec("combat_victim", vec![json!(node)])?;
        let v = self.answer("combat_victim", Some(node), Json::Null);
        Ok(v.as_str()
            .filter(|k| self.nodes.contains_key(*k))
            .map(str::to_owned))
    }
    fn combat_attack(&mut self, node: &N, victim: &N) -> fm::Result<()> {
        self.rec("combat_attack", vec![json!(node), json!(victim)])
    }
    fn combat_next_attack(&mut self, node: &N, ticks: i32) -> fm::Result<()> {
        self.rec("combat_next_attack", vec![json!(node), json!(ticks)])
    }
    fn combat_style(&mut self, node: &N) -> fm::Result<CombatStyle> {
        self.rec("combat_style", vec![json!(node)])?;
        let v = self.answer("combat_style", Some(node), json!("MELEE"));
        Ok(style_of(v.as_str().unwrap()))
    }
    fn in_combat(&mut self, node: &N) -> fm::Result<bool> {
        self.rec("in_combat", vec![json!(node)])?;
        Ok(self.bool("in_combat", Some(node), false))
    }
    fn multi_zone(&mut self, node: &N) -> fm::Result<bool> {
        self.rec("multi_zone", vec![json!(node)])?;
        Ok(self.bool("multi_zone", Some(node), false))
    }
    fn skull_wilderness(&mut self, player: &N) -> fm::Result<bool> {
        self.rec("skull_wilderness", vec![json!(player)])?;
        Ok(self.bool("skull_wilderness", Some(player), false))
    }
    fn npc_task(&mut self, npc: &N) -> fm::Result<Option<i32>> {
        self.rec("npc_task", vec![json!(npc)])?;
        Ok(self
            .answer("npc_task", Some(npc), Json::Null)
            .as_i64()
            .map(|v| v as i32))
    }
    fn is_active(&mut self, player: &N) -> fm::Result<bool> {
        self.rec("is_active", vec![json!(player)])?;
        Ok(self.bool("is_active", Some(player), true))
    }
    fn spawn_location(&mut self, owner: &N, node: &N) -> fm::Result<Option<Loc>> {
        self.rec("spawn_location", vec![json!(owner), json!(node)])?;
        let fallback = coord(self.node(owner).location);
        Ok(loc(&self.answer("spawn_location", None, fallback)))
    }
    fn lifepoints(&mut self, node: &N) -> fm::Result<i32> {
        self.rec("lifepoints", vec![json!(node)])?;
        Ok(10)
    }
    fn skill_level(&mut self, player: &N, skill: i32) -> fm::Result<i32> {
        self.rec("skill_level", vec![json!(player), json!(skill)])?;
        Ok(self.integer("skill_level", Some(player), 99))
    }
    fn static_level(&mut self, player: &N, skill: i32) -> fm::Result<i32> {
        self.rec("static_level", vec![json!(player), json!(skill)])?;
        Ok(self.integer("static_level", Some(player), 99))
    }
    fn update_level(
        &mut self,
        player: &N,
        skill: i32,
        amount: i32,
        maximum: i32,
    ) -> fm::Result<()> {
        self.rec(
            "update_level",
            vec![json!(player), json!(skill), json!(amount), json!(maximum)],
        )
    }
    fn add_experience(
        &mut self,
        player: &N,
        skill: i32,
        experience: f64,
        player_mod: Option<bool>,
    ) -> fm::Result<()> {
        self.rec(
            "add_experience",
            vec![
                json!(player),
                json!(skill),
                bits(experience),
                json!(player_mod),
            ],
        )
    }
    fn packet_message(&mut self, player: &N, text: &str) -> fm::Result<()> {
        self.rec("packet_message", vec![json!(player), json!(text)])
    }
    fn player_message(&mut self, player: &N, text: &str) -> fm::Result<()> {
        self.rec("player_message", vec![json!(player), json!(text)])
    }
    fn api_message(&mut self, player: &N, text: &str) -> fm::Result<()> {
        self.rec("api_message", vec![json!(player), json!(text)])
    }
    fn send_dialogue(&mut self, player: &N, text: &str) -> fm::Result<()> {
        self.rec("send_dialogue", vec![json!(player), json!(text)])
    }
    fn set_varp(&mut self, player: &N, varp: i32, value: i32) -> fm::Result<()> {
        self.rec("set_varp", vec![json!(player), json!(varp), json!(value)])
    }
    fn set_varbit(&mut self, player: &N, varbit: i32, value: i32) -> fm::Result<()> {
        self.rec(
            "set_varbit",
            vec![json!(player), json!(varbit), json!(value)],
        )
    }
    fn play_audio(&mut self, player: &N, sound: i32) -> fm::Result<()> {
        self.rec("play_audio", vec![json!(player), json!(sound)])
    }
    fn visualize(&mut self, player: &N, anim: i32, gfx: i32) -> fm::Result<()> {
        self.rec("visualize", vec![json!(player), json!(anim), json!(gfx)])
    }
    fn animate(&mut self, player: &N, anim: i32) -> fm::Result<()> {
        self.rec("animate", vec![json!(player), json!(anim)])
    }
    fn appearance_sync(&mut self, player: &N) -> fm::Result<()> {
        self.rec("appearance_sync", vec![json!(player)])
    }
    fn wolf_whistle_complete(&mut self, player: &N) -> fm::Result<bool> {
        self.rec("wolf_whistle_complete", vec![json!(player)])?;
        Ok(self.bool("wolf_whistle_complete", Some(player), false))
    }
    fn pet_mastery(&mut self, player: &N) -> fm::Result<bool> {
        self.rec("pet_mastery", vec![json!(player)])?;
        Ok(self.bool("pet_mastery", Some(player), false))
    }
    fn open_tab(&mut self, player: &N, component: i32) -> fm::Result<()> {
        self.rec("open_tab", vec![json!(player), json!(component)])
    }
    fn set_viewed_tab(&mut self, player: &N, tab: i32) -> fm::Result<()> {
        self.rec("set_viewed_tab", vec![json!(player), json!(tab)])
    }
    fn open_info_bars(&mut self, player: &N) -> fm::Result<()> {
        self.rec("open_info_bars", vec![json!(player)])
    }
    fn remove_tabs(&mut self, player: &N, tabs: &[i32]) -> fm::Result<()> {
        self.rec("remove_tabs", vec![json!(player), json!(tabs)])
    }
    fn has_main_component(&mut self, player: &N, id: i32) -> fm::Result<bool> {
        self.rec("has_main_component", vec![json!(player), json!(id)])?;
        Ok(self.bool("has_main_component", Some(player), false))
    }
    fn close(&mut self, player: &N) -> fm::Result<()> {
        self.rec("close", vec![json!(player)])
    }
    fn open(&mut self, player: &N, id: i32) -> fm::Result<bool> {
        self.rec("open", vec![json!(player), json!(id)])?;
        Ok(self.bool("open", Some(player), true))
    }
    fn set_close_event(&mut self, player: &N, id: i32) -> fm::Result<()> {
        self.rec("set_close_event", vec![json!(player), json!(id)])?;
        self.close_events.push((player.clone(), id));
        Ok(())
    }
    fn open_single_tab(&mut self, player: &N, id: i32) -> fm::Result<()> {
        self.rec("open_single_tab", vec![json!(player), json!(id)])
    }
    fn close_single_tab(&mut self, player: &N) -> fm::Result<()> {
        self.rec("close_single_tab", vec![json!(player)])
    }
    fn generate_items(
        &mut self,
        player: &N,
        items: &[Option<Item>],
        options: &[&str],
        interface: i32,
        child: i32,
        x: i32,
        y: i32,
        key: i32,
    ) -> fm::Result<()> {
        self.rec(
            "generate_items",
            vec![
                json!(player),
                items_json(items),
                json!(options),
                json!(interface),
                json!(child),
                json!(x),
                json!(y),
                json!(key),
            ],
        )
    }
    fn container_packet(
        &mut self,
        player: &N,
        items: &[Option<Item>],
        capacity: Option<i32>,
        slots: Option<&[i32]>,
    ) -> fm::Result<()> {
        self.rec(
            "container_packet",
            vec![
                json!(player),
                items_json(items),
                json!(capacity),
                json!(slots),
            ],
        )
    }
    fn battle_state(&mut self, attacker: &N, victim: &N) -> fm::Result<String> {
        self.rec("battle_state", vec![json!(attacker), json!(victim)])?;
        self.states += 1;
        Ok(format!("state:{}", self.states))
    }
    fn accurate_impact(
        &mut self,
        style: CombatStyle,
        attacker: &N,
        victim: &N,
    ) -> fm::Result<bool> {
        self.rec(
            "accurate_impact",
            vec![json!(style_name(style)), json!(attacker), json!(victim)],
        )?;
        Ok(self.bool("accurate_impact", None, true))
    }
    fn set_estimated_hit(&mut self, state: &mut String, hit: i32) -> fm::Result<()> {
        self.rec("set_estimated_hit", vec![json!(state), json!(hit)])
    }
    fn neutralize_hits(&mut self, state: &mut String) -> fm::Result<()> {
        self.rec("neutralize_hits", vec![json!(state)])
    }
    fn set_poison_damage(&mut self, state: &mut String, damage: i32) -> fm::Result<()> {
        self.rec("set_poison_damage", vec![json!(state), json!(damage)])
    }
    fn state_style(&mut self, state: &mut String) -> fm::Result<Option<CombatStyle>> {
        self.rec("state_style", vec![json!(state)])?;
        let v = self.answer("state_style", Some(state), json!("MELEE"));
        Ok(v.as_str().map(style_of))
    }
    fn handle_impact(
        &mut self,
        target: &N,
        source: &N,
        hit: i32,
        style: CombatStyle,
        state: &mut String,
    ) -> fm::Result<()> {
        self.rec(
            "handle_impact",
            vec![
                json!(target),
                json!(source),
                json!(hit),
                json!(style_name(style)),
                json!(state),
            ],
        )
    }
    fn projectile(
        &mut self,
        source: &N,
        target: &N,
        id: i32,
        a: i32,
        b: i32,
        c: i32,
        d: i32,
    ) -> fm::Result<()> {
        self.rec(
            "projectile",
            vec![
                json!(source),
                json!(target),
                json!(id),
                json!(a),
                json!(b),
                json!(c),
                json!(d),
            ],
        )
    }
    fn ticks(&mut self) -> fm::Result<i32> {
        self.rec("ticks", vec![])?;
        Ok(self.integer("ticks", None, 100))
    }
    fn random(&mut self, max: i32) -> fm::Result<i32> {
        self.rec("random", vec![json!(max)])?;
        Ok(self.integer("random", None, 0))
    }
    fn random_range(&mut self, a: i32, b: i32) -> fm::Result<i32> {
        self.rec("random_range", vec![json!(a), json!(b)])?;
        Ok(self.integer("random_range", None, a))
    }
    fn randomize(&mut self, value: i32) -> fm::Result<i32> {
        self.rec("randomize", vec![json!(value)])?;
        Ok(self.integer("randomize", None, value))
    }
    fn new_container(&mut self, size: i32) -> fm::Result<String> {
        self.rec("new_container", vec![json!(size)])?;
        self.containers += 1;
        Ok(format!("container:{}", self.containers))
    }
    fn register_burden_listener(&mut self, c: &String, owner: Option<&N>) -> fm::Result<()> {
        self.rec("register_burden_listener", vec![json!(c), json!(owner)])
    }
    fn get_amount(&mut self, c: &String, item: &Item) -> fm::Result<i32> {
        self.rec("get_amount", vec![json!(c), item_json(Some(item))])?;
        Ok(self.integer("get_amount", Some(c), 0))
    }
    fn maximum_add(&mut self, c: &String, item: &Item) -> fm::Result<i32> {
        self.rec("maximum_add", vec![json!(c), item_json(Some(item))])?;
        Ok(self.integer("maximum_add", Some(c), 0))
    }
    fn replace(
        &mut self,
        c: &String,
        item: Option<Item>,
        slot: i32,
        fire: Option<bool>,
    ) -> fm::Result<Option<Item>> {
        self.rec(
            "replace",
            vec![json!(c), item_json(item.as_ref()), json!(slot), json!(fire)],
        )?;
        Ok(item_of(&self.answer("replace", Some(c), Json::Null)))
    }
    fn add(&mut self, c: &String, item: &Item, fire: Option<bool>) -> fm::Result<bool> {
        self.rec("add", vec![json!(c), item_json(Some(item)), json!(fire)])?;
        Ok(self.bool("add", Some(c), true))
    }
    fn remove(&mut self, c: &String, item: &Item, fire: Option<bool>) -> fm::Result<bool> {
        self.rec("remove", vec![json!(c), item_json(Some(item)), json!(fire)])?;
        Ok(self.bool("remove", Some(c), true))
    }
    fn to_array(&mut self, c: &String) -> fm::Result<Vec<Option<Item>>> {
        self.rec("to_array", vec![json!(c)])?;
        let v = self.answer("to_array", Some(c), json!([]));
        Ok(v.as_array().unwrap().iter().map(item_of).collect())
    }
    fn get(&mut self, c: &String, slot: i32) -> fm::Result<Option<Item>> {
        self.rec("get", vec![json!(c), json!(slot)])?;
        Ok(item_of(&self.answer("get", Some(c), Json::Null)))
    }
    fn capacity(&mut self, c: &String) -> fm::Result<i32> {
        self.rec("capacity", vec![json!(c)])?;
        Ok(self.integer("capacity", Some(c), 0))
    }
    fn item_count(&mut self, c: &String) -> fm::Result<i32> {
        self.rec("item_count", vec![json!(c)])?;
        Ok(self.integer("item_count", Some(c), 0))
    }
    fn is_empty(&mut self, c: &String) -> fm::Result<bool> {
        self.rec("is_empty", vec![json!(c)])?;
        Ok(self.bool("is_empty", Some(c), true))
    }
    fn clear(&mut self, c: &String) -> fm::Result<()> {
        self.rec("clear", vec![json!(c)])
    }
    fn shift(&mut self, c: &String) -> fm::Result<()> {
        self.rec("shift", vec![json!(c)])
    }
    fn refresh(&mut self, c: &String) -> fm::Result<()> {
        self.rec("refresh", vec![json!(c)])
    }
    fn update(&mut self, c: &String) -> fm::Result<()> {
        self.rec("update", vec![json!(c)])
    }
    fn free_slots(&mut self, c: &String) -> fm::Result<i32> {
        self.rec("free_slots", vec![json!(c)])?;
        Ok(self.integer("free_slots", Some(c), 28))
    }
    fn has_space_for(&mut self, c: &String, item: &Item) -> fm::Result<bool> {
        self.rec("has_space_for", vec![json!(c), item_json(Some(item))])?;
        Ok(self.bool("has_space_for", Some(c), true))
    }
    fn contains(&mut self, c: &String, id: i32, amount: i32) -> fm::Result<bool> {
        self.rec("contains", vec![json!(c), json!(id), json!(amount)])?;
        Ok(self.bool("contains", Some(c), true))
    }
    fn contains_one(&mut self, c: &String, id: i32) -> fm::Result<bool> {
        self.rec("contains_one", vec![json!(c), json!(id)])?;
        Ok(self.bool(&format!("contains_one:{id}"), Some(c), false))
    }
    fn parse_container(&mut self, c: &String, data: &java_json::Json) -> fm::Result<()> {
        self.rec("parse_container", vec![json!(c), json!(data.text())])
    }
    fn saved_container(
        &mut self,
        _: &String,
    ) -> fm::Result<rs09_server::inventory::container::Container> {
        Err(Error::Binding(
            "saved_container is not part of the corpus".into(),
        ))
    }
    fn item_stackable(&mut self, id: i32) -> fm::Result<bool> {
        self.rec("item_stackable", vec![json!(id)])?;
        Ok(self.item_spec(id)["stackable"].as_bool().unwrap_or(false))
    }
    fn item_tradeable(&mut self, id: i32) -> fm::Result<bool> {
        self.rec("item_tradeable", vec![json!(id)])?;
        Ok(self.item_spec(id)["tradeable"].as_bool().unwrap_or(true))
    }
    fn item_bankable(&mut self, id: i32) -> fm::Result<bool> {
        self.rec("item_bankable", vec![json!(id)])?;
        Ok(self.item_spec(id)["bankable"].as_bool().unwrap_or(true))
    }
    fn item_value(&mut self, item: &Item) -> fm::Result<i32> {
        self.rec("item_value", vec![item_json(Some(item))])?;
        let fallback = self.item_spec(item.id)["value"].as_i64().unwrap_or(1) as i32;
        Ok(self.integer(&format!("item_value:{}", item.id), None, fallback))
    }
    fn item_name(&mut self, item: &Item) -> fm::Result<String> {
        self.rec("item_name", vec![item_json(Some(item))])?;
        Ok(self.item_spec(item.id)["name"]
            .as_str()
            .unwrap_or("null")
            .into())
    }
    fn item_definition_name(&mut self, id: i32) -> fm::Result<String> {
        self.rec("item_definition_name", vec![json!(id)])?;
        Ok(self.item_spec(id)["name"].as_str().unwrap_or("null").into())
    }
    fn ground_item(
        &mut self,
        item: &Item,
        location: Option<Loc>,
        decay: i32,
        owner: &N,
    ) -> fm::Result<()> {
        self.rec(
            "ground_item",
            vec![
                item_json(Some(item)),
                coord(location),
                json!(decay),
                json!(owner),
            ],
        )
    }
    fn pouch(&mut self, id: i32) -> fm::Result<Option<Pouch>> {
        self.rec("pouch", vec![json!(id)])?;
        let p = &self.input["pouches"][id.to_string()];
        if p.is_null() {
            return Ok(None);
        }
        Ok(Some(Pouch {
            level_required: int(&p["level"]),
            summon_cost: int(&p["cost"]),
            npc_id: int(&p["npc"]),
            summon_experience: p["exp"].as_f64().unwrap(),
            peaceful: p["peaceful"].as_bool().unwrap_or(false),
            abyssal: p["abyssal"].as_bool().unwrap_or(false),
        }))
    }
    fn scroll(&mut self, pouch_id: i32) -> fm::Result<Option<Scroll>> {
        self.rec("scroll", vec![json!(pouch_id)])?;
        let s = &self.input["scrolls"][pouch_id.to_string()];
        if s.is_null() {
            return Ok(None);
        }
        Ok(Some(Scroll {
            item_id: int(&s["item"]),
            experience: s["exp"].as_f64().unwrap(),
        }))
    }
    fn construct(&mut self, prototype: &Familiar, owner: &N, id: i32) -> fm::Result<Familiar> {
        self.rec(
            "construct",
            vec![json!(prototype.npc()), json!(owner), json!(id)],
        )?;
        let f = self.create(id, Some(owner.clone()))?;
        self.kept(&f);
        Ok(f)
    }
    fn special_move(
        &mut self,
        familiar: &Familiar,
        special: &FamiliarSpecial<World>,
    ) -> fm::Result<bool> {
        let k = familiar.npc();
        self.rec(
            "special_move",
            vec![
                json!(k),
                json!(special.node),
                json!(special.interface_id),
                json!(special.component),
                item_json(special.item.as_ref()),
            ],
        )?;
        Ok(self.bool("special_move", Some(&k), true))
    }
    fn ids(&mut self, familiar: &Familiar) -> fm::Result<Vec<i32>> {
        let k = familiar.npc();
        self.rec("ids", vec![json!(k)])?;
        let original = self.node(&k).original;
        Ok(self.class_spec(original)["ids"]
            .as_array()
            .unwrap()
            .iter()
            .map(int)
            .collect())
    }
    fn configure(&mut self, familiar: &Familiar) -> fm::Result<()> {
        self.rec("configure", vec![json!(familiar.npc())])
    }
    fn text(&mut self, familiar: &Familiar) -> fm::Result<String> {
        let k = familiar.npc();
        self.rec("text", vec![json!(k)])?;
        Ok(self
            .answer("text", Some(&k), json!(""))
            .as_str()
            .unwrap()
            .into())
    }
    fn familiar_tick(&mut self, familiar: &Familiar) -> fm::Result<()> {
        if familiar.class() == Class::Pet {
            return Ok(());
        }
        self.rec("familiar_tick", vec![json!(familiar.npc())])
    }
    fn adjust_player_battle(&mut self, familiar: &Familiar, state: &mut String) -> fm::Result<()> {
        if familiar.class() == Class::Pet {
            return Ok(());
        }
        self.rec(
            "adjust_player_battle",
            vec![json!(familiar.npc()), json!(state)],
        )
    }
    fn forager_random(&mut self, familiar: &Familiar) -> fm::Result<i32> {
        let k = familiar.npc();
        self.rec("forager_random", vec![json!(k)])?;
        Ok(self.integer("forager_random", Some(&k), 11))
    }
    fn log(&mut self, origin: Origin<'_, World>, level: LogLevel, message: &str) -> fm::Result<()> {
        let class = match origin {
            Origin::Pets => "content.global.skill.summoning.pet.Pets",
            Origin::Familiar(f) => f.class().name(),
        };
        let level = match level {
            LogLevel::Err => "ERR",
        };
        self.rec("log", vec![json!(class), json!(level), json!(message)])
    }
}

fn run_case(case: &Json) -> Vec<Json> {
    let mut w = World::new(case);
    let mut out = Vec::new();
    for op in case["ops"].as_array().unwrap() {
        w.fail_at = op.get("failAt").and_then(Json::as_i64).unwrap_or(0);
        w.op_calls = 0;
        w.events.clear();
        let (result, error) = match w.op(op) {
            Ok(v) => (v, Json::Null),
            Err(e) => (Json::Null, json!(error_name(&e))),
        };
        let events = std::mem::take(&mut w.events);
        let state = w.snapshot();
        out.push(json!({ "events": events, "result": result, "error": error, "state": state }));
    }
    out
}

#[test]
fn provenance_matches_the_fixture() {
    let provenance: Json = serde_json::from_str(PROVENANCE).unwrap();
    let digest = Sha256::digest(CORPUS.as_bytes());
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(provenance["fixtureSha256"], json!(hex));
    assert_eq!(provenance["caseCount"], json!(CORPUS.lines().count()));
}

#[test]
fn familiar_framework_matches_the_original() {
    let mut cases = 0;
    let mut operations = 0;
    let mut failures = Vec::new();
    for (number, line) in CORPUS.lines().enumerate() {
        let case: Json = serde_json::from_str(line).unwrap();
        let expected = case["expected"].as_array().unwrap().clone();
        let mut input = case.clone();
        input.as_object_mut().unwrap().remove("expected");
        let actual = run_case(&input);
        assert_eq!(actual.len(), expected.len(), "case {number}");
        cases += 1;
        for (index, (a, e)) in actual.iter().zip(&expected).enumerate() {
            operations += 1;
            if a != e {
                if failures.len() < 5 {
                    failures.push(format!(
                        "case {number} {} op {index} {}:\n  expected {}\n  actual   {}",
                        case["name"], input["ops"][index], e, a
                    ));
                } else {
                    failures.push(String::new());
                }
                break;
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} mismatching cases; first:\n{}",
        failures.len(),
        failures.join("\n")
    );
    assert_eq!(cases, 565);
    assert!(operations > 2000);
}

/// The persisted store and the manager share one map: what parse and
/// summon put there is what the save writes.
#[test]
fn the_manager_works_on_the_persisted_pet_store() {
    let mut persisted = rs09_server::player::persisted::familiars::Manager::default();
    let case: Json = serde_json::from_str(CORPUS.lines().next().unwrap()).unwrap();
    let mut world = World::new(&case);
    let manager = Manager::<World>::new("p".into(), persisted.pets.clone());
    manager.add_details(1555, PetDetails::new(12.5));
    manager.add_details(12522, PetDetails::new(100.0));
    assert!(manager.pet_details().same(&persisted.pets));
    let saved = persisted.saved();
    assert_eq!(saved.pets.len(), 2);
    assert_eq!(
        saved.pets.iter().find(|(id, _)| *id == 1555).unwrap().1[0].growth,
        12.5
    );
    // A pet's details are the stored object: its hunger reaches the save.
    let data = java_json::parse(
        r#"{"petDetails":{"1555":[{"hunger":"1.0","growth":"2.0"}]},"currentPet":"1555"}"#,
    )
    .unwrap();
    manager.parse(&mut world, &data).unwrap();
    manager
        .familiar()
        .unwrap()
        .pet_details()
        .unwrap()
        .update_hunger(5.0);
    let saved = persisted.saved();
    let cat = &saved.pets.iter().find(|(id, _)| *id == 1555).unwrap().1;
    assert_eq!((cat.len(), cat[0].hunger), (1, 6.0));
    persisted.active = manager.saved_active(&mut world).unwrap();
    assert!(matches!(
        persisted.active,
        rs09_server::player::save::ActiveFamiliar::Pet { item_id: 1555 }
    ));
}
