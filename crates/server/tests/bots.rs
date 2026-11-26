//! Replays the artificial player engine corpus (fixtures/bots) through
//! `world::bots`. Every service answers from the case the way the oracle's
//! recording stand-ins do (oracle/bots/adapters.py, R.java and
//! Support.java); every recorded call, result, failure class and the
//! snapshot after each operation (the repository's player list, the
//! engine's statics, every bot, script and script pulse, the disconnection
//! queue) are compared.
use rs09_game_core::java_json;
use rs09_server::auth::Clock;
use rs09_server::definitions::item::{Registry, Shared};
use rs09_server::inventory::{
    container::{ContainerType, Factory, SortType},
    container_listener::Handle,
    listener::Listener,
    player_binding::Capture,
};
use rs09_server::player::{
    details::PlayerDetails,
    owned::{
        Player as CanonicalPlayer,
        profile::{Fresh, Profile},
    },
    persisted::host::World as Registries,
};
use rs09_server::world::{
    appearance, doors,
    interfaces::Manager,
    player_render,
    player_walk::{NodeHandle, State},
    prepared_masks::PreparedMasks,
};
use rs09_server::world::{
    bots::{
        self, AiPlayer, BotClass, BotOffer, Dest, DestinationFlag, Direction, Error, Finder, Food,
        Gender, Handler, IntType, Item, JavaHashMap, Location, NodeKind, PlayerScript, Pulse,
        Script, ScriptApi, ScriptPulse, Statics, Used, Which, ai_player, assemblers,
        assemblers::{Tier, Type, Wealth},
        builder, creator, repository as ai_repository,
        script_api::{Accept, BuyTask},
    },
    disconnection, repository,
    repository::Repository,
};
use serde_json::{Map, Value as Json, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;

/// The canonical player a bot's `Player` object is here: the profile holds
/// its name and its `artificial` and `playing` flags (bots::canonical).
struct Canonical {
    player: CanonicalPlayer,
    _state: State,
    _owner: NodeHandle,
}

/// `new Player(new PlayerDetails(name))` as the canonical player's own test
/// constructs one: an actor, its containers and masks, and a fresh profile.
fn canonical(name: &str) -> Canonical {
    let definitions = Arc::new(Registry::default());
    let factory = Factory::new(&definitions);
    let location = doors::Location::new(2524, 5002, 0);
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
        Shared::new(player_render::Player::new(1, location, 2000)),
        Shared::new(appearance::Data::naked(name.into(), 0)),
    );
    let player = CanonicalPlayer::new(
        state.identity(),
        core,
        definitions,
        Manager::default(),
        Shared::new(false),
        masks,
    )
    .unwrap();
    state.bind_player_fields(player.clone()).unwrap();
    let clock: Arc<Clock> = Arc::new(|| 0);
    let details = PlayerDetails::new(name, clock);
    let world = Registries::default();
    let profile = Profile::new(
        details,
        Fresh {
            world: &world,
            ticks: 0,
        },
        &factory,
    )
    .unwrap();
    player.bind_profile(profile).unwrap();
    Canonical {
        player,
        _state: state,
        _owner: owner,
    }
}

const CORPUS: &str = include_str!("../../../fixtures/bots/oracle.jsonl");
const PROVENANCE: &str = include_str!("../../../fixtures/bots/provenance.json");

type N = String;
type R<T> = bots::Result<T>;

#[derive(Clone, Debug)]
struct NodeState {
    kind: NodeKind,
    id: i32,
    name: Option<String>,
    location: Option<Location>,
    active: bool,
    direction: Option<Direction>,
    index: i32,
    artificial: bool,
    playing: bool,
    dropper: Option<N>,
}

fn int(v: &Json) -> i32 {
    v.as_i64().unwrap_or_else(|| panic!("int {v}")) as i32
}
fn loc_of(v: &Json) -> Option<Location> {
    v.as_array()
        .map(|a| Location::new(int(&a[0]), int(&a[1]), int(&a[2])))
}
fn jloc(l: Option<Location>) -> Json {
    l.map_or(Json::Null, |l| json!({ "loc": [l.x(), l.y(), l.z()] }))
}
fn coords(l: Option<Location>) -> Json {
    l.map_or(Json::Null, |l| json!([l.x(), l.y(), l.z()]))
}
fn bits(d: f64) -> Json {
    json!({ "double": d.to_bits() as i64 })
}
fn jitem(i: Option<Item>) -> Json {
    i.map_or(Json::Null, |i| json!({ "item": [i.id, i.amount] }))
}
fn item_of(v: &Json) -> Option<Item> {
    let a = v.as_array()?;
    Some(Item::new(int(&a[0]), a.get(1).map_or(1, int)))
}
fn strings(v: &Json) -> Vec<String> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|s| s.as_str().unwrap().to_owned())
        .collect()
}
fn ints(v: &Json) -> Vec<i32> {
    v.as_array().unwrap().iter().map(int).collect()
}
fn scalar(v: &Json) -> Option<String> {
    match v {
        Json::String(s) => Some(s.clone()),
        Json::Number(n) => Some(n.to_string()),
        Json::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}
fn to_repo(e: Error) -> repository::Error {
    match e {
        Error::Null => repository::Error::NullPointer,
        Error::Index => repository::Error::IndexOutOfBounds,
        Error::NoSuchElement => repository::Error::NoSuchElement,
        Error::Argument => repository::Error::IllegalArgument,
        Error::Binding(text) => repository::Error::Exception(text),
        other => panic!("{other:?} inside Player.clear"),
    }
}

struct World {
    input: Json,
    nodes: BTreeMap<N, NodeState>,
    bots: Vec<N>,
    canonical: Vec<(N, Canonical)>,
    ais: Vec<(N, AiPlayer<World>)>,
    statics: Statics<World>,
    repository: Repository<N>,
    answers: Map<String, Json>,
    answer_calls: HashMap<String, usize>,
    counts: HashMap<String, i64>,
    events: Vec<Json>,
    calls: i64,
    fail_at: i64,
    pulses: Vec<(String, Pulse<World>)>,
    labelled: i32,
    last_pulse: Option<String>,
    stopped: HashSet<String>,
    paths: i32,
    launched: Vec<BuyTask<World>>,
    scripts: BTreeMap<String, Script<World>>,
    states: HashMap<String, String>,
    pests: i32,
    expected_events: Vec<Json>,
    kotlin_seen: usize,
}

impl World {
    fn new(case: &Json) -> Self {
        let config = &case["config"];
        let capacity = config["capacity"].as_i64().unwrap_or(2000) as i32;
        let mut w = Self {
            input: case.clone(),
            nodes: BTreeMap::new(),
            bots: Vec::new(),
            canonical: Vec::new(),
            ais: Vec::new(),
            statics: Statics::new(),
            repository: Repository::new(capacity, 1).unwrap(),
            answers: case["answers"].as_object().cloned().unwrap_or_default(),
            answer_calls: HashMap::new(),
            counts: HashMap::new(),
            events: Vec::new(),
            calls: 0,
            fail_at: 0,
            pulses: Vec::new(),
            labelled: 0,
            last_pulse: None,
            stopped: HashSet::new(),
            paths: 0,
            launched: Vec::new(),
            scripts: BTreeMap::new(),
            states: HashMap::new(),
            pests: 0,
            expected_events: Vec::new(),
            kotlin_seen: 0,
        };
        if let Some(nodes) = case["nodes"].as_object() {
            for (key, spec) in nodes {
                let kind = match spec["kind"].as_str().unwrap() {
                    "player" => NodeKind::Player,
                    "npc" => NodeKind::Npc,
                    "scenery" => NodeKind::Scenery,
                    "ground" | "item" => NodeKind::Item,
                    other => panic!("{other}"),
                };
                let id = spec["id"].as_i64().unwrap_or(0) as i32;
                let name = match kind {
                    // An item's name is its definition's.
                    NodeKind::Item => Some(w.item_name_of(id & 0xFFFF)),
                    _ => spec["name"].as_str().map(str::to_owned),
                };
                w.nodes.insert(
                    key.clone(),
                    NodeState {
                        kind,
                        id: if kind == NodeKind::Item {
                            id & 0xFFFF
                        } else {
                            id
                        },
                        name,
                        location: loc_of(&spec["loc"]),
                        active: spec["active"].as_bool().unwrap_or(true),
                        direction: spec["direction"].as_str().and_then(Direction::from_name),
                        index: if kind == NodeKind::Item { -1 } else { 0 },
                        artificial: false,
                        playing: false,
                        dropper: spec["dropper"].as_str().map(str::to_owned),
                    },
                );
            }
        }
        if let Some(specs) = case["scripts"].as_object() {
            for (key, spec) in specs {
                let class = spec["class"].as_str().unwrap_or("CaseScript").to_owned();
                let script = Script::new(class);
                script.write(|s| {
                    for i in spec["inventory"].as_array().into_iter().flatten() {
                        s.inventory.push(item_of(i).unwrap());
                    }
                    for i in spec["equipment"].as_array().into_iter().flatten() {
                        s.equipment.push(item_of(i).unwrap());
                    }
                    for p in spec["skills"].as_array().into_iter().flatten() {
                        s.skills.put(int(&p[0]), int(&p[1]));
                    }
                    for q in spec["quests"].as_array().into_iter().flatten() {
                        s.quests.push(q.as_str().unwrap().to_owned());
                    }
                    if let Some(running) = spec["running"].as_bool() {
                        s.running = running;
                    }
                    if let Some(end) = spec["endDialogue"].as_bool() {
                        s.end_dialogue = end;
                    }
                    s.bot = spec["bot"].as_str().map(str::to_owned);
                });
                if class_is(&script, "Adventurer") {
                    w.states.insert(
                        key.clone(),
                        spec["state"].as_str().unwrap_or("EXPLORE").to_owned(),
                    );
                }
                w.scripts.insert(key.clone(), script);
            }
        }
        w
    }

    /// The canonical player of a bot; a case's own players have none.
    fn canonical_of(&self, key: &N) -> Option<&CanonicalPlayer> {
        self.canonical
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, c)| &c.player)
    }
    fn name_of(&self, key: &N) -> Option<String> {
        match self.canonical_of(key) {
            Some(player) => Some(bots::canonical::name(player).unwrap()),
            None => self.node(key).name.clone(),
        }
    }
    fn artificial(&self, key: &N) -> bool {
        match self.canonical_of(key) {
            Some(player) => bots::canonical::is_artificial(player).unwrap(),
            None => self.node(key).artificial,
        }
    }
    fn playing(&self, key: &N) -> bool {
        match self.canonical_of(key) {
            Some(player) => bots::canonical::is_playing(player).unwrap(),
            None => self.node(key).playing,
        }
    }
    fn set_playing_of(&mut self, key: &N, playing: bool) {
        match self.canonical_of(key) {
            Some(player) => bots::canonical::set_playing(player, playing).unwrap(),
            None => self.nodes.get_mut(key).unwrap().playing = playing,
        }
    }
    fn node(&self, key: &N) -> &NodeState {
        self.nodes.get(key).unwrap_or_else(|| panic!("node {key}"))
    }
    fn item_name_of(&self, id: i32) -> String {
        self.input["items"][id.to_string()]["name"]
            .as_str()
            .map_or_else(|| format!("Item{id}"), str::to_owned)
    }
    fn script_label(&self, script: &Script<World>) -> String {
        self.scripts
            .iter()
            .find(|(_, s)| s.same(script))
            .map(|(k, _)| k.clone())
            .expect("a case script")
    }
    fn ai(&self, key: &N) -> AiPlayer<World> {
        self.ais
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, a)| a.clone())
            .unwrap_or_else(|| panic!("no AIPlayer {key}"))
    }

    /// The label of a pulse: a script pulse keeps the one it was first
    /// seen with; any other pulse object is new each time it is handed over.
    fn label_pulse(&mut self, pulse: &Pulse<World>) -> String {
        if let Pulse::Script(script) = pulse {
            for (label, held) in &self.pulses {
                if matches!(held, Pulse::Script(other) if other.same(script)) {
                    return label.clone();
                }
            }
        }
        self.labelled += 1;
        let label = format!("pulse#{}", self.labelled);
        self.pulses.push((label.clone(), pulse.clone()));
        label
    }
    fn pulse_by(&self, key: &str) -> Pulse<World> {
        let key = if key == "last" {
            self.last_pulse.clone().expect("a pulse")
        } else {
            key.to_owned()
        };
        self.pulses
            .iter()
            .find(|(label, _)| *label == key)
            .map(|(_, p)| p.clone())
            .unwrap_or_else(|| panic!("no pulse {key}"))
    }

    // ------------------------------------------------------------ recording
    fn q(&mut self, name: &str, args: Vec<Json>) -> R<Json> {
        let mut row = vec![json!(name)];
        row.extend(args);
        self.events.push(Json::Array(row.clone()));
        if name.starts_with('@') {
            return Ok(Json::Null);
        }
        self.calls += 1;
        let index = self.calls;
        let count = {
            let c = self.counts.entry(name.to_owned()).or_insert(0);
            *c += 1;
            *c
        };
        let hooks = self.input["hooks"].as_array().cloned().unwrap_or_default();
        for hook in hooks {
            if hook["event"] == name && count == hook["at"].as_i64().unwrap_or(1) {
                for action in hook["actions"].as_array().unwrap() {
                    op(self, action)?;
                }
            }
        }
        if index == self.fail_at {
            return Err(Error::Binding("injected".into()));
        }
        let a0 = row.get(1).and_then(scalar);
        let a1 = row.get(2).and_then(scalar);
        let keys = [
            match (&a0, &a1) {
                (Some(a0), Some(a1)) => Some(format!("{name}@{a0}#{a1}")),
                _ => None,
            },
            a0.as_ref().map(|a0| format!("{name}@{a0}")),
            Some(name.to_owned()),
        ];
        let mut answer = Json::Null;
        for key in keys.into_iter().flatten() {
            let Some(found) = self.answers.get(&key).cloned() else {
                continue;
            };
            answer = match found {
                Json::Array(list) if list.is_empty() => Json::Null,
                Json::Array(list) => {
                    let calls = self.answer_calls.entry(key).or_insert(0);
                    *calls += 1;
                    list[(*calls - 1).min(list.len() - 1)].clone()
                }
                other => other,
            };
            break;
        }
        if let Some(kind) = answer.get("throw").and_then(Json::as_str) {
            return Err(match kind {
                "Null" => Error::Null,
                "Cast" => Error::Cast,
                "Index" => Error::Index,
                other => panic!("{other}"),
            });
        }
        Ok(answer)
    }
    fn qv(&mut self, name: &str, args: Vec<Json>) -> R<()> {
        self.q(name, args).map(|_| ())
    }
    fn qi(&mut self, fallback: i32, name: &str, args: Vec<Json>) -> R<i32> {
        Ok(self.q(name, args)?.as_i64().map_or(fallback, |v| v as i32))
    }
    fn qb(&mut self, fallback: bool, name: &str, args: Vec<Json>) -> R<bool> {
        Ok(self.q(name, args)?.as_bool().unwrap_or(fallback))
    }
    fn qn(&mut self, name: &str, args: Vec<Json>) -> R<Option<N>> {
        Ok(self.q(name, args)?.as_str().map(str::to_owned))
    }
    fn qlist(&mut self, name: &str, args: Vec<Json>) -> R<Vec<Option<N>>> {
        Ok(self
            .q(name, args)?
            .as_array()
            .map(|l| l.iter().map(|k| k.as_str().map(str::to_owned)).collect())
            .unwrap_or_default())
    }
    fn container(player: &N, which: Which) -> Json {
        json!(format!("{player}.{}", which.name()))
    }
    fn used(&self, used: &Used<N>) -> Json {
        match used {
            Used::Item(item) => jitem(*item),
            Used::Node(node) => json!(node),
        }
    }
    fn hand_over(
        &mut self,
        name: &str,
        owner: Option<&N>,
        pulse: Pulse<World>,
        rest: Vec<Json>,
    ) -> R<()> {
        // A MovementPulse records its construction, which comes first.
        if let Some((mover, destination, flag)) = pulse.movement() {
            let destination = match destination {
                Dest::Node(node) => json!(node),
                Dest::Location(location) => jloc(Some(location)),
            };
            self.qv(
                "MovementPulse.new",
                vec![
                    json!(mover),
                    destination,
                    json!(flag.map(DestinationFlag::name)),
                ],
            )?;
        }
        let label = self.label_pulse(&pulse);
        self.last_pulse = Some(label.clone());
        let mut args = Vec::new();
        if let Some(owner) = owner {
            args.push(json!(owner));
        }
        args.extend([
            json!(label),
            json!(pulse.delay()),
            json!(pulse.checks()),
            json!(pulse.class_name()),
        ]);
        args.extend(rest);
        self.qv(name, args)
    }
}

fn class_is(script: &Script<World>, name: &str) -> bool {
    script.class() == name
}

impl bots::Host for World {
    type Node = N;
    type Path = (String, bool);
    type Region = i32;
    type PulseRef = String;
    type Consumable = String;
    type Quest = String;
    type QuestObject = String;
    type InteractOption = String;
    type ScriptClass = String;
    type ActiveScript = bool;
    type PestControlBot = ();

    fn statics(&mut self) -> Statics<Self> {
        self.statics.clone()
    }
    fn repository(&mut self) -> Repository<N> {
        self.repository.clone()
    }
    fn location(&mut self, node: &N) -> Option<Location> {
        self.node(node).location
    }
    fn kind(&mut self, node: &N) -> NodeKind {
        self.node(node).kind
    }
    fn id(&mut self, node: &N) -> i32 {
        self.node(node).id
    }
    fn name(&mut self, node: &N) -> Option<String> {
        self.name_of(node)
    }
    fn is_active(&mut self, node: &N) -> bool {
        self.node(node).active
    }
    fn direction(&mut self, node: &N) -> Option<Direction> {
        self.node(node).direction
    }
    fn set_index(&mut self, node: &N, index: i32) {
        self.nodes.get_mut(node).unwrap().index = index;
    }
    fn set_artificial(&mut self, player: &N, artificial: bool) {
        let canonical = self.canonical_of(player).expect("an artificial player");
        bots::canonical::set_artificial(canonical, artificial).unwrap();
    }
    fn ai_player(&mut self, node: &N) -> Option<AiPlayer<Self>> {
        self.ais
            .iter()
            .find(|(k, _)| k == node)
            .map(|(_, a)| a.clone())
    }
    fn attach_ai_player(&mut self, node: &N, ai: &AiPlayer<Self>) {
        self.ais.push((node.clone(), ai.clone()));
    }
    fn bot_data_path(&mut self) -> Option<String> {
        (self.input["config"]["dataPath"] != json!(false)).then(|| "data/".to_owned())
    }
    fn read_file(&mut self, path: &str) -> Option<String> {
        self.input["files"][path.strip_prefix("data/")?]
            .as_str()
            .map(str::to_owned)
    }
    fn max_pathfind_distance(&mut self) -> i32 {
        self.input["config"]["maxPathfind"].as_i64().unwrap_or(25) as i32
    }
    fn server_ge_name(&mut self) -> String {
        self.input["config"]["geName"]
            .as_str()
            .unwrap_or("Bot Exchange")
            .to_owned()
    }
    fn server_running(&mut self) -> bool {
        self.input["config"]["serverRunning"] != json!(false)
    }
    fn ge_announcement_limit(&mut self) -> Option<i32> {
        match self.input["config"].get("geLimit") {
            None => Some(500),
            Some(v) => v.as_i64().map(|v| v as i32),
        }
    }
    fn item_name(&mut self, id: i32) -> Option<String> {
        Some(self.item_name_of(id))
    }
    fn dropper(&mut self, ground: &N) -> Option<N> {
        self.node(ground).dropper.clone()
    }
    fn option_name(&mut self, option: &String) -> Option<String> {
        (option != "?").then(|| option.clone())
    }
    fn is_movement_pulse(&mut self, pulse: &String) -> bool {
        pulse == "foreign:movement"
            || self
                .pulses
                .iter()
                .any(|(label, p)| label == pulse && p.movement().is_some())
    }
    fn path_is_move_near(&mut self, path: &(String, bool)) -> bool {
        path.1
    }
    fn script_simple_name(&mut self, class: &String) -> String {
        class.clone()
    }
    fn script_is_idler(&mut self, class: &String) -> bool {
        class == "Idler"
    }
    fn adventurer_state(&mut self, script: &Script<Self>) -> Option<String> {
        let label = self.script_label(script);
        self.states.get(&label).cloned()
    }

    fn random(&mut self, max: i32) -> R<i32> {
        self.qi(0, "RandomFunction.random", vec![json!(max)])
    }
    fn random_between(&mut self, a: i32, b: i32) -> R<i32> {
        self.qi(0, "RandomFunction.random2", vec![json!(a), json!(b)])
    }
    fn random_double(&mut self, a: f64, b: f64) -> R<f64> {
        Ok(self
            .q("RandomFunction.randomDouble", vec![bits(a), bits(b)])?
            .as_f64()
            .unwrap_or(0.0))
    }
    fn get_random(&mut self, max: i32) -> R<i32> {
        self.qi(0, "RandomFunction.getRandom", vec![json!(max)])
    }
    fn roll(&mut self, chance: i32) -> R<bool> {
        self.qb(false, "RandomFunction.roll", vec![json!(chance)])
    }
    fn linear_decrease_rand(&mut self, max: i32) -> R<i32> {
        self.qi(0, "RandomFunction.linearDecreaseRand", vec![json!(max)])
    }
    fn normal_rand_dist(&mut self, max: i32) -> R<i32> {
        self.qi(0, "RandomFunction.normalRandDist", vec![json!(max)])
    }
    fn new_random_next_int(&mut self, bound: i32) -> R<i32> {
        self.qi(0, "Random.nextInt", vec![json!(bound)])
    }
    fn new_random_int(&mut self) -> R<i32> {
        self.qi(0, "Random.nextInt0", vec![])
    }
    fn kotlin_next_int(&mut self, from: i32, until: i32) -> R<i32> {
        // The original's generator cannot be replaced: the draw is the one
        // the oracle read off the original's result.
        let row = self
            .expected_events
            .iter()
            .filter(|e| e[0] == "@kotlin.Random.nextInt")
            .nth(self.kotlin_seen)
            .cloned()
            .expect("a recorded draw");
        self.kotlin_seen += 1;
        self.events
            .push(json!(["@kotlin.Random.nextInt", from, until, row[3]]));
        Ok(row[3].as_i64().unwrap_or(0) as i32)
    }
    fn kotlin_random_index(&mut self, size: i32) -> R<i32> {
        self.qi(0, "kotlin.Random.index", vec![json!(size)])
    }

    fn new_player(&mut self, username: &str) -> R<N> {
        self.qv("Player.new", vec![json!(username)])?;
        let key = format!("bot#{}", self.bots.len() + 1);
        self.nodes.insert(
            key.clone(),
            NodeState {
                kind: NodeKind::Player,
                id: 0,
                name: Some(username.to_owned()),
                // ServerConstants.START_LOCATION, until setLocation.
                location: Some(Location::new(2524, 5002, 0)),
                active: true,
                direction: None,
                index: 0,
                artificial: false,
                playing: false,
                dropper: None,
            },
        );
        self.bots.push(key.clone());
        self.canonical.push((key.clone(), canonical(username)));
        Ok(key)
    }
    fn set_location(&mut self, node: &N, location: Option<Location>) -> R<()> {
        self.qv("Node.setLocation", vec![json!(node), jloc(location)])?;
        self.nodes.get_mut(node).unwrap().location = location;
        Ok(())
    }
    fn details_set_session(&mut self, player: &N) -> R<()> {
        self.qv(
            "PlayerDetails.setSession",
            vec![json!(player), json!("ArtificialSession")],
        )
    }
    fn set_direction(&mut self, node: &N, direction: Direction) -> R<()> {
        self.qv(
            "Node.setDirection",
            vec![json!(node), json!(direction.name())],
        )?;
        self.nodes.get_mut(node).unwrap().direction = Some(direction);
        Ok(())
    }
    fn player_init(&mut self, player: &N) -> R<()> {
        self.qv("Player.init", vec![json!(player)])?;
        // The stand-in's init leaves the player playing, as the login
        // configuration it stands for does.
        self.set_playing_of(player, true);
        Ok(())
    }
    fn player_tick(&mut self, player: &N) -> R<()> {
        self.qv("Player.tick", vec![json!(player)])
    }
    fn player_reset(&mut self, player: &N) -> R<()> {
        self.qv("Player.reset", vec![json!(player)])
    }
    fn player_finalize_death(&mut self, player: &N, killer: Option<&N>) -> R<()> {
        self.qv("Player.finalizeDeath", vec![json!(player), json!(killer)])
    }
    fn full_restore(&mut self, player: &N) -> R<()> {
        self.qv("Player.fullRestore", vec![json!(player)])
    }
    fn face(&mut self, entity: &N, target: &N) -> R<()> {
        self.qv("Entity.face", vec![json!(entity), json!(target)])
    }
    fn face_location(&mut self, entity: &N, location: Option<Location>) -> R<()> {
        self.qv("Entity.faceLocation", vec![json!(entity), jloc(location)])
    }
    fn in_combat(&mut self, entity: &N) -> R<bool> {
        self.qb(false, "Entity.inCombat", vec![json!(entity)])
    }
    fn attack(&mut self, entity: &N, target: &N) -> R<()> {
        self.qv("Entity.attack", vec![json!(entity), json!(target)])
    }
    fn is_tele_blocked(&mut self, entity: &N) -> R<bool> {
        self.qb(false, "Entity.isTeleBlocked", vec![json!(entity)])
    }
    fn lock(&mut self, entity: &N) -> R<()> {
        self.qv("Entity.lock", vec![json!(entity)])
    }
    fn lock_for(&mut self, entity: &N, ticks: i32) -> R<()> {
        self.qv("Entity.lockFor", vec![json!(entity), json!(ticks)])
    }
    fn unlock(&mut self, entity: &N) -> R<()> {
        self.qv("Entity.unlock", vec![json!(entity)])
    }
    fn visualize(&mut self, entity: &N, animation: i32, graphics: i32) -> R<()> {
        self.qv(
            "Entity.visualize",
            vec![json!(entity), json!(animation), json!(graphics)],
        )
    }
    fn send_chat(&mut self, entity: &N, message: &str) -> R<()> {
        self.qv("Entity.sendChat", vec![json!(entity), json!(message)])
    }
    fn register_chat_mask(&mut self, player: &N, message: &str) -> R<()> {
        self.qv(
            "UpdateMasks.register",
            vec![json!(player), json!("Chat"), json!(message)],
        )
    }
    fn has_modal_open(&mut self, player: &N) -> R<bool> {
        self.qb(false, "Player.hasModalOpen", vec![json!(player)])
    }
    fn debug(&mut self, player: &N, text: &str) -> R<()> {
        self.qv("Player.debug", vec![json!(player), json!(text)])
    }
    fn set_attribute_true(&mut self, entity: &N, key: &str) -> R<()> {
        self.qv(
            "Entity.setAttribute",
            vec![json!(entity), json!(key), json!(true)],
        )
    }
    fn set_script_attribute(&mut self, player: &N, pulse: &ScriptPulse<Self>) -> R<()> {
        let label = self.label_pulse(&Pulse::Script(pulse.clone()));
        self.qv(
            "Entity.setAttribute",
            vec![json!(player), json!("botting:script"), json!(label)],
        )
    }
    fn botting_drops(&mut self, player: &N) -> R<Option<Vec<N>>> {
        let answer = self.q(
            "Entity.getAttribute",
            vec![json!(player), json!("botting:drops")],
        )?;
        Ok(answer.as_array().map(|_| strings(&answer)))
    }
    fn set_botting_drops(&mut self, player: &N, items: &[N]) -> R<()> {
        self.qv(
            "Entity.setAttribute",
            vec![json!(player), json!("botting:drops"), json!(items)],
        )
    }
    fn animator_reset(&mut self, entity: &N) -> R<()> {
        self.qv("Animator.reset", vec![json!(entity)])
    }
    fn impact_set_disabled_ticks(&mut self, entity: &N, ticks: i32) -> R<()> {
        self.qv(
            "ImpactHandler.setDisabledTicks",
            vec![json!(entity), json!(ticks)],
        )
    }
    fn walking_queue_is_moving(&mut self, entity: &N) -> R<bool> {
        self.qb(false, "WalkingQueue.isMoving", vec![json!(entity)])
    }

    fn appearance_set_gender(&mut self, player: &N, gender: Gender) -> R<()> {
        self.qv(
            "Appearance.setGender",
            vec![json!(player), json!(gender.name())],
        )
    }
    fn appearance_sync(&mut self, player: &N) -> R<()> {
        self.qv("Appearance.sync", vec![json!(player)])
    }
    fn appearance_copy(&mut self, player: &N, from: &N) -> R<()> {
        self.qv("Appearance.copy", vec![json!(player), json!(from)])
    }
    fn appearance_parse(&mut self, player: &N, json: &java_json::Json) -> R<()> {
        self.qv("Appearance.parse", vec![json!(player), json!(json.text())])
    }
    fn character_design_randomize(&mut self, player: &N, head: bool) -> R<()> {
        self.qv(
            "CharacterDesign.randomize",
            vec![json!(player), json!(head)],
        )
    }
    fn settings_set_run_toggled(&mut self, player: &N, enabled: bool) -> R<()> {
        self.qv(
            "Settings.setRunToggled",
            vec![json!(player), json!(enabled)],
        )
    }
    fn flags_is_update_scene_graph(&mut self, player: &N) -> R<bool> {
        self.qb(false, "PlayerFlags.isUpdateSceneGraph", vec![json!(player)])
    }
    fn flags_set_last_scene_graph(&mut self, player: &N, location: Option<Location>) -> R<()> {
        self.qv(
            "PlayerFlags.setLastSceneGraph",
            vec![json!(player), jloc(location)],
        )
    }
    fn interface_open_default_tabs(&mut self, player: &N) -> R<()> {
        self.qv("InterfaceManager.openDefaultTabs", vec![json!(player)])
    }
    fn interface_close_chatbox(&mut self, player: &N) -> R<()> {
        self.qv("InterfaceManager.closeChatbox", vec![json!(player)])
    }
    fn interface_open_chatbox(&mut self, player: &N, id: i32) -> R<()> {
        self.qv(
            "InterfaceManager.openChatbox",
            vec![json!(player), json!(id)],
        )
    }
    fn interface_close(&mut self, player: &N) -> R<()> {
        self.qv("InterfaceManager.close", vec![json!(player)])
    }
    fn interface_open_overlay(&mut self, player: &N, id: i32) -> R<()> {
        self.qv(
            "InterfaceManager.openOverlay",
            vec![json!(player), json!(id)],
        )
    }
    fn dialogue_close(&mut self, player: &N) -> R<()> {
        self.qv("DialogueInterpreter.close", vec![json!(player)])
    }
    fn send_interface_config(
        &mut self,
        player: &N,
        interface: i32,
        child: i32,
        hide: bool,
    ) -> R<()> {
        self.qv(
            "PacketDispatch.sendInterfaceConfig",
            vec![json!(player), json!(interface), json!(child), json!(hide)],
        )
    }
    fn send_string(&mut self, player: &N, text: &str, interface: i32, child: i32) -> R<()> {
        self.qv(
            "PacketDispatch.sendString",
            vec![json!(player), json!(text), json!(interface), json!(child)],
        )
    }

    fn properties_set_spawn_location(&mut self, entity: &N, location: Option<Location>) -> R<()> {
        self.qv(
            "Properties.setSpawnLocation",
            vec![json!(entity), jloc(location)],
        )
    }
    fn properties_set_teleport_location(&mut self, entity: &N, location: Location) -> R<()> {
        self.qv(
            "Properties.setTeleportLocation",
            vec![json!(entity), jloc(Some(location))],
        )
    }
    fn properties_is_multi_zone(&mut self, entity: &N) -> R<bool> {
        self.qb(false, "Properties.isMultiZone", vec![json!(entity)])
    }
    fn combat_victim(&mut self, entity: &N) -> R<Option<N>> {
        self.qn("CombatPulse.getVictim", vec![json!(entity)])
    }
    fn combat_delay_next_attack(&mut self, entity: &N, ticks: i32) -> R<()> {
        self.qv(
            "CombatPulse.delayNextAttack",
            vec![json!(entity), json!(ticks)],
        )
    }
    fn skull_is_wilderness(&mut self, player: &N) -> R<bool> {
        self.qb(false, "SkullManager.isWilderness", vec![json!(player)])
    }
    fn skull_set_level(&mut self, player: &N, level: i32) -> R<()> {
        self.qv("SkullManager.setLevel", vec![json!(player), json!(level)])
    }
    fn wilderness_level(&mut self, entity: &N) -> R<i32> {
        self.qi(0, "WildernessZone.getWilderness", vec![json!(entity)])
    }
    fn prayer_toggle(&mut self, player: &N, prayer: Option<&str>) -> R<()> {
        self.qv("Prayer.toggle", vec![json!(player), json!(prayer)])
    }
    fn npc_has_action(&mut self, npc: &N, action: &str) -> R<bool> {
        self.qb(
            false,
            "NPCDefinition.hasAction",
            vec![json!(npc), json!(action)],
        )
    }

    fn skills_set_static_level(&mut self, entity: &N, skill: i32, level: i32) -> R<()> {
        self.qv(
            "Skills.setStaticLevel",
            vec![json!(entity), json!(skill), json!(level)],
        )
    }
    fn skills_set_level(&mut self, entity: &N, skill: i32, level: i32) -> R<()> {
        self.qv(
            "Skills.setLevel",
            vec![json!(entity), json!(skill), json!(level)],
        )
    }
    fn skills_static_level(&mut self, entity: &N, skill: i32) -> R<i32> {
        self.qi(
            1,
            "Skills.getStaticLevel",
            vec![json!(entity), json!(skill)],
        )
    }
    fn skills_level(&mut self, entity: &N, skill: i32) -> R<i32> {
        self.qi(1, "Skills.getLevel", vec![json!(entity), json!(skill)])
    }
    fn skills_experience(&mut self, entity: &N, skill: i32) -> R<f64> {
        Ok(self
            .q("Skills.getExperience", vec![json!(entity), json!(skill)])?
            .as_f64()
            .unwrap_or(0.0))
    }
    fn skills_add_experience(&mut self, entity: &N, skill: i32, experience: f64) -> R<()> {
        self.qv(
            "Skills.addExperience",
            vec![json!(entity), json!(skill), bits(experience)],
        )
    }
    fn skills_level_from_xp(&mut self, entity: &N, experience: f64) -> R<i32> {
        self.qi(
            0,
            "Skills.levelFromXP",
            vec![json!(entity), bits(experience)],
        )
    }
    fn skills_lifepoints(&mut self, entity: &N) -> R<i32> {
        self.qi(10, "Skills.getLifepoints", vec![json!(entity)])
    }
    fn skills_set_lifepoints(&mut self, entity: &N, lifepoints: i32) -> R<()> {
        self.qv(
            "Skills.setLifepoints",
            vec![json!(entity), json!(lifepoints)],
        )
    }
    fn skills_update_combat_level(&mut self, entity: &N) -> R<()> {
        self.qv("Skills.updateCombatLevel", vec![json!(entity)])
    }
    fn skills_highest_combat_skill(&mut self, entity: &N) -> R<i32> {
        self.qi(0, "Skills.getHighestCombatSkillId", vec![json!(entity)])
    }
    fn skills_copy(&mut self, entity: &N, from: &N) -> R<()> {
        self.qv("Skills.copy", vec![json!(entity), json!(from)])
    }

    fn container_get(&mut self, player: &N, which: Which, slot: i32) -> R<Option<Item>> {
        let c = Self::container(player, which);
        Ok(item_of(&self.q("Container.get", vec![c, json!(slot)])?))
    }
    fn container_add(&mut self, player: &N, which: Which, item: Item) -> R<bool> {
        let c = Self::container(player, which);
        self.qb(true, "Container.add", vec![c, jitem(Some(item))])
    }
    fn equipment_add(
        &mut self,
        player: &N,
        item: Item,
        fire: bool,
        from_inventory: bool,
    ) -> R<bool> {
        let c = Self::container(player, Which::Equipment);
        self.qb(
            true,
            "EquipmentContainer.add3",
            vec![c, jitem(Some(item)), json!(fire), json!(from_inventory)],
        )
    }
    fn equipment_add_slot(
        &mut self,
        player: &N,
        item: Item,
        slot: i32,
        fire: bool,
        from_inventory: bool,
    ) -> R<bool> {
        let c = Self::container(player, Which::Equipment);
        self.qb(
            true,
            "EquipmentContainer.add4",
            vec![
                c,
                jitem(Some(item)),
                json!(slot),
                json!(fire),
                json!(from_inventory),
            ],
        )
    }
    fn container_replace(&mut self, player: &N, which: Which, item: Item, slot: i32) -> R<()> {
        let c = Self::container(player, which);
        self.qv("Container.replace", vec![c, jitem(Some(item)), json!(slot)])
    }
    fn container_remove(&mut self, player: &N, which: Which, item: Item) -> R<bool> {
        let c = Self::container(player, which);
        self.qb(true, "Container.remove", vec![c, jitem(Some(item))])
    }
    fn container_clear(&mut self, player: &N, which: Which) -> R<()> {
        let c = Self::container(player, which);
        self.qv("Container.clear", vec![c])
    }
    fn container_refresh(&mut self, player: &N, which: Which) -> R<()> {
        let c = Self::container(player, which);
        self.qv("Container.refresh", vec![c])
    }
    fn container_to_array(&mut self, player: &N, which: Which) -> R<Vec<Option<Item>>> {
        let c = Self::container(player, which);
        Ok(self
            .q("Container.toArray", vec![c])?
            .as_array()
            .map(|l| l.iter().map(item_of).collect())
            .unwrap_or_default())
    }
    fn container_capacity(&mut self, player: &N, which: Which) -> R<i32> {
        let c = Self::container(player, which);
        self.qi(0, "Container.capacity", vec![c])
    }
    fn container_amount(&mut self, player: &N, which: Which, id: i32) -> R<i32> {
        let c = Self::container(player, which);
        self.qi(0, "Container.getAmount", vec![c, json!(id)])
    }
    fn container_amount_of(&mut self, player: &N, which: Which, item: Item) -> R<i32> {
        let c = Self::container(player, which);
        self.qi(0, "Container.getAmountItem", vec![c, jitem(Some(item))])
    }
    fn container_get_item(&mut self, player: &N, which: Which, item: Item) -> R<Option<Item>> {
        let c = Self::container(player, which);
        Ok(item_of(
            &self.q("Container.getItem", vec![c, jitem(Some(item))])?,
        ))
    }
    fn container_contains_item(&mut self, player: &N, which: Which, item: Item) -> R<bool> {
        let c = Self::container(player, which);
        self.qb(false, "Container.containsItem", vec![c, jitem(Some(item))])
    }
    fn container_contains(&mut self, player: &N, which: Which, id: i32, amount: i32) -> R<bool> {
        let c = Self::container(player, which);
        self.qb(
            false,
            "Container.contains",
            vec![c, json!(id), json!(amount)],
        )
    }
    fn container_has_space_for(&mut self, player: &N, which: Which, item: Item) -> R<bool> {
        let c = Self::container(player, which);
        self.qb(true, "Container.hasSpaceFor", vec![c, jitem(Some(item))])
    }
    fn container_maximum_add(&mut self, player: &N, which: Which, item: Item) -> R<i32> {
        let c = Self::container(player, which);
        self.qi(0, "Container.getMaximumAdd", vec![c, jitem(Some(item))])
    }
    fn container_copy(&mut self, player: &N, which: Which, from: &N) -> R<()> {
        let c = Self::container(player, which);
        self.qv("Container.copy", vec![c, Self::container(from, which)])
    }
    fn container_parse(&mut self, player: &N, which: Which, json: &java_json::Json) -> R<()> {
        let c = Self::container(player, which);
        self.qv("Container.parse", vec![c, json!(json.text())])
    }

    fn item_handler(&mut self, id: i32, key: &str) -> R<Option<Handler>> {
        self.qv("ItemDefinition.handlers.get", vec![json!(id), json!(key)])?;
        Ok(match &self.input["items"][id.to_string()][key] {
            Json::Null => None,
            Json::Number(n) => Some(Handler::Int(n.as_i64().unwrap() as i32)),
            Json::Array(pairs) => {
                // The original's java.util.HashMap, filled in the listed order.
                let mut map = JavaHashMap::new();
                for pair in pairs {
                    map.put(int(&pair[0]), int(&pair[1]));
                }
                Some(Handler::Requirements(
                    map.iter().map(|(k, v)| (*k, *v)).collect(),
                ))
            }
            _ => Some(Handler::Other),
        })
    }
    fn item_note_id(&mut self, id: i32) -> R<i32> {
        self.qi(-1, "ItemDefinition.getNoteId", vec![json!(id)])
    }
    fn item_alchemy_value(&mut self, id: i32) -> R<i32> {
        self.qi(
            0,
            "ItemDefinition.getAlchemyValue",
            vec![json!(id), json!(true)],
        )
    }
    fn item_is_tradeable(&mut self, id: i32) -> R<bool> {
        self.qb(true, "ItemDefinition.isTradeable", vec![json!(id)])
    }
    fn item_value(&mut self, id: i32) -> R<i32> {
        self.qi(0, "ItemDefinition.getValue", vec![json!(id)])
    }
    fn item_ge_price(&mut self, id: i32) -> R<Option<i32>> {
        Ok(self
            .q(
                "ItemDefinition.getConfiguration",
                vec![json!(id), json!("grand_exchange_price")],
            )?
            .as_i64()
            .map(|v| v as i32))
    }

    fn pulse_manager_run(&mut self, entity: &N, pulse: Pulse<Self>, standard: bool) -> R<()> {
        self.hand_over(
            "PulseManager.run",
            Some(entity),
            pulse,
            vec![json!(standard)],
        )
    }
    fn pulser_submit(&mut self, pulse: Pulse<Self>) -> R<()> {
        self.hand_over("Pulser.submit", None, pulse, vec![])
    }
    fn pulse_manager_has_pulse_running(&mut self, entity: &N) -> R<bool> {
        self.qb(false, "PulseManager.hasPulseRunning", vec![json!(entity)])
    }
    fn pulse_manager_current(&mut self, entity: &N) -> R<Option<String>> {
        self.qn("PulseManager.getCurrent", vec![json!(entity)])
    }
    fn pulse_manager_clear(&mut self, entity: &N) -> R<()> {
        self.qv("PulseManager.clear", vec![json!(entity)])
    }
    fn pulse_stop(&mut self, pulse: &String) -> R<()> {
        // `stop()` is virtual: a BotScriptPulse runs its own.
        let held = self
            .pulses
            .iter()
            .find(|(l, _)| l == pulse)
            .map(|(_, p)| p.clone());
        match held {
            Some(pulse @ Pulse::Script(_)) => pulse.stop(self),
            _ => {
                self.qv("Pulse.stop", vec![json!(pulse)])?;
                self.stopped.insert(pulse.clone());
                Ok(())
            }
        }
    }
    fn script_pulse_super_stop(&mut self, pulse: &ScriptPulse<Self>) -> R<()> {
        let label = self.label_pulse(&Pulse::Script(pulse.clone()));
        self.qv("Pulse.stop", vec![json!(label)])?;
        self.stopped.insert(label);
        Ok(())
    }
    fn pulse_class_name(&mut self, pulse: &String) -> R<String> {
        Ok(match pulse.as_str() {
            "foreign:movement" => "oracle.R$ForeignMovement".to_owned(),
            "foreign:plain" => "oracle.R$ForeignPulse".to_owned(),
            _ => self.pulse_by(pulse).class_name().to_owned(),
        })
    }
    fn pulse_text(&mut self, pulse: Option<&String>) -> R<String> {
        Ok(match pulse {
            None => "null".to_owned(),
            Some(label) if label.starts_with("foreign:") => label.clone(),
            Some(label) => panic!("the text of {label} is an identity hash"),
        })
    }
    fn active_script(&mut self, entity: &N) -> R<Option<bool>> {
        Ok(self
            .qn("ScriptProcessor.getActiveScript", vec![json!(entity)])?
            .map(|key| key != "active:null"))
    }
    fn active_script_execution_class(&mut self, script: &bool) -> R<Option<String>> {
        Ok(script.then(|| "core.game.interaction.Script$Execution".to_owned()))
    }
    fn scripts_delay(&mut self, entity: &N) -> R<i32> {
        self.qi(0, "ScriptProcessor.getDelay", vec![json!(entity)])
    }
    fn script_tick(&mut self, script: &Script<Self>) -> R<()> {
        let label = self.script_label(script);
        self.qv("Script.tick", vec![json!(label)])
    }
    fn script_new_instance(&mut self, script: &Script<Self>) -> R<Script<Self>> {
        let label = self.script_label(script);
        let answer = self.qn("Script.newInstance", vec![json!(label)])?;
        // A null instance fails BotScriptPulse's parameter check.
        Ok(self.scripts[&answer.ok_or(Error::Null)?].clone())
    }
    fn world_ticks(&mut self) -> R<i32> {
        self.qi(0, "getWorldTicks", vec![])
    }
    fn launch(&mut self, task: BuyTask<Self>) -> R<()> {
        self.launched.push(task);
        let label = format!("task#{}", self.launched.len());
        self.qv("GlobalScope.launch", vec![json!(label)])
    }

    fn quest(&mut self, player: &N, quest: &String) -> R<Option<String>> {
        self.qn(
            "QuestRepository.getQuest",
            vec![json!(player), json!(quest)],
        )
    }
    fn quest_set_stage(&mut self, player: &N, quest: Option<&String>, stage: i32) -> R<()> {
        self.qv(
            "QuestRepository.setStage",
            vec![json!(player), json!(quest), json!(stage)],
        )
    }

    fn pathfinder_find(
        &mut self,
        mover: &N,
        destination: &Dest<N>,
        finder: Finder,
    ) -> R<(String, bool)> {
        let mut args = vec![
            json!(mover),
            match destination {
                Dest::Node(node) => json!(node),
                Dest::Location(location) => jloc(Some(*location)),
            },
        ];
        if let Finder::Smart { near } = finder {
            args.extend([json!(near), json!("SMART")]);
        }
        let move_near = self.q("Pathfinder.find", args)? == json!(true);
        self.paths += 1;
        Ok((format!("path#{}", self.paths), move_near))
    }
    fn path_walk(&mut self, path: &(String, bool), entity: &N) -> R<()> {
        self.qv("Path.walk", vec![json!(path.0), json!(entity)])
    }
    fn local_npcs(&mut self, location: Option<Location>, radius: i32) -> R<Vec<N>> {
        Ok(self
            .qlist(
                "RegionManager.getLocalNPCs",
                vec![jloc(location), json!(radius)],
            )?
            .into_iter()
            .flatten()
            .collect())
    }
    fn region_object(&mut self, x: i32, y: i32, z: i32) -> R<Option<N>> {
        self.qn(
            "RegionManager.getObject",
            vec![json!(x), json!(y), json!(z)],
        )
    }
    fn region_for_id(&mut self, region_id: i32) -> R<i32> {
        self.qv("RegionManager.forId", vec![json!(region_id)])?;
        Ok(region_id)
    }
    fn region_object_list(&mut self, region: &i32, z: i32) -> R<Vec<Option<N>>> {
        self.qlist("Region.assembleObjectList", vec![json!(region), json!(z)])
    }
    fn region_node_list(&mut self, region: &i32, z: i32) -> R<Vec<Option<N>>> {
        self.qlist("Region.assembleNodeList", vec![json!(region), json!(z)])
    }
    fn ground_items_contain(&mut self, item: Option<&N>) -> R<bool> {
        self.qb(false, "GroundItemManager.contains", vec![json!(item)])
    }
    fn send_grand_exchange_news(&mut self, text: &str) -> R<()> {
        self.qv("Repository.sendGrandExchangeNews", vec![json!(text)])
    }
    fn ge_add_bot_offer(&mut self, id: i32, amount: i32) -> R<bool> {
        self.qb(
            false,
            "GrandExchange.addBotOffer",
            vec![json!(id), json!(amount)],
        )
    }
    fn log_warn(&mut self, message: &str) -> R<()> {
        self.qv("log", vec![json!("WARN"), json!(message)])
    }
    fn print_stack_trace(&mut self, class: &str) {
        self.q("@printStackTrace", vec![json!(class)]).unwrap();
    }

    fn interaction(&mut self, node: &N) -> R<bool> {
        self.qb(true, "Node.getInteraction", vec![json!(node)])
    }
    fn interaction_options(&mut self, node: &N) -> R<Vec<Option<String>>> {
        self.qlist("Interaction.getOptions", vec![json!(node)])
    }
    fn interaction_get(&mut self, node: &N, index: i32) -> R<Option<String>> {
        self.qn("Interaction.get", vec![json!(node), json!(index)])
    }
    fn interaction_handle(&mut self, node: &N, player: &N, option: Option<&String>) -> R<()> {
        self.qv(
            "Interaction.handle",
            vec![json!(node), json!(player), json!(option)],
        )
    }
    fn listeners_run(
        &mut self,
        id: i32,
        kind: IntType,
        option: &str,
        player: &N,
        node: &N,
    ) -> R<bool> {
        self.qb(
            false,
            "InteractionListeners.run",
            vec![
                json!(id),
                json!(kind.name()),
                json!(option),
                json!(player),
                json!(node),
            ],
        )
    }
    fn listeners_run_use(
        &mut self,
        used: Option<Item>,
        with: &N,
        kind: IntType,
        player: &N,
    ) -> R<bool> {
        self.qb(
            false,
            "InteractionListeners.runUse",
            vec![jitem(used), json!(with), json!(kind.name()), json!(player)],
        )
    }
    fn scenery_child(&mut self, scenery: &N, player: &N) -> R<Option<N>> {
        self.qn("Scenery.getChild", vec![json!(scenery), json!(player)])
    }
    fn plugin_interaction_handle(&mut self, player: &N, used: &Used<N>, with: &Used<N>) -> R<bool> {
        let (used, with) = (self.used(used), self.used(with));
        self.qb(
            false,
            "PluginInteractionManager.handle",
            vec![json!(player), used, with],
        )
    }
    fn use_with_run(&mut self, player: &N, used: &Used<N>, with: &Used<N>) -> R<()> {
        let (used, with) = (self.used(used), self.used(with));
        self.qv("UseWithHandler.run", vec![json!(player), used, with])
    }

    fn consumable(&mut self, id: i32) -> R<Option<String>> {
        self.qn("Consumables.getConsumableById", vec![json!(id)])
    }
    fn consume(&mut self, consumable: Food<String>, food: Option<Item>, player: &N) -> R<()> {
        let consumable = match consumable {
            Food::Known(label) => json!(label),
            Food::Plain { ids, heal } => json!({ "food": ids, "heal": heal }),
        };
        self.qv(
            "Consumable.consume",
            vec![consumable, jitem(food), json!(player)],
        )
    }
    fn new_pest_control_test_bot(&mut self, second: bool, location: Location) -> R<()> {
        self.qv(
            "PestControlTestBot.new",
            vec![json!(second), jloc(Some(location))],
        )
    }
}

/// What `Player.clear` reaches of the repository port's services. Anything
/// an artificial player's clear cannot reach is not answered.
impl repository::Host<N> for World {
    type Session = ();
    type Clan = ();
    type Location = ();

    fn uid(&mut self, _: &N) -> i32 {
        0
    }
    fn name(&mut self, node: &N) -> Option<String> {
        self.name_of(node)
    }
    fn details_username(&mut self, node: &N) -> Option<String> {
        self.node(node).name.clone()
    }
    fn set_index(&mut self, node: &N, index: i32) {
        self.nodes.get_mut(node).unwrap().index = index;
    }
    fn is_artificial(&mut self, player: &N) -> bool {
        self.artificial(player)
    }
    fn afk_logout(&mut self, _: &N) -> Option<bool> {
        None
    }
    fn set_playing(&mut self, player: &N, playing: bool) {
        self.set_playing_of(player, playing);
    }
    fn session(&mut self, _: &N) -> Option<()> {
        unreachable!()
    }
    fn ticks(&mut self) -> i32 {
        unreachable!()
    }
    fn global_chat_enabled(&mut self) -> bool {
        unreachable!()
    }
    fn to_lower_case(&mut self, _: &str) -> String {
        unreachable!()
    }
    fn session_active(&mut self, _: &()) -> bool {
        unreachable!()
    }
    fn set_session_active(&mut self, _: &(), _: bool) {
        unreachable!()
    }
    fn session_has_key(&mut self, _: &()) -> bool {
        unreachable!()
    }
    fn key_cancel(&mut self, _: &()) -> repository::Result<()> {
        unreachable!()
    }
    fn socket_close(&mut self, _: &()) -> repository::Result<()> {
        unreachable!()
    }
    fn session_object(&mut self, _: &()) -> repository::SessionObject<N> {
        unreachable!()
    }
    fn set_session_object(&mut self, _: &(), _: Option<N>) {
        unreachable!()
    }
    fn session_username(&mut self, _: &()) -> Option<String> {
        unreachable!()
    }
    fn player_clear(&mut self, _: &Repository<N>, _: &N) -> repository::Result<()> {
        unreachable!()
    }
    fn renderable_players_remove(&mut self, player: &N) -> repository::Result<bool> {
        self.qb(
            false,
            "UpdateSequence.renderablePlayers.remove",
            vec![json!(player)],
        )
        .map_err(to_repo)
    }
    fn in_combat(&mut self, _: &N) -> repository::Result<bool> {
        unreachable!()
    }
    fn lifepoints(&mut self, _: &N) -> repository::Result<i32> {
        unreachable!()
    }
    fn is_dead(&mut self, _: &N) -> repository::Result<bool> {
        unreachable!()
    }
    fn is_teleporting(&mut self, _: &N) -> repository::Result<bool> {
        unreachable!()
    }
    fn has_soft_script(&mut self, _: &N) -> repository::Result<bool> {
        unreachable!()
    }
    fn logout_packet(&mut self, _: &N) -> repository::Result<()> {
        unreachable!()
    }
    fn world_logout_listeners(&mut self, _: &N) -> repository::Result<()> {
        unreachable!()
    }
    fn walking_queue_reset(&mut self, player: &N) -> repository::Result<()> {
        self.qv("WalkingQueue.reset", vec![json!(player)])
            .map_err(to_repo)
    }
    fn player_logout_listeners(&mut self, _: &N) -> repository::Result<()> {
        // The map is empty: nothing is invoked.
        Ok(())
    }
    fn has_familiar(&mut self, player: &N) -> repository::Result<bool> {
        self.qb(false, "FamiliarManager.hasFamiliar", vec![json!(player)])
            .map_err(to_repo)
    }
    fn familiar_clear(&mut self, player: &N) -> repository::Result<()> {
        self.qv("Familiar.clear", vec![json!(player)])
            .map_err(to_repo)
    }
    fn interface_close(&mut self, player: &N) -> repository::Result<()> {
        self.qv("InterfaceManager.close", vec![json!(player)])
            .map_err(to_repo)
    }
    fn interface_close_single_tab(&mut self, player: &N) -> repository::Result<()> {
        self.qv("InterfaceManager.closeSingleTab", vec![json!(player)])
            .map_err(to_repo)
    }
    fn entity_clear(&mut self, player: &N) -> repository::Result<()> {
        self.qv("Entity.clear", vec![json!(player)])
            .map_err(to_repo)
    }
    fn zone_monitor_clear(&mut self, player: &N) -> repository::Result<()> {
        self.qv("ZoneMonitor.clear", vec![json!(player)])
            .map_err(to_repo)
    }
    fn house_leave(&mut self, player: &N) -> repository::Result<()> {
        self.qv("HouseManager.leave", vec![json!(player)])
            .map_err(to_repo)
    }
    fn publish_player_status(
        &mut self,
        username: &str,
        world: i32,
        only: bool,
    ) -> repository::Result<()> {
        self.qv(
            "ManagementEvents.publish",
            vec![json!("status"), json!(username), json!(world), json!(only)],
        )
        .map_err(to_repo)
    }
    fn clan(&mut self, player: &N) -> repository::Result<Option<()>> {
        Ok(self
            .q("CommunicationInfo.getClan", vec![json!(player)])
            .map_err(to_repo)?
            .as_str()
            .map(|_| ()))
    }
    fn world_id(&mut self) -> repository::Result<i32> {
        self.qi(0, "GameSettings.getWorldId", vec![])
            .map_err(to_repo)
    }
    fn clan_owner(&mut self, _: &()) -> repository::Result<Option<String>> {
        self.qn("ClanRepository.getOwner", vec![]).map_err(to_repo)
    }
    fn publish_clan_leave(
        &mut self,
        username: &str,
        world: i32,
        clan: &str,
    ) -> repository::Result<()> {
        self.qv(
            "ManagementEvents.publish",
            vec![
                json!("clanLeave"),
                json!(username),
                json!(world),
                json!(clan),
            ],
        )
        .map_err(to_repo)
    }
    fn wealth_update(&mut self, player: &N, force: bool) -> repository::Result<()> {
        self.qv(
            "Player.checkForWealthUpdate",
            vec![json!(player), json!(force)],
        )
        .map_err(to_repo)
    }
    fn clan_leave(&mut self, _: &(), _: &N, _: bool) -> repository::Result<()> {
        unreachable!()
    }
    fn details_save(&mut self, player: &N) -> repository::Result<()> {
        self.qv("PlayerDetails.save", vec![json!(player)])
            .map_err(to_repo)
    }
    fn execute_sql(&mut self, _: disconnection::SaveTask<N>) -> repository::Result<()> {
        unreachable!()
    }
    fn set_thread_name(&mut self, _: &str) {
        unreachable!()
    }
    fn save_immediately(&mut self, _: &N) -> repository::Result<()> {
        unreachable!()
    }
    fn log(&mut self, _: repository::LogLevel, _: &str) -> repository::Result<()> {
        // DisconnectionQueue's own log line; the stand-in queue has none.
        Ok(())
    }
    fn print_stack_trace(&mut self, _: &repository::Error) {
        unreachable!()
    }
    fn is_revenant(&mut self, _: &N) -> bool {
        unreachable!()
    }
    fn set_renderable(&mut self, _: &N, _: bool) {
        unreachable!()
    }
    fn npc_id(&mut self, _: &N) -> i32 {
        unreachable!()
    }
    fn region_chunk_npcs(&mut self, _: &()) -> repository::Result<Vec<N>> {
        unreachable!()
    }
    fn npc_at(&mut self, _: &N, _: &()) -> bool {
        unreachable!()
    }
    fn ge_news_muted(&mut self, _: &N) -> repository::Result<bool> {
        unreachable!()
    }
    fn send_message(&mut self, _: &N, _: &str) -> repository::Result<()> {
        unreachable!()
    }
}

// ---------------------------------------------------------------- snapshot
fn state(w: &mut World) -> Json {
    let statics = w.statics.clone();
    let players = w.repository.players();
    let (init, uid, names, line) = statics.read(|s| {
        (
            s.initialised,
            s.current_uid,
            s.bot_names.clone(),
            s.osrs_copy_line.clone(),
        )
    });
    let mut mapping: Vec<(i32, N)> =
        statics.read(|s| s.bot_mapping.iter().map(|(k, v)| (*k, v.node())).collect());
    mapping.sort();
    let session = statics.read(|s| s.session.clone()).object();
    let mut ground: Vec<(N, Vec<N>)> = statics.read(|s| s.ground_items.clone());
    ground.sort_by_key(|(k, _)| k.clone());
    let mut offers: Vec<Json> = statics.read(|s| {
        s.ge_offers
            .iter()
            .map(|(player, offer)| {
                let o = offer.get();
                json!([
                    player,
                    o.item_id,
                    o.sell,
                    o.offered_value,
                    o.amount,
                    o.completed_amount,
                    o.player
                ])
            })
            .collect()
    });
    offers.sort_by_key(|o| o[0].as_str().unwrap().to_owned());
    let held: Vec<(Option<String>, ScriptPulse<World>)> = statics.read(|s| {
        s.pulse_repository
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    });
    let pulses: Vec<Json> = held
        .iter()
        .map(|(name, pulse)| {
            let label = w.pulses.iter().find_map(|(label, held)| {
                matches!(held, Pulse::Script(other) if other.same(pulse)).then(|| label.clone())
            });
            let running = label.map_or(true, |l| !w.stopped.contains(&l));
            json!([
                name,
                [
                    w.script_label(&pulse.script()),
                    pulse.is_player(),
                    pulse.ticks(),
                    pulse.random_delay(),
                    coords(Some(pulse.last_bot_location())),
                    pulse.last_bot_move_ticks(),
                    running
                ]
            ])
        })
        .collect();
    let (triggered, spawned) = statics.read(|s| (s.bot_pulses_triggered_this_tick, s.bots_spawned));
    let bots: Vec<Json> = w
        .bots
        .clone()
        .iter()
        .map(|key| {
            let ai = w.ai(key);
            let node = w.node(key);
            let class = match ai.class() {
                BotClass::Plain => "Plain",
                BotClass::Combat => "Combat",
                BotClass::Pvm => "Pvm",
            };
            json!([
                key,
                class,
                ai.uid(),
                ai.username(),
                coords(ai.start_location()),
                ai.is_high_priority(),
                ai.controler(),
                w.artificial(key),
                node.index,
                coords(node.location),
                w.playing(key),
                ai.tick_counter()
            ])
        })
        .collect();
    let scripts: Vec<Json> = w
        .scripts
        .iter()
        .map(|(key, s)| {
            json!([
                key,
                s.bot(),
                s.running(),
                s.end_dialogue(),
                s.script_api().is_some()
            ])
        })
        .collect();
    let queue: Vec<Json> = w
        .repository
        .disconnection_queue()
        .entries()
        .into_iter()
        .map(|(name, player, clear)| json!([name, player, clear]))
        .collect();
    let identifiers: Vec<Json> = statics.read(|s| {
        s.identifier_map
            .iter()
            .map(|(k, p)| json!([k, p.identifier, p.description, p.name, p.clazz]))
            .collect()
    });
    json!({
        "players": { "size": players.size(), "slots": players.occupied() },
        "init": init,
        "uid": uid,
        "names": names,
        "mapping": mapping,
        "line": line,
        "session": session,
        "ground": ground,
        "offers": offers,
        "pulses": pulses,
        "triggered": triggered,
        "spawned": spawned,
        "bots": bots,
        "scripts": scripts,
        "queue": queue,
        "identifiers": identifiers,
    })
}

// ---------------------------------------------------------------- operations
fn s<'a>(op: &'a Json, key: &str) -> &'a str {
    op[key].as_str().unwrap_or_else(|| panic!("{key} of {op}"))
}
fn key(op: &Json, key: &str) -> N {
    s(op, key).to_owned()
}
fn opt_key(op: &Json, key: &str) -> Option<N> {
    op[key].as_str().map(str::to_owned)
}
fn flag(op: &Json, key: &str) -> bool {
    op[key] == json!(true)
}
fn tier(op: &Json) -> Tier {
    match s(op, "tier") {
        "LOW" => Tier::Low,
        "MED" => Tier::Med,
        "HIGH" => Tier::High,
        "PURE" => Tier::Pure,
        other => panic!("{other}"),
    }
}
fn wealth(op: &Json) -> Wealth {
    match s(op, "wealth") {
        "POOR" => Wealth::Poor,
        "AVERAGE" => Wealth::Average,
        "RICH" => Wealth::Rich,
        other => panic!("{other}"),
    }
}
fn bot_key(bot: R<AiPlayer<World>>) -> R<Json> {
    bot.map(|b| json!(b.node()))
}
fn done(result: R<()>) -> R<Json> {
    result.map(|()| Json::Null)
}

fn op(w: &mut World, op: &Json) -> R<Json> {
    let name = s(op, "op");
    let l = |k: &str| loc_of(&op[k]);
    // A Kotlin parameter of type Location: null fails on entry.
    let nn = |k: &str| loc_of(&op[k]).ok_or(Error::Null);
    let i = |k: &str| int(&op[k]);
    match name {
        // ---- setup, not calls
        "setLocation" => {
            w.nodes.get_mut(s(op, "node")).unwrap().location = l("loc");
            Ok(Json::Null)
        }
        "setActive" => {
            w.nodes.get_mut(s(op, "node")).unwrap().active = flag(op, "value");
            Ok(Json::Null)
        }
        "setOffer" => {
            let offer = ai_repository::get_offer(w, &key(op, "player")).ok_or(Error::Null)?;
            offer.update(|o| o.completed_amount = i("completed"));
            Ok(Json::Null)
        }
        "setScript" => {
            let script = w.scripts[s(op, "script")].clone();
            script.write(|state| {
                if op.get("bot").is_some() {
                    state.bot = opt_key(op, "bot");
                }
                if op.get("running").is_some() {
                    state.running = flag(op, "running");
                }
                if op.get("endDialogue").is_some() {
                    state.end_dialogue = flag(op, "endDialogue");
                }
            });
            Ok(Json::Null)
        }
        "setTriggered" => {
            w.statics
                .write(|state| state.bot_pulses_triggered_this_tick = i("value"));
            Ok(Json::Null)
        }
        "setPulseDelay" => {
            let Pulse::Script(pulse) = w.pulse_by(s(op, "pulse")) else {
                panic!("not a script pulse")
            };
            pulse.set_random_delay(i("value"));
            Ok(Json::Null)
        }
        // ---- AIPlayer
        "newAIPlayer" => bot_key(AiPlayer::new(w, l("loc"))),
        "newAIPlayerFile" => bot_key(AiPlayer::from_file(w, s(op, "file"), l("loc"))),
        "combatBot" => bot_key(builder::combat_bot(w, nn("loc")?)),
        "pvmBots" => bot_key(builder::pvm_bots(w, l("loc"))),
        "pvmBotsFile" => bot_key(builder::pvm_bots_from_file(w, s(op, "file"), l("loc"))),
        "loadNames" => {
            ai_player::ensure_initialised(w);
            ai_player::load_names(w, s(op, "file"));
            Ok(Json::Null)
        }
        "getRandomName" => ai_player::get_random_name(w).map(|n| json!(n)),
        "updateRandomOSRScopyLine" => {
            done(ai_player::update_random_osrs_copy_line(w, s(op, "file")))
        }
        "get" => Ok(json!(ai_player::get(w, i("uid")).map(|b| b.node()))),
        "deregister" => done(ai_player::deregister(w, i("uid"))),
        "tick" => done(w.ai(&key(op, "bot")).tick(w)),
        "update" => {
            w.ai(&key(op, "bot")).update();
            Ok(Json::Null)
        }
        "clear" => done(w.ai(&key(op, "bot")).clear(w)),
        "reset" => done(w.ai(&key(op, "bot")).reset(w)),
        "init" => done(w.ai(&key(op, "bot")).init(w)),
        "updateRandomValues" => done(w.ai(&key(op, "bot")).update_random_values(w)),
        "finalizeDeath" => {
            let killer = opt_key(op, "killer");
            done(w.ai(&key(op, "bot")).finalize_death(w, killer.as_ref()))
        }
        "follow" => done(w.ai(&key(op, "bot")).follow(w, &key(op, "entity"))),
        "randomWalkAroundPoint" => done(w.ai(&key(op, "bot")).random_walk_around_point(
            w,
            l("point"),
            i("radius"),
        )),
        "randomWalk" => done(w.ai(&key(op, "bot")).random_walk(w, i("x"), i("y"))),
        "walkToPosSmartXY" => done(
            w.ai(&key(op, "bot"))
                .walk_to_pos_smart_xy(w, i("x"), i("y")),
        ),
        "walkToPosSmart" => done(
            w.ai(&key(op, "bot"))
                .walk_to_pos_smart(w, l("loc").unwrap()),
        ),
        "walkPos" => done(w.ai(&key(op, "bot")).walk_pos(w, i("x"), i("y"))),
        "checkVictimIsPlayer" => w
            .ai(&key(op, "bot"))
            .check_victim_is_player(w)
            .map(|b| json!(b)),
        "getItemById" => w.ai(&key(op, "bot")).get_item_by_id(w, i("id")).map(jitem),
        "getClosestNodeWithEntry" => w
            .ai(&key(op, "bot"))
            .get_closest_node_with_entry(w, i("range"), i("entry"))
            .map(|n| json!(n)),
        "getClosestNodeWithEntries" => w
            .ai(&key(op, "bot"))
            .get_closest_node_with_entries(w, i("range"), &ints(&op["entries"]))
            .map(|n| json!(n)),
        "getClosestNodeWithEntryAndDirection" => w
            .ai(&key(op, "bot"))
            .get_closest_node_with_entry_and_direction(
                w,
                i("range"),
                i("entry"),
                Direction::from_name(s(op, "direction")).unwrap(),
            )
            .map(|n| json!(n)),
        "getClosesCreature" => w
            .ai(&key(op, "bot"))
            .get_closes_creature(w, i("radius"))
            .map(|n| json!(n)),
        "getClosesCreatureEntry" => w
            .ai(&key(op, "bot"))
            .get_closes_creature_with_entry(w, i("radius"), i("entry"))
            .map(|n| json!(n)),
        "getClosesCreatureEntries" => w
            .ai(&key(op, "bot"))
            .get_closes_creature_with_entries(w, i("radius"), &ints(&op["entries"]))
            .map(|n| json!(n)),
        "setHighPriority" => {
            w.ai(&key(op, "bot")).set_high_priority(flag(op, "value"));
            Ok(Json::Null)
        }
        "setControler" => {
            w.ai(&key(op, "bot")).set_controler(opt_key(op, "player"));
            Ok(Json::Null)
        }
        "interactNoop" => {
            let bot = w.ai(&key(op, "bot"));
            bot.interact();
            bot.handle_incoming_chat();
            Ok(Json::Null)
        }
        "setLevels" => done(w.ai(&key(op, "bot")).set_levels(w)),
        "giveArmor" => done(w.ai(&key(op, "bot")).give_armor(w)),
        "session" => {
            let session = w.statics.read(|state| state.session.clone());
            session.write();
            session.write();
            session.queue();
            session.flush();
            session.disconnect();
            Ok(json!(session.remote_address()))
        }
        // ---- AIRepository
        "addItem" => done(ai_repository::add_item(w, &key(op, "item"))),
        "getItems" => Ok(json!(ai_repository::get_items(w, &key(op, "player")))),
        "addOffer" => {
            let player = key(op, "player");
            let offer = BotOffer::new();
            offer.update(|o| {
                o.item_id = i("id");
                o.amount = i("amount");
                o.player = Some(player.clone());
            });
            ai_repository::add_offer(w, &player, &offer);
            Ok(Json::Null)
        }
        "getOffer" => Ok(ai_repository::get_offer(w, &key(op, "player"))
            .map_or(Json::Null, |o| json!([o.get().item_id, o.get().amount]))),
        "clearAllBots" => done(ai_repository::clear_all_bots(w)),
        "sendBotInfo" => {
            let bot = w.ai(&key(op, "bot"));
            done(ai_repository::send_bot_info(w, &key(op, "player"), &bot))
        }
        // ---- Script
        "scriptInit" => done(
            w.scripts[s(op, "script")]
                .clone()
                .init(w, flag(op, "isPlayer")),
        ),
        "scriptToString" => w.scripts[s(op, "script")].clone().text(w).map(|t| json!(t)),
        "scriptSetLevel" => done(w.scripts[s(op, "script")].clone().set_level(
            w,
            i("skill"),
            i("level"),
        )),
        // ---- GeneralBotCreator
        "gbcLoc" => {
            let script = w.scripts[s(op, "script")].clone();
            creator::create_at(w, &script, l("loc")).map(|_| Json::Null)
        }
        "gbcBot" => {
            let script = w.scripts[s(op, "script")].clone();
            let bot = opt_key(op, "bot").map(|k| w.ai(&k));
            creator::create_for_bot(w, &script, bot.as_ref()).map(|_| Json::Null)
        }
        "gbcPlayer" => {
            let script = w.scripts[s(op, "script")].clone();
            creator::create_for_player(w, &script, &key(op, "player"), flag(op, "isPlayer"))
                .map(|_| Json::Null)
        }
        "gbcTick" => {
            creator::gbc_tick(w);
            Ok(Json::Null)
        }
        "transitionNew" => {
            let script = w.scripts[s(op, "script")].clone();
            let pulse = creator::TransitionPulse::new(w, &script)?;
            let delay = pulse.delay;
            let label = w.label_pulse(&Pulse::Transition(pulse));
            Ok(json!([label, delay]))
        }
        "runPulse" => w.pulse_by(s(op, "pulse")).pulse(w).map(|b| json!(b)),
        "stopPulse" => {
            let label = if s(op, "pulse") == "last" {
                w.last_pulse.clone().unwrap()
            } else {
                key(op, "pulse")
            };
            done(bots::Host::pulse_stop(w, &label))
        }
        "runLaunched" => {
            let task = w.launched[i("task") as usize - 1].clone();
            task.run(w)?;
            assert!(task.resume(w)?, "the case's hook ran the pulse");
            Ok(Json::Null)
        }
        // ---- ScriptAPI
        "apiDistance" => ScriptApi::new(key(op, "api"))
            .distance(w, &key(op, "a"), &key(op, "b"))
            .map(bits),
        "interact" => done(ScriptApi::new(key(op, "api")).interact(
            w,
            &key(op, "player"),
            opt_key(op, "node").as_ref(),
            s(op, "option"),
        )),
        "useWith" => done(ScriptApi::new(key(op, "api")).use_with(
            w,
            &key(op, "player"),
            i("item"),
            opt_key(op, "node").as_ref(),
        )),
        "sendChat" => done(ScriptApi::new(key(op, "api")).send_chat(w, s(op, "message"))),
        "getNearestNodeFromList" => ScriptApi::new(key(op, "api"))
            .get_nearest_node_from_list(w, &strings(&op["names"]), flag(op, "isObject"))
            .map(|n| json!(n)),
        "getNearestNodeId" => ScriptApi::new(key(op, "api"))
            .get_nearest_node_by_id(w, i("id"), flag(op, "isObject"))
            .map(|n| json!(n)),
        // `getNearestNode(name, isObject)` ignores its flag.
        "getNearestNodeName" | "getNearestNodeNameObj" => ScriptApi::new(key(op, "api"))
            .get_nearest_node(w, s(op, "name"))
            .map(|n| json!(n)),
        "getNearestObjectByPredicate" => ScriptApi::new(key(op, "api"))
            .get_nearest_object_by_predicate(w, &mut |h: &mut World, n: &N| {
                h.qb(false, "Predicate.invoke", vec![json!(n)])
            })
            .map(|n| json!(n)),
        "evaluateViability" => {
            let names = op["names"].as_array().map(|_| strings(&op["names"]));
            let mut predicate =
                |h: &mut World, n: &N| h.qb(false, "Predicate.invoke", vec![json!(n)]);
            let mut accept = Accept {
                names: names.as_deref(),
                id: op["id"].as_i64().map_or(-1, |v| v as i32),
                predicate: if flag(op, "predicate") {
                    Some(&mut predicate)
                } else {
                    None
                },
            };
            ScriptApi::new(key(op, "api"))
                .evaluate_viability(
                    w,
                    opt_key(op, "node").as_ref(),
                    op["min"].as_f64().unwrap(),
                    op["max"].as_f64().unwrap(),
                    &mut accept,
                )
                .map(|b| json!(b))
        }
        "takeNearestGroundItem" => ScriptApi::new(key(op, "api"))
            .take_nearest_ground_item(w, i("id"))
            .map(|b| json!(b)),
        "getNearestGameObject" => ScriptApi::new(key(op, "api"))
            .get_nearest_game_object(w, l("loc").unwrap(), i("id"))
            .map(|n| json!(n)),
        "attackNpcsInRadius" => ScriptApi::new(key(op, "api"))
            .attack_npcs_in_radius(w, &key(op, "player"), None, i("radius"))
            .map(|b| json!(b)),
        "attackNpcInRadius" => ScriptApi::new(key(op, "api"))
            .attack_npcs_in_radius(w, &key(op, "player"), Some(s(op, "name")), i("radius"))
            .map(|b| json!(b)),
        "walkTo" => done(ScriptApi::new(key(op, "api")).walk_to(w, l("loc").unwrap())),
        "walkArray" => {
            let steps: Vec<Location> = op["steps"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| loc_of(v).unwrap())
                .collect();
            done(ScriptApi::new(key(op, "api")).walk_array(w, &steps))
        }
        "randomWalkTo" => {
            done(ScriptApi::new(key(op, "api")).random_walk_to(w, l("loc").unwrap(), i("radius")))
        }
        "randomizeLocationInRanges" => {
            let from = l("loc").unwrap();
            ScriptApi::new(key(op, "api"))
                .randomize_location_in_ranges(
                    w,
                    from,
                    i("xMin"),
                    i("xMax"),
                    i("yMin"),
                    i("yMax"),
                    i("z"),
                )
                .map(|to| json!([to.x() - from.x(), to.y() - from.y(), to.z()]))
        }
        "teleportToGE" => ScriptApi::new(key(op, "api"))
            .teleport_to_ge(w)
            .map(|b| json!(b)),
        "teleport" => ScriptApi::new(key(op, "api"))
            .teleport(w, l("loc").unwrap())
            .map(|b| json!(b)),
        "sellOnGE" => done(ScriptApi::new(key(op, "api")).sell_on_ge(w, i("id"))),
        "sellAllOnGe" => done(ScriptApi::new(key(op, "api")).sell_all_on_ge(w)),
        "sellAllOnGeAdv" => done(ScriptApi::new(key(op, "api")).sell_all_on_ge_adv(w)),
        "depositAtBank" => done(ScriptApi::new(key(op, "api")).deposit_at_bank(w)),
        "saleIsBigNews" => ScriptApi::new(key(op, "api"))
            .sale_is_big_news(w, i("id"), i("amount"))
            .map(|b| json!(b)),
        "bankItem" => done(ScriptApi::new(key(op, "api")).bank_item(w, i("item"))),
        "bankAll" => {
            let callback: Option<bots::script_api::OnComplete<World>> = flag(op, "onComplete")
                .then(|| {
                    std::sync::Arc::new(|h: &mut World| h.qv("OnComplete.invoke", vec![]))
                        as bots::script_api::OnComplete<World>
                });
            done(ScriptApi::new(key(op, "api")).bank_all(w, callback))
        }
        "eat" => done(ScriptApi::new(key(op, "api")).eat(w, i("food"))),
        "forceEat" => done(ScriptApi::new(key(op, "api")).force_eat(w, i("food"))),
        "buyFromGE" => done(ScriptApi::new(key(op, "api")).buy_from_ge(
            w,
            &key(op, "player"),
            i("id"),
            i("amount"),
        )),
        "withdraw" => done(ScriptApi::new(key(op, "api")).withdraw(w, i("id"), i("amount"))),
        "equipAndSetStatsList" => {
            let items: Option<Vec<Item>> = op["items"]
                .as_array()
                .map(|list| list.iter().map(|v| item_of(v).unwrap()).collect());
            done(ScriptApi::new(key(op, "api")).equip_and_set_stats_all(w, items.as_deref()))
        }
        "equipAndSetStats" => done(
            ScriptApi::new(key(op, "api")).equip_and_set_stats(w, item_of(&op["item"]).unwrap()),
        ),
        "loadAppearanceAndEquipment" => {
            let json = op["json"]
                .as_str()
                .map(|text| java_json::parse(text).unwrap());
            done(ScriptApi::new(key(op, "api")).load_appearance_and_equipment(w, json.as_ref()))
        }
        "overlay" => {
            let overlay = ScriptApi::<World>::new(key(op, "api")).get_overlay();
            overlay.init(w)?;
            overlay.set_title(w, s(op, "title"))?;
            overlay.set_task_label(w, s(op, "label"))?;
            done(overlay.set_amount(w, i("amount")))
        }
        "checkPriceOverrides" => ScriptApi::new(key(op, "api"))
            .check_price_overrides(w, i("id"))
            .map(|v| json!(v)),
        // ---- AIPBuilder, CombatBot, PvMBots, PvMBotsBuilder
        "aipCreate" => bot_key(builder::create(w, l("loc"))),
        "aipCopy" => bot_key(builder::copy(w, &key(op, "player"))),
        "aipCopyAt" => bot_key(builder::copy_at(w, &key(op, "player"), l("loc"))),
        "randomAfkPlayer" => done(builder::random_afk_player(w, l("loc"))),
        "immersiveSpawns" => done(builder::immersive_spawns(w)),
        "checkPrayer" => {
            let types: Vec<Option<&str>> = op["types"]
                .as_array()
                .unwrap()
                .iter()
                .map(Json::as_str)
                .collect();
            let bot = w.ai(&key(op, "bot"));
            done(builder::check_prayer(w, &bot, &types))
        }
        "eatBot" => {
            let bot = w.ai(&key(op, "bot"));
            done(builder::eat(w, &bot, i("food")))
        }
        "findTargets" => {
            builder::find_targets(w, &key(op, "entity"), i("radius")).map(|t| json!(t))
        }
        "checkValidTargets" => {
            builder::check_valid_targets(w, &key(op, "target")).map(|b| json!(b))
        }
        "attackNpcsInRadiusPvm" => {
            let who = opt_key(op, "player").unwrap_or_else(|| key(op, "bot"));
            builder::attack_npcs_in_radius(w, &who, i("radius")).map(|b| json!(b))
        }
        "pvmCreate" => bot_key(builder::pvm_create(w, l("loc"))),
        "pvmCreatePc" => {
            builder::pvm_create_pest_control_test_bot(w, flag(op, "second"), l("loc"))?;
            w.pests += 1;
            Ok(json!(format!("pest#{}", w.pests)))
        }
        // ---- CombatBotAssembler, SkillingBotAssembler
        "produce" => {
            let kind = match s(op, "type") {
                "RANGE" => Type::Range,
                "MAGE" => Type::Mage,
                "MELEE" => Type::Melee,
                other => panic!("{other}"),
            };
            bot_key(assemblers::produce(w, kind, tier(op), nn("loc")?))
        }
        "assembleRangedBot" => bot_key(assemblers::assemble_ranged_bot(
            w,
            tier(op),
            nn("loc")?,
            op["crossbow"].as_bool(),
        )),
        "assembleMeleeBot" => bot_key(assemblers::assemble_melee_bot(w, tier(op), nn("loc")?)),
        "meleeAdventurer" => bot_key(assemblers::melee_adventurer(w, tier(op), nn("loc")?)),
        "rangeAdventurer" => bot_key(assemblers::range_adventurer(w, tier(op), nn("loc")?)),
        "assembleMeleeDragonBot" => bot_key(assemblers::assemble_melee_dragon_bot(
            w,
            tier(op),
            nn("loc")?,
        )),
        "assembleRangeDragonBot" => bot_key(assemblers::assemble_range_dragon_bot(
            w,
            tier(op),
            nn("loc")?,
        )),
        "gearRangedBot" => {
            let bot = w.ai(&key(op, "bot"));
            done(assemblers::gear_ranged_bot(
                w,
                &bot,
                op["crossbow"].as_bool(),
            ))
        }
        "gearMeleeBot" => {
            let bot = w.ai(&key(op, "bot"));
            done(assemblers::gear_melee_bot(w, &bot))
        }
        "gearPCnRangedBot" => {
            let bot = w.ai(&key(op, "bot"));
            done(assemblers::gear_pc_n_ranged_bot(
                w,
                &bot,
                op["crossbow"].as_bool(),
            ))
        }
        "gearPCnMeleeBot" => {
            let bot = w.ai(&key(op, "bot"));
            done(assemblers::gear_pc_n_melee_bot(w, &bot))
        }
        "gearPCiRangedBot" => {
            let bot = w.ai(&key(op, "bot"));
            done(assemblers::gear_pc_i_ranged_bot(
                w,
                &bot,
                op["crossbow"].as_bool(),
            ))
        }
        "gearPCiMeleeBot" => {
            let bot = w.ai(&key(op, "bot"));
            done(assemblers::gear_pc_i_melee_bot(w, &bot))
        }
        "generateStats" => {
            let bot = w.ai(&key(op, "bot"));
            done(assemblers::generate_stats(
                w,
                &bot,
                tier(op),
                &ints(&op["skills"]),
            ))
        }
        "lvlAvg" => assemblers::lvl_avg(w, item_of(&op["item"]).unwrap()).map(|v| json!(v)),
        "skillingProduce" => bot_key(assemblers::skilling_produce(w, wealth(op), nn("loc")?)),
        "assembleBot" => {
            let bot = w.ai(&key(op, "bot"));
            assemblers::assemble_bot(w, &bot, wealth(op)).map(|()| json!(bot.node()))
        }
        "equipSet" => {
            let bot = w.ai(&key(op, "bot"));
            assemblers::equip_set(w, &bot, &ints(&op["set"])).map(|()| json!(bot.node()))
        }
        // ---- PlayerScripts
        "playerScript" => {
            let clazz = w.scripts[s(op, "script")].class();
            let entry = PlayerScript {
                identifier: key(op, "identifier"),
                description: strings(&op["description"]),
                name: key(op, "name"),
                clazz,
            };
            w.statics
                .write(|state| state.identifier_map.put(Some(key(op, "identifier")), entry));
            Ok(Json::Null)
        }
        other => panic!("unknown operation {other}"),
    }
}

fn run_case(case: &Json) -> Vec<String> {
    let mut w = World::new(case);
    let mut differences = Vec::new();
    for (number, (operation, expected)) in case["ops"]
        .as_array()
        .unwrap()
        .iter()
        .zip(case["expected"].as_array().unwrap())
        .enumerate()
    {
        w.events.clear();
        w.calls = 0;
        w.fail_at = operation["failAt"].as_i64().unwrap_or(0);
        w.expected_events = expected["events"].as_array().unwrap().clone();
        w.kotlin_seen = 0;
        let outcome = op(&mut w, operation);
        w.fail_at = 0;
        let (result, error) = match outcome {
            Ok(result) => (result, Json::Null),
            Err(error) => (Json::Null, json!(error.code())),
        };
        let native = json!({
            "events": w.events,
            "result": result,
            "error": error,
            "state": state(&mut w),
        });
        for field in ["error", "events", "result", "state"] {
            if native[field] != expected[field] {
                let (mine, theirs) = if field == "events" {
                    let (a, b) = (
                        native[field].as_array().unwrap(),
                        expected[field].as_array().unwrap(),
                    );
                    let at = a
                        .iter()
                        .zip(b)
                        .position(|(x, y)| x != y)
                        .unwrap_or(a.len().min(b.len()));
                    (
                        format!(
                            "#{at} {} (of {})",
                            a.get(at).unwrap_or(&Json::Null),
                            a.len()
                        ),
                        format!(
                            "#{at} {} (of {})",
                            b.get(at).unwrap_or(&Json::Null),
                            b.len()
                        ),
                    )
                } else if field == "state" {
                    let (a, b) = (
                        native[field].as_object().unwrap(),
                        expected[field].as_object().unwrap(),
                    );
                    let k = a
                        .keys()
                        .chain(b.keys())
                        .find(|k| a.get(*k) != b.get(*k))
                        .unwrap();
                    (
                        format!("{k}: {}", a.get(k).unwrap_or(&Json::Null)),
                        format!("{k}: {}", b.get(k).unwrap_or(&Json::Null)),
                    )
                } else {
                    (native[field].to_string(), expected[field].to_string())
                };
                differences.push(format!(
                    "{} op {number} ({}) {field}:\n  native   {mine}\n  original {theirs}",
                    case["name"], operation["op"]
                ));
                return differences;
            }
        }
    }
    differences
}

#[test]
fn bot_engine_corpus_replays() {
    let filter = std::env::var("RS09_BOTS_CASE").ok();
    let mut differences = Vec::new();
    let mut cases = 0;
    let mut operations = 0;
    for line in CORPUS.lines().filter(|l| !l.is_empty()) {
        let case: Json = serde_json::from_str(line).unwrap();
        if filter
            .as_ref()
            .is_some_and(|f| !case["name"].as_str().unwrap().contains(f.as_str()))
        {
            continue;
        }
        cases += 1;
        operations += case["ops"].as_array().unwrap().len();
        differences.extend(run_case(&case));
    }
    assert!(
        differences.is_empty(),
        "{} of {cases} cases differ:\n{}",
        differences.len(),
        differences
            .iter()
            .take(12)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
    println!("{cases} cases, {operations} operations replayed");
    if filter.is_none() {
        let provenance: Json = serde_json::from_str(PROVENANCE).unwrap();
        assert_eq!(provenance["caseCount"], json!(cases));
        assert_eq!(provenance["coverage"]["actions"], json!(operations));
    }
}

#[test]
fn fixture_matches_its_provenance() {
    let provenance: Json = serde_json::from_str(PROVENANCE).unwrap();
    let digest = Sha256::digest(CORPUS.as_bytes());
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(provenance["fixtureSha256"], json!(hex));
    assert_eq!(
        provenance["upstream"]["server"]["commit"],
        json!("9921b98d49308edd0489dd50ee98011d53914a57")
    );
}

/// The bot's `Player` is a canonical player: its name is the one the
/// constructor builds, and the `artificial` and `playing` flags the engine
/// and `Player.clear` write are the canonical profile's.
#[test]
fn an_artificial_player_is_a_canonical_player() {
    let case = json!({
        "name": "canonical", "group": "canonical", "files": { "botnames.txt": "the_OLD_nite\nbeta\n" },
        "ops": []
    });
    let mut w = World::new(&case);
    let bot = AiPlayer::new(&mut w, Some(Location::new(3222, 3218, 0))).unwrap();
    let node = bot.node();
    let player = w.canonical_of(&node).unwrap().clone();
    assert_eq!(
        bots::canonical::details_username(1, "the_OLD_nite"),
        "/aip2:the_OLD_nite"
    );
    assert_eq!(
        player.with_profile(|p| p.name.clone()).unwrap(),
        "/aip2:the_OLD_nite"
    );
    assert_eq!(bot.username().as_deref(), Some("The Old Nite2"));
    assert_eq!(bot.uid(), 1);
    assert!(player.with_profile(|p| p.flags.artificial).unwrap());
    assert!(player.with_profile(|p| p.flags.playing).unwrap());
    // Placed by NodeList.add: slot 1, and in neither map of the repository.
    assert_eq!(w.repository.players().occupied(), vec![(1, node.clone())]);
    assert!(w.repository.uid_entries().is_empty());
    assert!(w.repository.name_entries().is_empty());
    // AIPlayer.clear is Player.clear: for an artificial player finishClear
    // at once, which stops it playing; the queue is not involved.
    bot.clear(&mut w).unwrap();
    assert!(!player.with_profile(|p| p.flags.playing).unwrap());
    assert!(w.repository.disconnection_queue().is_empty());
    assert!(ai_player::get(&mut w, 1).is_none());
    // The slot is only given up by deregister, which no longer finds it.
    ai_player::deregister(&mut w, 1).unwrap();
    assert_eq!(w.repository.players().occupied(), vec![(1, node)]);
}
