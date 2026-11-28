//! Interface buttons in the live world: `PacketProcessor`'s IfAction and
//! CloseIface bodies (`world::interface_packets`) over the player's
//! interface manager, its locks and its zone monitor, with the
//! InterfaceListeners registry (`world::interface_listeners`) and the
//! component plugins. The listeners and plugins are the global handlers'
//! (`world::global_handlers`, registered at boot by `register_scanned`,
//! [`super::global`]): their callbacks run through `registry::run_button`,
//! `run_open` and `run_plugin` with the live world as their host (the tab
//! classes `CombatTabInterface`, `EmoteTabInterface`, `MusicTabInterface`,
//! `LogoutTabInterface` and the rest). Everything else a button reaches
//! without a port is a named answer or a refusal.
use super::{
    P,
    host::{Live, fail},
};
use crate::world::{
    action_locks::{self, ActionLocks},
    component_objects::Plugin,
    driver,
    global_handlers::registry,
    interface_listeners::{self, Button, Callback},
    interface_packets::{self, Action, LoginPulse},
    interfaces::{self, Handle},
    player_tabs::emotes as tab_emotes,
    player_walk::NodeHandle,
    zones::Monitor,
};
use rs09_protocol::game::updates;

fn ui<E: std::fmt::Debug>(e: E) -> interfaces::Error {
    interfaces::Error::Binding(format!("{e:?}"))
}

pub(super) struct Buttons<'a> {
    pub(super) live: &'a mut Live,
}

impl Buttons<'_> {
    fn p(node: &NodeHandle) -> P {
        P(node.clone())
    }
    fn tabs(&mut self, p: &NodeHandle) -> super::tabs::Tabs<'_> {
        let live = &mut *self.live;
        super::tabs::Tabs {
            inner: &mut live.inner,
            p: P(p.clone()),
            walk: Some((&mut live.walk, &mut live.deferred)),
        }
    }
}

impl interface_listeners::Context for Buttons<'_> {
    fn invoke_open(
        &mut self,
        callback: Callback,
        player: &NodeHandle,
        component: &Handle,
    ) -> interfaces::Result<bool> {
        let Some(listener) = callback.value::<registry::Listener>().map(|l| *l) else {
            return Err(interfaces::Error::Binding(
                "an open or close listener this world did not register".into(),
            ));
        };
        let id = component.read().id;
        self.live.global_open(listener, player, id).map_err(ui)
    }
    fn invoke_button(
        &mut self,
        callback: Callback,
        player: &NodeHandle,
        component: &Handle,
        button: Button,
    ) -> interfaces::Result<bool> {
        // BankInterface's buttons (super::bank).
        if crate::world::bank_interface::Program::kind(&callback).is_some() {
            return self.live.bank_button(callback, player, component, button);
        }
        // The shop's on(620) and on(621) (super::shopping).
        if let Some(result) = self.live.shop_button(&callback, player, button) {
            return result;
        }
        // GlassCraftingListener's on(542) (super::items::crafting_wares).
        if let Some(result) = self.live.glass_button(&callback, player, button) {
            return result.map_err(ui);
        }
        // SilverCraftingListener's on(438) (super::items::crafting_stations).
        if let Some(result) = self.live.silver_button(&callback, player, button) {
            return result.map_err(ui);
        }
        let Some(listener) = callback.value::<registry::Listener>().map(|l| *l) else {
            return Err(interfaces::Error::Binding(
                "a button listener this world did not register".into(),
            ));
        };
        self.live
            .global_button(listener, player, button)
            .map_err(ui)
    }
}
impl interface_packets::Context for Buttons<'_> {
    type Dialogue = P;
    type Locks = ActionLocks;
    type Zone = Monitor<Live>;
    type Manager = interfaces::Manager;
    type Dispatch = P;
    fn debug(&mut self, p: &NodeHandle, text: String) -> interfaces::Result<()> {
        tab_emotes::Host::debug(&mut self.tabs(p), &text).map_err(ui)
    }
    fn dialogue(&mut self, p: &NodeHandle) -> interfaces::Result<P> {
        Ok(Self::p(p))
    }
    fn has_dialogue(&mut self, p: P) -> interfaces::Result<bool> {
        // player.getDialogueInterpreter().getDialogue() != null (super::talk).
        Ok(self.live.has_dialogue(&p))
    }
    fn close_dialogue(&mut self, p: P) -> interfaces::Result<()> {
        self.live.close_dialogue_of(&p).map(|_| ()).map_err(ui)
    }
    fn locks(&mut self, p: &NodeHandle) -> interfaces::Result<ActionLocks> {
        self.live.inner.state.locks(p).map_err(ui)
    }
    fn component_locked(&mut self, l: ActionLocks) -> interfaces::Result<bool> {
        l.is_kind_locked(
            action_locks::Kind::Component,
            &mut self.live.inner.state,
            &mut action_locks::Clock,
        )
        .map_err(ui)
    }
    fn zone(&mut self, p: &NodeHandle) -> interfaces::Result<Monitor<Live>> {
        Ok(self.live.monitor(p))
    }
    fn click_zone(&mut self, z: Monitor<Live>, a: Action) -> interfaces::Result<bool> {
        z.click_button(self.live, a.iface, a.child, a.slot, a.item, a.opcode)
            .map_err(ui)
    }
    fn manager(&mut self, p: &NodeHandle) -> interfaces::Result<interfaces::Manager> {
        self.live.inner.current = Some(Self::p(p));
        Ok(self.live.inner.canonical(&Self::p(p)).player.manager())
    }
    fn component(&mut self, m: interfaces::Manager, id: i32) -> interfaces::Result<Option<Handle>> {
        interfaces::get_component(&mut self.live.inner, &m, id)
    }
    fn hidden(&mut self, c: &Handle) -> interfaces::Result<bool> {
        Ok(c.read().hidden)
    }
    fn plugin(&mut self, c: &Handle) -> interfaces::Result<Option<Plugin>> {
        Ok(self.live.inner.registry.plugin(c))
    }
    fn id(&mut self, c: &Handle) -> interfaces::Result<i32> {
        Ok(c.read().id)
    }
    fn button_event(&mut self, _: &NodeHandle, _: i32, _: i32) -> interfaces::Result<()> {
        self.live
            .inner
            .not_ported
            .skip("ButtonClickEvent dispatch (entity event hooks not ported)");
        Ok(())
    }
    fn run_listener(&mut self, p: &NodeHandle, c: &Handle, b: Button) -> interfaces::Result<bool> {
        let registry = self.live.listeners.clone();
        registry.run(self, Some(p), Some(c), b)
    }
    fn handle_plugin(
        &mut self,
        plugin: Plugin,
        p: &NodeHandle,
        c: &Handle,
        b: Button,
    ) -> interfaces::Result<bool> {
        // TradeInterfacePlugin on 334, 335 and 336 (super::trading).
        if plugin.value::<crate::world::trade::TradePlugin>().is_some() {
            return self.live.trade_button(p, c, b).map_err(ui);
        }
        // SmeltingInterface on 311 (super::items::smelting).
        if plugin
            .value::<super::items::smelting::SmeltPlugin>()
            .is_some()
        {
            return self.live.smelting_button(&Self::p(p), b.button).map_err(ui);
        }
        // SmithingInterface on 300 (super::items::smithing).
        if plugin
            .value::<super::items::smithing::SmithPlugin>()
            .is_some()
        {
            return self.live.smithing_button(&Self::p(p), b.button).map_err(ui);
        }
        // JewelleryInterface on 446 (super::items::crafting_jewellery).
        if plugin
            .value::<super::items::crafting_jewellery::JewelleryPlugin>()
            .is_some()
        {
            return self.live.jewellery_button(&Self::p(p), b).map_err(ui);
        }
        // SpinningInterface on 459 (super::items::crafting_stations).
        if plugin
            .value::<super::items::crafting_stations::SpinPlugin>()
            .is_some()
        {
            return self.live.spin_button(&Self::p(p), b).map_err(ui);
        }
        let Some(plugin) = plugin.value::<registry::ComponentPlugin>().map(|p| *p) else {
            return Err(interfaces::Error::Binding(
                "a component plugin this world did not register".into(),
            ));
        };
        self.live.global_plugin(plugin, p, b).map_err(ui)
    }
    fn close(&mut self, m: interfaces::Manager) -> interfaces::Result<bool> {
        // InterfaceManager.close() with the world (a trade's close event).
        let owner = m.player().ok_or(interfaces::Error::Null)?;
        self.live.close_interfaces_of(&Self::p(&owner))
    }
    fn logging_present(&mut self, p: &NodeHandle) -> interfaces::Result<bool> {
        Ok(self
            .live
            .inner
            .canonical(&Self::p(p))
            .player
            .attributes()
            .get("logging_in")
            .is_some())
    }
    fn submit_login(&mut self, _: LoginPulse) -> interfaces::Result<()> {
        Err(interfaces::Error::Binding(
            "not ported in the live world: the deferred login world configuration".into(),
        ))
    }
    fn map_present(&mut self, p: &NodeHandle) -> interfaces::Result<bool> {
        Ok(self
            .live
            .inner
            .canonical(&Self::p(p))
            .player
            .attributes()
            .get("worldMap:viewing")
            .is_some())
    }
    fn remove_map(&mut self, p: &NodeHandle) -> interfaces::Result<()> {
        self.live
            .inner
            .canonical(&Self::p(p))
            .player
            .attributes()
            .remove("worldMap:viewing");
        Ok(())
    }
    fn dispatch(&mut self, p: &NodeHandle) -> interfaces::Result<P> {
        Ok(Self::p(p))
    }
    fn resizable(&mut self, m: interfaces::Manager) -> interfaces::Result<bool> {
        let p = self
            .live
            .inner
            .current
            .clone()
            .ok_or(interfaces::Error::Null)?;
        let _ = m;
        Ok(self.live.inner.canonical(&p).client.is_resizable())
    }
    fn windows_pane(&mut self, p: P, id: i32, kind: i32) -> interfaces::Result<()> {
        let counter = self
            .live
            .inner
            .canonical(&p)
            .player
            .manager()
            .packet_count(1);
        self.live
            .inner
            .queue_native(&p, updates::windows_pane(id, kind, counter));
        Ok(())
    }
    fn unlock(&mut self, p: &NodeHandle) -> interfaces::Result<()> {
        let locks = self.live.inner.state.locks(p).map_err(ui)?;
        locks.unlock();
        Ok(())
    }
    fn remove_logging(&mut self, p: &NodeHandle) -> interfaces::Result<()> {
        self.live
            .inner
            .canonical(&Self::p(p))
            .player
            .attributes()
            .remove("logging_in");
        Ok(())
    }
    fn configure_world(&mut self, _: &NodeHandle) -> interfaces::Result<()> {
        Err(interfaces::Error::Binding(
            "not ported in the live world: the deferred login world configuration".into(),
        ))
    }
}

impl Live {
    /// IfAction (`processIfAction`).
    pub fn if_action(&mut self, p: &P, a: Action) -> driver::Result<()> {
        let owner = p.0.clone();
        interface_packets::if_action(&mut Buttons { live: self }, &owner, a).map_err(fail)?;
        self.inner.drain(p);
        Ok(())
    }
    /// CloseIface (`processCloseIface`).
    pub fn close_iface(&mut self, p: &P) -> driver::Result<()> {
        let owner = p.0.clone();
        interface_packets::close_iface(&mut Buttons { live: self }, &owner).map_err(fail)?;
        self.inner.drain(p);
        Ok(())
    }
}

impl interface_packets::ContinueContext for Buttons<'_> {
    type DialogueAction = crate::world::dialogue::Action<Live>;
    fn close_chatbox(&mut self, m: interfaces::Manager) -> interfaces::Result<()> {
        interfaces::close_chatbox(&mut self.live.inner, &m)
    }
    fn pop_action(&mut self, p: P) -> interfaces::Result<Option<Self::DialogueAction>> {
        // getActions().pop(): the first queued action, if any.
        let actions = self.live.interpreter_of(&p).actions();
        let mut actions = actions.write();
        if actions.is_empty() {
            return Ok(None);
        }
        Ok(actions.remove(0))
    }
    fn handle_action(
        &mut self,
        a: Self::DialogueAction,
        p: &NodeHandle,
        child: i32,
    ) -> interfaces::Result<()> {
        a.handle(self.live, p, child).map_err(ui)
    }
    fn continue_dialogue(&mut self, p: P, iface: i32, child: i32) -> interfaces::Result<()> {
        // player.getDialogueInterpreter().handle(iface, child).
        let interpreter = self.live.interpreter_of(&p);
        interpreter.handle(self.live, iface, child).map_err(ui)
    }
}
