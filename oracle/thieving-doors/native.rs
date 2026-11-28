//! Native replay of the pickable door corpus.
//!
//! The case's declared state answers every query, exactly as the oracle's
//! recording world does, and every recorded call is logged in the same form;
//! each operation's result and log must equal the original's.
use rs09_game_core::direction::Direction;
use rs09_server::world::thieving_door::{self as doors, Error, PICKABLE, Result};
use serde_json::{Map, Value, json};

fn bits(v: f64) -> String {
    format!("d:{:016x}", v.to_bits())
}

fn tile(value: &Value) -> [i32; 3] {
    let at = value.as_array().unwrap();
    [0, 1, 2].map(|n| at[n].as_i64().unwrap() as i32)
}

pub struct World {
    state: Map<String, Value>,
    trace: Vec<Value>,
    /// The scenery of the operation: its tile and rotation.
    at: [i32; 3],
    rotation: i32,
    drawn: usize,
}
impl World {
    fn flag(&self, key: &str) -> bool {
        self.state[key] == Value::Bool(true)
    }
    fn emit(&mut self, row: Value) {
        self.trace.push(row);
    }
}

impl doors::Host for World {
    fn object_location(&mut self) -> Result<[i32; 3]> {
        Ok(self.at)
    }
    fn object_direction(&mut self) -> Result<Direction> {
        // Scenery's constructor: Direction.get(rotation).
        Ok(Direction::from_code(self.rotation).unwrap())
    }
    fn player_location(&mut self) -> Result<[i32; 3]> {
        Ok(tile(&self.state["player"]))
    }
    fn message(&mut self, message: &str) -> Result<()> {
        self.emit(json!(["message", message]));
        Ok(())
    }
    fn send_message(&mut self, message: &str) -> Result<()> {
        self.emit(json!(["sendMessage", message]));
        Ok(())
    }
    fn handle_autowalk_door(&mut self) -> Result<()> {
        self.emit(json!(["autowalk"]));
        Ok(())
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
    fn level(&mut self) -> Result<i32> {
        // Skills.THIEVING.
        self.emit(json!(["level", 17]));
        Ok(self.state["level"].as_i64().unwrap() as i32)
    }
    fn manual_hit(&mut self, hit: i32) -> Result<()> {
        self.emit(json!(["manualHit", hit, "NORMAL"]));
        Ok(())
    }
    fn has_lock_pick(&mut self) -> Result<bool> {
        self.emit(json!(["containsItem", doors::LOCK_PICK]));
        Ok(self.flag("lockpick"))
    }
    fn add_experience(&mut self, xp: f64) -> Result<()> {
        self.emit(json!(["xp", 17, bits(xp), true]));
        Ok(())
    }
    fn has_action(&mut self, action: &str) -> Result<bool> {
        self.emit(json!(["hasAction", action]));
        Ok(self.flag("hasAction"))
    }
    fn door_destination(&mut self) -> Result<[i32; 3]> {
        self.emit(json!(["doorDestination"]));
        Ok(tile(&self.state["destination"]))
    }
}

fn table() -> Value {
    PICKABLE
        .iter()
        .map(|d| {
            json!({
                "locations": [d.location],
                "level": d.level,
                "experience": bits(d.experience),
                "lockpick": d.lockpick,
                "flipped": d.flipped,
            })
        })
        .collect()
}

fn step(world: &mut World, op: &Value) -> Result<Value> {
    if op.get("at").is_some() {
        world.at = tile(&op["at"]);
        world.rotation = op["rotation"].as_i64().unwrap() as i32;
    }
    match op["op"].as_str().unwrap() {
        "registrations" => Ok(doors::DOORS
            .iter()
            .flat_map(|id| doors::KEYS.iter().map(move |key| json!([id, key])))
            .collect()),
        "doors" => Ok(table()),
        "set" => {
            for (key, value) in op["state"].as_object().unwrap() {
                world.state.insert(key.clone(), value.clone());
            }
            Ok(Value::Null)
        }
        "handle" => doors::handle(world, op["option"].as_str().unwrap()).map(|b| json!(b)),
        "destination" => Ok(doors::destination(world)?.map_or(Value::Null, |at| json!(at))),
        other => panic!("unknown operation {other}"),
    }
}

/// Replays one case and returns its results in the corpus's form.
pub fn replay(case: &Value) -> Value {
    let mut world = World {
        state: case["state"].as_object().unwrap().clone(),
        trace: Vec::new(),
        at: [0; 3],
        rotation: 0,
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
            Err(other) => panic!("binding failure: {other:?}"),
        };
        result.insert("trace".into(), Value::Array(world.trace.clone()));
        results.push(Value::Object(result));
    }
    Value::Array(results)
}
