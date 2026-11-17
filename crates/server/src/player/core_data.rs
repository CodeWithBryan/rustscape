//! Core character save state. Full Player construction, bank/equipment actions,
//! network listeners, other profile sections and world entry remain separate.
use super::attributes::AttributeLocation;
use crate::{
    definitions::item::Registry,
    inventory::{
        self,
        container::{Container, ContainerType, Factory, SortType},
        container_listener::Handle as Listener,
    },
    timers::{self, Context, Manager},
};
use rs09_game_core::{java_json::Json, java_map::int_key_order, java_string::parse_i32};
use serde_json::Value;
use std::collections::BTreeMap;
pub const BANK_TABS: usize = 11;
pub const DEFAULT_BANK_SIZE: i32 = 496;
#[derive(Debug)]
pub enum Error {
    Null,
    Type,
    Number,
    Index,
    Inventory(inventory::Error),
    Timer(timers::Error),
}
pub type Result<T> = std::result::Result<T, Error>;
impl From<inventory::Error> for Error {
    fn from(error: inventory::Error) -> Self {
        Self::Inventory(error)
    }
}
impl From<timers::Error> for Error {
    fn from(error: timers::Error) -> Self {
        Self::Timer(error)
    }
}
pub struct Bank {
    pub container: Container,
    pub tabs: [i32; BANK_TABS],
}
#[derive(Default)]
pub struct Varps {
    values: BTreeMap<i32, i32>,
    insertion: Vec<i32>,
}
impl Varps {
    pub fn insert(&mut self, key: i32, value: i32) {
        if !self.values.contains_key(&key) {
            self.insertion.push(key);
        }
        self.values.insert(key, value);
    }
    pub fn get(&self, key: i32) -> Option<i32> {
        self.values.get(&key).copied()
    }
    pub fn values(&self) -> &BTreeMap<i32, i32> {
        &self.values
    }
    pub fn order(&self) -> Vec<i32> {
        int_key_order(self.insertion.iter().copied())
    }
}
pub struct CoreData {
    pub inventory: Container,
    pub equipment: Container,
    pub primary: Bank,
    pub secondary: Bank,
    pub use_secondary: bool,
    pub location: Option<AttributeLocation>,
    pub varps: Varps,
    pub save_varps: BTreeMap<i32, Option<bool>>,
}
impl CoreData {
    /// Bank capacity and listener identities come from native world startup;
    /// the fixed inventory/equipment dimensions are the original constructors'.
    pub fn new(
        factory: &Factory,
        bank_size: i32,
        listeners: [Listener; 4],
        location: Option<AttributeLocation>,
    ) -> Result<Self> {
        let mut inventory = factory.create(28, ContainerType::Default, SortType::Id)?;
        inventory.register(Some(listeners[0].clone()));
        let mut equipment = factory.create(14, ContainerType::Default, SortType::Id)?;
        equipment.register(Some(listeners[1].clone()));
        let mut primary = factory.create(bank_size, ContainerType::AlwaysStack, SortType::Hash)?;
        primary.register(Some(listeners[2].clone()));
        let mut secondary =
            factory.create(bank_size, ContainerType::AlwaysStack, SortType::Hash)?;
        secondary.register(Some(listeners[3].clone()));
        Ok(Self {
            inventory,
            equipment,
            primary: Bank {
                container: primary,
                tabs: [0; BANK_TABS],
            },
            secondary: Bank {
                container: secondary,
                tabs: [0; BANK_TABS],
            },
            use_secondary: false,
            location,
            varps: Varps::default(),
            save_varps: BTreeMap::new(),
        })
    }
    pub fn bank(&self) -> &Bank {
        if self.use_secondary {
            &self.secondary
        } else {
            &self.primary
        }
    }
    pub fn bank_mut(&mut self) -> &mut Bank {
        if self.use_secondary {
            &mut self.secondary
        } else {
            &mut self.primary
        }
    }
    pub fn parse(
        &mut self,
        save_file: Option<&Json>,
        definitions: &Registry,
        timers: &mut Manager,
        ctx: &mut impl Context<Self>,
    ) -> Result<()> {
        let Some(save_file) = save_file.filter(|file| !matches!(file, Json::Null)) else {
            return Ok(());
        };
        object(save_file)?;
        let core = required(save_file, "core_data")?;
        object(core)?;
        let inventory = required(core, "inventory")?;
        array(inventory)?;
        let primary = required(core, "bank")?;
        array(primary)?;
        let empty = Json::Array(Vec::new());
        let secondary = core.get("bankSecondary").unwrap_or(&empty);
        array(secondary)?;
        let equipment = required(core, "equipment")?;
        array(equipment)?;
        let varps = core
            .get("varp")
            .filter(|value| matches!(value, Json::Array(_)));
        let timer_data = core
            .get("timers")
            .filter(|value| matches!(value, Json::Object(_)));
        let Json::Text(location) = required(core, "location")? else {
            return Err(Error::Type);
        };
        if let Some(tabs) = core
            .get("bankTabs")
            .filter(|value| !matches!(value, Json::Null))
        {
            parse_tabs(&mut self.primary.tabs, tabs)?;
        }
        if let Some(tabs) = core
            .get("bankTabsSecondary")
            .filter(|value| !matches!(value, Json::Null))
        {
            parse_tabs(&mut self.secondary.tabs, tabs)?;
        }
        self.use_secondary = match core.get("useSecondaryBank").unwrap_or(&Json::Bool(false)) {
            Json::Bool(value) => *value,
            Json::Null => return Err(Error::Null),
            _ => return Err(Error::Type),
        };
        self.inventory
            .parse(&container_json(inventory), definitions)?;
        self.primary
            .container
            .parse(&container_json(primary), definitions)?;
        self.secondary
            .container
            .parse(&container_json(secondary), definitions)?;
        self.equipment
            .parse(&container_json(equipment), definitions)?;
        self.location = Some(parse_location(location)?);
        if let Some(varps) = varps {
            for varp in array(varps)? {
                if matches!(varp, Json::Null) {
                    continue;
                }
                object(varp)?;
                let Some(index) = varp.get("index").filter(|v| !matches!(v, Json::Null)) else {
                    continue;
                };
                let index = integer(index)?;
                let Some(value) = varp.get("value").filter(|v| !matches!(v, Json::Null)) else {
                    continue;
                };
                let value = integer(value)?;
                self.varps.insert(index, value);
                self.save_varps.insert(index, Some(true));
            }
        }
        if let Some(timer_data) = timer_data {
            timers.parse(timer_data, self, ctx)?;
        }
        Ok(())
    }
    pub fn save(
        &mut self,
        root: &mut Json,
        timers: &mut Manager,
        ctx: &mut impl Context<Self>,
    ) -> Result<()> {
        object(root)?;
        let mut core = Json::object();
        put(&mut core, "inventory", save_container(&self.inventory))?;
        put(&mut core, "bank", save_container(&self.primary.container))?;
        put(
            &mut core,
            "bankSecondary",
            save_container(&self.secondary.container),
        )?;
        put(&mut core, "bankTabs", save_tabs(&self.primary.tabs))?;
        put(
            &mut core,
            "bankTabsSecondary",
            save_tabs(&self.secondary.tabs),
        )?;
        put(
            &mut core,
            "useSecondaryBank",
            Json::Bool(self.use_secondary),
        )?;
        put(&mut core, "equipment", save_container(&self.equipment))?;
        let [x, y, z] = self.location.as_ref().ok_or(Error::Null)?.coordinates();
        put(&mut core, "location", Json::Text(format!("{x},{y},{z}")))?;
        let mut varps = Vec::new();
        for index in self.varps.order() {
            if self.save_varps.get(&index).copied().flatten() != Some(true) {
                continue;
            }
            let value = self.varps.get(index).unwrap();
            if value == 0 {
                continue;
            }
            let mut varp = Json::object();
            put(&mut varp, "index", Json::Text(index.to_string()))?;
            put(&mut varp, "value", Json::Text(value.to_string()))?;
            varps.push(varp);
        }
        put(&mut core, "varp", Json::Array(varps))?;
        let mut saved = Json::object();
        timers.save(&mut saved, self, ctx)?;
        put(&mut core, "timers", saved)?;
        put(root, "core_data", core)
    }
}
fn object(value: &Json) -> Result<()> {
    if matches!(value, Json::Object(_)) {
        Ok(())
    } else {
        Err(if matches!(value, Json::Null) {
            Error::Null
        } else {
            Error::Type
        })
    }
}
fn array(value: &Json) -> Result<&[Json]> {
    if let Json::Array(values) = value {
        Ok(values)
    } else {
        Err(if matches!(value, Json::Null) {
            Error::Null
        } else {
            Error::Type
        })
    }
}
fn required<'a>(value: &'a Json, key: &str) -> Result<&'a Json> {
    value
        .get(key)
        .filter(|value| !matches!(value, Json::Null))
        .ok_or(Error::Null)
}
fn integer(value: &Json) -> Result<i32> {
    parse_i32(&value.text()).ok_or(Error::Number)
}
fn parse_tabs(tabs: &mut [i32; BANK_TABS], value: &Json) -> Result<()> {
    for tab in array(value)? {
        if matches!(tab, Json::Null) {
            continue;
        }
        object(tab)?;
        let index = integer(tab.get("index").unwrap_or(&Json::Null))?;
        let start = integer(tab.get("startSlot").unwrap_or(&Json::Null))?;
        *tabs.get_mut(index as usize).ok_or(Error::Index)? = start;
    }
    Ok(())
}
fn parse_location(text: &str) -> Result<AttributeLocation> {
    let values = text
        .split(',')
        .map(|part| parse_i32(part).ok_or(Error::Number))
        .collect::<Result<Vec<_>>>()?;
    Ok(AttributeLocation::new(
        *values.first().ok_or(Error::Index)?,
        *values.get(1).ok_or(Error::Index)?,
        *values.get(2).ok_or(Error::Index)?,
    ))
}
fn put(root: &mut Json, key: &str, value: Json) -> Result<()> {
    root.put(key, value).map_err(|_| Error::Type)?;
    Ok(())
}
fn save_tabs(tabs: &[i32; BANK_TABS]) -> Json {
    Json::Array(
        tabs.iter()
            .enumerate()
            .map(|(index, start)| {
                let mut row = Json::object();
                row.put("index", Json::Text(index.to_string())).unwrap();
                row.put("startSlot", Json::Text(start.to_string())).unwrap();
                row
            })
            .collect(),
    )
}
pub fn save_container(container: &Container) -> Json {
    Json::Array(
        container
            .items()
            .read()
            .iter()
            .flatten()
            .map(|item| {
                let item = item.read();
                let mut row = Json::object();
                for (key, value) in [
                    ("slot", item.index),
                    ("id", item.id()),
                    ("amount", item.amount()),
                    ("charge", item.charge()),
                ] {
                    row.put(key, Json::Text(value.to_string())).unwrap();
                }
                row
            })
            .collect(),
    )
}
/// Container's Java parser stringifies numeric fields before parseInt. Preserve
/// nonfinite primitive doubles as that text; serde_json cannot store them.
fn container_json(value: &Json) -> Value {
    match value {
        Json::Null => Value::Null,
        Json::Bool(value) => Value::Bool(*value),
        Json::Int(value) => (*value).into(),
        Json::Double(value) => serde_json::Number::from_f64(*value).map_or_else(
            || Value::String(rs09_game_core::java_double::format(*value)),
            Value::Number,
        ),
        Json::Text(value) => Value::String(value.clone()),
        Json::Array(values) => Value::Array(values.iter().map(container_json).collect()),
        Json::Object(entries) => Value::Object(
            entries
                .iter()
                .map(|(key, value)| (key.clone(), container_json(value)))
                .collect(),
        ),
    }
}
