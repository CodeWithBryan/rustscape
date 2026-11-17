//! Native replay of the binding corpus.
//!
//! The unmodified listeners and the fishing pulse ran over the original
//! Skills, Container and Item; here the native ports run over the native
//! `player::skills` and `inventory::container` through `binding::Host`.
//! Levels, experience and every slot are native state and are compared after
//! each operation. Calls to systems that are not bound (and the callbacks the
//! native skills and containers make) are the tape, as in the first corpus.
use super::{World, bits, failure, seed};
use rs09_game_core::random::JavaRandom;
use rs09_server::{
    definitions::item::{ItemDefinition, Registry, Shared},
    inventory::{
        self,
        container::{self, Container, ContainerType, Event, Factory, SortType},
        container_listener::{Handle as Listener, NativeListener},
        item::{Handle, Item},
    },
    player::skills::{self, Effect, EntityKind, Skills},
    world::gathering::{
        Settings,
        binding::Host,
        fishing::{self, FishingOption},
        mining,
        service::{Arg, Context, Result, Service, Task},
        woodcutting,
    },
};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
};

thread_local! {
    /// How often each service was requested of the host by the replayed bodies (diagnostic only).
    static REQUESTED: RefCell<BTreeMap<String, usize>> = const { RefCell::new(BTreeMap::new()) };
}
pub fn requested() -> BTreeMap<String, usize> {
    REQUESTED.with(|r| r.borrow().clone())
}
/// Counts what the ported bodies ask for, then hands the request to the host unchanged.
struct Counted<H>(H);
impl<H: Context> Context for Counted<H> {
    fn call(&mut self, service: Service, args: &[Arg]) -> Result<Arg> {
        let mut key = service.name().to_owned();
        if matches!(service, Service::ApiAddItem | Service::ApiRemoveItem) {
            if let Some(Arg::Text(container)) = args.last() {
                key = format!("{key}:{container}");
            }
        }
        REQUESTED.with(|r| *r.borrow_mut().entry(key).or_insert(0) += 1);
        self.0.call(service, args)
    }
    fn defer(&mut self, service: Service, args: &[Arg], task: Task) -> Result<()> {
        self.0.defer(service, args, task)
    }
}

/// Test-only label on a real native listener object: 0 inventory, 1 equipment.
struct Recording(usize);
impl NativeListener for Recording {}

/// One view of the recording world per service family of the host.
#[derive(Clone, Copy)]
struct Services<'a> {
    world: &'a RefCell<World>,
    combat: &'a Cell<i32>,
    summoning: &'a Cell<i32>,
    /// Set while the player is constructed: nothing is on a tape yet.
    setup: Option<i32>,
}
impl Services<'_> {
    fn ask(&self, name: &str, args: Value) -> skills::Result<Arg> {
        if self.setup.is_some() {
            return Ok(Arg::Null);
        }
        self.world
            .borrow_mut()
            .recorded(name, args)
            .map_err(|_| skills::Error::Binding)
    }
    fn flag(&self, name: &str, args: Value) -> skills::Result<bool> {
        match self.ask(name, args)? {
            Arg::Bool(v) => Ok(v),
            _ => Err(skills::Error::Binding),
        }
    }
    /// Clock reads are infallible in the native skills; the corpus injects no failure on them.
    fn clock(&self, name: &str) -> i32 {
        match (self.setup, self.ask(name, json!([]))) {
            (Some(ticks), _) => ticks,
            (None, Ok(Arg::Int(v))) => v as i32,
            _ => panic!("{name} is not on the tape"),
        }
    }
}
impl Context for Services<'_> {
    fn call(&mut self, service: Service, args: &[Arg]) -> Result<Arg> {
        self.world.borrow_mut().call(service, args)
    }
    fn defer(&mut self, service: Service, args: &[Arg], task: Task) -> Result<()> {
        self.world.borrow_mut().defer(service, args, task)
    }
}
impl skills::Context for Services<'_> {
    fn kind(&self) -> EntityKind {
        EntityKind::Player
    }
    fn version(&self) -> i32 {
        0
    }
    fn tick(&mut self) -> i32 {
        self.clock("gameWorld.ticks")
    }
    fn world_tick(&mut self) -> i32 {
        self.clock("api.getWorldTicks")
    }
    fn combat_level(&self) -> i32 {
        self.combat.get()
    }
    fn summoning_combat_level(&self) -> i32 {
        self.summoning.get()
    }
    fn npc_combat_level(&mut self) -> skills::Result<i32> {
        Err(skills::Error::Binding)
    }
    fn damage_spong_active(&mut self) -> skills::Result<bool> {
        self.flag("skillcapePerks.isActive", json!(["DAMAGE_SPONG"]))
    }
    // No assist session exists in the corpus: getExtension answers null without a call.
    fn assisted_level(&mut self, _: i32) -> skills::Result<Option<i32>> {
        Ok(None)
    }
    fn translate_experience(
        &mut self,
        _: &mut Skills,
        _: i32,
        _: f64,
        _: f64,
    ) -> skills::Result<bool> {
        Ok(false)
    }
    // The corpus's glove manager holds no charges.
    fn brawling_present(&mut self) -> skills::Result<bool> {
        self.ask("brawling.getInstance", json!([])).map(|_| false)
    }
    fn brawling_for_skill(&mut self, _: i32) -> skills::Result<Option<i32>> {
        Err(skills::Error::Binding)
    }
    fn equipment_contains(&mut self, _: i32) -> skills::Result<bool> {
        Err(skills::Error::Binding)
    }
    fn brawling_bonus(&mut self) -> skills::Result<f64> {
        Err(skills::Error::Binding)
    }
    fn tutorial_complete(&mut self) -> skills::Result<bool> {
        self.flag("player.getAttribute", json!(["tutorial:complete", false]))
    }
    fn artificial(&mut self) -> skills::Result<bool> {
        self.flag("player.isArtificial", json!([]))
    }
    fn username(&mut self) -> skills::Result<String> {
        match self.ask("player.getUsername", json!([]))? {
            Arg::Text(v) => Ok(v),
            _ => Err(skills::Error::Binding),
        }
    }
    fn permadeath_present(&self) -> bool {
        false
    }
    fn effect(&mut self, state: &mut Skills, effect: Effect) -> skills::Result<()> {
        let (name, args) = match effect {
            Effect::Combat(v) => {
                self.ask("properties.setCombatLevel", json!([v]))?;
                self.combat.set(v);
                return Ok(());
            }
            Effect::SummoningCombat(v) => {
                self.ask("familiar.setSummoningCombatLevel", json!([v]))?;
                self.summoning.set(v);
                return Ok(());
            }
            Effect::Packet(i) => (
                "packet.skillLevel",
                json!([
                    i,
                    state.static_levels[i],
                    state.dynamic_levels[i],
                    bits(state.experience[i])
                ]),
            ),
            Effect::DynamicLevel {
                skill,
                previous,
                level,
            } => (
                "player.dispatch",
                json!([format!(
                    "DynamicSkillLevelChangeEvent({skill},{previous},{level})"
                )]),
            ),
            Effect::XpEvent { skill, amount } => (
                "player.dispatch",
                json!([format!("XPGainEvent({skill},{})", bits(amount))]),
            ),
            Effect::XpPlugins { skill, amount } => {
                ("xpGainPlugins.run", json!([skill, bits(amount)]))
            }
            Effect::Appearance => ("player.updateAppearance", json!([])),
            Effect::LevelUp { skill, amount } => ("levelUp.levelup", json!([skill, amount])),
            Effect::FlashingIcons(skill) => ("levelUp.sendFlashingIcons", json!([skill])),
            Effect::News(text) => ("repository.sendNews", json!([text])),
            Effect::XpLog(diffs) => (
                "playerMonitor.logXpGains",
                json!([diffs
                    .iter()
                    .map(|(i, d)| json!([i, bits(*d)]))
                    .collect::<Vec<_>>()]),
            ),
            Effect::ConsumeGloves { .. } => return Err(skills::Error::Binding),
        };
        self.ask(name, args).map(|_| ())
    }
}
impl container::Context for Services<'_> {
    fn update(
        &mut self,
        listener: Listener,
        _: &mut Container,
        event: Event,
        _: &Registry,
    ) -> inventory::Result<()> {
        let label = listener.value::<Recording>().expect("recording listener").0;
        let slots: Vec<Value> = {
            let event = event.read();
            let items = event.items.read();
            event
                .slots()
                .into_iter()
                .map(|slot| {
                    let item = items[slot].as_ref().unwrap().read();
                    json!([slot, item.id(), item.amount()])
                })
                .collect()
        };
        self.world
            .borrow_mut()
            .recorded("containerListener.update", json!([label, slots]))
            .map(|_| ())
            .map_err(|_| inventory::Error::Binding("listener".into()))
    }
    fn refresh(
        &mut self,
        listener: Listener,
        _: &mut Container,
        _: &Registry,
    ) -> inventory::Result<()> {
        let label = listener.value::<Recording>().expect("recording listener").0;
        self.world
            .borrow_mut()
            .recorded("containerListener.refresh", json!([label]))
            .map(|_| ())
            .map_err(|_| inventory::Error::Binding("listener".into()))
    }
    fn ground(
        &mut self,
        _: &mut Container,
        _: Option<Handle>,
        _: &Registry,
    ) -> inventory::Result<()> {
        Err(inventory::Error::Binding("no ground items".into()))
    }
}

fn slots(container: &Container) -> Value {
    Value::Array(
        container
            .items()
            .read()
            .iter()
            .map(|item| match item {
                Some(item) => {
                    let item = item.read();
                    json!([item.id(), item.amount(), item.charge()])
                }
                None => Value::Null,
            })
            .collect(),
    )
}
fn skill_state(skills: &Skills) -> Value {
    let levels: Vec<Value> = (0..24)
        .map(|i| {
            json!([
                skills.dynamic_levels[i],
                skills.static_levels[i],
                bits(skills.experience[i])
            ])
        })
        .collect();
    json!([
        levels,
        skills.lifepoints,
        bits(skills.prayer_points),
        bits(skills.experience_gained),
        skills.total_level()
    ])
}
fn set_levels(skills: &mut Skills, declared: &Value) {
    for (key, value) in declared.as_object().into_iter().flatten() {
        let slot: usize = key.parse().unwrap();
        let fixed = value[1].as_i64().unwrap() as i32;
        skills.dynamic_levels[slot] = value[0].as_i64().unwrap() as i32;
        skills.static_levels[slot] = fixed;
        skills.experience[slot] = f64::from(Skills::experience_by_level(fixed));
    }
}

enum Active {
    None,
    Wood(woodcutting::Pulse),
    Mine(mining::Pulse),
    Fish(fishing::Pulse),
}

/// `names` are the declared item names of the corpus (its provenance).
pub fn evaluate(case: &Value, names: &Value) -> Value {
    // Definitions as the oracle declares them: a name per id, stackable ids listed by the case.
    let mut definitions = Registry::default();
    let stackable: Vec<i64> = case["stackable"]
        .as_array()
        .map(|a| a.iter().map(|v| v.as_i64().unwrap()).collect())
        .unwrap_or_default();
    let mut declared = names.as_object().unwrap().clone();
    for (key, value) in case["names"].as_object().into_iter().flatten() {
        declared.insert(key.clone(), value.clone());
    }
    for (key, name) in &declared {
        let id: i32 = key.parse().unwrap();
        definitions.insert(
            id,
            Shared::new(ItemDefinition {
                id,
                name: Some(name.as_str().unwrap().to_owned()),
                stackable: stackable.contains(&i64::from(id)),
                ..ItemDefinition::default()
            }),
        );
    }
    let item =
        |id: i64, amount: i64| Shared::new(Item::new(id as i32, amount as i32, 1000, &definitions));
    let world = RefCell::new(World::new(case));
    let (combat, summoning) = (Cell::new(0), Cell::new(0));
    let services = Services {
        world: &world,
        combat: &combat,
        summoning: &summoning,
        setup: None,
    };
    let mut ticks = case["ticks"].as_i64().unwrap_or(100);
    let mut setup = Services {
        setup: Some(ticks as i32),
        ..services
    };
    // The same construction and set-up calls as the oracle's Real.reset.
    let mut skills = Skills::new(&mut setup).unwrap();
    set_levels(&mut skills, &case["levels"]);
    for (key, value) in case["xp"].as_object().into_iter().flatten() {
        skills.experience[key.parse::<usize>().unwrap()] = value.as_f64().unwrap();
    }
    let factory = Factory::new(&definitions);
    let mut inventory = factory
        .create(28, ContainerType::Default, SortType::Id)
        .unwrap();
    let mut equipment = factory
        .create(14, ContainerType::Default, SortType::Id)
        .unwrap();
    for (slot, entry) in case["inventory"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
    {
        let held = item(entry[0].as_i64().unwrap(), entry[1].as_i64().unwrap());
        inventory
            .replace(Some(&held), slot as i32, false, &definitions, &mut setup)
            .unwrap();
    }
    let mut worn: Vec<(i32, i64)> = case["equipment"]
        .as_object()
        .into_iter()
        .flatten()
        .map(|(slot, id)| (slot.parse().unwrap(), id.as_i64().unwrap()))
        .collect();
    worn.sort_unstable();
    for (slot, id) in worn {
        equipment
            .replace(Some(&item(id, 1)), slot, false, &definitions, &mut setup)
            .unwrap();
    }
    inventory.refresh(None, &definitions, &mut setup).unwrap();
    equipment.refresh(None, &definitions, &mut setup).unwrap();
    inventory.register(Some(Listener::new(Recording(0))));
    equipment.register(Some(Listener::new(Recording(1))));

    let settings = Settings {
        max_players: case["maxPlayers"].as_i64().unwrap_or(2000) as i32,
    };
    let subject = case["subject"].as_str().unwrap();
    let fishing_subject = subject.starts_with("fishing");
    let spot = fishing_subject.then_some(1i64);
    assert!(
        case["node"]["kind"].is_null(),
        "binding cases interact with the declared node"
    );
    let (player, node) = (1000i64, spot.unwrap_or(0));
    let mut gems = mining::State::default();
    let mut active = Active::None;
    let (mut has_script, mut script_next, mut script_state) = (true, 0i64, 0i64);
    let persist = case["scriptPersist"].as_bool().unwrap_or(true);
    let mut full = serde_json::Map::new();
    let mut out = Vec::new();
    for (op, expected) in case["ops"]
        .as_array()
        .unwrap()
        .iter()
        .zip(case["expected"].as_array().unwrap())
    {
        world.borrow_mut().begin(&expected["events"]);
        let (mut required, mut skill_services, mut item_services) = (services, services, services);
        let mut host = Counted(Host {
            skills: &mut skills,
            inventory: &mut inventory,
            equipment: &mut equipment,
            definitions: &definitions,
            required: &mut required,
            skill_services: &mut skill_services,
            item_services: &mut item_services,
        });
        let wrap = |r: Result<bool>| r.map(|b| json!(b));
        let unit = |r: Result<()>| r.map(|()| Value::Null);
        let mut handle = |host: &mut Counted<
            Host<'_, Services<'_>, Services<'_>, Services<'_>>,
        >,
                          state: i32| match subject {
            "woodcutting" => woodcutting::handle_woodcutting(host, &settings, player, node, state),
            "mining" => mining::handle_mining(host, &settings, &mut gems, player, node, state),
            "fishing" => fishing::handle_fishing(host, player, spot, state),
            other => panic!("no listener for {other}"),
        };
        let ret: Result<Value> = match op["op"].as_str().unwrap() {
            "define" => {
                has_script = true;
                script_next = 0;
                script_state = 0;
                unit(match subject {
                    "woodcutting" => woodcutting::define_listeners(&mut host),
                    "mining" => mining::define_listeners(&mut host),
                    _ => fishing::define_listeners(&mut host),
                })
            }
            "handler" => wrap(handle(&mut host, op["state"].as_i64().unwrap_or(0) as i32)),
            "tick" => {
                ticks += 1;
                if !has_script || script_next >= ticks {
                    Ok(json!("idle"))
                } else {
                    let r = handle(&mut host, script_state as i32);
                    if let Ok(finished) = r {
                        script_state += 1;
                        if finished && persist {
                            script_state = 0;
                        }
                    }
                    wrap(r)
                }
            }
            "pulseNew" if subject == "woodcuttingPulse" => {
                active = Active::Wood(woodcutting::Pulse::new(player, node));
                Ok(Value::Null)
            }
            "pulseNew" if subject == "miningPulse" => {
                active = Active::Mine(mining::Pulse::new(player, node));
                Ok(Value::Null)
            }
            "pulseNew" => {
                let option = op["option"]
                    .as_str()
                    .map(|name| FishingOption::named(name).unwrap());
                fishing::Pulse::new(&mut host, player, node, option).map(|p| {
                    active = Active::Fish(p);
                    Value::Null
                })
            }
            "pulseStart" => unit(match &mut active {
                Active::Wood(p) => p.start(&mut host),
                Active::Mine(p) => p.start(&mut host),
                Active::Fish(p) => p.start(&mut host),
                Active::None => panic!("no pulse"),
            }),
            "pulseStop" => unit(match &mut active {
                Active::Wood(p) => p.stop(&mut host),
                Active::Mine(p) => p.stop(&mut host),
                Active::Fish(p) => p.stop(&mut host),
                Active::None => panic!("no pulse"),
            }),
            "pulsePulse" => wrap(match &mut active {
                Active::Wood(p) => p.pulse(&mut host, &settings),
                Active::Mine(p) => p.pulse(&mut host, &settings, &mut gems),
                Active::Fish(p) => p.pulse(&mut host),
                Active::None => panic!("no pulse"),
            }),
            "set" => {
                // The oracle's Real.set, call for call; nothing fires a listener.
                let host = &mut host.0;
                set_levels(host.skills, &op["levels"]);
                let mut quiet = services;
                if let Some(k) = op["fill"].as_i64() {
                    while i64::from(28 - host.inventory.free_slots()) < k {
                        host.inventory
                            .add(Some(&item(1, 1)), false, -1, &definitions, &mut quiet)
                            .unwrap();
                    }
                }
                if let Some(id) = op["take"].as_i64() {
                    host.inventory
                        .remove(Some(&item(id, 1)), None, false, &definitions, &mut quiet)
                        .unwrap();
                }
                if let Some(give) = op["give"].as_array() {
                    let given = item(give[0].as_i64().unwrap(), give[1].as_i64().unwrap());
                    host.inventory
                        .add(Some(&given), false, -1, &definitions, &mut quiet)
                        .unwrap();
                }
                if let Some(equip) = op["equip"].as_array() {
                    let worn = equip[1].as_i64().map(|id| item(id, 1));
                    host.equipment
                        .replace(
                            worn.as_ref(),
                            equip[0].as_i64().unwrap() as i32,
                            false,
                            &definitions,
                            &mut quiet,
                        )
                        .unwrap();
                }
                if let Some(t) = op["ticks"].as_i64() {
                    ticks = t;
                }
                let mut world = world.borrow_mut();
                if !op["sharedSeed"].is_null() {
                    world.shared = JavaRandom::new(seed(op, "sharedSeed", 0));
                }
                if !op["mathSeed"].is_null() {
                    world.math = JavaRandom::new(seed(op, "mathSeed", 0));
                }
                if !op["tlrSeed"].is_null() {
                    world.thread_local.seed = seed(op, "tlrSeed", 0);
                }
                Ok(Value::Null)
            }
            other => panic!("unknown binding op {other}"),
        };
        // Bound state is native and compared whole; the rest of the declared world
        // belongs to the recorded services, as in the first corpus.
        let world_now = world.borrow();
        let mut merged = full.clone();
        for (key, value) in expected["state"].as_object().into_iter().flatten() {
            merged.insert(key.clone(), value.clone());
        }
        merged.insert("realSkills".into(), skill_state(&skills));
        merged.insert("realInventory".into(), slots(&inventory));
        merged.insert("realEquipment".into(), slots(&equipment));
        merged.insert("tlr".into(), json!(world_now.thread_local.seed.to_string()));
        merged.insert(
            "gems".into(),
            Value::Array(gems.gem_rock_gems.iter().map(|g| json!(g.0)).collect()),
        );
        merged.insert("scriptState".into(), json!(script_state));
        merged.insert("queued".into(), json!(world_now.scripts.len()));
        let running = match &active {
            Active::Wood(p) => Some(p.running),
            Active::Mine(p) => Some(p.running),
            Active::Fish(p) => Some(p.running),
            Active::None => None,
        };
        if let (Some(running), Some(pulse)) = (running, merged.get_mut("pulse")) {
            pulse[0] = json!(running);
        }
        if let Active::Fish(p) = &active {
            if let Some(pulse) = merged.get_mut("pulse") {
                pulse[1] = json!(p.delay);
            }
            if p.fish.is_some() || merged.contains_key("fish") {
                merged.insert("fish".into(), json!(p.fish.map(|f| f.name)));
            }
        }
        has_script = merged
            .get("hasScript")
            .and_then(Value::as_bool)
            .unwrap_or(has_script);
        script_next = merged
            .get("scriptNext")
            .and_then(Value::as_i64)
            .unwrap_or(script_next);
        let snapshot: serde_json::Map<String, Value> = merged
            .iter()
            .filter(|(key, value)| full.get(*key) != Some(*value))
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        full = merged;
        let mut result = json!({"events":world_now.log.clone(),"state":snapshot});
        match ret {
            Ok(v) => result["ret"] = v,
            Err(e) => result["error"] = json!(failure(e)),
        }
        out.push(result);
    }
    Value::Array(out)
}
