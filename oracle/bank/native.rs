//! Test-only adapters for the complete native BankContainer implementation.
use rs09_server::{
    definitions::item::{ItemDefinition, Registry, Shared},
    inventory::{
        Error, Result,
        bank::{self, Bank, CloseEvent, Packet},
        container::{self, Container, ContainerType, Event, Factory, Items, SortType},
        container_listener::{self, NativeListener},
        item::{Handle, Item},
    },
    world::{
        doors::Location,
        player_walk::{NodeHandle, State},
    },
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, sync::Arc};
fn n(v: &Value, key: &str, default: i32) -> i32 {
    v[key].as_i64().map_or(default, |v| v as i32)
}
fn hex(v: &[u8]) -> String {
    v.iter().map(|v| format!("{v:02x}")).collect()
}
struct Recording(i32);
impl NativeListener for Recording {}
struct Context {
    row: Value,
    owners: Vec<NodeHandle>,
    inventories: Vec<Container>,
    equipment: Vec<Container>,
    banks: Vec<Bank>,
    active: Vec<usize>,
    defs: Arc<Registry>,
    null: Handle,
    named: BTreeMap<String, Option<Handle>>,
    log: Vec<Value>,
    frames: Vec<Value>,
    packets: Vec<Packet>,
    arrays: Vec<Items>,
    closes: Vec<CloseEvent>,
    hits: BTreeMap<String, i32>,
    vars: Vec<BTreeMap<String, i32>>,
    attributes: Vec<BTreeMap<String, i32>>,
}
impl Context {
    fn player(&self, p: &NodeHandle) -> usize {
        self.owners.iter().position(|v| v == p).unwrap()
    }
    fn hit(&mut self, key: &str, args: Vec<Value>) -> Result<()> {
        let mut entry = vec![json!(key)];
        entry.extend(args);
        self.log.push(json!(entry));
        let count = self.hits.entry(key.into()).or_default();
        *count += 1;
        let count = *count;
        for hook in self.row["hooks"].as_array().unwrap().clone() {
            if hook["when"] != key || n(&hook, "at", 1) != count {
                continue;
            }
            match hook["action"].as_str().unwrap() {
                "fail" => return Err(Error::Binding("controlled".into())),
                "swapBank" => self.active[0] = 1 - self.active[0],
                "clearInventory" => self.inventories[0].items().write().fill(None),
                "clearBank" => self.banks[0].container().items().write().fill(None),
                "replaceInventory" => {
                    self.inventories[0] = Factory {
                        null_item: self.null.clone(),
                    }
                    .create(28, ContainerType::Default, SortType::Id)?
                }
                "amount" => self.named[hook["item"].as_str().unwrap_or("a")]
                    .as_ref()
                    .unwrap()
                    .write()
                    .set_amount(n(&hook, "value", 7)),
                "event" => self.banks[0]
                    .container()
                    .event()
                    .read()
                    .flag(n(&hook, "slot", 0), self.named["a"].clone())?,
                "reenter" => {
                    let bank = self.banks[0].clone();
                    let defs = self.defs.clone();
                    bank.container().refresh(None, &defs, self)?;
                }
                "close" => self.banks[0].clone().close(self)?,
                action => panic!("unknown hook {action}"),
            }
        }
        Ok(())
    }
    fn action(&mut self, a: &Value) -> Result<Value> {
        let bank = self.banks[n(a, "bank", 0) as usize].clone();
        let defs = self.defs.clone();
        let slot = n(a, "slot", 0);
        let amount = n(a, "amount", 1);
        let value = n(a, "value", 0);
        let item = self
            .named
            .get(a["item"].as_str().unwrap_or("a"))
            .cloned()
            .flatten();
        match a["op"].as_str().unwrap() {
            "hasItem" => {
                return Ok(json!(rs09_server::player::item_ownership::has_item(
                    item.as_ref(),
                    &mut Ownership(self, n(a, "player", 0) as usize)
                )?));
            }
            "open" => bank.open_bank(self)?,
            "openFor" => {
                let p = n(a, "player", 0);
                let p = (p >= 0).then(|| self.owners[p as usize].clone());
                bank.open_for(p.as_ref(), self)?;
            }
            "close" => bank.close(self)?,
            "depositBox" => bank.open_deposit_box(self)?,
            "refreshDeposit" => bank.refresh_deposit_box(self)?,
            "deposit" => bank.add_item(slot, amount, self)?,
            "withdraw" => bank.take_item(slot, amount, self)?,
            "last" => bank.update_last_amount_x(value, self)?,
            "tab" => bank.set_tab_index(value, self)?,
            "increase" => bank.increase_tab_start_slots(value)?,
            "decrease" => bank.decrease_tab_start_slots(value, self)?,
            "collapse" => bank.collapse_tab(value, self)?,
            "itemsInTab" => return Ok(json!(bank.items_in_tab(value)?)),
            "tabBySlot" => return Ok(json!(bank.tab_by_item_slot(slot))),
            "arrayIndex" => return Ok(json!(bank::array_index(value))),
            "space" => bank.send_bank_space(self)?,
            "configs" => bank.set_tab_configurations(self)?,
            "canAdd" => return Ok(json!(bank::can_add(item.as_ref())?)),
            "maximum" => return Ok(json!(bank.container().maximum_add(item.as_ref())?)),
            "note" => bank.set_note_items(a["value"] == true, self)?,
            "insert" => bank.set_insert_items(a["value"] == true, self)?,
            "modes" => {
                return Ok(json!([
                    bank.is_note_items(self)?,
                    bank.is_insert_items(self)?
                ]));
            }
            "refresh" => bank.container().refresh(None, &defs, self)?,
            "update" => bank.container().update(None, &defs, self)?,
            "baseAdd" => {
                return Ok(json!(bank.container().add(
                    item.as_ref(),
                    a["fire"].as_bool().unwrap_or(true),
                    n(a, "slot", -1),
                    &defs,
                    self
                )?));
            }
            "clearListeners" => bank.container().listener_list().clear(),
            "array" => self.arrays.push(bank.container().items()),
            "clear" => bank.container().clear(false, &defs, self)?,
            "closeCallback" => {
                let event = self
                    .closes
                    .get(value as usize)
                    .cloned()
                    .ok_or(Error::Bounds)?;
                let player = self.owners[n(a, "player", 0) as usize].clone();
                self.hit("component", vec![json!(0)])?;
                return Ok(json!(event.close(Some(&player), self)?));
            }
            op => panic!("unknown action {op}"),
        }
        Ok(Value::Null)
    }
}
struct Ownership<'a>(&'a mut Context, usize);
impl rs09_server::player::item_ownership::Containers for Ownership<'_> {
    fn inventory(&mut self) -> Result<Container> {
        let owner = self.0.owners[self.1].clone();
        bank::Context::inventory(self.0, &owner)
    }
    fn bank(&mut self) -> Result<Container> {
        let owner = self.0.owners[self.1].clone();
        Ok(bank::Context::bank(self.0, &owner)?.container())
    }
    fn equipment(&mut self) -> Result<Container> {
        self.0.hit("equipment", vec![json!(self.1)])?;
        Ok(self.0.equipment[self.1].clone())
    }
}
impl container::Context for Context {
    fn update(
        &mut self,
        l: container_listener::Handle,
        c: &mut Container,
        e: Event,
        _: &Registry,
    ) -> Result<()> {
        if let Some(l) = l.value::<bank::Listener>() {
            l.update(c, Some(&e), self)
        } else {
            let l = l.value::<Recording>().ok_or(Error::Type)?;
            self.hit("inventoryUpdate", vec![json!(l.0)])
        }
    }
    fn refresh(
        &mut self,
        l: container_listener::Handle,
        c: &mut Container,
        _: &Registry,
    ) -> Result<()> {
        if let Some(l) = l.value::<bank::Listener>() {
            l.refresh(c, self)
        } else {
            let l = l.value::<Recording>().ok_or(Error::Type)?;
            self.hit("inventoryRefresh", vec![json!(l.0)])
        }
    }
    fn ground(&mut self, _: &mut Container, _: Option<Handle>, _: &Registry) -> Result<()> {
        Err(Error::Binding("ground outside bank oracle".into()))
    }
}
impl bank::Context for Context {
    type Ironman = usize;
    type Pin = usize;
    type Interfaces = usize;
    type Component = i32;
    type Dispatch = usize;
    fn inventory(&mut self, p: &NodeHandle) -> Result<Container> {
        let p = self.player(p);
        self.hit("inventory", vec![json!(p)])?;
        Ok(self.inventories[p].clone())
    }
    fn bank(&mut self, p: &NodeHandle) -> Result<Bank> {
        let p = self.player(p);
        self.hit("bank", vec![json!(p)])?;
        Ok(self.banks[self.active[p]].clone())
    }
    fn ironman(&mut self, p: &NodeHandle) -> Result<usize> {
        let p = self.player(p);
        self.hit("ironman", vec![json!(p)])?;
        Ok(p)
    }
    fn ultimate_restriction(&mut self, p: usize) -> Result<bool> {
        self.hit("restriction", vec![json!(p)])?;
        Ok(self.row["restricted"] == true)
    }
    fn pin(&mut self, p: &NodeHandle) -> Result<usize> {
        let p = self.player(p);
        self.hit("pin", vec![json!(p)])?;
        Ok(p)
    }
    fn unlocked(&mut self, p: usize) -> Result<bool> {
        self.hit("unlocked", vec![json!(p)])?;
        Ok(self.row["locked"] != true)
    }
    fn dev_mode(&mut self) -> Result<bool> {
        self.hit("dev", vec![])?;
        Ok(self.row["dev"] == true)
    }
    fn open_pin(&mut self, p: usize, kind: i32) -> Result<()> {
        self.hit("openPin", vec![json!(p), json!(kind)])
    }
    fn interfaces(&mut self, p: &NodeHandle) -> Result<usize> {
        let p = self.player(p);
        self.hit("interfaces", vec![json!(p)])?;
        Ok(p)
    }
    fn component(&mut self, id: i32) -> Result<i32> {
        self.hit("component", vec![json!(id)])?;
        Ok(id)
    }
    fn open(&mut self, p: usize, id: i32) -> Result<i32> {
        self.hit("open", vec![json!(p), json!(id)])?;
        Ok(id)
    }
    fn open_component(&mut self, p: usize, id: i32) -> Result<i32> {
        self.hit("openComponent", vec![json!(p), json!(id)])?;
        self.hit("component", vec![json!(id)])?;
        Ok(id)
    }
    fn close_event(&mut self, id: i32, event: CloseEvent) -> Result<()> {
        self.hit("closeEvent", vec![json!(id)])?;
        self.closes.push(event);
        Ok(())
    }
    fn remove_tabs(&mut self, p: usize, tabs: &[i32]) -> Result<()> {
        self.hit("removeTabs", vec![json!(p), json!(tabs)])
    }
    fn open_default_tabs(&mut self, p: usize) -> Result<()> {
        self.hit("defaultTabs", vec![json!(p)])
    }
    fn open_single_tab(&mut self, p: usize, id: i32) -> Result<()> {
        self.hit("singleTab", vec![json!(p), json!(id)])
    }
    fn close_single_tab(&mut self, p: usize) -> Result<()> {
        self.hit("closeSingle", vec![json!(p)])
    }
    fn dispatch(&mut self, p: &NodeHandle) -> Result<usize> {
        let p = self.player(p);
        self.hit("dispatch", vec![json!(p)])?;
        Ok(p)
    }
    fn iface_settings(&mut self, p: usize, fields: [i32; 5]) -> Result<()> {
        let mut args = vec![json!(p)];
        args.extend(fields.map(|v| json!(v)));
        self.hit("ifaceSettings", args)
    }
    fn script(&mut self, p: usize, id: i32) -> Result<()> {
        self.hit("script", vec![json!(p), json!(id)])
    }
    fn message(&mut self, p: &NodeHandle, text: &str) -> Result<()> {
        let p = self.player(p);
        self.hit("message", vec![json!(p), json!(text)])
    }
    fn packet_message(&mut self, p: usize, text: &str) -> Result<()> {
        self.hit("packetMessage", vec![json!(p), json!(text)])
    }
    fn deposit_items(
        &mut self,
        p: &NodeHandle,
        _: Items,
        options: &[&str],
        fields: [i32; 4],
    ) -> Result<()> {
        let p = self.player(p);
        let mut args = vec![json!(p), json!(options)];
        args.extend(fields.map(|v| json!(v)));
        self.hit("depositItems", args)
    }
    fn get_varbit(&mut self, p: &NodeHandle, id: i32) -> Result<i32> {
        let p = self.player(p);
        self.hit("getVarbit", vec![json!(p), json!(id)])?;
        Ok(self.vars[p].get(&id.to_string()).copied().unwrap_or(0))
    }
    fn set_varbit(&mut self, p: &NodeHandle, id: i32, value: i32, send: bool) -> Result<()> {
        let p = self.player(p);
        self.hit(
            "varbit",
            vec![json!(p), json!(id), json!(value), json!(send)],
        )?;
        self.vars[p].insert(id.to_string(), value);
        Ok(())
    }
    fn set_varp(&mut self, p: &NodeHandle, id: i32, value: i32) -> Result<()> {
        let p = self.player(p);
        self.hit("varp", vec![json!(p), json!(id), json!(value)])
    }
    fn set_varc(&mut self, p: &NodeHandle, id: i32, value: i32) -> Result<()> {
        let p = self.player(p);
        self.hit("varc", vec![json!(p), json!(id), json!(value)])
    }
    fn set_attribute(&mut self, p: &NodeHandle, key: &str, value: i32) -> Result<()> {
        let p = self.player(p);
        self.hit("attribute", vec![json!(p), json!(key), json!(value)])?;
        self.attributes[p].insert(key.into(), value);
        Ok(())
    }
    fn remove_attribute(&mut self, p: &NodeHandle, key: &str) -> Result<()> {
        let p = self.player(p);
        self.hit("removeAttribute", vec![json!(p), json!(key)])?;
        self.attributes[p].remove(key);
        Ok(())
    }
    fn packet(&mut self, p: Packet) -> Result<()> {
        let owner = p.player.as_ref().map_or(-1, |p| self.player(p) as i32);
        self.hit(
            "packet",
            vec![
                json!(owner),
                json!(if p.bank { 762 } else { 763 }),
                json!(64000),
                json!(if p.bank { 95 } else { 93 }),
            ],
        )?;
        self.packets.push(p);
        let packet = self.packets.last().unwrap();
        let frame = packet.frame(&self.null)?;
        if packet.player.is_none() {
            return Err(Error::Null);
        }
        self.frames.push(json!([frame.opcode, hex(&frame.payload)]));
        Ok(())
    }
}
type Ids = BTreeMap<usize, usize>;
fn tag(value: usize, ids: &mut Ids) -> usize {
    let next = ids.len();
    *ids.entry(value).or_insert(next)
}
fn item(i: Option<&Handle>, ids: &mut Ids) -> Value {
    i.map_or(Value::Null, |i| {
        let id = tag(i.identity(), ids);
        let i = i.read();
        json!([id, i.id(), i.amount(), i.charge(), i.index])
    })
}
fn array(a: &Items, ids: &mut Ids) -> Value {
    let reference = tag(a.identity(), ids);
    let a = a.read();
    let mut slots = BTreeMap::new();
    for (index, i) in a.iter().enumerate() {
        if i.is_some() {
            slots.insert(index.to_string(), item(i.as_ref(), ids));
        }
    }
    json!({"ref":reference,"length":a.len(),"slots":slots})
}
fn container(c: &Container, banks: &[Bank], ids: &mut Ids) -> Value {
    let items = array(&c.items(), ids);
    let event = c.event();
    let reference = tag(event.identity(), ids);
    let event = event.read();
    let event_items = array(&event.items, ids);
    let listeners: Vec<_> = c
        .listeners()
        .iter()
        .map(|l| {
            l.as_ref().map_or(Value::Null, |l| {
                if let Some(r) = l.value::<Recording>() {
                    json!(format!("inv{}", r.0))
                } else {
                    json!(
                        banks
                            .iter()
                            .position(|b| b.listener().same(l))
                            .map_or("unknown".into(), |i| format!("bank{i}"))
                    )
                }
            })
        })
        .collect();
    json!({"items":items,"event":{"ref":reference,"items":event_items,"clear":event.clear},"listeners":listeners})
}
impl Context {
    fn state(&self) -> Value {
        let mut ids = Ids::new();
        let mut banks = vec![];
        for b in &self.banks {
            let mut state = container(&b.container(), &self.banks, &mut ids);
            state["open"] = json!(b.is_open());
            state["last"] = json!(b.last_amount_x());
            state["tab"] = json!(b.tab_index());
            state["tabs"] = json!(*b.tab_start_slots().read());
            banks.push(state);
        }
        let inventories: Vec<_> = self
            .inventories
            .iter()
            .map(|c| container(c, &self.banks, &mut ids))
            .collect();
        let named: BTreeMap<_, _> = self
            .named
            .iter()
            .map(|(name, i)| (name, item(i.as_ref(), &mut ids)))
            .collect();
        let arrays: Vec<_> = self.arrays.iter().map(|a| array(a, &mut ids)).collect();
        let packets: Vec<_> = self
            .packets
            .iter()
            .map(|p| {
                let items = array(&p.items, &mut ids);
                json!({"items":items,"slots":p.slots.clone().unwrap_or_default(),"length":p.length})
            })
            .collect();
        let mut result = json!({"banks":banks,"inventories":inventories,"named":named,"arrays":arrays,"packets":packets,"active":self.active,"vars":self.vars,"attributes":self.attributes,"closes":self.closes.len()});
        if self.row["ownership"] == true {
            result["equipment"] = json!(
                self.equipment
                    .iter()
                    .map(|c| container(c, &self.banks, &mut ids))
                    .collect::<Vec<_>>()
            );
        }
        result
    }
}
pub fn evaluate(row: &Value) -> Value {
    let mut defs = Registry::default();
    for (id, value) in row["definitions"].as_object().unwrap() {
        let definition: ItemDefinition = serde_json::from_value(value.clone()).unwrap();
        defs.insert(id.parse().unwrap(), Shared::new(definition));
    }
    let defs = Arc::new(defs);
    let factory = Factory::new(&defs);
    let mut state = State::default();
    let mut owners = vec![];
    for index in 1..=2 {
        let node = state.add(index, Location::new(3200, 3200, 0), [0; 4]);
        owners.push(state.retain_node(node).unwrap());
    }
    let mut named = BTreeMap::new();
    for (name, value) in row["items"].as_object().unwrap() {
        named.insert(
            name.clone(),
            (!value.is_null()).then(|| {
                Shared::new(Item::new(
                    n(value, "id", 0),
                    n(value, "amount", 1),
                    n(value, "charge", 1000),
                    &defs,
                ))
            }),
        );
    }
    let banks = vec![
        Bank::new(
            &factory,
            defs.clone(),
            (row["nullPlayer"] != true).then(|| owners[0].clone()),
        )
        .unwrap(),
        Bank::new(&factory, defs.clone(), Some(owners[0].clone())).unwrap(),
        Bank::new(&factory, defs.clone(), Some(owners[1].clone())).unwrap(),
    ];
    let mut inventories = vec![];
    for i in 0..2 {
        let mut c = factory
            .create(28, ContainerType::Default, SortType::Id)
            .unwrap();
        c.register(Some(container_listener::Handle::new(Recording(i as i32))));
        for (slot, name) in row["inventories"][i].as_object().unwrap() {
            c.items().write()[slot.parse::<usize>().unwrap()] =
                named[name.as_str().unwrap()].clone();
        }
        inventories.push(c);
    }
    for (i, b) in banks.iter().enumerate() {
        for (slot, name) in row["banks"][i].as_object().unwrap() {
            b.container().items().write()[slot.parse::<usize>().unwrap()] =
                named[name.as_str().unwrap()].clone();
        }
        *b.tab_start_slots().write() = row["tabs"][i]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_i64().unwrap() as i32)
            .collect();
    }
    let mut equipment = vec![];
    for i in 0..2 {
        let c = factory
            .create(14, ContainerType::Default, SortType::Id)
            .unwrap();
        if row["ownership"] == true {
            for (slot, name) in row["equipment"][i].as_object().unwrap() {
                c.items().write()[slot.parse::<usize>().unwrap()] =
                    named[name.as_str().unwrap()].clone();
            }
        }
        equipment.push(c);
    }
    let mut ctx = Context {
        row: row.clone(),
        owners,
        inventories,
        equipment,
        banks,
        active: vec![n(row, "activeBank", 0) as usize, 2],
        defs,
        null: factory.null_item,
        named,
        log: vec![],
        frames: vec![],
        packets: vec![],
        arrays: vec![],
        closes: vec![],
        hits: BTreeMap::new(),
        vars: vec![BTreeMap::new(); 2],
        attributes: vec![BTreeMap::new(); 2],
    };
    if row["noteMode"] == true {
        ctx.vars[0].insert("7001".into(), 1);
    }
    let mut output = vec![];
    for a in row["actions"].as_array().unwrap() {
        let result = ctx.action(a);
        let (result, error) = match result {
            Ok(result) => (result, Value::Null),
            Err(error) => (
                Value::Null,
                json!(match error {
                    Error::Null => "Null",
                    Error::Bounds | Error::ListBounds => "Bounds",
                    Error::Type => "Type",
                    Error::ConcurrentModification => "Concurrent",
                    Error::Binding(_) => "Binding",
                    other => panic!("unexpected {other:?}"),
                }),
            ),
        };
        output.push(json!({"result":result,"error":error,"state":ctx.state(),"log":ctx.log,"frames":ctx.frames}));
    }
    json!(output)
}
