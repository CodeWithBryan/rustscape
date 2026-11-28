//! Native replay of the crafting jewellery corpus (jewellery casting).
//!
//! The case's declared state answers every query, exactly as the oracle's
//! recording world does, and every recorded call is logged in the same form;
//! each operation's result and log must equal the original's.
use rs09_server::world::crafting_jewellery::{
    self as jewellery, CRAFTING, Error, FURNACES, GOLD_BAR, INTERFACE, INTERFACE_CLASS, Input,
    JEWELLERY, Jewellery, JewelleryPulse, PLUGIN_CLASS, Result,
};
use serde_json::{Map, Value, json};

fn bits(v: f64) -> String {
    format!("d:{:016x}", v.to_bits())
}

/// The corpus's declared names of the products, in the table's order
/// (oracle/crafting-jewellery/cases.py); the first operation of the corpus
/// compares them with the oracle's.
const NAMES: [&str; 29] = [
    "Gold ring",
    "Sapphire ring",
    "Emerald ring",
    "Ruby ring",
    "Diamond ring",
    "Dragonstone ring",
    "Onyx ring",
    "Gold necklace",
    "Sapphire necklace",
    "Emerald necklace",
    "Ruby necklace",
    "Diamond necklace",
    "Dragon necklace",
    "Onyx necklace",
    "Ring of slaying(8)",
    "Gold amulet",
    "Sapphire amulet",
    "Emerald amulet",
    "Ruby amulet",
    "Diamond amulet",
    "Dragonstone ammy",
    "Onyx amulet",
    "Gold bracelet",
    "Sapphire bracelet",
    "Emerald bracelet",
    "Ruby bracelet",
    "Diamond bracelet",
    "Dragon bracelet",
    "Onyx bracelet",
];

pub struct World {
    state: Map<String, Value>,
    trace: Vec<Value>,
    input: Option<&'static Jewellery>,
    pulse: Option<JewelleryPulse>,
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
}

impl jewellery::Host for World {
    fn open_interface(&mut self, id: i32) -> Result<()> {
        self.emit(json!(["open", id]));
        Ok(())
    }
    fn close_interface(&mut self) -> Result<()> {
        self.emit(json!(["close"]));
        Ok(())
    }
    fn contains(&mut self, id: i32, amount: i32) -> Result<bool> {
        self.emit(json!(["contains", id, amount]));
        Ok(self.held(id) >= i64::from(amount))
    }
    fn amount(&mut self, id: i32) -> Result<i32> {
        self.emit(json!(["getAmount", id]));
        Ok(self.held(id) as i32)
    }
    fn send_interface_config(&mut self, interface: i32, child: i32, hide: bool) -> Result<()> {
        self.emit(json!(["interfaceConfig", interface, child, hide]));
        Ok(())
    }
    fn send_item_zoom(&mut self, item: i32, zoom: i32, interface: i32, child: i32) -> Result<()> {
        self.emit(json!(["itemZoom", item, zoom, interface, child]));
        Ok(())
    }
    fn level(&mut self) -> Result<i32> {
        self.emit(json!(["level", CRAFTING]));
        Ok(self.state["level"].as_i64().unwrap() as i32)
    }
    fn item_name_lowercase(&mut self, id: i32) -> Result<Option<String>> {
        self.emit(json!(["name", id]));
        // The case's own name or null for the id, else the declared table.
        if let Some(renamed) = self.state["renamed"].get(id.to_string()) {
            return Ok(renamed.as_str().map(str::to_lowercase));
        }
        let at = JEWELLERY.iter().position(|j| j.send_item == id);
        Ok(at.map(|at| NAMES[at].to_lowercase()))
    }
    fn send_message(&mut self, message: &str) -> Result<()> {
        self.emit(json!(["message", message]));
        Ok(())
    }
    fn send_messages(&mut self, messages: &[&str]) -> Result<()> {
        self.emit(json!(["messages", messages]));
        Ok(())
    }
    fn ring_unlocked(&mut self) -> Result<bool> {
        self.emit(json!(["ringUnlocked"]));
        Ok(self.state["ring"].as_bool() == Some(true))
    }
    fn send_input(&mut self, jewellery: &'static Jewellery) -> Result<()> {
        self.emit(json!(["input", "NUMERIC", "Enter the amount:"]));
        self.input = Some(jewellery);
        Ok(())
    }
    fn run_pulse(&mut self, mut pulse: JewelleryPulse) -> Result<()> {
        // The pulse as constructed (its node is null), then its start(),
        // as the manager calls it; the pulse is kept whether or not it is
        // running.
        self.emit(json!([
            "run",
            pulse.jewellery.name,
            pulse.amount,
            pulse.ticks,
            true,
            1,
            pulse.running
        ]));
        pulse.start(self)?;
        self.emit(json!(["started", pulse.running]));
        self.pulse = Some(pulse);
        Ok(())
    }
    fn remove_all(&mut self, ids: &[i32]) -> Result<bool> {
        // Each is taken when one is held; the answer is whether every one
        // was.
        let mut all = true;
        let mut rows = Vec::new();
        for &id in ids {
            rows.push(json!([id, 1]));
            let held = self.held(id);
            if held >= 1 {
                self.set_held(id, held - 1);
            } else {
                all = false;
            }
        }
        self.emit(json!(["remove", rows, all]));
        Ok(all)
    }
    fn add(&mut self, id: i32) -> Result<()> {
        let held = self.held(id);
        self.set_held(id, held + 1);
        self.emit(json!(["add", id, 1]));
        Ok(())
    }
    fn add_experience(&mut self, xp: f64) -> Result<()> {
        self.emit(json!(["xp", CRAFTING, bits(xp), true]));
        Ok(())
    }
    fn animate(&mut self, id: i32) -> Result<()> {
        self.emit(json!(["playerAnimate", id, 0, "MID"]));
        Ok(())
    }
    fn reset_animation(&mut self) -> Result<()> {
        self.emit(json!(["playerAnimate", -1, 0, "HIGH"]));
        Ok(())
    }
    fn play_audio(&mut self, id: i32) -> Result<()> {
        self.emit(json!(["audio", id, 0, 1]));
        Ok(())
    }
}

fn registrations() -> Value {
    let mut all = vec![json!(["allowed", PLUGIN_CLASS, [GOLD_BAR]])];
    for furnace in FURNACES {
        all.push(json!(["addHandler", PLUGIN_CLASS, furnace, 2]));
    }
    all.push(json!(["component", INTERFACE, INTERFACE_CLASS]));
    Value::Array(all)
}

fn tables() -> Value {
    JEWELLERY
        .iter()
        .map(|j| {
            // `forProduct(sendItem)`: the first constant of the product.
            let first = JEWELLERY
                .iter()
                .find(|other| other.send_item == j.send_item)
                .unwrap();
            json!([
                j.name,
                j.level,
                bits(j.experience),
                j.component,
                j.send_item,
                j.items,
                jewellery::mould_for(j.name),
                first.name
            ])
        })
        .collect()
}

fn number(op: &Value, key: &str) -> i32 {
    op[key].as_i64().unwrap() as i32
}

fn step(world: &mut World, op: &Value) -> Result<Value> {
    match op["op"].as_str().unwrap() {
        "registrations" => Ok(registrations()),
        "tables" => Ok(tables()),
        "names" => Ok(JEWELLERY
            .iter()
            .zip(NAMES)
            .map(|(j, name)| json!([j.send_item, name]))
            .collect()),
        "set" => {
            for (key, value) in op["state"].as_object().unwrap() {
                world.state.insert(key.clone(), value.clone());
            }
            Ok(Value::Null)
        }
        "handle" => jewellery::handle(world).map(|b| json!(b)),
        "button" => {
            jewellery::button(world, number(op, "opcode"), number(op, "button")).map(|b| json!(b))
        }
        "input" => {
            let Some(data) = world.input else {
                return Ok(json!("no input"));
            };
            let value = match &op["value"] {
                Value::String(text) => Input::Text(text.clone()),
                other => Input::Int(other.as_i64().unwrap() as i32),
            };
            jewellery::input(world, data, &value)?;
            Ok(Value::Null)
        }
        "pulse" => {
            let Some(mut pulse) = world.pulse.take() else {
                return Ok(json!("no pulse"));
            };
            let result = pulse
                .pulse(world)
                .map(|done| json!([done, 1, pulse.running]));
            world.pulse = Some(pulse);
            result
        }
        "stop" => {
            let Some(mut pulse) = world.pulse.take() else {
                return Ok(json!("no pulse"));
            };
            let result = pulse.stop(world).map(|()| json!(pulse.running));
            world.pulse = Some(pulse);
            result
        }
        other => panic!("unknown operation {other}"),
    }
}

/// Replays one case and returns its results in the corpus's form.
pub fn replay(case: &Value) -> Value {
    let mut world = World {
        state: case["state"].as_object().unwrap().clone(),
        trace: Vec::new(),
        input: None,
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
