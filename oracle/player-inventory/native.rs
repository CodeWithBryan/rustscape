use rs09_protocol::game::{Frame, updates};
use rs09_server::{
    definitions::{
        item::{ItemDefinition, Registry, Shared},
        property::Property,
    },
    inventory::{
        Error, Result,
        container::{self, Container, ContainerType, Event, Factory, SortType},
        container_listener::Handle as ListenerHandle,
        item::{Handle, Item},
        listener::Listener,
        player_binding::{self as binding, Context, Links},
    },
    player::settings::Settings,
    world::{doors::Location, interfaces::Manager, player_walk::State},
};
use serde_json::{Value, json};
use std::{collections::BTreeSet, sync::Arc};
struct Silent;
impl container::Context for Silent {
    fn update(
        &mut self,
        _: ListenerHandle,
        _: &mut Container,
        _: Event,
        _: &Registry,
    ) -> Result<()> {
        unreachable!()
    }
    fn refresh(&mut self, _: ListenerHandle, _: &mut Container, _: &Registry) -> Result<()> {
        unreachable!()
    }
    fn ground(&mut self, _: &mut Container, _: Option<Handle>, _: &Registry) -> Result<()> {
        unreachable!()
    }
}
pub struct Host {
    pub row: Value,
    pub definitions: Arc<Registry>,
    pub frames: Vec<Value>,
    pub done: BTreeSet<usize>,
    pub manager: Manager,
    pub pouch: Shared<bool>,
    pub syncs: usize,
}
fn n(v: &Value, key: &str, default: i32) -> i32 {
    v[key].as_i64().map_or(default, |n| n as i32)
}
fn property(v: &Value) -> Property {
    if v.is_null() {
        Property::Null
    } else if let Some(v) = v["bits"].as_str() {
        Property::Double(f64::from_bits(u64::from_str_radix(v, 16).unwrap()))
    } else if v["integer"].is_i64() {
        Property::Int(n(v, "integer", 0))
    } else {
        Property::Double(v.as_f64().unwrap())
    }
}
fn item(v: &Value, definitions: &Registry) -> Option<Handle> {
    (!v.is_null()).then(|| {
        Shared::new(Item::new(
            n(v, "id", 0),
            n(v, "amount", 1),
            n(v, "charge", 1000),
            definitions,
        ))
    })
}
fn items(c: &Container) -> Value {
    Value::Array(
        c.items()
            .read()
            .iter()
            .map(|i| {
                i.as_ref().map_or(Value::Null, |i| {
                    let i = i.read();
                    json!([i.id(), i.amount(), i.charge()])
                })
            })
            .collect(),
    )
}
impl binding::Services for Host {
    type Familiar = Shared<bool>;
    type Appearance = ();
    fn send(
        &mut self,
        links: &mut Links<'_>,
        inventory: &mut Container,
        frame: Frame,
    ) -> Result<()> {
        let index = self.frames.len();
        let hex: String = frame.payload.iter().map(|b| format!("{b:02x}")).collect();
        self.frames
            .push(json!({"opcode":frame.opcode,"payloadHex":hex}));
        let hooks = self.row["hooks"].as_array().unwrap().clone();
        for (i, h) in hooks.iter().enumerate() {
            if self.done.contains(&i) || n(h, "at", 0) as usize != index {
                continue;
            }
            self.done.insert(i);
            match h["action"].as_str().unwrap() {
                "clear_inventory" => inventory.clear(false, &self.definitions, &mut Silent)?,
                "clear_equipment" => {
                    links
                        .equipment
                        .clear(false, &self.definitions, &mut Silent)?
                }
                "pouch" => {
                    inventory.items().write()[0] = item(&json!({"id":12047}), &self.definitions)
                }
                "weight" => {
                    self.definitions
                        .for_id(1)
                        .write()
                        .handlers
                        .insert("weight".into(), Property::Double(4.75));
                }
                "fail" => return Err(Error::Binding("channel".into())),
                "refresh" => {
                    let definitions = self.definitions.clone();
                    inventory.refresh(
                        None,
                        &definitions,
                        &mut Context {
                            links,
                            services: self,
                        },
                    )?;
                }
                _ => panic!("unknown hook"),
            }
        }
        Ok(())
    }
    fn text(
        &mut self,
        links: &mut Links<'_>,
        inventory: &mut Container,
        text: String,
        interface: i32,
        child: i32,
    ) -> Result<()> {
        let counter = self.manager.packet_count(1);
        let frame =
            updates::interface_text(interface.wrapping_shl(16) | child, text.as_bytes(), counter)
                .unwrap();
        self.send(links, inventory, frame)
    }
    fn familiar(&mut self, _: &mut Links<'_>, _: &mut Container) -> Result<Option<Self::Familiar>> {
        Ok(Some(self.pouch.clone()))
    }
    fn has_pouch(
        &mut self,
        _: &mut Links<'_>,
        _: &mut Container,
        f: Self::Familiar,
    ) -> Result<bool> {
        Ok(*f.read())
    }
    fn set_has_pouch(
        &mut self,
        _: &mut Links<'_>,
        _: &mut Container,
        f: Self::Familiar,
        v: bool,
    ) -> Result<()> {
        *f.write() = v;
        Ok(())
    }
    fn appearance(&mut self, _: &mut Links<'_>, _: &mut Container) -> Result<Option<()>> {
        Ok(Some(()))
    }
    fn sync(&mut self, _: &mut Links<'_>, _: &mut Container, _: ()) -> Result<()> {
        self.syncs += 1;
        Ok(())
    }
    fn ground(
        &mut self,
        _: &mut Links<'_>,
        _: &mut Container,
        _: Option<Handle>,
        _: &Registry,
    ) -> Result<()> {
        unreachable!()
    }
    fn update_foreign(
        &mut self,
        _: &mut Links<'_>,
        _: ListenerHandle,
        _: &mut Container,
        _: Event,
        _: &Registry,
    ) -> Result<()> {
        Err(Error::Binding("foreign".into()))
    }
    fn refresh_foreign(
        &mut self,
        _: &mut Links<'_>,
        _: ListenerHandle,
        _: &mut Container,
        _: &Registry,
    ) -> Result<()> {
        Err(Error::Binding("foreign".into()))
    }
}
pub fn evaluate(row: &Value) -> Value {
    let mut definitions = Registry::default();
    for (id, weight) in row["weights"].as_object().unwrap() {
        let id = id.parse().unwrap();
        let mut d = ItemDefinition {
            id,
            stackable: id == 1,
            ..Default::default()
        };
        d.handlers.insert("weight".into(), property(weight));
        definitions.insert(id, Shared::new(d));
    }
    let definitions = Arc::new(definitions);
    let factory = Factory::new(&definitions);
    let mut inventory = factory
        .create(28, ContainerType::Default, SortType::Id)
        .unwrap();
    let mut equipment = factory
        .create(14, ContainerType::Default, SortType::Id)
        .unwrap();
    for (i, v) in row["inventory"].as_array().unwrap().iter().enumerate() {
        inventory.items().write()[i] = item(v, &definitions);
    }
    for (i, v) in row["equipment"].as_object().unwrap() {
        equipment.items().write()[i.parse::<usize>().unwrap()] = item(v, &definitions);
    }
    let mut actors = State::default();
    let node = actors.add(1, Location::new(3207, 3216, 0), [1, 2, 3, 4]);
    let player = actors.retain_node(node).unwrap();
    let settings = Settings::default();
    inventory.register(Some(ListenerHandle::new(Listener::new(Some(
        player.clone(),
    )))));
    let manager = Manager::default();
    manager.fields.write().packet_count = n(row, "counter", 0);
    let mut host = Host {
        row: row.clone(),
        definitions: definitions.clone(),
        frames: vec![],
        done: BTreeSet::new(),
        manager,
        pouch: Shared::new(row["had"] == true),
        syncs: 0,
    };
    let old = inventory.event();
    let mut links = Links {
        player: &player,
        inventory: inventory.clone(),
        equipment: &mut equipment,
        settings: &settings,
        null_item: &factory.null_item,
    };
    let input = item(&row["item"], &definitions);
    let mut ctx = Context {
        links: &mut links,
        services: &mut host,
    };
    let result = match row["op"].as_str().unwrap() {
        "refresh" => inventory.refresh(None, &definitions, &mut ctx),
        "add" => inventory
            .add(input.as_ref(), true, -1, &definitions, &mut ctx)
            .map(|_| ()),
        "clear" => inventory.clear(true, &definitions, &mut ctx),
        "replace" => inventory
            .replace(input.as_ref(), 0, true, &definitions, &mut ctx)
            .map(|_| ()),
        _ => panic!("unknown op"),
    };
    let error = result.err().map(|e| match e {
        Error::Null => "Null",
        Error::Type => "Type",
        Error::Binding(_) => "Binding",
        _ => panic!("{e:?}"),
    });
    let value = settings.fields.read().weight;
    let bits = if value.is_nan() {
        0x7ff8000000000000
    } else {
        value.to_bits()
    };
    json!({"frames":host.frames,"weightBits":format!("{bits:016x}"),"pouch":*host.pouch.read(),"syncs":host.syncs,"counter":host.manager.fields.read().packet_count,"inventory":items(&inventory),"equipment":items(&equipment),"sameEvent":inventory.event().same(&old),"error":error})
}
