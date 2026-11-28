//! `InterfaceContainer` in the live world: `world::grand_exchange`'s port of
//! `InterfaceContainer.generateItems` (`sets_interface::generate_items`,
//! grand-exchange-oracle) answered over the live frames. Its key counter
//! (`InterfaceContainer.index`, a static) is the `container_index` of one
//! `grand_exchange::Exchange` per world, kept here; the Grand Exchange itself
//! is not installed, so every other service of its Host is refused by name.
use super::{P, inner::Inner};
use crate::{
    definitions::item::Registry,
    inventory::{
        container::{Container, Event, Factory},
        container_listener::Handle as Listener,
    },
    player::ironman,
    world::{
        component_objects::Plugin,
        drop_tables::LogLevel,
        grand_exchange::{
            Error, Exchange, GuideType, Host, InputScript, Kind, Offer, Param, Pulse, Records,
            Result,
        },
        input_prompt::InputType,
        interface_listeners,
        interfaces::{CloseEvent, Handle as Component, Manager},
        player_walk::NodeHandle,
    },
};
use rs09_protocol::game::Frame;
use std::sync::Arc;

fn refuse<T>(what: &str) -> Result<T> {
    Err(Error::Service(format!(
        "not ported in the live world: grand exchange service {what} (only InterfaceContainer is bound)"
    )))
}

/// The world's `InterfaceContainer` state: the Exchange whose
/// `container_index` is `InterfaceContainer.index`, and BankDepositBoxInterface's
/// onOpen(11) waiting for the global handlers' host (super::banks).
#[derive(Default)]
pub struct InterfaceContainer {
    pub exchange: Exchange,
    pub pending_open: Option<(P, i32)>,
}

/// `grand_exchange::Host` for `InterfaceContainer.generateItems`.
pub struct Frames<'a> {
    pub inner: &'a mut Inner,
    pub exchange: Exchange,
}
impl Frames<'_> {
    fn player(&self, p: &NodeHandle) -> Result<P> {
        if self.inner.players.contains_key(&p.node) {
            Ok(P(p.clone()))
        } else {
            Err(Error::Null)
        }
    }
}

impl Host for Frames<'_> {
    fn exchange(&mut self) -> Result<Exchange> {
        Ok(self.exchange.clone())
    }
    fn definitions(&mut self) -> Result<Arc<Registry>> {
        refuse("definitions")
    }
    fn definition_order(&mut self) -> Result<Vec<i32>> {
        refuse("definition_order")
    }
    fn factory(&mut self) -> Result<Factory> {
        refuse("factory")
    }
    fn now(&mut self) -> Result<i64> {
        refuse("now")
    }
    fn cheater(&mut self) -> Result<bool> {
        refuse("cheater")
    }
    fn bots_influence_price_index(&mut self) -> Result<bool> {
        refuse("bots_influence_price_index")
    }
    fn botstock_limit(&mut self) -> Result<i32> {
        refuse("botstock_limit")
    }
    fn autostock_enabled(&mut self) -> Result<bool> {
        refuse("autostock_enabled")
    }
    fn data_path(&mut self) -> Result<Option<String>> {
        refuse("data_path")
    }
    fn read_file(&mut self, _: &str) -> Result<String> {
        refuse("read_file")
    }
    fn log(&mut self, _: LogLevel, _: &str) -> Result<()> {
        refuse("log")
    }
    fn log_ge(&mut self, _: &str) -> Result<()> {
        refuse("log_ge")
    }
    fn statement(&mut self, _: Kind, _: &str, _: &[Param]) -> Result<()> {
        refuse("statement")
    }
    fn interface_listeners(&mut self) -> Result<interface_listeners::Registry> {
        refuse("interface_listeners")
    }
    fn start_worker(&mut self) -> Result<()> {
        refuse("start_worker")
    }
    fn submit_pulse(&mut self, _: Pulse) -> Result<()> {
        refuse("submit_pulse")
    }
    fn news(&mut self, _: &str) -> Result<()> {
        refuse("news")
    }
    fn discord_new_offer(&mut self, _: bool, _: i32, _: i32, _: i32, _: &str) -> Result<()> {
        refuse("discord_new_offer")
    }
    fn details_uid(&mut self, _: &str) -> Result<i32> {
        refuse("details_uid")
    }
    fn player_by_uid(&mut self, _: i32) -> Result<Option<NodeHandle>> {
        refuse("player_by_uid")
    }
    fn player_by_account(&mut self, _: i32) -> Result<Option<NodeHandle>> {
        refuse("player_by_account")
    }
    fn examine(&mut self, _: i32) -> Result<String> {
        refuse("examine")
    }
    fn cs2_mapping(&mut self, _: i32, _: i32) -> Result<Option<Option<String>>> {
        refuse("cs2_mapping")
    }
    fn uid(&mut self, _: &NodeHandle) -> Result<i32> {
        refuse("uid")
    }
    fn username(&mut self, _: &NodeHandle) -> Result<String> {
        refuse("username")
    }
    fn artificial(&mut self, _: &NodeHandle) -> Result<bool> {
        refuse("artificial")
    }
    fn inventory(&mut self, _: &NodeHandle) -> Result<Container> {
        refuse("inventory")
    }
    fn ironman(&mut self, _: &NodeHandle) -> Result<ironman::Manager> {
        refuse("ironman")
    }
    fn interface_manager(&mut self, p: &NodeHandle) -> Result<Manager> {
        let p = self.player(p)?;
        Ok(self.inner.canonical(&p).player.manager())
    }
    fn pin_unlocked(&mut self, _: &NodeHandle) -> Result<bool> {
        refuse("pin_unlocked")
    }
    fn pin_open(&mut self, _: &NodeHandle, _: i32) -> Result<()> {
        refuse("pin_open")
    }
    fn records(&mut self, _: &NodeHandle) -> Result<Option<Records>> {
        refuse("records")
    }
    fn set_records(&mut self, _: &NodeHandle, _: Records) -> Result<()> {
        refuse("set_records")
    }
    fn temp_offer(&mut self, _: &NodeHandle) -> Result<Option<Offer>> {
        refuse("temp_offer")
    }
    fn set_temp_offer(&mut self, _: &NodeHandle, _: Offer) -> Result<()> {
        refuse("set_temp_offer")
    }
    fn remove_temp_offer(&mut self, _: &NodeHandle) -> Result<()> {
        refuse("remove_temp_offer")
    }
    fn ge_index(&mut self, _: &NodeHandle, _: i32) -> Result<i32> {
        refuse("ge_index")
    }
    fn set_ge_index(&mut self, _: &NodeHandle, _: i32) -> Result<()> {
        refuse("set_ge_index")
    }
    fn guide_price(&mut self, _: &NodeHandle) -> Result<Option<GuideType>> {
        refuse("guide_price")
    }
    fn set_guide_price(&mut self, _: &NodeHandle, _: GuideType) -> Result<()> {
        refuse("set_guide_price")
    }
    fn container_key(&mut self, _: &NodeHandle, _: i32) -> Result<i32> {
        refuse("container_key")
    }
    fn set_container_key(&mut self, _: &NodeHandle, _: i32) -> Result<()> {
        refuse("set_container_key")
    }
    fn remove_container_key(&mut self, _: &NodeHandle) -> Result<()> {
        refuse("remove_container_key")
    }
    fn sets_listener(&mut self, _: &NodeHandle) -> Result<Option<Listener>> {
        refuse("sets_listener")
    }
    fn set_sets_listener(&mut self, _: &NodeHandle, _: Listener) -> Result<()> {
        refuse("set_sets_listener")
    }
    fn remove_sets_listener(&mut self, _: &NodeHandle) -> Result<()> {
        refuse("remove_sets_listener")
    }
    fn message(&mut self, _: &NodeHandle, _: &str) -> Result<()> {
        refuse("message")
    }
    fn content_message(&mut self, _: &NodeHandle, _: &str) -> Result<()> {
        refuse("content_message")
    }
    fn notify(&mut self, _: &NodeHandle, _: &str) -> Result<()> {
        refuse("notify")
    }
    fn send(&mut self, p: &NodeHandle, frame: Frame) -> Result<()> {
        let p = self.player(p)?;
        self.inner.queue_native(&p, frame);
        Ok(())
    }
    fn set_varp(&mut self, _: &NodeHandle, _: i32, _: i32) -> Result<()> {
        refuse("set_varp")
    }
    fn audio(&mut self, _: &NodeHandle, _: i32) -> Result<()> {
        refuse("audio")
    }
    fn jingle(&mut self, _: &NodeHandle, _: i32) -> Result<()> {
        refuse("jingle")
    }
    fn input_dialogue(
        &mut self,
        _: &NodeHandle,
        _: InputType,
        _: &str,
        _: InputScript,
    ) -> Result<()> {
        refuse("input_dialogue")
    }
    fn listener_update(&mut self, _: Listener, _: &mut Container, _: Event) -> Result<()> {
        refuse("listener_update")
    }
    fn listener_refresh(&mut self, _: Listener, _: &mut Container) -> Result<()> {
        refuse("listener_refresh")
    }
    fn other_listener(
        &mut self,
        _: &NodeHandle,
        _: interface_listeners::Callback,
        _: &Component,
        _: Option<interface_listeners::Button>,
    ) -> Result<bool> {
        refuse("other_listener")
    }
    fn ui_validate(&mut self, _: &NodeHandle, _: &Manager) -> Result<()> {
        refuse("ui_validate")
    }
    fn ui_component(&mut self, _: &NodeHandle, _: i32) -> Result<Component> {
        refuse("ui_component")
    }
    fn ui_resizable(&mut self, _: &NodeHandle) -> Result<bool> {
        refuse("ui_resizable")
    }
    fn ui_log(&mut self, _: &NodeHandle, _: String) -> Result<()> {
        refuse("ui_log")
    }
    fn ui_plugin_open(&mut self, _: &NodeHandle, _: Plugin, _: &Component) -> Result<()> {
        refuse("ui_plugin_open")
    }
    fn ui_close_event(&mut self, _: &NodeHandle, _: CloseEvent, _: &Component) -> Result<bool> {
        refuse("ui_close_event")
    }
    fn ui_dispatch_open(&mut self, _: &NodeHandle, _: Component) -> Result<()> {
        refuse("ui_dispatch_open")
    }
    fn ui_dispatch_close(&mut self, _: &NodeHandle, _: Option<Component>) -> Result<()> {
        refuse("ui_dispatch_close")
    }
    fn ui_set_varp(&mut self, _: &NodeHandle, _: i32, _: i32) -> Result<()> {
        refuse("ui_set_varp")
    }
    fn ui_weapon_extension(&mut self, _: &NodeHandle) -> Result<Option<Component>> {
        refuse("ui_weapon_extension")
    }
    fn ui_create_weapon(&mut self, _: &NodeHandle) -> Result<Component> {
        refuse("ui_create_weapon")
    }
    fn ui_add_weapon(&mut self, _: &NodeHandle, _: Component) -> Result<()> {
        refuse("ui_add_weapon")
    }
    fn ui_spellbook(&mut self, _: &NodeHandle) -> Result<i32> {
        refuse("ui_spellbook")
    }
    fn ui_has_familiar(&mut self, _: &NodeHandle) -> Result<bool> {
        refuse("ui_has_familiar")
    }
    fn ui_friends_component(&mut self, _: &NodeHandle) -> Result<Component> {
        refuse("ui_friends_component")
    }
    fn ui_autocast(&mut self, _: &NodeHandle) -> Result<Option<usize>> {
        refuse("ui_autocast")
    }
    fn ui_auto_spell_id(&mut self, _: &NodeHandle, _: &Component, _: usize) -> Result<i32> {
        refuse("ui_auto_spell_id")
    }
    fn ui_select_auto_spell(&mut self, _: &NodeHandle, _: &Component, _: i32) -> Result<()> {
        refuse("ui_select_auto_spell")
    }
    fn ui_window_mode(&mut self, _: &NodeHandle) -> Result<i32> {
        refuse("ui_window_mode")
    }
    fn ui_set_window_mode(&mut self, _: &NodeHandle, _: i32) -> Result<()> {
        refuse("ui_set_window_mode")
    }
    fn ui_tutorial_complete(&mut self, _: &NodeHandle) -> Result<bool> {
        refuse("ui_tutorial_complete")
    }
    fn ui_tutorial_tabs(&mut self, _: &NodeHandle) -> Result<()> {
        refuse("ui_tutorial_tabs")
    }
    fn ui_has_runscript(&mut self, _: &NodeHandle) -> Result<bool> {
        refuse("ui_has_runscript")
    }
    fn ui_remove_runscript(&mut self, _: &NodeHandle) -> Result<()> {
        refuse("ui_remove_runscript")
    }
    fn ui_set_unclosable(&mut self, _: &NodeHandle, _: bool) -> Result<()> {
        refuse("ui_set_unclosable")
    }
    fn ui_unclosable(&mut self, _: &NodeHandle) -> Result<bool> {
        refuse("ui_unclosable")
    }
}
