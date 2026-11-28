//! Stealing from market stalls in the live world (`world::thieving_stall`,
//! corpus `fixtures/thieving-stalls`): ThievingOptionPlugin on scenery's
//! "steal-from", "steal from" and "steal" options, and StallThiefPulse as a
//! STANDARD pulse of the player's pulse manager whose `SkillPulse.stop()`
//! (the animation reset) also runs when the manager stops it.
//!
//! `Stealer` is the library's `Host` over the live world, for the player
//! and the scenery of the option: `inCombat`, the dynamic level, the
//! canonical inventory, the attributes, the scenery's definition name and
//! activity, `Scenery.transform` and `SceneryBuilder.replace` over the
//! cache scene, `RandomFunction.RANDOM`, the animator, the interaction
//! lock, the synth sound, the saved silk stamp, the local NPCs with their
//! combat pulses and forced chat, the manual hit and the stun.
//!
//! Not ported, and answered by name (docs/thieving-stalls.md): no quest is
//! ported, so `isQuestComplete` answers false and the Rellekka fur and fish
//! stalls always refuse; the ResourceProducedEvent is a named skip.
//!
//! Composed here rather than run: `stun`'s `pulseManager.clear()` reaches
//! the running stall pulse in the original (its `stop()` resets the
//! animation); this binding holds the pulse while its body runs, so the
//! reset is made before the stun. The binding has no JVM corpus.
use super::{super::host::Live, Ctx, Node, P, everyday::Everyday};
use crate::{
    definitions::{item::Shared, option_handler},
    player::attributes::AttributeValue,
    pulses,
    world::{
        action_locks,
        animation::{Animation, Priority},
        combat_pulse::{self as cp, impact},
        combat_swing, doors,
        entity_regions::Select,
        packet_handlers::Container as Held,
        repository,
        scenery::{self as builder, ReplaceMode},
        thieving_stall::{self as stalls, Error, Host, Result, StallThiefPulse},
    },
};
use rs09_protocol::game::updates;

/// `Skills.THIEVING`.
const THIEVING: i32 = 17;

/// The ThievingOptionPlugin object on the scenery definitions.
#[derive(Clone, Copy, Debug)]
pub struct StallPlugin;
impl option_handler::NativeOptionHandler for StallPlugin {}

/// The plugin and the running pulses.
#[derive(Default)]
pub struct StallWorld {
    /// The plugin (`this` of `newInstance`).
    pub plugin: Option<option_handler::Handle>,
    /// The stall pulses, by their pulse object, with the player's node and
    /// the scenery.
    pub pulses: Vec<(pulses::Handle, usize, doors::Handle, StallThiefPulse)>,
}

fn fail(e: impl std::fmt::Debug) -> Error {
    Error::Binding(format!("{e:?}"))
}

/// `world::thieving_stall`'s host over the live world.
struct Stealer<'a> {
    live: &'a mut Live,
    p: P,
    node: doors::Handle,
}
impl Stealer<'_> {
    fn player(&self) -> Node {
        Node::Player(self.p.0.clone())
    }
}

impl Host for Stealer<'_> {
    type Npc = P;

    fn world_ticks(&mut self) -> Result<i32> {
        Ok(self.live.inner.ticks)
    }
    fn set_thieve_delay(&mut self, ticks: i32) -> Result<()> {
        let attributes = self.live.inner.canonical(&self.p).player.attributes();
        attributes.set_value(stalls::THIEVE_DELAY, Some(AttributeValue::Int(ticks)));
        Ok(())
    }
    fn thieve_delay(&mut self) -> Result<i32> {
        let attributes = self.live.inner.canonical(&self.p).player.attributes();
        attributes
            .integer(stalls::THIEVE_DELAY, 0)
            .map_err(|_| Error::Cast)
    }
    fn checks_inactive(&mut self) -> Result<bool> {
        Ok(!self.live.inner.node_active(self.p.node()) || !self.node.read().active)
    }
    fn in_combat(&mut self) -> Result<bool> {
        repository::Host::in_combat(self.live, &self.p).map_err(fail)
    }
    fn message(&mut self, message: &str) -> Result<()> {
        self.live.inner.message(&self.p, message).map_err(fail)
    }
    fn level(&mut self) -> Result<i32> {
        combat_swing::Host::level(self.live, &self.p, THIEVING).map_err(fail)
    }
    fn node_name(&mut self) -> Result<String> {
        // `Node.name`, the definition's name the scenery was made with.
        self.node.read().name.clone().ok_or(Error::Null)
    }
    fn free_slots(&mut self) -> Result<i32> {
        let node = self.player();
        let inventory = Ctx(self.live)
            .container(&node, Held::Inventory)
            .map_err(fail)?;
        Ok(inventory.free_slots())
    }
    fn in_region(&mut self, region: i32) -> Result<bool> {
        let at = self
            .live
            .inner
            .state
            .node_location(self.p.node())
            .map_err(fail)?
            .ok_or(Error::Null)?;
        Ok(super::super::walk::region_id(&at) == region)
    }
    fn quest_complete(&mut self, _: &str) -> Result<bool> {
        self.live
            .inner
            .not_ported
            .default_answer("isQuestComplete (quests not ported: false)");
        Ok(false)
    }
    fn dialogue(&mut self, message: &str) -> Result<()> {
        let node = self.player();
        Everyday(Ctx(self.live))
            .dialogue(&node, message)
            .map_err(fail)
    }
    fn reset_animation(&mut self) -> Result<()> {
        let animation = Animation::with(-1, 0, Some(Priority::High));
        self.live
            .entity_visualize(&self.p, Some(animation), None)
            .map_err(fail)
    }
    fn animate(&mut self, id: i32) -> Result<()> {
        self.live
            .entity_visualize(&self.p, Some(Animation::new(id)), None)
            .map_err(fail)
    }
    fn lock_interactions(&mut self, ticks: i32) -> Result<()> {
        let state = &mut self.live.inner.state;
        state
            .locks(&self.p.0)
            .map_err(fail)?
            .lock_kind(
                action_locks::Kind::Interaction,
                ticks,
                state,
                &mut action_locks::Clock,
            )
            .map_err(fail)
    }
    fn play_audio(&mut self, id: i32) -> Result<()> {
        self.live
            .inner
            .queue_native(&self.p, updates::synth_sound(id, 1, 0));
        Ok(())
    }
    fn set_silk_steal(&mut self, after_millis: i64) -> Result<()> {
        let stamp = super::super::now_millis().wrapping_add(after_millis);
        self.live
            .inner
            .with_profile(&self.p, |profile| profile.saved.global.silk_steal = stamp)
            .map_err(fail)
    }
    fn node_active(&mut self) -> Result<bool> {
        Ok(self.node.read().active)
    }
    fn node_id(&mut self) -> Result<i32> {
        Ok(self.node.read().id)
    }
    fn replace(&mut self, id: i32, delay: i32) -> Result<()> {
        // `node.transform(id)`: the id at the node's location, with its
        // type and rotation.
        let (location, shape, rotation) = {
            let node = self.node.read();
            (node.location.clone(), node.shape, node.rotation)
        };
        let walk = &mut self.live.walk;
        let empty = crate::world::scenery_node::construct(
            &mut walk.scene.definitions,
            id,
            location,
            shape,
            rotation,
            false,
        )
        .map_err(fail)?;
        let mut world = walk.scene.context(&mut walk.maps);
        let mode = ReplaceMode::Temporary {
            clip: true,
            ticks: delay,
        };
        builder::replace(&mut world, Some(&self.node), Some(&empty), mode)
            .map(|_| ())
            .map_err(fail)
    }
    fn random(&mut self, max: i32) -> Result<i32> {
        // RandomFunction.random(maxValue): 0 without a draw below 1.
        if max <= 0 {
            return Ok(0);
        }
        // `RANDOM.nextInt(bound)` of a positive bound always answers.
        let drawn = self.live.inner.generators.shared_next_int(max);
        drawn.ok_or_else(|| fail("RandomFunction.RANDOM.nextInt"))
    }
    fn add_item(&mut self, id: i32, amount: i32) -> Result<()> {
        let node = self.player();
        Everyday(Ctx(self.live))
            .add_to_inventory(&node, id, amount)
            .map(|_| ())
            .map_err(fail)
    }
    fn add_experience(&mut self, xp: f64) -> Result<()> {
        cp::Host::add_experience(self.live, &self.p, THIEVING, xp, true).map_err(fail)
    }
    fn item_name(&mut self, id: i32) -> Result<String> {
        let name = self.live.inner.items.for_id(id).read().name.clone();
        name.ok_or(Error::Null)
    }
    fn resource_produced(&mut self, _: i32, _: i32) -> Result<()> {
        self.live
            .inner
            .not_ported
            .skip("ResourceProducedEvent dispatch (entity event hooks not ported)");
        Ok(())
    }
    fn stun(&mut self, ticks: i32) -> Result<()> {
        // `pulseManager.clear()` stops this pulse: `SkillPulse.stop()`.
        self.reset_animation()?;
        self.live.content_stun(&self.p, ticks, false).map_err(fail)
    }
    fn impact(&mut self, amount: i32) -> Result<()> {
        // `entity.impactHandler.manualHit(entity, amount, type)`, then
        // `playHurtAudio(entity)`.
        let hit = Some(cp::HitsplatType::Normal);
        impact::manual_hit(self.live, &self.p, Some(&self.p), amount, hit).map_err(fail)?;
        self.live.content_hurt_audio(&self.p, 0);
        Ok(())
    }
    fn local_npcs(&mut self, distance: i32) -> Result<Vec<P>> {
        let at = self
            .live
            .inner
            .state
            .node_location(self.p.node())
            .map_err(fail)?
            .ok_or(Error::Null)?;
        let nodes = self.live.local(at, distance, Select::Npcs).map_err(fail)?;
        Ok(nodes.into_iter().map(P).collect())
    }
    fn npc_attacking(&mut self, npc: &P) -> Result<bool> {
        cp::pulse::is_attacking(self.live, npc).map_err(fail)
    }
    fn npc_id(&mut self, npc: &P) -> Result<i32> {
        let row = self.live.inner.npcs.rows.get(&npc.0.node);
        row.map(|row| row.id).ok_or(Error::Null)
    }
    fn npc_chat(&mut self, npc: &P, text: &str) -> Result<()> {
        self.live
            .npc_chat(npc.node(), text.to_owned())
            .map(|_| ())
            .map_err(fail)
    }
    fn npc_attack(&mut self, npc: &P) -> Result<()> {
        self.live.combat_attack(npc, &self.p).map_err(fail)
    }
    fn run_pulse(&mut self, mut pulse: StallThiefPulse) -> Result<()> {
        // `SkillPulse(player, node)`: `Pulse(1, player, node)`. The player
        // leaving stops the pulse (super::super::round).
        let handle = Shared::new(pulses::Pulse::new(
            1,
            Some(Shared::new(Vec::new())),
            pulses::Class::Other,
            stalls::PULSE_CLASS.into(),
        ));
        let (p, node) = (self.p.clone(), self.node.clone());
        let mut started = Ok(());
        let submitted = self
            .live
            .run_content_pulse(&p, handle.clone(), |live| {
                let mut stealer = Stealer {
                    live,
                    p: p.clone(),
                    node: node.clone(),
                };
                started = pulse.start(&mut stealer);
                started.is_ok() && pulse.running
            })
            .map_err(fail)?;
        if let Err(error) = started {
            // `start()` threw inside PulseManager.run: nothing is submitted.
            pulses::stop_base(&handle);
            return Err(error);
        }
        if submitted {
            self.live.walk.stopping_pulses.push(handle.clone());
            let owner = p.node();
            self.live
                .round
                .stalls
                .pulses
                .push((handle, owner, node, pulse));
        }
        Ok(())
    }
}

impl Live {
    /// `ThievingOptionPlugin.newInstance(null)`, if the class scan names it.
    pub fn install_stalls(&mut self) -> std::result::Result<(), String> {
        let scanned = crate::startup::scan::section("scan.initializable");
        if !scanned.contains(&stalls::PLUGIN_CLASS) {
            return Ok(());
        }
        let plugin = option_handler::Handler::new(stalls::PLUGIN_CLASS, StallPlugin);
        for name in stalls::OPTIONS {
            option_handler::set_option_handler(
                &self.walk.scenery.defaults(),
                Some(name.into()),
                Some(plugin.clone()),
            );
        }
        self.round.stalls.plugin = Some(plugin);
        Ok(())
    }

    /// `ThievingOptionPlugin.handle(player, node, option)`.
    pub fn stall_handle(
        &mut self,
        player: &P,
        stall: &doors::Handle,
        option: &str,
    ) -> Result<bool> {
        let id = stall.read().id;
        let mut stealer = Stealer {
            live: self,
            p: player.clone(),
            node: stall.clone(),
        };
        let handled = stalls::handle(&mut stealer, id, option);
        self.inner.drain(player);
        handled
    }

    /// A stall pulse by identity: `Some` with whether the runner drops it.
    pub fn stall_pulse_body(&mut self, pulse: &pulses::Handle) -> Option<Result<bool>> {
        let pulses = &mut self.round.stalls.pulses;
        let at = pulses.iter().position(|(h, _, _, _)| h.same(pulse))?;
        let (handle, owner, node, mut task) = pulses.remove(at);
        if !self.inner.players.contains_key(&owner) {
            // The player left: the pulse ends.
            return Some(Ok(true));
        }
        let p = match self.inner.state.retain_node(owner) {
            Ok(h) => P(h),
            Err(e) => return Some(Err(fail(e))),
        };
        let mut stealer = Stealer {
            live: self,
            p: p.clone(),
            node: node.clone(),
        };
        let result = match task.update(&mut stealer) {
            // Pulse.update prints what the body threw and drops the pulse
            // (the vegetable stall 4708 has no empty id: `getEmpty` throws
            // after the sound, before any loot).
            Err(error @ (Error::Index | Error::Null | Error::Cast)) => {
                self.inner.log_line(format!(
                    "ERR  {} threw {error:?}: the pulse is dropped",
                    stalls::PULSE_CLASS
                ));
                Ok(true)
            }
            other => other,
        };
        self.inner.drain(&p);
        match result {
            Ok(false) => self.round.stalls.pulses.push((handle, owner, node, task)),
            _ => pulses::stop_base(&handle),
        }
        Some(result)
    }

    /// A stall pulse a pulse manager stopped: `SkillPulse.stop()`.
    pub fn stall_pulse_stopped(&mut self, pulse: &pulses::Handle) -> Option<Result<()>> {
        let pulses = &mut self.round.stalls.pulses;
        let at = pulses.iter().position(|(h, _, _, _)| h.same(pulse))?;
        let (_, owner, node, mut task) = pulses.remove(at);
        if !self.inner.players.contains_key(&owner) {
            return Some(Ok(()));
        }
        let p = match self.inner.state.retain_node(owner) {
            Ok(h) => P(h),
            Err(e) => return Some(Err(fail(e))),
        };
        Some(task.stop(&mut Stealer {
            live: self,
            p,
            node,
        }))
    }
}
