//! Recording boundaries mirror the explicit JVM adapters. Algorithms use the
//! production native definitions/store/API/manager and actual packet encoders.
use rs09_game_core::java_json::{self, Json};
use rs09_protocol::game::updates;
use rs09_server::{
    definitions::varbit::{self, Handle, Reader, Registry},
    player::variables::{self as v, Context, Error, Manager, Saved, Store, Values},
    world::{
        doors::Location,
        player_walk::{NodeHandle, State},
    },
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap};

fn n(row: &Value, key: &str, default: i32) -> i32 {
    row[key].as_i64().map(|n| n as i32).unwrap_or(default)
}
fn text(row: &Value, key: &str) -> String {
    row[key].as_str().unwrap_or("").into()
}
fn parsed(value: &Value) -> Json {
    java_json::parse(&value.to_string()).unwrap()
}
enum Identity {
    Values(Values),
    Saved(Saved),
    Def(Handle),
    Reader(Reader),
}
impl Identity {
    fn same(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Values(a), Self::Values(b)) => a.same(b),
            (Self::Saved(a), Self::Saved(b)) => a.same(b),
            (Self::Def(a), Self::Def(b)) => a.same(b),
            (Self::Reader(a), Self::Reader(b)) => a.0.same(&b.0),
            _ => false,
        }
    }
}
#[derive(Default)]
struct Ids(Vec<Identity>);
impl Ids {
    fn tag(&mut self, value: Identity) -> usize {
        if let Some(id) = self.0.iter().position(|old| old.same(&value)) {
            return id;
        }
        let id = self.0.len();
        self.0.push(value);
        id
    }
    fn values(&mut self, value: Option<&Values>) -> Value {
        value.map(|value| json!({"ref":self.tag(Identity::Values(value.clone())),"values":value.snapshot().into_iter().map(|(k,v)|(k.to_string(),v)).collect::<BTreeMap<_,_>>()})).unwrap_or(Value::Null)
    }
    fn saved(&mut self, value: Option<&Saved>) -> Value {
        value.map(|value| json!({"ref":self.tag(Identity::Saved(value.clone())),"values":value.snapshot().into_iter().map(|(k,v)|(k.to_string(),v)).collect::<BTreeMap<_,_>>()})).unwrap_or(Value::Null)
    }
    fn def(&mut self, value: Option<&Handle>) -> Value {
        value
            .map(|value| {
                let id = self.tag(Identity::Def(value.clone()));
                let d = value.read();
                json!([id, d.id, d.varp_id, d.start_bit, d.end_bit, d.text()])
            })
            .unwrap_or(Value::Null)
    }
    fn reader(&mut self, value: Option<&Reader>) -> Value {
        value
            .map(|value| json!(self.tag(Identity::Reader(value.clone()))))
            .unwrap_or(Value::Null)
    }
}
struct Host {
    row: Value,
    nodes: Vec<NodeHandle>,
    stores: Vec<Store>,
    manager: Manager,
    registry: Registry,
    loader: varbit::Loader,
    aliases: BTreeMap<String, Option<Handle>>,
    held_values: Vec<Option<Values>>,
    held_saved: Vec<Option<Saved>>,
    readers: Vec<Option<Reader>>,
    counts: HashMap<String, i32>,
    counters: [i32; 2],
    log: Vec<Value>,
    frames: Vec<Value>,
    loading: Value,
}
impl Host {
    fn hit(&mut self, label: &str, args: Vec<Value>) -> v::Result<()> {
        let mut event = vec![json!(label)];
        event.extend(args);
        self.log.push(json!(event));
        let count = self.counts.entry(label.into()).or_default();
        *count += 1;
        let count = *count;
        let hooks = self.row["hooks"].as_array().unwrap().clone();
        for hook in hooks {
            if hook["when"] != label || n(&hook, "at", 1) != count {
                continue;
            }
            match text(&hook, "action").as_str() {
                "fail" => return Err(Error::Binding("controlled".into())),
                "put" => {
                    self.stores[0]
                        .0
                        .read()
                        .values
                        .clone()
                        .ok_or(Error::Null)?
                        .put(n(&hook, "id", 0), Some(n(&hook, "value", 7)));
                }
                "clear" => self.stores[0]
                    .0
                    .read()
                    .values
                    .clone()
                    .ok_or(Error::Null)?
                    .clear(),
                "replace" => {
                    let map = Values::default();
                    map.put(n(&hook, "id", 9), Some(n(&hook, "value", 7)));
                    self.stores[0].0.write().values = Some(map);
                }
                "savedNull" => self.stores[0].0.write().saved = None,
                "savedReplace" => self.stores[0].0.write().saved = Some(Saved::default()),
                "create" => self.registry.create(
                    n(&hook, "varp", 2),
                    n(&hook, "id", 7),
                    n(&hook, "start", 2),
                    n(&hook, "end", 4),
                ),
                "counter" => self.counters[0] = n(&hook, "value", i32::MAX),
                action => panic!("unexpected hook {action}"),
            }
        }
        Ok(())
    }
    fn nil(&self, key: &str) -> bool {
        self.row["null"] == key
    }
    fn state(&self) -> Value {
        let mut ids = Ids::default();
        let (mut values, mut saved) = (vec![], vec![]);
        for store in &self.stores {
            let f = store.0.read();
            values.push(ids.values(f.values.as_ref()));
            saved.push(ids.saved(f.saved.as_ref()));
        }
        let mut mapping = BTreeMap::new();
        let sorted = self
            .registry
            .mapping()
            .read()
            .iter()
            .map(|(k, v)| (*k, v.clone()))
            .collect::<BTreeMap<_, _>>();
        for (k, v) in sorted {
            mapping.insert(k.to_string(), ids.def(v.as_ref()));
        }
        let aliases = self
            .aliases
            .iter()
            .map(|(k, v)| (k.clone(), ids.def(v.as_ref())))
            .collect::<BTreeMap<_, _>>();
        let hv = self
            .held_values
            .iter()
            .map(|v| ids.values(v.as_ref()))
            .collect::<Vec<_>>();
        let hs = self
            .held_saved
            .iter()
            .map(|v| ids.saved(v.as_ref()))
            .collect::<Vec<_>>();
        let rs = self
            .readers
            .iter()
            .map(|r| ids.reader(r.as_ref()))
            .collect::<Vec<_>>();
        let reader = ids.reader(self.loader.reader().as_ref());
        json!({"values":values,"saved":saved,"mapping":mapping,"aliases":aliases,"heldValues":hv,"heldSaved":hs,"counters":self.counters,"reader":reader,"readers":rs})
    }
    fn action(&mut self, a: &Value) -> v::Result<Option<Value>> {
        let p = n(a, "player", 0);
        let player = (p >= 0).then(|| self.nodes[p as usize].clone());
        let id = n(a, "id", 0);
        let value = n(a, "value", 0);
        let save = a["save"] == true;
        let alias = a["alias"].as_str().unwrap_or("a");
        let definition = self.aliases.get(alias).cloned().flatten();
        match text(a, "op").as_str() {
            "get" => return v::get_varp(self, player.as_ref(), id).map(|v| Some(json!(v))),
            "set" => v::set_varp(self, player.as_ref(), id, value, save)?,
            "saveFlag" => v::save_varp(self, player.as_ref(), id)?,
            "unsave" => v::unsave_varp(self, player.as_ref(), id)?,
            "varc" => v::set_varc(self, player.as_ref(), id, value)?,
            "reinit" => v::reinit(self, player.as_ref())?,
            "getBit" => return v::get_varbit(self, player.as_ref(), id).map(|v| Some(json!(v))),
            "setBit" => v::set_varbit(self, player.as_ref(), id, value, save)?,
            "getDef" => {
                return v::get_varbit_definition(self, player.as_ref(), definition.as_ref())
                    .map(|v| Some(json!(v)));
            }
            "setDef" => {
                v::set_varbit_definition(self, player.as_ref(), definition.as_ref(), value, save)?
            }
            "defValue" => {
                return v::definition_value(self, player.as_ref(), definition.as_ref())
                    .map(|v| Some(json!(v)));
            }
            "lookup" => {
                let registry = self.registry.clone();
                let d = match n(a, "kind", 0) {
                    1 => registry.for_object_id(id, self),
                    2 => registry.for_npc_id(id, self),
                    3 => registry.for_item_id(id, self),
                    _ => registry.for_id(id, self),
                }
                .map_err(Error::Definition)?;
                self.aliases.insert(alias.into(), Some(d));
            }
            "create" => self
                .registry
                .create(n(a, "varp", 0), id, n(a, "start", 0), n(a, "end", 0)),
            "mapNull" => {
                self.registry.mapping().write().insert(id, None);
            }
            "mapAlias" => {
                self.registry.mapping().write().insert(id, definition);
            }
            "mask" => {
                return definition
                    .ok_or(Error::Null)?
                    .read()
                    .mask()
                    .map(|m| Some(json!(m)))
                    .map_err(Error::Definition);
            }
            "parse" => {
                let data = parsed(&a["data"]);
                self.manager.clone().parse(Some(&data), self)?;
            }
            "noopSave" => {
                let mut data = parsed(&a["data"]);
                self.manager.save(Some(&mut data))?;
                return Ok(Some(a["data"].clone()));
            }
            "constructorNull" => {
                Manager::new(None)?;
            }
            "clear" => self
                .values(player.as_ref().ok_or(Error::Null)?)?
                .ok_or(Error::Null)?
                .clear(),
            "capture" => {
                let p = player.as_ref().ok_or(Error::Null)?;
                let values = self.values(p)?;
                let saved = self.saved(p)?;
                self.held_values.push(values);
                self.held_saved.push(saved);
            }
            "rawValue" => {
                self.values(player.as_ref().ok_or(Error::Null)?)?
                    .ok_or(Error::Null)?
                    .put(id, (!a["value"].is_null()).then_some(value));
            }
            "rawSaved" => {
                self.saved(player.as_ref().ok_or(Error::Null)?)?
                    .ok_or(Error::Null)?
                    .put(id, a["value"].as_bool());
            }
            "valuesNull" => self.stores[p as usize].0.write().values = None,
            "savedNull" => self.stores[p as usize].0.write().saved = None,
            "replaceValues" => self.stores[p as usize].0.write().values = Some(Values::default()),
            "replaceSaved" => self.stores[p as usize].0.write().saved = Some(Saved::default()),
            "load" => {
                self.loading = a.clone();
                self.loader
                    .clone()
                    .load(&self.registry.clone(), Some(""), self)
                    .map_err(Error::Definition)?;
            }
            "reader" => self.readers.push(self.loader.reader()),
            op => panic!("unexpected operation {op}"),
        }
        Ok(None)
    }
}
impl Context for Host {
    type Dispatch = usize;
    fn values(&mut self, p: &NodeHandle) -> v::Result<Option<Values>> {
        let id = self.nodes.iter().position(|n| n.node == p.node).unwrap();
        Ok(self.stores[id].0.read().values.clone())
    }
    fn saved(&mut self, p: &NodeHandle) -> v::Result<Option<Saved>> {
        let id = self.nodes.iter().position(|n| n.node == p.node).unwrap();
        Ok(self.stores[id].0.read().saved.clone())
    }
    fn dispatch(&mut self, p: &NodeHandle) -> v::Result<usize> {
        let id = self.nodes.iter().position(|n| n.node == p.node).unwrap();
        self.hit("dispatch", vec![json!(id)])?;
        if self.nil("dispatch") {
            Err(Error::Null)
        } else {
            Ok(id)
        }
    }
    fn send_varp(&mut self, p: usize, id: i32, value: i32) -> v::Result<()> {
        self.hit("packetVarp", vec![json!(p), json!(id), json!(value)])?;
        self.hit("session", vec![json!(p)])?;
        if self.nil("session") {
            return Err(Error::Null);
        }
        self.hit("pair", vec![json!(p)])?;
        if self.nil("pair") {
            return Err(Error::Null);
        }
        self.hit("cipher", vec![json!(p)])?;
        self.hit("artificial", vec![json!(p)])?;
        if self.row["artificial"] == true {
            return Ok(());
        }
        self.hit("details", vec![json!(p)])?;
        if self.nil("details") {
            return Err(Error::Null);
        }
        self.hit("detailSession", vec![json!(p)])?;
        if self.nil("detailSession") {
            return Err(Error::Null);
        }
        self.hit("write", vec![json!(p)])?;
        let frame = updates::varp(id, value);
        self.frames.push(json!([
            frame.opcode,
            frame
                .payload
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
        ]));
        Ok(())
    }
    fn send_varc(&mut self, p: usize, id: i16, value: i32) -> v::Result<()> {
        self.hit("packetVarc", vec![json!(p), json!(id), json!(value)])?;
        self.hit("interfaces", vec![json!(p)])?;
        if self.nil("interfaces") {
            return Err(Error::Null);
        }
        self.hit("counter", vec![json!(p), json!(1)])?;
        let count = self.counters[p];
        self.counters[p] = count.wrapping_add(1);
        self.hit("session", vec![json!(p)])?;
        if self.nil("session") {
            return Err(Error::Null);
        }
        self.hit("write", vec![json!(p)])?;
        let frame = updates::varc(i32::from(id), value, count);
        self.frames.push(json!([
            frame.opcode,
            frame
                .payload
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
        ]));
        Ok(())
    }
    fn definition(&mut self, id: i32) -> v::Result<Option<Handle>> {
        self.registry
            .clone()
            .for_id(id, self)
            .map(Some)
            .map_err(Error::Definition)
    }
    fn missing_definition(&mut self, id: i32) -> v::Result<()> {
        self.hit(
            "log",
            vec![json!(format!(
                "Trying to setVarbit {id}, which doesn't seem to exist."
            ))],
        )
    }
}
impl varbit::Assets for Host {
    fn file(&mut self, _: &Registry, group: u32, file: u32) -> varbit::Result<Option<Vec<u8>>> {
        self.hit("file", vec![json!(group), json!(file)])
            .map_err(|e| varbit::Error::Binding(format!("{e:?}")))?;
        Ok(
            self.row["files"][(((group << 10) | file) as i32).to_string()]
                .as_str()
                .map(|s| {
                    (0..s.len())
                        .step_by(2)
                        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
                        .collect()
                }),
        )
    }
}
impl varbit::Files for Host {
    fn open(&mut self, _: &str) -> varbit::Result<Reader> {
        if self.loading["missing"] == true {
            return Err(varbit::Error::Binding("io".into()));
        }
        Ok(Reader(rs09_server::definitions::item::Shared::new(text(
            &self.loading,
            "text",
        ))))
    }
    fn parse(&mut self, reader: Option<Reader>) -> varbit::Result<Json> {
        java_json::parse(&reader.ok_or(varbit::Error::Null)?.0.read()).map_err(varbit::Error::Json)
    }
    fn log(&mut self, count: i32) -> varbit::Result<()> {
        self.hit(
            "log",
            vec![json!(format!("Parsed {count} custom varbit definitions."))],
        )
        .map_err(|e| varbit::Error::Binding(format!("{e:?}")))
    }
}
fn error(error: Error) -> &'static str {
    match error {
        Error::Null => "Null",
        Error::Type => "Type",
        Error::Number => "Number",
        Error::ConcurrentModification => "Concurrent",
        Error::Binding(_) => "Binding",
        Error::Definition(e) => match e {
            varbit::Error::Null => "Null",
            varbit::Error::Type => "Type",
            varbit::Error::Number => "Number",
            varbit::Error::Underflow => "Underflow",
            varbit::Error::Json(_) => "Json",
            varbit::Error::Limit => "Limit",
            varbit::Error::Binding(s) => {
                if s == "io" {
                    "Io"
                } else {
                    "Binding"
                }
            }
        },
    }
}
pub fn evaluate(row: &Value) -> Value {
    let mut state = State::default();
    let mut nodes = vec![];
    for id in 0..2 {
        let node = state.add(id, Location::new(0, 0, 0), [0; 4]);
        nodes.push(state.retain_node(node).unwrap());
    }
    let stores: Vec<Store> = (0..2).map(|_| Store::default()).collect();
    for (p, store) in stores.iter().enumerate() {
        for (k, v) in row["values"][p].as_object().unwrap() {
            store
                .0
                .read()
                .values
                .as_ref()
                .unwrap()
                .put(k.parse().unwrap(), v.as_i64().map(|v| v as i32));
        }
        for (k, v) in row["saved"][p].as_object().unwrap() {
            store
                .0
                .read()
                .saved
                .as_ref()
                .unwrap()
                .put(k.parse().unwrap(), v.as_bool());
        }
    }
    let mut host = Host {
        row: row.clone(),
        manager: Manager::new(Some(nodes[n(row, "managerOwner", 0) as usize].clone())).unwrap(),
        nodes,
        stores,
        registry: Registry::default(),
        loader: Default::default(),
        aliases: BTreeMap::new(),
        held_values: vec![],
        held_saved: vec![],
        readers: vec![],
        counts: HashMap::new(),
        counters: [n(row, "counter", 0), 0],
        log: vec![],
        frames: vec![],
        loading: Value::Null,
    };
    let mut out = vec![];
    for action in row["actions"].as_array().unwrap() {
        let (result, error) = match host.action(action) {
            Ok(v) => (v.unwrap_or(json!("__NULL__")), json!("__NULL__")),
            Err(e) => (json!("__NULL__"), json!(error(e))),
        };
        out.push(json!({"result":result,"error":error,"state":host.state(),"log":host.log,"frames":host.frames}));
    }
    json!(out)
}
