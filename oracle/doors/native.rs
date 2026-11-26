//! Test-only native bindings for the original door-rule observations.
use rs09_server::{
    definitions::item::Shared,
    pulses,
    world::doors::{
        self, Action, Context, Diary, Door, DoorPulse, Error, Handle, Loader, Location, Registry,
        Result, Scenery,
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
    let a = v.as_array().unwrap();
    Location::new(n(&a[0]), n(&a[1]), n(&a[2]))
}
fn error(e: Error) -> &'static str {
    match e {
        Error::Null => "Null",
        Error::Type => "Type",
        Error::Number => "Number",
        Error::Direction => "Direction",
        Error::Json(_) => "Json",
        Error::Binding(_) => "Binding",
        Error::Index => "Index",
        Error::Io(_) => "Io",
    }
}
struct Model {
    input: Value,
    nodes: BTreeMap<String, Handle>,
    defs: BTreeMap<i32, (Option<String>, bool)>,
    region: Vec<String>,
    own: Location,
    player: bool,
    listener: Option<Location>,
    registry: Registry,
    loader: Loader,
    refs: BTreeMap<String, Option<Door>>,
    tasks: Vec<DoorPulse>,
    queue: VecDeque<usize>,
    events: Vec<Value>,
    calls: BTreeMap<String, i32>,
    enabled: bool,
    generated: usize,
    move_near: bool,
}
impl Model {
    fn new(c: &Value) -> Self {
        let mut s = Self {
            input: c.clone(),
            nodes: BTreeMap::new(),
            defs: BTreeMap::new(),
            region: c["region"].as_array().map_or(Vec::new(), |a| {
                a.iter().map(|v| v.as_str().unwrap().to_owned()).collect()
            }),
            own: location(c.get("own").unwrap_or(&json!([3207, 3217, 0]))),
            player: c["player"] != false,
            listener: None,
            registry: Registry::default(),
            loader: Loader::default(),
            refs: BTreeMap::new(),
            tasks: Vec::new(),
            queue: VecDeque::new(),
            events: Vec::new(),
            calls: BTreeMap::new(),
            enabled: false,
            generated: 0,
            move_near: c["moveNear"] == true,
        };
        for row in c["nodes"].as_object().unwrap().values() {
            s.defs.insert(
                n(&row["id"]),
                (
                    Some(row["name"].as_str().unwrap_or("Door").into()),
                    row["close"] == true,
                ),
            );
        }
        for (key, row) in c["nodes"].as_object().unwrap() {
            let node = s
                .construct(
                    n(&row["id"]),
                    location(&row["location"]),
                    val(row, "shape", 0),
                    val(row, "rotation", 0),
                )
                .unwrap();
            node.write().constructed = row["constructed"] == true;
            node.write().charge = val(row, "charge", 1000);
            s.nodes.insert(key.clone(), node);
        }
        for (key, row) in c["nodes"].as_object().unwrap() {
            let node = s.nodes[key].clone();
            node.write().wrapper = row["wrapper"]
                .as_str()
                .and_then(|k| s.nodes.get(k).cloned());
            node.write().replaced = row["replaced"]
                .as_str()
                .and_then(|k| s.nodes.get(k).cloned());
        }
        s.load(&c["configs"].to_string()).unwrap();
        s.enabled = true;
        s
    }
    fn label(&mut self, h: &Handle) -> String {
        if let Some((key, _)) = self.nodes.iter().find(|(_, v)| v.same(h)) {
            return key.clone();
        }
        let name = format!("g{}", self.generated);
        self.generated += 1;
        self.nodes.insert(name.clone(), h.clone());
        name
    }
    fn emit(&mut self, name: &str, args: Vec<Value>) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }
        let mut row = vec![json!(name)];
        row.extend(args);
        self.events.push(Value::Array(row));
        let count = self.calls.entry(name.into()).or_default();
        *count += 1;
        let count = *count;
        let hooks = self.input["hooks"].as_array().cloned().unwrap_or_default();
        for hook in hooks {
            if hook["event"] == name && val(&hook, "at", 1) == count {
                for a in hook["actions"].as_array().unwrap() {
                    self.act(a)?;
                }
            }
        }
        Ok(())
    }
    fn load(&mut self, text: &str) -> Result<()> {
        let registry = self.registry.clone();
        let mut loader = std::mem::take(&mut self.loader);
        let result = loader.load_text(text, &registry, &mut |n| {
            self.emit("log", vec![json!(format!("Parsed {n} door configs."))])
        });
        self.loader = loader;
        result.map(|_| ())
    }
    fn node(&self, a: &Value, key: &str) -> Option<Handle> {
        a[key].as_str().and_then(|n| self.nodes.get(n).cloned())
    }
    fn act(&mut self, a: &Value) -> Result<Value> {
        let op = a["op"].as_str().unwrap();
        let object = self.node(a, "node");
        let second = self.node(a, "second");
        let obj = || object.clone().ok_or(Error::Null);
        match op {
            "handle" => {
                if !self.player {
                    return Err(Error::Type);
                }
                doors::handle(self, 0, &obj()?)?;
            }
            "end" => return Ok(json!(doors::end_location(self, 0, &obj()?)?.coordinates())),
            "destination" => return Ok(json!(doors::destination(self, 0, &obj()?)?.coordinates())),
            "second" => {
                let h = doors::second_door(self, &obj()?)?;
                return Ok(h.map_or(Value::Null, |h| json!(self.label(&h))));
            }
            "point" => return Ok(json!(doors::rotation_point(n(&a["value"])))),
            "rotations" => {
                let p = &a["point"];
                return Ok(json!(doors::rotations(
                    &obj()?,
                    second.as_ref(),
                    [n(&p[0]), n(&p[1])]
                )));
            }
            "open" => doors::open(
                self,
                &obj()?,
                second.as_ref(),
                val(a, "id", 1),
                val(a, "otherId", 2),
                a["clip"] != false,
                val(a, "ticks", 500),
                a["fence"] == true,
            )?,
            "autowalk" => {
                return Ok(json!(doors::autowalk(
                    self,
                    0,
                    &obj()?,
                    location(&a["end"])
                )?));
            }
            "autowalkFence" => {
                return Ok(json!(doors::autowalk_fence(
                    self,
                    0,
                    &obj()?,
                    val(a, "id", 1),
                    val(a, "otherId", 2)
                )?));
            }
            "tick" => {
                let count = self.queue.len();
                for _ in 0..count {
                    let i = self.queue.pop_front().unwrap();
                    let p = self.tasks[i].pulse.clone();
                    if !pulses::update_base(&p, &mut (), self)
                        .map_err(|_| Error::Binding("pulse".into()))?
                        && p.read().running
                    {
                        self.queue.push_back(i);
                    }
                }
            }
            "logout" => {
                if self.player
                    && let Some(l) = self.listener.clone()
                {
                    self.emit("setLocation", vec![json!(l.coordinates())])?;
                    self.own = l;
                }
            }
            "fail" => return Err(Error::Binding("callback".into())),
            "rotate" => obj()?.write().rotation = n(&a["value"]),
            "charge" => obj()?.write().charge = n(&a["value"]),
            "own" => {
                let l = location(&a["value"]);
                self.emit("setLocation", vec![json!(l.coordinates())])?;
                self.own = l;
            }
            "region" => {
                self.region = a["value"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_str().unwrap().to_owned())
                    .collect()
            }
            "load" => self.load(a["text"].as_str().unwrap())?,
            "retain" => {
                self.refs.insert(
                    a["name"].as_str().unwrap().into(),
                    self.registry.get(n(&a["id"])),
                );
            }
            "configSet" => {
                let d = self.registry.get(n(&a["id"])).ok_or(Error::Null)?;
                let mut d = d.write();
                match a["field"].as_str().unwrap() {
                    "replaceId" => d.replace_id = n(&a["value"]),
                    "metal" => d.metal = a["value"].as_bool().unwrap(),
                    "fence" => d.fence = a["value"].as_bool().unwrap(),
                    _ => d.autowalk = a["value"].as_bool().unwrap(),
                }
            }
            _ => panic!("unknown action {op}"),
        }
        Ok(Value::Null)
    }
    fn snapshot(&mut self) -> Value {
        let mut nodes = BTreeMap::new();
        let all: Vec<_> = self
            .nodes
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        for (key, h) in all {
            let h = h.read();
            let previous = h.replaced.clone();
            let wrapper = h.wrapper.clone();
            nodes.insert(key.clone(),json!({"id":h.id,"shape":h.shape,"rotation":h.rotation,"direction":h.direction.code(),"location":h.location.coordinates(),"name":h.name,"close":h.close,"charge":h.charge,"constructed":h.constructed,"replaced":if h.constructed{previous.map(|p|self.label(&p))}else{None},"wrapper":wrapper.filter(|w|!w.same(&self.nodes[&key])).map(|w|self.label(&w))}));
        }
        let configs: BTreeMap<_, _> = self
            .registry
            .values
            .read()
            .iter()
            .map(|(id, d)| (id.to_string(), door(d)))
            .collect();
        let refs: BTreeMap<_, _> = self
            .refs
            .iter()
            .map(|(k, v)| (k.clone(), v.as_ref().map_or(Value::Null, door)))
            .collect();
        let pulses: Vec<_> = self
            .tasks
            .iter()
            .map(|p| {
                let b = p.pulse.read();
                let opened = match &*p.action.read() {
                    Action::Autowalk { opened, .. } | Action::Fence { opened, .. } => *opened,
                };
                json!({"running":b.running,"ticks":b.ticks_passed,"opened":opened})
            })
            .collect();
        json!({"nodes":nodes,"configs":configs,"refs":refs,"queue":self.queue,"pulses":pulses,"own":self.own.coordinates(),"listener":self.listener.is_some(),"events":self.events})
    }
}
fn door(d: &Door) -> Value {
    let d = d.read();
    json!({"id":d.id,"replaceId":d.replace_id,"fence":d.fence,"metal":d.metal,"autowalk":d.autowalk,"quest":d.quest_requirement})
}
impl Context for Model {
    fn config(&mut self, id: i32) -> Result<Option<Door>> {
        Ok(self.registry.get(id))
    }
    fn object(&mut self, l: Location, shape: i32) -> Result<Option<Handle>> {
        self.emit("object", vec![json!(l.coordinates()), json!(shape)])?;
        Ok(self
            .region
            .iter()
            .filter_map(|k| self.nodes.get(k))
            .find(|h| {
                let h = h.read();
                h.shape == shape && h.location.coordinates() == l.coordinates()
            })
            .cloned())
    }
    fn construct(&mut self, id: i32, l: Location, shape: i32, rotation: i32) -> Result<Handle> {
        let (name, close) = self
            .defs
            .entry(id)
            .or_insert_with(|| (Some("Door".into()), false))
            .clone();
        Ok(Shared::new(Scenery::new(
            id, l, shape, rotation, name, close,
        )?))
    }
    fn replace(
        &mut self,
        old: &Handle,
        new: Handle,
        ticks: Option<i32>,
        clip: bool,
    ) -> Result<bool> {
        let a = self.label(old);
        let b = self.label(&new);
        self.emit(
            "replace",
            vec![json!(a), json!(b), json!(ticks), json!(clip)],
        )?;
        Ok(self.input["replaceResult"] != false)
    }
    fn is_player(&mut self, _: usize) -> bool {
        self.player
    }
    fn entity_location(&mut self, _: usize) -> Result<Location> {
        self.emit("location", vec![])?;
        Ok(self.own.clone())
    }
    fn lock(&mut self, _: usize, ticks: i32) -> Result<()> {
        self.emit("lock", vec![json!(ticks)])
    }
    fn audio(&mut self, _: usize, id: i32, delay: Option<i32>) -> Result<()> {
        self.emit("audio", vec![json!(id), json!(delay)])
    }
    fn message(&mut self, _: usize, text: &str) -> Result<()> {
        self.emit("message", vec![json!(text)])
    }
    fn logout_listener(&mut self, _: usize, l: Option<Location>) -> Result<()> {
        self.emit(
            if l.is_some() {
                "logoutAdd"
            } else {
                "logoutRemove"
            },
            vec![],
        )?;
        self.listener = l;
        Ok(())
    }
    fn reset_path(&mut self, _: usize) -> Result<()> {
        self.emit("walkReset", vec![])
    }
    fn add_path(&mut self, _: usize, x: i32, y: i32) -> Result<()> {
        self.emit("walkAdd", vec![json!(x), json!(y)])
    }
    fn path_move_near(&mut self, _: usize, l: Location) -> Result<bool> {
        self.emit("find", vec![json!(l.coordinates())])?;
        Ok(self.move_near)
    }
    fn diary(&mut self, _: usize, d: Diary, l: i32, t: i32) -> Result<()> {
        self.emit(
            "diary",
            vec![
                json!(if d == Diary::Falador {
                    "FALADOR"
                } else {
                    "LUMBRIDGE"
                }),
                json!(l),
                json!(t),
            ],
        )
    }
    fn submit(&mut self, p: DoorPulse) -> Result<()> {
        let index = self.tasks.len();
        self.emit("submit", vec![json!(index)])?;
        self.tasks.push(p);
        self.queue.push_back(index);
        Ok(())
    }
}
impl pulses::Context<()> for Model {
    fn pulse(&mut self, p: &pulses::Handle, _: &mut ()) -> pulses::Result<bool> {
        let t = self.tasks.iter().find(|t| t.pulse.same(p)).unwrap().clone();
        doors::pulse_body(self, &t).map_err(|_| pulses::Error::Binding("callback".into()))
    }
    fn node_active(&mut self, _: usize, _: &mut ()) -> pulses::Result<bool> {
        Ok(true)
    }
    fn exception(&mut self, _: &pulses::Error, _: &mut ()) -> pulses::Result<()> {
        self.events.push(json!(["exception"]));
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
    let mut result = Vec::new();
    for a in c["actions"].as_array().unwrap() {
        s.events.clear();
        let (value, failure) = match s.act(a) {
            Ok(v) => (v, Value::Null),
            Err(e) => (Value::Null, json!(error(e))),
        };
        let mut state = s.snapshot();
        state["result"] = value;
        state["error"] = failure;
        result.push(state);
    }
    json!(result)
}
