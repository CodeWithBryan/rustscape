//! The native contexts the GE runs its joined pieces with, and the packet
//! writers it reaches through PacketDispatch and PacketRepository.
//!
//! [`Ui`] is the `interfaces::Context` of one player's InterfaceManager: its
//! listener open/close go through the world's native InterfaceListeners
//! registry, whose GE callbacks run the ported bodies ([`Callbacks`]); the
//! GE ComponentPlugin's inherited `open` does nothing; every other manager
//! service goes to the host. [`Items`] is the `container::Context` of every
//! container operation: ExchangeItemSets' listener runs the ported listener,
//! any other listener goes to the host. No lock is held while either calls
//! out.
use super::{
    Host, InterfacePlugin, MarketButtons, MarketClose, MarketOpen, SearchClose, SearchOpen,
    SetsClose, SideButtons, offer::WithdrawItem, sets_interface, stock_market,
};
use crate::{
    definitions::item::Registry,
    inventory::{
        self,
        container::{self, Container, Event},
        container_listener::Handle as Listener,
        item::Handle as Item,
    },
    world::{
        interface_listeners::{self, Button, Callback},
        interfaces::{self, CloseEvent, Handle as Component, Manager, Plugin},
        player_walk::NodeHandle,
    },
};
use rs09_protocol::game::{
    Frame,
    updates::{self, ScriptArg, WireItem},
};

pub struct Ui<'a, H: Host + ?Sized> {
    pub host: &'a mut H,
    pub player: NodeHandle,
}
impl<'a, H: Host + ?Sized> Ui<'a, H> {
    pub fn new(host: &'a mut H, player: &NodeHandle) -> Self {
        Self {
            host,
            player: player.clone(),
        }
    }
}
type UiResult<T> = interfaces::Result<T>;
fn ui<T>(r: super::Result<T>) -> UiResult<T> {
    r.map_err(Into::into)
}
impl<H: Host + ?Sized> interfaces::Context for Ui<'_, H> {
    fn validate_manager(&mut self, manager: &Manager) -> UiResult<()> {
        ui(self.host.ui_validate(&self.player, manager))
    }
    fn component(&mut self, id: i32) -> UiResult<Component> {
        ui(self.host.ui_component(&self.player, id))
    }
    fn resizable(&mut self) -> UiResult<bool> {
        ui(self.host.ui_resizable(&self.player))
    }
    fn send(&mut self, frame: Frame) -> UiResult<()> {
        ui(self.host.send(&self.player, frame))
    }
    fn log(&mut self, message: String) -> UiResult<()> {
        ui(self.host.ui_log(&self.player, message))
    }
    /// `InterfaceListeners.runOpen(player, component)`; its result is ignored by Component.open.
    fn listener_open(&mut self, component: &Component) -> UiResult<()> {
        let registry = ui(self.host.interface_listeners())?;
        let player = self.player.clone();
        registry
            .run_open(
                &mut Callbacks { host: self.host },
                Some(&player),
                Some(component),
            )
            .map(|_| ())
    }
    /// `InterfaceListeners.runClose(player, component)`.
    fn listener_close(&mut self, component: &Component) -> UiResult<bool> {
        let registry = ui(self.host.interface_listeners())?;
        let player = self.player.clone();
        registry.run_close(
            &mut Callbacks { host: self.host },
            Some(&player),
            Some(component),
        )
    }
    fn plugin_open(&mut self, plugin: Plugin, component: &Component) -> UiResult<()> {
        if plugin.value::<InterfacePlugin>().is_some() {
            // GrandExchangeInterface keeps ComponentPlugin's empty `open`.
            return Ok(());
        }
        ui(self.host.ui_plugin_open(&self.player, plugin, component))
    }
    fn close_event(&mut self, event: CloseEvent, component: &Component) -> UiResult<bool> {
        ui(self.host.ui_close_event(&self.player, event, component))
    }
    fn open_component(&mut self, manager: &Manager, component: &Component) -> UiResult<()> {
        interfaces::component_open(self, manager, component)
    }
    fn dispatch_open(&mut self, component: Component) -> UiResult<()> {
        ui(self.host.ui_dispatch_open(&self.player, component))
    }
    fn dispatch_close(&mut self, component: Option<Component>) -> UiResult<()> {
        ui(self.host.ui_dispatch_close(&self.player, component))
    }
    fn set_varp(&mut self, id: i32, value: i32) -> UiResult<()> {
        ui(self.host.ui_set_varp(&self.player, id, value))
    }
    fn weapon_extension(&mut self) -> UiResult<Option<Component>> {
        ui(self.host.ui_weapon_extension(&self.player))
    }
    fn create_weapon(&mut self) -> UiResult<Component> {
        ui(self.host.ui_create_weapon(&self.player))
    }
    fn add_weapon(&mut self, component: Component) -> UiResult<()> {
        ui(self.host.ui_add_weapon(&self.player, component))
    }
    fn spellbook(&mut self) -> UiResult<i32> {
        ui(self.host.ui_spellbook(&self.player))
    }
    fn has_familiar(&mut self) -> UiResult<bool> {
        ui(self.host.ui_has_familiar(&self.player))
    }
    fn friends_component(&mut self) -> UiResult<Component> {
        ui(self.host.ui_friends_component(&self.player))
    }
    fn autocast(&mut self) -> UiResult<Option<usize>> {
        ui(self.host.ui_autocast(&self.player))
    }
    fn auto_spell_id(&mut self, weapon: &Component, spell: usize) -> UiResult<i32> {
        ui(self.host.ui_auto_spell_id(&self.player, weapon, spell))
    }
    fn select_auto_spell(&mut self, weapon: &Component, id: i32) -> UiResult<()> {
        ui(self.host.ui_select_auto_spell(&self.player, weapon, id))
    }
    fn window_mode(&mut self) -> UiResult<i32> {
        ui(self.host.ui_window_mode(&self.player))
    }
    fn set_window_mode(&mut self, mode: i32) -> UiResult<()> {
        ui(self.host.ui_set_window_mode(&self.player, mode))
    }
    fn tutorial_complete(&mut self) -> UiResult<bool> {
        ui(self.host.ui_tutorial_complete(&self.player))
    }
    fn tutorial_tabs(&mut self) -> UiResult<()> {
        ui(self.host.ui_tutorial_tabs(&self.player))
    }
    fn has_runscript(&mut self) -> UiResult<bool> {
        ui(self.host.ui_has_runscript(&self.player))
    }
    fn remove_runscript(&mut self) -> UiResult<()> {
        ui(self.host.ui_remove_runscript(&self.player))
    }
    fn set_unclosable_attribute(&mut self, value: bool) -> UiResult<()> {
        ui(self.host.ui_set_unclosable(&self.player, value))
    }
    fn unclosable_attribute(&mut self) -> UiResult<bool> {
        ui(self.host.ui_unclosable(&self.player))
    }
}

/// The InterfaceListeners context: the GE's typed callbacks run the ported
/// StockMarket and ExchangeItemSets lambdas; any other callback goes to the
/// host.
pub struct Callbacks<'a, H: Host + ?Sized> {
    pub host: &'a mut H,
}
impl<H: Host + ?Sized> interface_listeners::Context for Callbacks<'_, H> {
    fn invoke_open(
        &mut self,
        callback: Callback,
        player: &NodeHandle,
        component: &Component,
    ) -> interfaces::Result<bool> {
        if callback.value::<MarketOpen>().is_some() {
            return ui(stock_market::market_open(self.host, player));
        }
        if callback.value::<MarketClose>().is_some() {
            return ui(stock_market::market_close(self.host, player));
        }
        if callback.value::<SearchOpen>().is_some() {
            return ui(stock_market::search_open(self.host, player));
        }
        if callback.value::<SearchClose>().is_some() {
            return ui(stock_market::search_close(self.host, player));
        }
        if callback.value::<SetsClose>().is_some() {
            return ui(sets_interface::sets_close(self.host, player));
        }
        ui(self.host.other_listener(player, callback, component, None))
    }
    fn invoke_button(
        &mut self,
        callback: Callback,
        player: &NodeHandle,
        component: &Component,
        button: Button,
    ) -> interfaces::Result<bool> {
        if callback.value::<SideButtons>().is_some() {
            return ui(stock_market::side_button(self.host, player, button));
        }
        if callback.value::<MarketButtons>().is_some() {
            return ui(stock_market::market_button(self.host, player, button));
        }
        ui(self
            .host
            .other_listener(player, callback, component, Some(button)))
    }
}

pub struct Items<'a, H: Host + ?Sized> {
    pub host: &'a mut H,
}
type ItemsResult<T> = inventory::Result<T>;
fn items<T>(r: super::Result<T>) -> ItemsResult<T> {
    r.map_err(Into::into)
}
impl<H: Host + ?Sized> container::Context for Items<'_, H> {
    fn update(
        &mut self,
        listener: Listener,
        container: &mut Container,
        event: Event,
        _definitions: &Registry,
    ) -> ItemsResult<()> {
        if let Some(sets) = listener.value::<sets_interface::SetsListener>() {
            return items(sets_interface::listener_update(self.host, &sets));
        }
        items(self.host.listener_update(listener, container, event))
    }
    fn refresh(
        &mut self,
        listener: Listener,
        container: &mut Container,
        _definitions: &Registry,
    ) -> ItemsResult<()> {
        if let Some(sets) = listener.value::<sets_interface::SetsListener>() {
            return items(sets_interface::listener_refresh(self.host, &sets));
        }
        items(self.host.listener_refresh(listener, container))
    }
    fn ground(
        &mut self,
        _container: &mut Container,
        _item: Option<Item>,
        _definitions: &Registry,
    ) -> ItemsResult<()> {
        // Container.add(item, player) is never reached by the GE classes.
        Err(inventory::Error::Binding(
            "Container.add(item, player) is not used by the Grand Exchange".into(),
        ))
    }
}

// ------------------------------------------------------------------- packets

fn protocol<T>(r: std::result::Result<T, rs09_protocol::Error>) -> super::Result<T> {
    r.map_err(|e| super::Error::Service(format!("protocol: {e}")))
}

/// `PacketDispatch.sendString(text, interface, line)`.
pub fn send_string<H: Host + ?Sized>(
    h: &mut H,
    p: &NodeHandle,
    text: &str,
    interface: i32,
    line: i32,
) -> super::Result<()> {
    let m = h.interface_manager(p)?;
    let frame = protocol(updates::interface_text(
        interface.wrapping_shl(16) | (line & 0xFFFF),
        text.as_bytes(),
        m.packet_count(1),
    ))?;
    h.send(p, frame)
}
/// `PacketDispatch.sendIfaceSettings(settings, child, interface, offset, length)`.
pub fn send_iface_settings<H: Host + ?Sized>(
    h: &mut H,
    p: &NodeHandle,
    settings: i32,
    child: i32,
    interface: i32,
    offset: i32,
    length: i32,
) -> super::Result<()> {
    let m = h.interface_manager(p)?;
    let frame = updates::interface_events(
        interface.wrapping_shl(16) | child,
        offset,
        length,
        settings,
        m.packet_count(1),
    );
    h.send(p, frame)
}
/// `PacketDispatch.sendRunScript(id, types, objects...)` (objects in call order).
pub fn send_run_script<H: Host + ?Sized>(
    h: &mut H,
    p: &NodeHandle,
    id: i32,
    types: &str,
    objects: &[ScriptArg],
) -> super::Result<()> {
    let m = h.interface_manager(p)?;
    let frame = protocol(updates::run_client_script(
        id,
        types.as_bytes(),
        objects,
        m.packet_count(1),
    ))?;
    h.send(p, frame)
}
/// `PacketDispatch.sendInterfaceConfig(interface, child, hide)`.
pub fn send_interface_config<H: Host + ?Sized>(
    h: &mut H,
    p: &NodeHandle,
    interface: i32,
    child: i32,
    hidden: bool,
) -> super::Result<()> {
    let m = h.interface_manager(p)?;
    let frame = updates::interface_hidden(
        hidden,
        m.packet_count(1),
        interface.wrapping_shl(16) | child,
    );
    h.send(p, frame)
}
/// `PacketRepository.send(Config, ConfigContext(player, id, value))`: the
/// Config writer does not write to an artificial player's session.
pub fn send_config<H: Host + ?Sized>(
    h: &mut H,
    p: &NodeHandle,
    id: i32,
    value: i32,
) -> super::Result<()> {
    let frame = updates::varp(id, value);
    if h.artificial(p)? {
        return Ok(());
    }
    h.send(p, frame)
}
/// `ContainerPacket` of a whole item array (not split) to interface/child
/// `interface`/`child`, container `container`.
pub fn send_items<H: Host + ?Sized>(
    h: &mut H,
    p: &NodeHandle,
    interface: i32,
    child: i32,
    container: i32,
    items: &[Option<WireItem>],
) -> super::Result<()> {
    let pointer = interface.wrapping_shl(16) | (child & 0xFFFF);
    let frame = protocol(updates::inventory_full(pointer, container & 0xFFFF, items))?;
    h.send(p, frame)
}
/// The withdraw slots of an offer to -1/-1757.
pub fn send_withdraw_container<H: Host + ?Sized>(
    h: &mut H,
    p: &NodeHandle,
    container: i32,
    withdraw: &[Option<WithdrawItem>; 2],
) -> super::Result<()> {
    let items: Vec<Option<WireItem>> = withdraw
        .iter()
        .map(|item| {
            item.map(|item| WireItem {
                id: item.id,
                amount: item.amount,
            })
        })
        .collect();
    send_items(h, p, -1, -1757, container, &items)
}
/// The items of a container (`container.toArray()`).
pub fn wire_items(container: &Container) -> Vec<Option<WireItem>> {
    container
        .items()
        .read()
        .iter()
        .map(|item| {
            item.as_ref().map(|item| {
                let item = item.read();
                WireItem {
                    id: item.id(),
                    amount: item.amount(),
                }
            })
        })
        .collect()
}
/// `GrandExchangePacket` (opcode 116): slot, packed state, item, price,
/// amount, completed amount and coins; a REMOVED offer is all zeros.
#[allow(clippy::too_many_arguments)]
pub fn exchange_frame(
    index: i8,
    state: i8,
    item: i16,
    sell: bool,
    value: i32,
    amount: i32,
    completed: i32,
    coins: i32,
) -> Frame {
    let mut payload = Vec::with_capacity(20);
    payload.push(index as u8);
    if state == 6 {
        payload.push(0);
        payload.extend_from_slice(&0i16.to_be_bytes());
        for _ in 0..4 {
            payload.extend_from_slice(&0i32.to_be_bytes());
        }
    } else {
        let mut packed = state.wrapping_add(1);
        if sell {
            packed = packed.wrapping_add(8);
        }
        if state == 2 {
            packed = if sell { -3 } else { 5 };
        }
        payload.push(packed as u8);
        payload.extend_from_slice(&item.to_be_bytes());
        payload.extend_from_slice(&value.to_be_bytes());
        payload.extend_from_slice(&amount.to_be_bytes());
        payload.extend_from_slice(&completed.to_be_bytes());
        payload.extend_from_slice(&coins.to_be_bytes());
    }
    Frame {
        opcode: 116,
        payload,
    }
}
