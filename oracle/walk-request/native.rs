use rs09_server::world::{
    doors::Location,
    walk_request::{self, Context, Error, Interfaces, Kind, Request, Result},
};
use serde_json::{Value, json};
use std::collections::HashMap;
fn n(v: &Value, key: &str, default: i32) -> i32 {
    v[key].as_i64().map_or(default, |n| n as i32)
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
fn failure(e: Error) -> &'static str {
    match e {
        Error::Null => "Null",
        Error::Binding(_) => "Binding",
    }
}
#[derive(Clone)]
struct Pulse {
    player: usize,
    location: Location,
    running: bool,
}
struct Model {
    input: Value,
    location: Option<Location>,
    ui: Interfaces,
    locked: bool,
    running: bool,
    maximum: i32,
    pulses: Vec<Pulse>,
    events: Vec<Value>,
    calls: HashMap<String, i32>,
}
impl Model {
    fn set(&mut self, v: &Value) {
        if v.get("location").is_some() {
            self.location = loc(&v["location"]);
        }
        for (key, field) in [
            ("locked", &mut self.locked),
            ("opened", &mut self.ui.opened),
            ("chatbox", &mut self.ui.chatbox),
            ("running", &mut self.running),
        ] {
            if v.get(key).is_some() {
                *field = v[key].as_bool().unwrap_or(false);
            }
        }
        if v.get("openWalkable").is_some() {
            self.ui.opened_walkable = v["openWalkable"].as_bool();
        }
        if v.get("chatWalkable").is_some() {
            self.ui.chatbox_walkable = v["chatWalkable"].as_bool();
        }
        for key in ["close", "dialogue", "maxDistance"] {
            if v.get(key).is_some() {
                self.input[key] = v[key].clone();
            }
        }
        self.maximum = n(&self.input, "maxDistance", 25);
    }
    fn emit(&mut self, event: &str, args: Vec<Value>) -> Result<()> {
        let mut row = vec![json!(event)];
        row.extend(args);
        self.events.push(json!(row));
        let count = self.calls.entry(event.into()).or_default();
        *count += 1;
        let count = *count;
        for hook in self.input["hooks"].as_array().cloned().unwrap_or_default() {
            if hook["event"] == event && n(&hook, "at", 1) == count {
                for a in hook["actions"].as_array().unwrap() {
                    self.act(a).map_err(|e| {
                        if e == "Null" {
                            Error::Null
                        } else {
                            Error::Binding(e.into())
                        }
                    })?;
                }
            }
        }
        Ok(())
    }
    fn act(&mut self, a: &Value) -> std::result::Result<Value, &'static str> {
        match a["op"].as_str().unwrap() {
            "fail" => return Err("Binding"),
            "set" => self.set(a),
            "arrival" => {
                let p = self
                    .pulses
                    .get(n(a, "index", self.pulses.len() as i32 - 1) as usize)
                    .cloned()
                    .ok_or("Index")?;
                return walk_request::arrival(self, p.player, p.running)
                    .map(|b| json!(b))
                    .map_err(failure);
            }
            "process" => {
                let kind = match a["kind"].as_str().unwrap_or("world") {
                    "world" => Some(Kind::Worldspace),
                    "interact" => Some(Kind::Interact),
                    "minimap" => Some(Kind::Minimap),
                    _ => None,
                };
                let request = kind.map(|kind| Request {
                    player: if self.input.get("player") == Some(&Value::Null) {
                        None
                    } else {
                        Some(0)
                    },
                    kind,
                    x: n(a, "x", 3210),
                    y: n(a, "y", 3216),
                    running: a["running"].as_bool().unwrap_or(false),
                });
                walk_request::process(self, request).map_err(failure)?;
            }
            op => panic!("unknown {op}"),
        }
        Ok(Value::Null)
    }
}
impl Context for Model {
    type Pulse = Pulse;
    fn location(&self, _: usize) -> Result<Option<Location>> {
        Ok(self.location.clone())
    }
    fn movement_locked(&mut self, _: usize) -> Result<bool> {
        self.emit("locked", vec![])?;
        Ok(self.locked)
    }
    fn interfaces(&self, _: usize) -> Result<Interfaces> {
        Ok(self.ui)
    }
    fn close_interface(&mut self, _: usize) -> Result<bool> {
        self.emit("closeInterface", vec![])?;
        Ok(self.input["close"] != false)
    }
    fn close_chatbox(&mut self, _: usize) -> Result<()> {
        self.emit("closeChatbox", vec![])
    }
    fn close_dialogue(&mut self, _: usize) -> Result<bool> {
        self.emit("dialogue", vec![])?;
        Ok(self.input["dialogue"] != false)
    }
    fn debug(&mut self, _: usize, s: &str) -> Result<()> {
        self.emit("debug", vec![json!(s)])
    }
    fn clear_minimap(&mut self, _: usize) -> Result<()> {
        self.emit("minimap", vec![])
    }
    fn set_running(&mut self, _: usize, running: bool) -> Result<()> {
        self.running = running;
        self.emit("setRunning", vec![json!(running)])
    }
    fn face_none(&mut self, _: usize) -> Result<()> {
        self.emit("face", vec![])
    }
    fn face_location_none(&mut self, _: usize) -> Result<()> {
        self.emit("faceLocation", vec![])
    }
    fn reset_scripts(&mut self, _: usize) -> Result<()> {
        self.emit("scripts", vec![])
    }
    fn construct(&mut self, player: usize, location: Location, force_run: bool) -> Result<Pulse> {
        self.emit(
            "construct",
            vec![json!(location.coordinates()), json!(force_run)],
        )?;
        Ok(Pulse {
            player,
            location,
            running: force_run,
        })
    }
    fn run(&mut self, _: usize, pulse: Pulse) -> Result<()> {
        self.emit("run", vec![])?;
        self.pulses.push(pulse);
        Ok(())
    }
    fn max_distance(&self) -> i32 {
        self.maximum
    }
}
pub fn evaluate(input: &Value) -> Value {
    let mut m = Model {
        input: input.clone(),
        location: loc(&input
            .get("location")
            .cloned()
            .unwrap_or(json!([3207, 3216, 0]))),
        ui: Interfaces {
            opened_walkable: Some(false),
            chatbox_walkable: Some(false),
            ..Interfaces::default()
        },
        locked: false,
        running: false,
        maximum: 25,
        pulses: vec![],
        events: vec![],
        calls: HashMap::new(),
    };
    m.set(input);
    let mut out = vec![];
    for a in input["actions"].as_array().unwrap() {
        m.events.clear();
        let (result, error) = match m.act(a) {
            Ok(v) => (v, None),
            Err(e) => (Value::Null, Some(e)),
        };
        out.push(json!({"location":coord(&m.location),"running":m.running,"events":m.events,"result":result,"error":error,"pulses":m.pulses.iter().map(|p|json!([p.location.coordinates(),p.running])).collect::<Vec<_>>(),"maxDistance":m.maximum}));
    }
    json!(out)
}
