use rs09_server::{
    definitions::item::Shared,
    world::{
        component_objects::NativeObject,
        interaction_listeners::{self as alg, *},
    },
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap, HashSet};
#[derive(Clone)]
struct Node {
    tag: usize,
    id: i32,
    player: bool,
    item: bool,
}
#[derive(Clone)]
struct Player {
    scripts: usize,
    manager: usize,
    zone: usize,
}
#[derive(Clone)]
struct Body {
    kind: FunctionKind,
    id: usize,
}
impl NativeObject for Body {}
fn category(k: FunctionKind) -> &'static str {
    match k {
        FunctionKind::On => "on",
        FunctionKind::UseWith => "use",
        FunctionKind::Predicate => "predicate",
        FunctionKind::Destination => "destination",
        FunctionKind::Interaction => "interaction",
        FunctionKind::UseInteraction => "useInteraction",
        FunctionKind::Void => "void",
    }
}
fn class(k: FunctionKind) -> &'static str {
    match k {
        FunctionKind::On => "oracle.OnHandler",
        FunctionKind::UseWith => "oracle.UseHandler",
        FunctionKind::Predicate => "oracle.PredHandler",
        FunctionKind::Destination => "oracle.DestHandler",
        FunctionKind::Interaction => "oracle.InterHandler",
        FunctionKind::UseInteraction => "oracle.UseInterHandler",
        FunctionKind::Void => "oracle.VoidHandler",
    }
}
fn descriptor(f: &Function) -> Value {
    let b = f.value::<Body>().unwrap();
    json!([category(b.kind), b.id])
}
fn n(v: &Value, key: &str, default: i32) -> i32 {
    v[key].as_i64().map_or(default, |v| v as i32)
}
fn b(v: &Value, key: &str, default: bool) -> bool {
    v[key].as_bool().unwrap_or(default)
}
fn s<'a>(v: &'a Value, key: &str, default: &'a str) -> &'a str {
    v[key].as_str().unwrap_or(default)
}
fn t(s: &str) -> Text {
    s.encode_utf16().collect()
}
fn ints(v: &Value, default: i32) -> Vec<i32> {
    v.as_array().map_or_else(
        || vec![default],
        |v| v.iter().map(|v| v.as_i64().unwrap() as i32).collect(),
    )
}
fn opts(a: &Value) -> Vec<Text> {
    a["options"].as_array().map_or_else(
        || vec![t(s(a, "option", "use"))],
        |v| v.iter().map(|v| t(v.as_str().unwrap())).collect(),
    )
}
struct Model {
    registry: Registry,
    input: Value,
    nodes: Vec<Shared<Node>>,
    player: Shared<Player>,
    functions: Vec<Vec<Function>>,
    events: Vec<Value>,
    calls: HashMap<String, i32>,
    hooks: HashSet<usize>,
    pulses: Vec<Arrival<Shared<Player>, Shared<Node>>>,
    result: bool,
    predicate: bool,
    zone: bool,
    locked: bool,
    movement: bool,
    locale: String,
    option: Option<Text>,
    installed: Value,
}
impl Model {
    fn new(input: &Value) -> Self {
        let registry = Registry::default();
        if b(input, "instant", false) {
            registry
                .instant_classes
                .write()
                .extend(["oracle.OnHandler".into(), "oracle.UseHandler".into()]);
        }
        Self {
            registry,
            input: input.clone(),
            nodes: (0..4)
                .map(|tag| {
                    Shared::new(Node {
                        tag,
                        id: tag as i32,
                        player: tag == 0 || tag == 3,
                        item: tag == 1,
                    })
                })
                .collect(),
            player: Shared::new(Player {
                scripts: 0,
                manager: 0,
                zone: 0,
            }),
            functions: [
                FunctionKind::On,
                FunctionKind::UseWith,
                FunctionKind::Predicate,
                FunctionKind::Destination,
                FunctionKind::Interaction,
                FunctionKind::UseInteraction,
            ]
            .into_iter()
            .map(|kind| {
                (0..2)
                    .map(|id| Function::new(kind, class(kind), Body { kind, id }))
                    .collect()
            })
            .collect(),
            events: vec![],
            calls: HashMap::new(),
            hooks: HashSet::new(),
            pulses: vec![],
            result: b(input, "result", true),
            predicate: b(input, "predicate", true),
            zone: b(input, "zone", false),
            locked: b(input, "locked", false),
            movement: b(input, "movementLocked", false),
            locale: s(input, "locale", "ROOT").into(),
            option: None,
            installed: Value::Null,
        }
    }
    fn function(&self, kind: usize, id: usize) -> Function {
        self.functions[kind][id].clone()
    }
    fn emit(&mut self, name: &str, args: Vec<Value>) -> Result<()> {
        let mut event = vec![json!(name)];
        event.extend(args);
        self.events.push(json!(event));
        let at = self.calls.entry(name.into()).or_default();
        *at += 1;
        let at = *at;
        let hooks = self.input["hooks"].as_array().cloned().unwrap_or_default();
        for (i, h) in hooks.iter().enumerate() {
            if s(h, "event", "") == name && n(h, "at", 1) == at && self.hooks.insert(i) {
                for a in h["actions"].as_array().unwrap() {
                    self.act(a)?;
                }
            }
        }
        if self.input["failures"][name].as_i64() == Some(i64::from(at)) {
            return Err(Error::Binding("recorded failure".into()));
        }
        Ok(())
    }
    fn act(&mut self, a: &Value) -> Result<Value> {
        let reg = self.registry.clone();
        let f = n(a, "function", 0) as usize;
        let kind = n(a, "kind", 1);
        let k = match kind {
            0 => Kind::Item,
            2 => Kind::Npc,
            3 => Kind::GroundItem,
            4 => Kind::Player,
            _ => Kind::Scenery,
        };
        let id = n(a, "id", 123);
        let option = t(s(a, "option", "use"));
        let u = n(a, "used", 1);
        let w = n(a, "with_", 2);
        match s(a, "op", "") {
            "add" => reg.add(self, id, kind, &opts(a), self.function(0, f))?,
            "addArrays" => reg.add_ids(
                self,
                &ints(&a["ids"], id),
                kind,
                &opts(a),
                self.function(0, f),
            )?,
            "generic" => reg.add_generics(self, &opts(a), kind, self.function(0, f))?,
            "use" => reg.add_use(u, w, kind, self.function(1, f))?,
            "useIds" => reg.add_use_ids(kind, u, &ints(&a["with_"], w), self.function(1, f))?,
            "useArrays" => reg.add_use_arrays(
                kind,
                &ints(&a["used"], u),
                &ints(&a["with_"], w),
                self.function(1, f),
            )?,
            "any" => reg.add_any(kind, &ints(&a["with_"], w), self.function(1, f))?,
            "wildcard" => reg.add_wildcard(kind, self.function(2, f), self.function(1, f))?,
            "equip" => reg.add_equip(id, self.function(0, f), b(a, "equip", true))?,
            "get" => {
                return Ok(reg
                    .get(self, id, kind, &option)
                    .map_or(Value::Null, |f| descriptor(&f)));
            }
            "getGeneric" => {
                return Ok(reg
                    .get_generic(self, &option, kind)
                    .map_or(Value::Null, |f| descriptor(&f)));
            }
            "getEquip" => {
                return Ok(reg
                    .get_equip(id, b(a, "equip", true))
                    .map_or(Value::Null, |f| descriptor(&f)));
            }
            "getDest" => {
                return Ok(match s(a, "mode", "specific") {
                    "specific" => reg.destination(self, kind, id, &option),
                    "id" => reg.destination_id(kind, id),
                    _ => reg.destination_generic(kind, &option),
                }
                .map_or(Value::Null, |f| descriptor(&f)));
            }
            "instant" => {
                let name = s(a, "class", "oracle.OnHandler").to_owned();
                if b(a, "enabled", true) {
                    reg.instant_classes.write().insert(name);
                } else {
                    reg.instant_classes.write().remove(&name);
                }
            }
            "isInstant" => {
                return Ok(json!(
                    reg.is_instant(&self.function(usize::from(b(a, "use", false)), f))
                ));
            }
            "getUse" => {
                return Ok(reg
                    .get_use(self, u, w, kind)?
                    .map_or(Value::Null, |f| descriptor(&f)));
            }
            "destination" => match s(a, "mode", "specific") {
                "specific" => {
                    reg.add_destinations(self, kind, Some(&[id]), &opts(a), self.function(3, f))?
                }
                "id" => reg.add_destination(kind, id, self.function(3, f))?,
                _ => reg.add_destinations(self, kind, None, &opts(a), self.function(3, f))?,
            },
            "metadata" => {
                let meta = Shared::new(Metadata {
                    function: self.function(4, f),
                    distance: n(a, "distance", 1),
                    persist: b(a, "persist", false),
                });
                if b(a, "generic", false) {
                    reg.add_generic_metadata(k, &opts(a), meta);
                } else {
                    reg.add_metadata(self, &ints(&a["ids"], id), k, &opts(a), meta);
                }
            }
            "useMetadata" => {
                let meta = Shared::new(Metadata {
                    function: self.function(5, f),
                    distance: n(a, "distance", 1),
                    persist: b(a, "persist", false),
                });
                if a["with_"].is_array() {
                    reg.add_use_metadata_array(u, &ints(&a["with_"], w), 1, k, meta);
                } else {
                    reg.add_use_metadata(u, w, k, meta);
                }
            }
            "run" => {
                return reg
                    .run(
                        self,
                        id,
                        k,
                        &option,
                        self.player.clone(),
                        self.nodes[n(a, "node", 2) as usize].clone(),
                    )
                    .map(|v| json!(v));
            }
            "runUse" => {
                return reg
                    .run_use(
                        self,
                        self.nodes[n(a, "usedNode", 1) as usize].clone(),
                        self.nodes[n(a, "withNode", 2) as usize].clone(),
                        k,
                        self.player.clone(),
                    )
                    .map(|v| json!(v));
            }
            "runEquip" => {
                return reg
                    .run_equip(
                        self,
                        id,
                        self.player.clone(),
                        self.nodes[n(a, "node", 2) as usize].clone(),
                        b(a, "equip", true),
                    )
                    .map(|v| json!(v));
            }
            "pulse" => {
                if let Some(p) = self.pulses.get(n(a, "index", 0) as usize).cloned() {
                    return alg::arrival(self, p).map(|v| json!(v));
                }
            }
            "field" => match s(a, "name", "") {
                "manager" => self.player.write().manager = n(a, "value", 0) as usize,
                "scripts" => self.player.write().scripts = n(a, "value", 0) as usize,
                "zone" => self.player.write().zone = n(a, "value", 0) as usize,
                "nodeId" => self.nodes[n(a, "node", 2) as usize].write().id = n(a, "value", 0),
                "locked" => self.locked = b(a, "value", false),
                "movement" => self.movement = b(a, "value", false),
                "result" => self.result = b(a, "value", false),
                "predicate" => self.predicate = b(a, "value", false),
                "locale" => self.locale = s(a, "value", "").into(),
                _ => {}
            },
            other => panic!("unknown original listener request {other}"),
        }
        Ok(Value::Null)
    }
    fn snapshot(&self, result: Value, error: Option<&str>, message: Value) -> Value {
        let mut maps = BTreeMap::new();
        for (name, mut entries) in self.registry.inspect() {
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            let rows = entries
                .into_iter()
                .map(|(key, value)| {
                    let mut key = key;
                    let key_s = String::from_utf16(&key).unwrap();
                    if let Some(at) = key_s.find("[I@") {
                        key = t(&format!("{}[I@array", &key_s[..at]));
                    }
                    let value = match value {
                        Entry::Function(f) => descriptor(&f),
                        Entry::Metadata(m) => {
                            let m = m.read();
                            json!([descriptor(&m.function), m.distance, m.persist])
                        }
                        Entry::Wildcards(pairs) => json!(
                            pairs
                                .iter()
                                .map(|p| json!([descriptor(&p.predicate), descriptor(&p.handler)]))
                                .collect::<Vec<_>>()
                        ),
                    };
                    json!([key, value])
                })
                .collect::<Vec<_>>();
            maps.insert(name, rows);
        }
        let p = self.player.read();
        json!({"result":result,"error":error,"message":message,"events":self.events,"maps":maps,"option":self.option,"installed":self.installed,"pulseCount":self.pulses.len(),"playerFields":[p.scripts,p.manager,p.zone],"nodeIds":self.nodes.iter().map(|n|n.read().id).collect::<Vec<_>>()})
    }
}
impl Casing for Model {
    fn lowercase_default(&self, s: &[u16]) -> Text {
        let s = String::from_utf16(s).unwrap();
        let s = if self.locale == "tr" {
            s.replace('I', "ı").replace('İ', "i")
        } else {
            s
        };
        t(&rs09_game_core::java_lower::lowercase_root(&s))
    }
    fn lowercase_root(&self, s: &[u16]) -> Text {
        t(&rs09_game_core::java_lower::lowercase_root(
            &String::from_utf16(s).unwrap(),
        ))
    }
}
impl Context for Model {
    type Player = Shared<Player>;
    type Node = Shared<Node>;
    type Scripts = usize;
    type Locks = ();
    type Script = (Function, i32, bool);
    type Manager = usize;
    type Pulse = usize;
    type Zone = usize;
    type Location = usize;
    fn scripts(&mut self, p: &Self::Player) -> Result<usize> {
        self.emit("scripts", vec![json!(0)])?;
        Ok(p.read().scripts)
    }
    fn remove_weak(&mut self, s: usize) -> Result<()> {
        self.emit("removeWeak", vec![json!(s)])
    }
    fn locks(&mut self, _: &Self::Player) -> Result<()> {
        self.emit("locks", vec![json!(0)])
    }
    fn interaction_locked(&mut self, _: ()) -> Result<bool> {
        self.emit("interactionLocked", vec![])?;
        Ok(self.locked)
    }
    fn movement_locked(&mut self, _: ()) -> Result<bool> {
        self.emit("movementLocked", vec![])?;
        Ok(self.movement)
    }
    fn set_option(&mut self, _: &Self::Player, option: Text) -> Result<()> {
        self.emit("setOption", vec![json!(0), json!(option)])?;
        self.option = Some(option);
        Ok(())
    }
    fn dispatch_on(&mut self, _: &Self::Player, n: &Self::Node, option: Text) -> Result<()> {
        let tag = n.read().tag;
        self.emit("dispatchOn", vec![json!(0), json!(tag), json!(option)])
    }
    fn dispatch_use(&mut self, _: &Self::Player, u: i32, w: i32) -> Result<()> {
        self.emit("dispatchUse", vec![json!(0), json!(u), json!(w)])
    }
    fn interaction(&mut self, f: Function, d: i32, p: bool) -> Result<Self::Script> {
        Ok((f, d, p))
    }
    fn install(&mut self, s: usize, n: &Self::Node, script: Self::Script) -> Result<()> {
        let tag = n.read().tag;
        let (f, d, p) = script;
        self.emit(
            "install",
            vec![json!(s), json!(tag), descriptor(&f), json!(d), json!(p)],
        )?;
        self.installed = descriptor(&f);
        Ok(())
    }
    fn manager(&mut self, p: &Self::Player) -> Result<usize> {
        self.emit("manager", vec![json!(0)])?;
        Ok(p.read().manager)
    }
    fn movement(
        &mut self,
        a: Arrival<Self::Player, Self::Node>,
        flag: Option<Flag>,
        dest: Option<Function>,
    ) -> Result<usize> {
        let target = match &a {
            Arrival::Empty { target, .. } | Arrival::On { target, .. } => target,
            Arrival::Use { with, .. } => with,
        };
        let tag = target.read().tag;
        let flag = flag.map(|f| match f {
            Flag::Entity => "ENTITY",
            Flag::Item => "ITEM",
            Flag::Object => "OBJECT",
        });
        self.emit(
            "movement",
            vec![
                json!(0),
                json!(tag),
                json!(flag),
                dest.as_ref().map_or(Value::Null, descriptor),
            ],
        )?;
        let id = self.pulses.len();
        self.pulses.push(a);
        Ok(id)
    }
    fn run(&mut self, m: usize, p: usize) -> Result<()> {
        self.emit("runPulse", vec![json!(m), json!(p)])
    }
    fn invoke_on(&mut self, f: Function, _: &Self::Player, n: &Self::Node) -> Result<bool> {
        let id = f.value::<Body>().ok_or(Error::Type)?.id;
        let tag = n.read().tag;
        self.emit("invokeOn", vec![json!(id), json!(0), json!(tag)])?;
        Ok(self.result)
    }
    fn invoke_use(
        &mut self,
        f: Function,
        _: &Self::Player,
        u: &Self::Node,
        w: &Self::Node,
    ) -> Result<bool> {
        let id = f.value::<Body>().ok_or(Error::Type)?.id;
        let u = u.read().tag;
        let w = w.read().tag;
        self.emit("invokeUse", vec![json!(id), json!(0), json!(u), json!(w)])?;
        Ok(self.result)
    }
    fn predicate(&mut self, f: Function, u: i32, w: i32) -> Result<bool> {
        let id = f.value::<Body>().ok_or(Error::Type)?.id;
        self.emit("predicate", vec![json!(id), json!(u), json!(w)])?;
        Ok(self.predicate)
    }
    fn id(&mut self, n: &Self::Node) -> Result<i32> {
        let tag = n.read().tag;
        self.emit("id", vec![json!(tag)])?;
        Ok(n.read().id)
    }
    fn is_player(&self, n: &Self::Node) -> bool {
        n.read().player
    }
    fn zone(&mut self, p: &Self::Player) -> Result<usize> {
        self.emit("zone", vec![json!(0)])?;
        Ok(p.read().zone)
    }
    fn zone_on(&mut self, z: usize, n: &Self::Node, o: Text, i: i32) -> Result<bool> {
        let tag = n.read().tag;
        self.emit("zoneOn", vec![json!(z), json!(tag), json!(o), json!(i)])?;
        Ok(self.zone)
    }
    fn as_item(&mut self, n: &Self::Node) -> Result<Self::Node> {
        if n.read().item {
            Ok(n.clone())
        } else {
            Err(Error::Type)
        }
    }
    fn zone_use(&mut self, z: usize, u: &Self::Node, w: &Self::Node) -> Result<bool> {
        let u = u.read().tag;
        let w = w.read().tag;
        self.emit("zoneUse", vec![json!(z), json!(u), json!(w)])?;
        Ok(self.zone)
    }
    fn location(&mut self, n: &Self::Node) -> Result<usize> {
        let tag = n.read().tag;
        self.emit("location", vec![json!(tag)])?;
        Ok(tag)
    }
    fn face(&mut self, _: &Self::Player, l: usize) -> Result<()> {
        self.emit("face", vec![json!(0), json!(l)])
    }
}
pub fn evaluate(input: &Value) -> Value {
    let mut model = Model::new(input);
    let mut expected = vec![];
    for a in input["actions"].as_array().unwrap() {
        let (result, error, message) = match model.act(a) {
            Ok(v) => (v, None, Value::Null),
            Err(e) => match e {
                Error::Null => (Value::Null, Some("Null"), Value::Null),
                Error::Index => (Value::Null, Some("Index"), Value::Null),
                Error::Type => (Value::Null, Some("Type"), Value::Null),
                Error::Concurrent => (Value::Null, Some("Concurrent"), Value::Null),
                Error::State(t) => (Value::Null, Some("State"), json!(t)),
                Error::Binding(_) => (Value::Null, Some("Binding"), Value::Null),
            },
        };
        expected.push(model.snapshot(result, error, message));
    }
    json!(expected)
}
