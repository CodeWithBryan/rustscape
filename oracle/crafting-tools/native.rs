//! Native replay of the crafting tools corpus (gem cutting, limestone,
//! amulet stringing, silver stringing).
//!
//! The case's declared state answers every query, exactly as the oracle's
//! recording world does, and every recorded call is logged in the same form;
//! each operation's result and log must equal the original's.
use rs09_server::world::crafting_tools::{
    self as tools, AMULET_ALLOWED, BALL_OF_WOOL, CHISEL, CRAFTING, Error, GEM_ALLOWED, GEMS, Gem,
    JEWELLERY, LIMESTONE, LIMESTONE_ALLOWED, Pulse, Result, SILVER, SILVER_LAMBDA, STRINGABLE,
};
use serde_json::{Map, Value, json};

fn bits(v: f64) -> String {
    format!("d:{:016x}", v.to_bits())
}

pub struct World {
    state: Map<String, Value>,
    trace: Vec<Value>,
    /// The two items of the event being handled.
    event: (i32, i32),
    /// The gem of the dialogue the case opened last.
    handler: Option<&'static Gem>,
    /// The pulse the case's last run kept.
    pulse: Option<Pulse>,
}
impl World {
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
    /// A removal succeeds when at least one is held.
    fn take(&mut self, id: i32) -> bool {
        let held = self.held(id);
        if held < 1 {
            return false;
        }
        self.set_held(id, held - 1);
        true
    }
    fn give(&mut self, id: i32) {
        let held = self.held(id);
        self.set_held(id, held + 1);
    }
}

impl tools::Host for World {
    fn get_amount(&mut self, id: i32) -> Result<i32> {
        self.emit(json!(["getAmount", id]));
        Ok(self.held(id) as i32)
    }
    fn contains_item(&mut self, id: i32) -> Result<bool> {
        self.emit(json!(["contains", id, 1]));
        Ok(self.held(id) >= 1)
    }
    fn remove(&mut self, id: i32) -> Result<bool> {
        let removed = self.take(id);
        self.emit(json!(["remove", [id], removed]));
        Ok(removed)
    }
    fn remove_event_items(&mut self) -> Result<bool> {
        let (used, base) = self.event;
        let mut all = true;
        for id in [used, base] {
            if !self.take(id) {
                all = false;
            }
        }
        self.emit(json!(["remove", [used, base], all]));
        Ok(all)
    }
    fn add(&mut self, id: i32) -> Result<()> {
        self.give(id);
        self.emit(json!(["add", id, 1]));
        Ok(())
    }
    fn level(&mut self) -> Result<i32> {
        self.emit(json!(["level", CRAFTING]));
        Ok(self.state["level"].as_i64().unwrap() as i32)
    }
    fn message(&mut self, message: &str) -> Result<()> {
        self.emit(json!(["message", message]));
        Ok(())
    }
    fn add_experience(&mut self, xp: f64) -> Result<()> {
        self.emit(json!(["xp", CRAFTING, bits(xp), true]));
        Ok(())
    }
    fn play_audio(&mut self, id: i32) -> Result<()> {
        self.emit(json!(["audio", id, 0, 1]));
        Ok(())
    }
    fn animate(&mut self, id: i32) -> Result<()> {
        self.emit(json!(["animate", id, 0, "MID"]));
        Ok(())
    }
    fn remove_item(&mut self, id: i32) -> Result<bool> {
        let removed = self.take(id);
        self.emit(json!(["removeItem", id, 1, removed]));
        Ok(removed)
    }
    fn add_item(&mut self, id: i32) -> Result<()> {
        self.give(id);
        self.emit(json!(["addItem", id, 1]));
        Ok(())
    }
    fn open_dialogue(&mut self, gem: &'static Gem) -> Result<()> {
        self.emit(json!(["open", "ONE_OPTION", [gem.gem]]));
        self.handler = Some(gem);
        Ok(())
    }
    fn run_pulse(&mut self, mut pulse: Pulse) -> Result<()> {
        let row = match &pulse {
            Pulse::Gem(p) => json!([
                "run",
                "gem",
                p.gem.name,
                p.amount,
                p.gem.uncut,
                1,
                p.running
            ]),
            Pulse::Limestone(p) => json!(["run", "limestone", p.ticks, LIMESTONE, 1, p.running]),
        };
        self.emit(row);
        pulse.start(self)?;
        self.emit(json!(["started", pulse.running()]));
        self.pulse = Some(pulse);
        Ok(())
    }
}

fn registrations() -> Value {
    json!([
        [
            "onUseWith",
            "ITEM",
            SILVER_LAMBDA,
            [BALL_OF_WOOL],
            STRINGABLE
        ],
        [
            "addHandler",
            "AmuletStringingPlugin",
            AMULET_ALLOWED,
            BALL_OF_WOOL,
            0
        ],
        [
            "addHandler",
            "ChiselLimestonePlugin",
            LIMESTONE_ALLOWED,
            CHISEL,
            0
        ],
        ["addHandler", "GemCutPlugin", GEM_ALLOWED, CHISEL, 0]
    ])
}

fn tables() -> Value {
    let gems: Vec<Value> = GEMS
        .iter()
        .map(|g| {
            json!([
                g.name,
                g.uncut,
                1,
                g.gem,
                1,
                g.level,
                g.animation,
                0,
                "MID",
                bits(g.experience)
            ])
        })
        .collect();
    let jewellery: Vec<Value> = JEWELLERY
        .iter()
        .map(|(name, level, send)| {
            // `forProduct(sendItem)`: the constant the map answers with.
            let (found_level, found_send) = tools::jewellery_for_product(*send).unwrap();
            let found = JEWELLERY
                .iter()
                .find(|row| row.1 == found_level && row.2 == found_send)
                .unwrap();
            json!([name, level, send, found.0])
        })
        .collect();
    let silver: Vec<Value> = SILVER
        .iter()
        .map(|(name, product, strung)| {
            let found = tools::silver_strung(*product).unwrap();
            let found = SILVER
                .iter()
                .find(|row| row.1 == *product && row.2 == found)
                .unwrap();
            json!([name, product, strung, found.0])
        })
        .collect();
    json!([gems, jewellery, silver])
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
            world.event = (used, with);
            match op["plugin"].as_str().unwrap() {
                "gem" => tools::gem_handle(world, used, with),
                "limestone" => tools::limestone_handle(world),
                _ => tools::amulet_handle(world, used, with),
            }
            .map(|b| json!(b))
        }
        "use" => {
            // The declared lookup: the pair as used, then flipped.
            let (used, with) = (number(op, "used"), number(op, "with"));
            for (used, with) in [(used, with), (with, used)] {
                if used == BALL_OF_WOOL && STRINGABLE.contains(&with) {
                    return tools::string_silver(world, used, with).map(|b| json!(b));
                }
            }
            Ok(json!("unhandled"))
        }
        "create" => {
            let Some(gem) = world.handler else {
                return Ok(json!("no handler"));
            };
            tools::gem_create(world, gem, number(op, "amount"))?;
            Ok(Value::Null)
        }
        "getAll" => {
            let Some(gem) = world.handler else {
                return Ok(json!("no handler"));
            };
            tools::gem_get_all(world, gem).map(|n| json!(n))
        }
        "pulse" => {
            let Some(mut pulse) = world.pulse.take() else {
                return Ok(json!("no pulse"));
            };
            let done = pulse.pulse(world);
            let result = json!([done.clone().unwrap_or(false), 1, pulse.running()]);
            world.pulse = Some(pulse);
            done.map(|_| result)
        }
        "stop" => {
            let Some(pulse) = world.pulse.as_mut() else {
                return Ok(json!("no pulse"));
            };
            pulse.stop();
            Ok(json!([1, pulse.running()]))
        }
        other => panic!("unknown operation {other}"),
    }
}

/// Replays one case and returns its results in the corpus's form.
pub fn replay(case: &Value) -> Value {
    let mut world = World {
        state: case["state"].as_object().unwrap().clone(),
        trace: Vec::new(),
        event: (0, 0),
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
