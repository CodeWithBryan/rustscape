//! The bank in the live world: the canonical player's `BankContainer`
//! (`inventory::bank`) with its private BankListener, opened, closed and
//! changed over the live interface manager, variables and packets
//! ([`BankUi`], a `bank::Context` over [`Inner`]); `BankInterface`'s
//! InterfaceListeners (`world::bank_interface`: the 762 open listener and the
//! 762/763/767 buttons) registered at boot and dispatched from the IfAction
//! path (super::buttons) and from `InterfaceListeners.runOpen`
//! (`Inner::listener_open`); the captured `BankClose` close event from
//! `InterfaceManager.close` (`Inner::close_event`); `BankUtils.transferX`
//! through the input prompts (super::talk).
//!
//! The bank PIN interface, the secondary bank, the deposit box's item
//! rendering and the bank's help/deposit dialogues are refused by name.
use super::{P, host::Live, inner::Inner, now_millis};
use crate::{
    definitions::item::{Handle as Definition, Registry as Items},
    inventory::{
        self as inv, bank,
        container::{Container, Event, Items as ContainerItems},
        container_listener::Handle as ListenerHandle,
        item::Handle as ItemHandle,
    },
    player::{attributes::AttributeValue, bank_pin, ironman, owned::Player, variables},
    world::{
        bank_interface::{self as content, Program},
        component_objects::{BankClose, Close},
        interface_listeners::{Button, Callback},
        interfaces::{self, Handle},
        player_walk::NodeHandle,
    },
};
use rs09_protocol::game::{Frame, updates};

fn error(e: impl std::fmt::Debug) -> inv::Error {
    inv::Error::Binding(format!("{e:?}"))
}
fn ui_error(e: impl std::fmt::Debug) -> interfaces::Error {
    interfaces::Error::Binding(format!("{e:?}"))
}

/// What a bank button asks of the rest of the world as its last call
/// (`BankUtils.transferX`, the help and deposit dialogues): made right after
/// the button's callback returns.
pub enum Pending {
    TransferX(P, i32, bool),
    Dialogue(P, content::Dialogue),
}

/// `bank::Context` (and `bank_interface::Context`) over the live world's
/// canonical players, interface manager and packets.
pub struct BankUi<'a> {
    pub inner: &'a mut Inner,
    pub pending: Vec<Pending>,
}

impl<'a> BankUi<'a> {
    pub fn new(inner: &'a mut Inner) -> Self {
        Self {
            inner,
            pending: Vec::new(),
        }
    }
    fn p(&self, owner: &NodeHandle) -> inv::Result<P> {
        if self.inner.players.contains_key(&owner.node) {
            Ok(P(owner.clone()))
        } else {
            Err(inv::Error::Binding(
                "a bank owner who is not a live player".into(),
            ))
        }
    }
    fn player(&self, owner: &NodeHandle) -> inv::Result<Player> {
        let p = self.p(owner)?;
        Ok(self.inner.canonical(&p).player.clone())
    }
    fn manager_call<T>(
        &mut self,
        manager: &interfaces::Manager,
        f: impl FnOnce(&mut Inner, &interfaces::Manager) -> interfaces::Result<T>,
    ) -> inv::Result<T> {
        let owner = manager.player().ok_or(inv::Error::Null)?;
        let p = self.p(&owner)?;
        let previous = self.inner.current.replace(p.clone());
        let result = f(self.inner, manager);
        self.inner.current = previous;
        self.inner.drain(&p);
        result.map_err(error)
    }
    fn send(&mut self, player: &Player, frame: Frame) -> inv::Result<()> {
        let p = self.p(&player.owner())?;
        self.inner.queue_native(&p, frame);
        Ok(())
    }
    fn text(&mut self, owner: &NodeHandle, text: &str) -> inv::Result<()> {
        let p = self.p(owner)?;
        self.inner.message(&p, text).map_err(error)
    }
    fn variable<T>(
        &mut self,
        owner: &NodeHandle,
        f: impl FnOnce(
            &mut crate::player::owned::VariableServices<
                '_,
                std::sync::Arc<rs09_cache::archive::Archive>,
            >,
            &NodeHandle,
        ) -> variables::Result<T>,
    ) -> inv::Result<T> {
        let p = self.p(owner)?;
        self.inner.variables(&p, f).map_err(error)
    }
}

impl inv::container::Context for BankUi<'_> {
    fn update(
        &mut self,
        listener: ListenerHandle,
        container: &mut Container,
        event: Event,
        _: &Items,
    ) -> inv::Result<()> {
        if let Some(receiver) = listener.value::<bank::Listener>() {
            self.player(&receiver.owner().ok_or(inv::Error::Null)?)?;
            return receiver.update(container, Some(&event), self);
        }
        if let Some(receiver) = listener.value::<inv::listener::Listener>() {
            let owner = receiver.owner().ok_or(inv::Error::Null)?;
            let player = self.player(&owner)?;
            let p = self.p(&owner)?;
            let inner = &mut *self.inner;
            let result = player.apply(
                &mut inner.state,
                &mut inner.frames,
                now_millis,
                |_, _, context| receiver.update_event(context, container, Some(&event)),
            );
            inner.drain(&p);
            return result;
        }
        Err(inv::Error::Binding(
            "not ported in the live world: a container listener other than the inventory's and the bank's during banking".into(),
        ))
    }
    fn refresh(
        &mut self,
        listener: ListenerHandle,
        container: &mut Container,
        _: &Items,
    ) -> inv::Result<()> {
        if let Some(receiver) = listener.value::<bank::Listener>() {
            self.player(&receiver.owner().ok_or(inv::Error::Null)?)?;
            return receiver.refresh(container, self);
        }
        if let Some(receiver) = listener.value::<inv::listener::Listener>() {
            let owner = receiver.owner().ok_or(inv::Error::Null)?;
            let player = self.player(&owner)?;
            let p = self.p(&owner)?;
            let inner = &mut *self.inner;
            let result = player.apply(
                &mut inner.state,
                &mut inner.frames,
                now_millis,
                |_, _, context| receiver.refresh(context, container),
            );
            inner.drain(&p);
            return result;
        }
        Err(inv::Error::Binding(
            "not ported in the live world: a container listener other than the inventory's and the bank's during banking".into(),
        ))
    }
    fn ground(&mut self, _: &mut Container, _: Option<ItemHandle>, _: &Items) -> inv::Result<()> {
        Err(inv::Error::Binding(
            "not ported in the live world: an overflowing bank transfer's ground drop".into(),
        ))
    }
}

impl ironman::Context for BankUi<'_> {
    fn message(&mut self, owner: Option<&NodeHandle>, text: &str) -> ironman::Result<()> {
        self.text(owner.ok_or(ironman::Error::Null)?, text)
            .map_err(|e| ironman::Error::Binding(format!("{e:?}")))
    }
}

impl bank::Context for BankUi<'_> {
    type Ironman = ironman::Manager;
    type Pin = bank_pin::Manager;
    type Interfaces = interfaces::Manager;
    type Component = Option<Handle>;
    type Dispatch = Player;
    fn inventory(&mut self, owner: &NodeHandle) -> inv::Result<Container> {
        Ok(self.player(owner)?.inventory())
    }
    fn bank(&mut self, owner: &NodeHandle) -> inv::Result<bank::Bank> {
        Ok(self.player(owner)?.bank())
    }
    fn ironman(&mut self, owner: &NodeHandle) -> inv::Result<ironman::Manager> {
        Ok(self.player(owner)?.ironman())
    }
    fn ultimate_restriction(&mut self, manager: ironman::Manager) -> inv::Result<bool> {
        self.player(&manager.player().ok_or(inv::Error::Null)?)?;
        manager
            .check(self, Some(ironman::Mode::Ultimate))
            .map_err(error)
    }
    fn pin(&mut self, owner: &NodeHandle) -> inv::Result<bank_pin::Manager> {
        Ok(self.player(owner)?.bank_pin())
    }
    fn unlocked(&mut self, pin: bank_pin::Manager) -> inv::Result<bool> {
        self.player(&pin.player().ok_or(inv::Error::Null)?)?;
        Ok(pin.is_unlocked())
    }
    fn dev_mode(&mut self) -> inv::Result<bool> {
        Ok(self.inner.settings.dev_mode)
    }
    fn open_pin(&mut self, _: bank_pin::Manager, _: i32) -> inv::Result<()> {
        Err(inv::Error::Binding(
            "not ported in the live world: the bank PIN entry interface (BankPinManager.openType)"
                .into(),
        ))
    }
    fn interfaces(&mut self, owner: &NodeHandle) -> inv::Result<interfaces::Manager> {
        Ok(self.player(owner)?.manager())
    }
    fn component(&mut self, id: i32) -> inv::Result<Option<Handle>> {
        Ok(Some(self.inner.registry.component(id)))
    }
    fn open(
        &mut self,
        manager: interfaces::Manager,
        component: Option<Handle>,
    ) -> inv::Result<Option<Handle>> {
        self.manager_call(&manager, |inner, m| interfaces::open(inner, m, component))
    }
    fn open_component(
        &mut self,
        manager: interfaces::Manager,
        id: i32,
    ) -> inv::Result<Option<Handle>> {
        self.manager_call(&manager, |inner, m| interfaces::open_id(inner, m, id))
    }
    fn close_event(
        &mut self,
        component: Option<Handle>,
        event: bank::CloseEvent,
    ) -> inv::Result<()> {
        component.ok_or(inv::Error::Null)?.write().close_event = Some(Close::new(BankClose(event)));
        Ok(())
    }
    fn remove_tabs(&mut self, manager: interfaces::Manager, tabs: &[i32]) -> inv::Result<()> {
        self.manager_call(&manager, |inner, m| interfaces::remove_tabs(inner, m, tabs))
    }
    fn open_default_tabs(&mut self, manager: interfaces::Manager) -> inv::Result<()> {
        self.manager_call(&manager, interfaces::open_default_tabs)
    }
    fn open_single_tab(
        &mut self,
        manager: interfaces::Manager,
        component: Option<Handle>,
    ) -> inv::Result<()> {
        self.manager_call(&manager, |inner, m| {
            interfaces::open_single_tab(inner, m, &component.ok_or(interfaces::Error::Null)?)
                .map(|_| ())
        })
    }
    fn close_single_tab(&mut self, manager: interfaces::Manager) -> inv::Result<()> {
        self.manager_call(&manager, |inner, m| {
            interfaces::close_single_tab(inner, m).map(|_| ())
        })
    }
    fn dispatch(&mut self, owner: &NodeHandle) -> inv::Result<Player> {
        self.player(owner)
    }
    fn iface_settings(
        &mut self,
        player: Player,
        [mask, child, pane, start, end]: [i32; 5],
    ) -> inv::Result<()> {
        let frame = updates::interface_events(
            pane.wrapping_shl(16) | child,
            start,
            end,
            mask,
            player.manager().packet_count(1),
        );
        self.send(&player, frame)
    }
    fn script(&mut self, player: Player, id: i32) -> inv::Result<()> {
        let frame = updates::run_client_script(id, b"", &[], player.manager().packet_count(1))
            .map_err(error)?;
        self.send(&player, frame)
    }
    fn message(&mut self, owner: &NodeHandle, text: &str) -> inv::Result<()> {
        self.text(owner, text)
    }
    fn packet_message(&mut self, player: Player, text: &str) -> inv::Result<()> {
        self.text(&player.owner(), text)
    }
    fn deposit_items(
        &mut self,
        _: &NodeHandle,
        _: ContainerItems,
        _: &[&str],
        _: [i32; 4],
    ) -> inv::Result<()> {
        Err(inv::Error::Binding(
            "not ported in the live world: the deposit box's item-select rendering (InterfaceContainer.generateItems)".into(),
        ))
    }
    fn get_varbit(&mut self, owner: &NodeHandle, id: i32) -> inv::Result<i32> {
        self.variable(owner, |ctx, owner| {
            variables::get_varbit(ctx, Some(owner), id)
        })
    }
    fn set_varbit(
        &mut self,
        owner: &NodeHandle,
        id: i32,
        value: i32,
        save: bool,
    ) -> inv::Result<()> {
        self.variable(owner, |ctx, owner| {
            variables::set_varbit(ctx, Some(owner), id, value, save)
        })
    }
    fn set_varp(&mut self, owner: &NodeHandle, id: i32, value: i32) -> inv::Result<()> {
        self.variable(owner, |ctx, owner| {
            variables::set_varp(ctx, Some(owner), id, value, false)
        })
    }
    fn set_varc(&mut self, owner: &NodeHandle, id: i32, value: i32) -> inv::Result<()> {
        self.variable(owner, |ctx, owner| {
            variables::set_varc(ctx, Some(owner), id, value)
        })
    }
    fn set_attribute(&mut self, owner: &NodeHandle, key: &str, value: i32) -> inv::Result<()> {
        self.player(owner)?.attributes().set_integer(key, value);
        Ok(())
    }
    fn remove_attribute(&mut self, owner: &NodeHandle, key: &str) -> inv::Result<()> {
        self.player(owner)?.attributes().remove(key);
        Ok(())
    }
    fn packet(&mut self, packet: bank::Packet) -> inv::Result<()> {
        let player = self.player(packet.player.as_ref().ok_or(inv::Error::Null)?)?;
        let frame = packet.frame(&player.core().null_item())?;
        self.send(&player, frame)
    }
}

impl content::Context for BankUi<'_> {
    type Container = Items2;
    type Item = ItemHandle;
    type Definition = Definition;
    type Dispatch = Player;
    type Dialogue = content::Dialogue;
    fn bank_size(&mut self) -> interfaces::Result<i32> {
        Ok(bank::SIZE)
    }
    fn bank(&mut self, owner: &NodeHandle) -> interfaces::Result<Items2> {
        Ok(Items2::Bank(self.player(owner).map_err(ui_error)?.bank()))
    }
    fn inventory(&mut self, owner: &NodeHandle) -> interfaces::Result<Items2> {
        Ok(Items2::Inventory(
            self.player(owner).map_err(ui_error)?.inventory(),
        ))
    }
    fn item(&mut self, c: Items2, slot: i32) -> interfaces::Result<Option<ItemHandle>> {
        Ok(c.container().get(slot))
    }
    fn amount(&mut self, c: Items2, item: ItemHandle) -> interfaces::Result<i32> {
        Ok(c.container().amount(Some(&item)))
    }
    fn amount_id(&mut self, c: Items2, id: i32) -> interfaces::Result<i32> {
        let definitions = self.inner.items.clone();
        Ok(c.container().amount_id(id, &definitions))
    }
    fn last_amount_x(&mut self, c: Items2) -> interfaces::Result<i32> {
        Ok(c.bank()?.last_amount_x())
    }
    fn take(&mut self, c: Items2, slot: i32, amount: i32) -> interfaces::Result<()> {
        c.bank()?.take_item(slot, amount, self).map_err(ui_error)
    }
    fn deposit(&mut self, c: Items2, slot: i32, amount: i32) -> interfaces::Result<()> {
        c.bank()?.add_item(slot, amount, self).map_err(ui_error)
    }
    fn transfer_x(
        &mut self,
        owner: &NodeHandle,
        slot: i32,
        withdraw: bool,
    ) -> interfaces::Result<()> {
        let p = self.p(owner).map_err(ui_error)?;
        self.pending.push(Pending::TransferX(p, slot, withdraw));
        Ok(())
    }
    fn tab_index(&mut self, c: Items2) -> interfaces::Result<i32> {
        Ok(c.bank()?.tab_index())
    }
    fn set_tab_index(&mut self, c: Items2, index: i32) -> interfaces::Result<()> {
        c.bank()?.set_tab_index(index, self).map_err(ui_error)
    }
    fn collapse_tab(&mut self, c: Items2, index: i32) -> interfaces::Result<()> {
        c.bank()?.collapse_tab(index, self).map_err(ui_error)
    }
    fn insert(&mut self, c: Items2) -> interfaces::Result<bool> {
        c.bank()?.is_insert_items(self).map_err(ui_error)
    }
    fn set_insert(&mut self, c: Items2, value: bool) -> interfaces::Result<()> {
        c.bank()?.set_insert_items(value, self).map_err(ui_error)
    }
    fn note(&mut self, c: Items2) -> interfaces::Result<bool> {
        c.bank()?.is_note_items(self).map_err(ui_error)
    }
    fn set_note(&mut self, c: Items2, value: bool) -> interfaces::Result<()> {
        c.bank()?.set_note_items(value, self).map_err(ui_error)
    }
    fn last_tab(&mut self, owner: &NodeHandle) -> interfaces::Result<i32> {
        match self
            .player(owner)
            .map_err(ui_error)?
            .attributes()
            .get("bank:lasttab")
        {
            None => Ok(0),
            Some(AttributeValue::Int(n)) => Ok(n),
            Some(AttributeValue::Long(n)) => Ok(n as i32),
            Some(AttributeValue::Short(n)) => Ok(i32::from(n)),
            Some(AttributeValue::Byte(n)) => Ok(i32::from(n)),
            _ => Err(interfaces::Error::Binding(
                "bank:lasttab of another type".into(),
            )),
        }
    }
    fn varc(&mut self, owner: &NodeHandle, id: i32, value: i32) -> interfaces::Result<()> {
        bank::Context::set_varc(self, owner, id, value).map_err(ui_error)
    }
    fn search(&mut self, owner: &NodeHandle) -> interfaces::Result<()> {
        self.player(owner)
            .map_err(ui_error)?
            .attributes()
            .set_boolean("search", true);
        Ok(())
    }
    fn definition(&mut self, item: ItemHandle) -> interfaces::Result<Definition> {
        item.read()
            .definition
            .clone()
            .ok_or(interfaces::Error::Null)
    }
    fn examine(&mut self, d: Definition) -> interfaces::Result<Option<content::Text>> {
        Ok(d.read()
            .examine
            .as_ref()
            .map(|s| s.encode_utf16().collect()))
    }
    fn id(&mut self, d: Definition) -> interfaces::Result<i32> {
        Ok(d.read().id)
    }
    fn name(&mut self, d: Definition) -> interfaces::Result<Option<content::Text>> {
        Ok(d.read().name.as_ref().map(|s| s.encode_utf16().collect()))
    }
    fn message(
        &mut self,
        owner: &NodeHandle,
        text: Option<content::Text>,
    ) -> interfaces::Result<()> {
        let text = String::from_utf16_lossy(&text.ok_or(interfaces::Error::Null)?);
        self.text(owner, &text).map_err(ui_error)
    }
    fn debug(&mut self, owner: &NodeHandle, text: content::Text) -> interfaces::Result<()> {
        let p = self.p(owner).map_err(ui_error)?;
        let debug = self
            .inner
            .canonical(&p)
            .player
            .attributes()
            .boolean("debug", false)
            .unwrap_or(false);
        if debug {
            let text = String::from_utf16_lossy(&text);
            self.inner.message(&p, &text).map_err(ui_error)?;
        }
        Ok(())
    }
    fn dispatch(&mut self, owner: &NodeHandle) -> interfaces::Result<Player> {
        self.player(owner).map_err(ui_error)
    }
    fn settings(&mut self, player: Player, fields: [i32; 5]) -> interfaces::Result<()> {
        bank::Context::iface_settings(self, player, fields).map_err(ui_error)
    }
    fn create_dialogue(
        &mut self,
        kind: content::Dialogue,
    ) -> interfaces::Result<content::Dialogue> {
        Ok(kind)
    }
    fn open_dialogue(
        &mut self,
        owner: &NodeHandle,
        dialogue: content::Dialogue,
    ) -> interfaces::Result<()> {
        let p = self.p(owner).map_err(ui_error)?;
        self.pending.push(Pending::Dialogue(p, dialogue));
        Ok(())
    }
    fn open_bank(&mut self, owner: &NodeHandle) -> interfaces::Result<()> {
        let player = self.player(owner).map_err(ui_error)?;
        player.bank().open_bank(self).map_err(ui_error)
    }
}

/// The bank interface's two container receivers.
#[derive(Clone)]
pub enum Items2 {
    Bank(bank::Bank),
    Inventory(Container),
}
impl Items2 {
    fn container(&self) -> Container {
        match self {
            Self::Bank(b) => b.container(),
            Self::Inventory(c) => c.clone(),
        }
    }
    fn bank(self) -> interfaces::Result<bank::Bank> {
        match self {
            Self::Bank(b) => Ok(b),
            Self::Inventory(_) => Err(interfaces::Error::Binding("not a bank container".into())),
        }
    }
}

impl Inner {
    /// `InterfaceListeners.runOpen` for a bank interface open listener: the
    /// BankInterface `onOpen(762)` callback. `Some` when the callback is the
    /// bank's.
    pub fn bank_open_listener(
        &mut self,
        callback: &Callback,
        component: &Handle,
    ) -> Option<interfaces::Result<bool>> {
        Program::kind(callback)?;
        let p = self.current.clone()?;
        let owner = p.0.clone();
        let mut ui = BankUi::new(self);
        let result = Program::invoke(
            callback.clone(),
            &mut ui,
            &owner,
            component,
            Button {
                opcode: 0,
                button: 0,
                slot: 0,
                item: 0,
            },
        );
        Some(result)
    }
    /// `CloseEvent.close` of a bank close event (`BankClose`); `None` when the
    /// event is another kind.
    pub fn bank_close_event(
        &mut self,
        event: &interfaces::CloseEvent,
    ) -> Option<interfaces::Result<bool>> {
        let event = event.value::<BankClose>()?;
        let p = self.current.clone()?;
        let owner = p.0.clone();
        let mut ui = BankUi::new(self);
        Some(event.0.close(Some(&owner), &mut ui).map_err(ui_error))
    }
}

impl Live {
    /// `openBankAccount(player)`: the player's bank opened.
    pub fn open_bank_of(&mut self, p: &P) -> inv::Result<()> {
        let bank = self.inner.canonical(p).player.bank();
        let result = bank.open_bank(&mut BankUi::new(&mut self.inner));
        self.inner.drain(p);
        result
    }
    /// ContentAPI `hasIronmanRestriction(player, restriction)`:
    /// `isIronman && mode.ordinal >= restriction.ordinal` (no message).
    pub fn has_ironman_restriction(
        &self,
        p: &P,
        restriction: ironman::Mode,
    ) -> ironman::Result<bool> {
        let manager = self.inner.canonical(p).player.ironman();
        if !manager.is_ironman() {
            return Ok(false);
        }
        let mode = manager.mode().ok_or(ironman::Error::Null)?;
        Ok(mode as i32 >= restriction as i32)
    }
    /// ContentAPI `openBankAccount(player)`:
    /// `restrictForIronman(player, ULTIMATE) { player.bank.open() }`.
    pub fn open_bank_account(&mut self, p: &P) -> inv::Result<()> {
        let manager = self.inner.canonical(p).player.ironman();
        let restricted = manager
            .check(
                &mut BankUi::new(&mut self.inner),
                Some(ironman::Mode::Ultimate),
            )
            .map_err(error)?;
        self.inner.drain(p);
        if restricted {
            return Ok(());
        }
        self.open_bank_of(p)
    }
    /// `removeItem(player, item)` from the inventory.
    pub fn remove_from_inventory(&mut self, p: &P, item: &ItemHandle) -> inv::Result<bool> {
        let player = self.inner.canonical(p).player.clone();
        let inner = &mut self.inner;
        let result = player.apply(
            &mut inner.state,
            &mut inner.frames,
            now_millis,
            |inventory, defs, ctx| inventory.remove(Some(item), None, true, defs, ctx),
        );
        inner.drain(p);
        result
    }
    pub fn bank_take(
        &mut self,
        p: &P,
        bank: &bank::Bank,
        slot: i32,
        amount: i32,
    ) -> inv::Result<()> {
        let result = bank.take_item(slot, amount, &mut BankUi::new(&mut self.inner));
        self.inner.drain(p);
        result
    }
    pub fn bank_deposit(
        &mut self,
        p: &P,
        bank: &bank::Bank,
        slot: i32,
        amount: i32,
    ) -> inv::Result<()> {
        let result = bank.add_item(slot, amount, &mut BankUi::new(&mut self.inner));
        self.inner.drain(p);
        result
    }
    pub fn bank_last_amount(&mut self, p: &P, bank: &bank::Bank, amount: i32) -> inv::Result<()> {
        let result = bank.update_last_amount_x(amount, &mut BankUi::new(&mut self.inner));
        self.inner.drain(p);
        result
    }
    /// A BankInterface button callback (762, 763, 767) with the live world,
    /// then what it asked for as its last call.
    pub fn bank_button(
        &mut self,
        callback: Callback,
        owner: &NodeHandle,
        component: &Handle,
        button: Button,
    ) -> interfaces::Result<bool> {
        let p = P(owner.clone());
        self.inner.current = Some(p.clone());
        let mut ui = BankUi::new(&mut self.inner);
        let result = Program::invoke(callback, &mut ui, owner, component, button);
        let pending = std::mem::take(&mut ui.pending);
        self.inner.drain(&p);
        let handled = result?;
        for request in pending {
            match request {
                Pending::TransferX(p, slot, withdraw) => {
                    self.transfer_x(&p, slot, withdraw).map_err(ui_error)?;
                }
                Pending::Dialogue(_, kind) => {
                    return Err(interfaces::Error::Binding(format!(
                        "not ported in the live world: BankInterface's {kind:?} dialogue (content dialogue)"
                    )));
                }
            }
        }
        Ok(handled)
    }
    /// ContentAPI `hasAwaitingGrandExchangeCollections(player)`.
    pub fn ge_awaiting_collections(&mut self, _: &P) -> Result<bool, String> {
        // GrandExchangeRecords.getInstance(player).offerRecords: no offer
        // record is loaded while the Grand Exchange is not installed.
        self.inner.not_ported.default_answer(
            "hasAwaitingGrandExchangeCollections (the Grand Exchange is not installed: no offer records, false)",
        );
        Ok(false)
    }
    /// ContentAPI `openGrandExchangeCollectionBox(player)`.
    pub fn ge_open_collection_box(&mut self, _: &P) -> Result<(), String> {
        Err("not ported in the live world: openGrandExchangeCollectionBox (the Grand Exchange is not installed)".into())
    }
    /// `BankInterface`'s listeners in the live InterfaceListeners registry.
    pub fn define_bank_interface(&mut self) -> interfaces::Result<()> {
        Program::default().define(&self.listeners)
    }
}
