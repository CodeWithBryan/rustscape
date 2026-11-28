//! Praying at altars in the live world (`world::prayer_altar`, ported
//! without a JVM corpus): PrayerAltarListener's "pray" and "pray-at"
//! options of its 87 altars, and the chaos altar 412's queued trap.
//!
//! Joined: the static levels, the prayer points, `Skills.setLevel`, `lock`,
//! the animation, sound and messages, `queueScript`, the INSTANT teleport
//! (`properties.teleportLocation`) and `stopExecuting`. `hasRequirement`
//! answers false by name (no quest is registered in this world), so the
//! tribal statue 3863 never recharges. `PrayerPointsRechargeEvent` is
//! skipped by name (entity event hooks are not ported). The listener's
//! second registration, the chaos altar 61's "check" (Merlin's Crystal's
//! stage), is not installed and is named at boot.
use super::{
    super::{
        host::Live,
        scripts::{Queued, ScriptCall},
    },
    Node, P,
    woodcutting::{Bound, bind},
};
use crate::{
    scripts::{self, Strength},
    world::{
        action_locks,
        animation::Animation,
        combat_swing,
        doors::Location,
        interaction_listeners::{self as listeners, Function, FunctionKind},
        prayer_altar::{self as altar, Error, Host, Result},
    },
};
use rs09_protocol::game::updates;

/// The `on(altars, IntType.SCENERY, "pray", "pray-at")` lambda.
pub struct Pray;
impl crate::world::component_objects::NativeObject for Pray {}

fn fail(e: impl std::fmt::Debug) -> Error {
    Error::Binding(format!("{e:?}"))
}

/// `prayer_altar::Host` over the live world, for one player.
struct Praying<'a> {
    live: &'a mut Live,
    p: P,
}
impl Host for Praying<'_> {
    fn has_requirement(&mut self, quest: &str) -> Result<bool> {
        // No quest is registered in this world: false, counted by name.
        crate::world::login::Host::has_requirement(self.live, &self.p, quest, true).map_err(fail)
    }
    fn static_level(&mut self, skill: i32) -> Result<i32> {
        combat_swing::Host::static_level(self.live, &self.p, skill).map_err(fail)
    }
    fn prayer_points(&mut self) -> Result<f64> {
        self.live
            .inner
            .with_profile(&self.p, |p| p.skills.prayer_points)
            .map_err(fail)
    }
    fn message(&mut self, message: &str) -> Result<()> {
        self.live.inner.message(&self.p, message).map_err(fail)
    }
    fn lock(&mut self, ticks: i32) -> Result<()> {
        let state = &mut self.live.inner.state;
        let locks = state.locks(&self.p.0).map_err(fail)?;
        locks
            .lock(ticks, state, &mut action_locks::Clock)
            .map_err(fail)
    }
    fn animate(&mut self, id: i32) -> Result<()> {
        self.live
            .entity_visualize(&self.p, Some(Animation::new(id)), None)
            .map_err(fail)
    }
    fn play_audio(&mut self, id: i32) -> Result<()> {
        // `playAudio(player, id)`: the synth sound, once, no delay.
        self.live
            .inner
            .queue_native(&self.p, updates::synth_sound(id, 1, 0));
        Ok(())
    }
    fn set_level(&mut self, skill: i32, level: i32) -> Result<()> {
        let p = self.p.clone();
        let set = self
            .live
            .with_skills(&p, |skills, ctx| skills.set_level(skill, level, ctx))
            .map_err(fail)?;
        self.live.inner.drain(&p);
        set.map_err(fail)
    }
    fn dispatch_recharge(&mut self) -> Result<()> {
        self.live
            .inner
            .not_ported
            .skip("PrayerPointsRechargeEvent dispatch (entity event hooks not ported)");
        Ok(())
    }
    fn queue_trap(&mut self, delay: i32) -> Result<()> {
        let function = Function::new(FunctionKind::Void, altar::QUEUED_LAMBDA, Queued::AltarTrap);
        let call = ScriptCall::Queue(function, delay, Strength::Strong, false);
        self.live
            .script_call(&self.p, call)
            .map(|_| ())
            .map_err(fail)
    }
    fn teleport(&mut self, to: [i32; 3]) -> Result<()> {
        // TeleportType.INSTANT: `entity.properties.teleportLocation = loc`.
        let [x, y, z] = to;
        self.live
            .inner
            .state
            .actor(self.p.node())
            .map_err(fail)?
            .queue
            .state
            .write()
            .entity
            .teleport_location = Some(Location::new(x, y, z));
        Ok(())
    }
    fn stop_executing(&mut self) -> Result<bool> {
        self.live
            .script_call(&self.p, ScriptCall::Stop)
            .map_err(fail)
    }
}

impl Live {
    /// `PrayerAltarListener.defineListeners`' first registration.
    pub fn define_prayer_altar_listener(&mut self) -> std::result::Result<(), String> {
        let casing = super::super::global::RootCasing;
        let options: Vec<Vec<u16>> = altar::OPTIONS
            .iter()
            .map(|option| option.encode_utf16().collect())
            .collect();
        self.item_actions
            .listeners
            .clone()
            .add_ids(
                &casing,
                &altar::ALTARS,
                listeners::Kind::Scenery.ordinal(),
                &options,
                Function::new(FunctionKind::On, altar::LAMBDA, Pray),
            )
            .map_err(|e| format!("{}: {e:?}", altar::LISTENER_CLASS))?;
        self.inner.not_ported.skip(
            "PrayerAltarListener's \"check\" of the chaos altar 61 (quest stages are content)",
        );
        Ok(())
    }

    /// The "pray" lambda.
    pub fn altar_pray(&mut self, player: &Node, node: &Node) -> Bound<bool> {
        let (Node::Player(player), Node::Scenery(scenery)) = (player, node) else {
            return Err(bind("the pray option of a node that is not scenery"));
        };
        let p = P(player.clone());
        let id = scenery.read().id;
        let mut host = Praying {
            live: self,
            p: p.clone(),
        };
        let result = altar::pray_at(&mut host, id);
        self.inner.drain(&p);
        result.map_err(bind)
    }

    /// The trap the chaos altar queued.
    pub fn altar_trap_queued(&mut self, p: &P) -> scripts::Result<bool> {
        let mut host = Praying {
            live: self,
            p: p.clone(),
        };
        altar::trap(&mut host)
            .map_err(|e| scripts::Error::Binding(format!("PrayerAltarListener: {e:?}")))
    }
}
