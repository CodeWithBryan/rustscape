//! Native replay of the stall thieving corpus.
//!
//! The case's declared state answers every query, exactly as the oracle's
//! recording world does, and every recorded call is logged in the same form;
//! each operation's result and log must equal the original's.
//!
//! `PulseManager.run` is the adapter's: the pulse is started and "submit"
//! is logged while it runs. A body that throws inside `Pulse.update` is the
//! original's printed stack trace and a dropped pulse (`true`).
use rs09_server::world::thieving_stall::{self as stalls, Error, Result, STALLS, StallThiefPulse};
use serde_json::{Map, Value, json};

fn bits(v: f64) -> String {
    format!("d:{:016x}", v.to_bits())
}

pub struct World {
    state: Map<String, Value>,
    trace: Vec<Value>,
    /// The scenery of the last `handle`.
    node: i32,
    /// The pulse the pulse manager was last given.
    pulse: Option<StallThiefPulse>,
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

impl stalls::Host for World {
    /// The NPC's place in the declared list.
    type Npc = usize;

    fn world_ticks(&mut self) -> Result<i32> {
        self.emit(json!(["ticks"]));
        Ok(self.number("ticks"))
    }
    fn set_thieve_delay(&mut self, ticks: i32) -> Result<()> {
        self.emit(json!(["setAttribute", stalls::THIEVE_DELAY, ticks]));
        let attributes = self.state["attr"].as_object_mut().unwrap();
        attributes.insert(stalls::THIEVE_DELAY.into(), json!(ticks));
        Ok(())
    }
    fn thieve_delay(&mut self) -> Result<i32> {
        self.emit(json!(["getAttribute", stalls::THIEVE_DELAY]));
        let value = self.state["attr"].get(stalls::THIEVE_DELAY);
        Ok(value.and_then(Value::as_i64).unwrap_or(0) as i32)
    }
    fn checks_inactive(&mut self) -> Result<bool> {
        // `checks` is the player, then the scenery.
        self.emit(json!(["active", "player"]));
        if !self.flag("playerActive") {
            return Ok(true);
        }
        self.emit(json!(["active", "node"]));
        Ok(!self.flag("nodeActive"))
    }
    fn in_combat(&mut self) -> Result<bool> {
        self.emit(json!(["inCombat"]));
        Ok(self.flag("inCombat"))
    }
    fn message(&mut self, message: &str) -> Result<()> {
        self.emit(json!(["message", message]));
        Ok(())
    }
    fn level(&mut self) -> Result<i32> {
        // Skills.THIEVING.
        self.emit(json!(["level", 17]));
        Ok(self.number("level"))
    }
    fn node_name(&mut self) -> Result<String> {
        self.emit(json!(["name"]));
        Ok(self.state["nodeName"].as_str().unwrap().to_owned())
    }
    fn free_slots(&mut self) -> Result<i32> {
        self.emit(json!(["freeSlots"]));
        Ok(self.number("freeSlots"))
    }
    fn in_region(&mut self, region: i32) -> Result<bool> {
        self.emit(json!(["inRegion", region]));
        Ok(self.number("region") == region)
    }
    fn quest_complete(&mut self, quest: &str) -> Result<bool> {
        self.emit(json!(["questComplete", quest]));
        Ok(self.flag("questComplete"))
    }
    fn dialogue(&mut self, message: &str) -> Result<()> {
        self.emit(json!(["dialogue", message]));
        Ok(())
    }
    fn reset_animation(&mut self) -> Result<()> {
        self.emit(json!(["animate", -1, "HIGH"]));
        Ok(())
    }
    fn animate(&mut self, id: i32) -> Result<()> {
        self.emit(json!(["animate", id, "MID"]));
        Ok(())
    }
    fn lock_interactions(&mut self, ticks: i32) -> Result<()> {
        self.emit(json!(["lockInteractions", ticks]));
        Ok(())
    }
    fn play_audio(&mut self, id: i32) -> Result<()> {
        self.emit(json!(["playAudio", id, 0]));
        Ok(())
    }
    fn set_silk_steal(&mut self, after_millis: i64) -> Result<()> {
        self.emit(json!(["silkSteal", after_millis]));
        Ok(())
    }
    fn node_active(&mut self) -> Result<bool> {
        self.emit(json!(["active", "node"]));
        Ok(self.flag("nodeActive"))
    }
    fn node_id(&mut self) -> Result<i32> {
        Ok(self.node)
    }
    fn replace(&mut self, id: i32, delay: i32) -> Result<()> {
        let node = self.node;
        self.emit(json!(["replace", node, id, delay]));
        Ok(())
    }
    fn random(&mut self, max: i32) -> Result<i32> {
        // RandomFunction.random(max): nothing is drawn below one.
        if max <= 0 {
            return Ok(0);
        }
        let draws = self.state["draws"].as_array_mut().unwrap();
        let draw = draws.remove(0).as_i64().unwrap() as i32;
        self.emit(json!(["random", max, draw]));
        Ok(draw)
    }
    fn add_item(&mut self, id: i32, amount: i32) -> Result<()> {
        self.emit(json!(["add", id, amount]));
        Ok(())
    }
    fn add_experience(&mut self, xp: f64) -> Result<()> {
        self.emit(json!(["xp", 17, bits(xp), true]));
        Ok(())
    }
    fn item_name(&mut self, id: i32) -> Result<String> {
        let names = &self.state["names"];
        let name = names.get(id.to_string()).or_else(|| names.get("*"));
        Ok(name.map_or_else(|| format!("Item {id}"), |n| n.as_str().unwrap().to_owned()))
    }
    fn resource_produced(&mut self, id: i32, amount: i32) -> Result<()> {
        let node = self.node;
        self.emit(json!([
            "dispatch",
            "ResourceProducedEvent",
            id,
            amount,
            node,
            0
        ]));
        Ok(())
    }
    fn stun(&mut self, ticks: i32) -> Result<()> {
        self.emit(json!(["stun", ticks, false]));
        Ok(())
    }
    fn impact(&mut self, amount: i32) -> Result<()> {
        self.emit(json!(["impact", amount, "NORMAL"]));
        Ok(())
    }
    fn local_npcs(&mut self, distance: i32) -> Result<Vec<usize>> {
        self.emit(json!(["localNPCs", distance]));
        Ok((0..self.state["npcs"].as_array().unwrap().len()).collect())
    }
    fn npc_attacking(&mut self, npc: &usize) -> Result<bool> {
        self.emit(json!(["attacking", npc]));
        Ok(self.state["npcs"][*npc][1] == Value::Bool(true))
    }
    fn npc_id(&mut self, npc: &usize) -> Result<i32> {
        Ok(self.state["npcs"][*npc][0].as_i64().unwrap() as i32)
    }
    fn npc_chat(&mut self, npc: &usize, text: &str) -> Result<()> {
        self.emit(json!(["chat", npc, text]));
        Ok(())
    }
    fn npc_attack(&mut self, npc: &usize) -> Result<()> {
        self.emit(json!(["attack", npc]));
        Ok(())
    }
    fn run_pulse(&mut self, mut pulse: StallThiefPulse) -> Result<()> {
        self.emit(json!(["run"]));
        let started = pulse.start(self);
        let running = pulse.running;
        self.pulse = Some(pulse);
        started?;
        if running {
            self.emit(json!(["submit"]));
        }
        Ok(())
    }
}

fn table() -> Value {
    STALLS
        .iter()
        .map(|s| {
            json!({
                "name": s.name,
                "full": s.full_ids,
                "empty": s.empty_ids,
                "level": s.level,
                "rewards": s.rewards.iter().map(|(id, amount)| json!([id, amount])).collect::<Vec<_>>(),
                "experience": bits(s.experience),
                "delay": s.delay,
                "msgItem": s.msg_item,
            })
        })
        .collect()
}

fn step(world: &mut World, op: &Value) -> Result<Value> {
    let node = || op["node"].as_i64().unwrap() as i32;
    match op["op"].as_str().unwrap() {
        "registrations" => Ok(json!(stalls::OPTIONS)),
        "stalls" => Ok(table()),
        "forObject" => {
            Ok(stalls::for_object(node()).map_or(Value::Null, |n| json!(STALLS[n].name)))
        }
        "empty" => {
            let name = op["stall"].as_str().unwrap();
            let stall = STALLS.iter().find(|s| s.name == name).unwrap();
            stall.empty(node()).map(|id| json!(id))
        }
        "set" => {
            for (key, value) in op["state"].as_object().unwrap() {
                world.state.insert(key.clone(), value.clone());
            }
            Ok(Value::Null)
        }
        "handle" => {
            world.node = node();
            stalls::handle(world, node(), op["option"].as_str().unwrap()).map(|b| json!(b))
        }
        "update" => {
            let mut pulse = world.pulse.take().expect("an update without a pulse");
            let done = pulse.update(world);
            world.pulse = Some(pulse);
            match done {
                Ok(done) => Ok(json!(done)),
                // Pulse.update prints what the body threw and drops the pulse.
                Err(Error::Index | Error::Null | Error::Cast) => Ok(json!(true)),
                Err(other) => Err(other),
            }
        }
        "stop" => {
            let mut pulse = world.pulse.take().expect("a stop without a pulse");
            let stopped = pulse.stop(world);
            world.pulse = Some(pulse);
            stopped.map(|()| Value::Null)
        }
        other => panic!("unknown operation {other}"),
    }
}

/// Replays one case and returns its results in the corpus's form.
pub fn replay(case: &Value) -> Value {
    let mut world = World {
        state: case["state"].as_object().unwrap().clone(),
        trace: Vec::new(),
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
