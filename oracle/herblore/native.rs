//! Native replay of the herblore corpus.
//!
//! The case's declared state answers every query, exactly as the oracle's
//! recording world does, and every recorded call is logged in the same form;
//! each operation's result and log must equal the original's.
use rs09_server::world::herblore::{
    self, Error, FINISHED, HERBLORE, HERBS, Listener, Potion, Pulse, Quest, Result, UNFINISHED,
};
use serde_json::{Map, Value, json};

fn bits(v: f64) -> String {
    format!("d:{:016x}", v.to_bits())
}

/// The item of a "clean" operation.
pub struct Item {
    id: i32,
    slot: i32,
}

pub struct World<'a> {
    state: Map<String, Value>,
    names: &'a Map<String, Value>,
    trace: Vec<Value>,
    /// The potion of the dialogue the case opened last.
    handler: Option<Potion>,
    /// The pulse the case's last run kept.
    pulse: Option<Pulse>,
    drawn: usize,
}
impl World<'_> {
    fn emit(&mut self, row: Value) {
        self.trace.push(row);
    }
    fn held(&self, id: i32) -> i64 {
        self.state["held"]
            .get(id.to_string())
            .and_then(Value::as_i64)
            .unwrap_or(0)
    }
    fn set_held(&mut self, id: i32, amount: i64) {
        let held = self.state["held"].as_object_mut().unwrap();
        held.insert(id.to_string(), json!(amount));
    }
}

impl herblore::Host for World<'_> {
    type Item = Item;

    fn quest_stage(&mut self, quest: Quest) -> Result<i32> {
        self.emit(json!(["questStage", quest.constant()]));
        let stage = self.state["quests"].get(quest.constant());
        Ok(stage.and_then(Value::as_i64).unwrap_or(0) as i32)
    }
    fn has_requirement(&mut self, quest: &str) -> Result<bool> {
        self.emit(json!(["hasRequirement", quest, false]));
        let listed = self.state["requirements"].as_array().unwrap();
        Ok(listed.iter().any(|q| q == quest))
    }
    fn level(&mut self) -> Result<i32> {
        self.emit(json!(["level", HERBLORE]));
        Ok(self.state["level"].as_i64().unwrap() as i32)
    }
    fn send_message(&mut self, message: &str) -> Result<()> {
        self.emit(json!(["sendMessage", message]));
        Ok(())
    }
    fn message(&mut self, message: &str) -> Result<()> {
        self.emit(json!(["message", message]));
        Ok(())
    }
    fn lock(&mut self, ticks: i32) -> Result<()> {
        self.emit(json!(["lock", ticks]));
        Ok(())
    }
    fn item_name(&mut self, id: i32) -> Result<Option<String>> {
        let key = id.to_string();
        let name = match self.state["names"].get(&key) {
            Some(own) => own,
            None => self.names.get(&key).unwrap_or(&Value::Null),
        };
        Ok(name.as_str().map(str::to_owned))
    }
    fn slot(&mut self, item: &Item) -> Result<i32> {
        Ok(item.slot)
    }
    fn replace_slot(&mut self, slot: i32, product: i32, item: &Item) -> Result<()> {
        self.emit(json!(["replaceSlot", slot, product, 1, item.id]));
        Ok(())
    }
    fn reward_xp(&mut self, xp: f64) -> Result<()> {
        self.emit(json!(["rewardXP", HERBLORE, bits(xp)]));
        Ok(())
    }
    fn add_experience(&mut self, xp: f64) -> Result<()> {
        self.emit(json!(["xp", HERBLORE, bits(xp), true]));
        Ok(())
    }
    fn play_audio(&mut self, id: i32) -> Result<()> {
        self.emit(json!(["audio", id, 0, 1]));
        Ok(())
    }
    fn amount_in_inventory(&mut self, id: i32) -> Result<i32> {
        self.emit(json!(["amount", id]));
        Ok(self.held(id) as i32)
    }
    fn contains_item(&mut self, id: i32) -> Result<bool> {
        self.emit(json!(["contains", id]));
        Ok(self.held(id) >= 1)
    }
    fn remove(&mut self, base: i32, ingredient: i32) -> Result<bool> {
        // Each removal succeeds when at least the amount is held.
        let mut all = true;
        for id in [base, ingredient] {
            let held = self.held(id);
            if held >= 1 {
                self.set_held(id, held - 1);
            } else {
                all = false;
            }
        }
        self.emit(json!(["remove", [base, ingredient], all]));
        Ok(all)
    }
    fn add(&mut self, id: i32) -> Result<()> {
        let held = self.held(id);
        self.set_held(id, held + 1);
        self.emit(json!(["add", id, 1]));
        Ok(())
    }
    fn animate(&mut self, id: i32) -> Result<()> {
        self.emit(json!(["animate", id]));
        Ok(())
    }
    fn reset_animation(&mut self) -> Result<()> {
        self.emit(json!(["animate", -1, "HIGH"]));
        Ok(())
    }
    fn attribute(&mut self, key: &str) -> Result<bool> {
        self.emit(json!(["attribute", key]));
        match self.state["attributes"].get(key) {
            None | Some(Value::Null) => Ok(false),
            Some(Value::Bool(value)) => Ok(*value),
            // The Kotlin read's cast to Boolean.
            Some(_) => Err(Error::Cast),
        }
    }
    fn random(&mut self, max: i32) -> Result<i32> {
        if max <= 0 {
            return Ok(0);
        }
        // RANDOM.nextInt(max): the next declared draw, reduced into the bound.
        let draws = self.state["draws"].as_array().unwrap();
        let value = (draws[self.drawn % draws.len()].as_i64().unwrap() % i64::from(max)) as i32;
        self.drawn += 1;
        self.emit(json!(["random", max, value]));
        Ok(value)
    }
    fn consumable_first_id(&mut self, id: i32) -> Result<Option<i32>> {
        let ids = self.state["consumables"].get(id.to_string());
        let first = ids.map(|ids| ids[0].as_i64().unwrap() as i32);
        self.emit(json!(["consumable", id, first]));
        Ok(first)
    }
    fn run_pulse(&mut self, potion: Potion, amount: i32) -> Result<()> {
        self.emit(json!([
            "run",
            potion.base,
            potion.ingredient,
            potion.level,
            bits(potion.experience),
            potion.product,
            amount
        ]));
        let mut pulse = Pulse::new(potion, amount);
        pulse.start(self)?;
        self.emit(json!(["started", pulse.running]));
        self.pulse = Some(pulse);
        Ok(())
    }
    fn open_dialogue(&mut self, potion: Potion) -> Result<()> {
        // sendSkillDialogue with one item: ONE_OPTION.
        self.emit(json!(["open", "ONE_OPTION", [potion.product]]));
        self.handler = Some(potion);
        Ok(())
    }
}

fn tables() -> Value {
    let herbs: Vec<Value> = HERBS
        .iter()
        .map(|h| json!([h.name, h.herb, bits(h.experience), h.level, h.product]))
        .collect();
    let unfinished: Vec<Value> = UNFINISHED
        .iter()
        .map(|p| json!([p.name, p.base, p.ingredient, p.level, p.potion]))
        .collect();
    let finished: Vec<Value> = FINISHED
        .iter()
        .map(|p| {
            json!([
                p.name,
                p.unfinished,
                p.ingredient,
                p.level,
                bits(p.experience),
                p.potion
            ])
        })
        .collect();
    json!([herbs, unfinished, finished])
}

fn number(op: &Value, key: &str) -> i32 {
    op[key].as_i64().unwrap() as i32
}

fn step(world: &mut World, op: &Value) -> Result<Value> {
    match op["op"].as_str().unwrap() {
        "registrations" => {
            let mut all = vec![json!(["on", "ITEM", ["clean"]])];
            for listener in Listener::ALL {
                let (used, with) = listener.ids();
                all.push(json!(["onUseWith", "ITEM", used, with]));
            }
            Ok(Value::Array(all))
        }
        "tables" => Ok(tables()),
        "canMake" => {
            let mut all = Vec::new();
            for potion in &FINISHED {
                all.push(json!(herblore::can_make(world, potion)?));
            }
            Ok(Value::Array(all))
        }
        "set" => {
            for (key, value) in op["state"].as_object().unwrap() {
                world.state.insert(key.clone(), value.clone());
            }
            Ok(Value::Null)
        }
        "clean" => {
            let item = Item {
                id: number(op, "id"),
                slot: number(op, "slot"),
            };
            herblore::clean(world, item.id, &item).map(|b| json!(b))
        }
        "use" => {
            // The declared lookup: the pair as used, then flipped.
            let (used, with) = (number(op, "used"), number(op, "with"));
            for (used, with) in [(used, with), (with, used)] {
                for listener in Listener::ALL {
                    let (first, second) = listener.ids();
                    if first.contains(&used) && second.contains(&with) {
                        return herblore::use_with(world, listener, used, with).map(|b| json!(b));
                    }
                }
            }
            Ok(json!("unhandled"))
        }
        "create" => {
            let Some(potion) = world.handler else {
                return Ok(json!("no handler"));
            };
            herblore::create(world, potion, number(op, "amount"), number(op, "index"))?;
            Ok(Value::Null)
        }
        "getAll" => {
            let Some(potion) = world.handler else {
                return Ok(json!("no handler"));
            };
            herblore::get_all(world, &potion, number(op, "index")).map(|n| json!(n))
        }
        "pulse" => {
            let Some(mut pulse) = world.pulse.take() else {
                return Ok(json!("no pulse"));
            };
            let done = pulse.pulse(world);
            let result = json!([done.clone().unwrap_or(false), pulse.delay, pulse.running]);
            world.pulse = Some(pulse);
            done.map(|_| result)
        }
        "stop" => {
            let Some(mut pulse) = world.pulse.take() else {
                return Ok(json!("no pulse"));
            };
            let stopped = pulse.stop(world);
            let result = json!([pulse.delay, pulse.running]);
            world.pulse = Some(pulse);
            stopped.map(|()| result)
        }
        other => panic!("unknown operation {other}"),
    }
}

/// Replays one case and returns its results in the corpus's form.
pub fn replay(case: &Value, names: &Map<String, Value>) -> Value {
    let mut world = World {
        state: case["state"].as_object().unwrap().clone(),
        names,
        trace: Vec::new(),
        handler: None,
        pulse: None,
        drawn: 0,
    };
    let mut results = Vec::new();
    for op in case["ops"].as_array().unwrap() {
        world.trace.clear();
        let mut result = Map::new();
        match step(&mut world, op) {
            Ok(value) => result.insert("result".into(), value),
            Err(Error::Null) => result.insert("throws".into(), json!("Null")),
            Err(Error::Cast) => result.insert("throws".into(), json!("Cast")),
            Err(Error::Index) => result.insert("throws".into(), json!("Index")),
            Err(other) => panic!("binding failure: {other:?}"),
        };
        result.insert("trace".into(), Value::Array(world.trace.clone()));
        results.push(Value::Object(result));
    }
    Value::Array(results)
}
