//! Jewellery casting in the live world (`world::crafting_jewellery`, whose
//! library has the JVM corpus `fixtures/crafting-jewellery`; this binding
//! has none): JewelleryCraftPlugin in `UseWithHandler.HANDLERS` under the
//! thirteen furnaces for the gold bar, JewelleryInterface as the component
//! plugin of 446 with its amount prompt, and JewelleryPulse on the
//! player's pulse manager with `SkillPulse.stop()` (the animation reset)
//! when it ends or the manager stops it.
//!
//! Joined: the canonical inventory, the dynamic level, the item
//! definitions' names, messages, the interface manager, the interface
//! config and item model frames, the experience, the animator and the
//! synth sound. Not joined: the Slayer ring's unlock (slayer is not
//! ported: the flag answers false by name, so the ring is always refused).
use super::{
    super::host::Live,
    Ctx, Node, P,
    everyday::Everyday,
    woodcutting::{Bound, bind},
};
use crate::{
    definitions::item::Shared,
    pulses,
    world::{
        animation::{Animation, Priority},
        combat_pulse, combat_swing,
        component_objects::{NativeObject, Plugin},
        crafting_jewellery::{
            self as jewellery, Error, Host, Input, Jewellery, JewelleryPulse, Result,
        },
        dialogue as d,
        interface_listeners::Button,
        packet_handlers::use_with::{HandlerData, OBJECT_TYPE},
    },
};
use rs09_game_core::java_lower::lowercase_root;
use rs09_protocol::game::updates;

/// JewelleryCraftPlugin's handler id in this world (`UseWithHandler.handle`,
/// super::milking's `use_handler_handle`).
pub const JEWELLERY_HANDLER: usize = 10;

/// JewelleryInterface, the component plugin of 446.
pub struct JewelleryPlugin;
impl NativeObject for JewelleryPlugin {}

/// The amount prompt's script: the player and the product.
#[derive(Clone)]
pub struct JewelleryScript(pub P, pub &'static Jewellery);

/// The running pulses in this world.
#[derive(Default)]
pub struct CraftingJewelleryWorld {
    /// The pulses with their player.
    pulses: Vec<(pulses::Handle, usize, JewelleryPulse)>,
}

fn fail(e: impl std::fmt::Debug) -> Error {
    Error::Binding(format!("{e:?}"))
}

/// `crafting_jewellery::Host` over the live world, for one player.
struct Crafter<'a> {
    live: &'a mut Live,
    p: P,
}
impl Crafter<'_> {
    fn node(&self) -> Node {
        Node::Player(self.p.0.clone())
    }
}
impl Host for Crafter<'_> {
    fn open_interface(&mut self, id: i32) -> Result<()> {
        crate::world::player_death::Host::open_interface(self.live, &self.p.0, id).map_err(fail)
    }
    fn close_interface(&mut self) -> Result<()> {
        crate::world::player_death::Host::close_interface(self.live, &self.p.0).map_err(fail)
    }
    fn contains(&mut self, id: i32, amount: i32) -> Result<bool> {
        let player = &self.live.inner.canonical(&self.p).player;
        Ok(player.inventory().contains(id, amount))
    }
    fn amount(&mut self, id: i32) -> Result<i32> {
        let item = Everyday(Ctx(self.live)).native(id, 1);
        let player = &self.live.inner.canonical(&self.p).player;
        Ok(player.inventory().amount(Some(&item)))
    }
    fn send_interface_config(&mut self, interface: i32, child: i32, hide: bool) -> Result<()> {
        let p = self.p.clone();
        d::Host::send_interface_config(self.live, p, interface, child, hide).map_err(fail)
    }
    fn send_item_zoom(&mut self, item: i32, zoom: i32, interface: i32, child: i32) -> Result<()> {
        let inner = &mut self.live.inner;
        let counter = inner.canonical(&self.p).player.manager().packet_count(1);
        let frame =
            updates::interface_object(interface.wrapping_shl(16) | child, item, zoom, counter);
        inner.queue_native(&self.p, frame);
        Ok(())
    }
    fn level(&mut self) -> Result<i32> {
        combat_swing::Host::level(self.live, &self.p, jewellery::CRAFTING).map_err(fail)
    }
    fn item_name_lowercase(&mut self, id: i32) -> Result<Option<String>> {
        let name = self.live.inner.items.for_id(id).read().name.clone();
        Ok(name.map(|name| lowercase_root(&name)))
    }
    fn send_message(&mut self, message: &str) -> Result<()> {
        self.live.inner.message(&self.p, message).map_err(fail)
    }
    fn send_messages(&mut self, messages: &[&str]) -> Result<()> {
        // `packetDispatch.sendMessages`: each in order.
        for message in messages {
            self.send_message(message)?;
        }
        Ok(())
    }
    fn ring_unlocked(&mut self) -> Result<bool> {
        self.live
            .inner
            .not_ported
            .default_answer("SlayerFlags.isRingUnlocked (slayer not ported: false)");
        Ok(false)
    }
    fn send_input(&mut self, jewellery: &'static Jewellery) -> Result<()> {
        let p = self.p.clone();
        self.live
            .jewellery_prompt(
                &p,
                "Enter the amount:",
                JewelleryScript(p.clone(), jewellery),
            )
            .map_err(fail)
    }
    fn run_pulse(&mut self, pulse: JewelleryPulse) -> Result<()> {
        let p = self.p.clone();
        self.live.run_jewellery_pulse(&p, pulse)
    }
    fn remove_all(&mut self, ids: &[i32]) -> Result<bool> {
        // `Container.remove(Item...)`: every item is tried; the container
        // is refreshed after each here, once after all in the original.
        let node = self.node();
        let mut all = true;
        for &id in ids {
            let mut everyday = Everyday(Ctx(self.live));
            let item = everyday.native(id, 1);
            if !everyday.remove_from_inventory(&node, &item).map_err(fail)? {
                all = false;
            }
        }
        self.live.inner.drain(&self.p);
        Ok(all)
    }
    fn add(&mut self, id: i32) -> Result<()> {
        let node = self.node();
        let added = Everyday(Ctx(self.live)).add_to_inventory(&node, id, 1);
        self.live.inner.drain(&self.p);
        added.map(|_| ()).map_err(fail)
    }
    fn add_experience(&mut self, xp: f64) -> Result<()> {
        let added =
            combat_pulse::Host::add_experience(self.live, &self.p, jewellery::CRAFTING, xp, true);
        self.live.inner.drain(&self.p);
        added.map_err(fail)
    }
    fn animate(&mut self, id: i32) -> Result<()> {
        self.live
            .entity_visualize(&self.p, Some(Animation::new(id)), None)
            .map_err(fail)
    }
    fn reset_animation(&mut self) -> Result<()> {
        self.live
            .entity_visualize(
                &self.p,
                Some(Animation::with(-1, 0, Some(Priority::High))),
                None,
            )
            .map_err(fail)
    }
    fn play_audio(&mut self, id: i32) -> Result<()> {
        // `playAudio(player, id)`: the synth sound, once, no delay.
        self.live
            .inner
            .queue_native(&self.p, updates::synth_sound(id, 1, 0));
        Ok(())
    }
}

impl Live {
    /// `JewelleryCraftPlugin.newInstance(null)` and
    /// `JewelleryInterface.newInstance(null)` (`ComponentDefinition.put(446,
    /// plugin)`), each if the class scan names it.
    pub fn install_crafting_jewellery(&mut self) -> std::result::Result<(), String> {
        let scanned = crate::startup::scan::section("scan.initializable");
        if scanned.contains(&jewellery::INTERFACE_CLASS) {
            self.inner
                .registry
                .for_id(jewellery::INTERFACE)
                .write()
                .plugin = Some(Plugin::new(JewelleryPlugin));
        }
        if scanned.contains(&jewellery::PLUGIN_CLASS) {
            let handler = Shared::new(HandlerData {
                name: "JewelleryCraftPlugin".into(),
                allowed_nodes: Some(vec![jewellery::GOLD_BAR]),
                dynamic: false,
                handler: JEWELLERY_HANDLER,
            });
            for furnace in jewellery::FURNACES {
                self.item_actions
                    .use_handlers
                    .add_handler(furnace, OBJECT_TYPE, &handler);
            }
        }
        Ok(())
    }

    /// `JewelleryCraftPlugin.handle(event)`.
    pub fn jewellery_handle(&mut self, player: &P) -> Bound<bool> {
        let mut host = Crafter {
            live: self,
            p: player.clone(),
        };
        let result = jewellery::handle(&mut host);
        self.inner.drain(player);
        result.map_err(bind)
    }

    /// `JewelleryInterface.handle`: a button of 446.
    pub fn jewellery_button(&mut self, player: &P, button: Button) -> Bound<bool> {
        let mut host = Crafter {
            live: self,
            p: player.clone(),
        };
        let result = jewellery::button(&mut host, button.opcode, button.button);
        self.inner.drain(player);
        result.map_err(bind)
    }

    /// The amount prompt answered.
    pub fn jewellery_amount(&mut self, script: &JewelleryScript, value: &Input) -> Result<()> {
        let p = script.0.clone();
        let mut host = Crafter {
            live: self,
            p: p.clone(),
        };
        let result = jewellery::input(&mut host, script.1, value);
        self.inner.drain(&p);
        result
    }

    /// `PulseManager.run(pulse)`: the constructor leaves the pulse stopped;
    /// `start()` runs it when the level holds.
    fn run_jewellery_pulse(&mut self, p: &P, mut pulse: JewelleryPulse) -> Result<()> {
        self.round
            .crafting_jewellery
            .pulses
            .retain(|(handle, ..)| handle.read().running);
        // `Pulse(1, player, null)`: no node is checked, and the player
        // leaving stops the pulse.
        let handle = Shared::new(pulses::Pulse::new(
            1,
            Some(Shared::new(Vec::new())),
            pulses::Class::Other,
            jewellery::PULSE_CLASS.into(),
        ));
        let mut started = Ok(());
        let submitted = self
            .run_content_pulse(p, handle.clone(), |live| {
                let mut host = Crafter { live, p: p.clone() };
                started = pulse.start(&mut host);
                started.is_ok() && pulse.running
            })
            .map_err(fail)?;
        if let Err(error) = started {
            // `start()` threw inside PulseManager.run: nothing is submitted.
            pulses::stop_base(&handle);
            return Err(error);
        }
        if submitted {
            self.walk.stopping_pulses.push(handle.clone());
            let pulses = &mut self.round.crafting_jewellery.pulses;
            pulses.push((handle, p.node(), pulse));
        }
        Ok(())
    }

    /// A jewellery pulse by identity: `Some` with whether the runner drops
    /// it.
    pub fn crafting_jewellery_pulse_body(&mut self, pulse: &pulses::Handle) -> Option<Bound<bool>> {
        let pulses = &mut self.round.crafting_jewellery.pulses;
        let at = pulses.iter().position(|(h, ..)| h.same(pulse))?;
        let (handle, owner, mut task) = pulses.remove(at);
        if !self.inner.players.contains_key(&owner) {
            // The player left: the pulse ends.
            return Some(Ok(true));
        }
        let p = match self.inner.state.retain_node(owner) {
            Ok(h) => P(h),
            Err(e) => return Some(Err(bind(e))),
        };
        let mut host = Crafter { live: self, p };
        match task.pulse(&mut host) {
            Ok(false) => {
                let pulses = &mut self.round.crafting_jewellery.pulses;
                pulses.push((handle, owner, task));
                Some(Ok(false))
            }
            finished => {
                // `Pulse.update()` stops a finished pulse: `SkillPulse.stop()`
                // resets the animation.
                let stopped = task.stop(&mut host);
                pulses::stop_base(&handle);
                Some(finished.and(stopped).map(|()| true).map_err(bind))
            }
        }
    }

    /// A jewellery pulse a pulse manager stopped: `SkillPulse.stop()`.
    pub fn crafting_jewellery_pulse_stopped(
        &mut self,
        pulse: &pulses::Handle,
    ) -> Option<Bound<()>> {
        let pulses = &mut self.round.crafting_jewellery.pulses;
        let at = pulses.iter().position(|(h, ..)| h.same(pulse))?;
        let (_, owner, mut task) = pulses.remove(at);
        if !self.inner.players.contains_key(&owner) {
            return Some(Ok(()));
        }
        let p = match self.inner.state.retain_node(owner) {
            Ok(h) => P(h),
            Err(e) => return Some(Err(bind(e))),
        };
        let mut host = Crafter { live: self, p };
        Some(task.stop(&mut host).map_err(bind))
    }
}
