//! Firemaking in the live world (`world::firemaking`, ported without a JVM
//! corpus): FiremakingListener's two inventory registrations (a tinderbox
//! or a bow used with logs), FireMakingPulse and BarbFirePulse.
//!
//! - The fire is `SceneryBuilder.add(object, life, ash)` over the scene. The
//!   scene's pulse hands the ash back as a token ([`Maps::ash_due`]), and
//!   [`Live::firemaking_tick`] (a content tick listener) creates the ground
//!   item then, so the ashes appear on the tick the fire goes out or the
//!   one after, with the 200 ticks the original's decay time leaves them.
//! - `LitFireEvent` is skipped by name (entity event hooks are not ported),
//!   so no diary or tutorial hook sees a fire.
//! - Not installed: the listener's three ground item registrations (a
//!   tinderbox or bow used on logs on the ground, and the logs' "light"
//!   option); the live world routes no ground item listener.
//! - `replaceSlot` is joined for the case the pulses reach, an unstacked log
//!   whose slot is emptied.
use super::{
    super::{ground_items::World, host::Live},
    Ctx, Node, P,
    everyday::Everyday,
    woodcutting::{Bound, bind},
};
use crate::{
    definitions::item::Shared,
    inventory::item::Item,
    pulses,
    world::{
        animation::{Animation, Priority},
        combat_pulse, combat_swing,
        doors::{Handle as Scenery, Location},
        firemaking::{self, Error, Host, Kind, Pulse, Result},
        ground_item::{self as ground, Handle as Ground},
        ground_manager as manager,
        interaction_listeners::{self as listeners, Function, FunctionKind},
        scenery as builder,
    },
};

const TINDERBOX_LAMBDA: &str =
    "content.global.skill.firemaking.FiremakingListener$defineListeners$1";
const BOW_LAMBDA: &str = "content.global.skill.firemaking.FiremakingListener$defineListeners$3";
const FIREMAKING_PULSE: &str = "content.global.skill.firemaking.FireMakingPulse";
const BARB_FIRE_PULSE: &str = "content.global.skill.firemaking.BarbFirePulse";

/// The tinderbox's `onUseWith(IntType.ITEM, ...)` lambda.
pub struct TinderboxOnLogs;
impl crate::world::component_objects::NativeObject for TinderboxOnLogs {}
/// The bows' `onUseWith(IntType.ITEM, ...)` lambda.
pub struct BowOnLogs;
impl crate::world::component_objects::NativeObject for BowOnLogs {}

/// The ash a fire leaves: `getAsh(player, fire, object)`.
pub struct Ash {
    location: Location,
    player: crate::world::player_walk::NodeHandle,
}

/// Firemaking's state in this world.
#[derive(Default)]
pub struct FiremakingWorld {
    /// The running pulses with their player.
    pulses: Vec<(pulses::Handle, usize, Pulse<Ground>)>,
    /// The ashes of the burning fires, by the token their scenery pulse
    /// holds.
    ashes: std::collections::BTreeMap<usize, Ash>,
    next_ash: usize,
}

fn fail(e: impl std::fmt::Debug) -> Error {
    Error::Binding(format!("{e:?}"))
}

/// `firemaking::Host` over the live world, for one player.
struct Fire<'a> {
    live: &'a mut Live,
    p: P,
}
impl Fire<'_> {
    fn location(&mut self) -> Result<Location> {
        self.live
            .inner
            .state
            .node_location(self.p.node())
            .map_err(fail)?
            .ok_or(Error::Null)
    }
    fn item(&self, id: i32, amount: i32) -> crate::inventory::item::Handle {
        Shared::new(Item::new(id, amount, 1000, &self.live.inner.items))
    }
}
impl Host for Fire<'_> {
    type Ground = Ground;
    type Scenery = Scenery;

    fn object_at_player(&mut self) -> Result<bool> {
        let at = self.location()?;
        let walk = &mut self.live.walk;
        let object = walk
            .scene
            .context(&mut walk.maps)
            .object(at, -1, -1)
            .map_err(fail)?;
        Ok(object.is_some())
    }
    fn in_zone(&mut self, name: &str) -> Result<bool> {
        let monitor = self.live.monitor(&self.p.0);
        monitor.is_in_zone(Some(name)).map_err(fail)
    }
    fn message(&mut self, message: &str) -> Result<()> {
        self.live.inner.message(&self.p, message).map_err(fail)
    }
    fn dialogue(&mut self, message: &str) -> Result<()> {
        let node = Node::Player(self.p.0.clone());
        Everyday(Ctx(self.live))
            .dialogue(&node, message)
            .map_err(fail)
    }
    fn in_inventory(&mut self, id: i32) -> Result<bool> {
        let player = &self.live.inner.canonical(&self.p).player;
        Ok(player.inventory().contains(id, 1))
    }
    fn level(&mut self) -> Result<i32> {
        combat_swing::Host::level(self.live, &self.p, firemaking::FIREMAKING).map_err(fail)
    }
    fn attribute_bool(&mut self, key: &str, default: bool) -> Result<bool> {
        let attributes = self.live.inner.canonical(&self.p).player.attributes();
        attributes.boolean(key, default).map_err(|_| Error::Cast)
    }
    fn attribute_int(&mut self, key: &str, default: i32) -> Result<i32> {
        let attributes = self.live.inner.canonical(&self.p).player.attributes();
        attributes.integer(key, default).map_err(|_| Error::Cast)
    }
    fn set_attribute_bool(&mut self, key: &str, value: bool) -> Result<()> {
        let attributes = self.live.inner.canonical(&self.p).player.attributes();
        attributes.set_boolean(key, value);
        Ok(())
    }
    fn set_attribute_int(&mut self, key: &str, value: i32) -> Result<()> {
        let attributes = self.live.inner.canonical(&self.p).player.attributes();
        attributes.set_integer(key, value);
        Ok(())
    }
    fn remove_attribute(&mut self, key: &str) -> Result<()> {
        self.live
            .inner
            .canonical(&self.p)
            .player
            .attributes()
            .remove(key);
        Ok(())
    }
    fn replace_slot(&mut self, node: firemaking::Node, amount: i32) -> Result<()> {
        if amount > 0 {
            return Err(Error::Binding(
                "not ported in the live world: replaceSlot of a stacked log".into(),
            ));
        }
        // `item.amount <= 0`: `cont.replace(null, slot)`.
        let player = Node::Player(self.p.0.clone());
        let replaced = Ctx(self.live).with_container(
            &player,
            crate::world::packet_handlers::Container::Inventory,
            |c, defs, ctx| c.replace(None, node.slot, true, defs, ctx),
        );
        self.live.inner.drain(&self.p);
        replaced.map(|_| ()).map_err(fail)
    }
    fn new_ground(&mut self, node: firemaking::Node) -> Result<Ground> {
        let at = self.location()?;
        let item = self.item(node.id, node.amount);
        let player = self.p.0.clone();
        World::with(self.live, |w| {
            ground::construct(w, &item, Some(at), 200, Some(player))
        })
        .map_err(fail)
    }
    fn create_ground(&mut self, ground: &Ground) -> Result<()> {
        let created = World::with(self.live, |w| {
            let manager = w.ground.manager.clone();
            manager::create(w, &manager, Some(ground.clone()))
        });
        self.live.inner.drain(&self.p);
        created.map(|_| ()).map_err(fail)
    }
    fn produce_ground(&mut self, like: &Ground) -> Result<Ground> {
        // GroundItemManager.create(Item(id, amount), location, owner).
        let (id, amount, at) = {
            let g = like.read();
            let item = g.item.read();
            (item.id(), item.amount(), g.location.clone())
        };
        let item = self.item(id, amount);
        let player = self.p.0.clone();
        let created = World::with(self.live, |w| {
            let manager = w.ground.manager.clone();
            manager::create_at(w, &manager, Some(item), at, Some(player))
        });
        self.live.inner.drain(&self.p);
        created.map_err(fail)
    }
    fn ground_active(&mut self, ground: &Ground) -> Result<bool> {
        World::with(self.live, |w| ground::is_active(w, ground)).map_err(fail)
    }
    fn destroy_ground(&mut self, ground: &Ground) -> Result<()> {
        let destroyed = World::with(self.live, |w| {
            let manager = w.ground.manager.clone();
            manager::destroy(w, &manager, Some(ground.clone()))
        });
        self.live.inner.drain(&self.p);
        destroyed.map(|_| ()).map_err(fail)
    }
    fn add_fire(&mut self, fire_id: i32, life: i32) -> Result<Scenery> {
        // `new Scenery(id, location)`: type 10, rotation 0.
        let at = self.location()?;
        let object = crate::world::scenery_node::construct(
            &mut self.live.walk.scene.definitions,
            fire_id,
            at.clone(),
            10,
            0,
            false,
        )
        .map_err(fail)?;
        let state = &mut self.live.round.firemaking;
        let token = state.next_ash;
        state.next_ash = state.next_ash.wrapping_add(1);
        state.ashes.insert(
            token,
            Ash {
                location: at,
                player: self.p.0.clone(),
            },
        );
        let items = Shared::new(vec![Some(token)].into_boxed_slice());
        let walk = &mut self.live.walk;
        let mut world = walk.scene.context(&mut walk.maps);
        builder::add(&mut world, Some(&object), life, Some(items)).map_err(fail)
    }
    fn move_step(&mut self) -> Result<()> {
        // The movement lock is `walk_path`'s own first check.
        let node = self.p.node();
        let [x, y, z] = self.location()?.coordinates();
        self.live
            .with_walk(|world| {
                for (dx, dy) in [(-1, 0), (0, -1), (1, 0), (0, 1)] {
                    let to = Location::new(x + dx, y + dy, z);
                    let path = world.find_path(node, to, crate::world::movement::DUMB, false)?;
                    if path.successful {
                        return world.walk_path(node, path);
                    }
                }
                Ok(())
            })
            .map_err(fail)
    }
    fn face_fire(&mut self, fire: &Scenery) -> Result<()> {
        // `object.getFaceLocation(player.getLocation())` of a 1x1 object on
        // the player's own tile is that tile.
        let at = fire.read().location.clone();
        let from = self.location()?;
        let face = crate::world::legacy_interaction::node_face_location(&at, 1, &from);
        self.live
            .inner
            .canonical(&self.p)
            .player
            .masks()
            .face_location(Some(face))
            .map(|_| ())
            .map_err(fail)
    }
    fn add_experience(&mut self, xp: f64) -> Result<()> {
        let added = combat_pulse::Host::add_experience(
            self.live,
            &self.p,
            firemaking::FIREMAKING,
            xp,
            false,
        );
        self.live.inner.drain(&self.p);
        added.map_err(fail)
    }
    fn world_ticks(&mut self) -> Result<i32> {
        Ok(self.live.inner.ticks)
    }
    fn dispatch_lit_fire(&mut self, _: i32) -> Result<()> {
        self.live
            .inner
            .not_ported
            .skip("LitFireEvent dispatch (entity event hooks not ported)");
        Ok(())
    }
    fn animate(&mut self, id: i32) -> Result<()> {
        self.live
            .entity_visualize(&self.p, Some(Animation::new(id)), None)
            .map_err(fail)
    }
    fn reset_animation(&mut self) -> Result<()> {
        let animation = Animation::with(-1, 0, Some(Priority::High));
        self.live
            .entity_visualize(&self.p, Some(animation), None)
            .map_err(fail)
    }
    fn random(&mut self, max: i32) -> Result<i32> {
        // RandomFunction.random(maxValue): 0 without a draw below 1.
        if max <= 0 {
            return Ok(0);
        }
        self.live
            .inner
            .generators
            .shared_next_int(max)
            .ok_or(Error::Null)
    }
}

impl Live {
    /// FiremakingListener's inventory registrations (the first and the
    /// third of `defineListeners`).
    pub fn define_firemaking_listener(&mut self) -> std::result::Result<(), String> {
        let registry = self.item_actions.listeners.clone();
        let item = listeners::Kind::Item.ordinal();
        registry
            .add_use_arrays(
                item,
                &[firemaking::TINDERBOX],
                &firemaking::LOGS,
                Function::new(FunctionKind::UseWith, TINDERBOX_LAMBDA, TinderboxOnLogs),
            )
            .map_err(|e| format!("{}: {e:?}", firemaking::LISTENER_CLASS))?;
        registry
            .add_use_arrays(
                item,
                &firemaking::BOWS,
                &firemaking::LOGS,
                Function::new(FunctionKind::UseWith, BOW_LAMBDA, BowOnLogs),
            )
            .map_err(|e| format!("{}: {e:?}", firemaking::LISTENER_CLASS))?;
        self.inner.not_ported.skip(
            "FiremakingListener's ground item registrations (ground item listeners not routed)",
        );
        Ok(())
    }

    /// The pulse's `node`: the log as the inventory holds it.
    fn firemaking_node(&mut self, p: &P, log: &Node) -> Bound<firemaking::Node> {
        let Node::Item(log) = log else {
            return Err(bind("firemaking with a node that is not an item"));
        };
        let inventory = self.inner.canonical(p).player.inventory();
        let slot = inventory
            .items()
            .read()
            .iter()
            .position(|held| held.as_ref().is_some_and(|held| held.same(log)))
            .map_or(-1, |slot| slot as i32);
        let item = log.read();
        Ok(firemaking::Node {
            id: item.id(),
            amount: item.amount(),
            slot,
        })
    }

    /// The two lambdas: `submitIndividualPulse(player, FireMakingPulse(player,
    /// with.asItem(), null))` or `BarbFirePulse(player, with.asItem(), null,
    /// used.asItem())`.
    pub fn firemaking_use(
        &mut self,
        player: &Node,
        used: Option<&Node>,
        log: &Node,
    ) -> Bound<bool> {
        let Node::Player(player) = player else {
            return Err(bind("firemaking by a node that is not a player"));
        };
        let p = P(player.clone());
        let node = self.firemaking_node(&p, log)?;
        let bow = match used {
            Some(Node::Item(bow)) => Some(bow.read().id()),
            Some(_) => return Err(bind("a bow that is not an item")),
            None => None,
        };
        let mut host = Fire {
            live: self,
            p: p.clone(),
        };
        let pulse = match bow {
            Some(bow) => Pulse::bow(&mut host, node, None, Some(bow)),
            None => Pulse::tinderbox(&mut host, node, None),
        }
        .map_err(bind)?;
        self.run_firemaking_pulse(&p, pulse)?;
        Ok(true)
    }

    /// `player.pulseManager.run(pulse)`: the constructor leaves the pulse
    /// stopped; `start()` runs it when the requirements hold.
    fn run_firemaking_pulse(&mut self, p: &P, mut pulse: Pulse<Ground>) -> Bound<()> {
        self.round
            .firemaking
            .pulses
            .retain(|(handle, ..)| handle.read().running);
        let class = match pulse.kind {
            Kind::Tinderbox => FIREMAKING_PULSE,
            Kind::Bow { .. } => BARB_FIRE_PULSE,
        };
        // `Pulse(1, player, node)`: the node is an Item, which is never
        // deactivated, and the player leaving stops the pulse.
        let handle = Shared::new(pulses::Pulse::new(
            1,
            Some(Shared::new(Vec::new())),
            pulses::Class::Other,
            class.into(),
        ));
        let mut started = Ok(());
        let submitted = self
            .run_content_pulse(p, handle.clone(), |live| {
                let mut host = Fire { live, p: p.clone() };
                started = pulse.start(&mut host);
                started.is_ok() && pulse.running
            })
            .map_err(bind)?;
        if let Err(error) = started {
            pulses::stop_base(&handle);
            return Err(bind(error));
        }
        if submitted {
            self.walk.stopping_pulses.push(handle.clone());
            self.round.firemaking.pulses.push((handle, p.node(), pulse));
        }
        Ok(())
    }

    /// A firemaking pulse by identity: `Some` with whether the runner drops
    /// it.
    pub fn firemaking_pulse_body(&mut self, pulse: &pulses::Handle) -> Option<Bound<bool>> {
        let pulses = &mut self.round.firemaking.pulses;
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
        let mut host = Fire { live: self, p };
        match task.pulse(&mut host) {
            Ok(false) => {
                self.round.firemaking.pulses.push((handle, owner, task));
                Some(Ok(false))
            }
            finished => {
                // `stop()`: the animation reset, the base flag, the message.
                let stopped = task.stop(&mut host);
                pulses::stop_base(&handle);
                Some(finished.and_then(|_| stopped.map(|()| true)).map_err(bind))
            }
        }
    }

    /// A firemaking pulse a pulse manager stopped: its `stop()`.
    pub fn firemaking_pulse_stopped(&mut self, pulse: &pulses::Handle) -> Option<Bound<()>> {
        let pulses = &mut self.round.firemaking.pulses;
        let at = pulses.iter().position(|(h, ..)| h.same(pulse))?;
        let (_, owner, mut task) = pulses.remove(at);
        if !self.inner.players.contains_key(&owner) {
            return Some(Ok(()));
        }
        let p = match self.inner.state.retain_node(owner) {
            Ok(h) => P(h),
            Err(e) => return Some(Err(bind(e))),
        };
        let mut host = Fire { live: self, p };
        Some(task.stop(&mut host).map_err(bind))
    }

    /// The ashes of the fires whose scenery pulse ran: `GroundItemManager
    /// .create(ash)` of `getAsh`'s ground item, whose decay time
    /// (`life + 200` from the lighting) leaves it 200 ticks.
    pub fn firemaking_tick(&mut self) -> std::result::Result<(), String> {
        let due = std::mem::take(&mut self.walk.maps.ash_due);
        for token in due {
            let Some(ash) = self.round.firemaking.ashes.remove(&token) else {
                continue;
            };
            let item = Shared::new(Item::new(firemaking::ASHES, 1, 1000, &self.inner.items));
            World::with(self, |w| {
                let manager = w.ground.manager.clone();
                manager::create_at(
                    w,
                    &manager,
                    Some(item),
                    Some(ash.location),
                    Some(ash.player),
                )
            })
            .map_err(|e| format!("the ashes of a fire: {e:?}"))?;
        }
        Ok(())
    }
}
