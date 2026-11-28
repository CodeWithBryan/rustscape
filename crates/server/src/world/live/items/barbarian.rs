//! Barbarian fishing in the live world (`world::gathering::barbarian`,
//! ported without a JVM corpus): SpotManager's five `BarbFishingSpot` NPCs
//! (2722) with their tick, BarbFishInteractionListeners (the bed's "search"
//! and the spots' "use-rod"), BarbFishingPulse, KnifeWithFish and
//! FishCuttingPulse.
//!
//! - `SpotManager.startup()` runs at boot when the class is in the scan
//!   (`install_content_use`): each spot is `new NPC(2722)` given its
//!   location, then `init()` (super::super::runtime_npcs). Its place among
//!   the other start-up listeners (most of which are not ported) is not
//!   reproduced, so the generators' state at its draws is this world's.
//! - `SpotManager.tick()` is a content tick listener (super::super::content).
//! - `BarbFishingSpot.handleTickActions()` replaces `NPC.handleTickActions`:
//!   `Live::npc_tick_of` runs it with the whole world right before the NPC's
//!   tick kernel, whose `behavior.tick` query answers `false` for a spot so
//!   the kernel's own tick actions do nothing.
//! - The player bodies answer through [`Gather`](super::woodcutting) with no
//!   target node; the familiar's boost is 0 by name (familiars not ported).
//!   Nothing in this world sets the barbarian training attribute the bed
//!   asks for (Otto's dialogue is not ported).
use super::{
    super::host::Live,
    Node, P,
    woodcutting::{Bound, PLAYER, Target, bind},
};
use crate::{
    definitions::item::Shared,
    pulses,
    startup::generators::WorldGenerators,
    world::{
        doors::Location,
        gathering::barbarian::{self, FishingPulse, Manager, SpotError, Spots},
        interaction_listeners::{self as listeners, Function, FunctionKind},
        npc,
    },
};
use rs09_game_core::direction::Direction;

/// BarbFishInteractionListeners' lambdas, in source order.
const BED_LAMBDA: &str = "content.global.skill.gather.fishing.barbfishing.BarbFishInteractionListeners$defineListeners$1";
const ROD_LAMBDA: &str = "content.global.skill.gather.fishing.barbfishing.BarbFishInteractionListeners$defineListeners$2";
/// KnifeWithFish's lambda.
const KNIFE_LAMBDA: &str =
    "content.global.skill.gather.fishing.barbfishing.KnifeWithFish$defineListeners$1";
const FISHING_PULSE: &str = "content.global.skill.gather.fishing.barbfishing.BarbFishingPulse";
const CUTTING_PULSE: &str = "content.global.skill.gather.fishing.barbfishing.FishCuttingPulse";

/// The bed's `on(..., "search")` lambda.
pub struct SearchBed;
impl crate::world::component_objects::NativeObject for SearchBed {}
/// The spots' `on(..., "use-rod")` lambda.
pub struct UseRod;
impl crate::world::component_objects::NativeObject for UseRod {}
/// KnifeWithFish's `onUseWith` lambda.
pub struct KnifeWithFish;
impl crate::world::component_objects::NativeObject for KnifeWithFish {}

/// Barbarian fishing's state in this world.
#[derive(Default)]
pub struct BarbarianWorld {
    /// `SpotManager` and `usedLocations`.
    pub manager: Manager,
    /// The NPC of each of the manager's spots, in the same order.
    pub spots: Vec<usize>,
    /// SpotManager is a tick listener of this world.
    pub ticking: bool,
    /// The running BarbFishingPulses: the pulse object, the player's node
    /// and the pulse's state.
    pub fishing: Vec<(pulses::Handle, usize, FishingPulse)>,
    /// The running FishCuttingPulses: the pulse object, the player's node
    /// and the fish.
    pub cutting: Vec<(pulses::Handle, usize, i32)>,
}

struct Draws<'a>(&'a mut WorldGenerators);
impl Spots for Draws<'_> {
    type Error = String;
    fn random_range(&mut self, a: i32, b: i32) -> Result<i32, String> {
        Ok(crate::world::drop_tables::random::random_range(
            self.0, a, b,
        ))
    }
    fn kotlin_next_int(&mut self, until: i32) -> Result<i32, String> {
        self.0
            .thread_local_next_int(until)
            .ok_or_else(|| "Random.nextInt of a bound below 1".to_owned())
    }
}

fn spot_error(e: SpotError<String>) -> String {
    match e {
        SpotError::NoSuchElement => {
            "SpotManager: NoSuchElementException (no free spot location)".into()
        }
        SpotError::Host(e) => format!("SpotManager: {e}"),
    }
}

impl Live {
    /// `BarbFishInteractionListeners.defineListeners`.
    pub fn define_barbarian_listeners(&mut self) -> Result<(), String> {
        let casing = super::super::global::RootCasing;
        let registry = self.item_actions.listeners.clone();
        registry
            .add(
                &casing,
                barbarian::BED,
                listeners::Kind::Scenery.ordinal(),
                &["search".encode_utf16().collect()],
                Function::new(FunctionKind::On, BED_LAMBDA, SearchBed),
            )
            .map_err(|e| format!("{}: {e:?}", barbarian::LISTENERS_CLASS))?;
        registry
            .add(
                &casing,
                barbarian::SPOT,
                listeners::Kind::Npc.ordinal(),
                &["use-rod".encode_utf16().collect()],
                Function::new(FunctionKind::On, ROD_LAMBDA, UseRod),
            )
            .map_err(|e| format!("{}: {e:?}", barbarian::LISTENERS_CLASS))
    }

    /// `KnifeWithFish.defineListeners`.
    pub fn define_knife_with_fish(&mut self) -> Result<(), String> {
        self.item_actions
            .listeners
            .add_use_arrays(
                listeners::Kind::Item.ordinal(),
                &[barbarian::KNIFE],
                &barbarian::LEAPING_FISH,
                Function::new(FunctionKind::UseWith, KNIFE_LAMBDA, KnifeWithFish),
            )
            .map_err(|e| format!("{}: {e:?}", barbarian::KNIFE_CLASS))
    }

    /// `SpotManager.startup()`, and the manager as a tick listener.
    pub fn barbarian_startup(&mut self) -> Result<(), String> {
        for _ in 0..barbarian::SPOTS {
            let spot = self
                .round
                .barbarian
                .manager
                .add_spot(&mut Draws(&mut self.inner.generators))
                .map_err(spot_error)?;
            // BarbFishingSpot(loc, ttl) : NPC(2722), `location = loc`, then
            // `init()`.
            let npc = self
                .npc_new(barbarian::SPOT, None, Some(Direction::South))
                .map_err(|e| format!("BarbFishingSpot: {e:?}"))?;
            let at = spot.loc.map(|[x, y, z]| Location::new(x, y, z));
            self.npc_set_location_field(npc.node, at);
            self.npc_init_base(npc.node)
                .map_err(|e| format!("BarbFishingSpot.init: {e:?}"))?;
            self.round.barbarian.spots.push(npc.node);
            self.walk.barbarian_spots.push(npc.node);
        }
        self.round.barbarian.ticking = true;
        Ok(())
    }

    /// `SpotManager.tick()`.
    pub fn barbarian_tick(&mut self) {
        if self.round.barbarian.ticking {
            self.round.barbarian.manager.tick();
        }
    }

    /// `BarbFishingSpot.handleTickActions()`.
    pub(in super::super) fn barbarian_spot_tick(&mut self, npc: &P) -> npc::Result<()> {
        let node = npc.node();
        let fail = |e: String| npc::Error::Binding(e);
        let spots = &self.round.barbarian.spots;
        let Some(index) = spots.iter().position(|n| *n == node) else {
            return Ok(());
        };
        let location = self
            .inner
            .state
            .node_location(node)
            .map_err(|e| fail(format!("BarbFishingSpot: {e:?}")))?
            .map(|l| l.coordinates());
        let teleport = self
            .round
            .barbarian
            .manager
            .handle_tick_actions(&mut Draws(&mut self.inner.generators), index, location)
            .map_err(|e| fail(spot_error(e)))?;
        if let Some(to) = teleport {
            self.inner
                .state
                .actor(node)
                .map_err(|e| fail(format!("BarbFishingSpot: {e:?}")))?
                .queue
                .state
                .write()
                .entity
                .teleport_location = to.map(|[x, y, z]| Location::new(x, y, z));
        }
        Ok(())
    }

    /// The bed's "search" lambda.
    pub fn barbarian_search_bed(&mut self, player: &Node, bed: &Node) -> Bound<bool> {
        let (Node::Player(player), Node::Scenery(bed)) = (player, bed) else {
            return Err(bind(
                "the barbarian bed's search of a node that is not scenery",
            ));
        };
        self.gather(&P(player.clone()), bed, "barbarian fishing", |host| {
            barbarian::search_bed(host, PLAYER)
        })
    }

    /// The spots' "use-rod" lambda.
    pub fn barbarian_use_rod(&mut self, player: &Node, spot: &Node) -> Bound<bool> {
        let (Node::Player(player), Node::Npc(spot)) = (player, spot) else {
            return Err(bind("use-rod of a node that is not an NPC"));
        };
        let target = Target::Npc(P(spot.clone()));
        self.gather_at(&P(player.clone()), target, "barbarian fishing", |host| {
            barbarian::use_rod(host, PLAYER)
        })
    }

    /// KnifeWithFish's lambda: `fish` is the item the knife was used with.
    pub fn barbarian_knife_with_fish(&mut self, player: &Node, fish: i32) -> Bound<bool> {
        let Node::Player(player) = player else {
            return Err(bind(
                "a knife used with a fish by a node that is not a player",
            ));
        };
        self.gather_at(
            &P(player.clone()),
            Target::None,
            "barbarian fishing",
            |host| barbarian::knife_with_fish(host, PLAYER, fish),
        )
    }

    /// `player.pulseManager.run(BarbFishingPulse(player))`: the constructor
    /// leaves the pulse stopped; `start()` runs it when the requirements
    /// hold.
    pub(super) fn run_barbarian_pulse(&mut self, p: &P) -> Bound<()> {
        self.round
            .barbarian
            .fishing
            .retain(|(pulse, ..)| pulse.read().running);
        let mut pulse = FishingPulse::new(PLAYER);
        // `Pulse(1, player, NPC(2722))`: the node is a new NPC that is never
        // deactivated, and the player leaving stops the pulse.
        let handle = Shared::new(pulses::Pulse::new(
            pulse.delay,
            Some(Shared::new(Vec::new())),
            pulses::Class::Other,
            FISHING_PULSE.into(),
        ));
        let mut started = Ok(());
        let submitted = self
            .run_content_pulse(p, handle.clone(), |live| {
                started = live.gather_at(p, Target::None, "barbarian fishing", |host| {
                    pulse.start(host)
                });
                started.is_ok() && pulse.running
            })
            .map_err(bind)?;
        if let Err(error) = started {
            pulses::stop_base(&handle);
            return Err(error);
        }
        if submitted {
            self.walk.stopping_pulses.push(handle.clone());
            self.round.barbarian.fishing.push((handle, p.node(), pulse));
        }
        Ok(())
    }

    /// A BarbFishingPulse by identity: `Some` with whether the runner drops
    /// it.
    pub fn barbarian_pulse_body(&mut self, pulse: &pulses::Handle) -> Option<Bound<bool>> {
        let pulses = &mut self.round.barbarian.fishing;
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
        let done = self.gather_at(&p, Target::None, "barbarian fishing", |host| {
            task.pulse(host)
        });
        // `setDelay(5)`.
        handle.write().delay = task.delay;
        match done {
            Ok(false) => {
                self.round.barbarian.fishing.push((handle, owner, task));
                Some(Ok(false))
            }
            finished => {
                // `stop()`: the animation reset, then the base flag.
                let stopped = self.gather_at(&p, Target::None, "barbarian fishing", |host| {
                    task.stop(host)
                });
                pulses::stop_base(&handle);
                Some(finished.and_then(|_| stopped.map(|()| true)))
            }
        }
    }

    /// A BarbFishingPulse a pulse manager stopped: its `stop()`.
    pub fn barbarian_pulse_stopped(&mut self, pulse: &pulses::Handle) -> Option<Bound<()>> {
        let pulses = &mut self.round.barbarian.fishing;
        let at = pulses.iter().position(|(h, ..)| h.same(pulse))?;
        let (_, owner, mut task) = pulses.remove(at);
        if !self.inner.players.contains_key(&owner) {
            return Some(Ok(()));
        }
        let p = match self.inner.state.retain_node(owner) {
            Ok(h) => P(h),
            Err(e) => return Some(Err(bind(e))),
        };
        Some(
            self.gather_at(&p, Target::None, "barbarian fishing", |host| {
                task.stop(host)
            }),
        )
    }

    /// `player.pulseManager.run(FishCuttingPulse(player, fish))`.
    pub(super) fn run_fish_cutting_pulse(&mut self, p: &P, fish: i32) -> Result<(), String> {
        // The pulses a manager stopped never reach their body.
        self.round
            .barbarian
            .cutting
            .retain(|(pulse, ..)| pulse.read().running);
        let handle = Shared::new(pulses::Pulse::new(
            0,
            None,
            pulses::Class::Other,
            CUTTING_PULSE.into(),
        ));
        if self.run_standard_pulse(p, handle.clone())? {
            self.round.barbarian.cutting.push((handle, p.node(), fish));
        }
        Ok(())
    }

    /// A FishCuttingPulse by identity: `Some` with whether it finished.
    pub fn fish_cutting_pulse_body(&mut self, pulse: &pulses::Handle) -> Option<Bound<bool>> {
        let pulses = &mut self.round.barbarian.cutting;
        let at = pulses.iter().position(|(h, ..)| h.same(pulse))?;
        let (handle, owner, fish) = pulses.remove(at);
        if !self.inner.players.contains_key(&owner) {
            // The player left: the pulse ends.
            return Some(Ok(true));
        }
        let p = match self.inner.state.retain_node(owner) {
            Ok(h) => P(h),
            Err(e) => return Some(Err(bind(e))),
        };
        let done = self.gather_at(&p, Target::None, "barbarian fishing", |host| {
            barbarian::fish_cutting_pulse(host, PLAYER, fish)
        });
        if matches!(done, Ok(false)) {
            self.round.barbarian.cutting.push((handle, owner, fish));
        }
        Some(done)
    }
}
