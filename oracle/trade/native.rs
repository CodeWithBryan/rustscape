//! Replays a trade oracle case on the native library. Players are canonical
//! node handles; their inventories are native containers, their interface
//! managers the native InterfaceManager, their Ironman managers the native
//! manager, and the Offer-X/Remove-X prompts go through the native input
//! prompt (`input_prompt::send` and `receive`). Everything else the library
//! asks of its host is answered from the case, as Support.java answers the
//! original classes, and recorded in the same form.
use rs09_protocol::game::Frame;
use rs09_server::{
    definitions::{
        item::{ItemDefinition, Registry as ItemRegistry, Shared},
        property::Property,
    },
    inventory::{
        container::{Container, ContainerType, Event, Factory, SortType},
        container_listener::{Handle as Listener, NativeListener},
        item::{Handle as Item, Item as ItemValue},
    },
    player::ironman,
    world::{
        input_prompt::{self, InputType, Response},
        interfaces::{self, CloseEvent, Handle as Component, Manager, Open, Plugin},
        player_walk::{NodeHandle, State},
        trade::{
            self, AmountScript, Bot, Components, Error, Host, Location, Module, RequestType,
            TradePlugin, module, plugin, request,
            ui::{Items, Ui},
        },
    },
};
use serde_json::{Map, Value, json};
use std::{collections::HashMap, sync::Arc};

const CONFIG: &str = include_str!("../../fixtures/interfaces/interface_configs.json");

#[derive(Clone)]
enum Attr {
    Int(i32),
    Bool(bool),
    Request(RequestType),
    Script(AmountScript<NodeHandle>),
    Type(InputType),
}

struct Player {
    node: NodeHandle,
    name: String,
    location: Location,
    active: bool,
    artificial: bool,
    ip: String,
    mac: String,
    host: String,
    attributes: HashMap<String, Attr>,
    module: Option<Module<NodeHandle>>,
    target: Option<NodeHandle>,
    inventory: Container,
    ironman: ironman::Manager,
    manager: Manager,
    weapon: Option<Component>,
}

/// The player's inventory listener stand-in, as Support.InventoryRecorder.
struct Recorder(usize);
impl NativeListener for Recorder {}

struct World {
    case: Value,
    players: Vec<Player>,
    registry: interfaces::Registry,
    definitions: Arc<ItemRegistry>,
    null_item: Item,
    components: Components,
    ticks: i32,
    dev: bool,
    bots: HashMap<String, Bot>,
    zone_refuse: Vec<usize>,
    examines: HashMap<i32, String>,
    modules: Vec<Module<NodeHandle>>,
    pulses: Vec<(usize, i32)>,
    events: Vec<Value>,
    calls: HashMap<String, i64>,
}

type R<T> = trade::Result<T>;

fn n(value: &Value) -> i32 {
    value.as_i64().unwrap() as i32
}

impl World {
    fn index(&self, p: &NodeHandle) -> usize {
        self.players
            .iter()
            .position(|q| &q.node == p)
            .expect("a case player")
    }
    fn player(&mut self, p: &NodeHandle) -> &mut Player {
        let i = self.index(p);
        &mut self.players[i]
    }
    fn emit(&mut self, name: &str, args: Vec<Value>) -> R<()> {
        let mut row = vec![json!(name)];
        row.extend(args);
        self.events.push(Value::Array(row));
        let at = self.calls.entry(name.into()).or_insert(0);
        *at += 1;
        if self.case["failures"][name].as_i64() == Some(*at) {
            return Err(Error::Service(format!("injected {name}")));
        }
        Ok(())
    }
    fn items(items: &[Option<Item>]) -> Value {
        Value::Array(
            items
                .iter()
                .map(|item| match item {
                    None => Value::Null,
                    Some(item) => {
                        let item = item.read();
                        json!([item.id(), item.amount()])
                    }
                })
                .collect(),
        )
    }
    fn container(container: &Container) -> Value {
        Self::items(&container.items().read())
    }
    fn attr_int(&self, i: usize, key: &str) -> Value {
        match self.players[i].attributes.get(key) {
            None => Value::Null,
            Some(Attr::Int(v)) => json!(v),
            Some(Attr::Bool(v)) => json!(v),
            _ => panic!("unexpected attribute type for {key}"),
        }
    }

    fn new(case: &Value) -> Self {
        let mut definitions = ItemRegistry::default();
        let mut examines = HashMap::new();
        for (key, spec) in case["items"].as_object().unwrap() {
            let id: i32 = key.parse().unwrap();
            let mut d = ItemDefinition {
                id,
                name: spec["name"].as_str().map(Into::into),
                stackable: spec["stackable"] == true,
                unnoted: spec["unnoted"] != false,
                note_id: spec["noteId"].as_i64().map_or(-1, |v| v as i32),
                ..ItemDefinition::default()
            };
            if let Some(options) = spec["options"].as_array() {
                d.options = Some(Shared::new(
                    options.iter().map(|o| o.as_str().map(Into::into)).collect(),
                ));
            }
            if let Some(value) = spec.get("tradeable") {
                d.handlers
                    .insert("tradeable".into(), Property::Bool(value.as_bool().unwrap()));
            }
            examines.insert(id, spec["examine"].as_str().unwrap().to_string());
            definitions.insert(id, Shared::new(d));
        }
        let definitions = Arc::new(definitions);
        let factory = Factory::new(&definitions);
        let mut registry = interfaces::Registry::default();
        registry.load(CONFIG).unwrap();
        trade::register_plugin(&mut registry);
        let components = trade::components(&mut registry);
        let mut state = State::default();
        let mut players = Vec::new();
        for (i, spec) in case["players"].as_array().unwrap().iter().enumerate() {
            let id = state.add(
                i as i32,
                rs09_server::world::doors::Location::new(3207, 3216, 0),
                [1, 2, 3, 4],
            );
            let node = state.retain_node(id).unwrap();
            let mut source = factory
                .create(28, ContainerType::Default, SortType::Id)
                .unwrap();
            let mut inventory = factory
                .create(28, ContainerType::Default, SortType::Id)
                .unwrap();
            for row in spec["inv"].as_array().unwrap() {
                let item: Item =
                    Shared::new(ItemValue::new(n(&row[1]), n(&row[2]), 1000, &definitions));
                source
                    .replace(
                        Some(&item),
                        n(&row[0]),
                        false,
                        &definitions,
                        &mut NoListeners,
                    )
                    .unwrap();
            }
            inventory.copy_from(&source, &definitions);
            inventory.register(Some(Listener::new(Recorder(i))));
            let ironman = ironman::Manager::new(Some(node.clone()));
            ironman.set_mode(Some(mode(spec["ironman"].as_str().unwrap_or("NONE"))));
            let mut attributes = HashMap::new();
            if spec["bot"] == true {
                attributes.insert("not_on_highscores".into(), Attr::Bool(true));
            }
            players.push(Player {
                manager: Manager::new(Some(node.clone())),
                node,
                name: spec["name"].as_str().unwrap().into(),
                location: Location {
                    x: n(&spec["x"]),
                    y: n(&spec["y"]),
                    z: n(&spec["z"]),
                },
                active: spec["active"] != false,
                artificial: spec["artificial"] == true,
                ip: spec["ip"].as_str().unwrap().into(),
                mac: spec["mac"].as_str().unwrap().into(),
                host: spec["host"].as_str().unwrap().into(),
                attributes,
                module: None,
                target: None,
                inventory,
                ironman,
                weapon: None,
            });
        }
        let bots = case["bots"]
            .as_object()
            .map(|bots| {
                bots.iter()
                    .map(|(name, kind)| {
                        (
                            name.clone(),
                            Bot {
                                doubling: kind == "doubling",
                            },
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        let zone_refuse = case["zoneRefuse"]
            .as_array()
            .map(|list| list.iter().map(|v| v.as_u64().unwrap() as usize).collect())
            .unwrap_or_default();
        Self {
            case: case.clone(),
            players,
            registry,
            null_item: factory.null_item.clone(),
            definitions,
            components,
            ticks: n(&case["ticks"]),
            dev: case["dev"] == true,
            bots,
            zone_refuse,
            examines,
            modules: Vec::new(),
            pulses: Vec::new(),
            events: Vec::new(),
            calls: HashMap::new(),
        }
    }

    fn state(&self) -> Value {
        let mut players = Vec::new();
        for (i, p) in self.players.iter().enumerate() {
            let f = p.manager.fields.read();
            let id = |c: &Option<Component>| c.as_ref().map(|c| c.read().id);
            let tabs: Vec<Value> = f
                .tabs
                .as_ref()
                .unwrap()
                .read()
                .iter()
                .map(|c| json!(id(c)))
                .collect();
            let mut s = Map::new();
            s.insert("inv".into(), Self::container(&p.inventory));
            s.insert("opened".into(), json!(id(&f.opened)));
            s.insert("single".into(), json!(id(&f.single_tab)));
            s.insert("chatbox".into(), json!(id(&f.chatbox)));
            s.insert("overlay".into(), json!(id(&f.overlay)));
            s.insert("tabs".into(), Value::Array(tabs));
            s.insert("count".into(), json!(f.packet_count));
            s.insert(
                "module".into(),
                json!(
                    p.module
                        .as_ref()
                        .map(|m| self.modules.iter().position(|x| x.same(m)).unwrap())
                ),
            );
            s.insert(
                "target".into(),
                json!(p.target.as_ref().map(|t| self.index(t))),
            );
            s.insert(
                "last".into(),
                json!(p.attributes.get("lastRequest").map(|a| match a {
                    Attr::Request(RequestType::Trade) => "TRADE",
                    Attr::Request(RequestType::Assist) => "ASSIST",
                    _ => panic!("lastRequest type"),
                })),
            );
            for key in ["busy", "alert", "alertSlot", "parseamount"] {
                s.insert(key.into(), self.attr_int(i, key));
            }
            s.insert(
                "runscript".into(),
                json!(p.attributes.contains_key("runscript")),
            );
            s.insert(
                "inputType".into(),
                json!(p.attributes.get("input-type").map(|a| match a {
                    Attr::Type(t) => t.to_string(),
                    _ => panic!("input-type type"),
                })),
            );
            s.insert("active".into(), json!(p.active));
            players.push(Value::Object(s));
        }
        let modules: Vec<Value> = self
            .modules
            .iter()
            .map(|m| {
                json!({
                    "owner": m.player().map(|p| self.index(&p)),
                    "target": m.target().map(|p| self.index(&p)),
                    "stage": m.stage(),
                    "accepted": m.is_accepted(),
                    "modified": m.is_modified(),
                    "retained": m.is_retained(),
                    "items": Self::container(&m.container().unwrap()),
                })
            })
            .collect();
        json!({
            "players": players,
            "modules": modules,
            "pulses": self.pulses.iter().map(|(p, slot)| json!([p, slot])).collect::<Vec<_>>(),
            "ticks": self.ticks,
        })
    }

    fn setup(&mut self) -> R<Value> {
        for i in 0..self.players.len() {
            let node = self.players[i].node.clone();
            let m = self.players[i].manager.clone();
            let pane = self.registry.component(548);
            interfaces::open_windows(
                &mut Ui::new(self, &node),
                &m,
                Some(pane),
                Open::Overlap(false),
            )?;
            interfaces::open_default_tabs(&mut Ui::new(self, &node), &m)?;
        }
        Ok(Value::Null)
    }

    fn act(&mut self, a: &Value) -> R<Value> {
        let op = a["op"].as_str().unwrap();
        let p = a
            .get("p")
            .map(|v| self.players[v.as_u64().unwrap() as usize].node.clone());
        let pi = a.get("p").map(|v| v.as_u64().unwrap() as usize);
        Ok(match op {
            "request" => {
                let t = self.players[a["t"].as_u64().unwrap() as usize].node.clone();
                let kind = if a["kind"] == "assist" {
                    RequestType::Assist
                } else {
                    RequestType::Trade
                };
                json!(request::request(self, &p.unwrap(), &t, kind)?)
            }
            "button" => {
                let p = p.unwrap();
                let m = self.player(&p).manager.clone();
                let Some(c) =
                    interfaces::get_component(&mut Ui::new(self, &p), &m, n(&a["iface"]))?
                else {
                    return Ok(json!("none"));
                };
                if c.read().hidden {
                    return Ok(json!("hidden"));
                }
                let Some(found) = self.registry.plugin(&c) else {
                    return Ok(json!("noplugin"));
                };
                assert!(
                    found.value::<TradePlugin>().is_some(),
                    "only the trade plugin is registered"
                );
                json!(plugin::handle(
                    self,
                    &p,
                    &c,
                    n(&a["opcode"]),
                    n(&a["button"]),
                    n(&a["slot"]),
                    n(&a["item"])
                )?)
            }
            "input" => {
                let p = p.unwrap();
                let value = match a.get("text") {
                    Some(text) => Response::Text(text.as_str().unwrap().encode_utf16().collect()),
                    None => Response::Int(n(&a["int"])),
                };
                let had = self.player(&p).attributes.contains_key("runscript");
                input_prompt::receive(&mut Input(self), &p, value).map_err(from_input)?;
                if had { Value::Null } else { json!("none") }
            }
            "tick" => {
                self.ticks = self.ticks.wrapping_add(n(&a["n"]));
                Value::Null
            }
            "pulse" => {
                let pi = pi.unwrap();
                let run: Vec<(usize, i32)> = self
                    .pulses
                    .iter()
                    .copied()
                    .filter(|(q, _)| *q == pi)
                    .collect();
                self.pulses.retain(|(q, _)| *q != pi);
                for (_, slot) in &run {
                    module::alert_pulse(self, &p.clone().unwrap(), *slot)?;
                }
                json!(run.len())
            }
            "move" => {
                self.players[pi.unwrap()].location = Location {
                    x: n(&a["x"]),
                    y: n(&a["y"]),
                    z: n(&a["z"]),
                };
                Value::Null
            }
            "active" => {
                self.players[pi.unwrap()].active = a["value"].as_bool().unwrap();
                Value::Null
            }
            "ironman" => {
                self.players[pi.unwrap()]
                    .ironman
                    .set_mode(Some(mode(a["mode"].as_str().unwrap())));
                Value::Null
            }
            "attr" => {
                let key = a["key"].as_str().unwrap().to_string();
                let attrs = &mut self.players[pi.unwrap()].attributes;
                if a["value"].is_null() {
                    attrs.remove(&key);
                } else {
                    attrs.insert(key, Attr::Int(n(&a["value"])));
                }
                Value::Null
            }
            "invAdd" => {
                let item: Item = Shared::new(ItemValue::new(
                    n(&a["id"]),
                    n(&a["amount"]),
                    1000,
                    &self.definitions,
                ));
                let mut inventory = self.players[pi.unwrap()].inventory.clone();
                let defs = self.definitions.clone();
                json!(inventory.add(Some(&item), true, -1, &defs, &mut Items { host: self })?)
            }
            "invReplace" => {
                let item: Option<Item> = (!a["id"].is_null()).then(|| {
                    Shared::new(ItemValue::new(
                        n(&a["id"]),
                        n(&a["amount"]),
                        1000,
                        &self.definitions,
                    ))
                });
                let mut inventory = self.players[pi.unwrap()].inventory.clone();
                let defs = self.definitions.clone();
                inventory.replace(
                    item.as_ref(),
                    n(&a["slot"]),
                    true,
                    &defs,
                    &mut Items { host: self },
                )?;
                Value::Null
            }
            "close" => {
                let p = p.unwrap();
                let m = self.player(&p).manager.clone();
                json!(interfaces::close(&mut Ui::new(self, &p), &m)?)
            }
            "open" => {
                let p = p.unwrap();
                let m = self.player(&p).manager.clone();
                let c = self.registry.component(n(&a["id"]));
                interfaces::open(&mut Ui::new(self, &p), &m, Some(c))?;
                Value::Null
            }
            "logout" => {
                let p = p.unwrap();
                let m = self.player(&p).manager.clone();
                interfaces::close(&mut Ui::new(self, &p), &m)?;
                interfaces::close_single_tab(&mut Ui::new(self, &p), &m)?;
                self.player(&p).active = false;
                Value::Null
            }
            other => panic!("unknown action {other}"),
        })
    }

    fn step(&mut self, body: impl FnOnce(&mut Self) -> R<Value>) -> Value {
        self.events.clear();
        let (result, error) = match body(self) {
            Ok(value) => (value, Value::Null),
            Err(e) => (Value::Null, json!(category(&e))),
        };
        json!({"result": result, "error": error, "events": std::mem::take(&mut self.events), "state": self.state()})
    }
}

fn mode(name: &str) -> ironman::Mode {
    match name {
        "NONE" => ironman::Mode::None,
        "STANDARD" => ironman::Mode::Standard,
        "HARDCORE" => ironman::Mode::Hardcore,
        "ULTIMATE" => ironman::Mode::Ultimate,
        other => panic!("ironman mode {other}"),
    }
}

/// Java exception names of the original for each native error.
pub fn category(e: &Error) -> String {
    match e {
        Error::Null => "NullPointerException".into(),
        Error::Number => "NumberFormatException".into(),
        Error::Bounds => "ArrayIndexOutOfBoundsException".into(),
        Error::ListBounds => "IndexOutOfBoundsException".into(),
        Error::StringBounds => "StringIndexOutOfBoundsException".into(),
        Error::ClassCast => "ClassCastException".into(),
        Error::Concurrent => "ConcurrentModificationException".into(),
        Error::Service(text) => match text.strip_prefix("injected ") {
            Some(name) => format!("Injected:{name}"),
            None => format!("Service:{text}"),
        },
    }
}

fn to_input(e: Error) -> input_prompt::Error {
    match e {
        Error::Number => input_prompt::Error::Number,
        Error::ClassCast => input_prompt::Error::Type,
        Error::Null => input_prompt::Error::Null,
        other => input_prompt::Error::Binding(format!("trade:{}", other.code())),
    }
}
fn from_input(e: input_prompt::Error) -> Error {
    match e {
        input_prompt::Error::Number => Error::Number,
        input_prompt::Error::Type => Error::ClassCast,
        input_prompt::Error::Null => Error::Null,
        input_prompt::Error::Binding(text) => match text.strip_prefix("trade:Service:") {
            Some(rest) => Error::Service(rest.into()),
            None => match text.strip_prefix("trade:") {
                Some("Bounds") => Error::Bounds,
                Some("ListBounds") => Error::ListBounds,
                Some("StringBounds") => Error::StringBounds,
                Some("Concurrent") => Error::Concurrent,
                _ => Error::Service(text),
            },
        },
    }
}

/// A container context for set-up only: those containers have no listeners.
struct NoListeners;
impl rs09_server::inventory::container::Context for NoListeners {
    fn update(
        &mut self,
        _: Listener,
        _: &mut Container,
        _: Event,
        _: &ItemRegistry,
    ) -> rs09_server::inventory::Result<()> {
        unreachable!()
    }
    fn refresh(
        &mut self,
        _: Listener,
        _: &mut Container,
        _: &ItemRegistry,
    ) -> rs09_server::inventory::Result<()> {
        unreachable!()
    }
    fn ground(
        &mut self,
        _: &mut Container,
        _: Option<Item>,
        _: &ItemRegistry,
    ) -> rs09_server::inventory::Result<()> {
        unreachable!()
    }
}

/// The native input prompt's context: attributes of the case player, the
/// recorded interpreter, and the trade amount script as the callback.
struct Input<'a>(&'a mut World);
impl input_prompt::Context for Input<'_> {
    type Value = Response;
    type Script = AmountScript<NodeHandle>;
    type Interpreter = usize;
    type Bank = ();
    fn script(&mut self, p: &NodeHandle) -> input_prompt::Result<Option<Self::Script>> {
        Ok(match self.0.player(p).attributes.get("runscript") {
            Some(Attr::Script(s)) => Some(s.clone()),
            None => None,
            _ => return Err(input_prompt::Error::Type),
        })
    }
    fn interaction_locked(&mut self, _: &NodeHandle) -> input_prompt::Result<bool> {
        Ok(false)
    }
    fn input_type(&mut self, p: &NodeHandle) -> input_prompt::Result<InputType> {
        Ok(match self.0.player(p).attributes.get("input-type") {
            Some(Attr::Type(t)) => *t,
            None => InputType::Numeric,
            _ => return Err(input_prompt::Error::Type),
        })
    }
    fn parse_amount(&mut self, p: &NodeHandle) -> input_prompt::Result<bool> {
        Ok(match self.0.player(p).attributes.get("parseamount") {
            Some(Attr::Bool(b)) => *b,
            None => false,
            _ => return Err(input_prompt::Error::Type),
        })
    }
    fn string(&mut self, value: &Response) -> input_prompt::Result<Vec<u16>> {
        Ok(match value {
            Response::Int(v) => v.to_string().encode_utf16().collect(),
            Response::Text(t) => t.clone(),
        })
    }
    fn invoke(&mut self, script: Self::Script, value: Response) -> input_prompt::Result<()> {
        plugin::run_amount(self.0, &script, &value).map_err(to_input)
    }
    fn dialogue(&mut self, p: &NodeHandle, text: Vec<u16>) -> input_prompt::Result<()> {
        let i = self.0.index(p);
        self.0
            .emit(
                "dialogue",
                vec![json!(i), json!(String::from_utf16(&text).unwrap())],
            )
            .map_err(to_input)
    }
    fn remove(&mut self, p: &NodeHandle, key: &str) -> input_prompt::Result<()> {
        self.0.player(p).attributes.remove(key);
        Ok(())
    }
    fn set_parse_amount(&mut self, p: &NodeHandle, value: bool) -> input_prompt::Result<()> {
        self.0
            .player(p)
            .attributes
            .insert("parseamount".into(), Attr::Bool(value));
        Ok(())
    }
    fn interpreter(&mut self, p: &NodeHandle) -> input_prompt::Result<usize> {
        Ok(self.0.index(p))
    }
    fn send_input(&mut self, d: usize, string: bool, prompt: Vec<u16>) -> input_prompt::Result<()> {
        self.0
            .emit(
                "sendInput",
                vec![
                    json!(d),
                    json!(string),
                    json!(String::from_utf16(&prompt).unwrap()),
                ],
            )
            .map_err(to_input)
    }
    fn send_long(&mut self, d: usize, prompt: Vec<u16>) -> input_prompt::Result<()> {
        self.0
            .emit(
                "sendLongInput",
                vec![json!(d), json!(String::from_utf16(&prompt).unwrap())],
            )
            .map_err(to_input)
    }
    fn send_message(&mut self, d: usize, prompt: Vec<u16>) -> input_prompt::Result<()> {
        self.0
            .emit(
                "sendMessageInput",
                vec![json!(d), json!(String::from_utf16(&prompt).unwrap())],
            )
            .map_err(to_input)
    }
    fn set_script(&mut self, p: &NodeHandle, script: Self::Script) -> input_prompt::Result<()> {
        self.0
            .player(p)
            .attributes
            .insert("runscript".into(), Attr::Script(script));
        Ok(())
    }
    fn set_type(&mut self, p: &NodeHandle, kind: InputType) -> input_prompt::Result<()> {
        self.0
            .player(p)
            .attributes
            .insert("input-type".into(), Attr::Type(kind));
        Ok(())
    }
    fn make_transfer(&mut self, _: input_prompt::Transfer) -> input_prompt::Result<Self::Script> {
        Err(input_prompt::Error::Binding(
            "bank transfer is not part of the trade".into(),
        ))
    }
    fn bank(&mut self, _: &NodeHandle) -> input_prompt::Result<()> {
        Err(input_prompt::Error::Binding(
            "bank transfer is not part of the trade".into(),
        ))
    }
    fn take(&mut self, _: (), _: i32, _: i32) -> input_prompt::Result<()> {
        Err(input_prompt::Error::Binding(
            "bank transfer is not part of the trade".into(),
        ))
    }
    fn deposit(&mut self, _: (), _: i32, _: i32) -> input_prompt::Result<()> {
        Err(input_prompt::Error::Binding(
            "bank transfer is not part of the trade".into(),
        ))
    }
    fn last_amount(&mut self, _: (), _: i32) -> input_prompt::Result<()> {
        Err(input_prompt::Error::Binding(
            "bank transfer is not part of the trade".into(),
        ))
    }
    fn after(&mut self, _: Plugin) -> input_prompt::Result<()> {
        Err(input_prompt::Error::Binding(
            "bank transfer is not part of the trade".into(),
        ))
    }
}

fn unavailable<T>(what: &str) -> R<T> {
    Err(Error::Service(format!(
        "{what} is not reached by the trade corpus"
    )))
}

impl Host for World {
    type Player = NodeHandle;
    fn ticks(&mut self) -> R<i32> {
        Ok(self.ticks)
    }
    fn dev_mode(&mut self) -> R<bool> {
        Ok(self.dev)
    }
    fn definitions(&mut self) -> R<Arc<ItemRegistry>> {
        Ok(self.definitions.clone())
    }
    fn factory(&mut self) -> R<Factory> {
        Ok(Factory {
            null_item: self.null_item.clone(),
        })
    }
    fn components(&mut self) -> R<Components> {
        Ok(self.components.clone())
    }
    fn username(&mut self, p: &NodeHandle) -> R<String> {
        Ok(self.player(p).name.clone())
    }
    fn location(&mut self, p: &NodeHandle) -> R<Location> {
        Ok(self.player(p).location)
    }
    fn is_active(&mut self, p: &NodeHandle) -> R<bool> {
        Ok(self.player(p).active)
    }
    fn is_artificial(&mut self, p: &NodeHandle) -> R<bool> {
        Ok(self.player(p).artificial)
    }
    fn inventory(&mut self, p: &NodeHandle) -> R<Container> {
        Ok(self.player(p).inventory.clone())
    }
    fn ironman(&mut self, p: &NodeHandle) -> R<ironman::Manager> {
        Ok(self.player(p).ironman.clone())
    }
    fn interface_manager(&mut self, p: &NodeHandle) -> R<Manager> {
        Ok(self.player(p).manager.clone())
    }
    fn ip_address(&mut self, p: &NodeHandle) -> R<String> {
        Ok(self.player(p).ip.clone())
    }
    fn mac_address(&mut self, p: &NodeHandle) -> R<String> {
        Ok(self.player(p).mac.clone())
    }
    fn computer_name(&mut self, p: &NodeHandle) -> R<String> {
        Ok(self.player(p).host.clone())
    }
    fn attribute_int(&mut self, p: &NodeHandle, key: &'static str, default: i32) -> R<i32> {
        match self.player(p).attributes.get(key) {
            None => Ok(default),
            Some(Attr::Int(v)) => Ok(*v),
            Some(_) => Err(Error::ClassCast),
        }
    }
    fn set_attribute_int(&mut self, p: &NodeHandle, key: &'static str, value: i32) -> R<()> {
        self.player(p)
            .attributes
            .insert(key.into(), Attr::Int(value));
        Ok(())
    }
    fn attribute_bool(&mut self, p: &NodeHandle, key: &'static str, default: bool) -> R<bool> {
        match self.player(p).attributes.get(key) {
            None => Ok(default),
            Some(Attr::Bool(v)) => Ok(*v),
            Some(_) => Err(Error::ClassCast),
        }
    }
    fn last_request(&mut self, p: &NodeHandle) -> R<Option<RequestType>> {
        match self.player(p).attributes.get("lastRequest") {
            None => Ok(None),
            Some(Attr::Request(kind)) => Ok(Some(*kind)),
            Some(_) => Err(Error::ClassCast),
        }
    }
    fn set_last_request(&mut self, p: &NodeHandle, kind: RequestType) -> R<()> {
        self.player(p)
            .attributes
            .insert("lastRequest".into(), Attr::Request(kind));
        Ok(())
    }
    fn request_target(&mut self, p: &NodeHandle) -> R<Option<NodeHandle>> {
        Ok(self.player(p).target.clone())
    }
    fn set_request_target(&mut self, p: &NodeHandle, target: Option<NodeHandle>) -> R<()> {
        self.player(p).target = target;
        Ok(())
    }
    fn trade_module(&mut self, p: &NodeHandle) -> R<Option<Module<NodeHandle>>> {
        Ok(self.player(p).module.clone())
    }
    fn add_trade_module(&mut self, p: &NodeHandle, module: Module<NodeHandle>) -> R<()> {
        if !self.modules.iter().any(|m| m.same(&module)) {
            self.modules.push(module.clone());
        }
        self.player(p).module = Some(module);
        Ok(())
    }
    fn remove_trade_module(&mut self, p: &NodeHandle) -> R<()> {
        self.player(p).module = None;
        Ok(())
    }
    fn message(&mut self, p: &NodeHandle, text: &str) -> R<()> {
        let i = self.index(p);
        self.emit("message", vec![json!(i), json!(text)])
    }
    fn content_message(&mut self, p: &NodeHandle, text: &str) -> R<()> {
        let i = self.index(p);
        self.emit("contentMessage", vec![json!(i), json!(text)])
    }
    fn set_varp(&mut self, p: &NodeHandle, id: i32, value: i32) -> R<()> {
        let i = self.index(p);
        self.emit("varp", vec![json!(i), json!(id), json!(value)])?;
        self.send(p, rs09_protocol::game::updates::varp(id, value))
    }
    fn send(&mut self, p: &NodeHandle, frame: Frame) -> R<()> {
        let i = self.index(p);
        let hex: String = frame.payload.iter().map(|b| format!("{b:02x}")).collect();
        self.emit("send", vec![json!(i), json!(frame.opcode), json!(hex)])
    }
    fn dialogue_close(&mut self, p: &NodeHandle) -> R<()> {
        let i = self.index(p);
        self.emit("dialogueClose", vec![json!(i)])
    }
    fn zone_can_request(
        &mut self,
        p: &NodeHandle,
        kind: RequestType,
        target: &NodeHandle,
    ) -> R<bool> {
        let (i, t) = (self.index(p), self.index(target));
        let name = match kind {
            RequestType::Trade => "TRADE",
            RequestType::Assist => "ASSIST",
        };
        self.emit("zone", vec![json!(i), json!(name), json!(t)])?;
        Ok(!self.zone_refuse.contains(&i))
    }
    fn bot(&mut self, name: &str) -> R<Option<Bot>> {
        Ok(self.bots.get(name).copied())
    }
    fn bot_trade_received(&mut self, bot: &str, from: &NodeHandle) -> R<()> {
        let i = self.index(from);
        self.emit("botTradeReceived", vec![json!(bot), json!(i)])
    }
    fn bot_items_received(&mut self, bot: &str, from: &NodeHandle, items: &Container) -> R<()> {
        let i = self.index(from);
        self.emit(
            "botItemsReceived",
            vec![json!(bot), json!(i), Self::container(items)],
        )
    }
    fn assist_open(&mut self, p: &NodeHandle, target: &NodeHandle) -> R<()> {
        let (i, t) = (self.index(p), self.index(target));
        self.emit("assistOpen", vec![json!(i), json!(t)])
    }
    fn log_trade(
        &mut self,
        p: &NodeHandle,
        target: &NodeHandle,
        mine: &Container,
        theirs: &Container,
    ) -> R<()> {
        let (i, t) = (self.index(p), self.index(target));
        self.emit(
            "logTrade",
            vec![
                json!(i),
                json!(t),
                Self::container(mine),
                Self::container(theirs),
            ],
        )
    }
    fn ground(&mut self, item: &Item, p: &NodeHandle) -> R<()> {
        let i = self.index(p);
        let (id, amount) = {
            let item = item.read();
            (item.id(), item.amount())
        };
        self.emit("ground", vec![json!(i), json!(id), json!(amount)])
    }
    fn submit_alert(&mut self, p: &NodeHandle, slot: i32) -> R<()> {
        let i = self.index(p);
        self.emit("alertSubmit", vec![json!(i), json!(slot)])?;
        self.pulses.push((i, slot));
        Ok(())
    }
    fn input_dialogue(
        &mut self,
        p: &NodeHandle,
        numeric: bool,
        prompt: &str,
        script: AmountScript<NodeHandle>,
    ) -> R<()> {
        let kind = if numeric {
            InputType::Numeric
        } else {
            InputType::Short
        };
        input_prompt::send(
            &mut Input(self),
            p,
            kind,
            prompt.encode_utf16().collect(),
            script,
        )
        .map_err(from_input)
    }
    fn examine(&mut self, item: &Item) -> R<String> {
        let id = item.read().id();
        self.emit("examine", vec![json!(id)])?;
        Ok(self.examines[&id].clone())
    }
    fn listener_update(&mut self, listener: Listener, _: &mut Container, event: Event) -> R<()> {
        let owner = listener
            .value::<Recorder>()
            .expect("only inventory recorders")
            .0;
        let slots = event.read().slots();
        self.emit("invUpdate", vec![json!(owner), json!(slots)])
    }
    fn listener_refresh(&mut self, listener: Listener, _: &mut Container) -> R<()> {
        let owner = listener
            .value::<Recorder>()
            .expect("only inventory recorders")
            .0;
        self.emit("invRefresh", vec![json!(owner)])
    }
    fn ui_validate(&mut self, p: &NodeHandle, m: &Manager) -> R<()> {
        assert!(
            m.player().as_ref() == Some(p),
            "a manager is driven only for its own player"
        );
        Ok(())
    }
    fn ui_component(&mut self, _: &NodeHandle, id: i32) -> R<Component> {
        Ok(self.registry.component(id))
    }
    fn ui_resizable(&mut self, _: &NodeHandle) -> R<bool> {
        Ok(false)
    }
    fn ui_log(&mut self, _: &NodeHandle, text: String) -> R<()> {
        self.emit("log", vec![json!(text)])
    }
    fn ui_listener_open(&mut self, p: &NodeHandle, c: &Component) -> R<()> {
        let (i, id) = (self.index(p), c.read().id);
        self.emit("listenerOpen", vec![json!(i), json!(id)])
    }
    fn ui_listener_close(&mut self, p: &NodeHandle, c: &Component) -> R<bool> {
        let (i, id) = (self.index(p), c.read().id);
        self.emit("listenerClose", vec![json!(i), json!(id)])?;
        Ok(true)
    }
    fn ui_plugin_open(&mut self, _: &NodeHandle, _: Plugin, _: &Component) -> R<()> {
        unavailable("another component plugin")
    }
    fn ui_close_event(&mut self, _: &NodeHandle, _: CloseEvent, _: &Component) -> R<bool> {
        unavailable("another close event")
    }
    fn ui_dispatch_open(&mut self, p: &NodeHandle, c: Component) -> R<()> {
        let (i, id) = (self.index(p), c.read().id);
        self.emit("dispatchOpen", vec![json!(i), json!(id)])
    }
    fn ui_dispatch_close(&mut self, p: &NodeHandle, c: Option<Component>) -> R<()> {
        let i = self.index(p);
        self.emit(
            "dispatchClose",
            vec![json!(i), json!(c.map(|c| c.read().id))],
        )
    }
    fn ui_weapon_extension(&mut self, p: &NodeHandle) -> R<Option<Component>> {
        Ok(self.player(p).weapon.clone())
    }
    fn ui_create_weapon(&mut self, p: &NodeHandle) -> R<Component> {
        let c = self.registry.component(92);
        c.write().weapon = true;
        let i = self.index(p);
        self.emit("weaponCreate", vec![json!(i)])?;
        Ok(c)
    }
    fn ui_add_weapon(&mut self, p: &NodeHandle, c: Component) -> R<()> {
        self.player(p).weapon = Some(c);
        Ok(())
    }
    fn ui_spellbook(&mut self, _: &NodeHandle) -> R<i32> {
        Ok(192)
    }
    fn ui_has_familiar(&mut self, _: &NodeHandle) -> R<bool> {
        Ok(false)
    }
    fn ui_friends_component(&mut self, _: &NodeHandle) -> R<Component> {
        let c = self.registry.component(550);
        self.emit("friendsCreate", vec![])?;
        Ok(c)
    }
    fn ui_autocast(&mut self, _: &NodeHandle) -> R<Option<usize>> {
        Ok(None)
    }
    fn ui_auto_spell_id(&mut self, _: &NodeHandle, _: &Component, _: usize) -> R<i32> {
        unavailable("autocast")
    }
    fn ui_select_auto_spell(&mut self, _: &NodeHandle, _: &Component, _: i32) -> R<()> {
        unavailable("autocast")
    }
    fn ui_window_mode(&mut self, _: &NodeHandle) -> R<i32> {
        Ok(1)
    }
    fn ui_set_window_mode(&mut self, _: &NodeHandle, _: i32) -> R<()> {
        unavailable("window mode switching")
    }
    fn ui_tutorial_complete(&mut self, _: &NodeHandle) -> R<bool> {
        unavailable("the tutorial")
    }
    fn ui_tutorial_tabs(&mut self, _: &NodeHandle) -> R<()> {
        unavailable("the tutorial")
    }
    fn ui_has_runscript(&mut self, p: &NodeHandle) -> R<bool> {
        Ok(self.player(p).attributes.contains_key("runscript"))
    }
    fn ui_remove_runscript(&mut self, p: &NodeHandle) -> R<()> {
        self.player(p).attributes.remove("runscript");
        Ok(())
    }
    fn ui_set_unclosable(&mut self, _: &NodeHandle, _: bool) -> R<()> {
        unavailable("setUnclosable")
    }
    fn ui_unclosable(&mut self, _: &NodeHandle) -> R<bool> {
        unavailable("setUnclosable")
    }
}

/// Runs one case and returns what the original recorded for it.
pub fn evaluate(case: &Value) -> Value {
    let mut world = World::new(case);
    let mut out = vec![world.step(|w| w.setup())];
    for a in case["actions"].as_array().unwrap() {
        out.push(world.step(|w| w.act(a)));
    }
    Value::Array(out)
}
