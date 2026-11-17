//! External player/content/container-result services. Ground state is native.
use super::*;
use rs09_server::inventory::{
    self,
    container::{self, Container},
    container_listener::{Handle as ListenerHandle, NativeListener},
};
use rs09_server::world::{ground_manager as gm, pickup as take, pickup_types::Types};
#[derive(Default)]
struct PlayerExtra {
    attributes: BTreeMap<String, Value>,
    inventory: usize,
    ironman: bool,
    owned: BTreeSet<i32>,
}
struct Inventory {
    space: bool,
    add: bool,
    added: Vec<[i32; 3]>,
}
impl Default for Inventory {
    fn default() -> Self {
        Self {
            space: true,
            add: true,
            added: vec![],
        }
    }
}
pub(super) struct Extra {
    players: [PlayerExtra; 2],
    inventories: [Inventory; 2],
    permitted: bool,
    plugin_result: bool,
    types: Types,
    native: Option<NativeContainers>,
}
impl Default for Extra {
    fn default() -> Self {
        Self {
            players: Default::default(),
            inventories: Default::default(),
            permitted: true,
            plugin_result: true,
            types: Types::new(&definitions::Registry::default()),
            native: None,
        }
    }
}
struct NativeContainers {
    values: Vec<Container>,
    events: Vec<(bool, Vec<usize>)>,
}
struct InventoryObserver;
impl NativeListener for InventoryObserver {}
impl container::Context for NativeContainers {
    fn update(
        &mut self,
        _: ListenerHandle,
        _: &mut Container,
        event: container::Event,
        _: &definitions::Registry,
    ) -> inventory::Result<()> {
        let e = event.read();
        self.events.push((e.clear, e.slots()));
        Ok(())
    }
    fn refresh(
        &mut self,
        _: ListenerHandle,
        _: &mut Container,
        _: &definitions::Registry,
    ) -> inventory::Result<()> {
        Err(inventory::Error::Binding(
            "no refresh expected in native pickup test".into(),
        ))
    }
    fn ground(
        &mut self,
        _: &mut Container,
        _: Option<item::Handle>,
        _: &definitions::Registry,
    ) -> inventory::Result<()> {
        Err(inventory::Error::Binding(
            "pickup must not drop inventory overflow".into(),
        ))
    }
}
fn inventory_error(e: inventory::Error) -> gm::Error {
    match e {
        inventory::Error::Null => gm::Error::Null,
        e => gm::Error::Binding(format!("{e:?}")),
    }
}
pub(super) fn initialize(m: &mut Model) {
    for id in [2412, 2413, 2414, 5509, 5510, 5511, 5512, 5513, 5514, 5515] {
        super::manager::prepare_definition(m, id);
    }
    m.pickup.types = Types::new(&m.defs);
}
pub(super) fn act(m: &mut Model, a: &Value) -> Result<Value> {
    match a["op"].as_str().unwrap() {
        "pickupSet" => {
            let p = m.p(a).ok_or(Error::Null)?;
            let i = m.pi(&p);
            let row = &mut m.pickup.players[i];
            if let Some(v) = a.get("deadline") {
                row.attributes
                    .insert(format!("droppedItem:{}", n(a, "id", 1)), v.clone());
            }
            if let Some(v) = a.get("pickup") {
                row.attributes.insert("pickup".into(), v.clone());
            }
            if a["removePickup"] == true {
                row.attributes.remove("pickup");
            }
            if let Some(v) = a.get("ironman") {
                row.ironman = v.as_bool().unwrap();
            }
            if let Some(v) = a.get("inventory") {
                row.inventory = num(v) as usize;
            }
            if let Some(v) = a.get("owned") {
                row.owned = v.as_array().unwrap().iter().map(num).collect();
            }
            if let Some(v) = a.get("name") {
                m.players[i].name = v.as_str().map(str::to_owned);
            }
            if let Some(v) = a.get("permitted") {
                m.pickup.permitted = v.as_bool().unwrap();
            }
            if let Some(v) = a.get("pluginResult") {
                m.pickup.plugin_result = v.as_bool().unwrap();
            }
        }
        "inventorySet" => {
            let i = &mut m.pickup.inventories[n(a, "inventory", 0) as usize];
            if let Some(v) = a.get("space") {
                i.space = v.as_bool().unwrap();
            }
            if let Some(v) = a.get("add") {
                i.add = v.as_bool().unwrap();
            }
        }
        "capeSet" => m.pickup.types.capes[n(a, "index", 0) as usize]
            .write()
            .set_id(n(a, "id", 1), &m.defs),
        "pouchSet" => m.pickup.types.pouches[n(a, "index", 0) as usize]
            [usize::from(a["decayed"] == true)]
        .write()
        .set_id(n(a, "id", 1), &m.defs),
        "take" | "canTake" | "pickupHandle" => {
            let p = m.p(a);
            let g = if a["g"].is_null() {
                None
            } else {
                Some(m.g(a)?)
            };
            let manager = m.extra.manager.clone();
            let r = match a["op"].as_str().unwrap() {
                "take" => take::take(m, &manager, p.as_ref(), g.as_ref()),
                "canTake" => take::can_take(m, p.as_ref(), g.as_ref(), n(a, "type", 0)),
                _ => take::Program.handle(m, &manager, p.as_ref(), g.as_ref()),
            };
            return r.map(|v| json!(v)).map_err(super::manager::error);
        }
        "pickupDestination" => {
            return Ok(json!(take::plugin_destination().map(|l| l.coordinates())));
        }
        "pickupRegister" => {
            return take::register(m, 0)
                .map(|h| json!(h == 0))
                .map_err(super::manager::error);
        }
        "fail" => return Err(Error::Binding("Failure".into())),
        _ => return super::ground_action::act(m, a),
    }
    Ok(Value::Null)
}
pub(super) fn snapshot(m: &Model, s: &mut Value) {
    s["pickupPlayers"]=json!(m.pickup.players.iter().enumerate().map(|(i,p)|json!({"attributes":p.attributes,"inventory":p.inventory,"ironman":p.ironman,"owned":p.owned,"name":m.players[i].name})).collect::<Vec<_>>());
    s["pickupInventories"] = json!(
        m.pickup
            .inventories
            .iter()
            .map(|i| json!([i.space, i.add, i.added]))
            .collect::<Vec<_>>()
    );
    s["pickupConfig"] = json!([
        m.pickup.permitted,
        m.pickup.plugin_result,
        m.pickup.types.capes.each_ref().map(|i| i.read().id()),
        m.pickup
            .types
            .pouches
            .each_ref()
            .map(|p| p.each_ref().map(|i| i.read().id()))
    ]);
}
fn error(e: Error) -> gm::Error {
    match e {
        Error::Null => gm::Error::Null,
        Error::Ground(e) => gm::Error::Ground(e),
        Error::Binding(s) => gm::Error::Binding(s),
    }
}
fn raw(i: &item::Handle) -> [i32; 3] {
    let i = i.read();
    [i.id(), i.amount(), i.charge()]
}
fn gid(m: &Model, g: &Ground) -> i32 {
    m.gid(g) as i32
}
fn iid(g: &Ground) -> i32 {
    g.read().item.read().id()
}
fn has(m: &mut Model, p: &NodeHandle, id: i32) -> gm::Result<bool> {
    let i = m.pi(p);
    let v = m.pickup.players[i].owned.contains(&id);
    m.emit("hasItem", vec![json!(i), json!(id), json!(v)])
        .map_err(error)?;
    Ok(v)
}
impl take::Registration for Model {
    type Handler = usize;
    fn set_option_handler(&mut self, name: &str, handler: usize) -> gm::Result<()> {
        self.emit("registerPickup", vec![json!(name), json!(handler == 0)])
            .map_err(error)
    }
}
impl take::Context for Model {
    type Inventory = usize;
    fn inventory(&mut self, p: &NodeHandle) -> gm::Result<usize> {
        let i = self.pi(p);
        let v = self.pickup.players[i].inventory;
        self.emit("inventory", vec![json!(i), json!(v)])
            .map_err(error)?;
        Ok(v)
    }
    fn has_space(&mut self, inventory: usize, item: &item::Handle) -> gm::Result<bool> {
        let v = if let Some(n) = &self.pickup.native {
            n.values[inventory]
                .has_space_for(Some(item))
                .map_err(inventory_error)?
        } else {
            self.pickup.inventories[inventory].space
        };
        self.emit("space", vec![json!(inventory), json!(raw(item)), json!(v)])
            .map_err(error)?;
        Ok(v)
    }
    fn add(&mut self, inventory: usize, item: &item::Handle) -> gm::Result<bool> {
        let v = if let Some(mut native) = self.pickup.native.take() {
            // The adapter lends the actual mutable Container to its callbacks;
            // its recorded observer has no world reentry services.
            let mut target = native.values.remove(inventory);
            let result = target.add(Some(item), true, -1, &self.defs, &mut native);
            native.values.insert(inventory, target);
            self.pickup.native = Some(native);
            result.map_err(inventory_error)?
        } else {
            self.pickup.inventories[inventory].add
        };
        let raw = raw(item);
        if v {
            self.pickup.inventories[inventory].added.push(raw);
        }
        self.emit("addInventory", vec![json!(inventory), json!(raw), json!(v)])
            .map_err(error)?;
        Ok(v)
    }
    fn drop_deadline(&mut self, p: &NodeHandle, key: &str) -> gm::Result<i32> {
        let i = self.pi(p);
        let v = self.pickup.players[i]
            .attributes
            .get(key)
            .filter(|v| !v.is_null())
            .cloned()
            .unwrap_or(json!(0));
        self.emit("deadline", vec![json!(i), json!(key), v.clone()])
            .map_err(error)?;
        v.as_i64()
            .map(|v| v as i32)
            .ok_or_else(|| gm::Error::Binding("ClassCastException".into()))
    }
    fn is_spawn(&mut self, g: &Ground) -> gm::Result<bool> {
        Ok(self.spawns.registry.get(g).is_some())
    }
    fn ironman_restriction(&mut self, p: &NodeHandle) -> gm::Result<bool> {
        let i = self.pi(p);
        let v = self.pickup.players[i].ironman;
        self.emit("ironman", vec![json!(i), json!(v)])
            .map_err(error)?;
        Ok(v)
    }
    fn god_cape(&mut self, g: &Ground) -> gm::Result<bool> {
        Ok(self.pickup.types.cape(&ground::as_item(g)).is_some())
    }
    fn has_god_cape(&mut self, p: &NodeHandle) -> gm::Result<bool> {
        let types = self.pickup.types.clone();
        types.has_cape(|i| {
            let id = i.read().id();
            has(self, p, id)
        })
    }
    fn rune_pouch(&mut self, g: &Ground) -> gm::Result<bool> {
        Ok(self.pickup.types.pouch(&ground::as_item(g)).is_some())
    }
    fn has_item(&mut self, p: &NodeHandle, g: &Ground) -> gm::Result<bool> {
        has(self, p, iid(g))
    }
    fn plugin_pickup(
        &mut self,
        plugin: i32,
        p: &NodeHandle,
        g: &Ground,
        kind: i32,
    ) -> gm::Result<bool> {
        let v = self.pickup.plugin_result;
        self.emit(
            "canPickUp",
            vec![
                json!(plugin),
                json!(self.pi(p)),
                json!(gid(self, g)),
                json!(kind),
                json!(v),
            ],
        )
        .map_err(error)?;
        Ok(v)
    }
    fn packet_message(&mut self, p: &NodeHandle, text: &str) -> gm::Result<()> {
        self.emit("packetMessage", vec![json!(self.pi(p)), json!(text)])
            .map_err(error)
    }
    fn message(&mut self, p: &NodeHandle, text: &str) -> gm::Result<()> {
        self.emit("message", vec![json!(self.pi(p)), json!(text)])
            .map_err(error)
    }
    fn messages(&mut self, p: &NodeHandle, text: &[&str]) -> gm::Result<()> {
        self.emit("messages", vec![json!(self.pi(p)), json!(text)])
            .map_err(error)
    }
    fn guild_dialogue(&mut self, p: &NodeHandle) -> gm::Result<()> {
        self.emit(
            "dialogue",
            vec![
                json!(self.pi(p)),
                json!(4300),
                json!("FURIOUS"),
                json!([
                    "Hey! You can't take that, it's guild property. Take one",
                    "from the pile."
                ]),
            ],
        )
        .map_err(error)
    }
    fn item_name(&mut self, id: i32) -> gm::Result<String> {
        super::manager::prepare_definition(self, id);
        let v = self.defs.for_id(id).read().name.clone();
        self.emit("itemName", vec![json!(id)]).map_err(error)?;
        v.ok_or(gm::Error::Null)
    }
    fn player_name(&mut self, p: &NodeHandle) -> gm::Result<Option<String>> {
        let i = self.pi(p);
        let v = self.players[i].name.clone();
        self.emit("playerName", vec![json!(i)]).map_err(error)?;
        Ok(v)
    }
    fn drop_trade_log(&mut self, p: Option<NodeHandle>, text: &str) -> gm::Result<()> {
        self.emit(
            "dropLog",
            vec![
                json!(p.as_ref().map(|p| self.pi(p))),
                json!("DROP_TRADE"),
                json!(text),
            ],
        )
        .map_err(error)
    }
    fn teleport_permitted(&mut self, l: Option<Location>) -> gm::Result<bool> {
        let v = self.pickup.permitted;
        self.emit(
            "permitted",
            vec![json!(l.map(|l| l.coordinates())), json!(v)],
        )
        .map_err(error)?;
        Ok(v)
    }
    fn animate(&mut self, p: &NodeHandle, id: i32) -> gm::Result<()> {
        self.emit("animate", vec![json!(self.pi(p)), json!(id)])
            .map_err(error)
    }
    fn audio(&mut self, p: &NodeHandle, id: i32) -> gm::Result<()> {
        self.emit("audio", vec![json!(self.pi(p)), json!(id)])
            .map_err(error)
    }
    fn dispatch(&mut self, p: &NodeHandle, id: i32) -> gm::Result<()> {
        self.emit("dispatch", vec![json!(self.pi(p)), json!(id)])
            .map_err(error)
    }
    fn pickup_attribute(&mut self, p: &NodeHandle) -> gm::Result<bool> {
        Ok(self.pickup.players[self.pi(p)]
            .attributes
            .contains_key("pickup"))
    }
    fn remove_pickup_attribute(&mut self, p: &NodeHandle) -> gm::Result<()> {
        let i = self.pi(p);
        self.pickup.players[i].attributes.remove("pickup");
        self.emit("removeAttribute", vec![json!(i), json!("pickup")])
            .map_err(error)
    }
}

#[cfg(test)]
mod container_checks {
    use super::*;
    use rs09_server::inventory::container::{ContainerType, Factory, SortType};
    fn native(capacity: i32, stackable: bool, input: Value) -> Model {
        let mut m = super::super::model(&input);
        super::super::manager::prepare_definition(&mut m, 2);
        m.defs.for_id(2).write().stackable = stackable;
        let f = Factory::new(&m.defs);
        let mut values: Vec<_> = (0..2)
            .map(|_| {
                f.create(capacity, ContainerType::Default, SortType::Id)
                    .unwrap()
            })
            .collect();
        for c in &mut values {
            c.register(Some(ListenerHandle::new(InventoryObserver)));
        }
        m.pickup.native = Some(NativeContainers {
            values,
            events: vec![],
        });
        m
    }
    #[test]
    fn original_take_transfers_to_real_container_and_respawns_the_same_world_item() {
        let mut m = native(2, true, json!({"manager":true,"spawns":true,"pickup":true}));
        m.act(&json!({"op":"spawnNew","id":2,"amount":3,"charge":500,"rate":2,"location":[3201,3202,0]})).unwrap();
        m.act(&json!({"op":"spawnInit"})).unwrap();
        let g = m.grounds[0].clone();
        let original_item = g.read().item.clone();
        assert_eq!(m.act(&json!({"op":"take","g":0})).unwrap(), true);
        let c = &m.pickup.native.as_ref().unwrap().values[0];
        let held = c.items().read()[0].clone().unwrap();
        assert_eq!(raw(&held), [2, 3, 500]);
        assert!(!held.same(&original_item));
        assert_eq!(m.pickup.native.as_ref().unwrap().events, [(false, vec![0])]);
        assert!(m.extra.manager.get_items().read().is_empty());
        assert!(
            !g.read().removed,
            "destroy removes membership but does not set the removed flag"
        );
        assert_eq!(m.spawns.runners[0].current().len(), 1);
        m.act(&json!({"op":"pickupSet","p":1,"inventory":1}))
            .unwrap();
        m.act(&json!({"op":"take","p":1,"g":0})).unwrap();
        assert_eq!(
            m.pickup.native.as_ref().unwrap().events.len(),
            1,
            "a second actor cannot duplicate an item no longer in the manager"
        );
        assert!(
            m.pickup.native.as_ref().unwrap().values[1]
                .items()
                .read()
                .iter()
                .all(Option::is_none)
        );
        m.act(&json!({"op":"spawnWorld"})).unwrap();
        m.act(&json!({"op":"spawnWorld"})).unwrap();
        assert!(
            m.extra
                .manager
                .get_items()
                .read()
                .get(0)
                .unwrap()
                .as_ref()
                .unwrap()
                .same(&g)
        );
        assert!(!g.read().removed);
        assert_eq!(raw(&held), [2, 3, 500]);
    }
    #[test]
    fn real_container_space_checks_and_failures_after_add_keep_the_original_prefix() {
        let mut full = native(
            2,
            false,
            json!({"manager":true,"spawns":true,"pickup":true}),
        );
        full.act(&json!({"op":"spawnNew","id":2,"amount":3,"rate":2,"location":[3201,3202,0]}))
            .unwrap();
        full.act(&json!({"op":"spawnInit"})).unwrap();
        full.act(&json!({"op":"take","g":0})).unwrap();
        assert!(
            full.pickup.native.as_ref().unwrap().values[0]
                .items()
                .read()
                .iter()
                .all(Option::is_none)
        );
        assert!(!full.grounds[0].read().removed);
        assert!(full.spawns.runners[0].current().is_empty());
        let mut failed = native(
            2,
            true,
            json!({"manager":true,"spawns":true,"pickup":true,"failures":{"permitted":1}}),
        );
        failed
            .act(&json!({"op":"spawnNew","id":2,"amount":3,"rate":2,"location":[3201,3202,0]}))
            .unwrap();
        failed.act(&json!({"op":"spawnInit"})).unwrap();
        assert!(failed.act(&json!({"op":"pickupHandle","g":0})).is_err());
        let c = &failed.pickup.native.as_ref().unwrap().values[0];
        assert_eq!(raw(c.items().read()[0].as_ref().unwrap()), [2, 3, 1000]);
        assert!(!failed.grounds[0].read().removed);
        assert_eq!(failed.extra.manager.get_items().read().len(), 1);
        assert!(
            !failed
                .events
                .iter()
                .any(|v| v[0] == "audio" || v[0] == "dispatch" || v[0] == "removeAttribute")
        );
    }
}
