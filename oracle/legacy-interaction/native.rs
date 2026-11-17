use rs09_game_core::java_lower::lowercase_root;
use rs09_server::{
    definitions::item::Shared,
    world::{
        doors::Location,
        interaction::{OptionData, OptionHandle},
        legacy_interaction::{self as legacy, Context, Error, Result, Task},
        player_walk::Node,
    },
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
fn n(v: &Value, k: &str, d: i32) -> i32 {
    v[k].as_i64().map_or(d, |v| v as i32)
}
fn loc(v: &Value) -> Option<Location> {
    let a = v.as_array()?;
    Some(Location::new(
        a[0].as_i64()? as i32,
        a[1].as_i64()? as i32,
        a[2].as_i64()? as i32,
    ))
}
fn coord(v: &Option<Location>) -> Value {
    v.as_ref().map_or(Value::Null, |v| json!(v.coordinates()))
}
#[derive(Clone)]
struct Handler {
    walk: bool,
    walk_for: bool,
    delayed: bool,
    handled: bool,
}
#[derive(Clone)]
struct Pulse {
    task: Task,
    checks: Option<Vec<Option<Node>>>,
    routing: Option<usize>,
}
struct Model {
    input: Value,
    player: Option<Node>,
    target: Option<Node>,
    locations: [Option<Location>; 2],
    size: i32,
    manager: Option<usize>,
    properties: Option<usize>,
    managers: [Option<usize>; 2],
    handlers: [Handler; 2],
    options: Vec<OptionHandle>,
    option: usize,
    pulses: Vec<Pulse>,
    events: Vec<Value>,
    calls: BTreeMap<String, i32>,
}
impl Model {
    fn option_key(&self, o: Option<&OptionHandle>) -> Value {
        o.map_or(Value::Null, |o| {
            json!(self.options.iter().position(|v| v.same(o)).unwrap())
        })
    }
    fn selected(&self) -> OptionHandle {
        self.options[self.option].clone()
    }
    fn set(&mut self, v: &Value) {
        for k in ["locked", "moving", "zone", "errorMessage"] {
            if v.get(k).is_some() {
                self.input[k] = v[k].clone();
            }
        }
        if v.get("manager").is_some() {
            self.manager = v["manager"].as_u64().map(|v| v as usize);
        }
        if v.get("properties").is_some() {
            self.properties = v["properties"].as_u64().map(|v| v as usize);
        }
        if v.get("handler").is_some() {
            self.selected().write().handler = v["handler"].as_u64().map(|v| v as usize);
        }
        if v.get("location").is_some() {
            self.locations[1] = loc(&v["location"]);
        }
        if v.get("playerLocation").is_some() {
            self.locations[0] = loc(&v["playerLocation"]);
        }
        if v.get("size").is_some() {
            self.size = n(v, "size", 1);
        }
        if let Some(handlers) = v["handlers"].as_object() {
            for (k, v) in handlers {
                let h = &mut self.handlers[k.parse::<usize>().unwrap() - 1];
                for (k, field) in [
                    ("walk", &mut h.walk),
                    ("walkFor", &mut h.walk_for),
                    ("delayed", &mut h.delayed),
                    ("handled", &mut h.handled),
                ] {
                    if v.get(k).is_some() {
                        *field = v[k] == true;
                    }
                }
            }
        }
    }
    fn replace(&mut self, v: &Value) {
        let mut o = OptionData::new(
            v.get("name")
                .map_or(Some("Open".into()), |v| v.as_str().map(String::from)),
            n(v, "index", 0),
        );
        o.handler = if v.get("handler").is_some_and(Value::is_null) {
            None
        } else {
            Some(n(v, "handler", 1) as usize)
        };
        self.option = self.options.len();
        self.options.push(Shared::new(o));
    }
    fn emit(&mut self, event: &str, args: Vec<Value>) -> Result<()> {
        let mut row = vec![json!(event)];
        row.extend(args);
        self.events.push(json!(row));
        let count = self.calls.entry(event.into()).or_default();
        *count += 1;
        let count = *count;
        for h in self.input["hooks"].as_array().cloned().unwrap_or_default() {
            if h["event"] == event && n(&h, "at", 1) == count {
                for a in h["actions"].as_array().unwrap() {
                    self.act(a)?;
                }
            }
        }
        Ok(())
    }
    fn act(&mut self, a: &Value) -> Result<Value> {
        let target = self.target;
        let player = self.player;
        let option = if a["nullOption"] == true {
            None
        } else {
            Some(self.selected())
        };
        match a["op"].as_str().unwrap() {
            "fail" => Err(Error::Binding(Some("failure".into()))),
            "set" => {
                self.set(a);
                Ok(Value::Null)
            }
            "replace" => {
                self.replace(a);
                Ok(Value::Null)
            }
            "handle" => legacy::handle(self, target, player, option).map(|()| Value::Null),
            "invalid" => legacy::invalid(self, target, player, option).map(|()| Value::Null),
            "walk" => legacy::walk_option(self, target, player, option).map(|()| Value::Null),
            "default" => legacy::default_option(self, target, player, option).map(|()| Value::Null),
            "arrival" => {
                let Some(p) = self
                    .pulses
                    .get(n(a, "index", self.pulses.len() as i32 - 1) as usize)
                    .cloned()
                else {
                    return Ok(Value::Null);
                };
                legacy::arrival(self, &p.task).map(|v| json!(v))
            }
            "checksClear" => {
                if let Some(p) = self.pulses.last_mut() {
                    p.checks = None;
                }
                Ok(Value::Null)
            }
            "face" => {
                let from = self.locations[0].clone();
                self.face_location(target, from).map(|v| coord(&v))
            }
            _ => unreachable!(),
        }
    }
    fn construct(&mut self, task: Task, routing: Option<usize>, movement: bool) -> Result<usize> {
        let id = self.pulses.len();
        let checks = vec![task.player, task.target];
        let class = task.class_name();
        self.pulses.push(Pulse {
            task,
            checks: Some(checks.clone()),
            routing: None,
        });
        self.emit(
            "construct",
            vec![json!(id), json!(class), json!(1), json!(checks)],
        )?;
        if movement {
            self.pulses[id].routing = routing;
        }
        Ok(id)
    }
    fn snapshot(&self, result: Value, error: Option<&str>) -> Value {
        json!({"events":self.events,"result":result,"error":error,"manager":self.manager,"properties":self.properties,"currentManagers":self.managers,
  "location":coord(&self.locations[1]),"playerLocation":coord(&self.locations[0]),"size":self.size,"locked":self.input["locked"]==true,"moving":self.input["moving"]==true,"zone":self.input["zone"]==true,
  "handlers":self.handlers.iter().map(|h|[h.walk,h.walk_for,h.delayed,h.handled]).collect::<Vec<_>>(),
  "options":self.options.iter().map(|o|{let o=o.read();json!([o.name(),o.index(),o.handler])}).collect::<Vec<_>>(),"option":self.option,
  "pulses":self.pulses.iter().map(|p|json!([p.task.class_name(),1,p.checks,p.task.player,p.task.target,self.option_key(p.task.option.as_ref()),p.routing])).collect::<Vec<_>>()})
    }
}
impl Context for Model {
    type Manager = usize;
    type Pulse = usize;
    fn class_name(&self) -> String {
        if self.input["derived"] == true {
            "oracle.Support$Derived".into()
        } else {
            "core.game.interaction.InteractPlugin".into()
        }
    }
    fn interaction_locked(&mut self, p: Option<Node>) -> Result<bool> {
        p.ok_or(Error::Null)?;
        self.emit("locks", vec![])?;
        self.emit("interactionLocked", vec![])?;
        Ok(self.input["locked"] == true)
    }
    fn movement_locked(&mut self, p: Option<Node>) -> Result<bool> {
        p.ok_or(Error::Null)?;
        self.emit("locks", vec![])?;
        self.emit("movementLocked", vec![])?;
        Ok(self.input["moving"] == true)
    }
    fn location(&mut self, node: Option<Node>) -> Result<Option<Location>> {
        let node = node.ok_or(Error::Null)?;
        self.emit("location", vec![json!(node)])?;
        Ok(self.locations[node - 1].clone())
    }
    fn debug(&mut self, p: Option<Node>, text: &str) -> Result<()> {
        p.ok_or(Error::Null)?;
        self.emit("debug", vec![json!(text)])
    }
    fn handler_walk(&mut self, id: usize) -> Result<bool> {
        self.emit("walk", vec![json!(id)])?;
        Ok(self.handlers[id - 1].walk)
    }
    fn handler_walk_for(&mut self, id: usize, p: Option<Node>, t: Option<Node>) -> Result<bool> {
        self.emit("walkFor", vec![json!(id), json!(p), json!(t)])?;
        Ok(self.handlers[id - 1].walk_for)
    }
    fn handler_delayed(&mut self, id: usize, p: Option<Node>) -> Result<bool> {
        self.emit("delayed", vec![json!(id), json!(p)])?;
        Ok(self.handlers[id - 1].delayed)
    }
    fn handler_name(&mut self, id: usize) -> Result<String> {
        Ok(if id == 1 { "Handler" } else { "Other" }.into())
    }
    fn handler_handle(
        &mut self,
        id: usize,
        p: Option<Node>,
        t: Option<Node>,
        name: &str,
    ) -> Result<bool> {
        self.emit("handle", vec![json!(id), json!(p), json!(t), json!(name)])?;
        Ok(self.handlers[id - 1].handled)
    }
    fn lowercase(&self, s: &str) -> String {
        lowercase_root(s)
    }
    fn manager(&mut self, p: Option<Node>) -> Result<Option<usize>> {
        p.ok_or(Error::Null)?;
        self.emit("manager", vec![])?;
        Ok(self.manager)
    }
    fn movement(&mut self, task: Task, handler: Option<usize>) -> Result<usize> {
        self.construct(task, handler, true)
    }
    fn delayed(&mut self, task: Task) -> Result<usize> {
        self.construct(task, None, false)
    }
    fn run(&mut self, manager: usize, pulse: usize) -> Result<()> {
        self.managers[manager] = Some(pulse);
        self.emit("run", vec![json!(manager), json!(pulse)])
    }
    fn clear(&mut self, manager: usize) -> Result<()> {
        self.managers[manager] = None;
        self.emit("clear", vec![json!(manager)])
    }
    fn unhandled(&mut self, manager: usize, p: Option<Node>) -> Result<()> {
        self.emit("unhandled", vec![json!(manager), json!(p)])
    }
    fn zone(&mut self, p: Option<Node>, t: Option<Node>, o: Option<&OptionHandle>) -> Result<bool> {
        p.ok_or(Error::Null)?;
        self.emit("zone", vec![json!(t), self.option_key(o)])?;
        Ok(self.input["zone"] == true)
    }
    fn combat_stop(&mut self, p: Option<Node>) -> Result<()> {
        p.ok_or(Error::Null)?;
        self.emit("properties", vec![])?;
        let props = self.properties.ok_or(Error::Null)?;
        self.emit("combat", vec![json!(props)])?;
        self.emit("combatStop", vec![json!(props)])
    }
    fn player_location(&mut self, p: Option<Node>) -> Result<Option<Location>> {
        self.location(p)
    }
    fn face_location(
        &mut self,
        t: Option<Node>,
        from: Option<Location>,
    ) -> Result<Option<Location>> {
        let t = t.ok_or(Error::Null)?;
        self.emit("faceLocation", vec![json!(t), coord(&from)])?;
        let [x, y, z] = self.locations[t - 1]
            .as_ref()
            .ok_or(Error::Null)?
            .coordinates();
        let size = if t == 1 { 1 } else { self.size };
        let from = from.ok_or(Error::Null)?;
        Ok(Some(legacy::node_face_location(
            &Location::new(x, y, z),
            size,
            &from,
        )))
    }
    fn face_player(&mut self, p: Option<Node>, l: Option<Location>) -> Result<()> {
        p.ok_or(Error::Null)?;
        self.emit("facePlayer", vec![coord(&l)])
    }
    fn message(&mut self, p: Option<Node>, text: &str) -> Result<()> {
        p.ok_or(Error::Null)?;
        self.emit("message", vec![json!(text)])
    }
    fn dispatch(&mut self, p: Option<Node>, t: Option<Node>, text: &str) -> Result<()> {
        p.ok_or(Error::Null)?;
        self.emit("dispatch", vec![json!(t), json!(text)])
    }
    fn exception(&mut self, e: &Error) -> Result<()> {
        self.emit(
            "exception",
            vec![json!(if matches!(e, Error::Null) {
                "Null"
            } else {
                "Binding"
            })],
        )
    }
    fn error_message(&mut self, error: &Error) -> Result<Option<String>> {
        if matches!(error, Error::Null) {
            return Ok(None);
        }
        self.emit("errorMessage", vec![])?;
        Ok(self
            .input
            .get("errorMessage")
            .map_or(Some("failure".into()), |v| v.as_str().map(String::from)))
    }
    fn log(&mut self, class: &str, message: &str) -> Result<()> {
        self.emit("log", vec![json!(class), json!(message)])
    }
}
pub fn evaluate(input: &Value) -> Value {
    let h = Handler {
        walk: true,
        walk_for: false,
        delayed: true,
        handled: true,
    };
    let mut m = Model {
        input: input.clone(),
        player: if input.get("player").is_some_and(Value::is_null) {
            None
        } else {
            Some(1)
        },
        target: if input.get("target").is_some_and(Value::is_null) {
            None
        } else {
            Some(2)
        },
        locations: [
            Some(Location::new(3207, 3216, 0)),
            Some(Location::new(3211, 3217, 0)),
        ],
        size: 1,
        manager: Some(0),
        properties: Some(0),
        managers: [None, None],
        handlers: [h.clone(), h],
        options: vec![],
        option: 0,
        pulses: vec![],
        events: vec![],
        calls: BTreeMap::new(),
    };
    m.replace(input);
    m.set(input);
    let mut out = vec![];
    for a in input["actions"].as_array().unwrap() {
        m.events.clear();
        let (result, error) = match m.act(a) {
            Ok(v) => (v, None),
            Err(Error::Null) => (Value::Null, Some("Null")),
            Err(Error::Binding(_)) => (Value::Null, Some("Binding")),
        };
        out.push(m.snapshot(result, error));
    }
    json!(out)
}
