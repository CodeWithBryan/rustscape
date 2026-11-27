//! Requests and trading in the live world (`world::trade`, docs/trade.md):
//! each player's RequestManager fields and TradeModule extension
//! ([`TradeWorld`]), the "Trade with" and "Req Assist" player options (the
//! global handlers' RequestOption listeners, reached through the native
//! InteractionListeners with the walk to the player), TradeInterfacePlugin on
//! 334/335/336 from the IfAction path, Offer-X/Remove-X through the input
//! prompts, the TradeCloseEvent, and the alert pulse on `GameWorld.Pulser`
//! (dispatched by identity).
//!
//! [`TradeHost`] is `trade::Host` over the whole live world;
//! `trade::ui::Ui<TradeHost>` is also the interface context the Close packet
//! and a walk's interface close use, so a trade's close event runs with the
//! world wherever a player's main interface is closed by those paths.
use super::{P, host::Live, now_millis};
use crate::{
    definitions::item::Registry as Items,
    inventory::{
        self as inv,
        container::{Container, Event, Factory},
        container_listener::Handle as Listener,
        item::Handle as Item,
    },
    player::{attributes::AttributeValue, ironman, variables},
    pulses,
    world::{
        interfaces::{self, CloseEvent, Handle as Component, Manager, Plugin},
        player_walk::NodeHandle,
        trade::{self, AmountScript, Bot, Components, Error, Module, RequestType, Result},
    },
};
use rs09_protocol::game::Frame;
use std::{collections::HashMap, sync::Arc};

/// The request and trade state of the live world.
#[derive(Default)]
pub struct TradeWorld {
    /// TradeModule's companion components (336, 335, 334), made at boot.
    pub components: Option<Components>,
    /// `getExtension(TradeModule.class)`, by player node.
    pub modules: HashMap<usize, Module<NodeHandle>>,
    /// RequestManager's `target` field.
    pub targets: HashMap<usize, NodeHandle>,
    /// The `lastRequest` attribute.
    pub last: HashMap<usize, RequestType>,
    /// The alert pulses submitted to `GameWorld.Pulser`: player and slot.
    pub alerts: Vec<(pulses::Handle, NodeHandle, i32)>,
}

pub struct TradeHost<'a> {
    pub live: &'a mut Live,
}

fn service(e: impl std::fmt::Debug) -> Error {
    Error::Service(format!("{e:?}"))
}
fn refuse<T>(what: &str) -> Result<T> {
    Err(Error::Service(format!(
        "not ported in the live world: {what}"
    )))
}

impl TradeHost<'_> {
    fn p(&self, owner: &NodeHandle) -> Result<P> {
        if self.live.inner.players.contains_key(&owner.node) {
            Ok(P(owner.clone()))
        } else {
            Err(Error::ClassCast)
        }
    }
    fn ui<T>(
        &mut self,
        owner: &NodeHandle,
        f: impl FnOnce(&mut super::inner::Inner) -> interfaces::Result<T>,
    ) -> Result<T> {
        let p = self.p(owner)?;
        let previous = self.live.inner.current.replace(p);
        let result = f(&mut self.live.inner);
        self.live.inner.current = previous;
        result.map_err(Into::into)
    }
    fn attribute(&self, owner: &NodeHandle, key: &str) -> Result<Option<AttributeValue>> {
        let p = self.p(owner)?;
        Ok(self.live.inner.canonical(&p).player.attributes().get(key))
    }
    fn inventory_listener(
        &mut self,
        listener: &Listener,
        container: &mut Container,
        event: Option<Event>,
    ) -> Result<()> {
        let Some(receiver) = listener.value::<inv::listener::Listener>() else {
            return refuse("a container listener other than the inventory's and the trade's");
        };
        let owner = receiver.owner().ok_or(Error::Null)?;
        let p = self.p(&owner)?;
        let player = self.live.inner.canonical(&p).player.clone();
        let inner = &mut self.live.inner;
        let result = player.apply(
            &mut inner.state,
            &mut inner.frames,
            now_millis,
            |_, _, context| match &event {
                Some(event) => receiver.update_event(context, container, Some(event)),
                None => receiver.refresh(context, container),
            },
        );
        inner.drain(&p);
        result.map_err(Into::into)
    }
}
impl trade::Host for TradeHost<'_> {
    type Player = NodeHandle;
    fn ticks(&mut self) -> Result<i32> {
        Ok(self.live.inner.ticks)
    }
    fn dev_mode(&mut self) -> Result<bool> {
        Ok(self.live.inner.settings.dev_mode)
    }
    fn definitions(&mut self) -> Result<Arc<Items>> {
        Ok(self.live.inner.items.clone())
    }
    fn factory(&mut self) -> Result<Factory> {
        Ok(Factory {
            null_item: self.live.inner.factory.null_item.clone(),
        })
    }
    fn components(&mut self) -> Result<Components> {
        self.live.round.trade.components.clone().ok_or(Error::Null)
    }
    fn username(&mut self, p: &NodeHandle) -> Result<String> {
        let p = self.p(p)?;
        Ok(self.live.inner.slot(&p).name.clone())
    }
    fn location(&mut self, p: &NodeHandle) -> Result<trade::Location> {
        let [x, y, z] = self
            .live
            .inner
            .state
            .node_location(p.node)
            .map_err(service)?
            .ok_or(Error::Null)?
            .coordinates();
        Ok(trade::Location { x, y, z })
    }
    fn is_active(&mut self, p: &NodeHandle) -> Result<bool> {
        Ok(self.live.inner.node_active(p.node))
    }
    fn is_artificial(&mut self, _: &NodeHandle) -> Result<bool> {
        Ok(false)
    }
    fn inventory(&mut self, p: &NodeHandle) -> Result<Container> {
        let p = self.p(p)?;
        Ok(self.live.inner.canonical(&p).player.inventory())
    }
    fn ironman(&mut self, p: &NodeHandle) -> Result<ironman::Manager> {
        let p = self.p(p)?;
        Ok(self.live.inner.canonical(&p).player.ironman())
    }
    fn interface_manager(&mut self, p: &NodeHandle) -> Result<Manager> {
        let p = self.p(p)?;
        Ok(self.live.inner.canonical(&p).player.manager())
    }
    fn ip_address(&mut self, p: &NodeHandle) -> Result<String> {
        let p = self.p(p)?;
        self.live
            .inner
            .with_profile(&p, |profile| {
                profile.details.ip_address().unwrap_or_default().to_owned()
            })
            .map_err(service)
    }
    fn mac_address(&mut self, p: &NodeHandle) -> Result<String> {
        let p = self.p(p)?;
        self.live
            .inner
            .with_profile(&p, |profile| {
                profile.details.uid.mac.clone().unwrap_or_default()
            })
            .map_err(service)
    }
    fn computer_name(&mut self, p: &NodeHandle) -> Result<String> {
        let p = self.p(p)?;
        self.live
            .inner
            .with_profile(&p, |profile| {
                profile
                    .details
                    .uid
                    .computer_name
                    .clone()
                    .unwrap_or_default()
            })
            .map_err(service)
    }
    fn attribute_int(&mut self, p: &NodeHandle, key: &'static str, default: i32) -> Result<i32> {
        match self.attribute(p, key)? {
            None => Ok(default),
            Some(AttributeValue::Int(n)) => Ok(n),
            Some(_) => Err(Error::ClassCast),
        }
    }
    fn set_attribute_int(&mut self, p: &NodeHandle, key: &'static str, value: i32) -> Result<()> {
        let p = self.p(p)?;
        self.live
            .inner
            .canonical(&p)
            .player
            .attributes()
            .set_value(key, Some(AttributeValue::Int(value)));
        Ok(())
    }
    fn attribute_bool(&mut self, p: &NodeHandle, key: &'static str, default: bool) -> Result<bool> {
        match self.attribute(p, key)? {
            None => Ok(default),
            Some(AttributeValue::Bool(b)) => Ok(b),
            Some(_) => Err(Error::ClassCast),
        }
    }
    fn last_request(&mut self, p: &NodeHandle) -> Result<Option<RequestType>> {
        Ok(self.live.round.trade.last.get(&p.node).copied())
    }
    fn set_last_request(&mut self, p: &NodeHandle, kind: RequestType) -> Result<()> {
        self.p(p)?;
        self.live.round.trade.last.insert(p.node, kind);
        Ok(())
    }
    fn request_target(&mut self, p: &NodeHandle) -> Result<Option<NodeHandle>> {
        Ok(self.live.round.trade.targets.get(&p.node).cloned())
    }
    fn set_request_target(&mut self, p: &NodeHandle, target: Option<NodeHandle>) -> Result<()> {
        match target {
            Some(target) => {
                self.live.round.trade.targets.insert(p.node, target);
            }
            None => {
                self.live.round.trade.targets.remove(&p.node);
            }
        }
        Ok(())
    }
    fn trade_module(&mut self, p: &NodeHandle) -> Result<Option<Module<NodeHandle>>> {
        Ok(self.live.round.trade.modules.get(&p.node).cloned())
    }
    fn add_trade_module(&mut self, p: &NodeHandle, module: Module<NodeHandle>) -> Result<()> {
        self.live.round.trade.modules.insert(p.node, module);
        Ok(())
    }
    fn remove_trade_module(&mut self, p: &NodeHandle) -> Result<()> {
        self.live.round.trade.modules.remove(&p.node);
        Ok(())
    }
    fn message(&mut self, p: &NodeHandle, text: &str) -> Result<()> {
        let p = self.p(p)?;
        self.live.inner.message(&p, text).map_err(service)
    }
    fn content_message(&mut self, p: &NodeHandle, text: &str) -> Result<()> {
        // ContentAPI.sendMessage: player.sendMessages(*splitLines(message, 86)).
        let p = self.p(p)?;
        let units: Vec<u16> = text.encode_utf16().collect();
        let lines = crate::world::plain_dialogue::split_lines(&units, 86).map_err(service)?;
        for line in lines {
            self.live
                .inner
                .message(&p, &String::from_utf16_lossy(&line))
                .map_err(service)?;
        }
        Ok(())
    }
    fn set_varp(&mut self, p: &NodeHandle, id: i32, value: i32) -> Result<()> {
        let p = self.p(p)?;
        self.live
            .inner
            .variables(&p, |ctx, owner| {
                variables::set_varp(ctx, Some(owner), id, value, false)
            })
            .map_err(service)
    }
    fn send(&mut self, p: &NodeHandle, frame: Frame) -> Result<()> {
        let p = self.p(p)?;
        self.live.inner.queue_native(&p, frame);
        Ok(())
    }
    fn dialogue_close(&mut self, p: &NodeHandle) -> Result<()> {
        let p = self.p(p)?;
        self.live.close_dialogue_of(&p).map(|_| ()).map_err(service)
    }
    fn zone_can_request(
        &mut self,
        p: &NodeHandle,
        _: RequestType,
        target: &NodeHandle,
    ) -> Result<bool> {
        // ZoneMonitor.canRequest(type, target) over the player's zones.
        let monitor = self.live.monitor(p);
        monitor
            .can_request(self.live, Some(&()), Some(target))
            .map_err(service)
    }
    fn bot(&mut self, _: &str) -> Result<Option<Bot>> {
        // AIRepository.PulseRepository holds no bot (bots are not ported).
        self.live
            .inner
            .not_ported
            .default_answer("AIRepository.PulseRepository (bots are not ported: no bot)");
        Ok(None)
    }
    fn bot_trade_received(&mut self, _: &str, _: &NodeHandle) -> Result<()> {
        refuse("DoublingMoney.tradeReceived (bots)")
    }
    fn bot_items_received(&mut self, _: &str, _: &NodeHandle, _: &Container) -> Result<()> {
        refuse("DoublingMoney.itemsReceived (bots)")
    }
    fn assist_open(&mut self, _: &NodeHandle, _: &NodeHandle) -> Result<()> {
        refuse("AssistSession.open (the assist session is not installed)")
    }
    fn log_trade(
        &mut self,
        _: &NodeHandle,
        _: &NodeHandle,
        _: &Container,
        _: &Container,
    ) -> Result<()> {
        self.live
            .inner
            .not_ported
            .skip("PlayerMonitor.logTrade (logged only)");
        Ok(())
    }
    fn ground(&mut self, item: &Item, p: &NodeHandle) -> Result<()> {
        // GroundItemManager.create(item, player): at the player's location.
        let at = self
            .live
            .inner
            .state
            .node_location(p.node)
            .map_err(service)?;
        let manager = self.live.walk.ground.manager.clone();
        crate::world::ground_manager::create_at(
            self.live,
            &manager,
            Some(item.clone()),
            at,
            Some(p.clone()),
        )
        .map(|_| ())
        .map_err(service)
    }
    fn submit_alert(&mut self, p: &NodeHandle, slot: i32) -> Result<()> {
        // GameWorld.getPulser().submit(new Pulse(1, player) { ... }).
        let handle = crate::definitions::item::Shared::new(pulses::Pulse::new(
            1,
            Some(crate::definitions::item::Shared::new(vec![Some(p.node)])),
            pulses::Class::Other,
            "core.game.container.impl.TradeContainer$1".into(),
        ));
        self.live
            .driver
            .pulser()
            .submit(Some(handle.clone()))
            .map_err(service)?;
        self.live.round.trade.alerts.push((handle, p.clone(), slot));
        Ok(())
    }
    fn input_dialogue(
        &mut self,
        p: &NodeHandle,
        numeric: bool,
        prompt: &str,
        script: AmountScript<NodeHandle>,
    ) -> Result<()> {
        let p = self.p(p)?;
        self.live
            .trade_prompt(&p, numeric, prompt, script)
            .map_err(service)
    }
    fn examine(&mut self, item: &Item) -> Result<String> {
        let definition = item.read().definition.clone().ok_or(Error::Null)?;
        crate::world::packet_handlers::examine::item_examine_text(&definition, |s| s.to_lowercase())
            .map_err(service)
    }
    fn listener_update(
        &mut self,
        listener: Listener,
        container: &mut Container,
        event: Event,
    ) -> Result<()> {
        self.inventory_listener(&listener, container, Some(event))
    }
    fn listener_refresh(&mut self, listener: Listener, container: &mut Container) -> Result<()> {
        self.inventory_listener(&listener, container, None)
    }
    fn ui_validate(&mut self, p: &NodeHandle, m: &Manager) -> Result<()> {
        self.ui(p, |inner| interfaces::Context::validate_manager(inner, m))
    }
    fn ui_component(&mut self, p: &NodeHandle, id: i32) -> Result<Component> {
        self.ui(p, |inner| interfaces::Context::component(inner, id))
    }
    fn ui_resizable(&mut self, p: &NodeHandle) -> Result<bool> {
        self.ui(p, interfaces::Context::resizable)
    }
    fn ui_log(&mut self, p: &NodeHandle, text: String) -> Result<()> {
        self.ui(p, |inner| interfaces::Context::log(inner, text))
    }
    fn ui_listener_open(&mut self, p: &NodeHandle, c: &Component) -> Result<()> {
        // A global handler's onOpen runs with the whole world
        // (super::global_ui); every other listener through the interface
        // context.
        self.p(p)?;
        if let Some(result) = self.live.global_interface_listener(p, c, false) {
            return result.map(|_| ()).map_err(Into::into);
        }
        self.ui(p, |inner| interfaces::Context::listener_open(inner, c))
    }
    fn ui_listener_close(&mut self, p: &NodeHandle, c: &Component) -> Result<bool> {
        self.p(p)?;
        if let Some(result) = self.live.global_interface_listener(p, c, true) {
            return result.map_err(Into::into);
        }
        self.ui(p, |inner| interfaces::Context::listener_close(inner, c))
    }
    fn ui_plugin_open(&mut self, p: &NodeHandle, plugin: Plugin, c: &Component) -> Result<()> {
        self.ui(p, |inner| {
            interfaces::Context::plugin_open(inner, plugin, c)
        })
    }
    fn ui_close_event(&mut self, p: &NodeHandle, event: CloseEvent, c: &Component) -> Result<bool> {
        self.ui(p, |inner| interfaces::Context::close_event(inner, event, c))
    }
    fn ui_dispatch_open(&mut self, p: &NodeHandle, c: Component) -> Result<()> {
        self.ui(p, |inner| interfaces::Context::dispatch_open(inner, c))
    }
    fn ui_dispatch_close(&mut self, p: &NodeHandle, c: Option<Component>) -> Result<()> {
        self.ui(p, |inner| interfaces::Context::dispatch_close(inner, c))
    }
    fn ui_weapon_extension(&mut self, p: &NodeHandle) -> Result<Option<Component>> {
        self.ui(p, interfaces::Context::weapon_extension)
    }
    fn ui_create_weapon(&mut self, p: &NodeHandle) -> Result<Component> {
        self.ui(p, interfaces::Context::create_weapon)
    }
    fn ui_add_weapon(&mut self, p: &NodeHandle, c: Component) -> Result<()> {
        self.ui(p, |inner| interfaces::Context::add_weapon(inner, c))
    }
    fn ui_spellbook(&mut self, p: &NodeHandle) -> Result<i32> {
        self.ui(p, interfaces::Context::spellbook)
    }
    fn ui_has_familiar(&mut self, p: &NodeHandle) -> Result<bool> {
        self.ui(p, interfaces::Context::has_familiar)
    }
    fn ui_friends_component(&mut self, p: &NodeHandle) -> Result<Component> {
        self.ui(p, interfaces::Context::friends_component)
    }
    fn ui_autocast(&mut self, p: &NodeHandle) -> Result<Option<usize>> {
        self.ui(p, interfaces::Context::autocast)
    }
    fn ui_auto_spell_id(
        &mut self,
        p: &NodeHandle,
        weapon: &Component,
        spell: usize,
    ) -> Result<i32> {
        self.ui(p, |inner| {
            interfaces::Context::auto_spell_id(inner, weapon, spell)
        })
    }
    fn ui_select_auto_spell(&mut self, p: &NodeHandle, weapon: &Component, id: i32) -> Result<()> {
        self.ui(p, |inner| {
            interfaces::Context::select_auto_spell(inner, weapon, id)
        })
    }
    fn ui_window_mode(&mut self, p: &NodeHandle) -> Result<i32> {
        self.ui(p, interfaces::Context::window_mode)
    }
    fn ui_set_window_mode(&mut self, p: &NodeHandle, mode: i32) -> Result<()> {
        self.ui(p, |inner| interfaces::Context::set_window_mode(inner, mode))
    }
    fn ui_tutorial_complete(&mut self, p: &NodeHandle) -> Result<bool> {
        self.ui(p, interfaces::Context::tutorial_complete)
    }
    fn ui_tutorial_tabs(&mut self, p: &NodeHandle) -> Result<()> {
        self.ui(p, interfaces::Context::tutorial_tabs)
    }
    fn ui_has_runscript(&mut self, p: &NodeHandle) -> Result<bool> {
        self.ui(p, interfaces::Context::has_runscript)
    }
    fn ui_remove_runscript(&mut self, p: &NodeHandle) -> Result<()> {
        self.ui(p, interfaces::Context::remove_runscript)
    }
    fn ui_set_unclosable(&mut self, p: &NodeHandle, value: bool) -> Result<()> {
        self.ui(p, |inner| {
            interfaces::Context::set_unclosable_attribute(inner, value)
        })
    }
    fn ui_unclosable(&mut self, p: &NodeHandle) -> Result<bool> {
        self.ui(p, interfaces::Context::unclosable_attribute)
    }
}

impl Live {
    /// `player.requestManager.request(target, type)`.
    pub fn trade_request(&mut self, player: &P, target: &P, kind: RequestType) -> Result<bool> {
        let result =
            trade::request::request(&mut TradeHost { live: self }, &player.0, &target.0, kind);
        self.drain_all();
        result
    }
    /// TradeInterfacePlugin's `handle` on 334, 335 and 336.
    pub fn trade_button(
        &mut self,
        owner: &NodeHandle,
        component: &Component,
        button: crate::world::interface_listeners::Button,
    ) -> Result<bool> {
        let result = trade::plugin::handle(
            &mut TradeHost { live: self },
            owner,
            component,
            button.opcode,
            button.button,
            button.slot,
            button.item,
        );
        self.drain_all();
        result
    }
    /// An Offer-X/Remove-X answer (`plugin::run_amount`).
    pub fn trade_amount(
        &mut self,
        script: &AmountScript<NodeHandle>,
        value: &crate::world::input_prompt::Response,
    ) -> Result<()> {
        let result = trade::plugin::run_amount(&mut TradeHost { live: self }, script, value);
        self.drain_all();
        result
    }
    /// `InterfaceManager.close()` with the world (a trade's close event runs
    /// with it): the Close packet's and a walk's.
    pub fn close_interfaces_of(&mut self, p: &P) -> interfaces::Result<bool> {
        let manager = self.inner.canonical(p).player.manager();
        let owner = p.0.clone();
        let mut host = TradeHost { live: self };
        let result = interfaces::close(&mut trade::ui::Ui::new(&mut host, &owner), &manager);
        self.drain_all();
        result
    }
    /// The alert pulse's body (`TradeContainer`'s `Pulse(1, player)`), if
    /// this world submitted the pulse.
    pub fn trade_pulse(&mut self, pulse: &pulses::Handle) -> Option<Result<bool>> {
        let at = self
            .round
            .trade
            .alerts
            .iter()
            .position(|(h, _, _)| h.same(pulse))?;
        let (_, player, slot) = self.round.trade.alerts.remove(at);
        if !self.inner.players.contains_key(&player.node) {
            return Some(Ok(true));
        }
        let result = trade::module::alert_pulse(&mut TradeHost { live: self }, &player, slot);
        self.drain_all();
        Some(result)
    }
    /// TradeModule's companion components and TradeInterfacePlugin's
    /// registration (`ComponentDefinition.put(334/335/336, plugin)`).
    pub fn install_trade(&mut self) {
        trade::register_plugin(&mut self.inner.registry);
        self.round.trade.components = Some(trade::components(&mut self.inner.registry));
    }
}
