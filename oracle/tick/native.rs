use rs09_server::world::{
    doors::Location,
    tick::{self, Context, EntityContext},
};
use serde_json::{Value, json};
use std::collections::HashMap;
type Result<T> = std::result::Result<T, String>;
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
struct Model {
    input: Value,
    location: Option<Location>,
    ticks: i32,
    packet_time: i32,
    clock: usize,
    events: Vec<Value>,
    calls: HashMap<String, i32>,
}
impl Model {
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
                    self.act(a)?;
                }
            }
        }
        if n(&self.input["failures"], event, -1) == count {
            return Err(event.into());
        }
        Ok(())
    }
    fn act(&mut self, a: &Value) -> Result<()> {
        match a["op"].as_str().unwrap() {
            "handle" => tick::handle(self, a["skip"].as_bool().unwrap_or(false)),
            "entity" => tick::entity(self),
            "set" => {
                if a.get("location").is_some() {
                    self.location = loc(&a["location"]);
                }
                Ok(())
            }
            "mutate" => {
                if let Some(l) = &self.location {
                    let [x, y, z] = loc(&a["location"]).unwrap().coordinates();
                    l.set(x, y, z);
                }
                Ok(())
            }
            op => panic!("unknown {op}"),
        }
    }
}
impl Context for Model {
    type Error = String;
    fn now_millis(&mut self) -> Result<i64> {
        let v = self.input["clock"]
            .get(self.clock)
            .and_then(Value::as_i64)
            .unwrap_or(self.clock as i64);
        self.clock += 1;
        self.emit("clock", vec![json!(v)])?;
        Ok(v)
    }
    fn packets(&mut self) -> Result<()> {
        self.emit("packets", vec![])
    }
    fn packet_time(&mut self, millis: i32) -> Result<()> {
        self.packet_time = millis;
        self.emit("packetTime", vec![json!(millis)])
    }
    fn disconnects(&mut self) -> Result<()> {
        self.emit("disconnects", vec![])
    }
    fn pulses(&mut self) -> Result<()> {
        self.emit("pulses", vec![])
    }
    fn listeners(&mut self) -> Result<()> {
        self.emit("listeners", vec![])
    }
    fn start(&mut self) -> Result<()> {
        self.emit("start", vec![])?;
        if self.input["doEntity"] == true {
            tick::entity(self)?;
        }
        Ok(())
    }
    fn run(&mut self) -> Result<()> {
        self.emit("run", vec![])
    }
    fn end(&mut self) -> Result<()> {
        self.emit("end", vec![])
    }
    fn pulse_world(&mut self) -> Result<()> {
        self.emit("pulseWorld", vec![])?;
        self.ticks = self.ticks.wrapping_add(1);
        Ok(())
    }
    fn managers(&mut self) -> Result<()> {
        self.emit("managers", vec![])
    }
    fn flush(&mut self) -> Result<()> {
        self.emit("flush", vec![])
    }
    fn exception(&mut self, error: String) -> Result<()> {
        self.emit("exception", vec![json!(error)])
    }
}
impl EntityContext for Model {
    type Error = String;
    fn pre_movement(&mut self) -> Result<()> {
        self.emit("pre", vec![])
    }
    fn ticks(&mut self) -> Result<i32> {
        Ok(self.ticks)
    }
    fn tick_event(&mut self, tick: i32) -> Result<()> {
        self.emit("tickEvent", vec![json!(tick)])
    }
    fn skills(&mut self) -> Result<()> {
        self.emit("skills", vec![])
    }
    fn location(&self) -> Option<Location> {
        self.location.clone()
    }
    fn walking(&mut self) -> Result<()> {
        self.emit("walking", vec![])?;
        if self.input.get("walkTo").is_some() {
            self.location = loc(&self.input["walkTo"]);
        }
        Ok(())
    }
    fn post_movement(&mut self, moved: bool) -> Result<()> {
        self.emit("post", vec![json!(moved)])
    }
    fn timers(&mut self) -> Result<()> {
        self.emit("timers", vec![])
    }
    fn prepare_masks(&mut self) -> Result<()> {
        self.emit("masks", vec![])
    }
}
pub fn evaluate(input: &Value) -> Value {
    let mut m = Model {
        input: input.clone(),
        location: loc(&input
            .get("location")
            .cloned()
            .unwrap_or(json!([3207, 3216, 0]))),
        ticks: n(input, "ticks", 0),
        packet_time: 0,
        clock: 0,
        events: vec![],
        calls: HashMap::new(),
    };
    let mut out = vec![];
    for a in input["actions"].as_array().unwrap() {
        m.events.clear();
        let error = m.act(a).err();
        out.push(json!({"location":m.location.as_ref().map(|l|l.coordinates()),"ticks":m.ticks,"packetTime":m.packet_time,"clockIndex":m.clock,"events":m.events,"error":error}));
    }
    json!(out)
}
