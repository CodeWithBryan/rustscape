//! `AggressiveHandler` and `AggressiveBehavior` (DEFAULT and WILDERNESS).
use super::{Error, Host, Result, Rights, ai, combat};
use crate::definitions::item::Shared;
use std::sync::Arc;

/// `NPCConfigParser.AGGRESSIVE_RADIUS`.
pub const AGGRESSIVE_RADIUS: &str = "agg_radius";

/// The fields of one `AggressiveHandler` object.
pub struct Fields<H: Host> {
    /// The final `entity` the handler was constructed for.
    pub entity: H::Entity,
    pub radius: i32,
    /// An absolute tick: `setPauseTicks` adds the current tick.
    pub pause_ticks: i32,
    pub target_switching: bool,
    /// The final `behavior`.
    pub behaviour: Arc<dyn AggressiveBehaviour<H>>,
    pub chance_ratio: i32,
    /// `playerTolerance.length`, fixed at construction from
    /// `ServerConstants.MAX_PLAYERS`. The array is only ever written with
    /// zero, so its elements are not stored.
    pub tolerance_length: i32,
    pub allow_tolerance: bool,
}

/// One `AggressiveHandler` object: the NPC holds it, and a running
/// `selectTarget` keeps the object it was called on.
pub struct Handler<H: Host>(pub Shared<Fields<H>>);

impl<H: Host> Clone for Handler<H> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<H: Host> Handler<H> {
    /// `new AggressiveHandler(entity, behavior)`.
    pub fn new(h: &H, entity: H::Entity, behaviour: Arc<dyn AggressiveBehaviour<H>>) -> Self {
        Self(Shared::new(Fields {
            entity,
            radius: 4,
            pause_ticks: 0,
            target_switching: false,
            behaviour,
            chance_ratio: 2,
            tolerance_length: h.max_players(),
            allow_tolerance: true,
        }))
    }

    /// Reference identity of two handler objects.
    pub fn same(&self, other: &Self) -> bool {
        self.0.same(&other.0)
    }

    /// `setPauseTicks(ticks)`: pauses until the current tick plus `ticks`.
    pub fn set_pause_ticks(&self, h: &mut H, ticks: i32) -> Result<()> {
        let now = h.ticks()?;
        self.0.write().pause_ticks = now.wrapping_add(ticks);
        Ok(())
    }

    /// `removeTolerance(index)`.
    pub fn remove_tolerance(&self, index: i32) -> Result<()> {
        if index < 0 || index >= self.0.read().tolerance_length {
            return Err(Error::Index);
        }
        Ok(())
    }

    /// `isAllowTolerance()`: the definition's `can_tolerate` is read first,
    /// for an NPC, whatever the field holds.
    pub fn is_allow_tolerance(&self, h: &mut H) -> Result<bool> {
        let entity = self.0.read().entity.clone();
        let mut config = true;
        if h.kind(&entity).is_npc() {
            config = h.config_bool(&entity, "can_tolerate", true)?;
        }
        Ok(self.0.read().allow_tolerance && config)
    }

    /// `selectTarget()`.
    pub fn select_target(&self, h: &mut H) -> Result<bool> {
        let entity = self.0.read().entity.clone();
        let pause = self.0.read().pause_ticks;
        if pause > h.ticks()? || h.interaction_locked(&entity)? {
            return Ok(false);
        }
        let switching = self.0.read().target_switching;
        if (!switching && h.is_attacking(&entity)?) || h.is_dead(&entity)? {
            return Ok(false);
        }
        let draw = randomize(h, 10)?;
        if draw > self.0.read().chance_ratio {
            return Ok(false);
        }
        let (behaviour, radius) = {
            let f = self.0.read();
            (f.behaviour.clone(), f.radius)
        };
        let targets = behaviour.possible_targets(h, &entity, radius)?;
        let target = behaviour.logical_target(h, &entity, &targets)?;
        if let Some(target) = &target {
            if h.kind(target).is_player() {
                if h.attribute_bool(target, "ignore_aggression", false)? {
                    return Ok(false);
                }
                let rights = h.rights(target)?.ok_or(Error::Null)?;
                if rights == Rights::Administrator
                    && !h.attribute_bool(target, "allow_admin_aggression", false)?
                {
                    return Ok(false);
                }
            }
        }
        if let Some(target) = target {
            h.set_attribute_entity(&target, "aggressor", &entity)?;
            if h.is_attacking(&entity)? {
                h.combat_set_victim(&entity, &target)?;
                h.face(&entity, Some(&target))?;
            } else {
                h.combat_attack(&entity, Some(&target))?;
            }
            return Ok(true);
        }
        h.is_attacking(&entity)
    }
}

/// `RandomFunction.randomize(value)`.
pub fn randomize<H: Host>(h: &mut H, value: i32) -> Result<i32> {
    if value < 1 {
        return Ok(0);
    }
    h.next_int(value)
}

/// `AggressiveBehavior`: the base class's members as defaults. A content
/// subclass overrides a method; the defaults call each other through
/// `self`, as the original's virtual calls do.
pub trait AggressiveBehaviour<H: Host>: Send + Sync {
    /// `canSelectTarget(entity, target)`.
    fn can_select_target(&self, h: &mut H, entity: &H::Entity, target: &H::Entity) -> Result<bool> {
        base_can_select_target(self, h, entity, target)
    }
    /// `ignoreCombatLevelDifference()`.
    fn ignore_combat_level_difference(&self, _h: &mut H) -> Result<bool> {
        Ok(false)
    }
    /// `getPriorityFlag(target)`.
    fn priority_flag(&self, h: &mut H, target: &H::Entity) -> Result<i32> {
        let mut flag = 0i32;
        if combat::in_combat(h, target)? {
            flag = flag.wrapping_add(1);
        }
        if h.interaction_locked(target)? {
            flag = flag.wrapping_add(1);
        }
        if let Some(e) = h.attribute_entity(target, "aggressor")? {
            let victim = h.combat_victim(&e)?;
            if victim.is_some_and(|v| h.same(&v, target)) {
                flag = flag.wrapping_add(1);
            }
        }
        Ok(flag)
    }
    /// `getPossibleTargets(entity, radius)`.
    fn possible_targets(
        &self,
        h: &mut H,
        entity: &H::Entity,
        radius: i32,
    ) -> Result<Vec<H::Entity>> {
        let location = h.location(entity).ok_or(Error::Null)?;
        let mut targets = Vec::new();
        for player in h.local_players(location, radius)? {
            if self.can_select_target(h, entity, &player)? {
                targets.push(player);
            }
        }
        Ok(targets)
    }
    /// `getLogicalTarget(entity, possibleTargets)`.
    fn logical_target(
        &self,
        h: &mut H,
        _entity: &H::Entity,
        targets: &[H::Entity],
    ) -> Result<Option<H::Entity>> {
        let mut target = None;
        let mut comparing = f64::MAX;
        for e in targets {
            let flag = f64::from(self.priority_flag(h, e)?) + h.math_random()?;
            if flag <= comparing {
                comparing = flag;
                target = Some(e.clone());
            }
        }
        Ok(target)
    }
}

/// The base `canSelectTarget`, callable from overrides.
pub fn base_can_select_target<H: Host, B: AggressiveBehaviour<H> + ?Sized>(
    behaviour: &B,
    h: &mut H,
    entity: &H::Entity,
    target: &H::Entity,
) -> Result<bool> {
    let region_id = region_id(h.location(target).ok_or(Error::Null)?);
    if !h.is_active(target)? || h.is_dead(target)? {
        return Ok(false);
    }
    if !h.is_multi_zone(target)? && combat::in_combat(h, target)? {
        return Ok(false);
    }
    if h.kind(entity).is_npc() && h.kind(target).is_player() {
        let handler = ai(h, entity)?.handler.clone();
        if let Some(handler) = handler {
            if handler.is_allow_tolerance(h)? && !h.wilderness_in_zone(entity)? {
                let region = h.region_for_id(region_id)?;
                if h.is_tolerated(&region, target)? {
                    return Ok(false);
                }
            }
        }
    }
    let level = h.current_combat_level(target)?;
    let refused = level > h.current_combat_level(entity)?.wrapping_shl(1)
        && !behaviour.ignore_combat_level_difference(h)?;
    Ok(!refused)
}

/// `Location.getRegionId()`.
pub fn region_id(location: crate::world::doors::Location) -> i32 {
    let [x, y, _] = location.coordinates();
    ((x >> 6) << 8) | (y >> 6)
}

/// `AggressiveBehavior.DEFAULT`.
pub struct DefaultAggression;

impl<H: Host> AggressiveBehaviour<H> for DefaultAggression {}

/// `AggressiveBehavior.WILDERNESS`.
pub struct Wilderness;

impl<H: Host> AggressiveBehaviour<H> for Wilderness {
    fn can_select_target(&self, h: &mut H, entity: &H::Entity, target: &H::Entity) -> Result<bool> {
        if combat::is_invisible(h, target)? {
            return Ok(false);
        }
        if !h.is_active(target)? || h.is_dead(target)? {
            return Ok(false);
        }
        if !combat::can_select_target(h, entity, target)? {
            return Ok(false);
        }
        if !h.is_multi_zone(target)? && combat::in_combat(h, target)? {
            return Ok(false);
        }
        if h.kind(target).is_player() {
            return h.skull_wilderness(target);
        }
        Ok(true)
    }
}

/// `NPC.setDefaultBehavior()`.
pub fn set_default_behaviour<H: Host>(h: &mut H, npc: &H::Entity) -> Result<()> {
    let brain = super::brain(h, npc)?;
    let aggressive = brain.fields.read().aggressive;
    if aggressive && h.definition_combat_level(npc)? > 0 {
        let handler = Handler::new(h, npc.clone(), Arc::new(DefaultAggression));
        ai(h, npc)?.handler = Some(handler.clone());
        brain.fields.write().has_handler = true;
        let radius = h.config_int(npc, AGGRESSIVE_RADIUS, 4)?;
        handler.0.write().radius = radius;
    }
    Ok(())
}

/// `NPC.configureBossData()`.
pub fn configure_boss_data<H: Host>(h: &mut H, npc: &H::Entity) -> Result<()> {
    h.set_npc_walkable(npc, true)?;
    h.set_combat_time_out(npc, 2)?;
    let brain = super::brain(h, npc)?;
    let aggressive = brain.fields.read().aggressive;
    if !aggressive {
        brain.fields.write().aggressive = true;
        set_default_behaviour(h, npc)?;
    }
    if let Some(handler) = ai(h, npc)?.handler.clone() {
        let mut f = handler.0.write();
        f.chance_ratio = 6;
        f.allow_tolerance = false;
        f.target_switching = true;
        f.radius = 64;
    }
    brain.fields.write().radius = 64;
    Ok(())
}
