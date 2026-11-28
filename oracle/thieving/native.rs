//! Native replay of the pickpocketing corpus.
//!
//! The case's declared state answers every query, exactly as the oracle's
//! recording world does, and every recorded call is logged in the same form;
//! each operation's result and log must equal the original's.
//!
//! The loot tables are not rolled here: `canRoll`, `excluding` and `roll`
//! are logged with the constant's name and answered from the case, as the
//! JVM adapter does. The table each constant is given is compared entry by
//! entry (`tables`).
use rs09_server::world::thieving_pickpocket::{self as pick, Error, PICKPOCKETS, Result};
use serde_json::{Map, Value, json};

fn bits(v: f64) -> String {
    format!("d:{:016x}", v.to_bits())
}

pub struct World {
    state: Map<String, Value>,
    trace: Vec<Value>,
    /// The NPC of the operation.
    npc: i32,
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

impl pick::Host for World {
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
    fn can_roll(&mut self, n: usize) -> Result<bool> {
        self.emit(json!(["canRoll", PICKPOCKETS[n].name]));
        Ok(self.flag("canRoll"))
    }
    fn animate_player(&mut self) -> Result<()> {
        self.emit(json!(["animator", pick::PICKPOCKET_ANIM, "HIGH"]));
        Ok(())
    }
    fn has_clue(&mut self) -> Result<bool> {
        self.emit(json!(["hasClue"]));
        Ok(self.flag("hasClue"))
    }
    fn exclude_clues(&mut self, n: usize) -> Result<()> {
        self.emit(json!([
            "excluding",
            PICKPOCKETS[n].name,
            pick::CLUE_SCROLL_SLOTS
        ]));
        Ok(())
    }
    fn smooth_hands(&mut self) -> Result<bool> {
        self.emit(json!(["isActive", "SMOOTH_HANDS"]));
        Ok(self.flag("smoothHands"))
    }
    fn wears_gloves_of_silence(&mut self) -> Result<bool> {
        self.emit(json!(["equipped", pick::GLOVES_OF_SILENCE, 1]));
        Ok(self.flag("gloves"))
    }
    fn random_double(&mut self) -> Result<f64> {
        self.emit(json!(["randomDouble", bits(1.0), bits(100.0)]));
        Ok(self.state["chance"].as_f64().unwrap())
    }
    fn roll(&mut self, n: usize, excluded: bool) -> Result<Vec<(i32, i32)>> {
        self.emit(json!(["roll", PICKPOCKETS[n].name, excluded]));
        let loot = self.state["loot"].as_array().unwrap();
        Ok(loot
            .iter()
            .map(|pair| {
                (
                    pair[0].as_i64().unwrap() as i32,
                    pair[1].as_i64().unwrap() as i32,
                )
            })
            .collect())
    }
    fn npc_face_player(&mut self) -> Result<()> {
        self.emit(json!(["face", "player"]));
        Ok(())
    }
    fn npc_attack_animation(&mut self) -> Result<()> {
        let id = self.number("attackAnimation");
        self.emit(json!(["animate", "npc", id, "MID", false]));
        Ok(())
    }
    fn npc_animate(&mut self, id: i32) -> Result<()> {
        self.emit(json!(["animate", "npc", id, "MID", false]));
        Ok(())
    }
    fn play_hurt_audio(&mut self, delay: i32) -> Result<()> {
        self.emit(json!(["playHurtAudio", delay]));
        Ok(())
    }
    fn stun(&mut self, ticks: i32) -> Result<()> {
        self.emit(json!(["stun", ticks]));
        Ok(())
    }
    fn random(&mut self, a: i32, b: i32) -> Result<i32> {
        // RandomFunction.random(a, b): equal ends draw nothing.
        let n = b.wrapping_sub(a).wrapping_abs();
        if n == 0 {
            return Ok(a.min(b));
        }
        let draws = self.state["draws"].as_array_mut().unwrap();
        let draw = draws.remove(0).as_i64().unwrap() as i32;
        self.emit(json!(["random", n, draw]));
        Ok(a.min(b).wrapping_add(draw))
    }
    fn manual_hit(&mut self, hit: i32) -> Result<()> {
        let npc = self.npc;
        self.emit(json!(["manualHit", npc, hit, "NORMAL"]));
        Ok(())
    }
    fn npc_face_nothing(&mut self) -> Result<()> {
        self.emit(json!(["face", null]));
        Ok(())
    }
    fn play_audio(&mut self, id: i32) -> Result<()> {
        self.emit(json!(["playAudio", id, 0]));
        Ok(())
    }
    fn lock(&mut self, ticks: i32) -> Result<()> {
        self.emit(json!(["lock", ticks]));
        Ok(())
    }
    fn add_item(&mut self, id: i32, amount: i32) -> Result<()> {
        self.emit(json!(["add", id, amount]));
        Ok(())
    }
    fn add_experience(&mut self, xp: f64) -> Result<()> {
        self.emit(json!(["xp", 17, bits(xp)]));
        Ok(())
    }
}

fn tables() -> Value {
    PICKPOCKETS
        .iter()
        .map(|p| {
            let entries: Vec<Value> = p
                .entries()
                .iter()
                .map(|e| json!([e.id, e.min_amt, e.max_amt, bits(e.weight), e.guaranteed]))
                .collect();
            json!({
                "name": p.name,
                "ids": p.ids,
                "level": p.level,
                "low": bits(p.low),
                "high": bits(p.high),
                "experience": bits(p.experience),
                "stunDamageMin": p.stun_damage_min,
                "stunDamageMax": p.stun_damage_max,
                "stunTime": p.stun_time,
                "entries": entries,
            })
        })
        .collect()
}

fn step(world: &mut World, op: &Value) -> Result<Value> {
    let npc = || op["npc"].as_i64().unwrap() as i32;
    match op["op"].as_str().unwrap() {
        "registrations" => Ok(json!([["NPC", pick::OPTIONS]])),
        "tables" => Ok(tables()),
        "forID" => Ok(pick::for_id(npc()).map_or(Value::Null, |n| json!(PICKPOCKETS[n].name))),
        "set" => {
            for (key, value) in op["state"].as_object().unwrap() {
                world.state.insert(key.clone(), value.clone());
            }
            Ok(Value::Null)
        }
        "pickpocket" => {
            world.npc = npc();
            pick::pickpocket(world, npc()).map(|b| json!(b))
        }
        other => panic!("unknown operation {other}"),
    }
}

/// Replays one case and returns its results in the corpus's form.
pub fn replay(case: &Value) -> Value {
    let mut world = World {
        state: case["state"].as_object().unwrap().clone(),
        trace: Vec::new(),
        npc: 0,
    };
    let mut results = Vec::new();
    for op in case["ops"].as_array().unwrap() {
        world.trace.clear();
        let mut result = Map::new();
        match step(&mut world, op) {
            Ok(value) => result.insert("result".into(), value),
            Err(Error::Index) => result.insert("throws".into(), json!("Index")),
            Err(Error::Cast) => result.insert("throws".into(), json!("Cast")),
            Err(other) => panic!("binding failure: {other:?}"),
        };
        result.insert("trace".into(), Value::Array(world.trace.clone()));
        results.push(Value::Object(result));
    }
    Value::Array(results)
}
