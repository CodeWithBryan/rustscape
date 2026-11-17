//! Mutable recording services for the complete original content/registry oracle.
use rs09_server::{
    definitions::item::Shared,
    world::{
        bank_interface::{self as bank, Program},
        component_objects::{NativeObject, Plugin},
        doors::Location,
        interface_listeners::{self as listeners, Button, Callback, Registry},
        interface_packets::{self as packets, Action, LoginPulse},
        interfaces::{self as ui, Error, Handle, Result},
        player_walk::{NodeHandle, State},
    },
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
struct Observed(Shared<(i32, i32, bool, bool)>);
impl NativeObject for Observed {}
struct ObservedPlugin(Shared<(i32, i32, bool)>);
impl NativeObject for ObservedPlugin {}
struct PacketPlayer {
    dialogue: bool,
    locked: bool,
    zone: bool,
    resizable: bool,
    close_allowed: bool,
    visible: BTreeMap<i32, Option<usize>>,
}
struct Player {
    bank: Option<usize>,
    inventory: Option<usize>,
    attributes: BTreeMap<String, Value>,
}
struct Container {
    slots: BTreeMap<i32, Option<usize>>,
    tab: i32,
    last: i32,
    insert: bool,
    note: bool,
}
struct Definition {
    id: i32,
    name: Option<String>,
    examine: Option<String>,
}
struct Model {
    row: Value,
    registry: Registry,
    programs: [Program; 2],
    nodes: Vec<NodeHandle>,
    components: Vec<Handle>,
    players: Vec<Player>,
    containers: Vec<Container>,
    items: Vec<(Option<usize>, i32)>,
    definitions: Vec<Definition>,
    events: Vec<Value>,
    calls: BTreeMap<String, i32>,
    hooks: BTreeSet<usize>,
    aliases: BTreeMap<String, Option<Callback>>,
    observed: Vec<Callback>,
    refs: Vec<Callback>,
    size: i32,
    packet_players: Vec<PacketPlayer>,
    plugins: Vec<Plugin>,
    plugin_aliases: BTreeMap<String, Option<Plugin>>,
    pulses: Vec<LoginPulse>,
    packet_debug: bool,
}
fn n(v: &Value) -> i32 {
    v.as_i64()
        .map_or_else(|| v.as_f64().unwrap() as i32, |n| n as i32)
}
fn number(a: &Value, k: &str, d: i32) -> i32 {
    a.get(k).filter(|v| !v.is_null()).map_or(d, n)
}
fn reference(refs: &mut Vec<Callback>, callback: Option<&Callback>) -> Option<usize> {
    callback.map(|c| {
        if let Some(i) = refs.iter().position(|v| v.same(c)) {
            i
        } else {
            refs.push(c.clone());
            refs.len() - 1
        }
    })
}
impl Model {
    fn player(&self, p: &NodeHandle) -> usize {
        self.nodes.iter().position(|n| n == p).unwrap()
    }
    fn emit(&mut self, phase: &str, args: Vec<Value>) -> Result<()> {
        let mut row = vec![json!(phase)];
        row.extend(args);
        self.events.push(json!(row));
        let count = self.calls.entry(phase.into()).or_default();
        *count = count.wrapping_add(1);
        let at = *count;
        let hooks = self.row["hooks"].as_array().cloned().unwrap_or_default();
        for (i, h) in hooks.iter().enumerate() {
            if h["event"] == phase && number(h, "at", 1) == at && self.hooks.insert(i) {
                for action in h["actions"].as_array().unwrap() {
                    self.act(action)?;
                }
            }
        }
        if self.row["failures"][phase].as_i64() == Some(i64::from(at)) {
            return Err(Error::Binding("injected service failure".into()));
        }
        Ok(())
    }
    fn act(&mut self, a: &Value) -> Result<Value> {
        match a["op"].as_str().unwrap() {
            "ifAction" => {
                let player = self.nodes[number(a, "player", 0) as usize].clone();
                self.packet_debug = true;
                let result = packets::if_action(
                    self,
                    &player,
                    Action {
                        iface: number(a, "iface", 762),
                        child: number(a, "child", 73),
                        slot: number(a, "slot", 0),
                        item: number(a, "item", 0),
                        opcode: number(a, "opcode", 155),
                        opt_index: number(a, "opt", 1),
                    },
                );
                self.packet_debug = false;
                result?;
            }
            "closePacket" => {
                let player = self.nodes[number(a, "player", 0) as usize].clone();
                packets::close_iface(self, &player)?;
            }
            "pulse" => {
                let pulse = self
                    .pulses
                    .get(number(a, "index", 0) as usize)
                    .cloned()
                    .ok_or(Error::Index)?;
                return Ok(json!(pulse.pulse(self)?));
            }
            "packetState" => {
                let p = &mut self.packet_players[number(a, "player", 0) as usize];
                for key in ["dialogue", "locked", "zone", "resizable", "closeAllowed"] {
                    if let Some(v) = a[key].as_bool() {
                        match key {
                            "dialogue" => p.dialogue = v,
                            "locked" => p.locked = v,
                            "zone" => p.zone = v,
                            "resizable" => p.resizable = v,
                            _ => p.close_allowed = v,
                        }
                    }
                }
                if let Some(visible) = a["visible"].as_object() {
                    p.visible = visible
                        .iter()
                        .map(|(k, v)| (k.parse().unwrap(), v.as_u64().map(|n| n as usize)))
                        .collect();
                }
            }
            "packetComponent" => {
                let c = &self.components[n(&a["component"]) as usize];
                if let Some(v) = a["hidden"].as_bool() {
                    c.write().hidden = v;
                }
                if let Some(v) = a.get("plugin") {
                    c.write().plugin = v
                        .as_str()
                        .and_then(|s| self.plugin_aliases.get(s))
                        .cloned()
                        .flatten();
                }
            }
            "plugin" => {
                let plugin = Plugin::new(ObservedPlugin(Shared::new((
                    number(a, "label", 7),
                    0,
                    true,
                ))));
                self.plugins.push(plugin.clone());
                self.plugin_aliases
                    .insert(a["name"].as_str().unwrap().into(), Some(plugin));
            }
            "run" => {
                let p = if a.get("player").is_some_and(Value::is_null) {
                    None
                } else {
                    Some(self.nodes[number(a, "player", 0) as usize].clone())
                };
                let c = if a.get("component").is_some_and(Value::is_null) {
                    None
                } else {
                    Some(self.components[number(a, "component", 0) as usize].clone())
                };
                let registry = self.registry.clone();
                let value = match a["kind"].as_str().unwrap_or("button") {
                    "open" => registry.run_open(self, p.as_ref(), c.as_ref())?,
                    "close" => registry.run_close(self, p.as_ref(), c.as_ref())?,
                    _ => registry.run(
                        self,
                        p.as_ref(),
                        c.as_ref(),
                        Button {
                            opcode: number(a, "opcode", 155),
                            button: number(a, "button", 73),
                            slot: number(a, "slot", 0),
                            item: number(a, "item", 0),
                        },
                    )?,
                };
                return Ok(json!(value));
            }
            "define" => self.programs[number(a, "program", 0) as usize].define(&self.registry)?,
            "attr" => {
                self.players[number(a, "player", 0) as usize]
                    .attributes
                    .insert(a["key"].as_str().unwrap().into(), a["value"].clone());
            }
            "set" => {
                if let Some(i) = a["player"].as_u64() {
                    let p = &mut self.players[i as usize];
                    if let Some(v) = a.get("bank") {
                        p.bank = v.as_u64().map(|n| n as usize);
                    }
                    if let Some(v) = a.get("inventory") {
                        p.inventory = v.as_u64().map(|n| n as usize);
                    }
                } else if let Some(i) = a["container"].as_u64() {
                    let c = &mut self.containers[i as usize];
                    if let Some(v) = a.get("last") {
                        c.last = n(v);
                    }
                    if let Some(v) = a.get("tab") {
                        c.tab = n(v);
                    }
                    if let Some(v) = a["insert"].as_bool() {
                        c.insert = v;
                    }
                    if let Some(v) = a["note"].as_bool() {
                        c.note = v;
                    }
                    if let Some(slots) = a["slots"].as_object() {
                        c.slots = slots
                            .iter()
                            .map(|(k, v)| (k.parse().unwrap(), v.as_u64().map(|n| n as usize)))
                            .collect();
                    }
                } else if let Some(i) = a["item"].as_u64() {
                    let item = &mut self.items[i as usize];
                    if let Some(v) = a.get("amount") {
                        item.1 = n(v);
                    }
                    if let Some(v) = a.get("definition") {
                        item.0 = v.as_u64().map(|n| n as usize);
                    }
                } else if let Some(i) = a["definition"].as_u64() {
                    let d = &mut self.definitions[i as usize];
                    if let Some(v) = a.get("id") {
                        d.id = n(v);
                    }
                    if let Some(v) = a.get("name") {
                        d.name = v.as_str().map(str::to_owned);
                    }
                    if let Some(v) = a.get("examine") {
                        d.examine = v.as_str().map(str::to_owned);
                    }
                } else if let Some(i) = a["component"].as_u64() {
                    self.components[i as usize].write().id = n(&a["id"]);
                }
                if let Some(v) = a.get("bankSize") {
                    self.size = n(v);
                }
            }
            "callback" => {
                let observed = Observed(Shared::new((
                    number(a, "label", 7),
                    0,
                    a["result"] != false,
                    a["kind"] != "open",
                )));
                let c = if a["kind"] == "open" {
                    Callback::open(observed)
                } else {
                    Callback::button(observed)
                };
                self.observed.push(c.clone());
                self.aliases
                    .insert(a["name"].as_str().unwrap().into(), Some(c));
            }
            "alias" => {
                let value = self
                    .aliases
                    .get(a["source"].as_str().unwrap())
                    .cloned()
                    .flatten();
                self.aliases
                    .insert(a["name"].as_str().unwrap().into(), value);
            }
            "result" => {
                let c = self.aliases[a["name"].as_str().unwrap()]
                    .clone()
                    .ok_or(Error::Null)?;
                c.value::<Observed>().ok_or(Error::Type)?.0.write().2 =
                    a["value"].as_bool().unwrap();
            }
            "same" => {
                let one = self
                    .aliases
                    .get(a["left"].as_str().unwrap())
                    .and_then(Option::as_ref);
                let two = self
                    .aliases
                    .get(a["right"].as_str().unwrap())
                    .and_then(Option::as_ref);
                return Ok(json!(match (one, two) {
                    (Some(a), Some(b)) => a.same(b),
                    (None, None) => true,
                    _ => false,
                }));
            }
            "map" => {
                let callback = a["value"]
                    .as_str()
                    .and_then(|s| self.aliases.get(s))
                    .cloned()
                    .flatten();
                let target = if a["map"] == "open" {
                    &self.registry.open_listeners
                } else {
                    &self.registry.button_listeners
                };
                target
                    .write()
                    .insert(a["key"].as_str().unwrap().into(), callback);
            }
            "get" => {
                let id = n(&a["id"]);
                let callback = match a["kind"].as_str().unwrap_or("button") {
                    "open" => self.registry.get_open(id)?,
                    "close" => self.registry.get_close(id)?,
                    _ => self.registry.get(id, a.get("button").map(n))?,
                };
                let r = reference(&mut self.refs, callback.as_ref());
                self.aliases
                    .insert(a["name"].as_str().unwrap().into(), callback);
                return Ok(json!(r));
            }
            _ => return Err(Error::Binding("unknown test action".into())),
        }
        Ok(Value::Null)
    }
    fn amount_value(&self, container: usize, id: i32) -> Result<i32> {
        let mut value = 0i32;
        for item in self.containers[container].slots.values().flatten() {
            let item = self.items[*item];
            let d = self
                .definitions
                .get(item.0.ok_or(Error::Null)?)
                .ok_or(Error::Index)?;
            if d.id == id {
                value = value.wrapping_add(item.1);
            }
        }
        Ok(value)
    }
    fn snapshot(&mut self, result: Value, error: Option<&str>) -> Value {
        let mapping =
            |refs: &mut Vec<Callback>, map: std::collections::HashMap<String, Option<Callback>>| {
                map.into_iter()
                    .collect::<BTreeMap<_, _>>()
                    .into_iter()
                    .map(|(k, c)| (k, reference(refs, c.as_ref())))
                    .collect::<BTreeMap<_, _>>()
            };
        let open = mapping(&mut self.refs, self.registry.open_listeners.read().clone());
        let buttons = mapping(
            &mut self.refs,
            self.registry.button_listeners.read().clone(),
        );
        let aliases = self
            .aliases
            .iter()
            .map(|(k, c)| (k.clone(), reference(&mut self.refs, c.as_ref())))
            .collect::<BTreeMap<_, _>>();
        let observed = self
            .observed
            .iter()
            .map(|c| {
                let r = reference(&mut self.refs, Some(c));
                let f = *c.value::<Observed>().unwrap().0.read();
                json!([r, f.0, f.1, f.2])
            })
            .collect::<Vec<_>>();
        let players = self
            .players
            .iter()
            .map(|p| json!({"bank":p.bank,"inventory":p.inventory,"attributes":p.attributes}))
            .collect::<Vec<_>>();
        let containers = self
            .containers
            .iter()
            .map(|c| {
                let slots = c
                    .slots
                    .iter()
                    .map(|(k, v)| (k.to_string(), *v))
                    .collect::<BTreeMap<_, _>>();
                json!({"slots":slots,"tab":c.tab,"last":c.last,"insert":c.insert,"note":c.note})
            })
            .collect::<Vec<_>>();
        let definitions = self
            .definitions
            .iter()
            .map(|d| json!([d.id, d.name, d.examine]))
            .collect::<Vec<_>>();
        let mut out = json!({"result":result,"error":error,"events":self.events,"open":open,"buttons":buttons,"aliases":aliases,"observed":observed,"players":players,"containers":containers,"definitions":definitions,"items":self.items,"components":self.components.iter().map(|c|c.read().id).collect::<Vec<_>>(),"bankSize":self.size});
        if self.row["packet"] == true {
            let players=self.packet_players.iter().map(|p|{let visible=p.visible.iter().map(|(k,v)|(k.to_string(),*v)).collect::<BTreeMap<_,_>>();json!({"dialogue":p.dialogue,"locked":p.locked,"zone":p.zone,"resizable":p.resizable,"closeAllowed":p.close_allowed,"visible":visible})}).collect::<Vec<_>>();
            let components = self
                .components
                .iter()
                .map(|c| {
                    let c = c.read();
                    json!([
                        c.hidden,
                        c.plugin.as_ref().map(|p| self
                            .plugins
                            .iter()
                            .position(|v| v.same(p))
                            .unwrap())
                    ])
                })
                .collect::<Vec<_>>();
            let plugins = self
                .plugins
                .iter()
                .map(|p| {
                    let f = *p.value::<ObservedPlugin>().unwrap().0.read();
                    json!([f.0, f.1, f.2])
                })
                .collect::<Vec<_>>();
            let aliases = self
                .plugin_aliases
                .iter()
                .map(|(k, p)| {
                    (
                        k.clone(),
                        p.as_ref()
                            .map(|p| self.plugins.iter().position(|v| v.same(p)).unwrap()),
                    )
                })
                .collect::<BTreeMap<_, _>>();
            let pulses = self
                .pulses
                .iter()
                .map(|pulse| vec![self.player(&pulse.player())])
                .collect::<Vec<_>>();
            out["packet"] = json!({"players":players,"components":components,"plugins":plugins,"aliases":aliases,"pulses":pulses});
        }
        out
    }
    fn invoke(
        &mut self,
        callback: Callback,
        p: &NodeHandle,
        c: &Handle,
        button: Option<Button>,
    ) -> Result<bool> {
        if let Some(observed) = callback.value::<Observed>() {
            let (label, _, _, is_button) = *observed.0.read();
            if is_button != button.is_some() {
                return Err(Error::Type);
            }
            let r = reference(&mut self.refs, Some(&callback));
            let component = self
                .components
                .iter()
                .position(|value| value.same(c))
                .unwrap();
            let mut args = vec![
                json!(r),
                json!(label),
                json!(self.player(p)),
                json!(component),
            ];
            if let Some(b) = button {
                args.extend([
                    json!(b.opcode),
                    json!(b.button),
                    json!(b.slot),
                    json!(b.item),
                ]);
            }
            self.emit("callback", args)?;
            let count = observed.0.read().1;
            observed.0.write().1 = count.wrapping_add(1);
            return Ok(observed.0.read().2);
        }
        if (Program::kind(&callback) == Some(bank::Kind::Open)) == button.is_some() {
            return Err(Error::Type);
        }
        Program::invoke(
            callback,
            self,
            p,
            c,
            button.unwrap_or(Button {
                opcode: 0,
                button: 0,
                slot: 0,
                item: 0,
            }),
        )
    }
}
impl listeners::Context for Model {
    fn invoke_open(&mut self, c: Callback, p: &NodeHandle, w: &Handle) -> Result<bool> {
        self.invoke(c, p, w, None)
    }
    fn invoke_button(
        &mut self,
        c: Callback,
        p: &NodeHandle,
        w: &Handle,
        b: Button,
    ) -> Result<bool> {
        self.invoke(c, p, w, Some(b))
    }
}
impl bank::Context for Model {
    type Container = Option<usize>;
    type Item = usize;
    type Definition = Option<usize>;
    type Dispatch = usize;
    type Dialogue = i32;
    fn bank_size(&mut self) -> Result<i32> {
        Ok(self.size)
    }
    fn bank(&mut self, p: &NodeHandle) -> Result<Option<usize>> {
        let p = self.player(p);
        let c = self.players[p].bank;
        self.emit("bank", vec![json!(p), json!(c)])?;
        Ok(c)
    }
    fn inventory(&mut self, p: &NodeHandle) -> Result<Option<usize>> {
        let p = self.player(p);
        let c = self.players[p].inventory;
        self.emit("inventory", vec![json!(p), json!(c)])?;
        Ok(c)
    }
    fn item(&mut self, c: Option<usize>, slot: i32) -> Result<Option<usize>> {
        let c = c.ok_or(Error::Null)?;
        let item = self.containers[c].slots.get(&slot).copied().flatten();
        self.emit("item", vec![json!(c), json!(slot), json!(item)])?;
        Ok(item)
    }
    fn amount(&mut self, c: Option<usize>, item: usize) -> Result<i32> {
        let c = c.ok_or(Error::Null)?;
        let id = self.definitions[self.items[item].0.ok_or(Error::Null)?].id;
        let n = self.amount_value(c, id)?;
        self.emit("amount", vec![json!(c), json!(item)])?;
        Ok(n)
    }
    fn amount_id(&mut self, c: Option<usize>, id: i32) -> Result<i32> {
        let c = c.ok_or(Error::Null)?;
        let n = self.amount_value(c, id)?;
        self.emit("amountId", vec![json!(c), json!(id)])?;
        Ok(n)
    }
    fn last_amount_x(&mut self, c: Option<usize>) -> Result<i32> {
        let c = c.ok_or(Error::Null)?;
        let n = self.containers[c].last;
        self.emit("last", vec![json!(c)])?;
        Ok(n)
    }
    fn take(&mut self, c: Option<usize>, slot: i32, n: i32) -> Result<()> {
        self.emit(
            "take",
            vec![json!(c.ok_or(Error::Null)?), json!(slot), json!(n)],
        )
    }
    fn deposit(&mut self, c: Option<usize>, slot: i32, n: i32) -> Result<()> {
        self.emit(
            "deposit",
            vec![json!(c.ok_or(Error::Null)?), json!(slot), json!(n)],
        )
    }
    fn transfer_x(&mut self, p: &NodeHandle, slot: i32, withdraw: bool) -> Result<()> {
        self.emit(
            "transfer",
            vec![json!(self.player(p)), json!(slot), json!(withdraw)],
        )
    }
    fn tab_index(&mut self, c: Option<usize>) -> Result<i32> {
        let c = c.ok_or(Error::Null)?;
        let n = self.containers[c].tab;
        self.emit("tab", vec![json!(c)])?;
        Ok(n)
    }
    fn set_tab_index(&mut self, c: Option<usize>, n: i32) -> Result<()> {
        let c = c.ok_or(Error::Null)?;
        self.containers[c].tab = n;
        self.emit("setTab", vec![json!(c), json!(n)])
    }
    fn collapse_tab(&mut self, c: Option<usize>, n: i32) -> Result<()> {
        self.emit("collapse", vec![json!(c.ok_or(Error::Null)?), json!(n)])
    }
    fn insert(&mut self, c: Option<usize>) -> Result<bool> {
        let c = c.ok_or(Error::Null)?;
        let n = self.containers[c].insert;
        self.emit("insert", vec![json!(c)])?;
        Ok(n)
    }
    fn set_insert(&mut self, c: Option<usize>, n: bool) -> Result<()> {
        let c = c.ok_or(Error::Null)?;
        self.containers[c].insert = n;
        self.emit("setInsert", vec![json!(c), json!(n)])
    }
    fn note(&mut self, c: Option<usize>) -> Result<bool> {
        let c = c.ok_or(Error::Null)?;
        let n = self.containers[c].note;
        self.emit("note", vec![json!(c)])?;
        Ok(n)
    }
    fn set_note(&mut self, c: Option<usize>, n: bool) -> Result<()> {
        let c = c.ok_or(Error::Null)?;
        self.containers[c].note = n;
        self.emit("setNote", vec![json!(c), json!(n)])
    }
    fn last_tab(&mut self, p: &NodeHandle) -> Result<i32> {
        let p = self.player(p);
        let n = self.players[p]
            .attributes
            .get("bank:lasttab")
            .filter(|v| !v.is_null())
            .cloned()
            .unwrap_or(json!(0));
        self.emit("lastTab", vec![json!(p), n.clone()])?;
        if n.is_number() {
            Ok(n.as_i64()
                .map_or_else(|| n.as_f64().unwrap() as i32, |v| v as i32))
        } else {
            Err(Error::Type)
        }
    }
    fn varc(&mut self, p: &NodeHandle, id: i32, n: i32) -> Result<()> {
        self.emit("varc", vec![json!(self.player(p)), json!(id), json!(n)])
    }
    fn search(&mut self, p: &NodeHandle) -> Result<()> {
        let p = self.player(p);
        self.players[p]
            .attributes
            .insert("search".into(), json!(true));
        self.emit("attribute", vec![json!(p), json!("search"), json!(true)])
    }
    fn definition(&mut self, item: usize) -> Result<Option<usize>> {
        let d = self.items[item].0;
        self.emit("definition", vec![json!(item), json!(d)])?;
        Ok(d)
    }
    fn examine(&mut self, d: Option<usize>) -> Result<Option<bank::Text>> {
        let d = d.ok_or(Error::Null)?;
        let n = self.definitions[d].examine.clone();
        self.emit("examine", vec![json!(d)])?;
        Ok(n.map(|s| s.encode_utf16().collect()))
    }
    fn id(&mut self, d: Option<usize>) -> Result<i32> {
        let d = d.ok_or(Error::Null)?;
        let n = self.definitions[d].id;
        self.emit("id", vec![json!(d)])?;
        Ok(n)
    }
    fn name(&mut self, d: Option<usize>) -> Result<Option<bank::Text>> {
        let d = d.ok_or(Error::Null)?;
        let n = self.definitions[d].name.clone();
        self.emit("name", vec![json!(d)])?;
        Ok(n.map(|s| s.encode_utf16().collect()))
    }
    fn message(&mut self, p: &NodeHandle, n: Option<bank::Text>) -> Result<()> {
        self.emit(
            "message",
            vec![
                json!(self.player(p)),
                json!(String::from_utf16(&n.ok_or(Error::Null)?).unwrap()),
            ],
        )
    }
    fn debug(&mut self, p: &NodeHandle, n: bank::Text) -> Result<()> {
        self.emit(
            if self.packet_debug {
                "packetDebug"
            } else {
                "debug"
            },
            vec![
                json!(self.player(p)),
                json!(String::from_utf16(&n).unwrap()),
            ],
        )
    }
    fn dispatch(&mut self, p: &NodeHandle) -> Result<usize> {
        let p = self.player(p);
        self.emit("dispatch", vec![json!(p)])?;
        Ok(p)
    }
    fn settings(&mut self, p: usize, fields: [i32; 5]) -> Result<()> {
        let mut a = vec![json!(p)];
        a.extend(fields.into_iter().map(|n| json!(n)));
        self.emit("settings", a)
    }
    fn create_dialogue(&mut self, kind: bank::Dialogue) -> Result<i32> {
        let kind = match kind {
            bank::Dialogue::Help => 0,
            bank::Dialogue::Deposit => 1,
        };
        self.emit("dialogueCreate", vec![json!(kind)])?;
        Ok(kind)
    }
    fn open_dialogue(&mut self, p: &NodeHandle, d: i32) -> Result<()> {
        self.emit("dialogue", vec![json!(self.player(p)), json!(d)])
    }
    fn open_bank(&mut self, p: &NodeHandle) -> Result<()> {
        self.emit("openBank", vec![json!(self.player(p))])
    }
}
impl packets::Context for Model {
    type Dialogue = usize;
    type Locks = usize;
    type Zone = usize;
    type Manager = usize;
    type Dispatch = usize;
    fn debug(&mut self, p: &NodeHandle, text: String) -> Result<()> {
        self.emit("packetDebug", vec![json!(self.player(p)), json!(text)])
    }
    fn dialogue(&mut self, p: &NodeHandle) -> Result<usize> {
        let p = self.player(p);
        self.emit("dialogue", vec![json!(p)])?;
        Ok(p)
    }
    fn has_dialogue(&mut self, p: usize) -> Result<bool> {
        let v = self.packet_players[p].dialogue;
        self.emit("active", vec![json!(p)])?;
        Ok(v)
    }
    fn close_dialogue(&mut self, p: usize) -> Result<()> {
        self.packet_players[p].dialogue = false;
        self.emit("dialogueClose", vec![json!(p)])
    }
    fn locks(&mut self, p: &NodeHandle) -> Result<usize> {
        let p = self.player(p);
        self.emit("locks", vec![json!(p)])?;
        Ok(p)
    }
    fn component_locked(&mut self, p: usize) -> Result<bool> {
        let v = self.packet_players[p].locked;
        self.emit("componentLocked", vec![json!(p)])?;
        Ok(v)
    }
    fn zone(&mut self, p: &NodeHandle) -> Result<usize> {
        let p = self.player(p);
        self.emit("zone", vec![json!(p)])?;
        Ok(p)
    }
    fn click_zone(&mut self, p: usize, a: Action) -> Result<bool> {
        self.emit(
            "zoneClick",
            vec![
                json!(p),
                json!(a.iface),
                json!(a.child),
                json!(a.slot),
                json!(a.item),
                json!(a.opcode),
            ],
        )?;
        Ok(self.packet_players[p].zone)
    }
    fn manager(&mut self, p: &NodeHandle) -> Result<usize> {
        let p = self.player(p);
        self.emit("manager", vec![json!(p)])?;
        Ok(p)
    }
    fn component(&mut self, p: usize, id: i32) -> Result<Option<Handle>> {
        let c = self.packet_players[p].visible.get(&id).copied().flatten();
        self.emit("lookup", vec![json!(p), json!(id), json!(c)])?;
        Ok(c.map(|i| self.components[i].clone()))
    }
    fn hidden(&mut self, c: &Handle) -> Result<bool> {
        let v = c.read().hidden;
        let i = self.components.iter().position(|v| v.same(c)).unwrap();
        self.emit("hidden", vec![json!(i)])?;
        Ok(v)
    }
    fn plugin(&mut self, c: &Handle) -> Result<Option<Plugin>> {
        let p = c.read().plugin.clone();
        let i = self.components.iter().position(|v| v.same(c)).unwrap();
        let r = p
            .as_ref()
            .map(|p| self.plugins.iter().position(|v| v.same(p)).unwrap());
        self.emit("pluginGet", vec![json!(i), json!(r)])?;
        Ok(p)
    }
    fn id(&mut self, c: &Handle) -> Result<i32> {
        Ok(c.read().id)
    }
    fn button_event(&mut self, p: &NodeHandle, id: i32, child: i32) -> Result<()> {
        self.emit(
            "buttonEvent",
            vec![json!(self.player(p)), json!(id), json!(child)],
        )
    }
    fn run_listener(&mut self, p: &NodeHandle, c: &Handle, b: Button) -> Result<bool> {
        let registry = self.registry.clone();
        registry.run(self, Some(p), Some(c), b)
    }
    fn handle_plugin(
        &mut self,
        plugin: Plugin,
        p: &NodeHandle,
        c: &Handle,
        b: Button,
    ) -> Result<bool> {
        let receiver = plugin.value::<ObservedPlugin>().ok_or(Error::Type)?;
        let label = receiver.0.read().0;
        let r = self.plugins.iter().position(|v| v.same(&plugin)).unwrap();
        let component = self.components.iter().position(|v| v.same(c)).unwrap();
        self.emit(
            "pluginHandle",
            vec![
                json!(r),
                json!(label),
                json!(self.player(p)),
                json!(component),
                json!(b.opcode),
                json!(b.button),
                json!(b.slot),
                json!(b.item),
            ],
        )?;
        let count = receiver.0.read().1;
        receiver.0.write().1 = count.wrapping_add(1);
        Ok(receiver.0.read().2)
    }
    fn close(&mut self, p: usize) -> Result<bool> {
        self.emit("managerClose", vec![json!(p)])?;
        let v = self.packet_players[p].close_allowed;
        if v {
            self.packet_players[p].visible.clear();
        }
        Ok(v)
    }
    fn logging_present(&mut self, p: &NodeHandle) -> Result<bool> {
        let p = self.player(p);
        let v = self.players[p]
            .attributes
            .get("logging_in")
            .cloned()
            .unwrap_or(Value::Null);
        self.emit("getAttr", vec![json!(p), json!("logging_in"), v.clone()])?;
        Ok(!v.is_null())
    }
    fn submit_login(&mut self, pulse: LoginPulse) -> Result<()> {
        self.emit("submit", vec![])?;
        self.pulses.push(pulse);
        Ok(())
    }
    fn map_present(&mut self, p: &NodeHandle) -> Result<bool> {
        let p = self.player(p);
        let v = self.players[p]
            .attributes
            .get("worldMap:viewing")
            .cloned()
            .unwrap_or(Value::Null);
        self.emit(
            "getAttr",
            vec![json!(p), json!("worldMap:viewing"), v.clone()],
        )?;
        Ok(!v.is_null())
    }
    fn remove_map(&mut self, p: &NodeHandle) -> Result<()> {
        let p = self.player(p);
        self.players[p].attributes.remove("worldMap:viewing");
        self.emit("removeAttr", vec![json!(p), json!("worldMap:viewing")])
    }
    fn dispatch(&mut self, p: &NodeHandle) -> Result<usize> {
        bank::Context::dispatch(self, p)
    }
    fn resizable(&mut self, p: usize) -> Result<bool> {
        let v = self.packet_players[p].resizable;
        self.emit("resizable", vec![json!(p)])?;
        Ok(v)
    }
    fn windows_pane(&mut self, p: usize, id: i32, kind: i32) -> Result<()> {
        self.emit("pane", vec![json!(p), json!(id), json!(kind)])
    }
    fn unlock(&mut self, p: &NodeHandle) -> Result<()> {
        let p = self.player(p);
        self.packet_players[p].locked = false;
        self.emit("unlock", vec![json!(p)])
    }
    fn remove_logging(&mut self, p: &NodeHandle) -> Result<()> {
        let p = self.player(p);
        self.players[p].attributes.remove("logging_in");
        self.emit("removeAttr", vec![json!(p), json!("logging_in")])
    }
    fn configure_world(&mut self, p: &NodeHandle) -> Result<()> {
        self.emit("configureWorld", vec![json!(self.player(p))])
    }
}
pub fn evaluate(row: &Value) -> Value {
    let mut state = State::default();
    let nodes = (1..=2)
        .map(|id| {
            let n = state.add(id, Location::new(3207, 3216, 0), [0; 4]);
            state.retain_node(n).unwrap()
        })
        .collect();
    let mut components = ui::Registry::default();
    let components = [762, 763, 767, 99]
        .into_iter()
        .map(|id| components.component(id))
        .collect();
    let mut containers = (0..4)
        .map(|_| Container {
            slots: BTreeMap::new(),
            tab: 10,
            last: 50,
            insert: false,
            note: false,
        })
        .collect::<Vec<_>>();
    containers[0].slots.insert(0, Some(0));
    containers[1].slots.insert(0, Some(1));
    let mut model = Model {
        row: row.clone(),
        registry: Registry::default(),
        programs: [Program::default(), Program::default()],
        nodes,
        components,
        players: (0..2)
            .map(|i| Player {
                bank: Some(i * 2),
                inventory: Some(i * 2 + 1),
                attributes: BTreeMap::new(),
            })
            .collect(),
        containers,
        items: vec![(Some(0), 100000), (Some(1), 3)],
        definitions: vec![
            Definition {
                id: 995,
                name: Some("Coins".into()),
                examine: Some("Lovely money!".into()),
            },
            Definition {
                id: 1351,
                name: Some("Bronze axe".into()),
                examine: Some("A woodcutter's axe.".into()),
            },
        ],
        events: vec![],
        calls: BTreeMap::new(),
        hooks: BTreeSet::new(),
        aliases: BTreeMap::new(),
        observed: vec![],
        refs: vec![],
        size: 496,
        packet_players: (0..2)
            .map(|_| PacketPlayer {
                dialogue: false,
                locked: false,
                zone: false,
                resizable: true,
                close_allowed: true,
                visible: [
                    (762, Some(0)),
                    (763, Some(1)),
                    (767, Some(2)),
                    (99, Some(3)),
                ]
                .into_iter()
                .collect(),
            })
            .collect(),
        plugins: vec![],
        plugin_aliases: BTreeMap::new(),
        pulses: vec![],
        packet_debug: false,
    };
    model.programs[0].define(&model.registry).unwrap();
    let mut out = vec![];
    for a in row["actions"].as_array().unwrap() {
        let (result, error) = match model.act(a) {
            Ok(v) => (v, None),
            Err(e) => (
                Value::Null,
                Some(match e {
                    Error::Null => "Null",
                    Error::Type => "Type",
                    Error::Index => "Index",
                    _ => "Binding",
                }),
            ),
        };
        out.push(model.snapshot(result, error));
    }
    json!(out)
}
