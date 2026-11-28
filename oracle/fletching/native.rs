//! Native replay of the fletching corpus.
//!
//! The case's declared state answers every query, exactly as the oracle's
//! recording world does, and every recorded call is logged in the same form;
//! each operation's result and log must equal the original's.
//!
//! The listener lookup (the pair as used, then flipped) is the same declared
//! model the JVM driver uses, over the ids the library registers. The
//! dialogue a listener opens and the stringing listener's RepositionChild
//! are logged here from the library's own answers (`finished_items`,
//! `Job::kind`, `Job::product`), as the live binding sends them.
use rs09_server::world::{
    dialogue_dsl::skill::SkillDialogue,
    fletching_ammo::{self as ammo, Craft},
    fletching_assembly::{self as assembly, Job, Listener},
    fletching_logs::{self as logs, Error, Quest, Result},
};
use serde_json::{Map, Value, json};

fn bits(v: f64) -> String {
    format!("d:{:016x}", v.to_bits())
}

/// What the open dialogue's handler captured.
enum Handler {
    Logs { log: i32, crafts: Vec<usize> },
    Assembly { job: Job, string: i32 },
    Ammo { craft: Craft, used: i32, with: i32 },
}
/// A script a `queueScript` holds.
#[derive(Clone)]
enum Script {
    Logs(usize, i32),
    Assembly(Job, i32),
    Ammo(ammo::Script, i32),
}

pub struct World {
    state: Map<String, Value>,
    names: Map<String, Value>,
    trace: Vec<Value>,
    handler: Option<Handler>,
    /// Every script the case queued, in order.
    scripts: Vec<Script>,
}
impl World {
    fn flag(&self, key: &str) -> bool {
        self.state[key] == Value::Bool(true)
    }
    fn held(&self, id: i32) -> i32 {
        self.state["held"][id.to_string()].as_i64().unwrap_or(0) as i32
    }
    fn set_held(&mut self, id: i32, amount: i32) {
        self.state["held"][id.to_string()] = json!(amount);
    }
    fn emit(&mut self, row: Value) {
        self.trace.push(row);
    }
    fn level(&self) -> i32 {
        self.state["level"].as_i64().unwrap() as i32
    }
    fn stage(&self, quest: Quest) -> i32 {
        let key = match quest {
            Quest::BigChompyBirdHunting => "BIG_CHOMPY_BIRD_HUNTING",
            Quest::ZogreFleshEaters => "ZOGRE_FLESH_EATERS",
        };
        self.state["quests"][key].as_i64().unwrap_or(0) as i32
    }
    fn remove(&mut self, id: i32, amount: i32) -> bool {
        let held = self.held(id);
        let ok = held >= amount;
        if ok {
            self.set_held(id, held - amount);
        }
        self.emit(json!(["remove", id, amount, ok]));
        ok
    }
    fn add(&mut self, id: i32, amount: i32) {
        let held = self.held(id);
        self.set_held(id, held + amount);
        self.emit(json!(["add", id, amount]));
    }
    fn at(&self) -> [i32; 3] {
        let at = self.state["at"].as_array().unwrap();
        [0, 1, 2].map(|n| at[n].as_i64().unwrap() as i32)
    }
    fn attribute_key(key: &str) -> &str {
        key.strip_prefix("/save:").unwrap_or(key)
    }
}

impl logs::Host for World {
    fn level(&mut self) -> Result<i32> {
        Ok(World::level(self))
    }
    fn item_name(&mut self, id: i32) -> Result<String> {
        let key = id.to_string();
        let own = self.state.get("names").and_then(|names| names.get(&key));
        match own.or_else(|| self.names.get(&key)) {
            Some(Value::String(name)) => Ok(name.clone()),
            _ => Err(Error::Null),
        }
    }
    fn dialogue(&mut self, message: &str) -> Result<()> {
        self.emit(json!(["dialogue", message]));
        Ok(())
    }
    fn message(&mut self, message: &str) -> Result<()> {
        self.emit(json!(["message", message]));
        Ok(())
    }
    fn quest_stage(&mut self, quest: Quest) -> Result<i32> {
        Ok(self.stage(quest))
    }
    fn amount_in_inventory(&mut self, id: i32) -> Result<i32> {
        Ok(self.held(id))
    }
    fn queue(&mut self, delay: i32, craft: usize, amount: i32) -> Result<()> {
        self.scripts.push(Script::Logs(craft, amount));
        self.emit(json!(["queue", delay, "WEAK", false]));
        Ok(())
    }
    fn clock_ready(&mut self) -> Result<bool> {
        Ok(self.flag("clockReady"))
    }
    fn keep_running(&mut self) -> Result<bool> {
        self.emit(json!(["keepRunning"]));
        Ok(false)
    }
    fn stop_executing(&mut self) -> Result<bool> {
        self.emit(json!(["stopExecuting"]));
        Ok(true)
    }
    fn remove_item(&mut self, id: i32) -> Result<bool> {
        Ok(self.remove(id, 1))
    }
    fn animate(&mut self, id: i32) -> Result<()> {
        self.emit(json!(["animate", id]));
        Ok(())
    }
    fn random(&mut self, max: i32) -> Result<i32> {
        // RandomFunction.random(int): nothing is drawn for a bound of 0 or less.
        if max <= 0 {
            return Ok(0);
        }
        let draws = self.state["draws"].as_array_mut().unwrap();
        let value = draws.remove(0).as_i64().unwrap() as i32;
        self.emit(json!(["random", max, value]));
        Ok(value)
    }
    fn add_item(&mut self, id: i32, amount: i32) -> Result<bool> {
        self.add(id, amount);
        Ok(true)
    }
    fn reward_xp(&mut self, xp: f64) -> Result<()> {
        self.emit(json!(["xp", logs::FLETCHING, bits(xp)]));
        Ok(())
    }
    fn location(&mut self) -> Result<[i32; 3]> {
        Ok(self.at())
    }
    fn seers_task_complete(&mut self, _: i32, _: i32) -> Result<bool> {
        Ok(self.flag("taskDone"))
    }
    fn set_attribute_true(&mut self, key: &str) -> Result<()> {
        self.state["attributes"][Self::attribute_key(key)] = json!(true);
        self.emit(json!(["setAttribute", key, true]));
        Ok(())
    }
    fn delay_clock_and_script(&mut self, ticks: i32) -> Result<bool> {
        self.emit(json!(["delayClock", logs::SKILLING, ticks, true]));
        Ok(false)
    }
}

impl assembly::Host for World {
    fn level(&mut self) -> Result<i32> {
        Ok(World::level(self))
    }
    fn dialogue(&mut self, message: &str) -> Result<()> {
        logs::Host::dialogue(self, message)
    }
    fn message(&mut self, message: &str) -> Result<()> {
        logs::Host::message(self, message)
    }
    fn quest_stage(&mut self, quest: Quest) -> Result<i32> {
        Ok(self.stage(quest))
    }
    fn broads_unlocked(&mut self) -> Result<bool> {
        Ok(self.flag("broads"))
    }
    fn amount_in_inventory(&mut self, id: i32) -> Result<i32> {
        Ok(self.held(id))
    }
    fn has_space_for(&mut self, _: i32) -> Result<bool> {
        Ok(self.flag("space"))
    }
    fn queue(&mut self, delay: i32, job: Job, amount: i32) -> Result<()> {
        self.scripts.push(Script::Assembly(job, amount));
        self.emit(json!(["queue", delay, "WEAK", false]));
        Ok(())
    }
    fn clock_ready(&mut self) -> Result<bool> {
        Ok(self.flag("clockReady"))
    }
    fn keep_running(&mut self) -> Result<bool> {
        logs::Host::keep_running(self)
    }
    fn stop_executing(&mut self) -> Result<bool> {
        logs::Host::stop_executing(self)
    }
    fn remove_item(&mut self, id: i32, amount: i32) -> Result<bool> {
        Ok(self.remove(id, amount))
    }
    fn animate(&mut self, id: i32) -> Result<()> {
        logs::Host::animate(self, id)
    }
    fn add_item(&mut self, id: i32, amount: i32) -> Result<bool> {
        self.add(id, amount);
        Ok(true)
    }
    fn reward_xp(&mut self, xp: f64) -> Result<()> {
        logs::Host::reward_xp(self, xp)
    }
    fn location(&mut self) -> Result<[i32; 3]> {
        Ok(self.at())
    }
    fn attribute(&mut self, key: &str) -> Result<bool> {
        match &self.state["attributes"][Self::attribute_key(key)] {
            Value::Null => Ok(false),
            Value::Bool(value) => Ok(*value),
            _ => Err(Error::Cast),
        }
    }
    fn finish_seers_task(&mut self, level: i32, index: i32) -> Result<()> {
        self.emit(json!(["finishTask", "SEERS_VILLAGE", level, index]));
        Ok(())
    }
    fn delay_clock_and_script(&mut self, ticks: i32) -> Result<bool> {
        logs::Host::delay_clock_and_script(self, ticks)
    }
}

impl ammo::Host for World {
    fn level(&mut self) -> Result<i32> {
        Ok(World::level(self))
    }
    fn dialogue(&mut self, message: &str) -> Result<()> {
        logs::Host::dialogue(self, message)
    }
    fn message(&mut self, message: &str) -> Result<()> {
        logs::Host::message(self, message)
    }
    fn tourist_trap_stage(&mut self) -> Result<i32> {
        Ok(self.state["quests"]["THE_TOURIST_TRAP"]
            .as_i64()
            .unwrap_or(0) as i32)
    }
    fn broads_unlocked(&mut self) -> Result<bool> {
        Ok(self.flag("broads"))
    }
    fn amount_in_inventory(&mut self, id: i32) -> Result<i32> {
        Ok(self.held(id))
    }
    fn has_space_for(&mut self, _: i32) -> Result<bool> {
        Ok(self.flag("space"))
    }
    fn queue(&mut self, delay: i32, script: ammo::Script, amount: i32) -> Result<()> {
        self.scripts.push(Script::Ammo(script, amount));
        self.emit(json!(["queue", delay, "WEAK", false]));
        Ok(())
    }
    fn clock_ready(&mut self) -> Result<bool> {
        Ok(self.flag("clockReady"))
    }
    fn keep_running(&mut self) -> Result<bool> {
        logs::Host::keep_running(self)
    }
    fn stop_executing(&mut self) -> Result<bool> {
        logs::Host::stop_executing(self)
    }
    fn remove_item(&mut self, id: i32, amount: i32) -> Result<bool> {
        Ok(self.remove(id, amount))
    }
    fn animate(&mut self, id: i32) -> Result<()> {
        logs::Host::animate(self, id)
    }
    fn add_item(&mut self, id: i32, amount: i32) -> Result<bool> {
        self.add(id, amount);
        Ok(true)
    }
    fn reward_xp(&mut self, xp: f64) -> Result<()> {
        logs::Host::reward_xp(self, xp)
    }
    fn delay_clock(&mut self, ticks: i32, delay_script: bool) -> Result<bool> {
        self.emit(json!(["delayClock", logs::SKILLING, ticks, delay_script]));
        Ok(false)
    }
    fn delay_script(&mut self, ticks: i32) -> Result<bool> {
        self.emit(json!(["delayScript", ticks]));
        Ok(false)
    }
}

/// What a registration's lambda is.
#[derive(Clone, Copy)]
enum Lambda {
    Logs,
    Assembly(Listener),
    Ammo(ammo::Listener),
}

/// The registrations in the order the three classes make them.
fn registrations() -> Vec<(Lambda, Vec<i32>, Vec<i32>)> {
    let mut all = vec![(Lambda::Logs, vec![logs::KNIFE], logs::log_ids())];
    for listener in Listener::ALL {
        let (used, with) = listener.ids();
        all.push((Lambda::Assembly(listener), used, with));
    }
    for listener in ammo::Listener::ALL {
        let (used, with) = listener.ids();
        all.push((Lambda::Ammo(listener), used, with));
    }
    all
}

fn open(world: &mut World, kind: Option<SkillDialogue>, items: Vec<i32>) {
    world.emit(json!(["open", kind.map(SkillDialogue::name), items]));
}

fn lambda(world: &mut World, which: Lambda, used: i32, with: i32) -> Result<Value> {
    match which {
        Lambda::Logs => {
            let Some(crafts) = logs::for_log_id(with) else {
                return Ok(json!(false));
            };
            let items = logs::finished_items(&crafts);
            open(world, SkillDialogue::for_length(crafts.len() as i32), items);
            world.handler = Some(Handler::Logs { log: with, crafts });
            Ok(json!(true))
        }
        Lambda::Assembly(listener) => match assembly::use_with(world, listener, used, with)? {
            None => Ok(json!(false)),
            Some(None) => Ok(json!(true)),
            Some(Some(job)) => {
                open(world, Some(job.kind()), vec![job.product()?]);
                world.handler = Some(Handler::Assembly { job, string: used });
                if listener == Listener::Stringing {
                    world.emit(json!(["packet", "RepositionChild", 309, 2, 215, 10]));
                }
                Ok(json!(true))
            }
        },
        Lambda::Ammo(listener) => match ammo::use_with(world, listener, used, with)? {
            None => Ok(json!(false)),
            Some(None) => Ok(json!(true)),
            Some(Some(craft)) => {
                open(world, Some(craft.kind()), vec![craft.product()?]);
                world.handler = Some(Handler::Ammo { craft, used, with });
                if let Some([interface, child, x, y]) = craft.reposition() {
                    world.emit(json!(["packet", "RepositionChild", interface, child, x, y]));
                }
                Ok(json!(true))
            }
        },
    }
}

fn step(world: &mut World, op: &Value) -> Result<Value> {
    let number = |key: &str| op[key].as_i64().unwrap() as i32;
    match op["op"].as_str().unwrap() {
        "registrations" => Ok(Value::Array(
            registrations()
                .into_iter()
                .map(|(_, used, with)| json!(["ITEM", used, with]))
                .collect(),
        )),
        "set" => {
            for (key, value) in op["state"].as_object().unwrap() {
                world.state.insert(key.clone(), value.clone());
            }
            Ok(Value::Null)
        }
        "use" => {
            let (used, with) = (number("used"), number("with"));
            let all = registrations();
            for (which, u, w) in &all {
                if u.contains(&used) && w.contains(&with) {
                    return lambda(world, *which, used, with);
                }
            }
            for (which, u, w) in &all {
                if u.contains(&with) && w.contains(&used) {
                    return lambda(world, *which, with, used);
                }
            }
            Ok(json!("unhandled"))
        }
        "create" => {
            let amount = number("amount");
            match world.handler.take() {
                None => Ok(json!("no handler")),
                Some(Handler::Logs { log, crafts }) => {
                    let result = logs::create(world, &crafts, amount, number("index"));
                    world.handler = Some(Handler::Logs { log, crafts });
                    result.map(|()| Value::Null)
                }
                Some(Handler::Assembly { job, string }) => {
                    world.handler = Some(Handler::Assembly { job, string });
                    assembly::create(world, job, amount).map(|()| Value::Null)
                }
                Some(Handler::Ammo { craft, used, with }) => {
                    world.handler = Some(Handler::Ammo { craft, used, with });
                    ammo::create(world, craft, amount).map(|()| Value::Null)
                }
            }
        }
        "getAll" => match &world.handler {
            None => Ok(json!("no handler")),
            // The live binding's `getAll`: amountInInventory(player, log.id).
            Some(Handler::Logs { log, .. }) => Ok(json!(world.held(*log))),
            Some(Handler::Assembly { job, string }) => {
                let (job, string) = (*job, *string);
                assembly::get_all(world, job, string).map(|n| json!(n))
            }
            Some(Handler::Ammo { craft, used, with }) => {
                let (craft, used, with) = (*craft, *used, *with);
                ammo::get_all(world, craft, used, with).map(|n| json!(n))
            }
        },
        "script" => {
            // The script the case names by its place in the queueing order,
            // or the last one queued.
            let n = match op.get("n") {
                Some(n) => usize::try_from(n.as_i64().unwrap()).ok(),
                None => world.scripts.len().checked_sub(1),
            };
            let stage = number("stage");
            match n.and_then(|n| world.scripts.get(n).cloned()) {
                None => Ok(json!("no script")),
                Some(Script::Logs(craft, amount)) => {
                    logs::script(world, craft, amount, stage).map(|b| json!(b))
                }
                Some(Script::Assembly(job, amount)) => {
                    assembly::script(world, job, amount, stage).map(|b| json!(b))
                }
                Some(Script::Ammo(script, amount)) => {
                    ammo::script(world, &script, amount, stage).map(|b| json!(b))
                }
            }
        }
        other => panic!("unknown operation {other}"),
    }
}

/// Replays one case and returns its results in the corpus's form.
pub fn replay(case: &Value, names: &Value) -> Value {
    let mut world = World {
        state: case["state"].as_object().unwrap().clone(),
        names: names.as_object().unwrap().clone(),
        trace: Vec::new(),
        handler: None,
        scripts: Vec::new(),
    };
    let mut results = Vec::new();
    for op in case["ops"].as_array().unwrap() {
        world.trace.clear();
        let mut result = Map::new();
        match step(&mut world, op) {
            Ok(value) => result.insert("result".into(), value),
            Err(Error::Null) => result.insert("throws".into(), json!("Null")),
            Err(Error::Index) => result.insert("throws".into(), json!("Index")),
            Err(Error::Cast) => result.insert("throws".into(), json!("Cast")),
            Err(Error::Binding(text)) => panic!("binding failure: {text}"),
        };
        result.insert("trace".into(), Value::Array(world.trace.clone()));
        results.push(Value::Object(result));
    }
    Value::Array(results)
}
