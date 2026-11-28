//! Native replay of the crafting wares corpus (battlestaves, glassblowing,
//! molten glass, spiky vambraces).
//!
//! The case's declared state answers every query, exactly as the oracle's
//! recording world does, and every recorded call is logged in the same form;
//! each operation's result and log must equal the original's.
use rs09_server::world::crafting_wares::{
    self as wares, BATTLESTAFF, BATTLESTAFF_LAMBDA, BATTLESTAVES, Battlestaff, CRAFTING, Error,
    FURNACES, GLASS, GLASS_BUTTON_LAMBDA, GLASS_INTERFACE, GLASS_LAMBDA, GLASS_MAKE_INPUTS,
    GLASS_MAKE_LAMBDA, GLASSBLOWING_PIPE, Glass, GlassMakePulse, GlassPulse, KEBBIT_CLAWS,
    MOLTEN_GLASS, ORBS, Result, VAMBRACES,
};
use serde_json::{Map, Value, json};

fn bits(v: f64) -> String {
    format!("d:{:016x}", v.to_bits())
}

/// The dialogue the case opened last.
#[derive(Clone, Copy)]
enum Dialogue {
    Staff(&'static Battlestaff, i32, i32),
    GlassMake,
}

/// The pulse the case's last run kept.
enum Pulse {
    /// `runTask`'s pulse: its delay and its cycles, with the task's
    /// product and amount.
    Task(&'static Battlestaff, i32, i32),
    Glass(GlassPulse),
    GlassMake(GlassMakePulse),
}

pub struct World {
    state: Map<String, Value>,
    trace: Vec<Value>,
    handler: Option<Dialogue>,
    input: Option<&'static Glass>,
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
}

impl wares::Host for World {
    fn amount_in_inventory(&mut self, id: i32) -> Result<i32> {
        self.emit(json!(["amount", id]));
        Ok(self.held(id) as i32)
    }
    fn in_inventory(&mut self, id: i32) -> Result<bool> {
        self.emit(json!(["inInventory", id, 1]));
        Ok(self.held(id) >= 1)
    }
    fn level(&mut self) -> Result<i32> {
        self.emit(json!(["level", CRAFTING]));
        Ok(self.state["level"].as_i64().unwrap() as i32)
    }
    fn send_message(&mut self, message: &str) -> Result<()> {
        self.emit(json!(["sendMessage", message]));
        Ok(())
    }
    fn remove_item(&mut self, id: i32) -> Result<bool> {
        // A removal succeeds when at least one is held.
        let held = self.held(id);
        let removed = held >= 1;
        if removed {
            self.set_held(id, held - 1);
        }
        self.emit(json!(["removeItem", id, 1, "INVENTORY", removed]));
        Ok(removed)
    }
    fn add_item(&mut self, id: i32, amount: i32) -> Result<()> {
        let held = self.held(id);
        self.set_held(id, held + i64::from(amount));
        self.emit(json!(["addItem", id, amount, "INVENTORY"]));
        Ok(())
    }
    fn reward_xp(&mut self, xp: f64) -> Result<()> {
        self.emit(json!(["xp", CRAFTING, bits(xp)]));
        Ok(())
    }
    fn finish_varrock_task(&mut self) -> Result<()> {
        self.emit(json!(["diary", true, "VARROCK", 2, 6]));
        Ok(())
    }
    fn animate(&mut self, id: i32) -> Result<()> {
        self.emit(json!(["animate", id, false]));
        Ok(())
    }
    fn play_audio(&mut self, id: i32) -> Result<()> {
        self.emit(json!(["audio", id, 0, 1]));
        Ok(())
    }
    fn resource_produced(&mut self, id: i32, amount: i32) -> Result<()> {
        self.emit(json!(["produced", id, amount, true, -1]));
        Ok(())
    }
    fn open_interface(&mut self, id: i32) -> Result<()> {
        self.emit(json!(["openInterface", id]));
        Ok(())
    }
    fn close_interface(&mut self) -> Result<()> {
        self.emit(json!(["closeInterface"]));
        Ok(())
    }
    fn open_battlestaff_dialogue(
        &mut self,
        staff: &'static Battlestaff,
        used: i32,
        with: i32,
    ) -> Result<()> {
        self.emit(json!(["open", "ONE_OPTION", [staff.product]]));
        self.handler = Some(Dialogue::Staff(staff, used, with));
        Ok(())
    }
    fn run_battlestaff_task(
        &mut self,
        staff: &'static Battlestaff,
        repeats: i32,
        amount: i32,
    ) -> Result<()> {
        self.emit(json!(["run", "task", "core.api.ContentAPIKt$runTask$1", 2]));
        self.pulse = Some(Pulse::Task(staff, repeats, amount));
        Ok(())
    }
    fn send_glass_input(&mut self, glass: &'static Glass) -> Result<()> {
        self.emit(json!(["input", "AMOUNT", "Enter the amount:"]));
        self.input = Some(glass);
        Ok(())
    }
    fn submit_glass_pulse(&mut self, pulse: GlassPulse) -> Result<()> {
        self.emit(json!([
            "run",
            "glass",
            pulse.glass.name,
            pulse.amount,
            true,
            pulse.delay
        ]));
        self.pulse = Some(Pulse::Glass(pulse));
        Ok(())
    }
    fn open_glass_make_dialogue(&mut self) -> Result<()> {
        self.emit(json!(["open", "ONE_OPTION", [MOLTEN_GLASS]]));
        self.handler = Some(Dialogue::GlassMake);
        Ok(())
    }
    fn submit_glass_make_pulse(&mut self, pulse: GlassMakePulse) -> Result<()> {
        self.emit(json!([
            "run",
            "glassMake",
            pulse.product,
            pulse.amount,
            true,
            pulse.delay
        ]));
        self.pulse = Some(Pulse::GlassMake(pulse));
        Ok(())
    }
}

fn registrations() -> Value {
    let mut all = vec![
        json!(["onUseWith", "ITEM", BATTLESTAFF_LAMBDA, ORBS, [BATTLESTAFF]]),
        json!([
            "onUseWith",
            "ITEM",
            GLASS_LAMBDA,
            [GLASSBLOWING_PIPE],
            [MOLTEN_GLASS]
        ]),
        json!([
            "onUseWith",
            "SCENERY",
            GLASS_MAKE_LAMBDA,
            GLASS_MAKE_INPUTS,
            FURNACES
        ]),
        json!(["on", GLASS_INTERFACE, GLASS_BUTTON_LAMBDA]),
        json!(["allowed", "SpikyVambraces", [KEBBIT_CLAWS]]),
    ];
    for (vamb, ..) in VAMBRACES {
        all.push(json!(["addHandler", "SpikyVambraces", vamb, 0]));
    }
    Value::Array(all)
}

fn tables() -> Value {
    let staves: Vec<Value> = BATTLESTAVES
        .iter()
        .map(|s| {
            json!([
                s.name,
                s.orb,
                s.product,
                s.amount,
                s.level,
                bits(s.experience),
                wares::battlestaff_for_orb(s.orb).unwrap().name
            ])
        })
        .collect();
    let glass: Vec<Value> = GLASS
        .iter()
        .map(|g| {
            json!([
                g.name,
                g.button,
                g.product,
                g.amount,
                g.level,
                bits(g.experience),
                wares::glass_for_button(g.button).unwrap().name
            ])
        })
        .collect();
    json!([staves, glass])
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
        "handle" => wares::vambraces_handle(world, number(op, "with")).map(|b| json!(b)),
        "use" => {
            // The declared lookup: the pair as used, then flipped, each
            // over the three registrations in order.
            let (used, with) = (number(op, "used"), number(op, "with"));
            for (used, with) in [(used, with), (with, used)] {
                if ORBS.contains(&used) && with == BATTLESTAFF {
                    return wares::battlestaff_use(world, used, with).map(|b| json!(b));
                }
                if used == GLASSBLOWING_PIPE && with == MOLTEN_GLASS {
                    return wares::glass_use(world).map(|b| json!(b));
                }
                if GLASS_MAKE_INPUTS.contains(&used) && FURNACES.contains(&with) {
                    return wares::glass_make_use(world).map(|b| json!(b));
                }
            }
            Ok(json!("unhandled"))
        }
        "button" => {
            if number(op, "component") != GLASS_INTERFACE {
                return Ok(json!("unhandled"));
            }
            wares::glass_button(world, number(op, "opcode"), number(op, "button")).map(|b| json!(b))
        }
        "input" => {
            let Some(glass) = world.input else {
                return Ok(json!("no input"));
            };
            wares::glass_input(world, glass, number(op, "value"))?;
            Ok(Value::Null)
        }
        "create" => {
            let amount = number(op, "amount");
            match world.handler {
                None => return Ok(json!("no handler")),
                Some(Dialogue::Staff(staff, used, with)) => {
                    wares::battlestaff_create(world, staff, used, with, amount)?
                }
                // `builder.items[index].id`: the molten glass.
                Some(Dialogue::GlassMake) => wares::glass_make_create(world, MOLTEN_GLASS, amount)?,
            }
            Ok(Value::Null)
        }
        "getAll" => match world.handler {
            None => Ok(json!("no handler")),
            Some(Dialogue::Staff(_, used, with)) => {
                wares::battlestaff_max(world, used, with).map(|n| json!(n))
            }
            Some(Dialogue::GlassMake) => wares::glass_make_max(world).map(|n| json!(n)),
        },
        "pulse" => {
            let Some(mut pulse) = world.pulse.take() else {
                return Ok(json!("no pulse"));
            };
            let result = match &mut pulse {
                Pulse::Task(staff, cycles, amount) => {
                    // `task.invoke()`, then `--cycles <= 0`.
                    wares::battlestaff_task(world, staff, *amount).map(|()| {
                        *cycles = cycles.wrapping_sub(1);
                        json!([*cycles <= 0, 2])
                    })
                }
                Pulse::Glass(p) => p.pulse(world).map(|done| json!([done, p.delay])),
                Pulse::GlassMake(p) => p.pulse(world).map(|done| json!([done, p.delay])),
            };
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
        handler: None,
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
