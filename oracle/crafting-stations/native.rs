//! Native replay of the crafting stations corpus (spinning and silver
//! casting).
//!
//! The case's declared state answers every query, exactly as the oracle's
//! recording world does, and every recorded call is logged in the same form;
//! each operation's result and log must equal the original's.
use rs09_server::world::crafting_stations::{
    self as stations, BOWSTRINGS, BOWSTRINGS_SAVED, CRAFTING, Error, FURNACE_ATTRIBUTE, Input,
    Result, SEERS_WHEEL, SILVER, SILVER_BAR, SILVER_BUTTON_LAMBDA, SILVER_CLASS, SILVER_FURNACES,
    SILVER_INTERFACE, SILVER_OPEN_LAMBDA, SILVER_USE_LAMBDA, SPIN_CLASS, SPIN_INTERFACE, SPINNING,
    Silver, SilverPulse, Spin, SpinPulse, WHEEL_CLASS, WHEELS,
};
use serde_json::{Map, Value, json};

fn bits(v: f64) -> String {
    format!("d:{:016x}", v.to_bits())
}

/// A furnace: its id and coordinates.
type Furnace = [i32; 4];

/// What the trace shows of a pulse's furnace: the attribute's scenery, or
/// the listener's fallback `Scenery(-1, -1, 0)` of an id, a type and a
/// rotation.
fn shown(furnace: &Option<Furnace>) -> Value {
    match furnace {
        Some(f) => json!(f),
        None => json!(["typed", -1, -1, 0]),
    }
}

/// The classes whose calls are being recorded. The spinning classes call
/// the player's own managers and the silver classes ContentAPI: the
/// recording world logs the two under different names.
#[derive(Clone, Copy, PartialEq)]
enum Caller {
    Spinning,
    Silver,
}

/// The handler of the case's last amount prompt.
#[derive(Clone, Copy)]
enum Prompt {
    Spin(&'static Spin),
    Silver(&'static Silver),
}

/// The pulse the case's last run kept; the silver pulse with
/// `Pulse.running`.
enum Pulse {
    Spin(SpinPulse),
    Silver(SilverPulse<Furnace>, bool),
}

pub struct World {
    state: Map<String, Value>,
    trace: Vec<Value>,
    caller: Caller,
    /// The attribute store's two keys.
    bowstrings: Option<i32>,
    furnace: Option<Furnace>,
    input: Option<Prompt>,
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
    fn flag(&self, key: &str) -> bool {
        self.state[key].as_bool() == Some(true)
    }
    fn spinning(&self) -> bool {
        self.caller == Caller::Spinning
    }
}

impl stations::Models for World {
    fn send_item_on_interface(&mut self, interface: i32, child: i32, item: i32) -> Result<()> {
        self.emit(json!(["itemOnInterface", interface, child, item, 1]));
        Ok(())
    }
}

impl stations::Host for World {
    type Furnace = Furnace;

    fn open_interface(&mut self, id: i32) -> Result<()> {
        let name = if self.spinning() {
            "open"
        } else {
            "openInterface"
        };
        self.emit(json!([name, id]));
        Ok(())
    }
    fn close_interface(&mut self) -> Result<()> {
        let name = if self.spinning() {
            "close"
        } else {
            "closeInterface"
        };
        self.emit(json!([name]));
        Ok(())
    }
    fn contains(&mut self, id: i32) -> Result<bool> {
        let name = if self.spinning() {
            "contains"
        } else {
            "inInventory"
        };
        self.emit(json!([name, id, 1]));
        Ok(self.held(id) >= 1)
    }
    fn amount(&mut self, id: i32) -> Result<i32> {
        let name = if self.spinning() {
            "getAmount"
        } else {
            "amount"
        };
        self.emit(json!([name, id]));
        Ok(self.held(id) as i32)
    }
    fn item_name(&mut self, id: i32) -> Result<String> {
        // The adapter's declared name of the id.
        self.emit(json!(["name", id]));
        Ok(format!("Name Of {id}"))
    }
    fn send_message(&mut self, message: &str) -> Result<()> {
        let name = if self.spinning() {
            "message"
        } else {
            "sendMessage"
        };
        self.emit(json!([name, message]));
        Ok(())
    }
    fn level(&mut self) -> Result<i32> {
        self.emit(json!(["level", CRAFTING]));
        Ok(self.state["level"].as_i64().unwrap() as i32)
    }
    fn animate(&mut self, id: i32) -> Result<()> {
        if self.spinning() {
            self.emit(json!(["playerAnimate", id, 0, "MID"]));
        } else {
            self.emit(json!(["animate", id, false]));
        }
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
    fn seers_hard_complete(&mut self) -> Result<bool> {
        self.emit(json!(["isComplete", "SEERS_VILLAGE", 2]));
        Ok(self.flag("seersHard"))
    }
    fn near_seers_wheel(&mut self) -> Result<bool> {
        let [x, y, z] = SEERS_WHEEL;
        self.emit(json!(["withinDistance", x, y, z]));
        Ok(self.flag("near"))
    }
    fn hat(&mut self) -> Result<Option<i32>> {
        // The original asks the slot once, and again for the id of a hat
        // that is worn.
        let hat = self.state["hat"].as_i64().map(|id| id as i32);
        self.emit(json!(["equipment", 0]));
        if hat.is_some() {
            self.emit(json!(["equipment", 0]));
        }
        Ok(hat)
    }
    fn remove(&mut self, id: i32) -> Result<bool> {
        // A removal succeeds when at least one is held.
        let held = self.held(id);
        let removed = held >= 1;
        if removed {
            self.set_held(id, held - 1);
        }
        if self.spinning() {
            self.emit(json!(["remove", id, 1, removed]));
        } else {
            self.emit(json!(["removeItem", id, 1, "INVENTORY", removed]));
        }
        Ok(removed)
    }
    fn add(&mut self, id: i32) -> Result<()> {
        let held = self.held(id);
        self.set_held(id, held + 1);
        self.emit(json!(["add", id, 1]));
        Ok(())
    }
    fn add_item(&mut self, id: i32, amount: i32) -> Result<bool> {
        // An addition succeeds unless the case declares the inventory full.
        let added = !self.flag("full");
        if added {
            let held = self.held(id);
            self.set_held(id, held + i64::from(amount));
        }
        self.emit(json!(["addItem", id, amount, "INVENTORY", added]));
        Ok(added)
    }
    fn add_experience(&mut self, xp: f64) -> Result<()> {
        self.emit(json!(["xp", CRAFTING, bits(xp), true]));
        Ok(())
    }
    fn reward_xp(&mut self, xp: f64) -> Result<()> {
        self.emit(json!(["xp", CRAFTING, bits(xp)]));
        Ok(())
    }
    fn region_id(&mut self) -> Result<i32> {
        self.emit(json!(["regionId"]));
        Ok(self.state["region"].as_i64().unwrap() as i32)
    }
    fn bowstring_task_complete(&mut self) -> Result<bool> {
        self.emit(json!(["isComplete", "SEERS_VILLAGE", 0, 4]));
        Ok(self.flag("taskDone"))
    }
    fn bowstrings(&mut self) -> Result<i32> {
        self.emit(json!([
            "getAttribute",
            BOWSTRINGS,
            self.bowstrings.is_none()
        ]));
        Ok(self.bowstrings.unwrap_or(0))
    }
    fn set_bowstrings(&mut self, value: i32) -> Result<()> {
        self.emit(json!(["setAttribute", BOWSTRINGS_SAVED, value]));
        self.bowstrings = Some(value);
        Ok(())
    }
    fn finish_bowstring_task(&mut self) -> Result<()> {
        self.emit(json!(["diary", true, "SEERS_VILLAGE", 0, 4]));
        Ok(())
    }
    fn send_spin_input(&mut self, spin: &'static Spin) -> Result<()> {
        self.emit(json!(["input", "NUMERIC", "Enter the amount:"]));
        self.input = Some(Prompt::Spin(spin));
        Ok(())
    }
    fn run_spin_pulse(&mut self, mut pulse: SpinPulse) -> Result<()> {
        // The pulse as constructed, then its start(), as the manager
        // calls it; the pulse is kept whether or not it is running.
        self.emit(json!([
            "run",
            "spin",
            pulse.spin.name,
            pulse.amount,
            pulse.ticks,
            pulse.spin.need,
            1,
            1,
            pulse.running
        ]));
        pulse.start(self)?;
        self.emit(json!(["started", pulse.running]));
        self.pulse = Some(Pulse::Spin(pulse));
        Ok(())
    }
    fn set_furnace(&mut self, furnace: &Furnace) -> Result<()> {
        self.emit(json!(["setAttribute", FURNACE_ATTRIBUTE, furnace]));
        self.furnace = Some(*furnace);
        Ok(())
    }
    fn furnace(&mut self) -> Result<Option<Furnace>> {
        self.emit(json!([
            "getAttribute",
            FURNACE_ATTRIBUTE,
            self.furnace.is_none()
        ]));
        Ok(self.furnace)
    }
    fn send_silver_input(&mut self, silver: &'static Silver) -> Result<()> {
        self.emit(json!(["input", "AMOUNT", "Enter the amount:"]));
        self.input = Some(Prompt::Silver(silver));
        Ok(())
    }
    fn submit_silver_pulse(&mut self, pulse: SilverPulse<Furnace>) -> Result<()> {
        // A Pulse is constructed running; Pulse.start() keeps it so.
        self.emit(json!([
            "run",
            "silver",
            pulse.silver.name,
            shown(&pulse.furnace),
            pulse.amount,
            true,
            pulse.delay,
            true
        ]));
        self.emit(json!(["started", true]));
        self.pulse = Some(Pulse::Silver(pulse, true));
        Ok(())
    }
    fn resource_produced(&mut self, id: i32, amount: i32, furnace: &Option<Furnace>) -> Result<()> {
        self.emit(json!(["produced", id, amount, shown(furnace), SILVER_BAR]));
        Ok(())
    }
}

fn registrations() -> Value {
    let mut all: Vec<Value> = WHEELS
        .iter()
        .map(|id| json!(["option", id, "option:spin", WHEEL_CLASS]))
        .collect();
    all.push(json!(["component", SPIN_INTERFACE, SPIN_CLASS]));
    all.push(json!([
        "onUseWith",
        "SCENERY",
        SILVER_USE_LAMBDA,
        [SILVER_BAR],
        SILVER_FURNACES
    ]));
    all.push(json!(["onOpen", SILVER_INTERFACE, SILVER_OPEN_LAMBDA]));
    all.push(json!(["on", SILVER_INTERFACE, SILVER_BUTTON_LAMBDA]));
    assert!(SILVER_USE_LAMBDA.starts_with(SILVER_CLASS));
    Value::Array(all)
}

fn tables() -> Value {
    let spins: Vec<Value> = SPINNING
        .iter()
        .map(|s| {
            json!([
                s.name,
                s.button,
                s.need,
                s.product,
                s.level,
                bits(s.experience),
                stations::spin_for_button(s.button).unwrap().name
            ])
        })
        .collect();
    let silver: Vec<Value> = SILVER
        .iter()
        .map(|s| {
            json!([
                s.name,
                s.button,
                s.required,
                s.product,
                s.amount,
                s.level,
                bits(s.experience),
                stations::silver_for_button(s.button).unwrap().name
            ])
        })
        .collect();
    json!([spins, silver])
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
                if key == "bowstrings" {
                    world.bowstrings = value.as_i64().map(|n| n as i32);
                } else {
                    world.state.insert(key.clone(), value.clone());
                }
            }
            Ok(Value::Null)
        }
        "wheel" => {
            world.caller = Caller::Spinning;
            // getDestination answers null for every node.
            stations::wheel_handle(world).map(|b| json!([b, true]))
        }
        "spinButton" => {
            world.caller = Caller::Spinning;
            stations::spin_button(world, number(op, "opcode"), number(op, "button"))
                .map(|b| json!(b))
        }
        "use" => {
            world.caller = Caller::Silver;
            let with = number(op, "with");
            if number(op, "used") != SILVER_BAR || !SILVER_FURNACES.contains(&with) {
                return Ok(json!("unhandled"));
            }
            stations::silver_use(world, &[with, number(op, "x"), number(op, "y"), 0])
                .map(|b| json!(b))
        }
        "open" => {
            world.caller = Caller::Silver;
            if number(op, "component") != SILVER_INTERFACE {
                return Ok(json!("unhandled"));
            }
            stations::silver_open(world).map(|b| json!(b))
        }
        "button" => {
            world.caller = Caller::Silver;
            if number(op, "component") != SILVER_INTERFACE {
                return Ok(json!("unhandled"));
            }
            stations::silver_button(world, number(op, "opcode"), number(op, "button"))
                .map(|b| json!(b))
        }
        "input" => {
            let Some(prompt) = world.input else {
                return Ok(json!("no input"));
            };
            let value = match &op["value"] {
                Value::String(text) => Input::Text(text.clone()),
                other => Input::Int(other.as_i64().unwrap() as i32),
            };
            match prompt {
                Prompt::Spin(spin) => {
                    world.caller = Caller::Spinning;
                    stations::spin_input(world, spin, &value)?;
                }
                Prompt::Silver(silver) => {
                    world.caller = Caller::Silver;
                    stations::silver_input(world, silver, &value)?;
                }
            }
            Ok(Value::Null)
        }
        "pulse" => {
            let Some(mut pulse) = world.pulse.take() else {
                return Ok(json!("no pulse"));
            };
            let result = match &mut pulse {
                Pulse::Spin(p) => {
                    world.caller = Caller::Spinning;
                    p.pulse(world).map(|done| json!([done, 1, p.running]))
                }
                Pulse::Silver(p, running) => {
                    world.caller = Caller::Silver;
                    p.pulse(world).map(|done| json!([done, p.delay, *running]))
                }
            };
            world.pulse = Some(pulse);
            result
        }
        "stop" => {
            let Some(mut pulse) = world.pulse.take() else {
                return Ok(json!("no pulse"));
            };
            let result = match &mut pulse {
                Pulse::Spin(p) => {
                    world.caller = Caller::Spinning;
                    p.stop(world).map(|()| json!(p.running))
                }
                // Pulse.stop() only clears the flag.
                Pulse::Silver(_, running) => {
                    *running = false;
                    Ok(json!(false))
                }
            };
            world.pulse = Some(pulse);
            result
        }
        other => panic!("unknown operation {other}"),
    }
}

/// Replays one case and returns its results in the corpus's form.
pub fn replay(case: &Value) -> Value {
    let state = case["state"].as_object().unwrap().clone();
    let mut world = World {
        bowstrings: state["bowstrings"].as_i64().map(|n| n as i32),
        state,
        trace: Vec::new(),
        caller: Caller::Spinning,
        furnace: None,
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
            Err(Error::Number) => result.insert("throws".into(), json!("Number")),
            Err(other) => panic!("binding failure: {other:?}"),
        };
        result.insert("trace".into(), Value::Array(world.trace.clone()));
        results.push(Value::Object(result));
    }
    Value::Array(results)
}
