//! Test-only command/observation boundary. No reference implementations run here.
#[allow(dead_code)]
#[path = "listeners.rs"]
mod listeners;
use rs09_server::{
    definitions::item::{ItemDefinition, Registry, Shared},
    inventory::{
        Error, Result,
        container::{
            Container, ContainerType, Context, Event, Factory, Items, Listeners, SortType,
        },
        container_listener::Handle as ListenerHandle,
        item::{self, Bindings, Handle, Item},
    },
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, sync::Arc};
fn int(v: &Value) -> i32 {
    v.as_i64().unwrap() as i32
}
fn number(v: &Value, key: &str, default: i32) -> i32 {
    v.get(key).map_or(default, int)
}
fn flag(v: &Value, key: &str, default: bool) -> bool {
    v.get(key).map_or(default, |v| v.as_bool().unwrap())
}
type Ids = BTreeMap<usize, usize>;
fn tag<T>(value: &Shared<T>, ids: &mut Ids) -> Value {
    let next = ids.len();
    json!(ids.entry(value.identity()).or_insert(next))
}
fn observe_item(item: Option<&Handle>, mut ids: Option<&mut Ids>) -> Value {
    let Some(handle) = item else {
        return Value::Null;
    };
    let item = handle.read();
    let definition=item.definition.as_ref().map(|definition|{let def=definition.read();json!({"id":def.id,"name":def.name,"value":def.value,"stackable":def.is_stackable(),"unnoted":def.unnoted,"noteId":def.note_id})});
    let mut value = json!({"idHash":item.id_hash(),"id":item.id(),"charge":item.charge(),"amount":item.amount(),"name":item.name,"index":item.index,"destination":item.item_destination.then_some("ITEM"),"definition":definition});
    if let Some(ids) = ids.as_mut() {
        value["ref"] = tag(handle, ids);
    }
    value
}
fn observe_array(items: &Items, mut ids: Option<&mut Ids>) -> Value {
    let reference = ids.as_mut().map(|ids| tag(items, ids));
    let values: Vec<_> = items
        .read()
        .iter()
        .map(|item| observe_item(item.as_ref(), ids.as_deref_mut()))
        .collect();
    if let Some(reference) = reference {
        json!({"ref":reference,"values":values})
    } else {
        json!(values)
    }
}
fn observe_event(event: &Event, mut ids: Option<&mut Ids>) -> Value {
    let reference = ids.as_mut().map(|ids| tag(event, ids));
    let event = event.read();
    let mut value = json!({"items":observe_array(&event.items,ids),"clear":event.clear,"changeCount":event.change_count(),"slots":event.slots()});
    if let Some(reference) = reference {
        value["ref"] = reference;
    }
    value
}
struct Env {
    listener_objects: listeners::Objects,
    named_listeners: BTreeMap<String, Option<ListenerHandle>>,
    definitions: Arc<Registry>,
    containers: BTreeMap<i32, Container>,
    items: BTreeMap<String, Option<Handle>>,
    events: Vec<Event>,
    arrays: Vec<Items>,
    null_item: Handle,
    trace: Vec<Value>,
    scripts: Value,
    drop: Value,
    drop_action: Value,
    calls: BTreeMap<(usize, bool), usize>,
    exposed_lists: Vec<Listeners>,
}
impl Env {
    fn new(case: &Value) -> Self {
        let mut definitions = Registry::default();
        for (key, fields) in case["definitions"].as_object().unwrap() {
            let def: ItemDefinition = serde_json::from_value(fields.clone()).unwrap();
            definitions.insert(key.parse().unwrap(), Shared::new(def));
        }
        let definitions = Arc::new(definitions);
        let factory = Factory::new(&definitions);
        let mut listener_objects = listeners::Objects::default();
        let mut containers = BTreeMap::new();
        for row in case["containers"].as_array().unwrap() {
            let mut c = factory
                .create(
                    int(&row["capacity"]),
                    serde_json::from_value::<ContainerType>(row["type"].clone()).unwrap(),
                    serde_json::from_value::<SortType>(row["sort"].clone()).unwrap(),
                )
                .unwrap();
            for listener in row["listeners"].as_array().unwrap() {
                c.register(listener_objects.optional(listener));
            }
            containers.insert(int(&row["id"]), c);
        }
        let items = case["items"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(key, row)| {
                let item = if row.is_null() {
                    None
                } else {
                    Some(Shared::new(if row["empty"] == true {
                        Item::default()
                    } else {
                        Item::new(
                            number(row, "id", 0),
                            number(row, "amount", 1),
                            number(row, "charge", 1000),
                            &definitions,
                        )
                    }))
                };
                (key.clone(), item)
            })
            .collect();
        Self {
            listener_objects,
            named_listeners: BTreeMap::new(),
            definitions,
            containers,
            items,
            events: Vec::new(),
            arrays: Vec::new(),
            null_item: factory.null_item,
            trace: Vec::new(),
            scripts: case["scripts"].clone(),
            drop: case["drop"].clone(),
            drop_action: case["dropAction"].clone(),
            calls: BTreeMap::new(),
            exposed_lists: vec![],
        }
    }
    fn state(&self) -> Value {
        let mut ids = Ids::new();
        let mut cs = BTreeMap::new();
        let mut items = BTreeMap::new();
        for (id, c) in &self.containers {
            let values = observe_array(&c.items(), Some(&mut ids));
            let event = observe_event(&c.event(), Some(&mut ids));
            cs.insert(id.to_string(),json!({"capacity":c.capacity(),"items":values,"event":event,"listeners":listeners::values(c.listeners()),"freeSlots":c.free_slots(),"itemCount":c.count()}));
        }
        for (name, item) in &self.items {
            items.insert(name, observe_item(item.as_ref(), Some(&mut ids)));
        }
        let events: Vec<_> = self
            .events
            .iter()
            .map(|event| observe_event(event, Some(&mut ids)))
            .collect();
        let arrays: Vec<_> = self
            .arrays
            .iter()
            .map(|array| observe_array(array, Some(&mut ids)))
            .collect();
        json!({"containers":cs,"items":items,"retainedEvents":events,"retainedArrays":arrays,"nullItem":observe_item(Some(&self.null_item),Some(&mut ids)),"trace":self.trace})
    }
    fn input(&self, a: &Value) -> Option<Handle> {
        a["item"]
            .as_str()
            .and_then(|name| self.items.get(name))
            .cloned()
            .flatten()
    }
    fn list(&self, a: &Value) -> Vec<Option<Handle>> {
        a["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|name| {
                name.as_str()
                    .and_then(|name| self.items.get(name))
                    .cloned()
                    .flatten()
            })
            .collect()
    }
    fn store(&mut self, a: &Value, value: Option<Handle>) -> Value {
        let result = observe_item(value.as_ref(), None);
        if let Some(name) = a["as_"].as_str() {
            self.items.insert(name.to_owned(), value);
        }
        result
    }
    fn dispatch(&mut self, a: &Value) -> Result<Value> {
        let id = number(a, "container", 0);
        let mut c = self.containers.get(&id).cloned();
        self.execute(a, c.as_mut())
    }
    fn execute(&mut self, a: &Value, c: Option<&mut Container>) -> Result<Value> {
        let op = a["op"].as_str().unwrap();
        match op {
            "newListener" => {
                self.named_listeners.insert(
                    a["name"].as_str().unwrap().into(),
                    Some(listeners::new(number(a, "label", 0) as usize)),
                );
                return Ok(Value::Null);
            }
            "aliasListener" => {
                let listener = self
                    .named_listeners
                    .get(a["name"].as_str().unwrap())
                    .cloned()
                    .flatten();
                self.named_listeners
                    .insert(a["as_"].as_str().unwrap().into(), listener);
                return Ok(Value::Null);
            }
            "retireListener" => {
                self.named_listeners.remove(a["name"].as_str().unwrap());
                return Ok(Value::Null);
            }
            "sameListener" => {
                return Ok(json!(
                    self.named_listeners
                        .get(a["name"].as_str().unwrap())
                        .cloned()
                        .flatten()
                        == self
                            .named_listeners
                            .get(a["other"].as_str().unwrap())
                            .cloned()
                            .flatten()
                ));
            }
            "listAddNamed" | "listSetNamed" | "listUnregisterNamed" => {
                let list = self
                    .exposed_lists
                    .get(number(a, "list", 0) as usize)
                    .ok_or(Error::ListBounds)?
                    .clone();
                let listener = self
                    .named_listeners
                    .get(a["name"].as_str().unwrap())
                    .cloned()
                    .flatten();
                match op {
                    "listAddNamed" => list.add(listener),
                    "listSetNamed" => {
                        list.set(number(a, "index", 0) as usize, listener)?;
                    }
                    "listUnregisterNamed" => return Ok(json!(list.remove_first(listener))),
                    _ => unreachable!(),
                }
                return Ok(Value::Null);
            }
            "via" => {
                return self.dispatch(&a["action"]);
            }
            "retireContainer" => {
                self.containers.remove(&number(a, "value", 0));
                return Ok(Value::Null);
            }
            "newContainer" => {
                let factory = Factory {
                    null_item: self.null_item.clone(),
                };
                let container = factory.create(
                    number(a, "capacity", 4),
                    serde_json::from_value(a["type"].clone()).unwrap(),
                    serde_json::from_value(a["sort"].clone()).unwrap(),
                )?;
                self.containers.insert(number(a, "value", 0), container);
                return Ok(Value::Null);
            }
            "sameContainer" => {
                let other = self.containers.get(&number(a, "source", 0));
                return Ok(json!(match (c, other) {
                    (Some(a), Some(b)) => a.same(b),
                    (None, None) => true,
                    _ => false,
                }));
            }
            "listAdd" | "listRemove" | "listUnregister" | "listValues" | "listSet"
            | "listClear" => {
                let list = self
                    .exposed_lists
                    .get(number(a, "list", 0) as usize)
                    .ok_or(Error::ListBounds)?
                    .clone();
                match op {
                    "listAdd" => list.add(self.listener_objects.optional(&a["value"])),
                    "listRemove" => {
                        list.remove(number(a, "value", 0) as usize)?;
                    }
                    "listUnregister" => {
                        return Ok(json!(
                            list.remove_first(self.listener_objects.optional(&a["value"]))
                        ));
                    }
                    "listValues" => return Ok(json!(listeners::values(list.values()))),
                    "listSet" => {
                        list.set(
                            number(a, "index", 0) as usize,
                            self.listener_objects.optional(&a["value"]),
                        )?;
                    }
                    "listClear" => list.clear(),
                    _ => unreachable!(),
                }
                return Ok(Value::Null);
            }
            _ => {}
        }
        let item = self.input(a);
        let defs = Arc::clone(&self.definitions);
        match op {
            "itemQuery" => {
                let item = item.as_ref().ok_or(Error::Null)?;
                let value = item.read().value(&defs)?;
                let alchemy = item.read().alchemy_value(&defs)?;
                let note = item.read().note_change()?;
                let data = item.read();
                return Ok(
                    json!({"value":value,"alchemy":alchemy,"noteChange":note,"charged":data.charged(),"text":data.text()}),
                );
            }
            "itemAlchemy" => {
                let def = item
                    .as_ref()
                    .ok_or(Error::Null)?
                    .read()
                    .definition
                    .clone()
                    .ok_or(Error::Null)?;
                return Ok(json!(item::definition_alchemy(
                    &def,
                    flag(a, "high", true),
                    &defs
                )?));
            }
            "itemAmount" => item
                .as_ref()
                .ok_or(Error::Null)?
                .write()
                .set_amount(int(&a["value"])),
            "itemCharge" => item
                .as_ref()
                .ok_or(Error::Null)?
                .write()
                .set_charge(int(&a["value"])),
            "itemId" => item
                .as_ref()
                .ok_or(Error::Null)?
                .write()
                .set_id(int(&a["value"]), &defs),
            "itemIndex" => item.as_ref().ok_or(Error::Null)?.write().index = int(&a["value"]),
            "itemDefinition" => {
                item.as_ref().ok_or(Error::Null)?.write().definition =
                    (!a["value"].is_null()).then(|| defs.for_id(int(&a["value"])))
            }
            "itemCopy" => {
                let copy = Shared::new(item.as_ref().ok_or(Error::Null)?.read().copy(&defs));
                return Ok(self.store(a, Some(copy)));
            }
            "itemUnnote" => {
                let result = item::unnoted(item.as_ref().ok_or(Error::Null)?, &defs)?;
                return Ok(self.store(a, Some(result)));
            }
            "itemDrop" => {
                let result = item::drop_item(item.as_ref().ok_or(Error::Null)?, &defs, self)?;
                return Ok(self.store(a, Some(result)));
            }
            "itemPlugin" => {
                return Ok(json!(item::plugin(
                    item.as_ref().ok_or(Error::Null)?,
                    self
                )?));
            }
            "itemOperate" => {
                return Ok(json!(item::operate(
                    item.as_ref().ok_or(Error::Null)?,
                    self
                )?));
            }
            "itemHasPlugin" => {
                return Ok(json!(item::has_plugin(
                    item.as_ref().ok_or(Error::Null)?,
                    self
                )?));
            }
            _ => {}
        }
        if op.starts_with("item") {
            return Ok(Value::Null);
        }
        if op == "fail" {
            return Err(Error::Binding("RuntimeException".into()));
        }
        let c = c.ok_or(Error::Null)?;
        let fire = flag(a, "fire", true);
        let slot = number(a, "slot", 0);
        match op {
            "registerNamed" => c.register(
                self.named_listeners
                    .get(a["name"].as_str().unwrap())
                    .cloned()
                    .flatten(),
            ),
            "unregisterNamed" => {
                return Ok(json!(
                    c.unregister(
                        self.named_listeners
                            .get(a["name"].as_str().unwrap())
                            .cloned()
                            .flatten()
                    )
                ));
            }
            "refreshNamed" => c.refresh_listener(
                self.named_listeners
                    .get(a["name"].as_str().unwrap())
                    .cloned()
                    .flatten(),
                &defs,
                self,
            )?,
            "aliasContainer" => {
                self.containers.insert(number(a, "value", 0), c.clone());
            }
            "captureListeners" => self.exposed_lists.push(c.listener_list()),
            "sameList" => {
                return Ok(json!(
                    c.listener_list().same(
                        self.exposed_lists
                            .get(number(a, "list", 0) as usize)
                            .ok_or(Error::ListBounds)?
                    )
                ));
            }
            "add" => {
                return Ok(json!(c.add(
                    item.as_ref(),
                    fire,
                    number(a, "preferred", -1),
                    &defs,
                    self
                )?));
            }
            "addGround" => return Ok(json!(c.add_or_ground(item.as_ref(), &defs, self)?)),
            "addAbsent" => return Ok(json!(c.add_if_absent(item.as_ref(), &defs, self)?)),
            "addMany" => {
                let values = Shared::new(self.list(a));
                return Ok(json!(c.add_many(&values, &defs, self)?));
            }
            "addList" => c.add_list(&self.list(a), &defs, self)?,
            "remove" => {
                return Ok(json!(c.remove(
                    item.as_ref(),
                    a.get("slot").map(int),
                    fire,
                    &defs,
                    self
                )?));
            }
            "removeMany" => {
                let values = Shared::new(self.list(a));
                return Ok(json!(c.remove_many(&values, &defs, self)?));
            }
            "removeAll" => {
                let ids: Vec<_> = a["ids"].as_array().unwrap().iter().map(int).collect();
                return Ok(json!(c.remove_all_ids(&ids, &defs, self)?));
            }
            "replace" => {
                let old = c.replace(item.as_ref(), slot, fire, &defs, self)?;
                return Ok(self.store(a, old));
            }
            "insert" => c.insert(number(a, "from_", 0), number(a, "to", 0), fire, &defs, self)?,
            "get" => return Ok(self.store(a, c.get(slot))),
            "getNew" => return Ok(self.store(a, Some(c.get_new(slot, &defs)?))),
            "getId" => return Ok(json!(c.get_id(slot)?)),
            "getAsId" => return Ok(json!(c.get_as_id(slot))),
            "getItem" => return Ok(self.store(a, c.get_item(item.as_ref()))),
            "getByItem" => return Ok(self.store(a, c.get_by_item(item.as_ref())?)),
            "getAll" => {
                return Ok(json!(
                    c.get_all(item.as_ref())?
                        .iter()
                        .map(|item| observe_item(Some(item), None))
                        .collect::<Vec<_>>()
                ));
            }
            "query" => {
                let maximum = c.maximum_add(item.as_ref())?;
                let space = c.has_space_for(item.as_ref())?;
                let add_slot = c.add_slot(item.as_ref())?;
                let wealth = c.wealth()?;
                return Ok(
                    json!({"slot":c.slot(item.as_ref(),false),"hashSlot":c.slot(item.as_ref(),true),"amount":c.amount(item.as_ref()),"maximum":maximum,"space":space,"addSlot":add_slot,"empty":c.empty(),"full":c.full(),"wealth":wealth,"freeSlot":c.free_slot()}),
                );
            }
            "contains" => {
                return Ok(json!(
                    c.contains(number(a, "id", 0), number(a, "amount", 0))
                ));
            }
            "containsItem" => return Ok(json!(c.contains_item(item.as_ref())?)),
            "containsItems" => return Ok(json!(c.contains_items(&self.list(a))?)),
            "containsOneItems" => return Ok(json!(c.contains_one_items(&self.list(a))?)),
            "containsOneId" => return Ok(json!(c.contains_one_id(number(a, "id", 0)))),
            "containsOneIds" => {
                let ids: Vec<_> = a["ids"].as_array().unwrap().iter().map(int).collect();
                return Ok(json!(c.contains_one_ids(&ids, &defs)));
            }
            "containsAll" => {
                let ids: Vec<_> = a["ids"].as_array().unwrap().iter().map(int).collect();
                return Ok(json!(c.contains_all(&ids)));
            }
            "containItems" => {
                let ids: Vec<_> = a["ids"].as_array().unwrap().iter().map(int).collect();
                return Ok(json!(c.contain_items(&ids)));
            }
            "update" => c.update(a.get("force").and_then(Value::as_bool), &defs, self)?,
            "refresh" => {
                if let Some(listener) = a.get("listener") {
                    c.refresh_listener(self.listener_objects.optional(listener), &defs, self)?;
                } else {
                    c.refresh(None, &defs, self)?;
                }
            }
            "clear" => c.clear(fire, &defs, self)?,
            "shift" => c.shift(&defs, self)?,
            "format" => return Ok(json!(c.format())),
            "parse" => c.parse(&a["value"], &defs)?,
            "copy" => {
                let source = self
                    .containers
                    .get(&number(a, "source", 0))
                    .ok_or(Error::Null)?;
                c.copy_from(source, &defs);
            }
            "addAll" => {
                let source = number(a, "source", 0);
                let array = self.containers.get(&source).ok_or(Error::Null)?.items();
                c.add_many(&array, &defs, self)?;
            }
            "spaceContainer" => {
                let source = number(a, "source", 0);
                let array = self.containers.get(&source).map(Container::items);
                return Ok(json!(c.has_space_for_container(
                    array.as_ref(),
                    &defs,
                    self
                )?));
            }
            "spaceMany" => {
                let items = Shared::new(self.list(a));
                return Ok(json!(c.has_space_for_many(&items, &defs, self)?));
            }
            "register" => c.register(self.listener_objects.optional(&a["value"])),
            "unregister" => {
                return Ok(json!(
                    c.unregister(self.listener_objects.optional(&a["value"]))
                ));
            }
            "removeListener" => c.remove_listener(int(&a["value"]) as usize)?,
            "captureEvent" => self.events.push(c.event()),
            "captureArray" => self.arrays.push(c.items()),
            "eventClear" => c.event().write().clear = a["value"].as_bool().unwrap(),
            "rawSet" => {
                *c.items()
                    .write()
                    .get_mut(slot as usize)
                    .ok_or(Error::Bounds)? = item;
            }
            "eventSet" => {
                let event_handle = c.event();
                let event = event_handle.read();
                *event
                    .items
                    .write()
                    .get_mut(slot as usize)
                    .ok_or(Error::Bounds)? = item;
            }
            "retainedSet" => {
                let array = self
                    .arrays
                    .get(int(&a["array"]) as usize)
                    .ok_or(Error::ListBounds)?;
                *array.write().get_mut(slot as usize).ok_or(Error::Bounds)? = item;
            }
            "nullAmount" => self.null_item.write().set_amount(int(&a["value"])),
            "mutateSlot" => {
                let value = c.get(slot).ok_or(Error::Null)?;
                if a["field"] == "amount" {
                    value.write().set_amount(int(&a["value"]));
                } else {
                    value.write().set_charge(int(&a["value"]));
                }
            }
            "fail" => return Err(Error::Binding("RuntimeException".into())),
            _ => panic!("unknown inventory command {op}"),
        }
        Ok(Value::Null)
    }
    fn callback(
        &mut self,
        id: usize,
        c: &mut Container,
        event: Option<Event>,
        definitions: &Registry,
    ) -> Result<()> {
        let update = event.is_some();
        let count = self.calls.entry((id, update)).or_default();
        *count += 1;
        let count = *count;
        self.trace.push(json!({"kind":if update{"update"}else{"refresh"},"listener":id,"items":observe_array(&c.items(),None),"event":event.as_ref().map(|event|observe_event(event,None))}));
        if let Some(event) = event {
            self.events.push(event);
        }
        let rules = self.scripts[id.to_string()]
            .as_array()
            .cloned()
            .unwrap_or_default();
        for rule in rules {
            if rule["when"] == if update { "update" } else { "refresh" }
                && rule["at"] == count as u64
            {
                self.execute(&rule["action"], Some(c))?;
            }
        }
        let _ = definitions;
        Ok(())
    }
}
impl Context for Env {
    fn update(
        &mut self,
        id: ListenerHandle,
        c: &mut Container,
        event: Event,
        definitions: &Registry,
    ) -> Result<()> {
        self.callback(listeners::label(&id), c, Some(event), definitions)
    }
    fn refresh(
        &mut self,
        id: ListenerHandle,
        c: &mut Container,
        definitions: &Registry,
    ) -> Result<()> {
        self.callback(listeners::label(&id), c, None, definitions)
    }
    fn ground(&mut self, _: &mut Container, item: Option<Handle>, _: &Registry) -> Result<()> {
        self.trace
            .push(json!({"kind":"ground","item":observe_item(item.as_ref(),None)}));
        Ok(())
    }
}
impl Bindings for Env {
    type Handler = String;
    type Plugin = String;
    fn drop_replacement(&mut self, id: i32) -> Result<i32> {
        self.trace.push(json!({"kind":"replacement","id":id}));
        if !self.drop_action.is_null() {
            let action = self.drop_action.clone();
            self.execute(&action, None)?;
        }
        Ok(self.drop[id.to_string()]
            .as_i64()
            .map_or(id, |id| id as i32))
    }
    fn operate_handler(&mut self, id: i32) -> Result<Option<String>> {
        let def = self.definitions.for_id(id);
        let def = def.read();
        def.handlers
            .get("option:operate")
            .filter(|v| !v.is_null())
            .map(|v| v.as_str().map(str::to_owned).ok_or(Error::Type))
            .transpose()
    }
    fn plugin(&mut self, def: &rs09_server::definitions::item::Handle) -> Result<Option<String>> {
        def.read()
            .handlers
            .get("wrapper")
            .filter(|v| !v.is_null())
            .map(|v| v.as_str().map(str::to_owned).ok_or(Error::Type))
            .transpose()
    }
}
fn error(error: Error) -> String {
    match error {
        Error::Null => "NullPointerException".into(),
        Error::Bounds => "ArrayIndexOutOfBoundsException".into(),
        Error::ListBounds => "IndexOutOfBoundsException".into(),
        Error::Type => "ClassCastException".into(),
        Error::Number => "NumberFormatException".into(),
        Error::ConcurrentModification => "ConcurrentModificationException".into(),
        Error::CyclicNote => "StackOverflowError".into(),
        Error::Binding(message) => message,
    }
}
pub fn evaluate(case: &Value) -> Value {
    if case["kind"] == "integerMap" {
        return json!(rs09_game_core::java_map::int_key_order(
            case["keys"].as_array().unwrap().iter().map(int)
        ));
    }
    let mut env = Env::new(case);
    let mut output = Vec::new();
    for action in case["actions"].as_array().unwrap() {
        let (result, error) = match env.dispatch(action) {
            Ok(result) => (result, None),
            Err(e) => (Value::Null, Some(error(e))),
        };
        output.push(json!({"result":result,"error":error,"state":env.state()}));
    }
    json!(output)
}
