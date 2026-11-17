use rs09_game_core::random::JavaRandom;
use rs09_server::{
    definitions::item::Shared,
    world::{
        doors::Location,
        npc::{Context, Error, Event, Query, Result, State},
    },
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
fn n(v: &Value, k: &str, d: i32) -> i32 {
    v[k].as_i64().map_or(d, |n| n as i32)
}
fn loc(v: &Value) -> Option<Location> {
    let a = v.as_array()?;
    Some(Location::new(
        a[0].as_i64()? as i32,
        a[1].as_i64()? as i32,
        a[2].as_i64()? as i32,
    ))
}
fn coord(l: &Option<Location>) -> Value {
    l.as_ref().map_or(Value::Null, |l| json!(l.coordinates()))
}
struct Model {
    input: Value,
    events: Vec<Value>,
    calls: BTreeMap<String, i32>,
    rng: JavaRandom,
    ticks: i32,
    locked: bool,
    impact: i32,
    pause: Option<i32>,
    pending: Option<Location>,
    next_pulse: usize,
    constructed: BTreeSet<usize>,
}
impl Model {
    fn emit(&mut self, s: &State, phase: &str, args: Vec<Value>) -> Result<()> {
        let mut row = vec![json!(phase)];
        row.extend(args);
        self.events.push(json!(row));
        let at = self.calls.entry(phase.into()).or_default();
        *at += 1;
        let at = *at;
        for hook in self.input["hooks"].as_array().cloned().unwrap_or_default() {
            if hook["event"] == phase && n(&hook, "at", 1) == at {
                for a in hook["actions"].as_array().unwrap() {
                    self.act(s, a)?;
                }
            }
        }
        if n(&self.input["failures"], phase, -1) == at {
            return Err(Error::Binding(String::new()));
        }
        Ok(())
    }
    fn answer(&mut self, s: &State, phase: &str, fallback: bool, args: Vec<Value>) -> Result<bool> {
        self.emit(s, phase, args)?;
        let value = &self.input["answers"][phase];
        if let Some(a) = value.as_array() {
            Ok(a.get((self.calls[phase] - 1) as usize)
                .and_then(Value::as_bool)
                .unwrap_or(fallback))
        } else {
            Ok(value.as_bool().unwrap_or(fallback))
        }
    }
    fn set(&mut self, s: &State, v: &Value) {
        let mut f = s.fields.write();
        if v.get("location").is_some() {
            f.location = loc(&v["location"]);
        }
        if v.get("spawn").is_some() {
            f.spawn = loc(&v["spawn"]);
        }
        if v.get("radius").is_some() {
            f.radius = n(v, "radius", 11);
        }
        if let Some(b) = v["walks"].as_bool() {
            f.walks = b;
        }
        if let Some(b) = v["pathBound"].as_bool() {
            f.path_bound = b;
        }
        if v.get("path").is_some() {
            f.path = v["path"]
                .as_array()
                .map(|a| Shared::new(a.iter().map(loc).collect()));
        }
        if v.get("pathIndex").is_some() {
            f.path_index = n(v, "pathIndex", 0);
        }
        if v.get("nextWalk").is_some() {
            f.next_walk = n(v, "nextWalk", 0);
        }
        if let Some(b) = v["aggressive"].as_bool() {
            f.aggressive = b;
        }
        if let Some(b) = v["handler"].as_bool() {
            f.has_handler = b;
            self.pause = b.then_some(0);
        }
        if let Some(b) = v["neverWalks"].as_bool() {
            f.never_walks = b;
        }
        if let Some(b) = v["noReturn"].as_bool() {
            f.no_spawn_return = b;
        }
        if let Some(b) = v["returning"].as_bool() {
            f.returning = b;
        }
        if v.get("returnPulse").is_some() {
            f.return_pulse = v["returnPulse"].as_u64().map(|v| v as usize);
        }
        if v.get("dialogue").is_some() {
            f.dialogue = v["dialogue"].as_u64().map(|v| v as usize);
        }
        if v.get("forceTalk").is_some() {
            f.force_talk = v["forceTalk"].as_str().map(|s| s.encode_utf16().collect());
        }
        if v.get("lastTalk").is_some() {
            f.last_force_talk = n(v, "lastTalk", 0);
        }
        if v.get("respawn").is_some() {
            f.respawn_tick = n(v, "respawn", -1);
        }
        if let Some(b) = v["respawning"].as_bool() {
            f.respawning = b;
        }
    }
    fn act(&mut self, s: &State, a: &Value) -> Result<Value> {
        match a["op"].as_str().unwrap() {
            "tick" => s.tick(self)?,
            "actions" => s.actions(self)?,
            "destination" => return Ok(coord(&s.destination(self)?)),
            "setNext" => s.set_next_walk(self)?,
            "arrival" => {
                let p = s.fields.read().return_pulse.ok_or(Error::Null)?;
                return Ok(json!(if self.constructed.contains(&p) {
                    s.return_arrival(self)?
                } else {
                    true
                }));
            }
            "advance" => self.ticks = self.ticks.wrapping_add(n(a, "ticks", 1)),
            "set" => self.set(s, &a["fields"]),
            _ => panic!("unknown action"),
        }
        Ok(Value::Null)
    }
    fn snapshot(&self, s: &State, result: Value, error: Option<String>) -> Value {
        let f = s.fields.read();
        let path = f
            .path
            .as_ref()
            .map(|p| p.read().iter().map(coord).collect::<Vec<_>>());
        json!({"location":coord(&f.location),"spawn":coord(&f.spawn),"radius":f.radius,"walks":f.walks,"pathBound":f.path_bound,"path":path,"pathIndex":f.path_index,"nextWalk":f.next_walk,"aggressive":f.aggressive,"handler":f.has_handler,"pause":self.pause,"neverWalks":f.never_walks,"noReturn":f.no_spawn_return,"returnPulse":f.return_pulse,"returning":f.returning,"dialogue":f.dialogue,"forceTalk":f.force_talk.as_ref().map(|s|String::from_utf16_lossy(s)),"lastTalk":f.last_force_talk,"respawn":f.respawn_tick,"respawning":f.respawning,"locked":self.locked,"impact":self.impact,"pendingTeleport":coord(&self.pending),"ticks":self.ticks,"nextPulse":self.next_pulse,"events":self.events,"result":result,"error":error})
    }
}
impl Context for Model {
    fn ticks(&mut self, s: &State) -> Result<i32> {
        let t = self.ticks;
        self.emit(s, "Ticks", vec![json!(t)])?;
        Ok(t)
    }
    fn region_active(&mut self, s: &State, l: Location) -> Result<bool> {
        self.answer(s, "RegionActive", true, vec![json!(l.coordinates())])
    }
    fn query(&mut self, s: &State, q: Query) -> Result<bool> {
        let fallback = match q {
            Query::Behavior => true,
            Query::MovementLocked => self.locked,
            _ => false,
        };
        self.answer(s, &format!("{q:?}"), fallback, vec![])
    }
    fn event(&mut self, s: &State, e: Event) -> Result<()> {
        let (phase, args) = match e {
            Event::Inactivity => ("Inactivity", vec![]),
            Event::Teleport(l) => {
                self.pending = l.clone();
                ("Teleport", vec![coord(&l)])
            }
            Event::BehaviorRespawn => ("BehaviorRespawn", vec![]),
            Event::Respawn => ("Respawn", vec![]),
            Event::Restore => ("Restore", vec![]),
            Event::EntityTick => ("EntityTick", vec![]),
            Event::PauseAggression(n) => {
                self.pause = Some(n);
                ("PauseAggression", vec![json!(n)])
            }
            Event::LockMovement(n) => {
                self.locked = true;
                ("LockMovement", vec![json!(n)])
            }
            Event::UnlockMovement => {
                self.locked = false;
                ("UnlockMovement", vec![])
            }
            Event::ImpactDisabled(n) => {
                self.impact = n;
                ("ImpactDisabled", vec![json!(n)])
            }
            Event::Attribute(k) => ("Attribute", vec![json!(k)]),
            Event::SubmitReturn(p) => ("SubmitReturn", vec![json!(p)]),
            Event::StopCombat => ("StopCombat", vec![]),
            Event::Chat(text) => (
                "Chat",
                vec![text.map_or(Value::Null, |s| json!(String::from_utf16_lossy(&s)))],
            ),
        };
        self.emit(s, phase, args)
    }
    fn pulse_running(&mut self, s: &State, p: usize) -> Result<bool> {
        self.answer(s, "ReturnRunning", true, vec![json!(p)])
    }
    fn return_pulse(&mut self, s: &State, l: Option<Location>) -> Result<usize> {
        self.next_pulse += 1;
        let p = self.next_pulse;
        self.constructed.insert(p);
        self.emit(s, "Construct", vec![json!(p), coord(&l)])?;
        Ok(p)
    }
    fn dialogue_active(&mut self, s: &State, p: usize) -> Result<bool> {
        self.answer(s, "DialogueActive", true, vec![json!(p)])
    }
    fn dialogue_chatbox(&mut self, s: &State, p: usize) -> Result<bool> {
        self.answer(s, "DialogueChatbox", true, vec![json!(p)])
    }
    fn random_bool(&mut self, s: &State) -> Result<bool> {
        let b = self.rng.next_bool();
        self.emit(s, "Bool", vec![json!(b)])?;
        Ok(b)
    }
    fn randomize(&mut self, s: &State, bound: i32) -> Result<i32> {
        if bound < 1 {
            return Ok(0);
        }
        let v = self.rng.next_bound(bound).unwrap();
        self.emit(s, "Int", vec![json!(bound), json!(v)])?;
        Ok(v)
    }
    fn random_range(&mut self, s: &State, min: i32, max: i32) -> Result<i32> {
        let n = max.wrapping_sub(min).wrapping_abs();
        Ok(min.min(max).wrapping_add(self.randomize(s, n)?))
    }
    fn can_move(&mut self, s: &State, l: Option<Location>) -> Result<bool> {
        self.answer(s, "CanMove", true, vec![coord(&l)])
    }
    fn walk(&mut self, s: &State, l: Option<Location>, water: bool) -> Result<()> {
        self.emit(s, "Find", vec![coord(&l), json!(water)])?;
        self.emit(s, "Walk", vec![coord(&l), json!(water)])
    }
}
pub fn evaluate(input: &Value) -> Value {
    let s = State::new(Location::new(3207, 3216, 0));
    let mut m = Model {
        input: input.clone(),
        events: vec![],
        calls: BTreeMap::new(),
        rng: JavaRandom::new(input["seed"].as_i64().unwrap_or(0)),
        ticks: n(input, "ticks", 100),
        locked: false,
        impact: 0,
        pause: None,
        pending: None,
        next_pulse: 0,
        constructed: BTreeSet::new(),
    };
    m.set(&s, &input["fields"]);
    let mut out = vec![];
    for a in input["actions"].as_array().unwrap() {
        m.events.clear();
        let (result, error) = match m.act(&s, a) {
            Ok(v) => (v, None),
            Err(e) => (
                Value::Null,
                Some(match e {
                    Error::Binding(_) => "Binding".into(),
                    e => e.to_string(),
                }),
            ),
        };
        out.push(m.snapshot(&s, result, error));
    }
    json!(out)
}
