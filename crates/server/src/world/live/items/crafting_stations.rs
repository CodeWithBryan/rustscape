//! Spinning and silver casting in the live world
//! (`world::crafting_stations`, whose library has the JVM corpus
//! `fixtures/crafting-stations`; this binding has none):
//! SpinSpinningWheelPlugin on the ten wheels' "spin" option,
//! SpinningInterface as the component plugin of 459 with its amount
//! prompt, SpinningPulse on the player's pulse manager with
//! `SkillPulse.stop()` (the animation reset) when it ends or the manager
//! stops it, SilverCraftingListener's `onUseWith` registration for the
//! furnaces, its `onOpen(438)` and `on(438)` with its amount prompt, and
//! SilverCraftingPulse.
//!
//! Joined: the canonical inventory and equipment, the dynamic level, the
//! item definitions' names, messages, the experience, the Seers' Village
//! diary, the location, the bowstring counter in the player's attributes,
//! the animator, the synth sound, the interface manager and the item
//! models of 438. Not joined: the ResourceProducedEvent (entity event
//! hooks are not ported; skipped by name). The furnace attribute
//! ("crafting:silver:furnace_id") is kept in this world's state by player
//! and not in the player's attribute store; only the event reads it.
use super::{
    super::host::Live,
    Ctx, Node, P,
    everyday::Everyday,
    fletching::SEERS_VILLAGE,
    woodcutting::{Bound, bind},
};
use crate::{
    definitions::{item::Shared, option_handler},
    player::attributes::AttributeValue,
    pulses,
    world::{
        animation::{Animation, Priority},
        combat_pulse, combat_swing,
        component_objects::{NativeObject, Plugin},
        crafting_stations::{
            self as stations, Error, Host, Input, Models, Result, Silver, SilverPulse, Spin,
            SpinPulse,
        },
        dialogue::NodeHandle,
        interaction_listeners::{self as listeners, Function, FunctionKind},
        interface_listeners::{Button, Callback},
        player_managers::diary,
    },
};
use rs09_protocol::game::updates;

/// `MapDistance.RENDERING.distance`, `withinDistance(other)`'s default.
const RENDERING: i32 = 15;

/// The SpinSpinningWheelPlugin object on the scenery definitions.
#[derive(Clone, Copy, Debug)]
pub struct WheelPlugin;
impl option_handler::NativeOptionHandler for WheelPlugin {}

/// SpinningInterface, the component plugin of 459.
pub struct SpinPlugin;
impl NativeObject for SpinPlugin {}

/// SilverCraftingListener's `onUseWith`, `onOpen(438)` and `on(438)`
/// lambdas.
pub struct SilverUse;
impl NativeObject for SilverUse {}
pub struct SilverOpen;
impl NativeObject for SilverOpen {}
pub struct SilverButtons;
impl NativeObject for SilverButtons {}

/// The two amount prompts' scripts: the player and the product.
#[derive(Clone)]
pub struct SpinScript(pub P, pub &'static Spin);
#[derive(Clone)]
pub struct SilverScript(pub P, pub &'static Silver);

/// One of the two pulses, as a pulse manager holds it.
pub enum Pulse {
    Spin(SpinPulse),
    Silver(SilverPulse<Node>),
}

/// The plugin, the pulses and the furnaces in this world.
#[derive(Default)]
pub struct CraftingStationsWorld {
    /// The wheel plugin (`this` of `newInstance`).
    pub plugin: Option<option_handler::Handle>,
    /// The running pulses with their player.
    pulses: Vec<(pulses::Handle, usize, Pulse)>,
    /// "crafting:silver:furnace_id", by the player's node.
    furnaces: Vec<(usize, Node)>,
}

fn fail(e: impl std::fmt::Debug) -> Error {
    Error::Binding(format!("{e:?}"))
}

/// `crafting_stations::Host` over the live world, for one player.
struct Crafter<'a> {
    live: &'a mut Live,
    p: P,
}
impl Crafter<'_> {
    fn node(&self) -> Node {
        Node::Player(self.p.0.clone())
    }
    fn location(&mut self) -> Result<[i32; 3]> {
        let at = self
            .live
            .inner
            .state
            .node_location(self.p.node())
            .map_err(fail)?
            .ok_or(Error::Null)?;
        Ok(at.coordinates())
    }
}
/// `onOpen(438)`'s host: the listener runs inside the interface manager's
/// open, over the inner world (super::super::inner's `listener_open`).
struct Opening<'a> {
    inner: &'a mut super::super::inner::Inner,
    p: P,
}
impl Models for Opening<'_> {
    fn send_item_on_interface(&mut self, interface: i32, child: i32, item: i32) -> Result<()> {
        // `player.packetDispatch.sendItemOnInterface(item, 1, iface, child)`.
        let counter = self
            .inner
            .canonical(&self.p)
            .player
            .manager()
            .packet_count(1);
        let frame = updates::interface_object(interface.wrapping_shl(16) | child, item, 1, counter);
        self.inner.queue_native(&self.p, frame);
        Ok(())
    }
}
impl Models for Crafter<'_> {
    fn send_item_on_interface(&mut self, interface: i32, child: i32, item: i32) -> Result<()> {
        Opening {
            inner: &mut self.live.inner,
            p: self.p.clone(),
        }
        .send_item_on_interface(interface, child, item)
    }
}
impl super::super::inner::Inner {
    /// SilverCraftingListener's `onOpen(438)`, if the callback is it.
    pub fn silver_open_listener(
        &mut self,
        callback: &Callback,
    ) -> Option<crate::world::interfaces::Result<bool>> {
        callback.value::<SilverOpen>()?;
        let p = self.current.clone()?;
        let result = stations::silver_open(&mut Opening { inner: self, p });
        Some(result.map_err(|e| crate::world::interfaces::Error::Binding(format!("{e:?}"))))
    }
}
impl Host for Crafter<'_> {
    type Furnace = Node;

    fn open_interface(&mut self, id: i32) -> Result<()> {
        crate::world::player_death::Host::open_interface(self.live, &self.p.0, id).map_err(fail)
    }
    fn close_interface(&mut self) -> Result<()> {
        crate::world::player_death::Host::close_interface(self.live, &self.p.0).map_err(fail)
    }
    fn contains(&mut self, id: i32) -> Result<bool> {
        let player = &self.live.inner.canonical(&self.p).player;
        Ok(player.inventory().contains(id, 1))
    }
    fn amount(&mut self, id: i32) -> Result<i32> {
        let item = Everyday(Ctx(self.live)).native(id, 1);
        let player = &self.live.inner.canonical(&self.p).player;
        Ok(player.inventory().amount(Some(&item)))
    }
    fn item_name(&mut self, id: i32) -> Result<String> {
        let name = self.live.inner.items.for_id(id).read().name.clone();
        name.ok_or(Error::Null)
    }
    fn send_message(&mut self, message: &str) -> Result<()> {
        self.live.inner.message(&self.p, message).map_err(fail)
    }
    fn level(&mut self) -> Result<i32> {
        combat_swing::Host::level(self.live, &self.p, stations::CRAFTING).map_err(fail)
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
    fn seers_hard_complete(&mut self) -> Result<bool> {
        let p = self.p.clone();
        super::super::subclasses::with_diaries(self.live, &p, |h, owner| {
            let manager = diary::Host::diaries(h, owner)?;
            let found = manager.diaries.iter().find(|d| d.kind == SEERS_VILLAGE);
            diary::level_complete(found.ok_or(crate::world::player_managers::Error::Null)?, 2)
        })
        .map_err(fail)
    }
    fn near_seers_wheel(&mut self) -> Result<bool> {
        // `withinDistance(other)`: the same plane and a Euclidean distance
        // of at most the rendering distance.
        let [x, y, z] = self.location()?;
        let [wx, wy, wz] = stations::SEERS_WHEEL;
        let (dx, dy) = (i64::from(x - wx), i64::from(y - wy));
        Ok(z == wz && ((dx * dx + dy * dy) as f64).sqrt() <= f64::from(RENDERING))
    }
    fn hat(&mut self) -> Result<Option<i32>> {
        // `getEquipment().get(EquipmentContainer.SLOT_HAT)`.
        let equipment = self.live.inner.canonical(&self.p).player.equipment();
        Ok(equipment.get(0).map(|item| item.read().id()))
    }
    fn remove(&mut self, id: i32) -> Result<bool> {
        let node = self.node();
        let mut everyday = Everyday(Ctx(self.live));
        let item = everyday.native(id, 1);
        let removed = everyday.remove_from_inventory(&node, &item);
        self.live.inner.drain(&self.p);
        removed.map_err(fail)
    }
    fn add(&mut self, id: i32) -> Result<()> {
        self.add_item(id, 1).map(|_| ())
    }
    fn add_item(&mut self, id: i32, amount: i32) -> Result<bool> {
        let node = self.node();
        let added = Everyday(Ctx(self.live)).add_to_inventory(&node, id, amount);
        self.live.inner.drain(&self.p);
        added.map_err(fail)
    }
    fn add_experience(&mut self, xp: f64) -> Result<()> {
        let added =
            combat_pulse::Host::add_experience(self.live, &self.p, stations::CRAFTING, xp, true);
        self.live.inner.drain(&self.p);
        added.map_err(fail)
    }
    fn reward_xp(&mut self, xp: f64) -> Result<()> {
        // `addExperience(skill, xp)`: `playerMod` false.
        let added =
            combat_pulse::Host::add_experience(self.live, &self.p, stations::CRAFTING, xp, false);
        self.live.inner.drain(&self.p);
        added.map_err(fail)
    }
    fn region_id(&mut self) -> Result<i32> {
        let [x, y, _] = self.location()?;
        Ok((x >> 6 << 8) | (y >> 6))
    }
    fn bowstring_task_complete(&mut self) -> Result<bool> {
        let p = self.p.clone();
        super::super::subclasses::with_diaries(self.live, &p, |h, owner| {
            diary::has_completed_task(h, owner, Some(SEERS_VILLAGE), 0, 4)
        })
        .map_err(fail)
    }
    fn bowstrings(&mut self) -> Result<i32> {
        let attributes = self.live.inner.canonical(&self.p).player.attributes();
        attributes
            .integer(stations::BOWSTRINGS, 0)
            .map_err(|_| Error::Cast)
    }
    fn set_bowstrings(&mut self, value: i32) -> Result<()> {
        let attributes = self.live.inner.canonical(&self.p).player.attributes();
        attributes.set_value(stations::BOWSTRINGS_SAVED, Some(AttributeValue::Int(value)));
        Ok(())
    }
    fn finish_bowstring_task(&mut self) -> Result<()> {
        let p = self.p.clone();
        let finished = super::super::subclasses::with_diaries(self.live, &p, |h, owner| {
            diary::finish_task(h, owner, owner, Some(SEERS_VILLAGE), 0, 4)
        });
        self.live.inner.drain(&self.p);
        finished.map_err(fail)
    }
    fn send_spin_input(&mut self, spin: &'static Spin) -> Result<()> {
        let p = self.p.clone();
        self.live
            .spin_prompt(&p, "Enter the amount:", SpinScript(p.clone(), spin))
            .map_err(fail)
    }
    fn run_spin_pulse(&mut self, pulse: SpinPulse) -> Result<()> {
        let p = self.p.clone();
        self.live.run_spin_pulse(&p, pulse)
    }
    fn set_furnace(&mut self, furnace: &Node) -> Result<()> {
        let node = self.p.node();
        let players = &self.live.inner.players;
        let furnaces = &mut self.live.round.crafting_stations.furnaces;
        // The attributes of a player who left went with the player.
        furnaces.retain(|(owner, _)| *owner != node && players.contains_key(owner));
        furnaces.push((node, furnace.clone()));
        Ok(())
    }
    fn furnace(&mut self) -> Result<Option<Node>> {
        let node = self.p.node();
        let furnaces = &self.live.round.crafting_stations.furnaces;
        Ok(furnaces
            .iter()
            .find(|(owner, _)| *owner == node)
            .map(|(_, furnace)| furnace.clone()))
    }
    fn send_silver_input(&mut self, silver: &'static Silver) -> Result<()> {
        let p = self.p.clone();
        self.live
            .silver_prompt(&p, "Enter the amount:", SilverScript(p.clone(), silver))
            .map_err(fail)
    }
    fn submit_silver_pulse(&mut self, pulse: SilverPulse<Node>) -> Result<()> {
        let p = self.p.clone();
        self.live.run_silver_pulse(&p, pulse).map_err(fail)
    }
    fn resource_produced(&mut self, _id: i32, _amount: i32, _furnace: &Option<Node>) -> Result<()> {
        self.live
            .inner
            .not_ported
            .skip("ResourceProducedEvent dispatch (entity event hooks not ported)");
        Ok(())
    }
}

impl Live {
    /// `SpinSpinningWheelPlugin.newInstance(null)` and
    /// `SpinningInterface.newInstance(null)` (`ComponentDefinition.put(459,
    /// plugin)`), each if the class scan names it.
    pub fn install_crafting_stations(&mut self) -> std::result::Result<(), String> {
        let scanned = crate::startup::scan::section("scan.initializable");
        if scanned.contains(&stations::SPIN_CLASS) {
            self.inner
                .registry
                .for_id(stations::SPIN_INTERFACE)
                .write()
                .plugin = Some(Plugin::new(SpinPlugin));
        }
        if scanned.contains(&stations::WHEEL_CLASS) {
            let plugin = option_handler::Handler::new(stations::WHEEL_CLASS, WheelPlugin);
            for id in stations::WHEELS {
                let definition = self.walk.scene.definitions.for_id(id);
                let handlers = definition.read().handlers.clone();
                handlers.write().insert(
                    "option:spin".to_owned(),
                    crate::definitions::property::Property::OptionHandler(plugin.clone()),
                );
            }
            self.round.crafting_stations.plugin = Some(plugin);
        }
        Ok(())
    }

    /// `SilverCraftingListener.defineListeners` and
    /// `defineInterfaceListeners`.
    pub fn define_silver_crafting_listener(&mut self) -> std::result::Result<(), String> {
        let class = |e: &dyn std::fmt::Debug| format!("{}: {e:?}", stations::SILVER_CLASS);
        self.item_actions
            .listeners
            .clone()
            .add_use_ids(
                listeners::Kind::Scenery.ordinal(),
                stations::SILVER_BAR,
                &stations::SILVER_FURNACES,
                Function::new(
                    FunctionKind::UseWith,
                    stations::SILVER_USE_LAMBDA,
                    SilverUse,
                ),
            )
            .map_err(|e| class(&e))?;
        self.listeners
            .add_open(stations::SILVER_INTERFACE, Callback::open(SilverOpen))
            .map_err(|e| class(&e))?;
        self.listeners
            .add(
                stations::SILVER_INTERFACE,
                None,
                Callback::button(SilverButtons),
            )
            .map_err(|e| class(&e))
    }

    /// `SpinSpinningWheelPlugin.handle(player, node, option)`.
    pub fn wheel_handle(&mut self, player: &P) -> Bound<bool> {
        let mut host = Crafter {
            live: self,
            p: player.clone(),
        };
        let result = stations::wheel_handle(&mut host);
        self.inner.drain(player);
        result.map_err(bind)
    }

    /// `SpinningInterface.handle`: a button of 459.
    pub fn spin_button(&mut self, player: &P, button: Button) -> Bound<bool> {
        let mut host = Crafter {
            live: self,
            p: player.clone(),
        };
        let result = stations::spin_button(&mut host, button.opcode, button.button);
        self.inner.drain(player);
        result.map_err(bind)
    }

    /// The spinning amount prompt answered.
    pub fn spin_amount(&mut self, script: &SpinScript, value: &Input) -> Result<()> {
        let p = script.0.clone();
        let mut host = Crafter {
            live: self,
            p: p.clone(),
        };
        let result = stations::spin_input(&mut host, script.1, value);
        self.inner.drain(&p);
        result
    }

    /// SilverCraftingListener's use-with lambda, if the function is it.
    pub fn crafting_station_use(
        &mut self,
        f: &Function,
        player: &Node,
        with: &Node,
    ) -> Option<Bound<bool>> {
        f.value::<SilverUse>()?;
        let Node::Player(player) = player else {
            return Some(Err(bind(
                "a silver bar used by a node that is not a player",
            )));
        };
        let p = P(player.clone());
        let mut host = Crafter {
            live: self,
            p: p.clone(),
        };
        let result = stations::silver_use(&mut host, with);
        self.inner.drain(&p);
        Some(result.map_err(bind))
    }

    /// SilverCraftingListener's `on(438)`, if the callback is it.
    pub fn silver_button(
        &mut self,
        callback: &Callback,
        owner: &NodeHandle,
        button: Button,
    ) -> Option<Bound<bool>> {
        callback.value::<SilverButtons>()?;
        let p = P(owner.clone());
        let mut host = Crafter {
            live: self,
            p: p.clone(),
        };
        let result = stations::silver_button(&mut host, button.opcode, button.button);
        self.inner.drain(&p);
        Some(result.map_err(bind))
    }

    /// The silver amount prompt answered.
    pub fn silver_amount(&mut self, script: &SilverScript, value: &Input) -> Result<()> {
        let p = script.0.clone();
        let mut host = Crafter {
            live: self,
            p: p.clone(),
        };
        let result = stations::silver_input(&mut host, script.1, value);
        self.inner.drain(&p);
        result
    }

    fn station_handle(&mut self, class: &'static str) -> pulses::Handle {
        self.round
            .crafting_stations
            .pulses
            .retain(|(handle, ..)| handle.read().running);
        // `Pulse(1, player, node)` with an Item, which is never
        // deactivated, and `Pulse()`: the player leaving stops the pulse.
        Shared::new(pulses::Pulse::new(
            1,
            Some(Shared::new(Vec::new())),
            pulses::Class::Other,
            class.into(),
        ))
    }

    /// `PulseManager.run(pulse)` of a SpinningPulse: the constructor leaves
    /// the pulse stopped; `start()` runs it when the requirements hold.
    fn run_spin_pulse(&mut self, p: &P, mut pulse: SpinPulse) -> Result<()> {
        let handle = self.station_handle(stations::SPIN_PULSE_CLASS);
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
            let pulses = &mut self.round.crafting_stations.pulses;
            pulses.push((handle, p.node(), Pulse::Spin(pulse)));
        }
        Ok(())
    }

    /// `submitIndividualPulse(player, SilverCraftingPulse(...))`: the pulse
    /// does not override `start()`.
    fn run_silver_pulse(&mut self, p: &P, pulse: SilverPulse<Node>) -> Bound<()> {
        let handle = self.station_handle(stations::SILVER_PULSE_CLASS);
        let submitted = self
            .run_content_pulse(p, handle.clone(), |_| true)
            .map_err(bind)?;
        if submitted {
            self.walk.stopping_pulses.push(handle.clone());
            let pulses = &mut self.round.crafting_stations.pulses;
            pulses.push((handle, p.node(), Pulse::Silver(pulse)));
        }
        Ok(())
    }

    /// One of the two pulses by identity: `Some` with whether the runner
    /// drops it.
    pub fn crafting_station_pulse_body(&mut self, pulse: &pulses::Handle) -> Option<Bound<bool>> {
        let pulses = &mut self.round.crafting_stations.pulses;
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
        let result = match &mut task {
            Pulse::Spin(spin) => spin.pulse(&mut host),
            Pulse::Silver(silver) => {
                let result = silver.pulse(&mut host);
                // `delay = 5`: the pulse's own change of its delay.
                handle.write().delay = silver.delay;
                result
            }
        };
        match result {
            Ok(false) => {
                let pulses = &mut self.round.crafting_stations.pulses;
                pulses.push((handle, owner, task));
                Some(Ok(false))
            }
            finished => {
                // `Pulse.update()` stops a finished pulse: SpinningPulse's
                // `SkillPulse.stop()` resets the animation.
                let stopped = match &mut task {
                    Pulse::Spin(spin) => spin.stop(&mut host),
                    Pulse::Silver(_) => Ok(()),
                };
                pulses::stop_base(&handle);
                Some(finished.and(stopped).map(|()| true).map_err(bind))
            }
        }
    }

    /// One of the two pulses a pulse manager stopped: `SkillPulse.stop()`
    /// of a SpinningPulse; SilverCraftingPulse does not override `stop()`.
    pub fn crafting_station_pulse_stopped(&mut self, pulse: &pulses::Handle) -> Option<Bound<()>> {
        let pulses = &mut self.round.crafting_stations.pulses;
        let at = pulses.iter().position(|(h, ..)| h.same(pulse))?;
        let (_, owner, task) = pulses.remove(at);
        let Pulse::Spin(mut spin) = task else {
            return Some(Ok(()));
        };
        if !self.inner.players.contains_key(&owner) {
            return Some(Ok(()));
        }
        let p = match self.inner.state.retain_node(owner) {
            Ok(h) => P(h),
            Err(e) => return Some(Err(bind(e))),
        };
        let mut host = Crafter { live: self, p };
        Some(spin.stop(&mut host).map_err(bind))
    }
}
