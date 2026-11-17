//! Replays a Grand Exchange oracle case on the native library. Players are
//! canonical node handles; their inventories are native containers, their
//! interface managers the native InterfaceManager, their Ironman managers the
//! native manager; the StockMarket/ExchangeItemSets listeners live in the
//! native InterfaceListeners registry and the GrandExchangeInterface plugin in
//! the native component definitions; the inputs go through the native input
//! prompt; the database is a native SQLite file per case. Everything else
//! the library asks of its host is answered from the case, as Support.java
//! answers the original classes, and recorded in the same form.
use rs09_game_core::java_json::{self, Json};
use rs09_protocol::game::Frame;
use rs09_server::{
    definitions::item::{ItemDefinition, Registry as ItemRegistry, Shared},
    inventory::{
        container::{Container, ContainerType, Event, Factory, SortType},
        container_listener::{Handle as Listener, NativeListener},
        item::{Handle as Item, Item as ItemValue},
    },
    player::ironman,
    world::{
        component_objects::Plugin,
        drop_tables::LogLevel,
        grand_exchange::{
            self as ge, Error, Exchange, GuideType, Host, InputScript, InterfacePlugin, Kind,
            Offer, OfferState, Param, Pulse, Records, Worker, auto_stock, db, exchange,
            guide_price, interface, item_sets, offer, price_index, records, sets_interface,
            stock_market, timer,
            ui::{Callbacks, Items, Ui},
        },
        input_prompt::{self, InputType, Response},
        interface_listeners::{self, Button, Callback},
        interfaces::{self, CloseEvent, Handle as Component, Manager, Open},
        player_walk::{NodeHandle, State},
    },
};
use serde_json::{Map, Value, json};
use std::{collections::HashMap, path::PathBuf, sync::Arc};

const CONFIG: &str = include_str!("../../fixtures/interfaces/interface_configs.json");
const INPUTS: &str = include_str!("../../fixtures/grand-exchange/inputs.json");

#[derive(Clone)]
enum Attr {
    Int(i32),
    Bool(bool),
    Offer(Offer),
    Records(Records),
    Guide(GuideType),
    Listener(Listener),
    Script(InputScript),
    Type(InputType),
}

struct Player {
    node: NodeHandle,
    name: String,
    uid: i32,
    artificial: bool,
    pin: bool,
    online: bool,
    attributes: HashMap<String, Attr>,
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
    listeners: interface_listeners::Registry,
    definitions: Arc<ItemRegistry>,
    order: Vec<i32>,
    factory_null: Item,
    exchange: Exchange,
    examines: HashMap<i32, String>,
    events: Vec<Value>,
    calls: HashMap<String, i64>,
    now: i64,
    step: i64,
    shared_uid: i32,
    cheater: bool,
    influence: bool,
    limit: i32,
    autostock: bool,
    data_path: String,
    autostock_file: Option<String>,
    pulses: Vec<Pulse>,
    last_db: Option<Value>,
}

type R<T> = ge::Result<T>;

fn n(value: &Value) -> i32 {
    value.as_i64().unwrap() as i32
}

/// A fresh directory for one case's database under the system temp folder.
fn case_dir(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("rs09-ge-replay-{}", std::process::id()));
    let dir = root.join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
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

    fn new(case: &Value) -> Self {
        let inputs: Value = serde_json::from_str(INPUTS).unwrap();
        let table = case["itemsTable"].as_str().unwrap();
        let mut definitions = ItemRegistry::default();
        let mut examines = HashMap::new();
        let mut ids = Vec::new();
        for (key, spec) in inputs["items"][table].as_object().unwrap() {
            let id: i32 = key.parse().unwrap();
            ids.push(id);
            let mut d = ItemDefinition {
                id,
                name: spec["name"].as_str().map(Into::into),
                stackable: spec["stackable"] == true,
                unnoted: spec["unnoted"] != false,
                note_id: spec["noteId"].as_i64().map_or(-1, |v| v as i32),
                value: spec["value"].as_i64().map_or(1, |v| v as i32),
                ..ItemDefinition::default()
            };
            if let Some(options) = spec["options"].as_array() {
                d.options = Some(Shared::new(
                    options.iter().map(|o| o.as_str().map(Into::into)).collect(),
                ));
            }
            if let Some(value) = spec.get("tradeable") {
                d.handlers.insert(
                    "tradeable".into(),
                    rs09_server::definitions::property::Property::Bool(value.as_bool().unwrap()),
                );
            }
            for key in ["high_alchemy", "low_alchemy"] {
                if let Some(value) = spec.get(key) {
                    d.handlers.insert(
                        key.into(),
                        rs09_server::definitions::property::Property::Int(n(value)),
                    );
                }
            }
            examines.insert(id, spec["examine"].as_str().unwrap_or("null").to_string());
            definitions.insert(id, Shared::new(d));
        }
        let order = db::definition_map_order(ids);
        let definitions = Arc::new(definitions);
        let factory = Factory::new(&definitions);
        let mut registry = interfaces::Registry::default();
        registry.load(CONFIG).unwrap();
        let mut state = State::default();
        let mut players = Vec::new();
        for (i, spec) in case["players"].as_array().unwrap().iter().enumerate() {
            let id = state.add(
                i as i32,
                rs09_server::world::doors::Location::new(3165, 3487, 0),
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
            players.push(Player {
                manager: Manager::new(Some(node.clone())),
                node,
                name: spec["name"].as_str().unwrap().into(),
                uid: n(&spec["uid"]),
                artificial: spec["artificial"] == true,
                pin: spec["pin"] != false,
                online: spec["online"] != false,
                attributes: HashMap::new(),
                inventory,
                ironman,
                weapon: None,
            });
        }
        let dir = case_dir(case["name"].as_str().unwrap());
        let data_path = format!("{}{}", dir.display(), std::path::MAIN_SEPARATOR);
        let exchange = Exchange::new();
        if let Some(index) = case.get("containerIndex") {
            exchange.tables().container_index = n(index);
        }
        let constants = &case["constants"];
        let mut world = Self {
            case: case.clone(),
            players,
            registry,
            listeners: interface_listeners::Registry::default(),
            factory_null: factory.null_item.clone(),
            definitions,
            order,
            exchange,
            examines,
            events: Vec::new(),
            calls: HashMap::new(),
            now: case
                .get("clock")
                .and_then(Value::as_i64)
                .unwrap_or(1_000_000),
            step: case.get("step").and_then(Value::as_i64).unwrap_or(1),
            shared_uid: case.get("sharedUid").map_or(0, n),
            cheater: false,
            influence: true,
            limit: 5000,
            autostock: false,
            data_path,
            autostock_file: None,
            pulses: Vec::new(),
            last_db: None,
        };
        world.constants(constants);
        if let Some(seed) = case["seed"].as_array() {
            world.raw_sql(seed);
        }
        world
    }

    fn constants(&mut self, c: &Value) {
        if let Some(v) = c.get("cheater") {
            self.cheater = v == true;
        }
        if let Some(v) = c.get("influence") {
            self.influence = v == true;
        }
        if let Some(v) = c.get("limit") {
            self.limit = n(v);
        }
        if let Some(v) = c.get("autostock") {
            self.autostock = v == true;
        }
    }

    fn db_file(&self) -> String {
        format!("{}grandexchange.db", self.data_path)
    }
    fn raw_sql(&self, statements: &[Value]) {
        let connection = rusqlite_connection(&self.db_file());
        for sql in statements {
            connection.execute(sql.as_str().unwrap(), []).unwrap();
        }
    }

    // ------------------------------------------------------------- state

    fn items(items: &[Option<Item>]) -> Value {
        Value::Array(
            items
                .iter()
                .enumerate()
                .filter_map(|(slot, item)| {
                    item.as_ref().map(|item| {
                        let item = item.read();
                        json!([slot, item.id(), item.amount()])
                    })
                })
                .collect(),
        )
    }

    fn dump_db(&self) -> Value {
        let path = self.db_file();
        if !std::path::Path::new(&path).is_file() {
            return Value::Null;
        }
        let connection = rusqlite_connection(&path);
        let mut out = Map::new();
        let mut master = Vec::new();
        let mut tables = Vec::new();
        {
            let mut s = connection
                .prepare("SELECT type, name, tbl_name, sql FROM sqlite_master ORDER BY rowid")
                .unwrap();
            let mut rows = s.query([]).unwrap();
            while let Some(r) = rows.next().unwrap() {
                let text = |i: usize| -> Value {
                    r.get::<_, Option<String>>(i)
                        .unwrap()
                        .map_or(Value::Null, Value::String)
                };
                let kind: Option<String> = r.get(0).unwrap();
                if kind.as_deref() == Some("table") {
                    tables.push(r.get::<_, String>(1).unwrap());
                }
                master.push(json!([text(0), text(1), text(2), text(3)]));
            }
        }
        out.insert("master".into(), Value::Array(master));
        for table in tables {
            let mut columns = Vec::new();
            {
                let mut s = connection
                    .prepare(&format!("PRAGMA table_info({table})"))
                    .unwrap();
                let mut rows = s.query([]).unwrap();
                while let Some(r) = rows.next().unwrap() {
                    columns.push(r.get::<_, String>(1).unwrap());
                }
            }
            let mut q = String::from("SELECT quote(rowid)");
            for column in &columns {
                q.push_str(&format!(", quote({column})"));
            }
            q.push_str(&format!(" FROM {table} ORDER BY rowid"));
            let mut s = connection.prepare(&q).unwrap();
            let mut rows = s.query([]).unwrap();
            let mut values = Vec::new();
            while let Some(r) = rows.next().unwrap() {
                let mut row = Vec::new();
                for i in 0..=columns.len() {
                    row.push(
                        r.get::<_, Option<String>>(i)
                            .unwrap()
                            .map_or(Value::Null, Value::String),
                    );
                }
                values.push(Value::Array(row));
            }
            out.insert(table, Value::Array(values));
        }
        Value::Object(out)
    }

    fn state(&mut self) -> Value {
        let mut labels = Labels::default();
        let mut players = Vec::new();
        for i in 0..self.players.len() {
            let p = &self.players[i];
            let f = p.manager.fields.read();
            let id = |c: &Option<Component>| c.as_ref().map(|c| c.read().id);
            let mut s = Map::new();
            s.insert("inv".into(), Self::items(&p.inventory.items().read()));
            let attribute = match p.attributes.get("ge-listener") {
                Some(Attr::Listener(l)) => Some(l.clone()),
                _ => None,
            };
            let listeners: Vec<Value> = p
                .inventory
                .listeners()
                .iter()
                .map(|l| match l {
                    None => Value::Null,
                    Some(l) => {
                        if let Some(r) = l.value::<Recorder>() {
                            json!(format!("rec:{}", r.0))
                        } else if let Some(sets) = l.value::<sets_interface::SetsListener>() {
                            let owner = self.index(&sets.player);
                            json!(["sets", owner, attribute.as_ref().is_some_and(|a| a.same(l))])
                        } else {
                            json!("other")
                        }
                    }
                })
                .collect();
            s.insert("listeners".into(), Value::Array(listeners));
            s.insert("opened".into(), json!(id(&f.opened)));
            s.insert("single".into(), json!(id(&f.single_tab)));
            s.insert("chatbox".into(), json!(id(&f.chatbox)));
            s.insert("count".into(), json!(f.packet_count));
            drop(f);
            let attrs = p.attributes.clone();
            s.insert(
                "geIndex".into(),
                match attrs.get("ge-index") {
                    Some(Attr::Int(v)) => json!(v),
                    None => Value::Null,
                    _ => panic!("ge-index type"),
                },
            );
            let temp = match attrs.get("ge-temp") {
                Some(Attr::Offer(o)) => Some(o.clone()),
                None => None,
                _ => panic!("ge-temp type"),
            };
            let temp = labels.offer(self, temp.as_ref());
            s.insert("temp".into(), temp);
            let recs = match attrs.get("ge-records") {
                Some(Attr::Records(r)) => Some(r.clone()),
                None => None,
                _ => panic!("ge-records type"),
            };
            let recs = labels.records(self, recs.as_ref());
            s.insert("records".into(), recs);
            s.insert(
                "guide".into(),
                match attrs.get("guide-price") {
                    Some(Attr::Guide(g)) => json!(g.name()),
                    None => Value::Null,
                    _ => panic!("guide type"),
                },
            );
            s.insert(
                "key".into(),
                match attrs.get("container-key") {
                    Some(Attr::Int(v)) => json!(v),
                    None => Value::Null,
                    _ => panic!("key type"),
                },
            );
            s.insert(
                "setsListener".into(),
                json!(attrs.contains_key("ge-listener")),
            );
            s.insert("runscript".into(), json!(attrs.contains_key("runscript")));
            s.insert(
                "parseamount".into(),
                match attrs.get("parseamount") {
                    Some(Attr::Bool(b)) => json!(b),
                    None => Value::Null,
                    _ => panic!("parseamount type"),
                },
            );
            s.insert(
                "inputType".into(),
                json!(attrs.get("input-type").map(|a| match a {
                    Attr::Type(t) => t.to_string(),
                    _ => panic!("input-type type"),
                })),
            );
            let mode = match self.players[i].ironman.mode() {
                Some(ironman::Mode::None) => "NONE",
                Some(ironman::Mode::Standard) => "STANDARD",
                Some(ironman::Mode::Hardcore) => "HARDCORE",
                Some(ironman::Mode::Ultimate) => "ULTIMATE",
                None => panic!("null ironman mode"),
            };
            s.insert("ironman".into(), json!(mode));
            players.push(Value::Object(s));
        }
        let mut pending = Vec::new();
        for o in self.exchange.pending() {
            pending.push(labels.offer(self, Some(&o)));
        }
        let hook = self.exchange.hook();
        let hook = labels.records(self, Some(&hook));
        let pulses: Vec<Value> = self
            .pulses
            .iter()
            .map(|p| match p {
                Pulse::Interface(_) => json!("interface"),
                Pulse::Cheat { .. } => json!("cheat"),
            })
            .collect();
        let worker = match self.exchange.worker() {
            Worker::NotStarted => "NotStarted",
            Worker::Waiting => "Waiting",
            Worker::Dead => "Dead",
        };
        let db = self.dump_db();
        let db = if self.last_db.as_ref() == Some(&db) {
            json!("same")
        } else {
            self.last_db = Some(db.clone());
            db
        };
        json!({
            "players": players,
            "pending": pending,
            "hook": hook,
            "offers": labels.table,
            "pulses": pulses,
            "worker": worker,
            "index": self.exchange.tables().container_index,
            "db": db,
        })
    }

    fn summary(&self, o: &Offer, label: Value) -> Value {
        let f = o.fields();
        let amount = if f.is_bot {
            f.amount.min(self.limit)
        } else {
            f.amount
        };
        let withdraw: Vec<Value> = f
            .withdraw
            .iter()
            .map(|w| w.map_or(Value::Null, |w| json!([w.id, w.amount])))
            .collect();
        json!([
            label,
            f.item_id,
            amount,
            f.completed_amount,
            f.offered_value,
            f.index,
            f.sell,
            f.state.name(),
            f.uid,
            f.time_stamp,
            withdraw,
            f.total_coin_exchange,
            f.player.as_ref().map(|p| self.index(p)),
            f.player_uid,
            f.is_limitation,
            f.is_bot
        ])
    }

    fn setup_result(&self) -> Value {
        let sets: Vec<Value> = item_sets::SETS
            .iter()
            .map(|(name, id, parts)| json!([name, id, parts]))
            .collect();
        let array: Vec<Value> = item_sets::item_array()
            .into_iter()
            .map(|id| json!([id, 1]))
            .collect();
        let guides: Vec<Value> = GuideType::ALL
            .iter()
            .map(|t| {
                let [from, to] = t.child_data();
                let items: Vec<Value> = t.items().iter().map(|(i, c)| json!([i, c])).collect();
                json!([t.name(), from, to, items])
            })
            .collect();
        let states: Vec<Value> = OfferState::ALL.iter().map(|s| json!(s.name())).collect();
        json!({"sets": sets, "setItems": array, "guides": guides, "states": states})
    }

    // ------------------------------------------------------------- actions

    fn setup(&mut self) -> R<Value> {
        stock_market::define_interface_listeners(&self.listeners)?;
        sets_interface::define_interface_listeners(&self.listeners)?;
        interface::new_instance(&mut self.registry);
        for (name, privilege, usage, description) in exchange::COMMANDS {
            self.emit(
                "command",
                vec![
                    json!(name),
                    json!(privilege),
                    json!(usage),
                    json!(description),
                ],
            )?;
        }
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
            let chat = self.registry.component(137);
            interfaces::open_chatbox(&mut Ui::new(self, &node), &m, &chat)?;
            interfaces::open_default_tabs(&mut Ui::new(self, &node), &m)?;
        }
        match self.case["startup"].as_str().unwrap_or("full") {
            "full" => exchange::startup(self)?,
            "init" => {
                db::init_default(self)?;
            }
            _ => {}
        }
        for i in 0..self.players.len() {
            if self.case["players"][i]["login"] != false {
                let node = self.players[i].node.clone();
                records::login(self, &node)?;
            }
        }
        Ok(self.setup_result())
    }

    fn act(&mut self, a: &Value) -> R<Value> {
        let op = a["op"].as_str().unwrap();
        let p = a
            .get("p")
            .filter(|v| !v.is_null())
            .map(|v| self.players[v.as_u64().unwrap() as usize].node.clone());
        let pi = a
            .get("p")
            .filter(|v| !v.is_null())
            .map(|v| v.as_u64().unwrap() as usize);
        Ok(match op {
            "button" => {
                let p = p.unwrap();
                let i = pi.unwrap();
                let m = self.player(&p).manager.clone();
                let Some(c) =
                    interfaces::get_component(&mut Ui::new(self, &p), &m, n(&a["iface"]))?
                else {
                    return Ok(json!("none"));
                };
                if c.read().hidden {
                    return Ok(json!("hidden"));
                }
                let plugin = self.registry.plugin(&c);
                let id = c.read().id;
                self.emit(
                    "buttonClick",
                    vec![json!(i), json!(id), a["button"].clone()],
                )?;
                let registry = self.listeners.clone();
                let button = Button {
                    opcode: n(&a["opcode"]),
                    button: n(&a["button"]),
                    slot: n(&a["slot"]),
                    item: a.get("item").map_or(-1, n),
                };
                if registry.run(&mut Callbacks { host: self }, Some(&p), Some(&c), button)? {
                    return Ok(json!(true));
                }
                let Some(plugin) = plugin else {
                    return Ok(json!("unhandled"));
                };
                assert!(
                    plugin.value::<InterfacePlugin>().is_some(),
                    "only the GE plugin"
                );
                json!(interface::handle(
                    self,
                    interface::ButtonPulse {
                        player: p.clone(),
                        component: c.clone(),
                        opcode: button.opcode,
                        button: button.button,
                        slot: button.slot,
                        item: button.item,
                    }
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
            "pulse" => {
                let due = std::mem::take(&mut self.pulses);
                let mut out = Vec::new();
                for (i, pulse) in due.iter().enumerate() {
                    match ge::run_pulse(self, pulse) {
                        Ok(done) => {
                            out.push(json!(done));
                            if !done {
                                self.pulses.push(pulse.clone());
                            }
                        }
                        Err(e) => {
                            self.pulses.extend(due[i + 1..].iter().cloned());
                            return Err(e);
                        }
                    }
                }
                Value::Array(out)
            }
            "worker" => match self.exchange.worker() {
                Worker::NotStarted => json!("notStarted"),
                Worker::Dead => json!("dead"),
                Worker::Waiting => {
                    exchange::run_worker(self)?;
                    json!("waiting")
                }
            },
            "boot" => {
                exchange::boot(self)?;
                Value::Null
            }
            "timer" => json!(timer::run(self, p.as_ref())?),
            "openFor" => {
                stock_market::open_for(self, &p.unwrap())?;
                Value::Null
            }
            "open" => {
                let p = p.unwrap();
                let m = self.player(&p).manager.clone();
                let c = self.registry.component(n(&a["iface"]));
                interfaces::open(&mut Ui::new(self, &p), &m, Some(c))?;
                Value::Null
            }
            "close" => {
                let p = p.unwrap();
                let m = self.player(&p).manager.clone();
                json!(interfaces::close(&mut Ui::new(self, &p), &m)?)
            }
            "closeChatbox" => {
                let p = p.unwrap();
                let m = self.player(&p).manager.clone();
                interfaces::close_chatbox(&mut Ui::new(self, &p), &m)?;
                Value::Null
            }
            "setOfferItem" => {
                stock_market::set_offer_item(self, &p.unwrap(), n(&a["item"]))?;
                Value::Null
            }
            "collection" => {
                let r = records::get_instance(self, p.as_ref())?;
                records::open_collection_box(self, &r)?;
                Value::Null
            }
            "history" => {
                let p = p.unwrap();
                let r = records::get_instance(self, Some(&p))?;
                records::open_history_log(self, &r, &p)?;
                Value::Null
            }
            "guide" => {
                let kind = GuideType::from_name(a["type"].as_str().unwrap()).unwrap();
                guide_price::open(self, &p.unwrap(), kind)?;
                Value::Null
            }
            "sets" => {
                sets_interface::open_for(self, &p.unwrap())?;
                Value::Null
            }
            "login" => {
                records::login(self, &p.unwrap())?;
                Value::Null
            }
            "parse" => {
                let data =
                    java_json::parse(a["json"].as_str().unwrap()).map_err(|_| Error::Parse)?;
                if !matches!(data, Json::Object(_)) {
                    return Err(Error::ClassCast);
                }
                let hook = self.exchange.hook();
                records::parse_player(self, &hook, &p.unwrap(), &data)?;
                Value::Null
            }
            "save" => {
                let mut save = Json::object();
                records::save_player(self, &p.unwrap(), &mut save)?;
                json!(save.text())
            }
            "botOffer" => json!(exchange::add_bot_offer(
                self,
                n(&a["item"]),
                n(&a["amount"])
            )?),
            "autostock" => {
                self.autostock_file = a["file"].as_str().map(Into::into);
                auto_stock::autostock(self)?;
                Value::Null
            }
            "stats" => json!(exchange::offer_stats(
                self,
                n(&a["item"]),
                a["sale"] == true
            )?),
            "price" => json!(exchange::recommended_price(
                self,
                n(&a["item"]),
                a["bot"] == true
            )?),
            "valid" => {
                let offers = exchange::valid_offers(self)?;
                Value::Array(
                    offers
                        .iter()
                        .map(|o| self.summary(o, Value::Null))
                        .collect(),
                )
            }
            "bots" => {
                let offers = exchange::bot_offers(self)?;
                Value::Array(
                    offers
                        .iter()
                        .map(|o| self.summary(o, Value::Null))
                        .collect(),
                )
            }
            "botStock" => json!(exchange::botstock_for_id(self, n(&a["item"]))?),
            "canTrade" => json!(price_index::can_trade(self, n(&a["item"]))?),
            "ban" => {
                price_index::ban_item(self, n(&a["item"]))?;
                Value::Null
            }
            "allow" => {
                price_index::allow_item(self, n(&a["item"]))?;
                Value::Null
            }
            "value" => json!(price_index::get_value(self, n(&a["item"]))?),
            "addTrade" => {
                price_index::add_trade(self, n(&a["item"]), n(&a["amount"]), n(&a["price"]))?;
                Value::Null
            }
            "dispatch" => {
                let spec = &a["offer"];
                let o = Offer::new();
                {
                    let mut f = o.fields();
                    f.item_id = spec.get("item").map_or(0, n);
                    f.amount = spec.get("amount").map_or(0, n);
                    f.offered_value = spec.get("value").map_or(0, n);
                    f.sell = spec["sell"] == true;
                    f.index = spec.get("index").map_or(0, n);
                    f.uid = spec.get("uid").and_then(Value::as_i64).unwrap_or(0);
                    f.state = OfferState::ALL
                        .into_iter()
                        .find(|s| Some(s.name()) == spec["state"].as_str())
                        .unwrap_or(OfferState::Pending);
                }
                json!(exchange::dispatch(self, &p.unwrap(), &o)?)
            }
            "command" => {
                let strings: Vec<String> = a["args"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|s| s.as_str().unwrap().to_string())
                    .collect();
                exchange::command(self, &p.unwrap(), a["name"].as_str().unwrap(), &strings)?;
                Value::Null
            }
            "awaiting" => json!(records::has_awaiting_collections(self, &p.unwrap())?),
            "active" => {
                let r = records::get_instance(self, p.as_ref())?;
                json!(records::has_active_offer(&r))
            }
            "format" => {
                let r = records::get_instance(self, p.as_ref())?;
                json!(records::format(self, &r)?)
            }
            "cacheValue" => {
                let r = records::get_instance(self, p.as_ref())?;
                match records::get_offer(self, &r, n(&a["slot"]))? {
                    None => Value::Null,
                    Some(o) => json!(offer::cache_value(self, &o)?),
                }
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
            "attr" => {
                let key = a["key"].as_str().unwrap().to_string();
                let attrs = &mut self.players[pi.unwrap()].attributes;
                match &a["value"] {
                    Value::Null => {
                        attrs.remove(&key);
                    }
                    value => {
                        attrs.insert(key, Attr::Int(n(value)));
                    }
                }
                Value::Null
            }
            "sql" => {
                self.raw_sql(a["statements"].as_array().unwrap());
                Value::Null
            }
            "clock" => {
                self.now = a["now"].as_i64().unwrap();
                self.step = a["step"].as_i64().unwrap();
                Value::Null
            }
            "constants" => {
                self.constants(&a.clone());
                Value::Null
            }
            "online" => {
                self.players[pi.unwrap()].online = a["value"] == true;
                Value::Null
            }
            "pin" => {
                self.players[pi.unwrap()].pin = a["value"] == true;
                Value::Null
            }
            "ironman" => {
                self.players[pi.unwrap()]
                    .ironman
                    .set_mode(Some(mode(a["mode"].as_str().unwrap())));
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
        let events = std::mem::take(&mut self.events);
        json!({"result": result, "error": error, "events": events, "state": self.state()})
    }
}

/// Identity labels in first-appearance order, one counter for offers and
/// records (Support.label), and the offer table.
#[derive(Default)]
struct Labels {
    seen: Vec<(usize, String)>,
    table: Vec<Value>,
}
impl Labels {
    fn find(&self, identity: usize) -> Option<String> {
        self.seen
            .iter()
            .find(|(i, _)| *i == identity)
            .map(|(_, l)| l.clone())
    }
    fn label(&mut self, identity: usize, prefix: &str) -> String {
        if let Some(found) = self.find(identity) {
            return found;
        }
        let label = format!("{prefix}{}", self.seen.len() + 1);
        self.seen.push((identity, label.clone()));
        label
    }
    fn offer(&mut self, world: &World, o: Option<&Offer>) -> Value {
        let Some(o) = o else {
            return Value::Null;
        };
        let known = self.find(o.identity()).is_some();
        let label = self.label(o.identity(), "o");
        if !known {
            self.table.push(world.summary(o, json!(label)));
        }
        json!(label)
    }
    fn records(&mut self, world: &World, r: Option<&Records>) -> Value {
        let Some(r) = r else {
            return Value::Null;
        };
        let slots: Vec<Value> = r
            .records()
            .iter()
            .map(|s| s.map_or(Value::Null, |s| json!([s.uid, s.slot])))
            .collect();
        let history: Vec<Value> = r
            .history()
            .iter()
            .map(|o| self.offer(world, o.as_ref()))
            .collect();
        let owner = r.player().map(|p| world.index(&p));
        let notification = r.fields().update_notification;
        json!([
            self.label(r.identity(), "r"),
            owner,
            slots,
            history,
            notification
        ])
    }
}

fn rusqlite_connection(path: &str) -> rusqlite::Connection {
    if let Some(parent) = std::path::Path::new(path).parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    rusqlite::Connection::open(path).unwrap()
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
        Error::ClassCast => "ClassCastException".into(),
        Error::NoSuchElement => "NoSuchElementException".into(),
        Error::Arithmetic => "ArithmeticException".into(),
        Error::Uninitialized => "UninitializedPropertyAccessException".into(),
        Error::Parse => "ParseException".into(),
        Error::File => "FileNotFoundException".into(),
        Error::Sql(_) => "SQLiteException".into(),
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
        other => input_prompt::Error::Binding(format!("ge:{}", other.code())),
    }
}
fn from_input(e: input_prompt::Error) -> Error {
    match e {
        input_prompt::Error::Number => Error::Number,
        input_prompt::Error::Type => Error::ClassCast,
        input_prompt::Error::Null => Error::Null,
        input_prompt::Error::Binding(text) => match text.strip_prefix("ge:") {
            Some(code) => Error::from_code(code),
            None => Error::Service(text),
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
/// recorded interpreter, and the GE input scripts as the callback.
struct Input<'a>(&'a mut World);
impl input_prompt::Context for Input<'_> {
    type Value = Response;
    type Script = InputScript;
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
        stock_market::run_input(self.0, &script, &value).map_err(to_input)
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
    fn send_long(&mut self, _: usize, _: Vec<u16>) -> input_prompt::Result<()> {
        unreachable!("the GE sends short and amount prompts only")
    }
    fn send_message(&mut self, _: usize, _: Vec<u16>) -> input_prompt::Result<()> {
        unreachable!("the GE sends short and amount prompts only")
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
        unreachable!("bank transfer is not part of the GE")
    }
    fn bank(&mut self, _: &NodeHandle) -> input_prompt::Result<()> {
        unreachable!("bank transfer is not part of the GE")
    }
    fn take(&mut self, _: (), _: i32, _: i32) -> input_prompt::Result<()> {
        unreachable!("bank transfer is not part of the GE")
    }
    fn deposit(&mut self, _: (), _: i32, _: i32) -> input_prompt::Result<()> {
        unreachable!("bank transfer is not part of the GE")
    }
    fn last_amount(&mut self, _: (), _: i32) -> input_prompt::Result<()> {
        unreachable!("bank transfer is not part of the GE")
    }
    fn after(&mut self, _: Plugin) -> input_prompt::Result<()> {
        unreachable!("bank transfer is not part of the GE")
    }
}

fn unavailable<T>(what: &str) -> R<T> {
    Err(Error::Service(format!(
        "{what} is not reached by the GE corpus"
    )))
}

fn param(p: &Param) -> Value {
    match p {
        Param::Int(v) => json!(["i", v]),
        Param::Long(v) => json!(["l", v]),
        Param::Bool(v) => json!(["b", v]),
        Param::Text(v) => json!(["s", v]),
    }
}

impl Host for World {
    fn exchange(&mut self) -> R<Exchange> {
        Ok(self.exchange.clone())
    }
    fn definitions(&mut self) -> R<Arc<ItemRegistry>> {
        Ok(self.definitions.clone())
    }
    fn definition_order(&mut self) -> R<Vec<i32>> {
        Ok(self.order.clone())
    }
    fn factory(&mut self) -> R<Factory> {
        Ok(Factory {
            null_item: self.factory_null.clone(),
        })
    }
    fn now(&mut self) -> R<i64> {
        let value = self.now;
        self.emit("clock", vec![json!(value)])?;
        self.now = value.wrapping_add(self.step);
        Ok(value)
    }
    fn cheater(&mut self) -> R<bool> {
        Ok(self.cheater)
    }
    fn bots_influence_price_index(&mut self) -> R<bool> {
        Ok(self.influence)
    }
    fn botstock_limit(&mut self) -> R<i32> {
        Ok(self.limit)
    }
    fn autostock_enabled(&mut self) -> R<bool> {
        Ok(self.autostock)
    }
    fn data_path(&mut self) -> R<Option<String>> {
        Ok(Some(self.data_path.clone()))
    }
    fn read_file(&mut self, path: &str) -> R<String> {
        assert_eq!(
            path,
            auto_stock::path(),
            "the case reads its autostock file"
        );
        self.autostock_file.clone().ok_or(Error::File)
    }
    fn log(&mut self, level: LogLevel, message: &str) -> R<()> {
        let level = match level {
            LogLevel::Fine => "FINE",
            LogLevel::Info => "INFO",
            LogLevel::Warn => "WARN",
            LogLevel::Err => "ERR",
            LogLevel::Debug => "DEBUG",
        };
        self.emit("log", vec![json!(level), json!(message)])
    }
    fn log_ge(&mut self, message: &str) -> R<()> {
        self.emit("logGE", vec![json!(message)])
    }
    fn statement(&mut self, kind: Kind, sql: &str, params: &[Param]) -> R<()> {
        let params: Vec<Value> = params.iter().map(param).collect();
        self.emit("sql", vec![json!(kind.name()), json!(sql), json!(params)])
    }
    fn interface_listeners(&mut self) -> R<interface_listeners::Registry> {
        Ok(self.listeners.clone())
    }
    fn start_worker(&mut self) -> R<()> {
        self.emit("workerStart", vec![])
    }
    fn submit_pulse(&mut self, pulse: Pulse) -> R<()> {
        let kind = match &pulse {
            Pulse::Interface(_) => "interface",
            Pulse::Cheat { .. } => "cheat",
        };
        self.emit("submit", vec![json!(kind), json!(pulse.delay())])?;
        self.pulses.push(pulse);
        Ok(())
    }
    fn news(&mut self, text: &str) -> R<()> {
        self.emit("news", vec![json!(text)])
    }
    fn discord_new_offer(
        &mut self,
        sell: bool,
        item: i32,
        value: i32,
        amount: i32,
        username: &str,
    ) -> R<()> {
        self.emit(
            "discord",
            vec![
                json!(sell),
                json!(item),
                json!(value),
                json!(amount),
                json!(username),
            ],
        )
    }
    fn details_uid(&mut self, username: &str) -> R<i32> {
        self.emit("details", vec![json!(username)])?;
        Ok(self.shared_uid)
    }
    fn player_by_uid(&mut self, uid: i32) -> R<Option<NodeHandle>> {
        self.emit("uidMap", vec![json!(uid)])?;
        Ok(self
            .players
            .iter()
            .filter(|p| p.online && p.uid == uid)
            .last()
            .map(|p| p.node.clone()))
    }
    fn player_by_account(&mut self, uid: i32) -> R<Option<NodeHandle>> {
        self.emit("players", vec![])?;
        Ok(self
            .players
            .iter()
            .find(|p| p.online && p.uid == uid)
            .map(|p| p.node.clone()))
    }
    fn examine(&mut self, id: i32) -> R<String> {
        let definition = self.definitions.for_id(id).read().id;
        self.emit("examine", vec![json!(definition)])?;
        Ok(self
            .examines
            .get(&definition)
            .cloned()
            .unwrap_or_else(|| "null".into()))
    }
    fn cs2_mapping(&mut self, mapping: i32, key: i32) -> R<Option<Option<String>>> {
        self.emit("cs2", vec![json!(mapping)])?;
        let Some(map) = self.case["cs2"].get(mapping.to_string()) else {
            return Ok(None);
        };
        Ok(Some(
            map.get(key.to_string())
                .and_then(Value::as_str)
                .map(Into::into),
        ))
    }
    fn uid(&mut self, p: &NodeHandle) -> R<i32> {
        Ok(self.player(p).uid)
    }
    fn username(&mut self, p: &NodeHandle) -> R<String> {
        Ok(self.player(p).name.clone())
    }
    fn artificial(&mut self, p: &NodeHandle) -> R<bool> {
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
    fn pin_unlocked(&mut self, p: &NodeHandle) -> R<bool> {
        let i = self.index(p);
        self.emit("pinUnlocked", vec![json!(i)])?;
        Ok(self.players[i].pin)
    }
    fn pin_open(&mut self, p: &NodeHandle, kind: i32) -> R<()> {
        let i = self.index(p);
        self.emit("pinOpen", vec![json!(i), json!(kind)])
    }
    fn records(&mut self, p: &NodeHandle) -> R<Option<Records>> {
        match self.player(p).attributes.get("ge-records") {
            None => Ok(None),
            Some(Attr::Records(r)) => Ok(Some(r.clone())),
            Some(_) => Err(Error::ClassCast),
        }
    }
    fn set_records(&mut self, p: &NodeHandle, records: Records) -> R<()> {
        self.player(p)
            .attributes
            .insert("ge-records".into(), Attr::Records(records));
        Ok(())
    }
    fn temp_offer(&mut self, p: &NodeHandle) -> R<Option<Offer>> {
        match self.player(p).attributes.get("ge-temp") {
            None => Ok(None),
            Some(Attr::Offer(o)) => Ok(Some(o.clone())),
            Some(_) => Err(Error::ClassCast),
        }
    }
    fn set_temp_offer(&mut self, p: &NodeHandle, offer: Offer) -> R<()> {
        self.player(p)
            .attributes
            .insert("ge-temp".into(), Attr::Offer(offer));
        Ok(())
    }
    fn remove_temp_offer(&mut self, p: &NodeHandle) -> R<()> {
        self.player(p).attributes.remove("ge-temp");
        Ok(())
    }
    fn ge_index(&mut self, p: &NodeHandle, default: i32) -> R<i32> {
        match self.player(p).attributes.get("ge-index") {
            None => Ok(default),
            Some(Attr::Int(v)) => Ok(*v),
            Some(_) => Err(Error::ClassCast),
        }
    }
    fn set_ge_index(&mut self, p: &NodeHandle, index: i32) -> R<()> {
        self.player(p)
            .attributes
            .insert("ge-index".into(), Attr::Int(index));
        Ok(())
    }
    fn guide_price(&mut self, p: &NodeHandle) -> R<Option<GuideType>> {
        match self.player(p).attributes.get("guide-price") {
            None => Ok(None),
            Some(Attr::Guide(g)) => Ok(Some(*g)),
            Some(_) => Err(Error::ClassCast),
        }
    }
    fn set_guide_price(&mut self, p: &NodeHandle, kind: GuideType) -> R<()> {
        self.player(p)
            .attributes
            .insert("guide-price".into(), Attr::Guide(kind));
        Ok(())
    }
    fn container_key(&mut self, p: &NodeHandle, default: i32) -> R<i32> {
        match self.player(p).attributes.get("container-key") {
            None => Ok(default),
            Some(Attr::Int(v)) => Ok(*v),
            Some(_) => Err(Error::ClassCast),
        }
    }
    fn set_container_key(&mut self, p: &NodeHandle, key: i32) -> R<()> {
        self.player(p)
            .attributes
            .insert("container-key".into(), Attr::Int(key));
        Ok(())
    }
    fn remove_container_key(&mut self, p: &NodeHandle) -> R<()> {
        self.player(p).attributes.remove("container-key");
        Ok(())
    }
    fn sets_listener(&mut self, p: &NodeHandle) -> R<Option<Listener>> {
        match self.player(p).attributes.get("ge-listener") {
            None => Ok(None),
            Some(Attr::Listener(l)) => Ok(Some(l.clone())),
            Some(_) => Err(Error::ClassCast),
        }
    }
    fn set_sets_listener(&mut self, p: &NodeHandle, listener: Listener) -> R<()> {
        self.player(p)
            .attributes
            .insert("ge-listener".into(), Attr::Listener(listener));
        Ok(())
    }
    fn remove_sets_listener(&mut self, p: &NodeHandle) -> R<()> {
        self.player(p).attributes.remove("ge-listener");
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
    fn notify(&mut self, p: &NodeHandle, text: &str) -> R<()> {
        let i = self.index(p);
        self.emit("notify", vec![json!(i), json!(text)])
    }
    fn send(&mut self, p: &NodeHandle, frame: Frame) -> R<()> {
        let i = self.index(p);
        let hex: String = frame.payload.iter().map(|b| format!("{b:02x}")).collect();
        self.emit("send", vec![json!(i), json!(frame.opcode), json!(hex)])
    }
    fn set_varp(&mut self, p: &NodeHandle, id: i32, value: i32) -> R<()> {
        let i = self.index(p);
        self.emit("setVarp", vec![json!(i), json!(id), json!(value)])
    }
    fn audio(&mut self, p: &NodeHandle, id: i32) -> R<()> {
        let i = self.index(p);
        self.emit("audio", vec![json!(i), json!(id)])
    }
    fn jingle(&mut self, p: &NodeHandle, id: i32) -> R<()> {
        let i = self.index(p);
        self.emit("jingle", vec![json!(i), json!(id)])
    }
    fn input_dialogue(
        &mut self,
        p: &NodeHandle,
        kind: InputType,
        prompt: &str,
        script: InputScript,
    ) -> R<()> {
        input_prompt::send(
            &mut Input(self),
            p,
            kind,
            prompt.encode_utf16().collect(),
            script,
        )
        .map_err(from_input)
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
    fn other_listener(
        &mut self,
        _: &NodeHandle,
        _: Callback,
        _: &Component,
        _: Option<Button>,
    ) -> R<bool> {
        unavailable("another interface listener")
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
        self.emit("log", vec![json!("WARN"), json!(text)])
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
    fn ui_set_varp(&mut self, p: &NodeHandle, id: i32, value: i32) -> R<()> {
        self.set_varp(p, id, value)
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
