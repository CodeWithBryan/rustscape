use super::{
    Error, Result,
    container_listener::Handle as Listener,
    item::{Handle, Item},
};
use crate::definitions::item::{Registry, Shared, WeakShared};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
pub type Items = Shared<Vec<Option<Handle>>>;
pub type Event = Shared<ContainerEvent>;
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ContainerType {
    Default,
    Shop,
    AlwaysStack,
    NeverStack,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum SortType {
    Id,
    Hash,
}
#[derive(Debug)]
pub struct ContainerEvent {
    pub items: Items,
    pub clear: bool,
    null_item: Handle,
}
impl ContainerEvent {
    pub fn new(capacity: usize, null_item: Handle) -> Self {
        Self {
            items: Shared::new(vec![None; capacity]),
            clear: false,
            null_item,
        }
    }
    pub fn flag(&self, slot: i32, item: Option<Handle>) -> Result<()> {
        *self
            .items
            .write()
            .get_mut(slot as usize)
            .ok_or(Error::Bounds)? = item;
        Ok(())
    }
    pub fn flag_null(&self, slot: i32) -> Result<()> {
        self.flag(slot, Some(self.null_item.clone()))
    }
    pub fn slots(&self) -> Vec<usize> {
        self.items
            .read()
            .iter()
            .enumerate()
            .filter_map(|(index, item)| item.as_ref().map(|_| index))
            .collect()
    }
    pub fn change_count(&self) -> usize {
        self.items
            .read()
            .iter()
            .filter(|item| item.is_some())
            .count()
    }
    pub fn flag_empty(&mut self) {
        self.clear = true;
        self.items.write().fill(None);
    }
}
/// One factory per world models ContainerEvent's single shared NULL_ITEM.
/// Create it at the corresponding world initialization point, after definitions.
pub struct Factory {
    pub null_item: Handle,
}
impl Factory {
    pub fn new(definitions: &Registry) -> Self {
        Self {
            null_item: Shared::new(Item::new(0, 0, 1000, definitions)),
        }
    }
    pub fn create(&self, capacity: i32, kind: ContainerType, sort: SortType) -> Result<Container> {
        if !(0..=1_000_000).contains(&capacity) {
            return Err(Error::Bounds);
        }
        Ok(Container {
            fields: Shared::new(Fields {
                items: Shared::new(vec![None; capacity as usize]),
                capacity,
                kind,
                sort,
                event: Shared::new(ContainerEvent::new(
                    capacity as usize,
                    self.null_item.clone(),
                )),
                listeners: Listeners::default(),
                null_item: self.null_item.clone(),
                equipment: None,
                bank: None,
            }),
        })
    }
}
/// Listeners are actual native objects. Implementations may synchronously
/// mutate the container, retain event/item handles, recurse, or fail.
pub trait Context {
    /// Virtual two-argument add. Base one/three-argument overloads bypass this.
    fn equip(
        &mut self,
        _container: &mut Container,
        _item: Option<&Handle>,
        _fire: bool,
        _definitions: &Registry,
    ) -> Result<bool> {
        Err(Error::Binding(
            "EquipmentContainer requires owning-player services".into(),
        ))
    }
    fn update(
        &mut self,
        listener: Listener,
        container: &mut Container,
        event: Event,
        definitions: &Registry,
    ) -> Result<()>;
    fn refresh(
        &mut self,
        listener: Listener,
        container: &mut Container,
        definitions: &Registry,
    ) -> Result<()>;
    fn ground(
        &mut self,
        container: &mut Container,
        item: Option<Handle>,
        definitions: &Registry,
    ) -> Result<()>;
}
/// The actual exposed ArrayList identity, independent of its owning container.
#[derive(Clone)]
pub struct Listeners(Shared<ListenerFields>);
struct ListenerFields {
    values: Vec<Option<Listener>>,
    modifications: u32,
}
impl Default for Listeners {
    fn default() -> Self {
        Self(Shared::new(ListenerFields {
            values: Vec::new(),
            modifications: 0,
        }))
    }
}
impl Listeners {
    pub(crate) fn strong_count(&self) -> usize {
        self.0.strong_count()
    }
    pub fn identity(&self) -> usize {
        self.0.identity()
    }
    pub fn same(&self, other: &Self) -> bool {
        self.0.same(&other.0)
    }
    pub fn values(&self) -> Vec<Option<Listener>> {
        self.0.read().values.clone()
    }
    pub fn len(&self) -> usize {
        self.0.read().values.len()
    }
    pub fn is_empty(&self) -> bool {
        self.0.read().values.is_empty()
    }
    pub fn add(&self, value: Option<Listener>) {
        let mut f = self.0.write();
        f.values.push(value);
        f.modifications = f.modifications.wrapping_add(1);
    }
    pub fn get(&self, index: usize) -> Result<Option<Listener>> {
        self.0
            .read()
            .values
            .get(index)
            .cloned()
            .ok_or(Error::ListBounds)
    }
    pub fn remove(&self, index: usize) -> Result<Option<Listener>> {
        let mut f = self.0.write();
        if index >= f.values.len() {
            return Err(Error::ListBounds);
        }
        let value = f.values.remove(index);
        f.modifications = f.modifications.wrapping_add(1);
        Ok(value)
    }
    pub fn remove_first(&self, value: Option<Listener>) -> bool {
        let mut f = self.0.write();
        if let Some(i) = f.values.iter().position(|v| *v == value) {
            f.values.remove(i);
            f.modifications = f.modifications.wrapping_add(1);
            true
        } else {
            false
        }
    }
    pub fn set(&self, index: usize, value: Option<Listener>) -> Result<Option<Listener>> {
        let mut f = self.0.write();
        let slot = f.values.get_mut(index).ok_or(Error::ListBounds)?;
        Ok(std::mem::replace(slot, value))
    }
    pub fn clear(&self) {
        let mut f = self.0.write();
        f.values.clear();
        f.modifications = f.modifications.wrapping_add(1);
    }
    fn modifications(&self) -> u32 {
        self.0.read().modifications
    }
}
struct Fields {
    items: Items,
    capacity: i32,
    kind: ContainerType,
    sort: SortType,
    event: Event,
    listeners: Listeners,
    null_item: Handle,
    equipment: Option<Option<crate::world::player_walk::NodeHandle>>,
    bank: Option<Box<super::bank::Data>>,
}
/// Clone retains the exact original Container, including replacement arrays and
/// events. No state or listener-list lock spans a world/listener callback.
#[derive(Clone)]
pub struct Container {
    fields: Shared<Fields>,
}
#[derive(Clone)]
pub struct WeakContainer(WeakShared<Fields>);
impl WeakContainer {
    pub fn upgrade(&self) -> Option<Container> {
        self.0.upgrade().map(|fields| Container { fields })
    }
}
impl Container {
    pub(crate) fn bind_bank(&self, data: super::bank::Data) {
        self.fields.write().bank = Some(Box::new(data));
    }
    pub fn is_bank(&self) -> bool {
        self.fields.read().bank.is_some()
    }
    pub(crate) fn bank_read<T>(
        &self,
        operation: impl FnOnce(&super::bank::Data) -> T,
    ) -> Result<T> {
        let fields = self.fields.read();
        Ok(operation(fields.bank.as_deref().ok_or(Error::Type)?))
    }
    pub(crate) fn bank_mutate<T>(
        &self,
        operation: impl FnOnce(&mut super::bank::Data) -> T,
    ) -> Result<T> {
        let mut fields = self.fields.write();
        Ok(operation(fields.bank.as_deref_mut().ok_or(Error::Type)?))
    }
    pub(crate) fn bind_equipment(&self, player: Option<crate::world::player_walk::NodeHandle>) {
        self.fields.write().equipment = Some(player);
    }
    pub fn equipment_owner(&self) -> Option<Option<crate::world::player_walk::NodeHandle>> {
        self.fields.read().equipment.clone()
    }
    pub(crate) fn strong_count(&self) -> usize {
        self.fields.strong_count()
    }
    pub fn same(&self, other: &Self) -> bool {
        self.fields.same(&other.fields)
    }
    pub fn identity(&self) -> usize {
        self.fields.identity()
    }
    pub fn downgrade(&self) -> WeakContainer {
        WeakContainer(self.fields.downgrade())
    }
    pub fn keeping_from<T: std::any::Any + Send + Sync>(&self, owner: &Shared<T>) -> Self {
        Self {
            fields: self.fields.keeping_from(owner),
        }
    }
    pub fn items(&self) -> Items {
        self.fields.read().items.clone()
    }
    pub fn event(&self) -> Event {
        self.fields.read().event.clone()
    }
    pub fn capacity(&self) -> i32 {
        self.fields.read().capacity
    }
    pub fn kind(&self) -> ContainerType {
        self.fields.read().kind
    }
    pub fn sort(&self) -> SortType {
        self.fields.read().sort
    }
    fn null_item(&self) -> Handle {
        self.fields.read().null_item.clone()
    }
    fn set_items(&self, items: Items) {
        self.fields.write().items = items;
    }
    pub fn listener_list(&self) -> Listeners {
        self.fields.read().listeners.clone()
    }
    /// Value snapshot for diagnostics; listener_list retains the live list.
    pub fn listeners(&self) -> Vec<Option<Listener>> {
        self.listener_list().values()
    }
    pub fn register(&mut self, listener: Option<Listener>) {
        self.listener_list().add(listener);
    }
    pub fn remove_listener(&mut self, index: usize) -> Result<()> {
        self.listener_list().remove(index).map(|_| ())
    }
    pub fn unregister(&mut self, listener: Option<Listener>) -> bool {
        self.listener_list().remove_first(listener)
    }
    fn reset_event(&mut self) {
        let event = Shared::new(ContainerEvent::new(
            self.capacity() as usize,
            self.null_item(),
        ));
        self.fields.write().event = event;
    }
    fn fire(
        &mut self,
        refresh: bool,
        definitions: &Registry,
        ctx: &mut impl Context,
    ) -> Result<()> {
        let listeners = self.listener_list();
        let expected = listeners.modifications();
        let mut cursor = 0;
        while cursor != listeners.len() {
            if listeners.modifications() != expected {
                return Err(Error::ConcurrentModification);
            }
            let listener = listeners.get(cursor)?.ok_or(Error::Null)?;
            cursor += 1;
            if refresh {
                ctx.refresh(listener, self, definitions)?;
            } else {
                let event = self.event();
                ctx.update(listener, self, event, definitions)?;
            }
        }
        Ok(())
    }
    pub fn update(
        &mut self,
        force: Option<bool>,
        definitions: &Registry,
        ctx: &mut impl Context,
    ) -> Result<()> {
        let event_handle = self.event();
        let event = event_handle.read();
        let skip = if let Some(force) = force {
            event.change_count() < 1 && !force
        } else {
            event.change_count() < 1 && !event.clear
        };
        drop(event);
        if skip {
            return Ok(());
        }
        self.fire(false, definitions, ctx)?;
        if force.is_none() {
            self.event().write().clear = false;
        }
        self.reset_event();
        Ok(())
    }
    pub fn refresh(
        &mut self,
        listener: Option<Listener>,
        definitions: &Registry,
        ctx: &mut impl Context,
    ) -> Result<()> {
        if let Some(listener) = listener {
            ctx.refresh(listener, self, definitions)?;
        } else {
            self.fire(true, definitions, ctx)?;
        }
        self.reset_event();
        Ok(())
    }
    pub fn refresh_listener(
        &mut self,
        listener: Option<Listener>,
        definitions: &Registry,
        ctx: &mut impl Context,
    ) -> Result<()> {
        ctx.refresh(listener.ok_or(Error::Null)?, self, definitions)?;
        self.reset_event();
        Ok(())
    }
    pub fn get(&self, slot: i32) -> Option<Handle> {
        self.items()
            .read()
            .get(slot as usize)
            .and_then(Clone::clone)
    }
    pub fn get_new(&self, slot: i32, definitions: &Registry) -> Result<Handle> {
        Ok(self
            .items()
            .read()
            .get(slot as usize)
            .ok_or(Error::Bounds)?
            .clone()
            .unwrap_or_else(|| Shared::new(Item::new(0, 1, 1000, definitions))))
    }
    pub fn get_id(&self, slot: i32) -> Result<i32> {
        let items_handle = self.items();
        let items = items_handle.read();
        if slot >= items.len() as i32 {
            return Ok(-1);
        }
        Ok(items
            .get(slot as usize)
            .ok_or(Error::Bounds)?
            .as_ref()
            .map_or(-1, |item| item.read().id()))
    }
    pub fn get_as_id(&self, slot: i32) -> i32 {
        self.get(slot).map_or(0, |item| item.read().id())
    }
    fn set(&self, slot: i32, item: Option<Handle>) -> Result<()> {
        *self
            .items()
            .write()
            .get_mut(slot as usize)
            .ok_or(Error::Bounds)? = item;
        Ok(())
    }
    fn checked(&self, slot: i32) -> Result<Option<Handle>> {
        self.items()
            .read()
            .get(slot as usize)
            .cloned()
            .ok_or(Error::Bounds)
    }
    pub fn count(&self) -> i32 {
        self.items()
            .read()
            .iter()
            .filter(|item| item.is_some())
            .count() as i32
    }
    pub fn free_slots(&self) -> i32 {
        self.capacity().wrapping_sub(self.count())
    }
    pub fn free_slot(&self) -> i32 {
        self.items()
            .read()
            .iter()
            .position(Option::is_none)
            .map_or(-1, |slot| slot as i32)
    }
    pub fn empty(&self) -> bool {
        self.items().read().iter().all(Option::is_none)
    }
    pub fn full(&self) -> bool {
        self.free_slots() < 1
    }
    pub fn contains(&self, id: i32, amount: i32) -> bool {
        let mut count = 0i32;
        for item in self.items().read().iter().flatten() {
            let value = item.read();
            if value.id() == id {
                count = count.wrapping_add(value.amount());
                if count >= amount {
                    return true;
                }
            }
        }
        false
    }
    pub fn contains_item(&self, item: Option<&Handle>) -> Result<bool> {
        let item = item.ok_or(Error::Null)?.read();
        Ok(self.contains(item.id(), item.amount()))
    }
    pub fn contains_items(&self, items: &[Option<Handle>]) -> Result<bool> {
        for item in items {
            if !self.contains_item(item.as_ref())? {
                return Ok(false);
            }
        }
        Ok(true)
    }
    pub fn contains_one_id(&self, id: i32) -> bool {
        self.items().read().iter().flatten().any(|item| {
            let item = item.read();
            item.id() == id && item.amount() > 0
        })
    }
    pub fn contains_one_ids(&self, ids: &[i32], definitions: &Registry) -> bool {
        ids.iter().any(|id| self.amount_id(*id, definitions) > 0)
    }
    pub fn contains_one_items(&self, items: &[Option<Handle>]) -> Result<bool> {
        for item in items {
            if self.contains_item(item.as_ref())? {
                return Ok(true);
            }
        }
        Ok(false)
    }
    pub fn contains_all(&self, ids: &[i32]) -> bool {
        ids.iter().all(|id| self.contains_one_id(*id))
    }
    pub fn contain_items(&self, ids: &[i32]) -> bool {
        ids.iter().all(|id| self.contains(*id, 1))
    }
    pub fn amount(&self, item: Option<&Handle>) -> i32 {
        let Some(item) = item else {
            return 0;
        };
        let id = item.read().id();
        self.items()
            .read()
            .iter()
            .flatten()
            .fold(0i32, |count, item| {
                let item = item.read();
                if item.id() == id {
                    count.wrapping_add(item.amount())
                } else {
                    count
                }
            })
    }
    pub fn amount_id(&self, id: i32, definitions: &Registry) -> i32 {
        self.amount(Some(&Shared::new(Item::new(id, 1, 1000, definitions))))
    }
    pub fn slot(&self, item: Option<&Handle>, hash: bool) -> i32 {
        let Some(item) = item else {
            return -1;
        };
        let item = item.read();
        let key = if hash { item.id_hash() } else { item.id() };
        self.items()
            .read()
            .iter()
            .position(|entry| {
                entry.as_ref().is_some_and(|entry| {
                    let entry = entry.read();
                    if hash {
                        entry.id_hash() == key
                    } else {
                        entry.id() == key
                    }
                })
            })
            .map_or(-1, |index| index as i32)
    }
    pub fn get_item(&self, item: Option<&Handle>) -> Option<Handle> {
        self.get(self.slot(item, false))
    }
    pub fn get_all(&self, item: Option<&Handle>) -> Result<Vec<Handle>> {
        let mut output = Vec::new();
        for entry in self.items().read().iter().flatten() {
            if item.ok_or(Error::Null)?.read().id() == entry.read().id() {
                output.push(entry.clone());
            }
        }
        Ok(output)
    }
    pub fn get_by_item(&self, item: Option<&Handle>) -> Result<Option<Handle>> {
        for entry in self.items().read().iter().flatten() {
            if item.ok_or(Error::Null)?.read().id() == entry.read().id() {
                return Ok(Some(entry.clone()));
            }
        }
        Ok(None)
    }
    fn stacks(&self, item: &Handle) -> Result<bool> {
        Ok(self.kind() != ContainerType::NeverStack
            && (item.read().stackable()?
                || matches!(
                    self.kind(),
                    ContainerType::AlwaysStack | ContainerType::Shop
                )))
    }
    pub fn maximum_add(&self, item: Option<&Handle>) -> Result<i32> {
        if self.is_bank() {
            let definitions = self.bank_read(|data| data.definitions.clone())?;
            let item = super::item::unnoted(item.ok_or(Error::Null)?, &definitions)?;
            return self.base_maximum_add(Some(&item));
        }
        self.base_maximum_add(item)
    }
    pub(crate) fn base_maximum_add(&self, item: Option<&Handle>) -> Result<i32> {
        if self.kind() != ContainerType::NeverStack {
            let item = item.ok_or(Error::Null)?;
            if item.read().stackable()?
                || matches!(
                    self.kind(),
                    ContainerType::AlwaysStack | ContainerType::Shop
                )
            {
                if self.contains(item.read().id(), 1) {
                    return Ok(i32::MAX.wrapping_sub(self.amount(Some(item))));
                }
                return Ok(if self.free_slots() > 0 { i32::MAX } else { 0 });
            }
        }
        Ok(self.free_slots())
    }
    pub fn add_slot(&self, item: Option<&Handle>) -> Result<i32> {
        if self.kind() != ContainerType::NeverStack {
            let item = item.ok_or(Error::Null)?;
            if self.stacks(item)? {
                let slot = self.slot(Some(item), self.sort() == SortType::Hash);
                if slot != -1 {
                    return Ok(slot);
                }
            }
        }
        Ok(self.free_slot())
    }
    pub fn add(
        &mut self,
        item: Option<&Handle>,
        fire: bool,
        mut preferred: i32,
        definitions: &Registry,
        ctx: &mut impl Context,
    ) -> Result<bool> {
        let item = Shared::new(item.ok_or(Error::Null)?.read().copy(definitions));
        let maximum = self.maximum_add(Some(&item))?;
        if maximum == 0 {
            return Ok(false);
        }
        if preferred > -1 && self.checked(preferred)?.is_some() {
            preferred = -1;
        }
        if item.read().amount() > maximum {
            item.write().set_amount(maximum);
        }
        if self.stacks(&item)? {
            let old_slot = self.slot(Some(&item), self.sort() == SortType::Hash);
            if old_slot != -1 {
                let old = self.checked(old_slot)?.unwrap();
                let value = old.read();
                let input = item.read();
                let new = Shared::new(Item::new(
                    value.id(),
                    input.amount().wrapping_add(value.amount()),
                    input.charge(),
                    definitions,
                ));
                drop(input);
                drop(value);
                self.set(old_slot, Some(new.clone()))?;
                new.write().index = old_slot;
                self.event().read().flag(old_slot, Some(new))?;
            } else {
                let slot = if preferred > -1 {
                    preferred
                } else {
                    self.free_slot()
                };
                if slot == -1 {
                    return Ok(false);
                }
                self.set(slot, Some(item.clone()))?;
                item.write().index = slot;
                self.event().read().flag(slot, Some(item))?;
            }
            if fire {
                self.update(None, definitions, ctx)?;
            }
            return Ok(true);
        }
        if self.free_slots() >= item.read().amount() {
            let amount = item.read().amount();
            for index in 0..amount {
                let slot = if index == 0 && preferred > -1 {
                    preferred
                } else {
                    self.free_slot()
                };
                let input = item.read();
                let new = Shared::new(Item::new(input.id(), 1, input.charge(), definitions));
                drop(input);
                self.set(slot, Some(new.clone()))?;
                new.write().index = slot;
                self.event().read().flag(slot, Some(new))?;
            }
            if fire {
                self.update(None, definitions, ctx)?;
            }
            return Ok(true);
        }
        Ok(false)
    }
    pub fn add_two(
        &mut self,
        item: Option<&Handle>,
        fire: bool,
        definitions: &Registry,
        ctx: &mut impl Context,
    ) -> Result<bool> {
        if self.equipment_owner().is_some() {
            ctx.equip(self, item, fire, definitions)
        } else {
            self.add(item, fire, -1, definitions, ctx)
        }
    }
    pub fn add_many(
        &mut self,
        items: &Items,
        definitions: &Registry,
        ctx: &mut impl Context,
    ) -> Result<bool> {
        let length = items.read().len();
        let mut all = true;
        for index in 0..length {
            let item = items.read()[index].clone();
            if item.is_none() {
                continue;
            }
            if !self.add_two(item.as_ref(), false, definitions, ctx)? {
                all = false;
                break;
            }
        }
        self.update(None, definitions, ctx)?;
        Ok(all)
    }
    pub fn add_list(
        &mut self,
        items: &[Option<Handle>],
        definitions: &Registry,
        ctx: &mut impl Context,
    ) -> Result<()> {
        for item in items.iter().flatten() {
            self.add(Some(item), true, -1, definitions, ctx)?;
        }
        self.update(None, definitions, ctx)
    }
    pub fn add_if_absent(
        &mut self,
        item: Option<&Handle>,
        definitions: &Registry,
        ctx: &mut impl Context,
    ) -> Result<bool> {
        if self.contains_item(item)? {
            Ok(false)
        } else {
            self.add(item, true, -1, definitions, ctx)
        }
    }
    pub fn add_or_ground(
        &mut self,
        item: Option<&Handle>,
        definitions: &Registry,
        ctx: &mut impl Context,
    ) -> Result<bool> {
        if !self.add(item, true, -1, definitions, ctx)? {
            ctx.ground(self, item.cloned(), definitions)?;
            return Ok(false);
        }
        Ok(true)
    }
    pub fn remove(
        &mut self,
        item: Option<&Handle>,
        slot: Option<i32>,
        fire: bool,
        definitions: &Registry,
        ctx: &mut impl Context,
    ) -> Result<bool> {
        let slot = if let Some(slot) = slot {
            slot
        } else {
            let slot = self.slot(item, false);
            if slot == -1 {
                return Ok(false);
            }
            slot
        };
        let input = item.ok_or(Error::Null)?;
        let (id, amount) = {
            let input = input.read();
            (input.id(), input.amount())
        };
        if !self.contains(id, amount) {
            return Ok(false);
        }
        let Some(old) = self.checked(slot)? else {
            return Ok(false);
        };
        if old.read().id() != id {
            return Ok(false);
        }
        if amount < 1 {
            return Ok(true);
        }
        if old.read().stackable()?
            || matches!(
                self.kind(),
                ContainerType::AlwaysStack | ContainerType::Shop
            )
        {
            if amount >= old.read().amount() {
                self.set(slot, None)?;
                self.event().read().flag_null(slot)?;
            } else {
                let new = Shared::new(Item::new(
                    id,
                    old.read().amount().wrapping_sub(amount),
                    input.read().charge(),
                    definitions,
                ));
                self.set(slot, Some(new.clone()))?;
                new.write().index = slot;
                self.event().read().flag(slot, Some(new))?;
            }
        } else {
            self.set(slot, None)?;
            self.event().read().flag_null(slot)?;
            for _ in 1..amount {
                let slot = self.slot(Some(input), false);
                if slot == -1 {
                    break;
                }
                self.set(slot, None)?;
                self.event().read().flag_null(slot)?;
            }
        }
        if fire {
            self.update(None, definitions, ctx)?;
        }
        Ok(true)
    }
    pub fn remove_many(
        &mut self,
        items: &Items,
        definitions: &Registry,
        ctx: &mut impl Context,
    ) -> Result<bool> {
        let length = items.read().len();
        let mut all = true;
        for index in 0..length {
            let item = items.read()[index].clone();
            if !self.remove(item.as_ref(), None, false, definitions, ctx)? {
                all = false;
            }
        }
        self.update(None, definitions, ctx)?;
        Ok(all)
    }
    pub fn remove_all(
        &mut self,
        id: i32,
        definitions: &Registry,
        ctx: &mut impl Context,
    ) -> Result<bool> {
        let items: Vec<_> = self
            .items()
            .read()
            .iter()
            .flatten()
            .filter(|item| item.read().id() == id)
            .cloned()
            .collect();
        let mut result = true;
        for item in items {
            if !self.remove(Some(&item), None, false, definitions, ctx)? {
                result = false;
            }
        }
        Ok(result)
    }
    pub fn remove_all_ids(
        &mut self,
        ids: &[i32],
        definitions: &Registry,
        ctx: &mut impl Context,
    ) -> Result<bool> {
        let mut all = true;
        for id in ids {
            if !self.remove_all(*id, definitions, ctx)? {
                all = false;
            }
        }
        self.update(None, definitions, ctx)?;
        Ok(all)
    }
    pub fn replace(
        &mut self,
        item: Option<&Handle>,
        slot: i32,
        fire: bool,
        definitions: &Registry,
        ctx: &mut impl Context,
    ) -> Result<Option<Handle>> {
        let item = match item {
            Some(item) if item.read().amount() >= 1 || self.kind() == ContainerType::Shop => {
                Some(Shared::new(item.read().copy(definitions)))
            }
            _ => None,
        };
        let old = self.checked(slot)?;
        self.set(slot, item.clone())?;
        if let Some(item) = item {
            item.write().index = slot;
            self.event().read().flag(slot, Some(item))?;
        } else {
            self.event().read().flag_null(slot)?;
        }
        if fire {
            self.update(None, definitions, ctx)?;
        }
        Ok(old)
    }
    pub fn insert(
        &mut self,
        from: i32,
        to: i32,
        fire: bool,
        definitions: &Registry,
        ctx: &mut impl Context,
    ) -> Result<()> {
        let temp = self.checked(from)?;
        match to.cmp(&from) {
            std::cmp::Ordering::Greater => {
                for index in from..to {
                    let item = self.get(index + 1);
                    self.replace(item.as_ref(), index, false, definitions, ctx)?;
                }
            }
            std::cmp::Ordering::Less => {
                let mut index = from;
                while index > to {
                    let item = self.get(index - 1);
                    self.replace(item.as_ref(), index, false, definitions, ctx)?;
                    index -= 1;
                }
            }
            std::cmp::Ordering::Equal => {}
        }
        self.replace(temp.as_ref(), to, fire, definitions, ctx)?;
        Ok(())
    }
    pub fn clear(
        &mut self,
        refresh: bool,
        definitions: &Registry,
        ctx: &mut impl Context,
    ) -> Result<()> {
        self.set_items(Shared::new(vec![None; self.capacity() as usize]));
        self.event().write().flag_empty();
        if refresh {
            self.refresh(None, definitions, ctx)?;
        }
        Ok(())
    }
    pub fn shift(&mut self, definitions: &Registry, ctx: &mut impl Context) -> Result<()> {
        let old = self.items();
        self.clear(false, definitions, ctx)?;
        for index in 0..old.read().len() {
            let item = old.read()[index].clone();
            if item.is_some() {
                self.add_two(item.as_ref(), false, definitions, ctx)?;
            }
        }
        self.refresh(None, definitions, ctx)
    }
    pub fn copy_items(&mut self, source: &Items, definitions: &Registry) {
        self.set_items(Shared::new(vec![None; source.read().len()]));
        for index in 0..source.read().len() {
            if let Some(item) = &source.read()[index] {
                let new = Shared::new(item.read().copy(definitions));
                new.write().index = index as i32;
                self.items().write()[index] = Some(new);
            }
        }
    }
    /// Original Container.copy receiver semantics, including another handle to
    /// this same object. Capturing its old array first would change self-copy.
    pub fn copy_from(&mut self, source: &Self, definitions: &Registry) {
        if self.same(source) {
            self.copy_self();
        } else {
            self.copy_items(&source.items(), definitions);
        }
    }
    pub fn copy_self(&mut self) {
        let length = self.items().read().len();
        self.set_items(Shared::new(vec![None; length]));
    }
    pub fn has_space_for(&self, item: Option<&Handle>) -> Result<bool> {
        let item = item.ok_or(Error::Null)?;
        let amount = item.read().amount();
        Ok(amount <= self.maximum_add(Some(item))?)
    }
    pub fn has_space_for_container(
        &self,
        items: Option<&Items>,
        definitions: &Registry,
        ctx: &mut impl Context,
    ) -> Result<bool> {
        let Some(items) = items else {
            return Ok(false);
        };
        let factory = Factory {
            null_item: self.null_item(),
        };
        let mut check = factory.create(self.capacity(), self.kind(), SortType::Id)?;
        check.add_many(&self.items(), definitions, ctx)?;
        for index in 0..items.read().len() {
            let item = items.read()[index].clone();
            if item.is_some() && !check.add(item.as_ref(), false, -1, definitions, ctx)? {
                return Ok(false);
            }
        }
        Ok(true)
    }
    pub fn has_space_for_many(
        &self,
        items: &Items,
        definitions: &Registry,
        ctx: &mut impl Context,
    ) -> Result<bool> {
        let factory = Factory {
            null_item: self.null_item(),
        };
        let mut source = factory.create(28, ContainerType::Default, SortType::Id)?;
        source.add_many(items, definitions, ctx)?;
        self.has_space_for_container(Some(&source.items()), definitions, ctx)
    }
    pub fn wealth(&self) -> Result<i32> {
        let mut wealth = 0i32;
        for item in self.items().read().iter().flatten() {
            let item = item.read();
            wealth = wealth.wrapping_add(
                item.definition
                    .as_ref()
                    .ok_or(Error::Null)?
                    .read()
                    .value
                    .wrapping_mul(item.amount()),
            );
        }
        Ok(wealth)
    }
    pub fn format(&self) -> String {
        let mut values = BTreeMap::new();
        let mut keys = Vec::new();
        for item in self.items().read().iter().flatten() {
            let item = item.read();
            keys.push(item.id());
            let old = values.entry(item.id()).or_insert(0i32);
            *old = old.wrapping_add(item.amount());
        }
        use std::fmt::Write;
        let mut text = String::new();
        for key in rs09_game_core::java_map::int_key_order(keys) {
            if !text.is_empty() {
                text.push('|');
            }
            write!(text, "{key},{}", values[&key]).unwrap();
        }
        text
    }
    pub fn parse(&mut self, value: &Value, definitions: &Registry) -> Result<()> {
        let rows = value.as_array().ok_or(if value.is_null() {
            Error::Null
        } else {
            Error::Type
        })?;
        let mut total = 0i32;
        for row in rows {
            let row = row.as_object().ok_or(if row.is_null() {
                Error::Null
            } else {
                Error::Type
            })?;
            let read = |key: &str| -> Result<i32> {
                let value = row
                    .get(key)
                    .filter(|value| !value.is_null())
                    .ok_or(Error::Null)?;
                let text = match value {
                    Value::String(value) => value.clone(),
                    Value::Number(n) if n.is_f64() => {
                        rs09_game_core::java_double::format(n.as_f64().unwrap())
                    }
                    _ => value.to_string(),
                };
                rs09_game_core::java_string::parse_i32(&text).ok_or(Error::Number)
            };
            let slot = read("slot")?;
            let id = read("id")?;
            let amount = read("amount")?;
            let charge = read("charge")?;
            if id >= definitions.definitions().len() as i32
                || id < 0
                || slot >= self.items().read().len() as i32
                || id == 5614
            {
                continue;
            }
            let item = Shared::new(Item::new(id, amount, charge, definitions));
            self.set(slot, Some(item.clone()))?;
            item.write().index = slot;
            total = total.wrapping_add(item.read().value(definitions)? as i32);
        }
        let _ = total;
        Ok(())
    }
}
