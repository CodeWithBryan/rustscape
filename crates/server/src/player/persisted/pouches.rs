//! PouchManager's four runecrafting pouches as persisted: charges, current
//! capacity and the essence each holds. Filling, emptying, decay and repair
//! are not ported.
use super::{kotlin_array, kotlin_object, kotlin_text, parse_int};
use crate::{
    definitions::item::{Registry, Shared},
    inventory::{
        self,
        container::{Container, ContainerType, Context, Event, Factory, SortType},
        container_listener::Handle as Listener,
        item::{Handle, Item},
    },
    player::save::{Error, Result},
};
use rs09_game_core::java_json::Json;

/// `Container(capacity)`: default type and id sort. A negative capacity is
/// Java's NegativeArraySizeException; the native factory also refuses
/// capacities above 1,000,000, which the original would try to allocate.
pub fn container(factory: &Factory, capacity: i32) -> Result<Container> {
    if capacity < 0 {
        return Err(Error::NegativeSize);
    }
    factory
        .create(capacity, ContainerType::Default, SortType::Id)
        .map_err(Error::Inventory)
}

pub struct Pouch {
    pub id: i32,
    pub capacity: i32,
    pub max_charges: i32,
    pub level: i32,
    pub container: Container,
    pub current_cap: i32,
    pub charges: i32,
}
impl Pouch {
    pub fn remake_container(&mut self, factory: &Factory) -> Result<()> {
        self.container = container(factory, self.current_cap)?;
        Ok(())
    }
}
/// A pouch container has no listeners; nothing it does may reach one.
struct Unbound;
impl Context for Unbound {
    fn update(
        &mut self,
        _: Listener,
        _: &mut Container,
        _: Event,
        _: &Registry,
    ) -> inventory::Result<()> {
        Err(inventory::Error::Binding(
            "pouch containers have no listeners".into(),
        ))
    }
    fn refresh(&mut self, _: Listener, _: &mut Container, _: &Registry) -> inventory::Result<()> {
        Err(inventory::Error::Binding(
            "pouch containers have no listeners".into(),
        ))
    }
    fn ground(
        &mut self,
        _: &mut Container,
        _: Option<Handle>,
        _: &Registry,
    ) -> inventory::Result<()> {
        Err(inventory::Error::Binding(
            "pouch containers have no ground".into(),
        ))
    }
}

pub struct Manager {
    /// `mapOf(...)`: a LinkedHashMap in declaration order.
    pub pouches: Vec<Pouch>,
}
impl Manager {
    pub fn new(factory: &Factory) -> Result<Self> {
        let pouch = |id, capacity, max_charges, level| -> Result<Pouch> {
            Ok(Pouch {
                id,
                capacity,
                max_charges,
                level,
                container: container(factory, capacity)?,
                current_cap: capacity,
                charges: max_charges,
            })
        };
        Ok(Self {
            pouches: vec![
                pouch(5509, 3, 3, 1)?,
                pouch(5510, 6, 264, 25)?,
                pouch(5512, 9, 186, 50)?,
                pouch(5514, 12, 140, 75)?,
            ],
        })
    }
    pub fn get_mut(&mut self, id: i32) -> Option<&mut Pouch> {
        self.pouches.iter_mut().find(|p| p.id == id)
    }
    /// `container.add(Item(id, amount))`; the result is ignored by the parser.
    pub fn add(pouch: &mut Pouch, id: i32, amount: i32, definitions: &Registry) -> Result<bool> {
        let item = Shared::new(Item::new(id, amount, 1000, definitions));
        pouch
            .container
            .add(Some(&item), true, -1, definitions, &mut Unbound)
            .map_err(Error::Inventory)
    }
    /// PouchManager.save: writes the root's `pouches` key itself.
    pub fn save(&self, root: &mut Json) -> Result<()> {
        let text = |value: i32| Json::Text(value.to_string());
        let mut pouches = Vec::with_capacity(self.pouches.len());
        for pouch in &self.pouches {
            let mut object = Json::object();
            let put = |o: &mut Json, k: &str, v| o.put(k, v).map(|_| ()).map_err(|_| Error::Type);
            put(&mut object, "id", text(pouch.id))?;
            let mut items = Vec::new();
            for item in pouch.container.items().read().iter().flatten() {
                let item = item.read();
                let mut it = Json::object();
                put(&mut it, "itemId", text(item.id()))?;
                put(&mut it, "amount", text(item.amount()))?;
                items.push(it);
            }
            put(&mut object, "container", Json::Array(items))?;
            put(&mut object, "charges", text(pouch.charges))?;
            put(&mut object, "currentCap", text(pouch.current_cap))?;
            pouches.push(object);
        }
        root.put("pouches", Json::Array(pouches))
            .map(|_| ())
            .map_err(|_| Error::Type)
    }
    /// PouchManager.parse. An unknown pouch id ends the whole parse without a
    /// failure; values are stored and the container remade before its items
    /// are read, so a later failure keeps them. Rows without `itemId` are skipped.
    pub fn parse(
        &mut self,
        data: &[Json],
        factory: &Factory,
        definitions: &Registry,
    ) -> Result<()> {
        for pouch in data {
            let pouch = kotlin_object(Some(pouch))?;
            let id = parse_int(&kotlin_text(pouch.get("id")))?;
            let Some(target) = self.get_mut(id) else {
                return Ok(());
            };
            let charges = parse_int(&kotlin_text(pouch.get("charges")))?;
            let current_cap = parse_int(&kotlin_text(pouch.get("currentCap")))?;
            target.charges = charges;
            target.current_cap = current_cap;
            target.remake_container(factory)?;
            for item in kotlin_array(pouch.get("container"))? {
                let item = kotlin_object(Some(item))?;
                if matches!(item.get("itemId"), None | Some(Json::Null)) {
                    continue;
                }
                let id = parse_int(&kotlin_text(item.get("itemId")))?;
                let amount = parse_int(&kotlin_text(item.get("amount")))?;
                Self::add(target, id, amount, definitions)?;
            }
        }
        Ok(())
    }
}
