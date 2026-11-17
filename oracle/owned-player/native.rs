//! Recording boundary over the actual canonical-player/owned inventory runtime.
use rs09_server::{
    definitions::{
        item::{ItemDefinition, Registry, Shared},
        property::Property,
    },
    inventory::{
        self as inv,
        container::{Container, ContainerType, Factory, SortType},
        container_listener::Handle,
        item::Item,
        listener::Listener,
        player_binding::Capture,
    },
    player::owned::Player,
    world::{
        appearance, doors::Location, interfaces::Manager, player_render, player_walk::State,
        prepared_masks::PreparedMasks,
    },
};
use serde_json::{Value, json};
use std::{cell::Cell, collections::BTreeMap, sync::Arc};
thread_local! { static CLOCKS: Cell<usize> = const { Cell::new(0) }; }
fn clock() -> i64 {
    CLOCKS.with(|v| v.set(v.get() + 1));
    2000
}
fn n(v: &Value, key: &str, fallback: i32) -> i32 {
    v[key].as_i64().map_or(fallback, |v| v as i32)
}
fn item(v: &Value, defs: &Registry) -> Option<inv::item::Handle> {
    (!v.is_null()).then(|| {
        Shared::new(Item::new(
            n(v, "id", 0),
            n(v, "amount", 1),
            n(v, "charge", 1000),
            defs,
        ))
    })
}
fn items(container: &Container) -> Value {
    json!(
        container
            .items()
            .read()
            .iter()
            .map(|i| i.as_ref().map(|i| {
                let i = i.read();
                [i.id(), i.amount(), i.charge()]
            }))
            .collect::<Vec<_>>()
    )
}
fn observe(player: &Player) -> Value {
    let weight = player.settings().fields.read().weight;
    let bits = if weight.is_nan() {
        0x7ff8000000000000
    } else {
        weight.to_bits()
    };
    let syncs = if player.masks().status().appearance_stamp != 0 {
        CLOCKS.with(Cell::get)
    } else {
        0
    };
    json!({"weightBits":format!("{bits:016x}"),"pouch":*player.pouch().read(),"syncs":syncs,"counter":player.manager().fields.read().packet_count,"inventory":items(&player.inventory()),"equipment":items(&player.equipment())})
}
pub fn evaluate(row: &Value) -> Value {
    CLOCKS.with(|v| v.set(0));
    let mut defs = Registry::default();
    for (key, value) in row["weights"].as_object().unwrap() {
        let id = key.parse().unwrap();
        let mut d = ItemDefinition {
            id,
            stackable: id == 1,
            ..Default::default()
        };
        let weight = if let Some(bits) = value["bits"].as_str() {
            Property::Double(f64::from_bits(u64::from_str_radix(bits, 16).unwrap()))
        } else if let Some(i) = value["integer"].as_i64() {
            Property::Int(i as i32)
        } else if value.is_null() {
            Property::Null
        } else {
            Property::Double(value.as_f64().unwrap())
        };
        d.handlers.insert("weight".into(), weight);
        defs.insert(id, Shared::new(d));
    }
    let defs = Arc::new(defs);
    let factory = Factory::new(&defs);
    let mut state = State::default();
    let mut players = BTreeMap::new();
    let mut names = BTreeMap::new();
    for (index, name) in [(1, "A"), (2, "B")] {
        let location = Location::new(3207, 3216, 0);
        let node = state.add(index, location.clone(), [1, 2, 3, 4]);
        let owner = state.retain_node(node).unwrap();
        names.insert(node, name);
        let inventory = factory
            .create(28, ContainerType::Default, SortType::Id)
            .unwrap();
        let equipment = factory
            .create(14, ContainerType::Default, SortType::Id)
            .unwrap();
        for (i, v) in row[name]["inventory"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
        {
            inventory.items().write()[i] = item(v, &defs);
        }
        for (i, v) in row[name]["equipment"].as_object().unwrap() {
            equipment.items().write()[i.parse::<usize>().unwrap()] = item(v, &defs);
        }
        let manager = Manager::default();
        manager.fields.write().packet_count = n(&row[name], "counter", 0);
        let masks = PreparedMasks::new(
            Shared::new(player_render::Player::new(index, location, 2000)),
            Shared::new(appearance::Data::naked(name.into(), 0)),
        );
        let core = Capture::new(
            owner,
            inventory,
            equipment,
            Default::default(),
            factory.null_item.clone(),
        );
        let player = Player::new(
            state.identity(),
            core,
            defs.clone(),
            manager,
            Shared::new(row[name]["had"] == true),
            masks,
        )
        .unwrap();
        state.bind_player_fields(player.clone()).unwrap();
        players.insert(name, player);
    }
    let player = &players[row["target"].as_str().unwrap()];
    let receiver = &players[row["listener"].as_str().unwrap()];
    let mut target = if row["equipmentTarget"] == true {
        player.equipment()
    } else {
        player.inventory()
    };
    target.register(Some(Handle::new(Listener::new(Some(receiver.owner())))));
    let input = item(&row["item"], &defs);
    let mut frames = BTreeMap::new();
    let result = player.apply(
        &mut state,
        &mut frames,
        clock,
        |_, defs, context| match row["op"].as_str().unwrap() {
            "refresh" => target.refresh(None, defs, context),
            "clear" => target.clear(true, defs, context),
            "add" => target
                .add(input.as_ref(), true, -1, defs, context)
                .map(|_| ()),
            "replace" => target
                .replace(input.as_ref(), 0, true, defs, context)
                .map(|_| ()),
            _ => panic!("unknown op"),
        },
    );
    let error = result.err().map(|e| match e {
        inv::Error::Type => "Type",
        inv::Error::Null => "Null",
        inv::Error::Binding(_) => "Binding",
        e => panic!("{e:?}"),
    });
    let frames:Vec<_>=frames.into_iter().flat_map(|(node,values)| {let name=names[&node];values.into_iter().map(move|frame|json!({"owner":name,"opcode":frame.opcode,"payloadHex":frame.payload.iter().map(|b|format!("{b:02x}")).collect::<String>()}))}).collect();
    json!({"frames":frames,"A":observe(&players["A"]),"B":observe(&players["B"]),"error":error})
}
