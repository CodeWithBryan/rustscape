//! Test-only landscape/chunk services around the native builder.
use rs09_server::{
    definitions::item::Shared,
    pulses,
    world::{
        doors::{Handle, Location, Scenery},
        scenery::{
            self, Context, Error, GroundItems, ObjectUpdate, ReplaceMode, Result, SceneryPulse,
        },
    },
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, VecDeque};
fn n(v: &Value) -> i32 {
    v.as_i64().unwrap() as i32
}
fn val(v: &Value, k: &str, d: i32) -> i32 {
    v[k].as_i64().map_or(d, |v| v as i32)
}
fn location(v: &Value) -> Location {
    Location::new(n(&v[0]), n(&v[1]), n(&v[2]))
}
#[derive(Clone)]
enum Task {
    Native(SceneryPulse),
    Custom { pulse: pulses::Handle, finish: bool },
}
impl Task {
    fn pulse(&self) -> pulses::Handle {
        match self {
            Self::Native(p) => p.pulse.clone(),
            Self::Custom { pulse, .. } => pulse.clone(),
        }
    }
}
struct Model {
    input: Value,
    nodes: BTreeMap<String, Option<Handle>>,
    tasks: BTreeMap<String, Task>,
    queue: VecDeque<String>,
    items: BTreeMap<String, Option<GroundItems>>,
    chunks: Vec<(String, [i32; 3])>,
    flags: BTreeMap<String, Vec<ObjectUpdate>>,
    flag_order: Vec<String>,
    events: Vec<Value>,
    calls: BTreeMap<String, i32>,
    hooks: Vec<Value>,
    generated: usize,
    pulse_generated: usize,
    enabled: bool,
}
fn class(e: &Error) -> &str {
    match e {
        Error::Null => "Null",
        Error::Index => "Index",
        Error::Direction => "Direction",
        Error::State(_) => "State",
        Error::Binding(s) if s == "Type" => "Type",
        Error::Binding(_) => "Binding",
    }
}
impl Model {
    fn new(c: &Value) -> Self {
        let mut s = Self {
            input: c.clone(),
            nodes: BTreeMap::new(),
            tasks: BTreeMap::new(),
            queue: VecDeque::new(),
            items: BTreeMap::new(),
            chunks: Vec::new(),
            flags: BTreeMap::new(),
            flag_order: Vec::new(),
            events: Vec::new(),
            calls: BTreeMap::new(),
            hooks: c["hooks"].as_array().cloned().unwrap_or_default(),
            generated: 0,
            pulse_generated: 0,
            enabled: false,
        };
        for (k, row) in c["nodes"].as_object().unwrap() {
            if row.is_null() {
                s.nodes.insert(k.clone(), None);
            } else {
                let h = s
                    .construct(
                        n(&row["id"]),
                        location(&row["location"]),
                        val(row, "shape", 0),
                        val(row, "rotation", 0),
                    )
                    .unwrap();
                h.write().constructed = row["constructed"] == true;
                s.nodes.insert(k.clone(), Some(h));
            }
        }
        for (k, row) in c["pulses"].as_object().into_iter().flatten() {
            let p = Shared::new(pulses::Pulse::new(
                val(row, "delay", 1),
                None,
                pulses::Class::Other,
                "custom".into(),
            ));
            p.write().running = row["stopped"] != true;
            s.tasks.insert(
                k.clone(),
                Task::Custom {
                    pulse: p,
                    finish: row["finish"] != false,
                },
            );
        }
        for (k, row) in c["nodes"].as_object().unwrap() {
            if let Some(h) = s.get(k) {
                let mut h = h.write();
                h.wrapper = row["wrapper"].as_str().and_then(|k| s.get(k));
                h.replaced = row["replaced"].as_str().and_then(|k| s.get(k));
                h.restore_pulse = row["restore"]
                    .as_str()
                    .and_then(|k| s.tasks.get(k).map(Task::pulse));
                h.destruction_pulse = row["destruction"]
                    .as_str()
                    .and_then(|k| s.tasks.get(k).map(Task::pulse));
            }
        }
        for (k, row) in c["items"].as_object().into_iter().flatten() {
            let items = row.as_array().map(|a| {
                Shared::new(
                    a.iter()
                        .map(|v| v.as_u64().map(|v| v as usize))
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                )
            });
            s.items.insert(k.clone(), items);
        }
        s.enabled = true;
        s
    }
    fn get(&self, k: &str) -> Option<Handle> {
        self.nodes.get(k).cloned().flatten()
    }
    fn named(&self, a: &Value, key: &str) -> Option<Handle> {
        a[key].as_str().and_then(|k| self.get(k))
    }
    fn label(&mut self, h: &Handle) -> String {
        if let Some((k, _)) = self
            .nodes
            .iter()
            .find(|(_, v)| v.as_ref().is_some_and(|v| v.same(h)))
        {
            return k.clone();
        }
        let k = format!("g{}", self.generated);
        self.generated += 1;
        self.nodes.insert(k.clone(), Some(h.clone()));
        k
    }
    fn pulse_label(&mut self, p: &pulses::Handle) -> String {
        if let Some((k, _)) = self.tasks.iter().find(|(_, v)| v.pulse().same(p)) {
            return k.clone();
        }
        panic!("unregistered pulse {}", p.identity())
    }
    fn emit(&mut self, event: &str, args: Vec<Value>) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }
        let mut row = vec![json!(event)];
        row.extend(args);
        self.events.push(json!(row));
        let count = self.calls.entry(event.into()).or_default();
        *count += 1;
        let count = *count;
        for h in self.hooks.clone() {
            if h["event"] == event && val(&h, "at", 1) == count {
                for a in h["actions"].as_array().unwrap() {
                    self.act(a)?;
                }
            }
        }
        Ok(())
    }
    fn act(&mut self, a: &Value) -> Result<Value> {
        let op = a["op"].as_str().unwrap();
        let object = self.named(a, "node");
        let other = self.named(a, "other");
        let obj = || object.clone().ok_or(Error::Null);
        match op {
            "swap" => {
                return Ok(json!(scenery::replace(
                    self,
                    object.as_ref(),
                    other.as_ref(),
                    ReplaceMode::Swap {
                        clip: a["clip"] != false,
                        permanent: a["permanent"] == true
                    }
                )?));
            }
            "temporary" => {
                return Ok(json!(scenery::replace(
                    self,
                    object.as_ref(),
                    other.as_ref(),
                    ReplaceMode::Temporary {
                        clip: a["clip"] != false,
                        ticks: val(a, "ticks", 1)
                    }
                )?));
            }
            "add" => {
                let items = if let Some(k) = a["items"].as_str() {
                    self.items.get(k).cloned().flatten()
                } else {
                    Some(Shared::new(Vec::new().into_boxed_slice()))
                };
                let h = scenery::add(self, object.as_ref(), val(a, "ticks", -1), items)?;
                return Ok(json!(self.label(&h)));
            }
            "remove" => {
                return Ok(json!(if a.get("ticks").is_some() {
                    scenery::remove_respawn(self, object.as_ref(), n(&a["ticks"]))?
                } else {
                    scenery::remove(self, object.as_ref())?
                }));
            }
            "update" => {
                let objects: Vec<_> = a["nodes"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_str().and_then(|k| self.get(k)))
                    .collect();
                scenery::update(self, &objects)?;
            }
            "active" => scenery::set_active(self, &obj()?, a["value"] == true)?,
            "renderable" => obj()?.write().renderable = a["value"] == true,
            "rotate" => obj()?.write().rotation = n(&a["value"]),
            "location" => obj()?.write().location = location(&a["value"]),
            "wrapper" => obj()?.write().wrapper = other,
            "replaced" => {
                let h = obj()?;
                if !h.read().constructed {
                    return Err(Error::Binding("Type".into()));
                }
                h.write().replaced = other;
            }
            "restore" | "destruction" => {
                let h = obj()?;
                let p = a["pulse"]
                    .as_str()
                    .and_then(|k| self.tasks.get(k).map(Task::pulse));
                if op == "restore" {
                    h.write().restore_pulse = p;
                } else {
                    h.write().destruction_pulse = p;
                }
            }
            "setItem" => {
                let ar = self
                    .items
                    .get(a["items"].as_str().unwrap())
                    .cloned()
                    .flatten()
                    .ok_or(Error::Null)?;
                let i = n(&a["index"]) as usize;
                *ar.write().get_mut(i).ok_or(Error::Index)? =
                    a["value"].as_u64().map(|v| v as usize);
            }
            "tick" => {
                let count = self.queue.len();
                for _ in 0..count {
                    let k = self.queue.pop_front().unwrap();
                    let p = self.tasks[&k].pulse();
                    if !pulses::update_base(&p, &mut (), self)
                        .map_err(|_| Error::Binding("unexpected pulse failure".into()))?
                        && p.read().running
                    {
                        self.queue.push_back(k);
                    }
                }
            }
            "pulse" => {
                let p = self
                    .tasks
                    .get(a["pulse"].as_str().unwrap())
                    .map(Task::pulse)
                    .ok_or(Error::Null)?;
                return Ok(json!(self.pulse_direct(&p)?));
            }
            "fail" => return Err(Error::Binding("callback".into())),
            "flush" => {
                let mut wire = Vec::new();
                for key in &self.flag_order {
                    let [x, y, _] = self.chunks.iter().find(|(k, _)| k == key).unwrap().1;
                    let frames: Vec<_> = self.flags[key].iter().map(ObjectUpdate::frame).collect();
                    let f = rs09_protocol::game::updates::zone_enclosed(x, y, &frames).unwrap();
                    let mut bytes = Vec::new();
                    f.write(
                        rs09_protocol::game::Direction::ServerToClient,
                        None,
                        &mut bytes,
                    )
                    .unwrap();
                    wire.push(bytes);
                }
                return Ok(json!(wire));
            }
            name => panic!("unknown {name}"),
        }
        Ok(Value::Null)
    }
    fn snapshot(&mut self) -> Value {
        let mut nodes = BTreeMap::new();
        let all: Vec<_> = self
            .nodes
            .iter()
            .map(|(k, h)| (k.clone(), h.clone()))
            .collect();
        for (k, h) in all {
            let Some(h) = h else {
                nodes.insert(k, Value::Null);
                continue;
            };
            let h = h.read();
            let wrapper = h.wrapper.clone();
            let prev = h.replaced.clone();
            let restore = h.restore_pulse.clone();
            let destroy = h.destruction_pulse.clone();
            nodes.insert(k.clone(),json!({"id":h.id,"shape":h.shape,"rotation":h.rotation,"location":h.location.coordinates(),"active":h.active,"renderable":h.renderable,"constructed":h.constructed,"wrapper":wrapper.filter(|w|!w.same(self.nodes[&k].as_ref().unwrap())).map(|w|self.label(&w)),"replaced":if h.constructed{prev.map(|h|self.label(&h))}else{None},"restore":restore.map(|p|self.pulse_label(&p)),"destruction":destroy.map(|p|self.pulse_label(&p))}));
        }
        let mut tasks = BTreeMap::new();
        let all: Vec<_> = self
            .tasks
            .iter()
            .map(|(k, t)| (k.clone(), t.clone()))
            .collect();
        for (k, t) in all {
            let p = t.pulse();
            let p = p.read();
            let checks = p.checks.as_ref().map(|c| {
                c.read()
                    .iter()
                    .map(|id| {
                        id.and_then(|id| {
                            self.nodes
                                .iter()
                                .find(|(_, h)| h.as_ref().is_some_and(|h| h.identity() == id))
                                .map(|(k, _)| k.clone())
                        })
                    })
                    .collect::<Vec<_>>()
            });
            tasks.insert(
                k,
                json!({"running":p.running,"ticks":p.ticks_passed,"delay":p.delay,"checks":checks}),
            );
        }
        let flags: BTreeMap<_, _> = self
            .flags
            .iter()
            .map(|(k, v)| {
                (
                    k.clone(),
                    v.iter()
                        .map(|f| {
                            json!([
                                self.nodes
                                    .iter()
                                    .find(|(_, h)| h.as_ref().is_some_and(|h| h.same(&f.object)))
                                    .unwrap()
                                    .0,
                                f.remove
                            ])
                        })
                        .collect::<Vec<_>>(),
                )
            })
            .collect();
        let items: BTreeMap<_, _> = self
            .items
            .iter()
            .map(|(k, a)| (k.clone(), a.as_ref().map(|a| a.read().to_vec())))
            .collect();
        json!({"nodes":nodes,"pulses":tasks,"queue":self.queue,"flags":flags,"items":items,"events":self.events})
    }
}
impl Context for Model {
    fn construct(&mut self, id: i32, l: Location, shape: i32, rot: i32) -> Result<Handle> {
        Ok(Shared::new(
            Scenery::new(id, l, shape, rot, Some("Door".into()), false)
                .map_err(|_| Error::Direction)?,
        ))
    }
    fn remove_landscape(&mut self, h: &Handle) -> Result<Option<Handle>> {
        let k = self.label(h);
        self.emit("remove", vec![json!(k)])?;
        let current = if self.input.get("removeResult").is_some() {
            self.input["removeResult"]
                .as_str()
                .and_then(|k| self.get(k))
        } else {
            Some(h.clone())
        };
        let Some(c) = current else {
            return Ok(None);
        };
        scenery::set_active(self, &c, false)?;
        scenery::set_active(self, h, false)?;
        Ok(Some(c))
    }
    fn add_landscape(&mut self, h: &Handle) -> Result<()> {
        let k = self.label(h);
        self.emit("add", vec![json!(k)])?;
        scenery::set_active(self, h, true)?;
        h.write().renderable = true;
        Ok(())
    }
    fn chunk(&mut self, l: Location) -> Result<usize> {
        let [x, y, z] = l.coordinates();
        let at = [x & !7, y & !7, z];
        let k = format!("{},{},{}", at[0], at[1], at[2]);
        self.emit("chunk", vec![json!(l.coordinates())])?;
        if let Some(i) = self.chunks.iter().position(|(key, _)| key == &k) {
            return Ok(i);
        }
        let i = self.chunks.len();
        self.chunks.push((k, at));
        Ok(i)
    }
    fn flag(&mut self, chunk: usize, update: ObjectUpdate) -> Result<()> {
        let k = self.chunks[chunk].0.clone();
        let name = self.label(&update.object);
        self.emit("flag", vec![json!(k), json!(name), json!(update.remove)])?;
        if !self.flags.contains_key(&k) {
            self.flag_order.push(k.clone());
        }
        self.flags.entry(k).or_default().push(update);
        Ok(())
    }
    fn stop(&mut self, p: &pulses::Handle) -> Result<()> {
        let k = self.pulse_label(p);
        if matches!(self.tasks[&k], Task::Custom { .. }) {
            self.emit("stop", vec![json!(k)])?;
        }
        pulses::stop_base(p);
        Ok(())
    }
    fn pulse_direct(&mut self, p: &pulses::Handle) -> Result<bool> {
        let k = self.pulse_label(p);
        match self.tasks[&k].clone() {
            Task::Native(task) => scenery::pulse_body(self, &task),
            Task::Custom { finish, .. } => {
                self.emit("custom", vec![json!(k)])?;
                Ok(finish)
            }
        }
    }
    fn submit(&mut self, p: SceneryPulse) -> Result<()> {
        let name = if let Some((k, _)) = self.tasks.iter().find(|(_, t)| t.pulse().same(&p.pulse)) {
            k.clone()
        } else {
            let k = format!("t{}", self.pulse_generated);
            self.pulse_generated += 1;
            self.tasks.insert(k.clone(), Task::Native(p));
            k
        };
        self.emit("submit", vec![json!(name)])?;
        self.queue.push_back(name);
        Ok(())
    }
    fn ground_create(&mut self, item: Option<usize>) -> Result<()> {
        self.emit("ground", vec![json!(item)])
    }
}
impl pulses::Context<()> for Model {
    fn pulse(&mut self, p: &pulses::Handle, _: &mut ()) -> pulses::Result<bool> {
        self.pulse_direct(p)
            .map_err(|e| pulses::Error::Binding(class(&e).into()))
    }
    fn stop(&mut self, p: &pulses::Handle, _: &mut ()) -> pulses::Result<()> {
        Context::stop(self, p).map_err(|e| pulses::Error::Binding(class(&e).into()))
    }
    fn node_active(&mut self, id: usize, _: &mut ()) -> pulses::Result<bool> {
        Ok(self
            .nodes
            .values()
            .flatten()
            .find(|h| h.identity() == id)
            .unwrap()
            .read()
            .active)
    }
    fn exception(&mut self, e: &pulses::Error, _: &mut ()) -> pulses::Result<()> {
        if matches!(e,pulses::Error::Binding(s)if s=="Binding") {
            self.events.push(json!(["exception"]));
        }
        Ok(())
    }
    fn remove_weak_scripts(&mut self, _: &mut ()) -> pulses::Result<()> {
        unreachable!()
    }
    fn remove_normal_scripts(&mut self, _: &mut ()) -> pulses::Result<()> {
        unreachable!()
    }
    fn is_dead(&mut self, _: &mut ()) -> pulses::Result<bool> {
        unreachable!()
    }
    fn send_message(&mut self, _: usize, _: &str, _: &mut ()) -> pulses::Result<()> {
        unreachable!()
    }
    fn now_millis(&mut self, _: &mut ()) -> pulses::Result<i64> {
        unreachable!()
    }
    fn log(&mut self, _: pulses::Level, _: String, _: &mut ()) -> pulses::Result<()> {
        unreachable!()
    }
    fn pulse_length(&mut self, _: &str, _: i32, _: &mut ()) -> pulses::Result<()> {
        unreachable!()
    }
    fn count_pulse(&mut self, _: &str, _: &mut ()) -> pulses::Result<()> {
        unreachable!()
    }
    fn metrics_enabled(&self, _: &()) -> bool {
        false
    }
    fn set_bot_total(&mut self, _: i32, _: &mut ()) -> pulses::Result<()> {
        unreachable!()
    }
    fn set_other_total(&mut self, _: i32, _: &mut ()) -> pulses::Result<()> {
        unreachable!()
    }
}
pub fn evaluate(c: &Value) -> Value {
    let mut s = Model::new(c);
    let mut out = Vec::new();
    for a in c["actions"].as_array().unwrap() {
        s.events.clear();
        let (result, error) = match s.act(a) {
            Ok(v) => (v, Value::Null),
            Err(e) => (Value::Null, json!(class(&e))),
        };
        let mut snapshot = s.snapshot();
        snapshot["result"] = result;
        snapshot["error"] = error;
        out.push(snapshot);
    }
    json!(out)
}
