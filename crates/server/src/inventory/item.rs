use super::{Error, Result};
use crate::definitions::item::{Handle as Definition, Registry, Shared};
use std::collections::BTreeSet;

pub type Handle = Shared<Item>;
/// Item-owned state only. Node positioning, interaction dispatch and concrete
/// item plugins bind through the player/world layer rather than being emulated.
#[derive(Debug)]
pub struct Item {
    id_hash: i32,
    amount: i32,
    pub name: Option<String>,
    pub index: i32,
    pub item_destination: bool,
    pub definition: Option<Definition>,
}
impl Default for Item {
    fn default() -> Self {
        Self {
            id_hash: (-1i32 << 16) | 1000,
            amount: 0,
            name: Some("null".into()),
            index: 0,
            item_destination: false,
            definition: None,
        }
    }
}
impl Item {
    pub fn new(id: i32, amount: i32, charge: i32, definitions: &Registry) -> Self {
        let name = definitions.for_id(id).read().name.clone();
        Self {
            id_hash: (id << 16) | charge,
            amount,
            name,
            index: -1,
            item_destination: true,
            definition: Some(definitions.for_id(id)),
        }
    }
    pub fn id(&self) -> i32 {
        (self.id_hash >> 16) & 65535
    }
    pub fn charge(&self) -> i32 {
        self.id_hash & 65535
    }
    pub fn id_hash(&self) -> i32 {
        self.id_hash
    }
    pub fn amount(&self) -> i32 {
        self.amount
    }
    pub fn set_amount(&mut self, amount: i32) {
        self.amount = amount.max(0);
    }
    pub fn set_id(&mut self, id: i32, definitions: &Registry) {
        self.id_hash = (id << 16) | (self.id_hash & 65535);
        self.definition = Some(definitions.for_id(id));
    }
    pub fn set_charge(&mut self, charge: i32) {
        self.id_hash = (((self.id_hash >> 16) & 65535) << 16) | charge;
    }
    pub fn charged(&self) -> bool {
        self.charge() > 0
    }
    pub fn copy(&self, definitions: &Registry) -> Self {
        Self::new(self.id(), self.amount, self.charge(), definitions)
    }
    pub fn stackable(&self) -> Result<bool> {
        Ok(self
            .definition
            .as_ref()
            .ok_or(Error::Null)?
            .read()
            .is_stackable())
    }
    pub fn note_change(&self) -> Result<i32> {
        let id = self.definition.as_ref().ok_or(Error::Null)?.read().note_id;
        Ok(if id > -1 { id } else { self.id() })
    }
    pub fn value(&self, definitions: &Registry) -> Result<i64> {
        let def = self.definition.as_ref().ok_or(Error::Null)?;
        let mut value = 1i64;
        let raw = def.read().value;
        if i64::from(raw) > value {
            value = i64::from(def.read().value);
        }
        if i64::from(definition_alchemy(def, true, definitions)?) > value {
            value = i64::from(definition_alchemy(def, true, definitions)?);
        }
        Ok(value.wrapping_mul(i64::from(self.amount)))
    }
    pub fn alchemy_value(&self, definitions: &Registry) -> Result<i64> {
        let def = self.definition.as_ref().ok_or(Error::Null)?;
        let mut value = 1i64;
        if i64::from(definition_alchemy(def, true, definitions)?) > value {
            value = i64::from(definition_alchemy(def, true, definitions)?);
        }
        Ok(value.wrapping_mul(i64::from(self.amount)))
    }
    pub fn text(&self) -> String {
        format!(
            "Item id={}, name={}, amount={}",
            self.id(),
            self.name.as_deref().unwrap_or("null"),
            self.amount
        )
    }
}
/// Explicit integration contracts for the original degradation and plugin lookups.
pub trait Bindings {
    type Handler;
    type Plugin;
    fn drop_replacement(&mut self, id: i32) -> Result<i32>;
    fn operate_handler(&mut self, id: i32) -> Result<Option<Self::Handler>>;
    fn plugin(&mut self, definition: &Definition) -> Result<Option<Self::Plugin>>;
}
pub fn drop_item(
    item: &Handle,
    definitions: &Registry,
    bindings: &mut impl Bindings,
) -> Result<Handle> {
    let id = item.read().id();
    let replacement = bindings.drop_replacement(id)?;
    if replacement != item.read().id() {
        Ok(Shared::new(Item::new(
            replacement,
            item.read().amount(),
            1000,
            definitions,
        )))
    } else {
        Ok(item.clone())
    }
}
pub fn unnoted(item: &Handle, definitions: &Registry) -> Result<Handle> {
    let definition = item.read().definition.clone().ok_or(Error::Null)?;
    let plain = definition.read().unnoted;
    if plain {
        return Ok(item.clone());
    }
    let id = definition.read().note_id;
    let value = item.read();
    Ok(Shared::new(Item::new(
        id,
        value.amount(),
        value.charge(),
        definitions,
    )))
}
pub fn operate<B: Bindings>(item: &Handle, bindings: &mut B) -> Result<Option<B::Handler>> {
    let id = item.read().id();
    bindings.operate_handler(id)
}
pub fn plugin<B: Bindings>(item: &Handle, bindings: &mut B) -> Result<Option<B::Plugin>> {
    let definition = item.read().definition.clone();
    match definition {
        None => Ok(None),
        Some(definition) => bindings.plugin(&definition),
    }
}
pub fn has_plugin<B: Bindings>(item: &Handle, bindings: &mut B) -> Result<bool> {
    Ok(plugin(item, bindings)?.is_some())
}
/// Original ItemDefinition.isTradeable, including the case-sensitive impling
/// jar exception and getConfiguration's null-as-default Boolean behavior.
pub fn definition_tradeable(definition: &Definition) -> Result<bool> {
    use crate::definitions::property::Property;
    let definition = definition.read();
    if definition.has_destroy_action()
        && !definition
            .name
            .as_ref()
            .ok_or(Error::Null)?
            .contains("impling jar")
    {
        return Ok(false);
    }
    match definition.handlers.get("tradeable") {
        None | Some(Property::Null) => Ok(false),
        Some(Property::Bool(value)) => Ok(*value),
        _ => Err(Error::Type),
    }
}
pub fn definition_alchemy(
    definition: &Definition,
    high: bool,
    definitions: &Registry,
) -> Result<i32> {
    let mut current = definition.clone();
    let mut seen = BTreeSet::new();
    loop {
        if !seen.insert(current.identity()) {
            return Err(Error::CyclicNote);
        }
        let def = current.read();
        if !def.unnoted && def.note_id > -1 {
            let next = definitions.for_id(def.note_id);
            drop(def);
            current = next;
            continue;
        }
        let default =
            (f64::from(def.value) * if high { 0.6 } else { 0.4 }).round_ties_even() as i32;
        return match def
            .handlers
            .get(if high { "high_alchemy" } else { "low_alchemy" })
            .filter(|v| !v.is_null())
        {
            None => Ok(default),
            Some(value) => value
                .as_i64()
                .and_then(|v| v.try_into().ok())
                .ok_or(Error::Type),
        };
    }
}
