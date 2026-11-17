//! The canonical player: its profile state against original freshly
//! constructed and freshly loaded Players (fixtures/canonical-player), and the
//! character save file written from and loaded into it (the world-trace
//! character files and every whole file of the player-save corpus).
use rs09_game_core::java_json::{self, Json};
use rs09_server::{
    auth::Clock,
    definitions::item::{ItemDefinition, Registry, Shared},
    inventory::{
        self,
        container::{Container, ContainerType, Event, Factory, SortType},
        container_listener::Handle,
        item::Handle as ItemHandle,
        listener::Listener,
        player_binding::Capture,
    },
    player::{
        attributes::AttributeValue,
        core_data::CoreData,
        details::PlayerDetails,
        owned::{
            Player,
            profile::{Environment, Fresh, Profile},
        },
        persisted::{
            host::{Remaining, World},
            music, quests, spells,
        },
        save::{self, Level, Parser},
        settings::{self, Settings},
        skills::{self, Effect, EntityKind, Skills},
    },
    timers,
    world::{
        appearance,
        doors::Location,
        interfaces::Manager,
        player_render,
        player_walk::{NodeHandle, State},
        prepared_masks::PreparedMasks,
    },
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::sync::Arc;

const FIXTURE: &str = include_str!("../../../fixtures/canonical-player/oracle.jsonl");
const PROVENANCE: &str = include_str!("../../../fixtures/canonical-player/provenance.json");
const TRACE: &str = include_str!("../../../fixtures/world-trace/trace.jsonl");
const SAVE_CORPUS: &str = include_str!("../../../fixtures/player-save/oracle.jsonl");

// ---------------------------------------------------------------- services

/// The services that stay outside the canonical player, as stand-ins. Content
/// hooks write the JSON they are given (what the original's hooks wrote); every
/// other service succeeds without effect. None of this is a port.
#[derive(Default)]
struct Rest {
    hooks: Vec<Vec<(String, String)>>,
    calls: Vec<String>,
}
impl Remaining for Rest {
    fn log(&mut self, _: Level, message: &str) {
        self.calls.push(format!("log {message}"));
    }
    fn retaliating(&mut self) -> save::Result<bool> {
        unreachable!("answered by the canonical player")
    }
    fn attribute_set(&mut self, key: &str, _: &AttributeValue) -> save::Result<()> {
        self.calls.push(format!("attribute_set {key}"));
        Ok(())
    }
    fn content_hooks(&mut self) -> usize {
        self.hooks.len()
    }
    fn save_content(&mut self, hook: usize, root: &mut Json) -> save::Result<()> {
        for (key, value) in &self.hooks[hook] {
            root.put(key.clone(), java_json::parse(value).unwrap())
                .unwrap();
        }
        Ok(())
    }
    fn parse_content(&mut self, hook: usize, _: &Json) -> save::Result<()> {
        self.calls.push(format!("parse_content {hook}"));
        Ok(())
    }
    fn parse_familiars(&mut self, _: &Json) -> save::Result<()> {
        self.calls.push("parse_familiars".into());
        Ok(())
    }
    fn parse_house(&mut self, _: &Json) -> save::Result<()> {
        self.calls.push("parse_house".into());
        Ok(())
    }
}
impl inventory::container::Context for Rest {
    fn update(
        &mut self,
        _: Handle,
        _: &mut Container,
        _: Event,
        _: &Registry,
    ) -> inventory::Result<()> {
        Ok(())
    }
    fn refresh(&mut self, _: Handle, _: &mut Container, _: &Registry) -> inventory::Result<()> {
        Ok(())
    }
    fn ground(
        &mut self,
        _: &mut Container,
        _: Option<ItemHandle>,
        _: &Registry,
    ) -> inventory::Result<()> {
        Ok(())
    }
}
impl skills::Context for Rest {
    fn kind(&self) -> EntityKind {
        EntityKind::Player
    }
    fn version(&self) -> i32 {
        unreachable!()
    }
    fn tick(&mut self) -> i32 {
        0
    }
    fn world_tick(&mut self) -> i32 {
        0
    }
    fn combat_level(&self) -> i32 {
        unreachable!("answered by the canonical player")
    }
    fn summoning_combat_level(&self) -> i32 {
        unreachable!("answered by the canonical player")
    }
    fn npc_combat_level(&mut self) -> skills::Result<i32> {
        unreachable!()
    }
    fn damage_spong_active(&mut self) -> skills::Result<bool> {
        unreachable!()
    }
    fn assisted_level(&mut self, _: i32) -> skills::Result<Option<i32>> {
        unreachable!()
    }
    fn translate_experience(
        &mut self,
        _: &mut Skills,
        _: i32,
        _: f64,
        _: f64,
    ) -> skills::Result<bool> {
        unreachable!()
    }
    fn brawling_present(&mut self) -> skills::Result<bool> {
        unreachable!()
    }
    fn brawling_for_skill(&mut self, _: i32) -> skills::Result<Option<i32>> {
        unreachable!()
    }
    fn equipment_contains(&mut self, _: i32) -> skills::Result<bool> {
        unreachable!()
    }
    fn brawling_bonus(&mut self) -> skills::Result<f64> {
        unreachable!()
    }
    fn tutorial_complete(&mut self) -> skills::Result<bool> {
        unreachable!()
    }
    fn artificial(&mut self) -> skills::Result<bool> {
        unreachable!()
    }
    fn username(&mut self) -> skills::Result<String> {
        unreachable!()
    }
    fn permadeath_present(&self) -> bool {
        unreachable!("the parser supplies the permadeath attribute")
    }
    fn effect(&mut self, _: &mut Skills, effect: Effect) -> skills::Result<()> {
        self.calls.push(format!("effect {effect:?}"));
        Ok(())
    }
}
impl settings::Context for Rest {
    fn varp(&mut self, _: &Settings, _: i32, _: i32) -> settings::Result<()> {
        unreachable!()
    }
    fn session(&mut self, _: &Settings) -> settings::Result<Option<usize>> {
        unreachable!()
    }
    fn send(
        &mut self,
        _: &Settings,
        _: Option<usize>,
        _: rs09_protocol::game::Frame,
    ) -> settings::Result<()> {
        unreachable!()
    }
    fn dispatcher(&mut self, _: &Settings) -> settings::Result<Option<usize>> {
        unreachable!()
    }
    fn run_energy(&mut self, _: &Settings, _: Option<usize>) -> settings::Result<()> {
        unreachable!()
    }
    fn message(&mut self, _: &Settings, _: Option<usize>, _: &str) -> settings::Result<()> {
        unreachable!()
    }
    fn string(
        &mut self,
        _: &Settings,
        _: Option<usize>,
        _: String,
        _: i32,
        _: i32,
    ) -> settings::Result<()> {
        unreachable!()
    }
    fn properties(&mut self, _: &Settings) -> settings::Result<Option<usize>> {
        unreachable!("answered by the canonical player")
    }
    fn retaliating(&mut self, _: &Settings, _: Option<usize>) -> settings::Result<bool> {
        unreachable!("answered by the canonical player")
    }
    fn set_retaliating(&mut self, _: &Settings, _: Option<usize>, _: bool) -> settings::Result<()> {
        unreachable!("answered by the canonical player")
    }
    fn ironman(&mut self, _: &Settings) -> settings::Result<bool> {
        unreachable!("answered by the canonical player")
    }
    fn container(&mut self, _: &Settings, _: settings::ContainerKind) -> settings::Result<usize> {
        unreachable!()
    }
    fn item(&mut self, _: &Settings, _: usize, _: i32) -> settings::Result<Option<usize>> {
        unreachable!()
    }
    fn weight(&mut self, _: &Settings, _: usize) -> settings::Result<f64> {
        unreachable!()
    }
}

/// Timer content is not ported. This stand-in resolves `antimacro` to a timer
/// with AntiMacro's own two persistence methods (`ticksRemaining`, against
/// world tick 0) and any other identifier to a persistent timer with RSTimer's
/// base save and parse (`ticksLeft`).
struct BaseTimers;
impl timers::Context<CoreData> for BaseTimers {
    fn is_player(&self, _: &CoreData) -> bool {
        true
    }
    fn modal_open(&mut self, _: &mut CoreData) -> timers::Result<bool> {
        Ok(false)
    }
    fn script_delay(&mut self, _: &mut CoreData) -> timers::Result<i32> {
        Ok(0)
    }
    fn ticks(&mut self, _: &mut CoreData) -> timers::Result<i32> {
        Ok(0)
    }
    fn entity_name(&mut self, _: &mut CoreData) -> timers::Result<Option<String>> {
        Ok(None)
    }
    fn before_register(
        &mut self,
        _: &timers::Handle,
        _: &mut timers::Manager,
        _: &mut CoreData,
    ) -> timers::Result<()> {
        Ok(())
    }
    fn on_register(
        &mut self,
        _: &timers::Handle,
        _: &mut timers::Manager,
        _: &mut CoreData,
    ) -> timers::Result<()> {
        Ok(())
    }
    fn on_removal(
        &mut self,
        _: &timers::Handle,
        _: &mut timers::Manager,
        _: &mut CoreData,
    ) -> timers::Result<()> {
        Ok(())
    }
    fn run(
        &mut self,
        _: &timers::Handle,
        _: &mut timers::Manager,
        _: &mut CoreData,
    ) -> timers::Result<bool> {
        Ok(true)
    }
    fn initial_delay(
        &mut self,
        _: &timers::Handle,
        _: &mut timers::Manager,
        _: &mut CoreData,
    ) -> timers::Result<i32> {
        Ok(0)
    }
    fn save(
        &mut self,
        timer: &timers::Handle,
        root: &mut rs09_server::definitions::item_config::json::Json,
        _: &mut timers::Manager,
        state: &mut CoreData,
    ) -> timers::Result<()> {
        if timer.read().class_name == "AntiMacro" {
            let left = timer.read().next_execution.wrapping_sub(0);
            root.put(
                "ticksRemaining",
                rs09_server::definitions::item_config::json::Json::Text(left.to_string()),
            )
            .map_err(|_| timers::Error::Type)?;
            return Ok(());
        }
        timers::save_base(timer, root, state, self)
    }
    fn parse(
        &mut self,
        timer: &timers::Handle,
        data: &rs09_server::definitions::item_config::json::Json,
        _: &mut timers::Manager,
        _: &mut CoreData,
    ) -> timers::Result<()> {
        if timer.read().class_name == "AntiMacro" {
            let text = data.get("ticksRemaining").map(|v| v.text());
            let interval = text
                .and_then(|t| rs09_game_core::java_string::parse_i32(&t))
                .unwrap_or(0);
            let mut timer = timer.write();
            timer.run_interval = interval;
            timer.next_execution = interval;
            return Ok(());
        }
        timers::parse_base(timer, data)
    }
    fn resolve(
        &mut self,
        identifier: &str,
        _: &mut timers::Manager,
        _: &mut CoreData,
    ) -> timers::Result<Option<timers::Handle>> {
        let class = if identifier == "antimacro" {
            "AntiMacro"
        } else {
            "RSTimer"
        };
        Ok(Some(Shared::new(timers::Timer::new(
            0,
            identifier.to_owned(),
            class.into(),
            true,
        ))))
    }
    fn run_failure(&mut self, _: &str, _: Option<&str>, _: &mut CoreData) -> timers::Result<()> {
        Ok(())
    }
    fn unknown_timer(&mut self, _: &str, _: &mut CoreData) -> timers::Result<()> {
        Ok(())
    }
    fn exception(&mut self, _: &timers::Error, _: &mut CoreData) -> timers::Result<()> {
        Ok(())
    }
}

// ---------------------------------------------------------------- players

struct Canonical {
    state: State,
    player: Player,
    _owner: NodeHandle,
    location: Option<Location>,
}
fn canonical(
    name: &str,
    definitions: Arc<Registry>,
    world: &World,
    clock: Arc<Clock>,
    location: Location,
    ticks: i32,
) -> Canonical {
    let factory = Factory::new(&definitions);
    let mut state = State::default();
    let node = state.add(1, location.clone(), [1, 2, 3, 4]);
    let owner = state.retain_node(node).unwrap();
    let mut inventory = factory
        .create(28, ContainerType::Default, SortType::Id)
        .unwrap();
    inventory.register(Some(Handle::new(Listener::new(Some(owner.clone())))));
    let equipment = factory
        .create(14, ContainerType::Default, SortType::Id)
        .unwrap();
    let core = Capture::new(
        owner.clone(),
        inventory,
        equipment,
        Default::default(),
        factory.null_item.clone(),
    );
    let masks = PreparedMasks::new(
        Shared::new(player_render::Player::new(1, location.clone(), 2000)),
        Shared::new(appearance::Data::naked(name.into(), 0)),
    );
    let player = Player::new(
        state.identity(),
        core,
        definitions,
        Manager::default(),
        Shared::new(false),
        masks,
    )
    .unwrap();
    state.bind_player_fields(player.clone()).unwrap();
    let details = PlayerDetails::new(name, clock);
    let profile = Profile::new(details, Fresh { world, ticks }, &factory).unwrap();
    player.bind_profile(profile).unwrap();
    Canonical {
        state,
        player,
        _owner: owner,
        location: Some(location),
    }
}
impl Canonical {
    fn save(&self, clock: &Clock, world: &mut World, rest: &mut Rest) -> String {
        let mut timers = BaseTimers;
        let mut env = Environment {
            clock,
            timers: &mut timers,
            services: rest,
            world,
            attribute_path: "unused-legacy-attributes/",
        };
        self.player
            .save_text(&self.state, self.location.as_ref(), &mut env)
            .unwrap()
    }
    /// parse, then runContentHooks, as LoginParser does; the location the
    /// load produced becomes the location the next save writes.
    fn load(
        &mut self,
        text: &str,
        clock: &Clock,
        world: &mut World,
        rest: &mut Rest,
    ) -> Result<(), String> {
        let mut timers = BaseTimers;
        let mut env = Environment {
            clock,
            timers: &mut timers,
            services: rest,
            world,
            attribute_path: "unused-legacy-attributes/",
        };
        let mut parser = Parser::default();
        let outcome = self
            .player
            .load_text(
                &self.state,
                self.location.as_ref(),
                &mut parser,
                text,
                &mut env,
            )
            .unwrap();
        self.location = outcome.location;
        outcome.result.map_err(|e| format!("{:?}", e.category()))?;
        self.player
            .run_content_hooks(&self.state, self.location.as_ref(), &parser, &mut env)
            .unwrap()
            .map_err(|e| format!("{:?}", e.category()))
    }
}

fn fixed_clock(value: i64) -> Arc<Clock> {
    Arc::new(move || value)
}
fn define(definitions: &mut Registry, id: i32, stackable: bool) {
    definitions.insert(
        id,
        Shared::new(ItemDefinition {
            id,
            name: Some(format!("item{id}")),
            value: id * 11,
            stackable,
            ..Default::default()
        }),
    );
}
fn int(value: &Value) -> i32 {
    value.as_i64().unwrap() as i32
}
/// The first place two texts part, for messages.
fn parting(a: &str, b: &str) -> String {
    let (a, b) = (a.chars().collect::<Vec<_>>(), b.chars().collect::<Vec<_>>());
    let at = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let window = |t: &[char]| {
        t[at.saturating_sub(50)..t.len().min(at + 50)]
            .iter()
            .collect::<String>()
    };
    format!(
        "at char {at}: native=...{:?}..., original=...{:?}...",
        window(&a),
        window(&b)
    )
}
/// The top-level save keys whose values differ.
fn differing_keys(native: &str, original: &str) -> Vec<String> {
    let a: Value = serde_json::from_str(native).unwrap();
    let b: Value = serde_json::from_str(original).unwrap();
    let (a, b) = (a.as_object().unwrap(), b.as_object().unwrap());
    let mut keys: Vec<String> = a
        .keys()
        .chain(b.keys())
        .filter(|k| a.get(*k) != b.get(*k))
        .cloned()
        .collect();
    keys.sort();
    keys.dedup();
    keys
}

// ---------------------------------------------------------------- fixture

fn records() -> Vec<Value> {
    FIXTURE
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}
fn world_inputs(record: &Value) -> World {
    let mut world = World::default();
    for row in record["quests"].as_array().unwrap() {
        world.quests.register(
            int(&row[0]),
            quests::Quest {
                name: row[1].as_str().unwrap().to_owned(),
                index: int(&row[2]),
                button: int(&row[3]),
                points: int(&row[4]),
            },
        );
    }
    for row in record["songs"].as_array().unwrap() {
        world.songs.songs.insert(
            int(&row[0]),
            music::Song {
                name: row[1].as_str().unwrap().to_owned(),
                index: int(&row[2]),
            },
        );
    }
    world
}
fn start_location(record: &Value) -> Location {
    let v: Vec<i32> = record["startLocation"]
        .as_str()
        .unwrap()
        .trim_start_matches("Location(")
        .trim_end_matches(')')
        .split(',')
        .map(|n| n.parse().unwrap())
        .collect();
    Location::new(v[0], v[1], v[2])
}
fn hooks(record: &Value) -> Vec<Vec<(String, String)>> {
    record["populate"]["hooks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|hook| {
            hook["keys"]
                .as_array()
                .unwrap()
                .iter()
                .map(|row| {
                    let key = row[0].as_str().unwrap().to_owned();
                    let one = java_json::parse(row[1].as_str().unwrap()).unwrap();
                    (key.clone(), one.get(&key).unwrap().text())
                })
                .collect()
        })
        .collect()
}

#[test]
fn fixture_pins_its_sources_and_the_world_trace_boot() {
    let p: Value = serde_json::from_str(PROVENANCE).unwrap();
    assert_eq!(
        p["fixtureSha256"],
        format!("{:x}", Sha256::digest(FIXTURE.as_bytes()))
    );
    assert_eq!(p["bootRecord"]["identicalToWorldTrace"], true);
    assert_eq!(
        p["bootRecord"]["worldTraceFixtureSha256"],
        format!("{:x}", Sha256::digest(TRACE.as_bytes()))
    );
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    for (source, expected) in p["sources"].as_object().unwrap() {
        let path = root.join(source);
        if path.is_file() {
            let data: Vec<u8> = std::fs::read(path)
                .unwrap()
                .into_iter()
                .filter(|b| *b != b'\r')
                .collect();
            assert_eq!(
                format!("{:x}", Sha256::digest(data)),
                expected.as_str().unwrap(),
                "{source}"
            );
        }
    }
}

// ---------------------------------------------------------------- player-save corpus

/// Every whole file the player-save corpus holds that the original wrote
/// (populate texts and saved files without error), with the world inputs of
/// its case: (case index, label, name, clock, world, definitions, hooks, text).
struct WholeFile {
    index: usize,
    label: String,
    name: String,
    clock: i64,
    world: World,
    definitions: Registry,
    hooks: Vec<Vec<(String, String)>>,
    text: String,
}
fn whole_files() -> Vec<WholeFile> {
    let mut out = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for (index, line) in SAVE_CORPUS.lines().enumerate() {
        let case: Value = serde_json::from_str(line).unwrap();
        let state = case["state"] == true;
        let config = if state {
            &case["world"]
        } else {
            &case["config"]
        };
        let w = if state {
            &case["world"]
        } else {
            &case["config"]["world"]
        };
        for (action, expected) in case["actions"]
            .as_array()
            .unwrap()
            .iter()
            .zip(case["expected"].as_array().unwrap())
        {
            let key = match action["op"].as_str().unwrap() {
                "populate" => "text",
                "profileSave" => "file",
                _ => continue,
            };
            let step = if state { expected } else { &expected["result"] };
            let (Value::Null, Some(text)) = (&step["error"], step[key].as_str()) else {
                continue;
            };
            let mut definitions = Registry::default();
            for id in 0..8 {
                define(&mut definitions, id, false);
            }
            let mut world = World::default();
            if state {
                for row in w["items"].as_array().into_iter().flatten() {
                    define(&mut definitions, int(&row[0]), row[1] == true);
                }
                for row in w["quests"].as_array().into_iter().flatten() {
                    world.quests.register(
                        int(&row[0]),
                        quests::Quest {
                            name: String::new(),
                            index: int(&row[1]),
                            button: int(&row[2]),
                            points: int(&row[3]),
                        },
                    );
                }
                for row in w["songs"].as_array().into_iter().flatten() {
                    world.songs.songs.insert(
                        int(&row[0]),
                        music::Song {
                            name: row[1].as_str().unwrap().to_owned(),
                            index: int(&row[2]),
                        },
                    );
                }
                for row in w["spells"].as_array().into_iter().flatten() {
                    world.spells.spells.insert(
                        (int(&row[0]), int(&row[1])),
                        spells::Spell {
                            book: int(&row[3]),
                            combat: row[2] == true,
                        },
                    );
                }
            } else {
                for row in w["songs"].as_array().into_iter().flatten() {
                    world.songs.songs.insert(
                        int(&row[0]),
                        music::Song {
                            name: String::new(),
                            index: int(&row[1]),
                        },
                    );
                }
                for row in w["spells"].as_array().into_iter().flatten() {
                    world.spells.spells.insert(
                        (int(&row[0]), int(&row[1])),
                        spells::Spell {
                            book: int(&row[0]),
                            combat: row[2] == true,
                        },
                    );
                }
            }
            let hooks = w["hooks"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|row| {
                    vec![(
                        row[0].as_str().unwrap().to_owned(),
                        row[1].as_str().unwrap().to_owned(),
                    )]
                })
                .collect();
            let name = config["name"].as_str().unwrap_or("entity").to_owned();
            if !seen.insert((text.to_owned(), name.clone(), w.to_string())) {
                continue;
            }
            out.push(WholeFile {
                index,
                label: case["label"].as_str().unwrap_or("").to_owned(),
                name,
                clock: config["clock"].as_i64().unwrap_or(0),
                world,
                definitions,
                hooks,
                text: text.to_owned(),
            });
        }
    }
    out
}
/// What a whole file does after a load into a new canonical player and a save:
/// `Ok(true)` byte for byte, `Ok(false)` with the differing top-level keys,
/// `Err` when the load fails.
fn round_trip(file: WholeFile) -> (usize, String, Result<Vec<String>, String>) {
    let WholeFile {
        index,
        label,
        name,
        clock,
        mut world,
        definitions,
        hooks,
        text,
    } = file;
    let clock = fixed_clock(clock);
    let mut p = canonical(
        &name,
        Arc::new(definitions),
        &world,
        clock.clone(),
        Location::new(3222, 3218, 0),
        0,
    );
    let mut rest = Rest {
        hooks,
        ..Default::default()
    };
    if let Err(e) = p.load(&text, &*clock, &mut world, &mut rest) {
        if std::env::var("RS09_CANONICAL_CASES")
            .is_ok_and(|v| v.split(',').any(|c| c == index.to_string()))
        {
            eprintln!(
                "case {index} ({label}) load failed {e}: {}",
                &text[..text.len().min(400)]
            );
        }
        return (index, label, Err(e));
    }
    let written = p.save(&*clock, &mut world, &mut rest);
    let keys = if written == text {
        Vec::new()
    } else {
        differing_keys(&written, &text)
    };
    if !keys.is_empty()
        && std::env::var("RS09_CANONICAL_CASES")
            .is_ok_and(|v| v.split(',').any(|c| c == index.to_string()))
    {
        let (a, b): (Value, Value) = (
            serde_json::from_str(&written).unwrap(),
            serde_json::from_str(&text).unwrap(),
        );
        for key in &keys {
            let (x, y) = (a[key].to_string(), b[key].to_string());
            eprintln!("case {index} ({label}) {key}: {}", parting(&x, &y));
        }
    }
    (index, label, Ok(keys))
}

// ---------------------------------------------------------------- original Players

fn bits(value: f64) -> Value {
    json!(format!("d:{:016x}", value.to_bits()))
}
fn animation(a: &Option<rs09_server::world::animation::Animation>) -> Value {
    a.as_ref().map_or(Value::Null, |a| {
        json!(format!(
            "Animation({},{},{})",
            a.id(),
            a.delay(),
            a.priority().map_or("null", |p| p.name())
        ))
    })
}
fn location(l: &Option<Location>) -> Value {
    l.as_ref().map_or(Value::Null, |l| {
        let [x, y, z] = l.coordinates();
        json!(format!("Location({x},{y},{z})"))
    })
}

/// The canonical player's values under the original classes' field names.
/// Only the fields the canonical player holds are produced; the fields left
/// out are listed in NOT_HELD below.
fn observe(c: &Canonical, node: rs09_server::world::player_walk::Node) -> Value {
    let p = &c.player;
    let settings = p.settings().fields.read().clone();
    let variables = p.variables();
    let values = variables.values().unwrap().snapshot();
    let saved = variables.saved().unwrap().snapshot();
    let tabs = |b: rs09_server::inventory::bank::Bank| json!(b.tab_start_slots().read().clone());
    let actor = c.state.actor(node).unwrap();
    let owner = p.owner();
    let clocks = c.state.clocks(&owner).unwrap().read().clone();
    let pouch = *p.pouch().read();

    let capacities = json!([
        p.inventory().capacity(),
        p.equipment().capacity(),
        p.bank_primary().container().capacity(),
        p.bank_secondary().container().capacity(),
        p.drop_log().container().capacity()
    ]);
    p.with_profile(|pr| {
        let props = &pr.properties;
        let s = &pr.skills;
        let account = pr.details.account.lock().unwrap().fields.clone();
        let communication = &pr.details.communication;
        let rank = |r: Option<rs09_server::player::communication::ClanRank>| {
            r.map_or(Value::Null, |r| json!(format!("enum:{}", r.name())))
        };
        let f = &pr.flags;
        json!({
            "Node": {
                "name": pr.name,
                "location": location(&actor.view.location),
                "direction": json!(format!("enum:{}", match actor.view.direction { Some(rs09_game_core::direction::Direction::South) => "SOUTH", _ => "other" })),
                "size": actor.view.size,
                "active": actor.view.active,
            },
            "Entity": {
                "clocks": clocks,
                "invisible": f.entity_invisible,
            },
            "Player": {
                "archeryTargets": f.archery_targets,
                "archeryTotal": f.archery_total,
                "artificial": f.artificial,
                "customState": f.custom_state,
                "inWardrobe": f.in_wardrobe,
                "invalidPacketCount": f.invalid_packet_count,
                "invisible": f.invisible,
                "isAfkLogout": f.is_afk_logout,
                "newPlayer": f.new_player,
                "opCounts": f.op_counts,
                "playing": f.playing,
                "startLocation": f.start_location.map(|[x, y, z]| format!("Location({x},{y},{z})")),
                "useSecondaryBank": p.use_secondary_bank(),
                "version": pr.version,
                "varpMap": format!("map:java.util.HashMap[{}]", values.len()),
                "saveVarp": format!("map:java.util.HashMap[{}]", saved.len()),
            },
            "Properties": {
                "armourSet": props.armour_set.map(|a| format!("{a:?}")),
                "attackAnimation": animation(&props.attack_animation),
                "attackSpeed": props.attack_speed,
                "attackStyle": props.attack_style.map(|a| format!("{a:?}")),
                "autocastSpell": pr.persisted.spells.autocast.map(|a| format!("{a:?}")),
                "bonuses": props.bonuses,
                "combatLevel": props.combat_level,
                "combatTimeOut": props.combat_time_out,
                "deathAnimation": animation(&props.death_animation),
                "deathGfx": props.death_gfx.as_ref().map(|g| format!("Graphics({},{},{})", g.id(), g.height(), g.delay())),
                "defenceAnimation": animation(&props.defence_animation),
                "lastAnimationEnd": props.last_animation_end,
                "magicAnimation": animation(&props.magic_animation),
                "multiZone": props.multi_zone,
                "npcWalkable": props.npc_walkable,
                "protectStyle": props.protect_style.map(|s| format!("{s:?}")),
                "rangeAnimation": animation(&props.range_animation),
                "retaliating": props.retaliating,
                "safeRespawn": location(&props.safe_respawn),
                "safeZone": props.safe_zone,
                "spawnLocation": location(&props.spawn_location),
                "spell": props.spell.map(|s| format!("{s:?}")),
                "teleportLocation": location(&props.teleport_location),
                "teleporting": props.teleporting,
            },
            "Skills": {
                "combatMilestone": s.combat_milestone,
                "dynamicLevels": s.dynamic_levels,
                "experience": s.experience.iter().map(|v| bits(*v)).collect::<Vec<_>>(),
                "experienceGained": bits(s.experience_gained),
                "experienceMultiplier": bits(s.experience_multiplier),
                "lastTrainedSkill": s.last_trained_skill,
                "lastUpdate": s.last_update,
                "lastUpdateXp": s.last_update_xp.map(|x| x.iter().map(|v| bits(*v)).collect::<Vec<_>>()),
                "lastXpGain": s.last_xp_gain,
                "lifepoints": s.lifepoints,
                "lifepointsIncrease": s.lifepoints_increase,
                "lifepointsUpdate": s.lifepoints_update,
                "prayerPoints": bits(s.prayer_points),
                "skillMilestone": s.skill_milestone,
                "staticLevels": s.static_levels,
            },
            "PlayerDetails": {
                "saveParsed": pr.details.save_parsed,
                "session": pr.details.session_address.as_ref().map(|_| "session"),
            },
            "UserAccountInfo": {
                "banEndTime": account.ban_end_time,
                "blocked": account.blocked,
                "clanName": account.clan_name,
                "clanReqs": account.clan_reqs,
                "contacts": account.contacts,
                "credits": account.credits,
                "currentClan": account.current_clan,
                "ip": account.ip,
                "lastLogin": account.last_login,
                "lastUsedIp": account.last_used_ip,
                "muteEndTime": account.mute_end_time,
                "online": account.online,
                "password": account.password,
                "rights": account.rights,
                "timePlayed": account.time_played,
                "uid": account.uid,
                "username": account.username,
            },
            "UIDInfo": {
                "compName": pr.details.uid.computer_name,
                "ip": pr.details.uid.ip,
                "mac": pr.details.uid.mac,
                "serial": pr.details.uid.serial,
            },
            "CommunicationInfo": {
                "blocked": format!("collection:java.util.ArrayList[{}]", communication.blocked.len()),
                "clanName": communication.clan_name,
                "contacts": format!("map:java.util.HashMap[{}]", communication.contacts.iter().count()),
                "currentClan": communication.current_clan,
                "joinRequirement": rank(communication.requirements[0]),
                "messageRequirement": rank(communication.requirements[1]),
                "kickRequirement": rank(communication.requirements[2]),
                "lootRequirement": rank(communication.requirements[3]),
                "lootShare": communication.loot_share,
            },
            "SpellBookManager": {"spellBook": pr.spell_book},
            "FamiliarManager": {
                "summoningCombatLevel": pr.summoning_combat_level,
                "hasPouch": pouch,
                "petDetails": format!("map:java.util.HashMap[{}]", pr.persisted.familiars.pets.len()),
                "familiar": match pr.persisted.familiars.active { rs09_server::player::save::ActiveFamiliar::None => Value::Null, _ => json!("familiar") },
            },
            "Settings": {
                "acceptAid": settings.accept_aid,
                "areaSoundVolume": settings.area_sound_volume,
                "assistSetting": settings.assist_setting,
                "attackStyleIndex": settings.attack_style_index,
                "brightness": settings.brightness,
                "clanChatSetting": settings.clan_chat_setting,
                "disableChatEffects": settings.disable_chat_effects,
                "musicVolume": settings.music_volume,
                "privateChatSetting": settings.private_chat_setting,
                "publicChatSetting": settings.public_chat_setting,
                "runEnergy": bits(settings.run_energy),
                "runToggled": settings.run_toggled,
                "singleMouseButton": settings.single_mouse_button,
                "soundEffectVolume": settings.sound_effect_volume,
                "specialEnergy": settings.special_energy,
                "specialToggled": settings.special_toggled,
                "splitPrivateChat": settings.split_private_chat,
                "tradeSetting": settings.trade_setting,
                "weight": bits(settings.weight),
            },
            "derived": {
                "totalLevel": s.total_level(),
                "rights": (["REGULAR_PLAYER", "PLAYER_MODERATOR", "ADMINISTRATOR"][pr.details.rights().unwrap().ordinal() as usize]),
                "ipAddress": pr.details.ip_address(),
                "bankTabs": tabs(p.bank_primary()),
                "bankSecondaryTabs": tabs(p.bank_secondary()),
                "capacities": capacities,
            },
        })
    })
    .unwrap()
}
/// Fields of the original's dump the canonical player does not hold (each an
/// object the original constructs that has no native counterpart bound to the
/// canonical player, or one held elsewhere), or holds in another form.
const NOT_HELD: &[(&str, &[&str])] = &[
    (
        "Node",
        &["destinationFlag", "index", "interactPlugin", "renderable"],
    ),
    (
        "Entity",
        &[
            "animator",
            "attributes",
            "currentMovement",
            "extensions",
            "hooks",
            "impactHandler",
            "locks",
            "properties",
            "pulseManager",
            "scripts",
            "skills",
            "teleporter",
            "timers",
            "updateMasks",
            "walkingQueue",
            "zoneMonitor",
        ],
    ),
    (
        "Player",
        &[
            "POHStorageProxyContainer",
            "POHStorageState",
            "achievementDiaryManager",
            "appearance",
            "bank",
            "bankPinManager",
            "bankSecondary",
            "degrader",
            "details",
            "dialogueInterpreter",
            "dropLog",
            "emoteManager",
            "equipment",
            "familiarManager",
            "hintIconManager",
            "houseManager",
            "interfaceManager",
            "inventory",
            "ironmanManager",
            "logoutListeners",
            "musicPlayer",
            "packetDispatch",
            "playerFlags",
            "pouchManager",
            "prayer",
            "questRepository",
            "renderInfo",
            "requestManager",
            "savedData",
            "settings",
            "skillTasks",
            "skullManager",
            "spellBookManager",
            "varpManager",
            "wardrobe_hold_graphics",
            "warningMessages",
        ],
    ),
    ("Properties", &["combatPulse", "entity"]),
    ("Skills", &["entity"]),
    (
        "PlayerDetails",
        &["accountInfo", "communicationInfo", "info"],
    ),
    ("UserAccountInfo", &["initialValues", "joinDate"]),
    ("UIDInfo", &[]),
    ("CommunicationInfo", &["clan", "lootSharePulse"]),
    ("SpellBookManager", &[]),
    ("FamiliarManager", &["player"]),
    ("Settings", &["player"]),
    ("derived", &["attributes", "varpMap", "saveVarp"]),
];
/// Fields the canonical player holds that differ from the original, each explained in docs/canonical-player.md.
const KNOWN: &[(&str, &str)] = &[
    // Node.active is false until Player.init; State.add makes an active actor.
    ("Node", "active"),
];
/// Compares every field of `original` the canonical player holds; returns the
/// differences outside KNOWN, and checks that every original field is either
/// compared or listed in NOT_HELD.
fn compare_dump(native: &Value, original: &Value, what: &str) -> Vec<String> {
    let mut out = Vec::new();
    for (section, fields) in original.as_object().unwrap() {
        let skip = NOT_HELD
            .iter()
            .find(|(s, _)| s == section)
            .map_or(&[][..], |(_, f)| *f);
        for (field, value) in fields.as_object().unwrap() {
            if skip.contains(&field.as_str()) {
                continue;
            }
            let Some(mine) = native.get(section).and_then(|s| s.get(field)) else {
                out.push(format!(
                    "{what}: {section}.{field} is neither compared nor listed"
                ));
                continue;
            };
            if KNOWN.contains(&(section.as_str(), field.as_str())) {
                continue;
            }
            if mine != value {
                out.push(format!(
                    "{what}: {section}.{field}: native={mine}, original={value}"
                ));
            }
        }
    }
    out
}

struct Scenario {
    clock: Arc<Clock>,
    world: World,
    start: Location,
    ticks: i32,
}
fn scenario() -> Scenario {
    let p: Value = serde_json::from_str(PROVENANCE).unwrap();
    let records = records();
    let w = records.iter().find(|r| r["t"] == "world").unwrap();
    Scenario {
        clock: fixed_clock(p["clockStart"].as_i64().unwrap()),
        world: world_inputs(w),
        start: start_location(w),
        ticks: int(&w["ticks"]),
    }
}

#[test]
fn fresh_canonical_player_matches_the_original_constructor_and_first_save() {
    let mut s = scenario();
    let p: Value = serde_json::from_str(PROVENANCE).unwrap();
    let name = p["name"].as_str().unwrap();
    let record = records().into_iter().find(|r| r["t"] == "fresh").unwrap();
    let c = canonical(
        name,
        Arc::new(Registry::default()),
        &s.world,
        s.clock.clone(),
        s.start.clone(),
        s.ticks,
    );
    let node = c.player.owner().node;
    let native = observe(&c, node);
    let differences = compare_dump(&native, &record["constructed"], "fresh");
    assert!(differences.is_empty(), "{differences:#?}");
    // The known difference, pinned so that a change is noticed.
    assert_eq!(record["constructed"]["Node"]["active"], false);
    assert_eq!(native["Node"]["active"], true);
    let mut rest = Rest {
        hooks: hooks(&record),
        ..Default::default()
    };
    let text = c.save(&*s.clock, &mut s.world, &mut rest);
    let original = record["populate"]["text"].as_str().unwrap();
    assert!(text == original, "{}", parting(&text, original));
}

#[test]
fn constructing_and_saving_read_the_clock_as_often_as_the_original() {
    // The original's first PlayerDetails also initialises UserAccountInfo's
    // static `default`, one more clock read than every later construction.
    let records = records();
    let fresh = records.iter().find(|r| r["t"] == "fresh").unwrap();
    let reads = |r: &Value, a: &str, b: &str| {
        r[b]["clockReads"].as_i64().unwrap() - r[a]["clockReads"].as_i64().unwrap()
    };
    assert_eq!(reads(fresh, "generatorsBefore", "generatorsConstructed"), 2);
    for load in records.iter().filter(|r| r["t"] == "load") {
        assert_eq!(reads(load, "generatorsBefore", "generatorsConstructed"), 1);
        assert_eq!(reads(load, "generatorsConstructed", "generatorsLoaded"), 0);
        assert_eq!(reads(load, "generatorsLoaded", "generatorsSaved"), 1);
    }
    assert_eq!(reads(fresh, "generatorsConstructed", "generatorsSaved"), 1);
    for r in &records[1..records.len() - 1] {
        for key in ["shared", "math", "shuffle", "seeded", "threadLocal"] {
            assert_eq!(
                r["generatorsBefore"][key], r["generatorsSaved"][key],
                "{key}"
            );
        }
    }
    let count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter = count.clone();
    let clock: Arc<Clock> = Arc::new(move || {
        counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        1_689_163_200_000
    });
    let mut s = scenario();
    let c = canonical(
        "traceone",
        Arc::new(Registry::default()),
        &s.world,
        clock.clone(),
        s.start.clone(),
        s.ticks,
    );
    assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 1);
    let mut rest = Rest {
        hooks: hooks(fresh),
        ..Default::default()
    };
    c.save(&*clock, &mut s.world, &mut rest);
    assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 2);
}

/// Each world-trace character file loaded into a new canonical player and
/// saved again, against the original loading the same file into a new Player
/// and saving it (and against the file itself).
#[test]
fn world_trace_character_files_load_into_the_canonical_player() {
    let records = records();
    let mut files = Vec::new();
    for line in TRACE.lines() {
        let r: Value = serde_json::from_str(line).unwrap();
        for f in r["files"].as_array().into_iter().flatten() {
            if let Some(hex) = f["hex"].as_str() {
                let name = f["path"]
                    .as_str()
                    .unwrap()
                    .trim_start_matches("data/players/")
                    .trim_end_matches(".json")
                    .to_owned();
                files.push((name, hex.to_owned()));
            }
        }
    }
    assert_eq!(files.len(), 4);
    let mut summary = Vec::new();
    for (name, hex) in &files {
        let record = records
            .iter()
            .find(|r| r["t"] == "load" && r["file"] == json!(hex) && r["name"] == json!(name))
            .unwrap();
        let bytes: Vec<u8> = (0..hex.len() / 2)
            .map(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap())
            .collect();
        let text = String::from_utf8(bytes).unwrap();
        let mut s = scenario();
        let mut c = canonical(
            name,
            Arc::new(Registry::default()),
            &s.world,
            s.clock.clone(),
            s.start.clone(),
            s.ticks,
        );
        let node = c.player.owner().node;
        let mut rest = Rest {
            hooks: hooks(record),
            ..Default::default()
        };
        c.load(&text, &*s.clock, &mut s.world, &mut rest).unwrap();
        let native = observe(&c, node);
        let mut differences = compare_dump(&native, &record["loaded"], name);
        // Entity.location after the load is the actor's business at login;
        // the load returns it and the actor stays where State.add put it.
        differences.retain(|d| !d.contains("Node.location"));
        assert_eq!(
            c.location.as_ref().map(Location::coordinates),
            Some([3094, 3107, 0])
        );
        assert!(differences.is_empty(), "{differences:#?}");
        let written = c.save(&*s.clock, &mut s.world, &mut rest);
        let original = record["populate"]["text"].as_str().unwrap();
        summary.push((
            name.clone(),
            differing_keys(&written, original),
            differing_keys(&written, &text),
        ));
        assert_eq!(written.as_bytes(), original.as_bytes(), "{name}");
        assert_eq!(written.as_bytes(), text.as_bytes(), "{name}");
        if written != original && std::env::var_os("RS09_CANONICAL_VERBOSE").is_some() {
            eprintln!("{name}: {}", parting(&written, original));
        }
    }
    // Byte for byte against the original's save after the same load, and
    // against the file itself.
    for (name, against_original_save, against_file) in &summary {
        assert!(
            against_original_save.is_empty(),
            "{name}: {against_original_save:?}"
        );
        assert!(against_file.is_empty(), "{name}: {against_file:?}");
    }
}

/// Every whole file of the player-save corpus that the original wrote,
/// loaded into a new canonical player and saved again. The run prints each
/// category of difference; the categories are pinned below and explained in
/// docs/canonical-player.md.
#[test]
fn player_save_corpus_whole_files_round_trip_through_the_canonical_player() {
    let files = whole_files();
    let total = files.len();
    let mut matched = 0;
    let mut categories: std::collections::BTreeMap<String, Vec<usize>> = Default::default();
    for file in files {
        let (index, _label, outcome) = round_trip(file);
        match outcome {
            Ok(keys) if keys.is_empty() => matched += 1,
            Ok(keys) => categories.entry(keys.join("+")).or_default().push(index),
            Err(error) => categories
                .entry(format!("load failed: {error}"))
                .or_default()
                .push(index),
        }
    }
    let summary: Vec<String> = categories
        .iter()
        .map(|(k, v)| format!("{k}: {} files, cases {:?}", v.len(), v))
        .collect();
    eprintln!(
        "{matched} of {total} whole files byte for byte; differing:\n{}",
        summary.join("\n")
    );
    assert_eq!(
        (matched, total),
        EXPECTED_ROUND_TRIPS,
        "{}",
        summary.join("\n")
    );
    let keys: Vec<&str> = categories.keys().map(String::as_str).collect();
    assert_eq!(keys, EXPECTED_CATEGORIES);
}

/// The profile is bound once, is taken out (never shared) by an operation,
/// and holds no node handle: a disconnected player with a bound profile is
/// collected like one without.
#[test]
fn profile_binding_is_once_and_adds_no_ownership_edge() {
    use rs09_server::player::owned::profile::Error;
    let s = scenario();
    let mut c = canonical(
        "owner",
        Arc::new(Registry::default()),
        &s.world,
        s.clock.clone(),
        s.start.clone(),
        s.ticks,
    );
    let factory = Factory::new(&Registry::default());
    let again = Profile::new(
        PlayerDetails::new("owner", s.clock.clone()),
        Fresh {
            world: &s.world,
            ticks: 0,
        },
        &factory,
    )
    .unwrap();
    assert!(matches!(
        c.player.bind_profile(again),
        Err(Error::Binding(_))
    ));
    let nested = c
        .player
        .with_profile(|_| c.player.with_profile(|_| ()).err())
        .unwrap();
    assert!(matches!(nested, Some(Error::InUse)));
    assert!(c.player.with_profile(|p| p.name.clone()).unwrap() == "owner");
    let node = c.player.owner().node;
    let weak = c.player.downgrade();
    c.state.actor_mut(node).unwrap().connected = false;
    let Canonical {
        mut state,
        player,
        _owner,
        ..
    } = c;
    drop(player);
    drop(_owner);
    state.collect();
    assert!(state.actor(node).is_err());
    assert!(weak.upgrade().is_none());
}
/// Whole files that come back byte for byte, of all whole files.
const EXPECTED_ROUND_TRIPS: (usize, usize) = (38, 237);
/// The top-level keys that differ, per category; see docs/canonical-player.md.
const EXPECTED_CATEGORIES: &[&str] = &[
    "achievementDiaries+attributes+houseData+pohstorage+pouches",
    "achievementDiaries+core_data+familiarManager+houseData+pohstorage+pouches+quests",
    "achievementDiaries+core_data+houseData+pohstorage+pouches",
    "achievementDiaries+familiarManager+houseData+pohstorage+pouches+quests",
    "achievementDiaries+houseData+pohstorage+pouches",
    "emoteData+quests",
    "load failed: Index",
    "load failed: Null",
    "load failed: Number",
    "load failed: Type",
    "quests",
];

/// The strongest corpus evidence: in the persisted-state cases the original
/// wrote a file, constructed a new Player, loaded the file and wrote it
/// again. The canonical player loads the same file and must write what the
/// original wrote after its load.
#[test]
fn files_the_original_reloaded_into_a_new_player_reload_identically() {
    let files = whole_files();
    let mut pairs = 0;
    let mut failures = Vec::new();
    for (index, line) in SAVE_CORPUS.lines().enumerate() {
        let case: Value = serde_json::from_str(line).unwrap();
        if case["state"] != true {
            continue;
        }
        let actions = case["actions"].as_array().unwrap();
        let expected = case["expected"].as_array().unwrap();
        let mut last_file: Option<String> = None;
        let mut fresh = false;
        let mut source: Option<String> = None;
        for (a, e) in actions.iter().zip(expected) {
            let op = a["op"].as_str().unwrap();
            match op {
                "newPlayer" => {
                    fresh = true;
                    source = None;
                }
                "profileParse" if fresh && e["error"].is_null() => source = last_file.clone(),
                "contentHooks" => {}
                "populate" | "profileSave" if e["error"].is_null() => {
                    let key = if op == "populate" { "text" } else { "file" };
                    let Some(written) = e[key].as_str() else {
                        continue;
                    };
                    if let Some(input) = source.take() {
                        let template = files.iter().find(|f| f.index == index).unwrap();
                        let mut world = World::default();
                        world.quests = template.world.quests.clone();
                        world.songs = template.world.songs.clone();
                        world.spells = template.world.spells.clone();
                        let file = WholeFile {
                            index,
                            label: template.label.clone(),
                            name: template.name.clone(),
                            clock: template.clock,
                            world,
                            definitions: clone_registry(&template.definitions),
                            hooks: template.hooks.clone(),
                            text: input,
                        };
                        pairs += 1;
                        let reloaded = reload(file);
                        if reloaded.as_deref() != Ok(written) {
                            failures.push(format!(
                                "case {index}: {:?}",
                                reloaded.map(|r| differing_keys(&r, written))
                            ));
                        }
                    }
                    if op == "profileSave" {
                        last_file = Some(written.to_owned());
                    }
                    fresh = false;
                }
                _ => {
                    fresh = false;
                    source = None;
                }
            }
        }
    }
    eprintln!("{pairs} original reloads compared");
    assert!(failures.is_empty(), "{failures:#?}");
    assert_eq!(pairs, EXPECTED_RELOADS);
}
fn clone_registry(r: &Registry) -> Registry {
    let mut out = Registry::default();
    for (id, d) in r.definitions() {
        out.insert(*id, d.clone());
    }
    out
}
fn reload(file: WholeFile) -> Result<String, String> {
    let WholeFile {
        name,
        clock,
        mut world,
        definitions,
        hooks,
        text,
        ..
    } = file;
    let clock = fixed_clock(clock);
    let mut p = canonical(
        &name,
        Arc::new(definitions),
        &world,
        clock.clone(),
        Location::new(3222, 3218, 0),
        0,
    );
    let mut rest = Rest {
        hooks,
        ..Default::default()
    };
    p.load(&text, &*clock, &mut world, &mut rest)?;
    Ok(p.save(&*clock, &mut world, &mut rest))
}
const EXPECTED_RELOADS: usize = 34;
/// The canonical player's `core_data` (its own containers, bank tab arrays,
/// selector, Entity location and the variable store, projected for the
/// save) against the plain core_data library loading and saving the same
/// file: for every whole file of the corpus that loads, the section is
/// identical, so the projection adds nothing of its own.
#[test]
fn canonical_core_data_matches_the_plain_library_for_every_corpus_file() {
    use rs09_server::player::core_data::DEFAULT_BANK_SIZE;
    let mut compared = 0;
    for file in whole_files() {
        let definitions = clone_registry(&file.definitions);
        let index = file.index;
        let text = file.text.clone();
        let Ok(canonical_text) = reload(file) else {
            continue;
        };
        let root = java_json::parse(&text).unwrap();
        let factory = Factory::new(&definitions);
        let listeners = [0, 1, 2, 3].map(|_| Handle::new(Listener::new(None)));
        let mut core = CoreData::new(
            &factory,
            DEFAULT_BANK_SIZE,
            listeners,
            Some(Location::new(3222, 3218, 0)),
        )
        .unwrap();
        let mut manager = timers::Manager::default();
        core.parse(Some(&root), &definitions, &mut manager, &mut BaseTimers)
            .unwrap();
        let mut out = Json::object();
        core.save(&mut out, &mut manager, &mut BaseTimers).unwrap();
        let plain = out.get("core_data").unwrap().text();
        let canonical = java_json::parse(&canonical_text)
            .unwrap()
            .get("core_data")
            .unwrap()
            .text();
        assert_eq!(canonical, plain, "case {index}");
        compared += 1;
    }
    eprintln!("{compared} core_data sections compared");
    assert_eq!(compared, EXPECTED_CORE_COMPARISONS);
}
const EXPECTED_CORE_COMPARISONS: usize = 229;
