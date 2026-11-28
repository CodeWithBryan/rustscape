//! Bone burying in the live world (`world::bone_burying`, ported without a
//! JVM corpus): BoneBuryListener's "bury" option of the 31 `Bones` items and
//! the STRONG script it queues on the player's ScriptProcessor
//! (super::super::scripts).
//!
//! Joined: the SKILLING clock, `stopWalk`, `lock`, the message, animation
//! and sound, `queueScript`, `replaceSlot` with an empty item,
//! `BarbarianTraining`'s pyre bonus attribute, `rewardXP` and
//! `stopExecuting`. `BoneBuryEvent` is skipped by name (entity event hooks
//! are not ported), so the jobs' bone burying counter never moves.
use super::{
    super::{
        host::Live,
        scripts::{Queued, ScriptCall},
    },
    Ctx, Node, P,
    woodcutting::{Bound, bind},
};
use crate::{
    inventory::item::Handle as ItemHandle,
    scripts::{self, Strength},
    world::{
        action_locks,
        animation::Animation,
        bone_burying::{self as bones, Error, Host, Result},
        combat_pulse,
        interaction_listeners::{self as listeners, Function, FunctionKind},
    },
};
use rs09_protocol::game::updates;

/// The `on(Bones.array, IntType.ITEM, "bury")` lambda.
pub struct Bury;
impl crate::world::component_objects::NativeObject for Bury {}

fn fail(e: impl std::fmt::Debug) -> Error {
    Error::Binding(format!("{e:?}"))
}

/// `bone_burying::Host` over the live world, for one player.
struct Burial<'a> {
    live: &'a mut Live,
    p: P,
}
impl Host for Burial<'_> {
    type Item = ItemHandle;

    fn clock_ready(&mut self) -> Result<bool> {
        let clock = self
            .live
            .script_clock(&self.p, bones::SKILLING as usize)
            .map_err(fail)?;
        Ok(clock <= self.live.inner.ticks)
    }
    fn in_inventory(&mut self, id: i32) -> Result<bool> {
        let player = &self.live.inner.canonical(&self.p).player;
        Ok(player.inventory().contains(id, 1))
    }
    fn stop_walk(&mut self) -> Result<()> {
        self.live.walk_reset(&self.p).map_err(fail)
    }
    fn lock(&mut self, ticks: i32) -> Result<()> {
        let state = &mut self.live.inner.state;
        let locks = state.locks(&self.p.0).map_err(fail)?;
        locks
            .lock(ticks, state, &mut action_locks::Clock)
            .map_err(fail)
    }
    fn delay_clock(&mut self, ticks: i32) -> Result<()> {
        let value = self.live.inner.ticks.wrapping_add(ticks);
        self.live
            .set_script_clock(&self.p, bones::SKILLING as usize, value)
            .map_err(fail)
    }
    fn message(&mut self, message: &str) -> Result<()> {
        self.live.inner.message(&self.p, message).map_err(fail)
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
    fn queue_strong(&mut self, delay: i32, id: i32, item: ItemHandle) -> Result<()> {
        let function = Function::new(
            FunctionKind::Void,
            bones::QUEUED_LAMBDA,
            Queued::BoneBury(id, item),
        );
        let call = ScriptCall::Queue(function, delay, Strength::Strong, false);
        self.live
            .script_call(&self.p, call)
            .map(|_| ())
            .map_err(fail)
    }
    fn slot(&mut self, item: &ItemHandle) -> Result<i32> {
        Ok(item.read().index)
    }
    fn empty_slot(&mut self, slot: i32) -> Result<Option<ItemHandle>> {
        // `item.amount <= 0`: `cont.replace(null, slot)`.
        let player = Node::Player(self.p.0.clone());
        let replaced = Ctx(self.live).with_container(
            &player,
            crate::world::packet_handlers::Container::Inventory,
            |c, defs, ctx| c.replace(None, slot, true, defs, ctx),
        );
        self.live.inner.drain(&self.p);
        replaced.map_err(|e| match e {
            crate::world::packet_handlers::Error::Index(_) => Error::IndexOutOfBounds,
            e => fail(e),
        })
    }
    fn same(&mut self, a: &ItemHandle, b: &ItemHandle) -> bool {
        a.same(b)
    }
    fn attribute_int(&mut self, key: &str, default: i32) -> Result<i32> {
        let attributes = self.live.inner.canonical(&self.p).player.attributes();
        attributes.integer(key, default).map_err(|_| Error::Cast)
    }
    fn set_attribute_int(&mut self, key: &str, value: i32) -> Result<()> {
        let attributes = self.live.inner.canonical(&self.p).player.attributes();
        attributes.set_integer(key, value);
        Ok(())
    }
    fn reward_xp(&mut self, xp: f64) -> Result<()> {
        let added =
            combat_pulse::Host::add_experience(self.live, &self.p, bones::PRAYER, xp, false);
        self.live.inner.drain(&self.p);
        added.map_err(fail)
    }
    fn dispatch_bone_bury(&mut self, _: i32) -> Result<()> {
        self.live
            .inner
            .not_ported
            .skip("BoneBuryEvent dispatch (entity event hooks not ported)");
        Ok(())
    }
    fn stop_executing(&mut self) -> Result<bool> {
        self.live
            .script_call(&self.p, ScriptCall::Stop)
            .map_err(fail)
    }
}

impl Live {
    /// `BoneBuryListener.defineListeners`.
    pub fn define_bone_bury_listener(&mut self) -> std::result::Result<(), String> {
        let casing = super::super::global::RootCasing;
        self.item_actions
            .listeners
            .clone()
            .add_ids(
                &casing,
                &bones::ids(),
                listeners::Kind::Item.ordinal(),
                &["bury".encode_utf16().collect()],
                Function::new(FunctionKind::On, bones::LAMBDA, Bury),
            )
            .map_err(|e| format!("{}: {e:?}", bones::LISTENER_CLASS))
    }

    /// The "bury" lambda.
    pub fn bone_bury(&mut self, player: &Node, node: &Node) -> Bound<bool> {
        let (Node::Player(player), Node::Item(item)) = (player, node) else {
            return Err(bind("the bury option of a node that is not an item"));
        };
        let p = P(player.clone());
        let id = item.read().id();
        let mut host = Burial {
            live: self,
            p: p.clone(),
        };
        let result = bones::bury(&mut host, id, item.clone());
        self.inner.drain(&p);
        result.map_err(bind)
    }

    /// The script the lambda queued.
    pub fn bone_bury_queued(&mut self, p: &P, id: i32, item: &ItemHandle) -> scripts::Result<bool> {
        let mut host = Burial {
            live: self,
            p: p.clone(),
        };
        bones::buried(&mut host, id, item)
            .map_err(|e| scripts::Error::Binding(format!("BoneBuryListener: {e:?}")))
    }
}
