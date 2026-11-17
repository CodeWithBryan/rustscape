//! Complete BankContainer control flow over actual shared native containers.
//! Player/interface/var/content services are explicit retained receivers.
use super::{
    Error, Result,
    container::{self, Container, ContainerType, Event, Factory, Items, SortType},
    container_listener::{Handle as ListenerHandle, NativeListener},
    item::{self, Handle, Item},
};
use crate::{
    definitions::{
        item::{Registry, Shared},
        property::Property,
    },
    world::player_walk::NodeHandle,
};
use rs09_protocol::game::{
    Frame,
    updates::{self, WireItem},
};
use std::sync::Arc;

pub const SIZE: i32 = 496;
pub const TAB_SIZE: usize = 11;
pub const INSERT_MODE: i32 = 7000;
pub const NOTE_MODE: i32 = 7001;
const INVENTORY_SETTINGS: i32 = 126 | (1 << 9) | (1 << 21);

pub(crate) struct Data {
    pub definitions: Arc<Registry>,
    pub player: Option<NodeHandle>,
    pub listener: ListenerHandle,
    open: bool,
    last_amount_x: i32,
    tab_index: i32,
    tabs: Shared<Vec<i32>>,
}
#[derive(Clone)]
pub struct Bank(Container);
impl Bank {
    pub fn new(
        factory: &Factory,
        definitions: Arc<Registry>,
        player: Option<NodeHandle>,
    ) -> Result<Self> {
        let mut container = factory.create(SIZE, ContainerType::AlwaysStack, SortType::Hash)?;
        let listener = ListenerHandle::new(Listener {
            player: player.clone(),
        });
        container.register(Some(listener.clone()));
        container.bind_bank(Data {
            definitions,
            player,
            listener,
            open: false,
            last_amount_x: 50,
            tab_index: 10,
            tabs: Shared::new(vec![0; TAB_SIZE]),
        });
        Ok(Self(container))
    }
    pub fn from_container(container: Container) -> Result<Self> {
        if !container.is_bank() {
            return Err(Error::Type);
        }
        Ok(Self(container))
    }
    pub fn container(&self) -> Container {
        self.0.clone()
    }
    pub fn same(&self, other: &Self) -> bool {
        self.0.same(&other.0)
    }
    pub fn owner(&self) -> Option<NodeHandle> {
        self.0.bank_read(|data| data.player.clone()).unwrap()
    }
    pub fn listener(&self) -> ListenerHandle {
        self.0.bank_read(|data| data.listener.clone()).unwrap()
    }
    pub fn tab_start_slots(&self) -> Shared<Vec<i32>> {
        self.0.bank_read(|data| data.tabs.clone()).unwrap()
    }
    pub fn tab_index(&self) -> i32 {
        self.0.bank_read(|data| data.tab_index).unwrap()
    }
    pub fn last_amount_x(&self) -> i32 {
        self.0.bank_read(|data| data.last_amount_x).unwrap()
    }
    pub fn is_open(&self) -> bool {
        self.0.bank_read(|data| data.open).unwrap()
    }
    fn player(&self) -> Result<NodeHandle> {
        self.owner().ok_or(Error::Null)
    }
    fn definitions(&self) -> Arc<Registry> {
        self.0.bank_read(|data| data.definitions.clone()).unwrap()
    }
    fn tab(&self, index: i32) -> Result<i32> {
        self.tab_start_slots()
            .read()
            .get(index as usize)
            .copied()
            .ok_or(Error::Bounds)
    }
    fn set_tab(&self, index: i32, value: i32) -> Result<()> {
        *self
            .tab_start_slots()
            .write()
            .get_mut(index as usize)
            .ok_or(Error::Bounds)? = value;
        Ok(())
    }
    pub fn open_deposit_box<C: Context>(&self, ctx: &mut C) -> Result<()> {
        let interfaces = ctx.interfaces(&self.player()?)?;
        let component = ctx.component(11)?;
        let component = ctx.open(interfaces, component)?;
        ctx.close_event(component, CloseEvent::Deposit)?;
        let interfaces = ctx.interfaces(&self.player()?)?;
        ctx.remove_tabs(interfaces, &[0, 1, 2, 3, 4, 5, 6])?;
        self.refresh_deposit_box(ctx)
    }
    pub fn refresh_deposit_box<C: Context>(&self, ctx: &mut C) -> Result<()> {
        let inventory = ctx.inventory(&self.player()?)?;
        ctx.deposit_items(
            &self.player()?,
            inventory.items(),
            &[
                "Examine",
                "Deposit-X",
                "Deposit-All",
                "Deposit-10",
                "Deposit-5",
                "Deposit-1",
            ],
            [11, 15, 5, 7],
        )
    }
    pub fn open_bank<C: Context>(&self, ctx: &mut C) -> Result<()> {
        self.open_inner(None, ctx)
    }
    pub fn open_for<C: Context>(&self, player: Option<&NodeHandle>, ctx: &mut C) -> Result<()> {
        self.open_inner(Some(player), ctx)
    }
    fn open_inner<C: Context>(
        &self,
        supplied: Option<Option<&NodeHandle>>,
        ctx: &mut C,
    ) -> Result<()> {
        if self.is_open() {
            return Ok(());
        }
        let own = self.owner();
        let player = supplied.unwrap_or(own.as_ref()).ok_or(Error::Null)?;
        let ironman = ctx.ironman(player)?;
        if ctx.ultimate_restriction(ironman)? {
            return Ok(());
        }
        let pin = ctx.pin(player)?;
        if !ctx.unlocked(pin)? && !ctx.dev_mode()? {
            let pin = ctx.pin(player)?;
            ctx.open_pin(pin, 1)?;
            return Ok(());
        }
        let interfaces = ctx.interfaces(player)?;
        let component = ctx.open_component(interfaces, 762)?;
        ctx.close_event(component, CloseEvent::Bank(self.clone()))?;
        if supplied.is_some() {
            let dispatch = ctx.dispatch(player)?;
            ctx.iface_settings(dispatch, [1278, 73, 762, 0, SIZE])?;
        }
        let dispatch = ctx.dispatch(player)?;
        ctx.iface_settings(dispatch, [INVENTORY_SETTINGS, 0, 763, 0, 27])?;
        if supplied.is_some() {
            let dispatch = ctx.dispatch(player)?;
            ctx.script(dispatch, 1451)?;
        }
        ctx.bank(player)?.set_tab_configurations(ctx)?;
        ctx.bank(player)?.send_bank_space(ctx)?;
        let interfaces = ctx.interfaces(player)?;
        let component = ctx.component(763)?;
        ctx.open_single_tab(interfaces, component)?;
        let mut container = self.container();
        let defs = self.definitions();
        if supplied.is_some() {
            container.refresh_listener(Some(self.listener()), &defs, ctx)?;
        } else {
            container.refresh(None, &defs, ctx)?;
        }
        ctx.inventory(player)?.refresh(None, &defs, ctx)?;
        let inventory = ctx.inventory(player)?;
        let listener = if supplied.is_some() {
            ctx.bank(player)?.listener()
        } else {
            self.listener()
        };
        inventory.listener_list().add(Some(listener));
        ctx.set_varp(player, 1249, self.last_amount_x())?;
        self.0.bank_mutate(|data| data.open = true)
    }
    pub fn close<C: Context>(&self, ctx: &mut C) -> Result<()> {
        self.0.bank_mutate(|data| data.open = false)?;
        let inventory = ctx.inventory(&self.player()?)?;
        inventory
            .listener_list()
            .remove_first(Some(self.listener()));
        let interfaces = ctx.interfaces(&self.player()?)?;
        ctx.close_single_tab(interfaces)?;
        ctx.remove_attribute(&self.player()?, "search")?;
        let dispatch = ctx.dispatch(&self.player()?)?;
        ctx.script(dispatch, 571)
    }
    pub fn add_item<C: Context>(&self, slot: i32, mut amount: i32, ctx: &mut C) -> Result<()> {
        if slot < 0 || slot > ctx.inventory(&self.player()?)?.capacity() || amount < 1 {
            return Ok(());
        }
        let Some(mut item) = ctx.inventory(&self.player()?)?.get(slot) else {
            return Ok(());
        };
        if !can_add(Some(&item))? {
            ctx.message(
                &self.player()?,
                "A magical force prevents you from banking this item",
            )?;
            return Ok(());
        }
        let maximum = ctx.inventory(&self.player()?)?.amount(Some(&item));
        if amount > maximum {
            amount = maximum;
        }
        let defs = self.definitions();
        let (id, charge) = {
            let value = item.read();
            (value.id(), value.charge())
        };
        item = Shared::new(Item::new(id, amount, charge, &defs));
        let add = item::unnoted(&item, &defs)?;
        let max_count = self.0.maximum_add(Some(&add))?;
        if amount > max_count {
            add.write().set_amount(max_count);
            item.write().set_amount(max_count);
            if max_count < 1 {
                let dispatch = ctx.dispatch(&self.player()?)?;
                ctx.packet_message(dispatch, "There is not enough space left in your bank.")?;
                return Ok(());
            }
        }
        if ctx
            .inventory(&self.player()?)?
            .remove(Some(&item), Some(slot), false, &defs, ctx)?
        {
            let mut preferred = -1;
            if self.tab_index() != 0
                && self.tab_index() != 10
                && !self.0.contains(add.read().id(), 1)
            {
                preferred = self
                    .tab(self.tab_index())?
                    .wrapping_add(self.items_in_tab(self.tab_index())?);
                self.container()
                    .insert(self.0.free_slot(), preferred, false, &defs, ctx)?;
                self.increase_tab_start_slots(self.tab_index())?;
            }
            self.container()
                .add(Some(&add), true, preferred, &defs, ctx)?;
            ctx.inventory(&self.player()?)?.update(None, &defs, ctx)?;
        }
        Ok(())
    }
    pub fn take_item<C: Context>(&self, slot: i32, mut amount: i32, ctx: &mut C) -> Result<()> {
        if slot < 0 || slot > self.0.capacity() || amount <= 0 {
            return Ok(());
        }
        let Some(item) = self.0.get(slot) else {
            return Ok(());
        };
        if amount > item.read().amount() {
            amount = item.read().amount();
        }
        let defs = self.definitions();
        let (id, charge) = {
            let value = item.read();
            (value.id(), value.charge())
        };
        let item = Shared::new(Item::new(id, amount, charge, &defs));
        let note_id = item
            .read()
            .definition
            .clone()
            .ok_or(Error::Null)?
            .read()
            .note_id;
        let mut add = if self.is_note_items(ctx)? && note_id > 0 {
            Shared::new(Item::new(note_id, amount, item.read().charge(), &defs))
        } else {
            item.clone()
        };
        let max_count = ctx.inventory(&self.player()?)?.maximum_add(Some(&add))?;
        if amount > max_count {
            item.write().set_amount(max_count);
            add.write().set_amount(max_count);
            if max_count < 1 {
                let dispatch = ctx.dispatch(&self.player()?)?;
                ctx.packet_message(dispatch, "Not enough space in your inventory.")?;
                return Ok(());
            }
        }
        if self.is_note_items(ctx)? && note_id < 0 {
            let dispatch = ctx.dispatch(&self.player()?)?;
            ctx.packet_message(dispatch, "This item can't be withdrawn as a note.")?;
            add = item.clone();
        }
        if self
            .container()
            .remove(Some(&item), Some(slot), false, &defs, ctx)?
        {
            ctx.inventory(&self.player()?)?
                .add_two(Some(&add), false, &defs, ctx)?;
        }
        if self.0.get(slot).is_none() {
            self.decrease_tab_start_slots(self.tab_by_item_slot(slot), ctx)?;
            self.container().shift(&defs, ctx)?;
        } else {
            self.container().update(None, &defs, ctx)?;
        }
        ctx.inventory(&self.player()?)?.update(None, &defs, ctx)
    }
    pub fn update_last_amount_x<C: Context>(&self, amount: i32, ctx: &mut C) -> Result<()> {
        self.0.bank_mutate(|data| data.last_amount_x = amount)?;
        ctx.set_varp(&self.player()?, 1249, amount)
    }
    pub fn tab_by_item_slot(&self, slot: i32) -> i32 {
        let tabs = self.tab_start_slots();
        let mut result = 0;
        for i in 0..tabs.read().len() {
            if slot >= tabs.read()[i] {
                result = i as i32;
            }
        }
        result
    }
    pub fn increase_tab_start_slots(&self, start: i32) -> Result<()> {
        let mut i = start.wrapping_add(1);
        while i < self.tab_start_slots().read().len() as i32 {
            self.set_tab(i, self.tab(i)?.wrapping_add(1))?;
            i = i.wrapping_add(1);
        }
        Ok(())
    }
    fn decrease_prefix(&self, start: i32) -> Result<()> {
        if start == 10 {
            return Ok(());
        }
        let mut i = start.wrapping_add(1);
        while i < self.tab_start_slots().read().len() as i32 {
            self.set_tab(i, self.tab(i)?.wrapping_sub(1))?;
            i = i.wrapping_add(1);
        }
        Ok(())
    }
    pub fn decrease_tab_start_slots<C: Context>(&self, start: i32, ctx: &mut C) -> Result<()> {
        self.decrease_prefix(start)?;
        if start != 10 && self.items_in_tab(start)? == 0 {
            self.collapse_tab(start, ctx)?;
        }
        Ok(())
    }
    pub fn items_in_tab(&self, tab: i32) -> Result<i32> {
        Ok(self.tab(tab.wrapping_add(1))?.wrapping_sub(self.tab(tab)?))
    }
    pub fn send_bank_space<C: ListenerContext>(&self, ctx: &mut C) -> Result<()> {
        ctx.listener_varc(
            &self.player()?,
            192,
            self.0.capacity().wrapping_sub(self.0.free_slots()),
        )
    }
    pub fn collapse_tab<C: Context>(&self, tab: i32, ctx: &mut C) -> Result<()> {
        let size = self.items_in_tab(tab)?;
        if !(0..=1_000_000).contains(&size) {
            return Err(Error::Bounds);
        }
        let mut items = vec![];
        let defs = self.definitions();
        for i in 0..size {
            items.push(self.0.get(self.tab(tab)?.wrapping_add(i)));
            self.container()
                .replace(None, self.tab(tab)?.wrapping_add(i), false, &defs, ctx)?;
        }
        self.container().shift(&defs, ctx)?;
        let mut i = tab;
        while i < self.tab_start_slots().read().len() as i32 - 1 {
            self.set_tab(i, self.tab(i.wrapping_add(1))?.wrapping_sub(size))?;
            i = i.wrapping_add(1);
        }
        self.set_tab(10, self.tab(10)?.wrapping_sub(size))?;
        for item in items {
            self.container()
                .replace(item.as_ref(), self.0.free_slot(), false, &defs, ctx)?;
        }
        self.container().refresh(None, &defs, ctx)
    }
    pub fn set_tab_configurations<C: ListenerContext>(&self, ctx: &mut C) -> Result<()> {
        for i in 0..8 {
            ctx.listener_varbit(&self.player()?, 4885 + i, self.items_in_tab(i + 1)?, false)?;
        }
        Ok(())
    }
    pub fn is_note_items<C: Context>(&self, ctx: &mut C) -> Result<bool> {
        Ok(ctx.get_varbit(&self.player()?, NOTE_MODE)? == 1)
    }
    pub fn set_note_items<C: Context>(&self, value: bool, ctx: &mut C) -> Result<()> {
        ctx.set_varbit(&self.player()?, NOTE_MODE, i32::from(value), true)
    }
    pub fn is_insert_items<C: Context>(&self, ctx: &mut C) -> Result<bool> {
        Ok(ctx.get_varbit(&self.player()?, INSERT_MODE)? == 1)
    }
    pub fn set_insert_items<C: Context>(&self, value: bool, ctx: &mut C) -> Result<()> {
        ctx.set_varbit(&self.player()?, INSERT_MODE, i32::from(value), true)
    }
    pub fn set_tab_index<C: Context>(&self, index: i32, ctx: &mut C) -> Result<()> {
        self.0
            .bank_mutate(|data| data.tab_index = if index == 0 { 10 } else { index })?;
        ctx.set_varbit(&self.player()?, 4893, index.wrapping_add(1), false)?;
        ctx.set_attribute(&self.player()?, "bank:lasttab", index)
    }
}
pub fn array_index(tab: i32) -> i32 {
    if tab == 41 || tab == 74 {
        return 10;
    }
    (1..10).find(|i| tab == 39 - 2 * (i - 1)).unwrap_or(-1)
}
pub fn can_add(item: Option<&Handle>) -> Result<bool> {
    let definition = item
        .ok_or(Error::Null)?
        .read()
        .definition
        .clone()
        .ok_or(Error::Null)?;
    let value = definition.read().handlers.get("bankable").cloned();
    match value {
        None | Some(Property::Null) => Ok(true),
        Some(Property::Bool(v)) => Ok(v),
        _ => Err(Error::Type),
    }
}

#[derive(Clone)]
pub enum CloseEvent {
    Bank(Bank),
    Deposit,
}
impl CloseEvent {
    pub fn close<C: Context>(&self, player: Option<&NodeHandle>, ctx: &mut C) -> Result<bool> {
        match self {
            Self::Bank(bank) => bank.close(ctx)?,
            Self::Deposit => {
                let interfaces = ctx.interfaces(player.ok_or(Error::Null)?)?;
                ctx.open_default_tabs(interfaces)?;
            }
        }
        Ok(true)
    }
}
/// Receiver tokens may represent null. Lookups and invocations remain separate
/// so adapters preserve evaluation order, reentry and failed mutation prefixes.
pub trait Context: container::Context {
    type Ironman;
    type Pin;
    type Interfaces;
    type Component;
    type Dispatch;
    fn inventory(&mut self, player: &NodeHandle) -> Result<Container>;
    fn bank(&mut self, player: &NodeHandle) -> Result<Bank>;
    fn ironman(&mut self, player: &NodeHandle) -> Result<Self::Ironman>;
    fn ultimate_restriction(&mut self, manager: Self::Ironman) -> Result<bool>;
    fn pin(&mut self, player: &NodeHandle) -> Result<Self::Pin>;
    fn unlocked(&mut self, pin: Self::Pin) -> Result<bool>;
    fn dev_mode(&mut self) -> Result<bool>;
    fn open_pin(&mut self, pin: Self::Pin, kind: i32) -> Result<()>;
    fn interfaces(&mut self, player: &NodeHandle) -> Result<Self::Interfaces>;
    fn component(&mut self, id: i32) -> Result<Self::Component>;
    fn open(
        &mut self,
        interfaces: Self::Interfaces,
        component: Self::Component,
    ) -> Result<Self::Component>;
    fn open_component(&mut self, interfaces: Self::Interfaces, id: i32) -> Result<Self::Component>;
    fn close_event(&mut self, component: Self::Component, event: CloseEvent) -> Result<()>;
    fn remove_tabs(&mut self, interfaces: Self::Interfaces, tabs: &[i32]) -> Result<()>;
    fn open_default_tabs(&mut self, interfaces: Self::Interfaces) -> Result<()>;
    fn open_single_tab(
        &mut self,
        interfaces: Self::Interfaces,
        component: Self::Component,
    ) -> Result<()>;
    fn close_single_tab(&mut self, interfaces: Self::Interfaces) -> Result<()>;
    fn dispatch(&mut self, player: &NodeHandle) -> Result<Self::Dispatch>;
    fn iface_settings(&mut self, dispatch: Self::Dispatch, fields: [i32; 5]) -> Result<()>;
    fn script(&mut self, dispatch: Self::Dispatch, id: i32) -> Result<()>;
    fn message(&mut self, player: &NodeHandle, text: &str) -> Result<()>;
    fn packet_message(&mut self, dispatch: Self::Dispatch, text: &str) -> Result<()>;
    fn deposit_items(
        &mut self,
        player: &NodeHandle,
        items: Items,
        options: &[&str],
        fields: [i32; 4],
    ) -> Result<()>;
    fn get_varbit(&mut self, player: &NodeHandle, id: i32) -> Result<i32>;
    fn set_varbit(&mut self, player: &NodeHandle, id: i32, value: i32, send: bool) -> Result<()>;
    fn set_varp(&mut self, player: &NodeHandle, id: i32, value: i32) -> Result<()>;
    fn set_varc(&mut self, player: &NodeHandle, id: i32, value: i32) -> Result<()>;
    fn set_attribute(&mut self, player: &NodeHandle, key: &str, value: i32) -> Result<()>;
    fn remove_attribute(&mut self, player: &NodeHandle, key: &str) -> Result<()>;
    fn packet(&mut self, packet: Packet) -> Result<()>;
}
/// The private bank listener only needs its actual bank, packets and variables.
/// Inventory mutations can dispatch it without fabricating unused UI services.
pub trait ListenerContext {
    fn listener_bank(&mut self, player: &NodeHandle) -> Result<Bank>;
    fn listener_packet(&mut self, packet: Packet) -> Result<()>;
    fn listener_varbit(
        &mut self,
        player: &NodeHandle,
        id: i32,
        value: i32,
        save: bool,
    ) -> Result<()>;
    fn listener_varc(&mut self, player: &NodeHandle, id: i32, value: i32) -> Result<()>;
}
impl<C: Context> ListenerContext for C {
    fn listener_bank(&mut self, player: &NodeHandle) -> Result<Bank> {
        self.bank(player)
    }
    fn listener_packet(&mut self, packet: Packet) -> Result<()> {
        self.packet(packet)
    }
    fn listener_varbit(
        &mut self,
        player: &NodeHandle,
        id: i32,
        value: i32,
        save: bool,
    ) -> Result<()> {
        self.set_varbit(player, id, value, save)
    }
    fn listener_varc(&mut self, player: &NodeHandle, id: i32, value: i32) -> Result<()> {
        self.set_varc(player, id, value)
    }
}
pub struct Packet {
    pub player: Option<NodeHandle>,
    pub items: Items,
    pub slots: Option<Vec<usize>>,
    pub bank: bool,
    pub length: i32,
}
impl Packet {
    pub fn frame(&self, null_item: &Handle) -> Result<Frame> {
        let pointer = (if self.bank { 762 } else { 763 }) << 16 | 64000;
        let container = if self.bank { 95 } else { 93 };
        let items = self.items.read();
        let wire = |item: &Handle| {
            let item = item.read();
            WireItem {
                id: item.id(),
                amount: item.amount(),
            }
        };
        let frame = if let Some(slots) = &self.slots {
            let values = slots
                .iter()
                .map(|&slot| {
                    let item = items.get(slot).ok_or(Error::Bounds)?;
                    Ok((
                        slot as i32,
                        item.as_ref().filter(|i| !i.same(null_item)).map(wire),
                    ))
                })
                .collect::<Result<Vec<_>>>()?;
            updates::inventory_partial(pointer, container, &values)
        } else {
            // Original writer ignores context.length when using Item[].
            updates::inventory_full(
                pointer,
                container,
                &items
                    .iter()
                    .map(|i| i.as_ref().map(wire))
                    .collect::<Vec<_>>(),
            )
        };
        frame.map_err(|e| Error::Binding(e.to_string()))
    }
}
pub struct Listener {
    player: Option<NodeHandle>,
}
impl NativeListener for Listener {
    fn retained_nodes(&self) -> Vec<NodeHandle> {
        self.player.clone().into_iter().collect()
    }
}
impl Listener {
    pub fn owner(&self) -> Option<NodeHandle> {
        self.player.clone()
    }
    pub fn refresh<C: ListenerContext>(&self, c: &mut Container, ctx: &mut C) -> Result<()> {
        let bank = c.is_bank();
        ctx.listener_packet(Packet {
            player: self.player.clone(),
            items: c.items(),
            slots: None,
            bank,
            length: if bank { c.capacity() } else { 28 },
        })?;
        if bank {
            self.after(ctx)?;
        }
        Ok(())
    }
    pub fn update<C: ListenerContext>(
        &self,
        c: &mut Container,
        event: Option<&Event>,
        ctx: &mut C,
    ) -> Result<()> {
        let bank = c.is_bank();
        let event = event.ok_or(Error::Null)?;
        let items = event.read().items.clone();
        let slots = event.read().slots();
        let length = items.read().len() as i32;
        ctx.listener_packet(Packet {
            player: self.player.clone(),
            items,
            slots: Some(slots),
            bank,
            length,
        })?;
        if bank {
            self.after(ctx)?;
        }
        Ok(())
    }
    fn after<C: ListenerContext>(&self, ctx: &mut C) -> Result<()> {
        let p = self.player.as_ref().ok_or(Error::Null)?;
        ctx.listener_bank(p)?.set_tab_configurations(ctx)?;
        ctx.listener_bank(p)?.send_bank_space(ctx)
    }
}
