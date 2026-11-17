//! The two native contexts the trade runs its joined pieces with.
//!
//! [`Ui`] is the `interfaces::Context` of one player's InterfaceManager: the
//! trade's own close event (TradeCloseEvent) and plugin open (the inherited
//! empty ComponentPlugin.open) run here; every other manager service goes to
//! the host. [`Items`] is the `container::Context` of every container
//! operation: a TradeListener runs the ported listener, any other listener
//! goes to the host. No lock is held while either calls out.
use super::{Host, TradeClose, TradePlugin, module};
use crate::{
    definitions::item::Registry,
    inventory::{
        self,
        container::{self, Container, Event},
        container_listener::Handle as Listener,
        item::Handle as Item,
    },
    world::interfaces::{self, CloseEvent, Handle as Component, Manager, Plugin},
};
use rs09_protocol::game::Frame;

pub struct Ui<'a, H: Host> {
    pub host: &'a mut H,
    pub player: H::Player,
}
impl<'a, H: Host> Ui<'a, H> {
    pub fn new(host: &'a mut H, player: &H::Player) -> Self {
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
impl<H: Host> interfaces::Context for Ui<'_, H> {
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
    fn listener_open(&mut self, component: &Component) -> UiResult<()> {
        ui(self.host.ui_listener_open(&self.player, component))
    }
    fn listener_close(&mut self, component: &Component) -> UiResult<bool> {
        ui(self.host.ui_listener_close(&self.player, component))
    }
    fn plugin_open(&mut self, plugin: Plugin, component: &Component) -> UiResult<()> {
        if plugin.value::<TradePlugin>().is_some() {
            // TradeInterfacePlugin inherits ComponentPlugin.open's empty body.
            return Ok(());
        }
        ui(self.host.ui_plugin_open(&self.player, plugin, component))
    }
    fn close_event(&mut self, event: CloseEvent, component: &Component) -> UiResult<bool> {
        if event.value::<TradeClose>().is_some() {
            let player = self.player.clone();
            return ui(module::close_event(self.host, &player, component));
        }
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
        ui(self.host.set_varp(&self.player, id, value))
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

pub struct Items<'a, H: Host> {
    pub host: &'a mut H,
}
type ItemsResult<T> = inventory::Result<T>;
impl<H: Host> container::Context for Items<'_, H> {
    fn update(
        &mut self,
        listener: Listener,
        container: &mut Container,
        event: Event,
        _definitions: &Registry,
    ) -> ItemsResult<()> {
        if let Some(trade) = listener.value::<module::TradeListener<H::Player>>() {
            return module::listener_update(self.host, &trade, container).map_err(Into::into);
        }
        self.host
            .listener_update(listener, container, event)
            .map_err(Into::into)
    }
    fn refresh(
        &mut self,
        listener: Listener,
        container: &mut Container,
        _definitions: &Registry,
    ) -> ItemsResult<()> {
        if let Some(trade) = listener.value::<module::TradeListener<H::Player>>() {
            return module::listener_refresh(self.host, &trade).map_err(Into::into);
        }
        self.host
            .listener_refresh(listener, container)
            .map_err(Into::into)
    }
    fn ground(
        &mut self,
        _container: &mut Container,
        _item: Option<Item>,
        _definitions: &Registry,
    ) -> ItemsResult<()> {
        // Container.add(item, player) is never reached by the trade classes.
        Err(inventory::Error::Binding(
            "Container.add(item, player) is not used by the trade".into(),
        ))
    }
}
