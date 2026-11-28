//! Native replay of the chest thieving corpus.
//!
//! The case's declared state answers every query, exactly as the oracle's
//! recording world does, and every recorded call is logged in the same form;
//! each operation's result and log must equal the original's.
//!
//! `Pulse.update` is modelled here for the one submitted pulse, as the
//! driver calls it: the player's activity, the running flag, the body and
//! `stop()`.
use rs09_server::world::thieving_chest::{
    self as chests, CHESTS, Error, Respawns, Result, TrapPulse,
};
use serde_json::{Map, Value, json};

fn bits(v: f64) -> String {
    format!("d:{:016x}", v.to_bits())
}

pub struct World {
    state: Map<String, Value>,
    trace: Vec<Value>,
    respawns: Respawns,
    /// The scenery of the last `handle`, and of the submitted pulse.
    node: i32,
    pulse: Option<(TrapPulse, bool, i32)>,
}
impl World {
    fn flag(&self, key: &str) -> bool {
        self.state[key] == Value::Bool(true)
    }
    fn number(&self, key: &str) -> i32 {
        self.state[key].as_i64().unwrap() as i32
    }
    fn emit(&mut self, row: Value) {
        self.trace.push(row);
    }
}

impl chests::Host for World {
    fn respawns(&mut self) -> &mut Respawns {
        &mut self.respawns
    }
    fn world_ticks(&mut self) -> Result<i32> {
        self.emit(json!(["ticks"]));
        Ok(self.number("ticks"))
    }
    fn message(&mut self, message: &str) -> Result<()> {
        self.emit(json!(["message", message]));
        Ok(())
    }
    fn message_later(&mut self, message: &str, ticks: i32) -> Result<()> {
        self.emit(json!(["messageLater", message, ticks]));
        Ok(())
    }
    fn lock(&mut self, ticks: i32) -> Result<()> {
        self.emit(json!(["lock", ticks]));
        Ok(())
    }
    fn lifepoints(&mut self) -> Result<i32> {
        self.emit(json!(["lifepoints"]));
        Ok(self.number("lifepoints"))
    }
    fn manual_hit(&mut self, hit: i32) -> Result<()> {
        self.emit(json!(["manualHit", hit, "NORMAL"]));
        Ok(())
    }
    fn face_chest(&mut self) -> Result<()> {
        self.emit(json!(["faceLocation"]));
        Ok(())
    }
    fn level(&mut self) -> Result<i32> {
        // Skills.THIEVING.
        self.emit(json!(["level", 17]));
        Ok(self.number("level"))
    }
    fn animator_animate(&mut self, id: i32) -> Result<()> {
        self.emit(json!(["animator", id, "MID"]));
        Ok(())
    }
    fn animate(&mut self, id: i32) -> Result<()> {
        self.emit(json!(["animate", id, "MID"]));
        Ok(())
    }
    fn free_slots(&mut self) -> Result<i32> {
        self.emit(json!(["freeSlots"]));
        Ok(self.number("freeSlots"))
    }
    fn set_disabled_ticks(&mut self, ticks: i32) -> Result<()> {
        self.emit(json!(["disabledTicks", ticks]));
        Ok(())
    }
    fn submit(&mut self, pulse: TrapPulse) -> Result<()> {
        self.emit(json!(["submit"]));
        self.pulse = Some((pulse, true, self.node));
        Ok(())
    }
    fn add_or_drop(&mut self, id: i32, amount: i32) -> Result<()> {
        self.emit(json!(["addOrDrop", id, amount]));
        Ok(())
    }
    fn add_experience(&mut self, xp: f64) -> Result<()> {
        self.emit(json!(["xp", 17, bits(xp), true]));
        Ok(())
    }
    fn node_active(&mut self) -> Result<bool> {
        self.emit(json!(["active", "node"]));
        Ok(self.flag("nodeActive"))
    }
    fn replace(&mut self, id: i32, ticks: i32) -> Result<()> {
        let node = self.pulse.as_ref().map_or(self.node, |(_, _, node)| *node);
        self.emit(json!(["replace", node, id, ticks]));
        Ok(())
    }
}

fn table() -> Value {
    CHESTS
        .iter()
        .map(|c| {
            json!({
                "name": c.name,
                "ids": c.object_ids,
                "level": c.level,
                "experience": bits(c.experience),
                "rewards": c.rewards.iter().map(|(id, amount)| json!([id, amount])).collect::<Vec<_>>(),
                "respawn": c.respawn,
            })
        })
        .collect()
}

/// `Pulse.update()` of the submitted pulse (its delay is one tick).
fn update(world: &mut World) -> Result<Value> {
    let Some((mut pulse, running, node)) = world.pulse.take() else {
        return Ok(json!("none"));
    };
    // `checks` is the player.
    world.emit(json!(["active", "player"]));
    if !world.flag("playerActive") {
        world.pulse = Some((pulse, false, node));
        return Ok(json!(true));
    }
    if !running {
        world.pulse = Some((pulse, false, node));
        return Ok(json!(true));
    }
    world.pulse = Some((pulse, true, node));
    let done = pulse.pulse(world)?;
    world.pulse = Some((pulse, !done, node));
    Ok(json!(done))
}

fn step(world: &mut World, op: &Value) -> Result<Value> {
    let node = || op["node"].as_i64().unwrap() as i32;
    match op["op"].as_str().unwrap() {
        "registrations" => Ok(CHESTS
            .iter()
            .flat_map(|c| c.object_ids.iter())
            .flat_map(|id| chests::KEYS.iter().map(move |key| json!([id, key])))
            .collect()),
        "chests" => Ok(table()),
        "forId" => Ok(chests::for_id(node()).map_or(Value::Null, |n| json!(CHESTS[n].name))),
        "respawning" => {
            let name = op["chest"].as_str().unwrap();
            let chest = CHESTS.iter().position(|c| c.name == name).unwrap();
            let current = world.respawns.0[chest];
            Ok(json!(current > chests::Host::world_ticks(world)?))
        }
        "set" => {
            for (key, value) in op["state"].as_object().unwrap() {
                world.state.insert(key.clone(), value.clone());
            }
            Ok(Value::Null)
        }
        "handle" => {
            world.node = node();
            chests::handle(world, node(), op["option"].as_str().unwrap()).map(|b| json!(b))
        }
        "update" => update(world),
        other => panic!("unknown operation {other}"),
    }
}

/// Replays one case and returns its results in the corpus's form.
pub fn replay(case: &Value) -> Value {
    let mut world = World {
        state: case["state"].as_object().unwrap().clone(),
        trace: Vec::new(),
        respawns: Respawns::default(),
        node: 0,
        pulse: None,
    };
    let mut results = Vec::new();
    for op in case["ops"].as_array().unwrap() {
        world.trace.clear();
        let mut result = Map::new();
        match step(&mut world, op) {
            Ok(value) => result.insert("result".into(), value),
            Err(Error::Index) => result.insert("throws".into(), json!("Index")),
            Err(Error::Null) => result.insert("throws".into(), json!("Null")),
            Err(Error::Cast) => result.insert("throws".into(), json!("Cast")),
            Err(other) => panic!("binding failure: {other:?}"),
        };
        result.insert("trace".into(), Value::Array(world.trace.clone()));
        results.push(Value::Object(result));
    }
    Value::Array(results)
}
