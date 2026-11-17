//! The NPC as a live actor: aggression, the behaviour hooks, and the members
//! of `NPC`, `Entity` and `Player` that sit on the combat path.
//!
//! Ports the pinned `AggressiveHandler.java` and `AggressiveBehavior.java`
//! (`aggression`), `NPCBehavior.kt` (the [`Behaviour`] trait and its
//! [`Registry`]), the `NPC.java` members `tick` leaves to its callers
//! (`lifecycle`: invisibility, region inactivity, the default aggressive
//! behaviour, boss data) and the combat members of `NPC`, `Entity` and
//! `Player` (`combat`: attackability, continueAttack, checkImpact, onImpact
//! and its retaliation, sendImpact, inCombat, the NPC's swing handler).
//! `tick` joins the existing `world::npc` tick kernel to these pieces.
//!
//! This is a library. State the original keeps on the NPC lives in [`Ai`],
//! which the host hands out through a plain accessor; no borrow of it is held
//! across a service call. Everything the original reaches outside these
//! members is a required [`Host`] service called in the original order. None
//! has a default. See docs/npc-behaviour.md.
pub mod aggression;
pub mod combat;
pub mod lifecycle;
pub mod tick;

use crate::world::combat_swing::{self as cs, BattleState, CombatStyle};
pub use crate::world::combat_swing::{Error, Kind, Result};
use crate::world::doors::Location;
use crate::world::drop_tables::RolledItem;
use crate::world::npc;
use std::collections::HashMap;
use std::sync::Arc;

/// `Skills.SLAYER`.
pub const SLAYER: i32 = 18;

/// `Rights`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rights {
    RegularPlayer,
    PlayerModerator,
    Administrator,
}

/// `HitsplatType` as `Player.onImpact` passes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hitsplat {
    Normal,
}

/// The pathfinder a behaviour may substitute (`getPathfinderOverride`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Finder {
    Smart,
    Dumb,
}

/// The clip mask supplier a behaviour names (`getClippingSupplier`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClipSupplier {
    /// `NPCBehavior.StandardClipMaskSupplier`: `RegionManager.getClippingFlag`.
    Standard,
    /// Any other supplier content defines; the host owns its meaning.
    Other(u32),
}

impl ClipSupplier {
    /// `getClippingFlag(z, x, y)` of the supplier.
    pub fn clipping_flag<H: Host>(self, h: &mut H, z: i32, x: i32, y: i32) -> Result<i32> {
        match self {
            Self::Standard => h.clipping_flag(z, x, y),
            Self::Other(id) => h.other_clipping_flag(id, z, x, y),
        }
    }
}

/// The slayer `Tasks` constant `configure` assigned (`Tasks.forId(id)`);
/// only `levelReq` is read here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SlayerTask {
    pub level_req: i32,
}

/// `NPCBehavior`: every hook with the original default. Content overrides a
/// method; a behaviour that overrides nothing behaves like the original's
/// shared default instance.
#[allow(unused_variables)]
pub trait Behaviour<H: Host>: Send + Sync {
    /// `tick(self)`: false skips the rest of `handleTickActions`.
    fn tick(&self, h: &mut H, npc: &H::Entity) -> Result<bool> {
        Ok(true)
    }
    fn before_damage_received(
        &self,
        h: &mut H,
        npc: &H::Entity,
        attacker: &H::Entity,
        state: &mut BattleState<H>,
    ) -> Result<()> {
        Ok(())
    }
    fn after_damage_received(
        &self,
        h: &mut H,
        npc: &H::Entity,
        attacker: &H::Entity,
        state: &mut BattleState<H>,
    ) -> Result<()> {
        Ok(())
    }
    fn before_attack_finalized(
        &self,
        h: &mut H,
        npc: &H::Entity,
        victim: &H::Entity,
        state: &mut BattleState<H>,
    ) -> Result<()> {
        Ok(())
    }
    fn on_removal(&self, h: &mut H, npc: &H::Entity) -> Result<()> {
        Ok(())
    }
    fn on_creation(&self, h: &mut H, npc: &H::Entity) -> Result<()> {
        Ok(())
    }
    fn on_respawn(&self, h: &mut H, npc: &H::Entity) -> Result<()> {
        Ok(())
    }
    fn on_death_started(&self, h: &mut H, npc: &H::Entity, killer: &H::Entity) -> Result<()> {
        Ok(())
    }
    fn on_death_finished(&self, h: &mut H, npc: &H::Entity, killer: &H::Entity) -> Result<()> {
        Ok(())
    }
    fn on_drop_table_rolled(
        &self,
        h: &mut H,
        npc: &H::Entity,
        killer: &H::Entity,
        drops: &mut Vec<RolledItem>,
    ) -> Result<()> {
        Ok(())
    }
    /// `!(attacker is Player && !hasAction("attack") && !hasAction("smash-ice"))`
    /// on the NPC's definition. Kotlin refuses a null style before any hook
    /// body runs, so callers pass a style or fail with `Error::Null`.
    fn can_be_attacked_by(
        &self,
        h: &mut H,
        npc: &H::Entity,
        attacker: &H::Entity,
        style: CombatStyle,
        message: bool,
    ) -> Result<bool> {
        Ok(!(h.kind(attacker).is_player()
            && !h.has_action(npc, "attack")?
            && !h.has_action(npc, "smash-ice")?))
    }
    fn should_ignore_multi_restrictions(
        &self,
        h: &mut H,
        npc: &H::Entity,
        victim: &H::Entity,
    ) -> Result<bool> {
        Ok(false)
    }
    fn swing_handler_override(
        &self,
        h: &mut H,
        npc: &H::Entity,
        original: H::Handler,
    ) -> Result<H::Handler> {
        Ok(original)
    }
    fn pathfinder_override(&self, h: &mut H, npc: &H::Entity) -> Result<Option<Finder>> {
        Ok(None)
    }
    fn clipping_supplier(&self, h: &mut H, npc: &H::Entity) -> Result<Option<ClipSupplier>> {
        Ok(Some(ClipSupplier::Standard))
    }
    fn xp_multiplier(&self, h: &mut H, npc: &H::Entity, attacker: &H::Entity) -> Result<f64> {
        Ok(1.0)
    }
}

/// The shared default `NPCBehavior()` instance.
pub struct DefaultBehaviour;

impl<H: Host> Behaviour<H> for DefaultBehaviour {}

/// `NPCBehavior`'s companion: the id map and the default instance. Insertion
/// order is never observed: lookups and overwrites only.
pub struct Registry<H: Host> {
    ids: HashMap<i32, Arc<dyn Behaviour<H>>>,
    default: Arc<dyn Behaviour<H>>,
}

impl<H: Host> Default for Registry<H> {
    fn default() -> Self {
        Self {
            ids: HashMap::new(),
            default: Arc::new(DefaultBehaviour),
        }
    }
}

impl<H: Host> Registry<H> {
    /// `forId(id)`: the registered behaviour or the shared default.
    pub fn for_id(&self, id: i32) -> Arc<dyn Behaviour<H>> {
        self.ids
            .get(&id)
            .cloned()
            .unwrap_or_else(|| self.default.clone())
    }
    /// `register(ids, behavior)`: a later registration of an id replaces it.
    pub fn register(&mut self, ids: &[i32], behaviour: Arc<dyn Behaviour<H>>) {
        for id in ids {
            self.ids.insert(*id, behaviour.clone());
        }
    }
    /// The shared default instance (`NPCBehavior.defaultBehavior`).
    pub fn default_behaviour(&self) -> Arc<dyn Behaviour<H>> {
        self.default.clone()
    }
}

/// The overridable members of an NPC's runtime class (`NPC` and the content
/// subclasses of `AbstractNPC`), with `NPC.java`'s and `Entity.java`'s
/// bodies as the defaults.
#[allow(unused_variables)]
pub trait NpcClass<H: Host>: Send + Sync {
    /// `canAttack(victim)`.
    fn can_attack(&self, h: &mut H, npc: &H::Entity, victim: &H::Entity) -> Result<bool> {
        Ok(true)
    }
    /// `canStartCombat(victim)`.
    fn can_start_combat(&self, h: &mut H, npc: &H::Entity, victim: &H::Entity) -> Result<bool> {
        Ok(true)
    }
    /// `isIgnoreAttackRestrictions(victim)`.
    fn is_ignore_attack_restrictions(
        &self,
        h: &mut H,
        npc: &H::Entity,
        victim: &H::Entity,
    ) -> Result<bool> {
        Ok(false)
    }
    /// `Entity.canSelectTarget(target)`.
    fn can_select_target(&self, h: &mut H, npc: &H::Entity, target: &H::Entity) -> Result<bool> {
        Ok(true)
    }
    /// The protected `onRespawn()`.
    fn on_respawn(&self, h: &mut H, npc: &H::Entity) -> Result<()> {
        Ok(())
    }
    /// `isAttackable(entity, style, message)`; `NPC.isAttackable` by default.
    fn is_attackable(
        &self,
        h: &mut H,
        npc: &H::Entity,
        attacker: &H::Entity,
        style: Option<CombatStyle>,
        message: bool,
    ) -> Result<bool> {
        combat::npc_is_attackable(h, npc, attacker, style, message)
    }
}

/// The plain `NPC` class.
pub struct PlainNpc;

impl<H: Host> NpcClass<H> for PlainNpc {}

/// What the original keeps on one NPC beyond `world::npc::Fields`.
pub struct Ai<H: Host> {
    /// `aggressiveHandler`.
    pub handler: Option<aggression::Handler<H>>,
    /// `behavior`.
    pub behaviour: Arc<dyn Behaviour<H>>,
    /// The NPC's runtime class.
    pub class: Arc<dyn NpcClass<H>>,
    /// `task`.
    pub task: Option<SlayerTask>,
    /// `Entity.invisible`.
    pub invisible: bool,
}

impl<H: Host> Ai<H> {
    pub fn new(behaviour: Arc<dyn Behaviour<H>>, class: Arc<dyn NpcClass<H>>) -> Self {
        Self {
            handler: None,
            behaviour,
            class,
            task: None,
            invisible: false,
        }
    }
}

/// Everything the ported members reach outside themselves. The first group
/// are plain accessors to state and class identity: they are not calls in the
/// original and are never recorded. Each fallible method after them is one
/// original call; a failure leaves earlier effects in place.
pub trait Host: cs::Types + Sized {
    /// One `CombatSwingHandler` instance.
    type Handler: Clone;
    /// One `Region` object.
    type Region;

    /// The entity's class.
    fn kind(&self, entity: &Self::Entity) -> Kind;
    /// Reference identity.
    fn same(&self, a: &Self::Entity, b: &Self::Entity) -> bool;
    /// The NPC's [`Ai`]; `None` for an entity that is not an NPC.
    fn ai(&mut self, npc: &Self::Entity) -> Option<&mut Ai<Self>>;
    /// The NPC's `world::npc` state (fields `tick` reads and writes).
    fn brain(&self, npc: &Self::Entity) -> Option<npc::State>;
    /// The entity's `location` field.
    fn location(&self, entity: &Self::Entity) -> Option<Location>;
    /// Assigns the NPC's `location` field.
    fn set_location(&mut self, npc: &Self::Entity, location: Option<Location>);
    /// `CombatStyle.getSwingHandler()`.
    fn style_handler(&self, style: CombatStyle) -> Self::Handler;
    /// `region instanceof DynamicRegion`.
    fn region_dynamic(&self, region: &Self::Region) -> bool;
    /// `ServerConstants.MAX_PLAYERS`.
    fn max_players(&self) -> i32;

    /// `GameWorld.getTicks()`.
    fn ticks(&mut self) -> Result<i32>;
    /// `System.currentTimeMillis()`.
    fn current_time_millis(&mut self) -> Result<i64>;
    /// `RandomFunction.RANDOM.nextInt(bound)`.
    fn next_int(&mut self, bound: i32) -> Result<i32>;
    /// `Math.random()`.
    fn math_random(&mut self) -> Result<f64>;
    /// `Node.isActive()`.
    fn is_active(&mut self, entity: &Self::Entity) -> Result<bool>;
    /// `DeathTask.isDead(entity)`.
    fn is_dead(&mut self, entity: &Self::Entity) -> Result<bool>;
    /// `getLocks().isInteractionLocked()`.
    fn interaction_locked(&mut self, entity: &Self::Entity) -> Result<bool>;
    /// `getAttribute(key, default)` of a boolean attribute.
    fn attribute_bool(&mut self, entity: &Self::Entity, key: &str, default: bool) -> Result<bool>;
    /// `getAttribute(key, default)` of a long attribute.
    fn attribute_long(&mut self, entity: &Self::Entity, key: &str, default: i64) -> Result<i64>;
    /// `getAttribute(key)` of an entity attribute.
    fn attribute_entity(
        &mut self,
        entity: &Self::Entity,
        key: &str,
    ) -> Result<Option<Self::Entity>>;
    /// `setAttribute(key, value)` with an entity value.
    fn set_attribute_entity(
        &mut self,
        entity: &Self::Entity,
        key: &str,
        value: &Self::Entity,
    ) -> Result<()>;
    fn remove_attribute(&mut self, entity: &Self::Entity, key: &str) -> Result<()>;
    /// `definition.getConfiguration(key, default)` of a boolean value.
    fn config_bool(&mut self, npc: &Self::Entity, key: &str, default: bool) -> Result<bool>;
    /// `definition.getConfiguration(key, default)` of an integer value.
    fn config_int(&mut self, npc: &Self::Entity, key: &str, default: i32) -> Result<i32>;
    /// `definition.getCombatLevel()`.
    fn definition_combat_level(&mut self, npc: &Self::Entity) -> Result<i32>;
    /// `definition.hasAction(action)`.
    fn has_action(&mut self, npc: &Self::Entity, action: &str) -> Result<bool>;
    /// `npc.getShownNPC(player)`.
    fn shown_npc(&mut self, npc: &Self::Entity, player: &Self::Entity) -> Result<Self::Entity>;

    /// `combatPulse.isAttacking()`.
    fn is_attacking(&mut self, entity: &Self::Entity) -> Result<bool>;
    /// `combatPulse.getVictim()`.
    fn combat_victim(&mut self, entity: &Self::Entity) -> Result<Option<Self::Entity>>;
    /// `combatPulse.setVictim(victim)`.
    fn combat_set_victim(&mut self, entity: &Self::Entity, victim: &Self::Entity) -> Result<()>;
    /// `combatPulse.attack(victim)`.
    fn combat_attack(&mut self, entity: &Self::Entity, victim: Option<&Self::Entity>)
    -> Result<()>;
    /// `combatPulse.stop()`.
    fn combat_stop(&mut self, entity: &Self::Entity) -> Result<()>;
    /// `combatPulse.getNextAttack()`.
    fn next_attack(&mut self, entity: &Self::Entity) -> Result<i32>;
    /// `combatPulse.getStyle()`.
    fn combat_style(&mut self, entity: &Self::Entity) -> Result<Option<CombatStyle>>;
    fn set_last_received_attack(&mut self, entity: &Self::Entity, tick: i32) -> Result<()>;
    fn last_sent_attack(&mut self, entity: &Self::Entity) -> Result<i32>;
    fn set_last_sent_attack(&mut self, entity: &Self::Entity, tick: i32) -> Result<()>;
    /// `Entity.face(target)`.
    fn face(&mut self, entity: &Self::Entity, target: Option<&Self::Entity>) -> Result<()>;
    /// `properties.isMultiZone()`.
    fn is_multi_zone(&mut self, entity: &Self::Entity) -> Result<bool>;
    /// `properties.getCurrentCombatLevel()`.
    fn current_combat_level(&mut self, entity: &Self::Entity) -> Result<i32>;
    /// `properties.isRetaliating()`.
    fn is_retaliating(&mut self, entity: &Self::Entity) -> Result<bool>;
    /// `properties.setNPCWalkable(value)`.
    fn set_npc_walkable(&mut self, npc: &Self::Entity, value: bool) -> Result<()>;
    /// `properties.setCombatTimeOut(value)`.
    fn set_combat_time_out(&mut self, npc: &Self::Entity, value: i32) -> Result<()>;

    /// `RegionManager.getLocalPlayers(location, radius)`.
    fn local_players(&mut self, location: Location, radius: i32) -> Result<Vec<Self::Entity>>;
    /// `WildernessZone.isInZone(entity)`.
    fn wilderness_in_zone(&mut self, entity: &Self::Entity) -> Result<bool>;
    /// `RegionManager.forId(regionId)`.
    fn region_for_id(&mut self, region_id: i32) -> Result<Self::Region>;
    /// `region.isTolerated(player)`.
    fn is_tolerated(&mut self, region: &Self::Region, player: &Self::Entity) -> Result<bool>;
    /// `player.getRights()`; `None` is null.
    fn rights(&mut self, player: &Self::Entity) -> Result<Option<Rights>>;
    /// `Player.isInvisible()`.
    fn player_invisible(&mut self, player: &Self::Entity) -> Result<bool>;
    /// `skullManager.isWilderness()`.
    fn skull_wilderness(&mut self, player: &Self::Entity) -> Result<bool>;
    /// `skullManager.hasWildernessProtection()`.
    fn wilderness_protection(&mut self, player: &Self::Entity) -> Result<bool>;
    /// `GameWorld.getSettings() != null`.
    fn settings(&mut self) -> Result<bool>;
    /// `settings.getWild_pvp_enabled()`.
    fn wild_pvp_enabled(&mut self) -> Result<bool>;
    /// `zoneMonitor.continueAttack(target, style, message)` of `entity`.
    fn zone_continue_attack(
        &mut self,
        entity: &Self::Entity,
        target: &Self::Entity,
        style: Option<CombatStyle>,
        message: bool,
    ) -> Result<bool>;
    /// `zoneMonitor.isPvPable(attacker, victim, style, message)` of `owner`.
    fn zone_is_pvpable(
        &mut self,
        owner: &Self::Entity,
        attacker: &Self::Entity,
        victim: &Self::Entity,
        style: Option<CombatStyle>,
        message: bool,
    ) -> Result<bool>;

    /// `skills.getLevel(slot)`.
    fn level(&mut self, entity: &Self::Entity, slot: i32) -> Result<i32>;
    /// `Familiar.getOwner()`.
    fn familiar_owner(&mut self, familiar: &Self::Entity) -> Result<Option<Self::Entity>>;
    /// `packetDispatch.sendMessage(text)`.
    fn send_message(&mut self, player: &Self::Entity, text: &str) -> Result<()>;
    /// `Pathfinder.find(mover, destination)`.
    fn path_find(
        &mut self,
        mover: Option<&Self::Entity>,
        destination: Option<Location>,
    ) -> Result<()>;
    /// `.walk(mover)` of the path just found.
    fn path_walk(&mut self, mover: Option<&Self::Entity>) -> Result<()>;
    /// `Entity.moveStep()`.
    fn move_step(&mut self, entity: &Self::Entity) -> Result<()>;
    /// `walkingQueue.hasPath()`.
    fn walking_has_path(&mut self, entity: &Self::Entity) -> Result<bool>;
    /// `pulseManager.isMovingPulse()`.
    fn pulse_manager_is_moving(&mut self, entity: &Self::Entity) -> Result<bool>;
    /// `equipment.getNew(SLOT_RING).getId()`.
    fn ring_id(&mut self, player: &Self::Entity) -> Result<i32>;
    /// `sendChat(text)`.
    fn send_chat(&mut self, entity: &Self::Entity, text: &str) -> Result<()>;
    /// `target.getImpactHandler().manualHit(source, hit, type)`.
    fn manual_hit(
        &mut self,
        target: &Self::Entity,
        source: &Self::Entity,
        hit: i32,
        kind: Hitsplat,
    ) -> Result<()>;
    /// `getImpactHandler().handleRecoilEffect(attacker, hit)`.
    fn handle_recoil_effect(
        &mut self,
        player: &Self::Entity,
        attacker: Option<&Self::Entity>,
        hit: i32,
    ) -> Result<()>;
    /// `degrader.checkArmourDegrades(player)`.
    fn check_armour_degrades(&mut self, player: &Self::Entity) -> Result<()>;

    /// `walkingQueue.reset()`.
    fn walking_reset(&mut self, entity: &Self::Entity) -> Result<()>;
    /// `pulseManager.clear()`.
    fn pulse_manager_clear(&mut self, entity: &Self::Entity) -> Result<()>;
    /// `updateMasks.reset()`.
    fn masks_reset(&mut self, entity: &Self::Entity) -> Result<()>;
    /// `location.getChunk().remove(npc)`.
    fn chunk_remove(&mut self, npc: &Self::Entity, location: Location) -> Result<()>;
    /// `location.getChunk().add(npc)`.
    fn chunk_add(&mut self, npc: &Self::Entity, location: Location) -> Result<()>;
    /// The return-to-spawn `MovementPulse.pulse()` body: the binding runs
    /// `world::npc::State::return_arrival`.
    fn return_arrival(&mut self, npc: &Self::Entity) -> Result<bool>;
    /// `Pulse.stop()` of the return-to-spawn pulse.
    fn pulse_stop(&mut self, npc: &Self::Entity, pulse: usize) -> Result<()>;
    /// `Repository.removeRenderableNPC(npc)`.
    fn remove_renderable(&mut self, npc: &Self::Entity) -> Result<()>;
    /// `NPC.clear()`.
    fn npc_clear(&mut self, npc: &Self::Entity) -> Result<()>;
    /// `skills.restore()`.
    fn skills_restore(&mut self, entity: &Self::Entity) -> Result<()>;
    /// `timers.removeTimer(name)`.
    fn remove_timer(&mut self, entity: &Self::Entity, name: &str) -> Result<()>;
    /// `RegionManager.getClippingFlag(z, x, y)`.
    fn clipping_flag(&mut self, z: i32, x: i32, y: i32) -> Result<i32>;
    /// `getClippingFlag` of a content-defined supplier.
    fn other_clipping_flag(&mut self, supplier: u32, z: i32, x: i32, y: i32) -> Result<i32>;
}

/// The NPC's [`Ai`], or a null dereference.
pub(crate) fn ai<'a, H: Host>(h: &'a mut H, npc: &H::Entity) -> Result<&'a mut Ai<H>> {
    h.ai(npc).ok_or(Error::Null)
}

/// The NPC's `world::npc` state, or a null dereference.
pub(crate) fn brain<H: Host>(h: &H, npc: &H::Entity) -> Result<npc::State> {
    h.brain(npc).ok_or(Error::Null)
}

/// The NPC's behaviour, cloned out so no borrow spans the hook.
pub fn behaviour<H: Host>(h: &mut H, npc: &H::Entity) -> Result<Arc<dyn Behaviour<H>>> {
    Ok(ai(h, npc)?.behaviour.clone())
}

/// The NPC's runtime class, cloned out so no borrow spans the call.
pub fn class<H: Host>(h: &mut H, npc: &H::Entity) -> Result<Arc<dyn NpcClass<H>>> {
    Ok(ai(h, npc)?.class.clone())
}
