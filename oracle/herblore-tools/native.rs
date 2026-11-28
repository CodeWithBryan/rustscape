//! Native replay of the herblore tools corpus (grinding, herb tar,
//! barbarian mixes).
//!
//! The case's declared state answers every query, exactly as the oracle's
//! recording world does, and every recorded call is logged in the same form;
//! each operation's result and log must equal the original's.
use rs09_server::world::herblore_tools::{
    self as tools, Dialogue, Error, GRINDING, HERBLORE, MIXES, PESTLE_AND_MORTAR, Pulse, Result,
    SWAMP_TAR, TARS,
};
use serde_json::{Map, Value, json};

fn bits(v: f64) -> String {
    format!("d:{:016x}", v.to_bits())
}

pub struct World<'a> {
    state: Map<String, Value>,
    names: &'a Map<String, Value>,
    trace: Vec<Value>,
    /// The dialogue the case opened last.
    handler: Option<Dialogue>,
    /// The pulse the case's last run kept.
    pulse: Option<Pulse>,
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
    /// A removal succeeds when at least the amount is held.
    fn take(&mut self, id: i32, amount: i32) -> bool {
        let held = self.held(id);
        if held < i64::from(amount) {
            return false;
        }
        self.set_held(id, held - i64::from(amount));
        true
    }
    fn give(&mut self, id: i32, amount: i32) {
        let held = self.held(id);
        // The adapter's int sum.
        self.set_held(id, i64::from((held as i32).wrapping_add(amount)));
    }
}

impl tools::Host for World<'_> {
    fn amount_in_inventory(&mut self, id: i32) -> Result<i32> {
        self.emit(json!(["amount", id]));
        Ok(self.held(id) as i32)
    }
    fn remove_item(&mut self, id: i32, amount: i32) -> Result<bool> {
        let removed = self.take(id, amount);
        self.emit(json!(["removeItem", id, amount, removed]));
        Ok(removed)
    }
    fn add_item(&mut self, id: i32, amount: i32) -> Result<()> {
        self.give(id, amount);
        self.emit(json!(["addItem", id, amount]));
        Ok(())
    }
    fn animator_animate(&mut self, id: i32) -> Result<()> {
        self.emit(json!(["animator", id]));
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
    fn druidic_ritual_stage(&mut self) -> Result<i32> {
        self.emit(json!(["questStage", "DRUIDIC_RITUAL"]));
        let stage = self.state["quests"].get("DRUIDIC_RITUAL");
        Ok(stage.and_then(Value::as_i64).unwrap_or(0) as i32)
    }
    fn level(&mut self) -> Result<i32> {
        self.emit(json!(["level", HERBLORE]));
        Ok(self.state["level"].as_i64().unwrap() as i32)
    }
    fn static_level(&mut self) -> Result<i32> {
        self.emit(json!(["staticLevel", HERBLORE]));
        Ok(self.state["staticLevel"].as_i64().unwrap() as i32)
    }
    fn message(&mut self, message: &str) -> Result<()> {
        self.emit(json!(["message", message]));
        Ok(())
    }
    fn send_message(&mut self, message: &str) -> Result<()> {
        self.emit(json!(["sendMessage", message]));
        Ok(())
    }
    fn send_dialogue(&mut self, message: &str) -> Result<()> {
        self.emit(json!(["sendDialogue", message]));
        Ok(())
    }
    fn contains_item(&mut self, id: i32, amount: i32) -> Result<bool> {
        self.emit(json!(["contains", id, amount]));
        Ok(self.held(id) >= i64::from(amount))
    }
    fn remove(&mut self, id: i32, amount: i32) -> Result<bool> {
        let removed = self.take(id, amount);
        self.emit(json!(["remove", id, amount, removed]));
        Ok(removed)
    }
    fn add(&mut self, id: i32, amount: i32) -> Result<()> {
        self.give(id, amount);
        self.emit(json!(["add", id, amount]));
        Ok(())
    }
    fn get_amount(&mut self, id: i32) -> Result<i32> {
        self.emit(json!(["getAmount", id]));
        Ok(self.held(id) as i32)
    }
    fn add_experience(&mut self, xp: f64) -> Result<()> {
        self.emit(json!(["xp", HERBLORE, bits(xp), true]));
        Ok(())
    }
    fn reward_xp(&mut self, xp: f64) -> Result<()> {
        self.emit(json!(["xp", HERBLORE, bits(xp)]));
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
    fn attribute_int(&mut self, key: &str) -> Result<i32> {
        self.emit(json!(["attribute", key]));
        match self.state["attributes"].get(key) {
            None | Some(Value::Null) => Ok(0),
            Some(Value::Number(value)) => Ok(value.as_i64().unwrap() as i32),
            // The Kotlin read's cast to a number.
            Some(_) => Err(Error::Cast),
        }
    }
    fn set_attribute_int(&mut self, key: &str, value: i32) -> Result<()> {
        self.emit(json!(["setAttribute", key, value]));
        let attributes = self.state["attributes"].as_object_mut().unwrap();
        attributes.insert(key.into(), json!(value));
        Ok(())
    }
    fn open_dialogue(&mut self, dialogue: Dialogue) -> Result<()> {
        self.emit(json!(["open", "ONE_OPTION", [dialogue.item()]]));
        self.handler = Some(dialogue);
        Ok(())
    }
    fn reposition_child(&mut self, interface: i32, child: i32, x: i32, y: i32) -> Result<()> {
        self.emit(json!(["send", "RepositionChild", interface, child, x, y]));
        Ok(())
    }
    fn run_pulse(&mut self, mut pulse: Pulse) -> Result<()> {
        let row = match &pulse {
            Pulse::Grind(p) => json!(["run", "grind", p.node, p.amt, p.delay, p.running]),
            Pulse::Tar(p) => json!([
                "run",
                "tar",
                p.tar.map(|t| t.name),
                p.amount,
                p.delay,
                p.running
            ]),
        };
        self.emit(row);
        pulse.start(self)?;
        self.emit(json!(["started", pulse.running()]));
        self.pulse = Some(pulse);
        Ok(())
    }
}

fn registrations() -> Value {
    let mut all = Vec::new();
    for (lambda, used, with) in tools::mix_registrations() {
        all.push(json!(["onUseWith", "ITEM", lambda, [used], [with]]));
    }
    for id in tools::grind_handler_ids() {
        all.push(json!([
            "addHandler",
            "GrindItemPlugin",
            [PESTLE_AND_MORTAR],
            id,
            0
        ]));
    }
    for id in tools::tar_handler_ids() {
        all.push(json!(["addHandler", "HerbTarPlugin", [SWAMP_TAR], id, 0]));
    }
    Value::Array(all)
}

fn tables() -> Value {
    let grinding: Vec<Value> = GRINDING
        .iter()
        .map(|g| json!([g.name, [[g.item, 1]], g.product, 1]))
        .collect();
    let tars: Vec<Value> = TARS
        .iter()
        .map(|t| {
            json!([
                t.name,
                t.ingredient,
                1,
                t.level,
                bits(t.experience),
                t.tar,
                1
            ])
        })
        .collect();
    let mixes: Vec<Value> = MIXES
        .iter()
        .map(|m| {
            json!([
                m.name,
                m.item,
                m.level,
                m.product,
                bits(m.experience),
                m.both
            ])
        })
        .collect();
    json!([grinding, tars, mixes])
}

fn number(op: &Value, key: &str) -> i32 {
    op[key].as_i64().unwrap() as i32
}

fn step(world: &mut World, op: &Value) -> Result<Value> {
    match op["op"].as_str().unwrap() {
        "registrations" => Ok(registrations()),
        "tables" => Ok(tables()),
        "set" => {
            for (key, value) in op["state"].as_object().unwrap() {
                world.state.insert(key.clone(), value.clone());
            }
            Ok(Value::Null)
        }
        "handle" => {
            let (used, with) = (number(op, "used"), number(op, "with"));
            match op["plugin"].as_str().unwrap() {
                "grind" => tools::grind_handle(world, used, with),
                _ => tools::tar_handle(world, used, with),
            }
            .map(|b| json!(b))
        }
        "use" => {
            // The declared lookup: the pair as used, then flipped.
            let (used, with) = (number(op, "used"), number(op, "with"));
            for (used, with) in [(used, with), (with, used)] {
                let registered = tools::mix_registrations();
                if registered.iter().any(|r| r.1 == used && r.2 == with) {
                    return tools::mix(world, used, with).map(|b| json!(b));
                }
            }
            Ok(json!("unhandled"))
        }
        "mix" => tools::mix(world, number(op, "used"), number(op, "with")).map(|b| json!(b)),
        "create" => {
            let Some(dialogue) = world.handler else {
                return Ok(json!("no handler"));
            };
            tools::create(world, dialogue, number(op, "amount"))?;
            Ok(Value::Null)
        }
        "getAll" => {
            let Some(dialogue) = world.handler else {
                return Ok(json!("no handler"));
            };
            tools::get_all(world, dialogue).map(|n| json!(n))
        }
        "pulse" => {
            let Some(mut pulse) = world.pulse.take() else {
                return Ok(json!("no pulse"));
            };
            let done = pulse.pulse(world);
            let result = json!([
                done.clone().unwrap_or(false),
                pulse.delay(),
                pulse.running()
            ]);
            world.pulse = Some(pulse);
            done.map(|_| result)
        }
        "stop" => {
            let Some(mut pulse) = world.pulse.take() else {
                return Ok(json!("no pulse"));
            };
            let stopped = pulse.stop(world);
            let result = json!([pulse.delay(), pulse.running()]);
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
    };
    let mut results = Vec::new();
    for op in case["ops"].as_array().unwrap() {
        world.trace.clear();
        let mut result = Map::new();
        match step(&mut world, op) {
            Ok(value) => result.insert("result".into(), value),
            Err(Error::Null) => result.insert("throws".into(), json!("Null")),
            Err(Error::Cast) => result.insert("throws".into(), json!("Cast")),
            Err(other) => panic!("binding failure: {other:?}"),
        };
        result.insert("trace".into(), Value::Array(world.trace.clone()));
        results.push(Value::Object(result));
    }
    Value::Array(results)
}
