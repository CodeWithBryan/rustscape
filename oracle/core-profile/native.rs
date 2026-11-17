//! Composite profile observation boundary; production code performs all mutations.
#[allow(dead_code)]
#[path = "../inventory/listeners.rs"]
mod listeners;
#[allow(dead_code)]
#[path = "../timers/native.rs"]
mod timer_native;
use rs09_game_core::java_json::{self, Json};
use rs09_server::{
    definitions::item::{ItemDefinition, Registry, Shared},
    inventory::{
        self,
        container::{Container, Factory},
        item::Item,
    },
    player::{
        attributes::AttributeLocation,
        core_data::{CoreData, DEFAULT_BANK_SIZE, Error},
    },
    timers,
};
use serde_json::{Value, json};
fn number(a: &Value, key: &str, default: i32) -> i32 {
    a.get(key)
        .and_then(Value::as_i64)
        .map_or(default, |v| v as i32)
}
fn error_name(error: &Error) -> &'static str {
    match error {
        Error::Null
        | Error::Inventory(inventory::Error::Null)
        | Error::Timer(timers::Error::Null) => "Null",
        Error::Type
        | Error::Inventory(inventory::Error::Type)
        | Error::Timer(timers::Error::Type) => "Type",
        Error::Number
        | Error::Inventory(inventory::Error::Number)
        | Error::Timer(timers::Error::Number) => "Number",
        Error::Index
        | Error::Inventory(inventory::Error::Bounds | inventory::Error::ListBounds)
        | Error::Timer(timers::Error::Index) => "Index",
        Error::Inventory(inventory::Error::ConcurrentModification)
        | Error::Timer(timers::Error::ConcurrentModification) => "ConcurrentModification",
        Error::Timer(timers::Error::Binding(_))
        | Error::Inventory(inventory::Error::Binding(_)) => "Binding",
        Error::Inventory(inventory::Error::CyclicNote) => panic!("unexpected note cycle"),
    }
}
fn container<'a>(core: &'a mut CoreData, name: &str) -> &'a mut Container {
    match name {
        "bank" => &mut core.primary.container,
        "secondary" => &mut core.secondary.container,
        "equipment" => &mut core.equipment,
        _ => &mut core.inventory,
    }
}
fn observe_container(c: &Container) -> Value {
    let items_handle = c.items();
    let items = items_handle.read();
    let sparse = items
        .iter()
        .enumerate()
        .filter_map(|(i, t)| {
            t.as_ref().map(|t| {
                let t = t.read();
                json!([i, t.id(), t.amount(), t.charge(), t.index])
            })
        })
        .collect::<Vec<_>>();
    let kind = match c.kind() {
        inventory::container::ContainerType::Default => "DEFAULT",
        inventory::container::ContainerType::AlwaysStack => "ALWAYS_STACK",
        _ => unreachable!(),
    };
    let sort = match c.sort() {
        inventory::container::SortType::Id => "ID",
        inventory::container::SortType::Hash => "HASH",
    };
    let event_handle = c.event();
    let event = event_handle.read();
    json!([
        items.len(),
        kind,
        sort,
        c.listeners().len(),
        sparse,
        event.slots(),
        event.clear
    ])
}
fn observe(core: &CoreData) -> Value {
    let varps = core
        .varps
        .order()
        .iter()
        .map(|k| json!([k, core.varps.get(*k).unwrap()]))
        .collect::<Vec<_>>();
    json!({"inventory":observe_container(&core.inventory),"bank":observe_container(&core.primary.container),"secondary":observe_container(&core.secondary.container),"equipment":observe_container(&core.equipment),"tabs":core.primary.tabs,"tabsSecondary":core.secondary.tabs,"secondarySelected":core.use_secondary,"location":core.location.as_ref().map(AttributeLocation::coordinates),"varps":varps,"saveVarps":core.save_varps})
}
pub fn evaluate(request: &Value) -> Value {
    let mut definitions = Registry::default();
    for id in 0..8 {
        definitions.insert(
            id,
            Shared::new(ItemDefinition {
                id,
                name: Some(format!("item{id}")),
                value: id * 11,
                ..Default::default()
            }),
        );
    }
    let mut core = CoreData::new(
        &Factory::new(&definitions),
        DEFAULT_BANK_SIZE,
        [0, 1, 2, 3].map(listeners::new),
        Some(AttributeLocation::new(3222, 3218, 0)),
    )
    .unwrap();
    let (mut harness, mut manager) = timer_native::Harness::new(request);
    let mut states = Vec::new();
    for a in request["actions"].as_array().unwrap() {
        harness.clear_events();
        let result: std::result::Result<Value, Error> = (|| {
            match a["op"].as_str().unwrap() {
                "coreLoad" => {
                    let root = java_json::parse(a["text"].as_str().unwrap()).unwrap();
                    core.parse(Some(&root), &definitions, &mut manager, &mut harness)?;
                }
                "coreSave" => {
                    let mut root = std::mem::replace(&mut harness.root, Json::object());
                    let result = core.save(&mut root, &mut manager, &mut harness);
                    harness.root = root;
                    result?;
                }
                "coreTab" => {
                    let tabs = if a["secondary"] == true {
                        &mut core.secondary.tabs
                    } else {
                        &mut core.primary.tabs
                    };
                    *tabs
                        .get_mut(number(a, "index", 0) as usize)
                        .ok_or(Error::Index)? = number(a, "value", 0);
                }
                "coreRaw" => {
                    let c = container(&mut core, a["container"].as_str().unwrap_or("inventory"));
                    let t = if a["null"] == true {
                        None
                    } else {
                        let mut t = Item::new(
                            number(a, "id", 1),
                            number(a, "amount", 1),
                            number(a, "charge", 1000),
                            &definitions,
                        );
                        t.index = number(a, "index", number(a, "slot", 0));
                        Some(Shared::new(t))
                    };
                    *c.items()
                        .write()
                        .get_mut(number(a, "slot", 0) as usize)
                        .ok_or(Error::Index)? = t;
                }
                _ => return Ok(harness.action(&mut manager, &mut core, a)?),
            }
            Ok(Value::Null)
        })();
        let (result, error) = match result {
            Ok(v) => (v, None),
            Err(e) => (Value::Null, Some(error_name(&e))),
        };
        let mut state = harness.snapshot(&manager, result, error);
        state["extra"] = observe(&core);
        states.push(state);
    }
    json!(states)
}
