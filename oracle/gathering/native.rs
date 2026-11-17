//! Native replay of the gathering corpus.
//!
//! Recorded service results are the tape: each native request must match the
//! next recorded [name, arguments] and receives the recorded result. Generator
//! draws are not taken from the tape: java.util.Random and ThreadLocalRandom
//! run natively from the case seeds and their draws are logged for comparison.
use rs09_game_core::{java_lower::lowercase_root, random::JavaRandom};
use rs09_server::world::gathering::{
    Settings,
    fishing::{self, Fish, FishingOption, FishingSpot},
    mining,
    nodes::{MiningNode, SkillingResource, WoodcuttingNode, respawn_duration},
    random::{self, ThreadLocalRandom},
    service::{Arg, Context, Error, Result, Service, Task},
    tools::{self, Tool},
    woodcutting,
};
use serde_json::{Value, json};

#[path = "binding_native.rs"]
pub mod binding;

fn bits(v: f64) -> String {
    format!("d:{:016x}", v.to_bits())
}
fn double(v: &Value) -> f64 {
    f64::from_bits(u64::from_str_radix(&v.as_str().unwrap()[2..], 16).unwrap())
}
fn to_json(a: &Arg) -> Value {
    match a {
        Arg::Null => Value::Null,
        Arg::Bool(v) => json!(v),
        Arg::Int(v) => json!(v),
        Arg::Double(v) => json!(bits(*v)),
        Arg::Text(v) => json!(v),
        Arg::List(v) => Value::Array(v.iter().map(to_json).collect()),
    }
}
fn from_json(v: &Value) -> Arg {
    match v {
        Value::Null => Arg::Null,
        Value::Bool(b) => Arg::Bool(*b),
        Value::Number(n) => Arg::Int(n.as_i64().unwrap()),
        Value::String(s) if s.len() == 18 && s.starts_with("d:") => Arg::Double(double(v)),
        Value::String(s) => Arg::Text(s.clone()),
        Value::Array(a) => Arg::List(a.iter().map(from_json).collect()),
        Value::Object(_) => panic!("object reply"),
    }
}
fn seed(case: &Value, key: &str, default: i64) -> i64 {
    match &case[key] {
        Value::Null => default,
        Value::String(s) => s.parse().unwrap(),
        v => v.as_i64().unwrap(),
    }
}

struct World {
    tape: Vec<Value>,
    log: Vec<Value>,
    shared: JavaRandom,
    math: JavaRandom,
    shuffle: JavaRandom,
    thread_local: ThreadLocalRandom,
    announced: i64,
    scripts: Vec<Task>,
    pulses: Vec<Task>,
    diverged: bool,
}
impl World {
    fn new(case: &Value) -> Self {
        let thread_local = ThreadLocalRandom {
            seed: seed(case, "tlrSeed", 4),
        };
        Self {
            tape: Vec::new(),
            log: Vec::new(),
            shared: JavaRandom::new(seed(case, "sharedSeed", 1)),
            math: JavaRandom::new(seed(case, "mathSeed", 2)),
            shuffle: JavaRandom::new(seed(case, "shuffleSeed", 3)),
            thread_local,
            announced: thread_local.seed,
            scripts: Vec::new(),
            pulses: Vec::new(),
            diverged: false,
        }
    }
    fn begin(&mut self, tape: &Value) {
        self.tape = tape.as_array().cloned().unwrap_or_default();
        self.log.clear();
        self.diverged = false;
    }
    fn record(&mut self, name: &str, args: Value, result: Value) {
        let mut event = vec![json!(name), args, result];
        if self.thread_local.seed != self.announced {
            self.announced = self.thread_local.seed;
            event.push(json!(self.announced.to_string()));
        }
        self.log.push(Value::Array(event));
    }
    /// Answers a recorded service from the tape, or fails on divergence.
    fn recorded(&mut self, name: &str, args: Value) -> Result<Arg> {
        let index = self.log.len();
        let entry = self.tape.get(index).cloned().unwrap_or(Value::Null);
        if self.diverged || entry[0] != name || entry[1] != args {
            self.diverged = true;
            self.record(name, args, json!("!diverged"));
            return Err(Error::Service);
        }
        let result = entry[2].clone();
        self.record(name, args, result.clone());
        if result == "!fail" {
            return Err(Error::Service);
        }
        Ok(from_json(&result))
    }
}
impl Context for World {
    fn call(&mut self, service: Service, args: &[Arg]) -> Result<Arg> {
        let listed = Value::Array(args.iter().map(to_json).collect());
        let int = |a: &Arg| match a {
            Arg::Int(v) => *v as i32,
            _ => panic!("integer argument"),
        };
        match service {
            Service::RngSharedNextInt | Service::RngShuffleNextInt => {
                let source = if service == Service::RngSharedNextInt {
                    &mut self.shared
                } else {
                    &mut self.shuffle
                };
                let v = source.next_bound(int(&args[0])).ok_or(Error::Reply)?;
                // The recording generator draws before an injected failure is raised.
                if self
                    .tape
                    .get(self.log.len())
                    .is_some_and(|e| e[2] == "!fail")
                {
                    self.record(service.name(), listed, json!("!fail"));
                    return Err(Error::Service);
                }
                self.record(service.name(), listed, json!(v));
                Ok(Arg::Int(i64::from(v)))
            }
            Service::RngMathNextDouble => {
                let v = self.math.next_double();
                if self
                    .tape
                    .get(self.log.len())
                    .is_some_and(|e| e[2] == "!fail")
                {
                    self.record(service.name(), listed, json!("!fail"));
                    return Err(Error::Service);
                }
                self.record(service.name(), listed, json!(bits(v)));
                Ok(Arg::Double(v))
            }
            Service::RngSharedNextDouble => {
                let v = self.shared.next_double();
                if self
                    .tape
                    .get(self.log.len())
                    .is_some_and(|e| e[2] == "!fail")
                {
                    self.record(service.name(), listed, json!("!fail"));
                    return Err(Error::Service);
                }
                self.record(service.name(), listed, json!(bits(v)));
                Ok(Arg::Double(v))
            }
            // The JVM cannot intercept these; they are compared through the seed.
            Service::RngThreadLocalNextDouble => {
                let (Arg::Double(origin), Arg::Double(bound)) = (&args[0], &args[1]) else {
                    panic!("double arguments")
                };
                Ok(Arg::Double(self.thread_local.next_double(*origin, *bound)))
            }
            Service::RngThreadLocalNextInt => Ok(Arg::Int(i64::from(
                self.thread_local.next_int(int(&args[0])),
            ))),
            // en-US default locale in the corpus: identical to ROOT lowercasing.
            Service::LowercaseDefault => match &args[0] {
                Arg::Text(t) => Ok(Arg::Text(lowercase_root(t))),
                _ => panic!("text argument"),
            },
            Service::PlayerDispatch => {
                let text = format!(
                    "ResourceProducedEvent({},{},{},{})",
                    int(&args[0]),
                    int(&args[1]),
                    int(&args[2]),
                    int(&args[3])
                );
                self.recorded(service.name(), json!([text]))
            }
            _ => self.recorded(service.name(), listed),
        }
    }
    fn defer(&mut self, service: Service, args: &[Arg], task: Task) -> Result<()> {
        let mut listed: Vec<Value> = args.iter().map(to_json).collect();
        if service == Service::ApiQueueScript {
            listed.push(json!(self.scripts.len()));
            self.scripts.push(task);
        } else {
            self.pulses.push(task);
        }
        self.recorded(service.name(), Value::Array(listed))
            .map(|_| ())
    }
}

fn woodcutting_row(n: &WoodcuttingNode) -> Value {
    json!({"name":n.name,"ordinal":n.ordinal,"id":n.full,"empty":n.empty,"identifier":n.identifier,"farming":n.farming,
        "reward":n.reward,"respawnRate":n.respawn_rate,"level":n.level,"rewardAmount":n.reward_amount,
        "experience":bits(n.experience),"rate":bits(n.rate),"baseLow":bits(n.base_low),"baseHigh":bits(n.base_high),
        "tierModLow":bits(n.tier_mod_low),"tierModHigh":bits(n.tier_mod_high),
        "minimumRespawn":n.minimum_respawn(),"maximumRespawn":n.maximum_respawn()})
}
fn mining_row(n: &MiningNode) -> Value {
    json!({"name":n.name,"ordinal":n.ordinal,"id":n.full,"empty":n.empty,"identifier":n.identifier,"reward":n.reward,
        "respawnRate":n.respawn_rate,"level":n.level,"rewardAmount":n.reward_amount(),"experience":bits(n.experience),
        "rate":bits(n.rate),"minimumRespawn":n.minimum_respawn(),"maximumRespawn":n.maximum_respawn()})
}
fn resource_row(r: &SkillingResource) -> Value {
    json!({"name":r.name,"ordinal":r.ordinal,"id":r.id,"level":r.level,"rate":bits(r.rate),"respawnRate":r.respawn_rate,
        "experience":bits(r.experience),"reward":r.reward,"rewardAmount":r.reward_amount,"label":r.label,"animation":Value::Null,
        "empty":r.empty_id,"skill":r.skill_id,"farming":r.farming,"minimumRespawn":r.minimum_respawn(),"maximumRespawn":r.maximum_respawn()})
}
fn tool_row(t: Tool) -> Value {
    json!({"name":t.name,"ordinal":t.ordinal,"id":t.id,"level":t.level,"ratio":bits(t.ratio),"animation":t.animation,
        "forId":Tool::for_id(t.id).map(|f| f.name)})
}
fn gem_list(gems: &[(i32, i32, i32, i32)]) -> Value {
    Value::Array(gems.iter().map(|g| json!([g.0, g.1, g.2, g.3])).collect())
}
fn gem_order(gems: &[(i32, i32, i32, i32)]) -> Value {
    Value::Array(gems.iter().map(|g| json!(g.0)).collect())
}
fn failure(e: Error) -> &'static str {
    match e {
        Error::Service | Error::Reply => "service",
        Error::NullPointer => "NullPointerException",
        Error::IndexOutOfBounds => "IndexOutOfBoundsException",
        Error::ClassCast => "ClassCastException",
    }
}

pub fn evaluate(case: &Value) -> Value {
    let index = case["index"].as_u64().unwrap_or(0) as usize;
    match case["kind"].as_str().unwrap() {
        "counts" => json!([
            WoodcuttingNode::values().len(),
            MiningNode::values().len(),
            SkillingResource::values().len(),
            Tool::values().count(),
            gem_list(&mining::State::default().gem_rock_gems)
        ]),
        "woodcuttingNode" => woodcutting_row(&WoodcuttingNode::values()[index]),
        "miningNode" => mining_row(&MiningNode::values()[index]),
        "resource" => resource_row(&SkillingResource::values()[index]),
        "tool" => tool_row(Tool::values().nth(index).unwrap()),
        "lookup" => Value::Array(
            case["ids"]
                .as_array()
                .unwrap()
                .iter()
                .map(|id| {
                    let id = id.as_i64().unwrap() as i32;
                    match case["table"].as_str().unwrap() {
                        "woodcutting" => json!([
                            WoodcuttingNode::for_id(id).map(|n| n.name),
                            WoodcuttingNode::is_empty(id)
                        ]),
                        "mining" => json!([
                            MiningNode::for_id(id).map(|n| n.name),
                            MiningNode::is_empty(id)
                        ]),
                        "resource" => json!([
                            SkillingResource::for_id(id).map(|n| n.name),
                            SkillingResource::is_empty(id)
                        ]),
                        _ => json!([Tool::for_id(id).map(|n| n.name), false]),
                    }
                })
                .collect(),
        ),
        "respawn" => {
            let name = case["name"].as_str().unwrap();
            let rate = match case["table"].as_str().unwrap() {
                "woodcutting" => {
                    WoodcuttingNode::values()
                        .iter()
                        .find(|n| n.name == name)
                        .unwrap()
                        .respawn_rate
                }
                "mining" => MiningNode::by_name(name).unwrap().respawn_rate,
                _ => {
                    SkillingResource::values()
                        .iter()
                        .find(|n| n.name == name)
                        .unwrap()
                        .respawn_rate
                }
            };
            let players = case["playerCount"].as_i64().unwrap() as i32;
            let ret = respawn_duration(rate, case["maxPlayers"].as_i64().unwrap() as i32, players);
            json!({"events":[["repository.players.size",[],players]],"ret":ret})
        }
        "random" => randoms(case),
        "fishingCounts" => json!([
            Fish::values().len(),
            FishingOption::values().len(),
            FishingSpot::values().len(),
            FishingSpot::all_ids()
        ]),
        "fish" => {
            let f = &Fish::values()[index];
            json!({"name":f.name,"ordinal":f.ordinal,"id":f.id,"level":f.level,"experience":bits(f.experience),
                "lowChance":bits(f.low_chance),"highChance":bits(f.high_chance),"bigFish":f.big_fish(),
                "barehandFishingReq":f.barehand_fishing_req(),"barehandStrengthReq":f.barehand_strength_req(),
                "barehandStrengthXp":f.barehand_strength_xp().map(bits),"forItem":Fish::for_item(f.id).map(|x| x.name)})
        }
        "fishingOption" => {
            let o = &FishingOption::values()[index];
            let caught: Vec<&str> = o.fish.iter().map(|&f| Fish::values()[f].name).collect();
            json!({"name":o.name,"ordinal":o.ordinal,"tool":o.tool,"level":o.level,"animation":o.animation,"option":o.option,
                "startMessage":o.start_message(),"bait":o.bait,"fish":caught,"forName":FishingOption::for_name(o.option).map(|x| x.name)})
        }
        "fishingSpot" => {
            let s = &FishingSpot::values()[index];
            let options: Vec<&str> = s
                .options
                .iter()
                .map(|&o| FishingOption::values()[o].name)
                .collect();
            json!({"name":s.name,"ordinal":s.ordinal,"ids":s.ids,"options":options})
        }
        "fishingLookup" => Value::Array(
            case["keys"]
                .as_array()
                .unwrap()
                .iter()
                .map(|key| match case["table"].as_str().unwrap() {
                    "spot" => {
                        json!(FishingSpot::for_id(key.as_i64().unwrap() as i32).map(|s| s.name))
                    }
                    "fish" => json!(Fish::for_item(key.as_i64().unwrap() as i32).map(|s| s.name)),
                    "option" => {
                        json!(FishingOption::for_name(key.as_str().unwrap()).map(|s| s.name))
                    }
                    "chance" => json!(bits(
                        Fish::named(key[0].as_str().unwrap())
                            .unwrap()
                            .success_chance(key[1].as_i64().unwrap() as i32)
                    )),
                    _ => json!(
                        FishingSpot::values()
                            .iter()
                            .find(|s| s.name == key[0].as_str().unwrap())
                            .unwrap()
                            .option_by_name(key[1].as_str().unwrap())
                            .map(|o| o.name)
                    ),
                })
                .collect(),
        ),
        "selectTool" => select(case),
        "scenario" => scenario(case),
        other => panic!("unknown kind {other}"),
    }
}

fn randoms(case: &Value) -> Value {
    let mut world = World::new(case);
    let mut gems = mining::State::default();
    let mut out = Vec::new();
    for call in case["calls"].as_array().unwrap() {
        world.begin(&Value::Null);
        let a = &call[1];
        let int = |i: usize| a[i].as_i64().unwrap() as i32;
        let ret = match call[0].as_str().unwrap() {
            "random" => json!(random::random(&mut world, int(0)).unwrap()),
            "randomRange" => json!(random::random_range(&mut world, int(0), int(1)).unwrap()),
            "roll" => json!(random::roll(&mut world, int(0)).unwrap()),
            "randomize" => json!(random::randomize(&mut world, int(0)).unwrap()),
            "getRandom" => json!(random::get_random(&mut world, int(0)).unwrap()),
            "chance" => json!(bits(random::skill_success_chance(
                double(&a[0]),
                double(&a[1]),
                int(2)
            ))),
            "randomDouble" => json!(bits(
                random::random_double(&mut world, double(&a[0])).unwrap()
            )),
            "gems" => json!(
                random::roll_weighted_chance_table(&mut world, &mut gems.gem_rock_gems)
                    .unwrap()
                    .unwrap()
                    .0
            ),
            other => panic!("unknown call {other}"),
        };
        let mut entry = json!({"ret":ret,"events":world.log});
        if call[0] == "randomDouble" {
            entry["tlr"] = json!(world.thread_local.seed.to_string());
        }
        if call[0] == "gems" {
            entry["gems"] = gem_order(&gems.gem_rock_gems);
        }
        out.push(entry);
    }
    Value::Array(out)
}

fn select(case: &Value) -> Value {
    let mut world = World::new(case);
    world.begin(&case["expected"]["events"]);
    let skill = case["skill"].as_i64().unwrap_or(0) as i32;
    let name = |t: Result<Option<Tool>>| t.map(|t| json!(t.map(|t| t.name)));
    let ret = match case["method"].as_str().unwrap() {
        "getHatchet" => name(tools::get_hatchet(&mut world, 1000)),
        "getPickaxe" => name(tools::get_pickaxe(&mut world, 1000)),
        "getHarpoon" => name(tools::get_harpoon(&mut world, 1000)),
        "getButterflyNet" => name(tools::get_butterfly_net(&mut world, 1000)),
        "getToolForSkill" => name(tools::get_tool_for_skill(&mut world, 1000, skill)),
        "checkTool" => tools::check_tool(
            &mut world,
            1000,
            skill,
            Tool::named(case["tool"].as_str().unwrap()),
        )
        .map(|b| json!(b)),
        other => panic!("unknown method {other}"),
    };
    json!({"ret":ret.unwrap_or(json!("!error")),"events":world.log})
}

enum Active {
    None,
    Wood(woodcutting::Pulse),
    Mine(mining::Pulse),
    Fish(fishing::Pulse),
}

fn scenario(case: &Value) -> Value {
    let mut world = World::new(case);
    let settings = Settings {
        max_players: case["maxPlayers"].as_i64().unwrap_or(2000) as i32,
    };
    let subject = case["subject"].as_str().unwrap();
    // A fishing spot is registered after the scenario scenery, so its token is 1.
    let fishing_subject = subject.starts_with("fishing");
    let spot = (fishing_subject && case["node"]["kind"] != "scenery").then_some(1i64);
    let (player, node) = (1000i64, spot.unwrap_or(0));
    let mut gems = mining::State::default();
    let mut active = Active::None;
    let mut ticks = case["ticks"].as_i64().unwrap_or(100);
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
        world.begin(&expected["events"]);
        let state = op["state"].as_i64().unwrap_or(0) as i32;
        let wrap = |r: Result<bool>| r.map(|b| json!(b));
        let unit = |r: Result<()>| r.map(|()| Value::Null);
        let ret: Result<Value> = match op["op"].as_str().unwrap() {
            "define" => {
                has_script = true;
                script_next = 0;
                script_state = 0;
                unit(match subject {
                    "fishing" => fishing::define_listeners(&mut world),
                    "woodcutting" => woodcutting::define_listeners(&mut world),
                    "mining" => mining::define_listeners(&mut world),
                    _ => mining::define_prospect_listeners(&mut world),
                })
            }
            "handler" if fishing_subject => {
                wrap(fishing::handle_fishing(&mut world, player, spot, state))
            }
            "tick" if fishing_subject => {
                ticks += 1;
                if !has_script || script_next >= ticks {
                    Ok(json!("idle"))
                } else {
                    let r = fishing::handle_fishing(&mut world, player, spot, script_state as i32);
                    if let Ok(finished) = r {
                        script_state += 1;
                        if finished && persist {
                            script_state = 0;
                        }
                    }
                    wrap(r)
                }
            }
            "runPulse" if fishing_subject => {
                let index = op["index"].as_u64().unwrap() as usize;
                match world.pulses.get(index).cloned() {
                    Some(mut task) => {
                        let r = fishing::run_task(&mut world, &mut task);
                        // The counter belongs to the submitted pulse object.
                        world.pulses[index] = task;
                        wrap(r)
                    }
                    None => Err(Error::IndexOutOfBounds),
                }
            }
            "pulseNew" if fishing_subject => {
                let option = op["option"]
                    .as_str()
                    .map(|name| FishingOption::named(name).unwrap());
                fishing::Pulse::new(&mut world, player, node, option).map(|p| {
                    active = Active::Fish(p);
                    Value::Null
                })
            }
            "pulseCheck" | "pulseAnimate" | "pulseReward" | "pulseMessage" => {
                let Active::Fish(p) = &mut active else {
                    panic!("no fishing pulse")
                };
                match op["op"].as_str().unwrap() {
                    "pulseCheck" => wrap(p.check_requirements(&mut world)),
                    "pulseAnimate" => unit(p.animate(&mut world)),
                    "pulseReward" => wrap(p.reward(&mut world)),
                    _ => unit(p.message(&mut world, op["type"].as_i64().unwrap() as i32)),
                }
            }
            "handler" => wrap(handler(
                &mut world, &settings, &mut gems, subject, player, node, state,
            )),
            "tick" => {
                ticks += 1;
                if !has_script || script_next >= ticks {
                    Ok(json!("idle"))
                } else {
                    let r = handler(
                        &mut world,
                        &settings,
                        &mut gems,
                        subject,
                        player,
                        node,
                        script_state as i32,
                    );
                    if let Ok(finished) = r {
                        script_state += 1;
                        if finished && persist {
                            script_state = 0;
                        }
                    }
                    wrap(r)
                }
            }
            "script" => match world
                .scripts
                .get(op["index"].as_u64().unwrap() as usize)
                .cloned()
            {
                Some(task) => wrap(woodcutting::run_task(
                    &mut world,
                    &task,
                    op["stage"].as_i64().unwrap_or(0) as i32,
                )),
                // The driver asked for a body that was never queued.
                None => Err(Error::IndexOutOfBounds),
            },
            "on" => wrap(mining::prospect(&mut world, player, node)),
            "runPulse" => {
                let task = world.pulses[op["index"].as_u64().unwrap() as usize].clone();
                wrap(mining::run_task(&mut world, &task))
            }
            "pulseNew" => {
                active = if subject == "woodcuttingPulse" {
                    Active::Wood(woodcutting::Pulse::new(player, node))
                } else {
                    Active::Mine(mining::Pulse::new(player, node))
                };
                Ok(Value::Null)
            }
            "pulseStart" => unit(match &mut active {
                Active::Wood(p) => p.start(&mut world),
                Active::Mine(p) => p.start(&mut world),
                Active::Fish(p) => p.start(&mut world),
                Active::None => panic!("no pulse"),
            }),
            "pulseStop" => unit(match &mut active {
                Active::Wood(p) => p.stop(&mut world),
                Active::Mine(p) => p.stop(&mut world),
                Active::Fish(p) => p.stop(&mut world),
                Active::None => panic!("no pulse"),
            }),
            "pulsePulse" => wrap(match &mut active {
                Active::Wood(p) => p.pulse(&mut world, &settings),
                Active::Mine(p) => p.pulse(&mut world, &settings, &mut gems),
                Active::Fish(p) => p.pulse(&mut world),
                Active::None => panic!("no pulse"),
            }),
            "set" => {
                if let Some(t) = op["ticks"].as_i64() {
                    ticks = t;
                }
                if !op["sharedSeed"].is_null() {
                    world.shared = JavaRandom::new(seed(op, "sharedSeed", 0));
                }
                if !op["mathSeed"].is_null() {
                    world.math = JavaRandom::new(seed(op, "mathSeed", 0));
                }
                if !op["shuffleSeed"].is_null() {
                    world.shuffle = JavaRandom::new(seed(op, "shuffleSeed", 0));
                }
                if !op["tlrSeed"].is_null() {
                    world.thread_local.seed = seed(op, "tlrSeed", 0);
                }
                if let (Some(delay), Active::Fish(p)) = (op["delay"].as_i64(), &mut active) {
                    p.delay = delay as i32;
                }
                Ok(Value::Null)
            }
            other => panic!("unknown op {other}"),
        };
        // World state belongs to the recorded services and is written as changes
        // since the previous operation; only natively owned fields are recomputed.
        let mut merged = full.clone();
        for (key, value) in expected["state"].as_object().into_iter().flatten() {
            merged.insert(key.clone(), value.clone());
        }
        merged.insert("tlr".into(), json!(world.thread_local.seed.to_string()));
        merged.insert("gems".into(), gem_order(&gems.gem_rock_gems));
        merged.insert("scriptState".into(), json!(script_state));
        merged.insert("queued".into(), json!(world.scripts.len()));
        let running = match &active {
            Active::Wood(p) => Some(p.running),
            Active::Mine(p) => Some(p.running),
            Active::Fish(p) => Some(p.running),
            Active::None => None,
        };
        if let Active::Fish(p) = &active {
            // The pulse delay and the rolled fish are native fields too.
            if let Some(pulse) = merged.get_mut("pulse") {
                pulse[1] = json!(p.delay);
            }
            if p.fish.is_some() || merged.contains_key("fish") {
                merged.insert("fish".into(), json!(p.fish.map(|f| f.name)));
            }
        }
        if let (Some(running), Some(pulse)) = (running, merged.get_mut("pulse")) {
            pulse[0] = json!(running);
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
        let mut result = json!({"events":world.log,"state":snapshot});
        match ret {
            Ok(v) => result["ret"] = v,
            Err(e) => result["error"] = json!(failure(e)),
        }
        out.push(result);
    }
    Value::Array(out)
}

fn handler(
    world: &mut World,
    settings: &Settings,
    gems: &mut mining::State,
    subject: &str,
    player: i64,
    node: i64,
    state: i32,
) -> Result<bool> {
    match subject {
        "woodcutting" => woodcutting::handle_woodcutting(world, settings, player, node, state),
        "mining" => mining::handle_mining(world, settings, gems, player, node, state),
        other => panic!("no handler for {other}"),
    }
}
