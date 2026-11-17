//! Replays a shops oracle case on the native library. Players are canonical
//! node handles; their inventories are native containers, their interface
//! managers the native InterfaceManager, their Ironman managers the native
//! manager, the shop's interface listeners live in the native
//! InterfaceListeners registry, and Buy-X/Sell-X go through the native input
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
        component_objects::{NativeObject, Plugin},
        drop_tables::LogLevel,
        input_prompt::{self, InputType, Response},
        interface_listeners::{self, Button, Callback},
        interfaces::{self, CloseEvent, Handle as Component, Manager, Open},
        player_walk::{NodeHandle, State},
        shops::{
            self, AmountScript, Error, Host, Location, Npc, Quest, Shop, ShopEvent, ShopListener,
            Status, World as Tables,
            controller::{self as listeners, NpcListener},
            shop as shop_fns,
            ui::{Callbacks, Items, Ui},
        },
    },
};
use serde_json::{Map, Value, json};
use std::{collections::HashMap, sync::Arc};

const CONFIG: &str = include_str!("../../fixtures/interfaces/interface_configs.json");
const INPUTS: &str = include_str!("../../fixtures/shops/inputs.json");
const PINNED: &str = include_str!("../../fixtures/startup-loaders/shops.json");
const CASES_DIR: &str = "runs/shops-oracle/cases";

#[derive(Clone)]
enum Attr {
    Bool(bool),
    Int(i32),
    Shop(Shop),
    Container(Container),
    Script(AmountScript),
    Type(InputType),
}

struct Player {
    node: NodeHandle,
    name: String,
    uid: i32,
    quest_points: i32,
    quests: HashMap<String, i32>,
    attributes: HashMap<String, Attr>,
    inventory: Container,
    ironman: ironman::Manager,
    manager: Manager,
    weapon: Option<Component>,
}

/// The player's inventory listener stand-in, as Support.InventoryRecorder.
struct Recorder(usize);
impl NativeListener for Recorder {}

/// A content item view, as Support.View: hides ids or shows one id as another.
struct ViewObject {
    hide: Vec<i32>,
    swap: Vec<i32>,
}
impl NativeObject for ViewObject {}

struct World {
    case: Value,
    players: Vec<Player>,
    registry: interfaces::Registry,
    listeners: interface_listeners::Registry,
    definitions: Arc<ItemRegistry>,
    factory_null: Item,
    tables: Tables,
    examines: HashMap<i32, String>,
    config: HashMap<String, bool>,
    ticks: i32,
    server_name: String,
    stock_clear: i32,
    recirculate: bool,
    shops_file: String,
    files: HashMap<String, String>,
    events: Vec<Value>,
    calls: HashMap<String, i64>,
    last_dump: HashMap<i32, String>,
    /// The facing_booth attribute of the current action's NPC.
    case_booth: Option<bool>,
}

type R<T> = shops::Result<T>;

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

    fn new(case: &Value) -> Self {
        let inputs: Value = serde_json::from_str(INPUTS).unwrap();
        let table = case["itemsTable"].as_str().unwrap();
        let mut definitions = ItemRegistry::default();
        let mut examines = HashMap::new();
        for (key, spec) in inputs["items"][table].as_object().unwrap() {
            let id: i32 = key.parse().unwrap();
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
                d.handlers
                    .insert("tradeable".into(), Property::Bool(value.as_bool().unwrap()));
            }
            for key in [
                "high_alchemy",
                "low_alchemy",
                "tokkul_price",
                "archery_ticket_price",
                "castle_wars_ticket_price",
            ] {
                if let Some(value) = spec.get(key) {
                    d.handlers.insert(key.into(), Property::Int(n(value)));
                }
            }
            examines.insert(id, spec["examine"].as_str().unwrap().to_string());
            definitions.insert(id, Shared::new(d));
        }
        let definitions = Arc::new(definitions);
        let factory = Factory::new(&definitions);
        let mut registry = interfaces::Registry::default();
        registry.load(CONFIG).unwrap();
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
            let quests = spec["quests"]
                .as_object()
                .map(|q| q.iter().map(|(k, v)| (k.clone(), n(v))).collect())
                .unwrap_or_default();
            players.push(Player {
                manager: Manager::new(Some(node.clone())),
                node,
                name: spec["name"].as_str().unwrap().into(),
                uid: n(&spec["uid"]),
                quest_points: spec["questPoints"].as_i64().map_or(0, |v| v as i32),
                quests,
                attributes: HashMap::new(),
                inventory,
                ironman,
                weapon: None,
            });
        }
        let mut config = HashMap::new();
        if let Some(value) = case["personalized"].as_bool() {
            config.insert(shops::PERSONALIZED_SHOPS.to_string(), value);
        }
        let files = inputs["files"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(k, v)| (k.clone(), v.as_str().unwrap().to_string()))
            .collect();
        Self {
            case: case.clone(),
            players,
            registry,
            listeners: interface_listeners::Registry::default(),
            factory_null: factory.null_item.clone(),
            tables: Tables::new(&factory).unwrap(),
            definitions,
            examines,
            config,
            ticks: n(&case["ticks"]),
            server_name: case["serverName"].as_str().unwrap().into(),
            stock_clear: n(&case["stockClear"]),
            recirculate: case["recirculate"] == true,
            shops_file: case["shopsFile"].as_str().unwrap().into(),
            files,
            events: Vec::new(),
            calls: HashMap::new(),
            last_dump: HashMap::new(),
            case_booth: None,
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
    fn container(container: &Container) -> Value {
        Self::items(&container.items().read())
    }
    fn sorted_shops(&self) -> Vec<(i32, Shop)> {
        let mut shops: Vec<(i32, Shop)> = self
            .tables
            .tables()
            .shops_by_id
            .iter()
            .map(|(k, v)| (*k, v.clone()))
            .collect();
        shops.sort_by_key(|(k, _)| *k);
        shops
    }
    fn shop_id(&self, shop: &Shop) -> Value {
        for (id, s) in self.sorted_shops() {
            if s.same(shop) {
                return json!(id);
            }
        }
        json!("other")
    }
    fn label(&self, c: Option<&Container>) -> Value {
        let Some(c) = c else {
            return Value::Null;
        };
        if c.same(&self.tables.general_player_stock()) {
            return json!("general");
        }
        for (id, shop) in self.sorted_shops() {
            for (key, instance) in shop.instances() {
                if instance.same(c) {
                    return json!(format!("stock:{id}:{key}"));
                }
            }
            if shop.player_stock().same(c) {
                return json!(format!("player:{id}"));
            }
        }
        for (i, p) in self.players.iter().enumerate() {
            if p.inventory.same(c) {
                return json!(format!("inv:{i}"));
            }
        }
        json!("other")
    }
    fn listener_labels(&self, c: &Container) -> Value {
        Value::Array(
            c.listeners()
                .iter()
                .map(|l| match l {
                    None => Value::Null,
                    Some(l) => {
                        if let Some(s) = l.value::<ShopListener>() {
                            let i = self.index(&s.player);
                            let uid = self.players[i].uid;
                            let current = self.tables.listener(uid).is_some_and(|x| x.same(l));
                            json!([i, current])
                        } else if let Some(r) = l.value::<Recorder>() {
                            json!(format!("inv:{}", r.0))
                        } else {
                            json!("other")
                        }
                    }
                })
                .collect(),
        )
    }
    fn sorted_map<V: Clone + serde::Serialize>(
        map: &rs09_server::world::repository::hash_map::JavaHashMap<i32, V>,
    ) -> Value {
        let mut entries: Vec<(i32, V)> = map.iter().map(|(k, v)| (*k, v.clone())).collect();
        entries.sort_by_key(|(k, _)| *k);
        Value::Array(entries.iter().map(|(k, v)| json!([k, v])).collect())
    }
    fn dump(&self, shop: &Shop, full: bool) -> Map<String, Value> {
        let mut s = Map::new();
        let general = shop
            .player_stock()
            .same(&self.tables.general_player_stock());
        if full {
            s.insert("title".into(), json!(shop.title()));
            s.insert("general".into(), json!(shop.general()));
            s.insert("currency".into(), json!(shop.currency()));
            s.insert("highAlch".into(), json!(shop.high_alch()));
            s.insert("forceShared".into(), json!(shop.force_shared()));
            let stock: Vec<Value> = shop
                .stock()
                .iter()
                .map(|i| json!([i.item_id, i.amount, i.restock_rate]))
                .collect();
            s.insert("stock".into(), Value::Array(stock));
            s.insert("sharedGeneral".into(), json!(general));
        }
        let instances: Vec<Value> = shop
            .instances()
            .iter()
            .map(|(k, c)| json!([k, Self::container(c), self.listener_labels(c)]))
            .collect();
        s.insert("instances".into(), Value::Array(instances));
        {
            let f = shop.fields();
            s.insert("needsUpdate".into(), Self::sorted_map(&f.needs_update));
            s.insert("rates".into(), Self::sorted_map(&f.restock_rates));
        }
        if !general {
            let ps = shop.player_stock();
            s.insert(
                "player".into(),
                json!([Self::container(&ps), self.listener_labels(&ps)]),
            );
        }
        s
    }
    fn dirty(&self, shop: &Shop) -> bool {
        if !shop.fields().needs_update.is_empty() {
            return true;
        }
        let shared = shops::java_hash(&self.server_name);
        for (key, c) in shop.instances() {
            if key != shared || !c.listeners().is_empty() {
                return true;
            }
        }
        let ps = shop.player_stock();
        !ps.same(&self.tables.general_player_stock())
            && (!ps.listeners().is_empty() || ps.items().read().iter().any(Option::is_some))
    }
    fn digest(&self) -> String {
        let mut b = String::new();
        let append = |b: &mut String, c: &Container| {
            for item in c.items().read().iter() {
                match item {
                    None => b.push_str("_,"),
                    Some(item) => {
                        let item = item.read();
                        b.push_str(&format!("{}x{},", item.id(), item.amount()));
                    }
                }
            }
        };
        for (id, shop) in self.sorted_shops() {
            b.push_str(&format!("#{id}|"));
            for (key, c) in shop.instances() {
                b.push_str(&format!("{key}="));
                append(&mut b, &c);
                b.push(';');
            }
            b.push('|');
            let mut updates: Vec<(i32, bool)> = shop
                .fields()
                .needs_update
                .iter()
                .map(|(k, v)| (*k, *v))
                .collect();
            updates.sort_by_key(|(k, _)| *k);
            for (k, v) in updates {
                b.push_str(&format!("{k}:{v},"));
            }
            b.push('|');
            let ps = shop.player_stock();
            if !ps.same(&self.tables.general_player_stock()) {
                append(&mut b, &ps);
            }
        }
        let mut h: u64 = 0xcbf29ce484222325;
        for byte in b.bytes() {
            h ^= u64::from(byte);
            h = h.wrapping_mul(0x100000001b3);
        }
        format!("{h:x}")
    }
    fn state(&mut self) -> Value {
        let mut players = Vec::new();
        for p in &self.players {
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
                "shop".into(),
                match p.attributes.get("shop") {
                    Some(Attr::Shop(shop)) => self.shop_id(shop),
                    None => Value::Null,
                    _ => panic!("shop attribute type"),
                },
            );
            s.insert(
                "main".into(),
                match p.attributes.get("shop-main") {
                    Some(Attr::Bool(b)) => json!(b),
                    Some(Attr::Int(v)) => json!(v),
                    None => Value::Null,
                    _ => panic!("shop-main attribute type"),
                },
            );
            s.insert(
                "cont".into(),
                match p.attributes.get("shop-cont") {
                    Some(Attr::Container(c)) => self.label(Some(c)),
                    None => Value::Null,
                    _ => panic!("shop-cont attribute type"),
                },
            );
            s.insert(
                "runscript".into(),
                json!(p.attributes.contains_key("runscript")),
            );
            s.insert(
                "parseamount".into(),
                match p.attributes.get("parseamount") {
                    Some(Attr::Bool(b)) => json!(b),
                    None => Value::Null,
                    _ => panic!("parseamount type"),
                },
            );
            s.insert(
                "inputType".into(),
                json!(p.attributes.get("input-type").map(|a| match a {
                    Attr::Type(t) => t.to_string(),
                    _ => panic!("input-type type"),
                })),
            );
            let mode = match p.ironman.mode() {
                Some(ironman::Mode::None) => "NONE",
                Some(ironman::Mode::Standard) => "STANDARD",
                Some(ironman::Mode::Hardcore) => "HARDCORE",
                Some(ironman::Mode::Ultimate) => "ULTIMATE",
                None => panic!("null ironman mode"),
            };
            s.insert("ironman".into(), json!(mode));
            players.push(Value::Object(s));
        }
        let mut dirty = Vec::new();
        for (id, shop) in self.sorted_shops() {
            if self.dirty(&shop) || self.last_dump.contains_key(&id) {
                let mut s = Map::new();
                s.insert("id".into(), json!(id));
                s.extend(self.dump(&shop, false));
                let text = serde_json::to_string(&s).unwrap();
                if self.last_dump.get(&id) != Some(&text) {
                    self.last_dump.insert(id, text);
                    dirty.push(Value::Object(s));
                }
            }
        }
        let general = self.tables.general_player_stock();
        let mut instances: Vec<(i32, usize)> = self
            .tables
            .tables()
            .listener_instances
            .iter()
            .map(|(k, v)| (*k, v.value::<ShopListener>().unwrap().player.clone()))
            .collect::<Vec<_>>()
            .into_iter()
            .map(|(k, p)| (k, self.index(&p)))
            .collect();
        instances.sort_by_key(|(k, _)| *k);
        json!({
            "players": players,
            "shops": dirty,
            "general": [Self::container(&general), self.listener_labels(&general)],
            "listeners": instances.iter().map(|(k, i)| json!([k, i])).collect::<Vec<_>>(),
            "digest": self.digest(),
            "ticks": self.ticks,
        })
    }
    fn setup_result(&self) -> Value {
        let shops: Vec<Value> = self
            .sorted_shops()
            .iter()
            .map(|(id, shop)| {
                let mut s = Map::new();
                s.insert("id".into(), json!(id));
                s.extend(self.dump(shop, true));
                Value::Object(s)
            })
            .collect();
        let mut npcs: Vec<(i32, Shop)> = self
            .tables
            .tables()
            .shops_by_npc
            .iter()
            .map(|(k, v)| (*k, v.clone()))
            .collect();
        npcs.sort_by_key(|(k, _)| *k);
        let npcs: Vec<Value> = npcs
            .iter()
            .map(|(npc, shop)| json!([npc, self.shop_id(shop)]))
            .collect();
        json!({"shops": shops, "npcs": npcs})
    }

    // ------------------------------------------------------------- actions

    fn setup(&mut self) -> R<Value> {
        for (ids, options) in listeners::NPC_LISTENERS {
            let ids = ids.map(|id| json!([id])).unwrap_or(Value::Null);
            self.emit("listen", vec![ids, json!("NPC"), json!(options)])?;
        }
        self.emit(
            "destination",
            vec![json!("NPC"), json!(listeners::DESTINATION_OPTIONS)],
        )?;
        listeners::define_interface_listeners(&self.listeners)?;
        for (name, privilege) in listeners::COMMANDS {
            self.emit("command", vec![json!(name), json!(privilege)])?;
        }
        listeners::startup(self)?;
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
        Ok(self.setup_result())
    }

    fn npc(a: &Value) -> Npc {
        let step = match a["dir"].as_str().unwrap_or("SOUTH") {
            "NORTH_WEST" => (-1, 1),
            "NORTH" => (0, 1),
            "NORTH_EAST" => (1, 1),
            "WEST" => (-1, 0),
            "EAST" => (1, 0),
            "SOUTH_WEST" => (-1, -1),
            "SOUTH" => (0, -1),
            "SOUTH_EAST" => (1, -1),
            other => panic!("direction {other}"),
        };
        let location = if a["x"].is_null() {
            Location {
                x: 3200,
                y: 3200,
                z: 0,
            }
        } else {
            Location {
                x: n(&a["x"]),
                y: n(&a["y"]),
                z: n(&a["z"]),
            }
        };
        Npc {
            id: n(&a["npc"]),
            step,
            location,
        }
    }

    fn act(&mut self, a: &Value) -> R<Value> {
        let op = a["op"].as_str().unwrap();
        let p = a
            .get("p")
            .map(|v| self.players[v.as_u64().unwrap() as usize].node.clone());
        let pi = a.get("p").map(|v| v.as_u64().unwrap() as usize);
        Ok(match op {
            "open" => {
                listeners::open_id(self, &p.unwrap(), n(&a["shop"]))?;
                Value::Null
            }
            "openView" => {
                let Some(shop) = self.tables.shop_by_id(n(&a["shop"])) else {
                    return Ok(json!("none"));
                };
                let ids = |key: &str| -> Vec<i32> {
                    a[key]
                        .as_array()
                        .map(|v| v.iter().map(n).collect())
                        .unwrap_or_default()
                };
                let view = Plugin::new(ViewObject {
                    hide: ids("hide"),
                    swap: ids("swap"),
                });
                shop_fns::open_for(self, &shop, &p.unwrap(), Some(view))?;
                Value::Null
            }
            "npc" => {
                let option = a["option"].as_str().unwrap_or("trade");
                let npc = Self::npc(a);
                let specific = listeners::NPC_LISTENERS
                    .iter()
                    .position(|(ids, options)| *ids == Some(npc.id) && options.contains(&option));
                let generic = listeners::NPC_LISTENERS
                    .iter()
                    .position(|(ids, options)| ids.is_none() && options.contains(&option));
                let which = match specific.or(generic) {
                    Some(0) => NpcListener::Generic,
                    Some(1) => NpcListener::Siegfried,
                    Some(2) => NpcListener::FurTrader,
                    Some(3) => NpcListener::CandleMaker,
                    _ => return Ok(json!("none")),
                };
                json!(listeners::npc_option(self, which, &p.unwrap(), &npc)?)
            }
            "dest" => {
                let l = listeners::destination(self, &Self::npc(a))?;
                json!([l.x, l.y, l.z])
            }
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
                    item: n(&a["item"]),
                };
                if registry.run(&mut Callbacks { host: self }, Some(&p), Some(&c), button)? {
                    return Ok(json!(true));
                }
                if plugin.is_none() {
                    return Ok(json!("unhandled"));
                }
                panic!("no component plugin is registered for the shop interfaces");
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
                for _ in 0..n(&a["n"]) {
                    match listeners::tick(self) {
                        Ok(()) => self.ticks = self.ticks.wrapping_add(1),
                        Err(e) => self.emit("tickFailure", vec![json!(category(&e))])?,
                    }
                }
                Value::Null
            }
            "setTicks" => {
                self.ticks = n(&a["value"]);
                Value::Null
            }
            "buy" | "sell" => {
                let p = p.unwrap();
                let Some(Attr::Shop(shop)) = self.player(&p).attributes.get("shop").cloned() else {
                    return Ok(json!("noshop"));
                };
                let status = if op == "buy" {
                    shop_fns::buy(self, &shop, &p, n(&a["slot"]), n(&a["amount"]))?
                } else {
                    shop_fns::sell(self, &shop, &p, n(&a["slot"]), n(&a["amount"]))?
                };
                match status {
                    Status::Success => json!("success"),
                    Status::Failure(reason) => json!(format!("failure:{reason}")),
                }
            }
            "price" => {
                let p = p.unwrap();
                let Some(Attr::Shop(shop)) = self.player(&p).attributes.get("shop").cloned() else {
                    return Ok(json!("noshop"));
                };
                if a["sell"] == true {
                    let (container, price) =
                        shop_fns::get_sell_price(self, &shop, &p, n(&a["slot"]))?;
                    let (id, amount) = {
                        let price = price.read();
                        (price.id(), price.amount())
                    };
                    json!([self.label(container.as_ref()), id, amount])
                } else {
                    let price = shop_fns::get_buy_price(self, &shop, &p, n(&a["slot"]))?;
                    let price = price.read();
                    json!([price.id(), price.amount()])
                }
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
            "ironman" => {
                self.players[pi.unwrap()]
                    .ironman
                    .set_mode(Some(mode(a["mode"].as_str().unwrap())));
                Value::Null
            }
            "attr" => {
                let key = a["key"].as_str().unwrap().to_string();
                let attrs = &mut self.players[pi.unwrap()].attributes;
                match &a["value"] {
                    Value::Null => {
                        attrs.remove(&key);
                    }
                    Value::Bool(b) => {
                        attrs.insert(key, Attr::Bool(*b));
                    }
                    value => {
                        attrs.insert(key, Attr::Int(n(value)));
                    }
                }
                Value::Null
            }
            "config" => {
                match a["personalized"].as_bool() {
                    None => self.config.remove(shops::PERSONALIZED_SHOPS),
                    Some(value) => self
                        .config
                        .insert(shops::PERSONALIZED_SHOPS.to_string(), value),
                };
                Value::Null
            }
            "close" => {
                let p = p.unwrap();
                let m = self.player(&p).manager.clone();
                json!(interfaces::close(&mut Ui::new(self, &p), &m)?)
            }
            "logout" => {
                let p = p.unwrap();
                let m = self.player(&p).manager.clone();
                interfaces::close(&mut Ui::new(self, &p), &m)?;
                interfaces::close_single_tab(&mut Ui::new(self, &p), &m)?;
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
        Error::Concurrent => "ConcurrentModificationException".into(),
        Error::Arithmetic => "ArithmeticException".into(),
        Error::Argument => "IllegalArgumentException".into(),
        Error::Parse => "ParseException".into(),
        Error::File => "FileNotFoundException".into(),
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
        other => input_prompt::Error::Binding(format!("shops:{}", other.code())),
    }
}
fn from_input(e: input_prompt::Error) -> Error {
    match e {
        input_prompt::Error::Number => Error::Number,
        input_prompt::Error::Type => Error::ClassCast,
        input_prompt::Error::Null => Error::Null,
        input_prompt::Error::Binding(text) => match text.strip_prefix("shops:") {
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
/// recorded interpreter, and the shop amount script as the callback.
struct Input<'a>(&'a mut World);
impl input_prompt::Context for Input<'_> {
    type Value = Response;
    type Script = AmountScript;
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
        shop_fns::run_amount(self.0, &script, &value).map_err(to_input)
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
        unreachable!("the shops send amount prompts only")
    }
    fn send_message(&mut self, _: usize, _: Vec<u16>) -> input_prompt::Result<()> {
        unreachable!("the shops send amount prompts only")
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
        unreachable!("bank transfer is not part of the shops")
    }
    fn bank(&mut self, _: &NodeHandle) -> input_prompt::Result<()> {
        unreachable!("bank transfer is not part of the shops")
    }
    fn take(&mut self, _: (), _: i32, _: i32) -> input_prompt::Result<()> {
        unreachable!("bank transfer is not part of the shops")
    }
    fn deposit(&mut self, _: (), _: i32, _: i32) -> input_prompt::Result<()> {
        unreachable!("bank transfer is not part of the shops")
    }
    fn last_amount(&mut self, _: (), _: i32) -> input_prompt::Result<()> {
        unreachable!("bank transfer is not part of the shops")
    }
    fn after(&mut self, _: Plugin) -> input_prompt::Result<()> {
        unreachable!("bank transfer is not part of the shops")
    }
}

fn unavailable<T>(what: &str) -> R<T> {
    Err(Error::Service(format!(
        "{what} is not reached by the shops corpus"
    )))
}

impl Host for World {
    fn world(&mut self) -> R<Tables> {
        Ok(self.tables.clone())
    }
    fn ticks(&mut self) -> R<i32> {
        Ok(self.ticks)
    }
    fn definitions(&mut self) -> R<Arc<ItemRegistry>> {
        Ok(self.definitions.clone())
    }
    fn factory(&mut self) -> R<Factory> {
        Ok(Factory {
            null_item: self.factory_null.clone(),
        })
    }
    fn config_bool(&mut self, key: &str, default: bool) -> R<bool> {
        self.emit("config", vec![json!(key)])?;
        Ok(self.config.get(key).copied().unwrap_or(default))
    }
    fn server_name(&mut self) -> R<String> {
        Ok(self.server_name.clone())
    }
    fn config_path(&mut self) -> R<Option<String>> {
        Ok(Some(format!("{CASES_DIR}/{}/", self.shops_file)))
    }
    fn stock_clear_interval(&mut self) -> R<i32> {
        Ok(self.stock_clear)
    }
    fn stock_recirculate(&mut self) -> R<bool> {
        Ok(self.recirculate)
    }
    fn read_file(&mut self, path: &str) -> R<String> {
        let expected = format!("{CASES_DIR}/{}/shops.json", self.shops_file);
        assert_eq!(path, expected, "the case reads its own shops.json");
        if self.shops_file == "pinned" {
            return Ok(PINNED.replace('\r', ""));
        }
        self.files.get(&self.shops_file).cloned().ok_or(Error::File)
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
    fn interface_listeners(&mut self) -> R<interface_listeners::Registry> {
        Ok(self.listeners.clone())
    }
    fn uid(&mut self, p: &NodeHandle) -> R<i32> {
        Ok(self.player(p).uid)
    }
    fn username(&mut self, p: &NodeHandle) -> R<String> {
        Ok(self.player(p).name.clone())
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
    fn shop_attribute(&mut self, p: &NodeHandle) -> R<Option<Shop>> {
        match self.player(p).attributes.get("shop") {
            None => Ok(None),
            Some(Attr::Shop(shop)) => Ok(Some(shop.clone())),
            Some(_) => Err(Error::ClassCast),
        }
    }
    fn set_shop_attribute(&mut self, p: &NodeHandle, shop: Shop) -> R<()> {
        self.player(p)
            .attributes
            .insert("shop".into(), Attr::Shop(shop));
        Ok(())
    }
    fn shop_container(&mut self, p: &NodeHandle) -> R<Option<Container>> {
        match self.player(p).attributes.get("shop-cont") {
            None => Ok(None),
            Some(Attr::Container(c)) => Ok(Some(c.clone())),
            Some(_) => Err(Error::ClassCast),
        }
    }
    fn set_shop_container(&mut self, p: &NodeHandle, container: Container) -> R<()> {
        self.player(p)
            .attributes
            .insert("shop-cont".into(), Attr::Container(container));
        Ok(())
    }
    fn shop_main(&mut self, p: &NodeHandle, default: bool) -> R<bool> {
        match self.player(p).attributes.get("shop-main") {
            None => Ok(default),
            Some(Attr::Bool(b)) => Ok(*b),
            Some(_) => Err(Error::ClassCast),
        }
    }
    fn set_shop_main(&mut self, p: &NodeHandle, main: bool) -> R<()> {
        self.player(p)
            .attributes
            .insert("shop-main".into(), Attr::Bool(main));
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
    fn dialogue(&mut self, p: &NodeHandle, text: &str) -> R<()> {
        let i = self.index(p);
        self.emit("dialogue", vec![json!(i), json!(text)])
    }
    fn debug(&mut self, p: &NodeHandle, text: &str) -> R<()> {
        let i = self.index(p);
        self.emit("debug", vec![json!(i), json!(text)])
    }
    fn send(&mut self, p: &NodeHandle, frame: Frame) -> R<()> {
        let i = self.index(p);
        let hex: String = frame.payload.iter().map(|b| format!("{b:02x}")).collect();
        self.emit("send", vec![json!(i), json!(frame.opcode), json!(hex)])
    }
    fn examine(&mut self, id: i32) -> R<String> {
        self.emit("examine", vec![json!(id)])?;
        Ok(self.examines.get(&id).cloned().unwrap_or_default())
    }
    fn shop_event(
        &mut self,
        p: &NodeHandle,
        kind: ShopEvent,
        item: i32,
        amount: i32,
        currency: &Item,
    ) -> R<()> {
        let i = self.index(p);
        let (id, value) = {
            let c = currency.read();
            (c.id(), c.amount())
        };
        let name = match kind {
            ShopEvent::Purchase => "purchase",
            ShopEvent::Sale => "sale",
        };
        self.emit(
            name,
            vec![
                json!(i),
                json!(item),
                json!(amount),
                json!(id),
                json!(value),
            ],
        )
    }
    fn view(&mut self, p: &NodeHandle, item: &Item, view: &Plugin) -> R<Option<Item>> {
        let i = self.index(p);
        let (id, amount) = {
            let item = item.read();
            (item.id(), item.amount())
        };
        self.emit("view", vec![json!(i), json!(id), json!(amount)])?;
        let view = view.value::<ViewObject>().expect("only case views");
        if view.hide.contains(&id) {
            return Ok(None);
        }
        if view.swap.len() == 2 && view.swap[0] == id {
            return Ok(Some(Shared::new(ItemValue::new(
                view.swap[1],
                amount,
                1000,
                &self.definitions,
            ))));
        }
        Ok(Some(item.clone()))
    }
    fn input_dialogue(&mut self, p: &NodeHandle, prompt: &str, script: AmountScript) -> R<()> {
        input_prompt::send(
            &mut Input(self),
            p,
            InputType::Amount,
            prompt.encode_utf16().collect(),
            script,
        )
        .map_err(from_input)
    }
    fn ge_offer(&mut self, id: i32, amount: i32) -> R<bool> {
        self.emit("geOffer", vec![json!(id), json!(amount)])?;
        Ok(true)
    }
    fn tanning(&mut self, p: &NodeHandle, npc: i32) -> R<()> {
        let i = self.index(p);
        self.emit("tanning", vec![json!(i), json!(npc)])
    }
    fn quest_points(&mut self, p: &NodeHandle) -> R<i32> {
        let i = self.index(p);
        self.emit("questPoints", vec![json!(i)])?;
        Ok(self.players[i].quest_points)
    }
    fn quest_stage(&mut self, p: &NodeHandle, quest: Quest) -> R<i32> {
        let i = self.index(p);
        self.emit("questStage", vec![json!(i), json!(quest.name())])?;
        Ok(self.players[i]
            .quests
            .get(quest.name())
            .copied()
            .unwrap_or(0))
    }
    fn quest_complete(&mut self, p: &NodeHandle, quest: Quest) -> R<bool> {
        let i = self.index(p);
        self.emit("questComplete", vec![json!(i), json!(quest.name())])?;
        Ok(self.players[i]
            .quests
            .get(quest.name())
            .copied()
            .unwrap_or(0)
            == 100)
    }
    fn npc_dialogue(
        &mut self,
        p: &NodeHandle,
        npc: i32,
        text: &str,
        expression: &'static str,
    ) -> R<()> {
        let i = self.index(p);
        self.emit(
            "npcDialogue",
            vec![
                json!(i),
                json!(npc),
                json!(text),
                json!(expression),
                json!(false),
            ],
        )
    }
    fn npc_facing_booth(&mut self, npc: &Npc) -> R<bool> {
        self.emit("npcAttribute", vec![json!(npc.id), json!("facing_booth")])?;
        // The case's NPC is built from the current action (see Support.npc).
        Ok(self.case_booth.unwrap_or(false))
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
        unavailable("a component plugin")
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
        let i = self.index(p);
        self.emit("varp", vec![json!(i), json!(id), json!(value)])?;
        self.send(p, rs09_protocol::game::updates::varp(id, value))
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
        world.case_booth = match &a["booth"] {
            Value::Bool(b) => Some(*b),
            _ => None,
        };
        out.push(world.step(|w| w.act(a)));
    }
    Value::Array(out)
}
