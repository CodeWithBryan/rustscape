//! The members of `CombatSwingHandler` and its melee, range and magic
//! subclasses that run around and after `swing`: reach and attackability,
//! battle-state adjustment, experience, ammunition, visuals, impact and the
//! special-attack registry. Each function names the handler instance by its
//! host reference and dispatches on the instance's class.
use super::{
    AmmunitionVisuals, Host, Prayer, ProjectileRequest, Task, WorldPulse, impact, pulse, skill,
    slot,
};
use crate::world::combat_swing::{
    self as cs, ArmourSet, BattleState, BoltEffect, CombatStyle, Error, HandlerKind,
    InteractionType, Kind, Result, Target, WeaponType,
};
use crate::world::doors::Location;
use crate::world::pathfinding::{self, projectile};
use rs09_game_core::direction::Direction;
use std::cell::RefCell;

type Loc = [i32; 3];

const PREVENT_NORTH: i32 = 0x12c0120;
const PREVENT_EAST: i32 = 0x12c0180;
const PREVENT_NORTHEAST: i32 = 0x12c01e0;
const PREVENT_SOUTH: i32 = 0x12c0102;
const PREVENT_SOUTHEAST: i32 = 0x12c0183;
const PREVENT_WEST: i32 = 0x12c0108;
const PREVENT_SOUTHWEST: i32 = 0x12c010e;
const PREVENT_NORTHWEST: i32 = 0x12c0138;

/// `Location(x, y, z)`: both coordinates keep fifteen signed bits and the
/// plane is reduced into `0..4`.
pub fn location(x: i32, y: i32, z: i32) -> Loc {
    let signed = |n: i32| {
        let raw = n & 0x7fff;
        if raw >= 0x4000 { raw - 0x8000 } else { raw }
    };
    [
        signed(x),
        signed(y),
        z.wrapping_rem(4).wrapping_add(4).wrapping_rem(4) & 3,
    ]
}

/// `Location.transform(diffX, diffY, z)`.
fn transform(l: Loc, dx: i32, dy: i32, dz: i32) -> Loc {
    location(
        l[0].wrapping_add(dx),
        l[1].wrapping_add(dy),
        l[2].wrapping_add(dz).wrapping_rem(4),
    )
}

/// `Location.transform(dir)`.
fn step(l: Loc, dir: Direction) -> Loc {
    let (dx, dy) = dir.step();
    location(l[0].wrapping_add(dx), l[1].wrapping_add(dy), l[2])
}

/// `this.withinDistance(other, dist)`.
fn within_distance(this: Loc, other: Loc, dist: i32) -> bool {
    if other[2] != this[2] {
        return false;
    }
    cs::distance(this, other) <= f64::from(dist)
}

/// `this.deriveDirection(location)`.
fn derive_direction(this: Loc, to: Loc) -> Option<Direction> {
    let clamp = |d: i32| if d >= 0 { d.min(1) } else { -1 };
    let dx = clamp(to[0].wrapping_sub(this[0]));
    let dy = clamp(to[1].wrapping_sub(this[1]));
    if dx == 0 && dy == 0 {
        return None;
    }
    Some(Direction::from_delta(dx, dy))
}

/// `Location.getStepComponents(dir)`: the x neighbour first, then the y one.
fn step_components(this: Loc, dir: Direction) -> Vec<Loc> {
    let (dx, dy) = dir.step();
    let mut out = Vec::with_capacity(2);
    if dx != 0 {
        out.push(transform(this, -dx, 0, 0));
    }
    if dy != 0 {
        out.push(transform(this, 0, -dy, 0));
    }
    out
}

/// `Pathfinder.isStandingIn`.
#[allow(clippy::too_many_arguments)]
fn is_standing_in(
    x: i32,
    y: i32,
    mover_x: i32,
    mover_y: i32,
    dest_x: i32,
    dest_y: i32,
    size_x: i32,
    size_y: i32,
) -> bool {
    if x >= size_x.wrapping_add(dest_x) || mover_x.wrapping_add(x) <= dest_x {
        return false;
    }
    if dest_y.wrapping_add(size_y) <= y || y.wrapping_add(mover_y) <= dest_y {
        return false;
    }
    true
}

/// `RandomFunction.random(maxValue)` and `randomize(value)`: no draw unless
/// the bound is positive.
fn random<H: Host>(h: &mut H, max_value: i32) -> Result<i32> {
    if max_value <= 0 {
        return Ok(0);
    }
    h.next_int(max_value)
}

fn kind_of<H: Host>(h: &mut H, r: &H::HandlerRef) -> HandlerKind {
    h.handler(r).kind
}

fn style_of<H: Host>(h: &mut H, r: &H::HandlerRef) -> Option<CombatStyle> {
    h.handler(r).style
}

fn clip<H: Host>(h: &mut H, l: Loc) -> Result<i32> {
    h.clipping_flag(l[2], l[0], l[1])
}

/// `isProjectileClipped(entity, victim, checkClose)`, joined to the native
/// projectile traversal. The answer is true when a projectile can travel.
pub fn is_projectile_clipped<H: Host>(
    h: &mut H,
    entity: &H::Entity,
    victim: Option<&H::Entity>,
    check_close: bool,
) -> Result<bool> {
    struct Node<'a, 'b, H: Host>(&'a RefCell<&'b mut H>, &'a H::Entity);
    struct Flags<'a, 'b, H: Host>(&'a RefCell<&'b mut H>);
    fn out(error: Error) -> pathfinding::Error {
        match error {
            Error::Null => pathfinding::Error::Null,
            Error::Index => pathfinding::Error::Index,
            Error::Binding(text) => pathfinding::Error::Binding(text),
        }
    }
    impl<H: Host> projectile::Footprint for Node<'_, '_, H> {
        fn size(&self) -> pathfinding::Result<i32> {
            self.0.borrow_mut().size(self.1).map_err(out)
        }
        fn location(&self) -> pathfinding::Result<Option<Location>> {
            let at = self.0.borrow_mut().location(self.1).map_err(out)?;
            Ok(at.map(|[x, y, z]| Location::new(x, y, z)))
        }
    }
    impl<H: Host> projectile::Context for Flags<'_, '_, H> {
        fn flag(&mut self, z: i32, x: i32, y: i32, projectile: bool) -> pathfinding::Result<i32> {
            let mut h = self.0.borrow_mut();
            if projectile {
                h.projectile_flag(z, x, y).map_err(out)
            } else {
                h.clipping_flag(z, x, y).map_err(out)
            }
        }
    }
    let cell = RefCell::new(h);
    let source = Node(&cell, entity);
    let target = victim.map(|v| Node(&cell, v));
    let result = projectile::can_reach(
        &mut Flags(&cell),
        &source,
        target.as_ref().map(|t| t as &dyn projectile::Footprint),
        check_close,
    );
    result.map_err(|error| match error {
        pathfinding::Error::Null => Error::Null,
        pathfinding::Error::Index => Error::Index,
        pathfinding::Error::Binding(text) => Error::Binding(text),
        other => Error::Binding(format!("projectile traversal: {other:?}")),
    })
}

/// `MeleeSwingHandler.usingHalberd`.
fn using_halberd<H: Host>(h: &mut H, entity: &H::Entity) -> Result<bool> {
    match h.kind(entity) {
        Kind::Player => {
            if let Some(weapon) = h.equipment(entity, slot::WEAPON)? {
                let id = h.item_id(&weapon);
                return Ok((3190..=3204).contains(&id) || id == 6599);
            }
            Ok(false)
        }
        Kind::Npc | Kind::Familiar => Ok(h.id(entity)? == 8612),
        Kind::Other => Ok(false),
    }
}

/// `MeleeSwingHandler.canMelee(entity, victim, distance)`.
pub fn can_melee<H: Host>(
    h: &mut H,
    entity: &H::Entity,
    victim: Option<&H::Entity>,
    distance: i32,
) -> Result<bool> {
    let e = h.location(entity)?;
    let Some(victim) = victim else {
        return Ok(false);
    };
    if h.id(entity)? == 7135 {
        let here = h.location(entity)?.ok_or(Error::Null)?;
        let there = h.location(victim)?.ok_or(Error::Null)?;
        if within_distance(here, there, 2) {
            return Ok(true);
        }
    }
    let x = h.location(victim)?.ok_or(Error::Null)?[0];
    let y = h.location(victim)?.ok_or(Error::Null)?[1];
    let size = h.size(entity)?;
    if distance == 1 {
        for i in 0..size {
            let at = e.ok_or(Error::Null)?;
            let (vx, vy) = (h.size(victim)?, h.size(victim)?);
            if is_standing_in(
                at[0].wrapping_sub(1),
                at[1].wrapping_add(i),
                1,
                1,
                x,
                y,
                vx,
                vy,
            ) {
                return Ok(true);
            }
            let (vx, vy) = (h.size(victim)?, h.size(victim)?);
            if is_standing_in(
                at[0].wrapping_add(size),
                at[1].wrapping_add(i),
                1,
                1,
                x,
                y,
                vx,
                vy,
            ) {
                return Ok(true);
            }
            let (vx, vy) = (h.size(victim)?, h.size(victim)?);
            if is_standing_in(
                at[0].wrapping_add(i),
                at[1].wrapping_sub(1),
                1,
                1,
                x,
                y,
                vx,
                vy,
            ) {
                return Ok(true);
            }
            let (vx, vy) = (h.size(victim)?, h.size(victim)?);
            if is_standing_in(
                at[0].wrapping_add(i),
                at[1].wrapping_add(size),
                1,
                1,
                x,
                y,
                vx,
                vy,
            ) {
                return Ok(true);
            }
        }
        if e == h.location(victim)? {
            return Ok(true);
        }
        let theirs = h.swing_handler(victim, false)?;
        if style_of(h, &theirs) != Some(CombatStyle::Melee) {
            return Ok(false);
        }
        let there = h.location(victim)?;
        let at = e.ok_or(Error::Null)?;
        if !within_distance(at, there.ok_or(Error::Null)?, 1) {
            return Ok(false);
        }
        let target = h.combat_pulse(victim).victim.clone();
        if !target.is_some_and(|t| h.same_entity(&t, entity)) {
            return Ok(false);
        }
        return Ok(h.index(entity)? < h.index(victim)?);
    }
    let here = h.center_location(entity)?;
    let there = h.center_location(victim)?;
    let reach = distance
        .wrapping_add(size >> 1)
        .wrapping_add(h.size(victim)? >> 1);
    Ok(within_distance(here, there, reach))
}

/// `checkStepInterval(dir, next)`.
fn check_step_interval<H: Host>(h: &mut H, dir: Direction, next: Loc) -> Result<InteractionType> {
    let components = step_components(next, dir);
    let blocked = match dir {
        Direction::North => clip(h, next)? & PREVENT_NORTH != 0,
        Direction::East => clip(h, next)? & PREVENT_EAST != 0,
        Direction::South => clip(h, next)? & PREVENT_SOUTH != 0,
        Direction::West => clip(h, next)? & PREVENT_WEST != 0,
        Direction::NorthEast => {
            clip(h, components[0])? & PREVENT_EAST != 0
                || clip(h, components[1])? & PREVENT_NORTH != 0
                || clip(h, next)? & PREVENT_NORTHEAST != 0
        }
        Direction::NorthWest => {
            clip(h, components[0])? & PREVENT_WEST != 0
                || clip(h, components[1])? & PREVENT_NORTH != 0
                || clip(h, next)? & PREVENT_NORTHWEST != 0
        }
        Direction::SouthEast => {
            clip(h, components[0])? & PREVENT_EAST != 0
                || clip(h, components[1])? & PREVENT_SOUTH != 0
                || clip(h, next)? & PREVENT_SOUTHEAST != 0
        }
        Direction::SouthWest => {
            clip(h, components[0])? & PREVENT_WEST != 0
                || clip(h, components[1])? & PREVENT_SOUTH != 0
                || clip(h, next)? & PREVENT_SOUTHWEST != 0
        }
    };
    Ok(if blocked {
        InteractionType::NoInteract
    } else {
        InteractionType::StillInteract
    })
}

/// `canStepTowards(entity, victim)`.
pub fn can_step_towards<H: Host>(
    h: &mut H,
    entity: &H::Entity,
    victim: &H::Entity,
) -> Result<InteractionType> {
    let from = h.location(entity)?;
    let closest_victim = h.closest_occupied_tile(victim, from)?;
    let closest_entity = h.closest_occupied_tile(entity, closest_victim)?;
    let closest_entity = closest_entity.ok_or(Error::Null)?;
    let closest_victim = closest_victim.ok_or(Error::Null)?;
    let Some(dir) = derive_direction(closest_entity, closest_victim) else {
        return Ok(InteractionType::StillInteract);
    };
    let mut next = closest_entity;
    let max_skip_steps = (cs::distance(next, closest_victim) as i32).wrapping_add(1);
    for _ in 0..max_skip_steps {
        if cs::distance(next, closest_victim) <= 3.0 {
            break;
        }
        next = step(next, dir);
    }
    if cs::distance(next, closest_victim) > 3.0 {
        return Ok(InteractionType::StillInteract);
    }
    let mut result = InteractionType::StillInteract;
    let max_iterations = cs::distance(next, closest_victim) as i32;
    for _ in 0..max_iterations {
        next = step(next, dir);
        result = check_step_interval(h, dir, next)?;
        if result == InteractionType::NoInteract {
            break;
        }
    }
    Ok(result)
}

/// `CombatSwingHandler.isAttackable(entity, victim)`.
pub fn is_attackable<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    entity: &H::Entity,
    victim: &H::Entity,
) -> Result<InteractionType> {
    if h.attribute_flag(victim, "return-to-spawn")? {
        return Ok(InteractionType::NoInteract);
    }
    if style_of(h, r) == Some(CombatStyle::Melee) {
        let step_type = can_step_towards(h, entity, victim)?;
        if step_type != InteractionType::StillInteract {
            return Ok(step_type);
        }
    }
    let component = h.autocast_component(entity)?;
    if (component || style_of(h, r) == Some(CombatStyle::Magic))
        && (h.autocast_spell(entity)?.is_none() || {
            let spell = h.autocast_spell(entity)?.ok_or(Error::Null)?;
            h.spell_id(&spell)? == 0
        })
        && h.kind(entity).is_player()
    {
        let extension = h.weapon_interface(entity)?;
        if component {
            h.close_component(entity)?;
            h.open_weapon_tab(entity, extension)?;
            pulse::stop(h, entity)?;
            h.attack(entity, victim)?;
            h.remove_attribute(entity, "autocast_component")?;
        }
        if extension {
            h.update_weapon_interface(entity)?;
        }
        h.debug(entity, "Adjusting attack style")?;
    }
    if h.location(entity)? == h.location(victim)? {
        if h.kind(entity).is_player()
            && h.kind(victim).is_player()
            && h.client_index(entity)? < h.client_index(victim)?
        {
            let target = h.combat_pulse(victim).victim.clone();
            if target.is_some_and(|t| h.same_entity(&t, entity)) {
                return Ok(InteractionType::StillInteract);
            }
        }
        return Ok(InteractionType::NoInteract);
    }
    let el = h.location(entity)?;
    let vl = h.location(victim)?;
    let (sx, sy) = (h.size(victim)?, h.size(victim)?);
    let vl = vl.ok_or(Error::Null)?;
    let evl = transform(vl, sx, sy, 0);
    let el = el.ok_or(Error::Null)?;
    if (el[0] >= vl[0] && el[0] < evl[0] && el[1] >= vl[1] && el[1] < evl[1]) || el[2] != vl[2] {
        return Ok(InteractionType::NoInteract);
    }
    Ok(InteractionType::StillInteract)
}

/// The range and magic reach test: the victim's centre within the combat
/// distance of the attacker's centre.
fn in_range<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    entity: &H::Entity,
    victim: &H::Entity,
    distance: i32,
) -> Result<bool> {
    let theirs = h.center_location(victim)?;
    let ours = h.center_location(entity)?;
    let handler = h.handler(r).clone();
    let reach = handler.combat_distance(h, Some(entity), Some(victim), distance)?;
    Ok(within_distance(theirs, ours, reach))
}

fn melee_can_swing<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    entity: &H::Entity,
    victim: &H::Entity,
) -> Result<InteractionType> {
    let mut distance = if using_halberd(h, entity)? { 2 } else { 1 };
    let mut kind = InteractionType::StillInteract;
    let mut good_range = can_melee(h, entity, Some(victim), distance)?;
    if !good_range {
        let target = h.combat_pulse(victim).victim.clone();
        if !target.is_some_and(|t| h.same_entity(&t, entity))
            && h.is_moving(victim)?
            && h.size(entity)? == 1
        {
            kind = InteractionType::MoveInteract;
            distance += if h.is_running_both(entity)? { 2 } else { 1 };
            good_range = can_melee(h, entity, Some(victim), distance)?;
        }
    }
    let close = !using_halberd(h, entity)?;
    if !is_projectile_clipped(h, entity, Some(victim), close)? {
        return Ok(InteractionType::NoInteract);
    }
    let is_running = h.run_dir(entity)? != -1;
    let enemy_running = h.run_dir(victim)? != -1;
    if is_attackable(h, r, entity, victim)? != InteractionType::NoInteract {
        let max_distance = if is_running {
            if enemy_running { 3 } else { 4 }
        } else {
            2
        };
        if h.is_moving(entity)?
            && {
                let here = h.location(entity)?;
                let there = h.location(victim)?;
                let here = here.ok_or(Error::Null)?;
                cs::distance(here, there.ok_or(Error::Null)?) <= f64::from(max_distance)
            }
            && good_range
        {
            return Ok(kind);
        } else if good_range {
            if can_step_towards(h, entity, victim)? == InteractionType::NoInteract {
                return Ok(InteractionType::NoInteract);
            }
            if kind == InteractionType::StillInteract {
                h.walking_reset(entity)?;
            }
            return Ok(kind);
        }
    }
    Ok(InteractionType::NoInteract)
}

fn range_can_swing<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    entity: &H::Entity,
    victim: &H::Entity,
) -> Result<InteractionType> {
    if !is_projectile_clipped(h, entity, Some(victim), false)? {
        return Ok(InteractionType::NoInteract);
    }
    let mut distance: i32 = 7;
    if h.kind(entity).is_player() {
        if !h.weapon_interface(entity)? {
            return Err(Error::Null);
        }
        let interface = h.weapon_interfaces(entity)?.ok_or(Error::Null)?;
        if interface.interface_id() == 91 {
            distance -= 2;
        }
    }
    if h.attack_style(entity)?.ok_or(Error::Null)?.style == cs::STYLE_LONG_RANGE {
        distance += 2;
    }
    if h.kind(entity).is_player() {
        let weapon = h.equipment_new(entity, slot::WEAPON)?;
        if let Some(rw) = h.range_weapon(h.item_id(&weapon)) {
            if rw.kind()? == WeaponType::DoubleShot || rw.kind()? == WeaponType::Degrading {
                distance = 10;
            }
        }
    }
    let mut good_range = in_range(h, r, entity, victim, distance)?;
    let mut kind = InteractionType::StillInteract;
    if h.is_moving(victim)? && !good_range {
        distance += 1;
        good_range = in_range(h, r, entity, victim, distance)?;
        kind = InteractionType::MoveInteract;
    }
    if good_range && is_attackable(h, r, entity, victim)? != InteractionType::NoInteract {
        if kind == InteractionType::StillInteract {
            h.walking_reset(entity)?;
        }
        return Ok(kind);
    }
    Ok(InteractionType::NoInteract)
}

fn magic_can_swing<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    entity: &H::Entity,
    victim: &H::Entity,
) -> Result<InteractionType> {
    if !is_projectile_clipped(h, entity, Some(victim), false)? {
        return Ok(InteractionType::NoInteract);
    }
    let mut distance: i32 = 10;
    let mut kind = InteractionType::StillInteract;
    let mut good_range = in_range(h, r, entity, victim, distance)?;
    if h.is_moving(victim)? && !good_range {
        distance += 1;
        good_range = in_range(h, r, entity, victim, distance)?;
        kind = InteractionType::MoveInteract;
    }
    if good_range && is_attackable(h, r, entity, victim)? != InteractionType::NoInteract {
        if kind == InteractionType::StillInteract {
            h.walking_reset(entity)?;
        }
        return Ok(kind);
    }
    Ok(InteractionType::NoInteract)
}

/// `canSwing(entity, victim)` of the instance's class.
pub fn can_swing<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    entity: &H::Entity,
    victim: &H::Entity,
) -> Result<InteractionType> {
    match kind_of(h, r) {
        HandlerKind::Melee => melee_can_swing(h, r, entity, victim),
        HandlerKind::Range => range_can_swing(h, r, entity, victim),
        HandlerKind::Magic => magic_can_swing(h, r, entity, victim),
    }
}

/// `getFormattedHit(attacker, victim, state, rawHit)`.
pub fn formatted_hit<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    attacker: &H::Entity,
    victim: &H::Entity,
    state: &BattleState<H>,
    raw_hit: i32,
) -> Result<i32> {
    let mut hit = h.entity_formatted_hit(attacker, state, raw_hit)? as i32;
    if h.kind(victim).is_player() {
        let shield = h.equipment(victim, slot::SHIELD)?;
        let shield_id = shield.as_ref().map(|s| h.item_id(s));
        if shield_id == Some(13742) && random(h, 100)? < 71 {
            hit = hit.wrapping_sub((f64::from(hit) * 0.25) as i32);
            if hit < 1 {
                hit = 0;
            }
        }
        if shield_id == Some(13740) {
            let reduce = f64::from(hit) * 0.30;
            let mut drain = f64::from(hit) * 0.15;
            if h.prayer_points(victim)? > drain && drain > 0.0 {
                if drain < 1.0 {
                    drain = 1.0;
                }
                hit = hit.wrapping_sub(reduce as i32);
                if hit < 1 {
                    hit = 0;
                }
                h.decrement_prayer_points(victim, drain)?;
            }
        }
    }
    if h.kind(attacker).is_player() && h.equipment(attacker, slot::WEAPON)?.is_some() {
        let weapon = h.equipment(attacker, slot::WEAPON)?.ok_or(Error::Null)?;
        if h.item_id(&weapon) == 14726 && state.style == Some(CombatStyle::Magic) {
            hit = hit.wrapping_add((f64::from(hit) * 0.15) as i32);
        }
    }
    if h.kind(attacker) == Kind::Familiar
        && h.kind(victim).is_player()
        && h.protects_from_summoning(victim)?
    {
        hit = 0;
    }
    let handler = h.handler(r).clone();
    handler.format_hit(h, Some(victim), hit)
}

/// The poison roll an NPC attacker makes in the melee and range classes.
fn npc_poison<H: Host>(h: &mut H, entity: &H::Entity, victim: &H::Entity) -> Result<()> {
    let poisonous = h.npc_poisonous(entity)?;
    let damage = h.npc_poison_severity(entity)?;
    if poisonous && damage > -1 && random(h, 10)? < 4 {
        h.apply_poison(victim, entity, damage)?;
    }
    Ok(())
}

/// `CombatSwingHandler.adjustBattleState`, the body the subclasses call last.
fn base_adjust<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    entity: &H::Entity,
    victim: &H::Entity,
    state: &mut BattleState<H>,
) -> Result<()> {
    *h.experience_mod() = 4.0;
    let mut total_hit: i32 = 0;
    if h.kind(entity).is_player() {
        h.familiar_adjust_battle_state(entity, state)?;
    }
    h.send_impact(entity, state)?;
    h.check_impact(victim, state)?;
    if (h.id(victim)? == 4474 && style_of(h, r) == Some(CombatStyle::Magic))
        || (h.id(victim)? == 7891 && style_of(h, r) == Some(CombatStyle::Melee))
    {
        *h.experience_mod() = 0.1;
        h.full_restore(victim)?;
        if state.estimated_hit >= 15 {
            state.estimated_hit = 14;
        }
        if state.secondary_hit >= 15 {
            state.secondary_hit = 14;
        }
    }
    if h.id(victim)? == 757 {
        *h.experience_mod() = 0.01;
    }
    let recurse = match &state.targets {
        Some(list) if !list.is_empty() => {
            !(list.len() == 1 && matches!(list[0], Some(Target::This)))
        }
        _ => false,
    };
    if recurse {
        let count = state.targets.as_ref().map_or(0, Vec::len);
        for at in 0..count {
            let taken = match state.targets.as_mut().and_then(|list| list.get_mut(at)) {
                Some(entry @ Some(Target::State(_))) => entry.take(),
                _ => None,
            };
            if let Some(Target::State(mut nested)) = taken {
                let outcome = adjust_battle_state(h, r, entity, victim, &mut nested);
                if let Some(entry) = state.targets.as_mut().and_then(|list| list.get_mut(at)) {
                    *entry = Some(Target::State(nested));
                }
                outcome?;
            }
        }
    }
    if state.estimated_hit > 0 {
        let raw = state.estimated_hit;
        state.estimated_hit = formatted_hit(h, r, entity, victim, state, raw)?;
        total_hit = total_hit.wrapping_add(state.estimated_hit);
    }
    if state.secondary_hit > 0 {
        let raw = state.secondary_hit;
        state.secondary_hit = formatted_hit(h, r, entity, victim, state, raw)?;
        total_hit = total_hit.wrapping_add(state.secondary_hit);
    }
    if h.kind(entity).is_player() {
        h.check_weapon_degrades(entity)?;
        if total_hit > 0
            && h.prayer_active(entity, Prayer::Smite)?
            && h.prayer_points(victim)? > 0.0
        {
            h.decrement_prayer_points(victim, f64::from(total_hit) * 0.25)?;
        }
        if h.attribute_flag(entity, "1hko")? {
            state.estimated_hit = h.lifepoints(victim)?;
        }
    }
    if h.kind(victim).is_npc()
        && h.protect_style(victim)?.is_some()
        && state.style == h.protect_style(victim)?
    {
        state.neutralize_hits();
    }
    Ok(())
}

/// `MagicSwingHandler.adjustBattleState`.
fn magic_adjust<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    entity: &H::Entity,
    victim: &H::Entity,
    state: &mut BattleState<H>,
) -> Result<()> {
    fn one<H: Host>(
        h: &mut H,
        r: &H::HandlerRef,
        entity: &H::Entity,
        s: &mut BattleState<H>,
    ) -> Result<()> {
        let victim = s.victim.clone().ok_or(Error::Null)?;
        base_adjust(h, r, entity, &victim, s)?;
        if let Some(spell) = s.spell.clone() {
            let victim = s.victim.clone();
            h.spell_fire_effect(&spell, entity, victim.as_ref(), s)?;
        }
        Ok(())
    }
    if state.targets.is_none() {
        base_adjust(h, r, entity, victim, state)?;
        if let Some(spell) = state.spell.clone() {
            h.spell_fire_effect(&spell, entity, Some(victim), state)?;
        }
        return Ok(());
    }
    let count = state.targets.as_ref().map_or(0, Vec::len);
    for at in 0..count {
        let this = matches!(
            state.targets.as_ref().and_then(|list| list.get(at)),
            Some(Some(Target::This))
        );
        if this {
            one(h, r, entity, state)?;
            continue;
        }
        let taken = match state.targets.as_mut().and_then(|list| list.get_mut(at)) {
            Some(entry @ Some(Target::State(_))) => entry.take(),
            _ => None,
        };
        if let Some(Target::State(mut nested)) = taken {
            let outcome = one(h, r, entity, &mut nested);
            if let Some(entry) = state.targets.as_mut().and_then(|list| list.get_mut(at)) {
                *entry = Some(Target::State(nested));
            }
            outcome?;
        }
    }
    Ok(())
}

/// `adjustBattleState(entity, victim, state)` of the instance's class.
pub fn adjust_battle_state<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    entity: &H::Entity,
    victim: &H::Entity,
    state: &mut BattleState<H>,
) -> Result<()> {
    match kind_of(h, r) {
        HandlerKind::Melee => {
            if h.kind(entity).is_player() {
                if state.estimated_hit > 0 {
                    let weapon = h.equipment_new(entity, slot::WEAPON)?;
                    let name = h.item_name(&weapon);
                    let damage =
                        if name.contains("(p++") || name.contains("(s)") || name.contains("(kp)") {
                            30
                        } else if name.contains("(p+)") {
                            25
                        } else if name.contains("(p)") {
                            20
                        } else {
                            -1
                        };
                    if damage > -1 && random(h, 10)? < 4 {
                        h.apply_poison(victim, entity, damage)?;
                    }
                }
            } else if h.kind(entity).is_npc() {
                npc_poison(h, entity, victim)?;
            }
            base_adjust(h, r, entity, victim, state)
        }
        HandlerKind::Range => {
            if state.ammunition.is_some() && h.kind(entity).is_player() {
                let damage = state.ammunition.as_ref().map_or(0, |a| a.poison_damage);
                if state.estimated_hit > 0 && damage > 8 && random(h, 10)? < 4 {
                    h.apply_poison(victim, entity, damage)?;
                }
            } else if h.kind(entity).is_npc() {
                npc_poison(h, entity, victim)?;
            }
            base_adjust(h, r, entity, victim, state)
        }
        HandlerKind::Magic => magic_adjust(h, r, entity, victim, state),
    }
}

/// `CombatSwingHandler.addExperience`.
fn base_add_experience<H: Host>(
    h: &mut H,
    entity: Option<&H::Entity>,
    victim: Option<&H::Entity>,
    state: Option<&BattleState<H>>,
) -> Result<()> {
    let Some(entity) = entity else {
        return Ok(());
    };
    let victim_is_npc = victim.is_some_and(|v| h.kind(v).is_npc());
    if victim.is_some_and(|v| h.kind(v).is_player())
        && h.kind(entity).is_player()
        && h.is_ironman(entity)?
    {
        return Ok(());
    }
    let (player, attack_style) = match h.kind(entity) {
        Kind::Familiar => {
            let owner = h.familiar_owner(entity)?.ok_or(Error::Null)?;
            (owner, h.familiar_attack_style(entity)?)
        }
        Kind::Player => (
            entity.clone(),
            h.attack_style(entity)?.ok_or(Error::Null)?.style,
        ),
        _ => return Ok(()),
    };
    let multiplier = |h: &mut H| -> Result<f64> {
        match victim {
            Some(v) => h.npc_xp_multiplier(v, &player),
            None => Err(Error::Null),
        }
    };
    if victim_is_npc {
        let factor = multiplier(h)?;
        *h.experience_mod() *= factor;
    }
    let Some(state) = state else {
        return Ok(());
    };
    let hit = f64::from(state.total_damage());
    if h.kind(entity).is_player() {
        let mut experience = hit * 1.33;
        if victim_is_npc {
            experience *= multiplier(h)?;
        }
        h.add_experience(&player, skill::HITPOINTS, experience, true)?;
    }
    let slot = match attack_style {
        cs::STYLE_DEFENSIVE => skill::DEFENCE,
        cs::STYLE_ACCURATE => skill::ATTACK,
        cs::STYLE_AGGRESSIVE => skill::STRENGTH,
        cs::STYLE_CONTROLLED => {
            let mut experience = hit * *h.experience_mod();
            experience /= 3.0;
            h.add_experience(&player, skill::ATTACK, experience, true)?;
            h.add_experience(&player, skill::STRENGTH, experience, true)?;
            h.add_experience(&player, skill::DEFENCE, experience, true)?;
            return Ok(());
        }
        cs::STYLE_RANGE_ACCURATE | cs::STYLE_RAPID => skill::RANGE,
        cs::STYLE_LONG_RANGE => {
            let experience = hit * (*h.experience_mod() / 2.0);
            h.add_experience(&player, skill::RANGE, experience, true)?;
            let experience = hit * (*h.experience_mod() / 2.0);
            h.add_experience(&player, skill::DEFENCE, experience, true)?;
            return Ok(());
        }
        cs::STYLE_CAST => skill::MAGIC,
        cs::STYLE_DEFENSIVE_CAST => {
            let mut experience = hit;
            if victim_is_npc {
                experience *= multiplier(h)?;
            }
            h.add_experience(&player, skill::MAGIC, experience * 1.33, true)?;
            h.add_experience(&player, skill::DEFENCE, experience, true)?;
            return Ok(());
        }
        _ => return Ok(()),
    };
    let experience = hit * *h.experience_mod();
    h.add_experience(&player, slot, experience, true)
}

/// `addExperience(entity, victim, state)` of the instance's class.
pub fn add_experience<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    entity: Option<&H::Entity>,
    victim: Option<&H::Entity>,
    state: Option<&BattleState<H>>,
) -> Result<()> {
    if kind_of(h, r) == HandlerKind::Magic {
        if let (Some(state), Some(entity)) = (state, entity) {
            if let Some(spell) = state.spell.clone() {
                if h.kind(entity).is_player() {
                    return h.spell_add_experience(&spell, entity, state.total_damage());
                }
            }
        }
    }
    base_add_experience(h, entity, victim, state)
}

/// `RangeSwingHandler.getDropRate`.
fn drop_rate<H: Host>(h: &mut H, e: &H::Entity) -> Result<f64> {
    if h.kind(e).is_player() {
        let cape = h.equipment(e, slot::CAPE)?;
        let weapon = h.equipment(e, slot::WEAPON)?;
        let ammo = h.equipment(e, slot::ARROWS)?;
        let ammo_excluded = ammo.is_some_and(|a| h.item_id(&a) == 4740);
        let cape_id = cape.map(|c| h.item_id(&c));
        let weapon_id = weapon.map(|w| h.item_id(&w));
        if matches!(cape_id, Some(10498 | 10499))
            && weapon_id.is_some_and(|id| id != 10034 && id != 10033)
            && !ammo_excluded
        {
            if random(h, 100)? < 80 {
                let torso = h.equipment(e, slot::CHEST)?;
                let model = match torso {
                    Some(item) => h.item_male_worn_model(&item)?,
                    None => -1,
                };
                if model == 301 || model == 306 || model == 3379 {
                    h.send_message(e, "Your armour interferes with Ava's device.")?;
                    return Ok(1.0);
                }
                return Ok(-1.0);
            }
            return Ok(0.33);
        }
    }
    Ok(1.0)
}

/// `RangeSwingHandler.useAmmo(e, state, location)`.
pub fn use_ammo<H: Host>(
    h: &mut H,
    e: &H::Entity,
    state: &BattleState<H>,
    location: Option<[i32; 3]>,
) -> Result<()> {
    let mut drop_location = location;
    if !h.kind(e).is_player() {
        return Ok(());
    }
    let weapon = state.weapon.as_ref().ok_or(Error::Null)?;
    let kind = weapon.kind;
    let amount = if kind == Some(WeaponType::DoubleShot) {
        2
    } else {
        1
    };
    if kind == Some(WeaponType::Degrading) {
        return Ok(());
    }
    let Some(ammo) = weapon.ammunition.clone() else {
        return Ok(());
    };
    if weapon.ammunition_slot == -1 {
        return Ok(());
    }
    let ammunition_slot = weapon.ammunition_slot;
    let rate = drop_rate(h, e)?;
    if rate == -1.0 {
        return Ok(());
    }
    let (id, held, charge) = (h.item_id(&ammo), h.item_amount(&ammo), h.item_charge(&ammo));
    h.equipment_replace(
        e,
        Some((id, held.wrapping_sub(amount), charge)),
        ammunition_slot,
    )?;
    let Some(at) = drop_location else {
        return Ok(());
    };
    let flag = clip(h, at)?;
    if flag & 0x200000 != 0 {
        drop_location = None;
    }
    let range_weapon = state.range_weapon.clone();
    if let Some(at) = drop_location {
        if range_weapon.as_ref().ok_or(Error::Null)?.drop_ammo {
            let chance = 5.0 * (1.0 + f64::from(h.level(e, skill::RANGE)?) * 0.01) * rate;
            if random(h, chance as i32)? != 0 {
                if let Some(drop) = h.ground_item_increase(id, amount, at, e)? {
                    if h.duel_ammo(e)? {
                        h.duel_ammo_add(e, drop)?;
                    }
                }
            }
        }
    }
    let quiver = range_weapon.ok_or(Error::Null)?.ammunition_slot;
    if h.equipment(e, quiver)?.is_none() {
        h.send_message(e, "You have no ammo left in your quiver!")?;
    }
    Ok(())
}

/// `postSwing(entity, victim, state)` of the instance's class.
pub fn post_swing<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    entity: Option<&H::Entity>,
    victim: Option<&H::Entity>,
    state: &BattleState<H>,
) -> Result<()> {
    if kind_of(h, r) != HandlerKind::Range {
        return Ok(());
    }
    let entity = entity.ok_or(Error::Null)?;
    let location = h.location(victim.ok_or(Error::Null)?)?;
    use_ammo(h, entity, state, location)
}

/// `getAttackAnimation(e, style)`.
pub fn attack_animation<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    e: &H::Entity,
    style: Option<CombatStyle>,
) -> Result<H::Animation> {
    let mut animation = None;
    if style_of(h, r).is_some() && h.kind(e).is_npc() {
        let ordinal = match style.ok_or(Error::Null)? {
            CombatStyle::Melee => 0,
            CombatStyle::Range => 1,
            CombatStyle::Magic => 2,
        };
        animation = h.combat_animation(e, ordinal % 3)?;
    }
    match animation {
        Some(animation) => Ok(animation),
        None => h.attack_animation(e)?.ok_or(Error::Null),
    }
}

/// `visualize(entity, victim, state)` of the instance's class.
pub fn visualize<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    entity: &H::Entity,
    victim: Option<&H::Entity>,
    state: &BattleState<H>,
) -> Result<()> {
    match kind_of(h, r) {
        HandlerKind::Melee => {
            let animation = h.attack_animation(entity)?;
            h.animate(entity, animation.as_ref())
        }
        HandlerKind::Range => {
            let mut start = None;
            if let Some(ammunition) = state.ammunition.clone() {
                let AmmunitionVisuals {
                    start_graphics,
                    dark_bow_graphics,
                    projectile_id,
                } = h.ammunition_visuals(ammunition.item_id);
                start = start_graphics;
                if projectile_id.is_none() {
                    return Err(Error::Null);
                }
                h.send_projectile(ProjectileRequest::AmmunitionCopy {
                    ammunition_id: ammunition.item_id,
                    source: entity,
                    victim,
                    speed: 5.0,
                })?;
                let weapon = state.weapon.as_ref().ok_or(Error::Null)?;
                if weapon.kind == Some(WeaponType::DoubleShot) && dark_bow_graphics.is_some() {
                    start = dark_bow_graphics;
                    let here = h.location(entity)?;
                    let there = h.location(victim.ok_or(Error::Null)?)?;
                    let here = here.ok_or(Error::Null)?;
                    let gap = cs::distance(here, there.ok_or(Error::Null)?);
                    let speed = (55.0 + gap * 10.0) as i32;
                    h.send_projectile(ProjectileRequest::Create {
                        source: entity,
                        victim,
                        id: projectile_id.ok_or(Error::Null)?,
                        start_height: 40,
                        end_height: 36,
                        start_delay: 41,
                        speed,
                        angle: 25,
                    })?;
                }
            } else if h.kind(entity).is_npc() {
                let first = |h: &mut H, at: usize| -> Result<Option<cs::Graphics>> {
                    let list = h.npc_combat_graphics(entity)?.ok_or(Error::Null)?;
                    list.get(at).copied().ok_or(Error::Index)
                };
                if first(h, 0)?.is_some() {
                    start = first(h, 0)?;
                }
                if let Some(g) = first(h, 1)? {
                    h.send_projectile(ProjectileRequest::Ranged {
                        source: entity,
                        victim,
                        id: g.id,
                        start_height: g.height,
                        end_height: 36,
                        start_delay: 41,
                        angle: 5,
                    })?;
                }
            }
            let weapon = state.weapon.as_ref().and_then(|w| h.range_weapon(w.id));
            let current = h.attack_animation(entity)?.ok_or(Error::Null)?;
            let id = h.animation_id(&current);
            if let Some(weapon) = weapon {
                if id == 422 || id == 423 {
                    let animation = h.range_weapon_animation(weapon.item_id);
                    return h.visualize_entity(entity, animation.as_ref(), start);
                }
            }
            let animation = h.attack_animation(entity)?;
            h.visualize_entity(entity, animation.as_ref(), start)
        }
        HandlerKind::Magic => {
            let spell = state.spell.clone().ok_or(Error::Null)?;
            h.spell_visualize(&spell, entity, victim)
        }
    }
}

/// `visualizeImpact(entity, victim, state)` of the instance's class.
pub fn visualize_impact<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    entity: Option<&H::Entity>,
    victim: Option<&H::Entity>,
    state: &mut BattleState<H>,
) -> Result<()> {
    if kind_of(h, r) != HandlerKind::Magic {
        let victim = victim.ok_or(Error::Null)?;
        let animation = h.defence_animation(victim)?;
        return h.animate(victim, animation.as_ref());
    }
    if state.targets.is_none() {
        if let Some(spell) = state.spell.clone() {
            h.spell_visualize_impact(&spell, entity, victim, state)?;
        }
        return Ok(());
    }
    let count = state.targets.as_ref().map_or(0, Vec::len);
    for at in 0..count {
        let this = match state.targets.as_ref().and_then(|list| list.get(at)) {
            Some(Some(Target::This)) => true,
            Some(Some(Target::State(_))) => false,
            _ => continue,
        };
        let spell = state.spell.clone().ok_or(Error::Null)?;
        if this {
            let target = state.victim.clone();
            h.spell_visualize_impact(&spell, entity, target.as_ref(), state)?;
            continue;
        }
        let taken = state
            .targets
            .as_mut()
            .and_then(|list| list.get_mut(at))
            .and_then(Option::take);
        if let Some(Target::State(mut nested)) = taken {
            let target = nested.victim.clone();
            let outcome = h.spell_visualize_impact(&spell, entity, target.as_ref(), &mut nested);
            if let Some(entry) = state.targets.as_mut().and_then(|list| list.get_mut(at)) {
                *entry = Some(Target::State(nested));
            }
            outcome?;
        }
    }
    Ok(())
}

/// `visualizeAudio(entity, victim, state)`.
pub fn visualize_audio<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    entity: &H::Entity,
    state: &BattleState<H>,
) -> Result<()> {
    if h.kind(entity).is_player() {
        let style_index = h.attack_style_index(entity)?;
        let item = state.weapon.as_ref().and_then(|w| w.item.clone());
        if let Some(item) = item {
            if let Some(audios) = h.item_attack_audios(&item)? {
                let mut audio = None;
                if i64::from(style_index) < audios.len() as i64 {
                    let at = usize::try_from(style_index).map_err(|_| Error::Index)?;
                    audio = audios[at];
                }
                if audio.is_none() || audio == Some(0) {
                    audio = *audios.first().ok_or(Error::Index)?;
                }
                let at = h.location(entity)?;
                let id = audio.ok_or(Error::Null)?;
                h.play_global_audio(at.ok_or(Error::Null)?, id)?;
            }
        } else if style_of(h, r) == Some(CombatStyle::Melee) {
            let at = h.location(entity)?;
            h.play_global_audio(at.ok_or(Error::Null)?, 2564)?;
        }
    } else if h.kind(entity).is_npc() {
        if let Some(id) = h.npc_audio(entity, 0)? {
            let at = h.location(entity)?;
            h.play_global_audio(at.ok_or(Error::Null)?, id)?;
        }
    }
    Ok(())
}

/// The two hits one melee target state delivers.
fn melee_hits<H: Host>(
    h: &mut H,
    entity: Option<&H::Entity>,
    victim: Option<&H::Entity>,
    estimated: i32,
    secondary: i32,
) -> Result<()> {
    for hit in [estimated, secondary] {
        if hit > -1 {
            let victim = victim.ok_or(Error::Null)?;
            impact::handle_impact(
                h,
                victim,
                entity,
                hit,
                Some(CombatStyle::Melee),
                None,
                false,
            )?;
        }
    }
    Ok(())
}

/// `impact(entity, victim, state)` of the instance's class.
pub fn impact<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    entity: Option<&H::Entity>,
    victim: Option<&H::Entity>,
    state: &mut BattleState<H>,
) -> Result<()> {
    match kind_of(h, r) {
        HandlerKind::Melee => {
            let Some(list) = state.targets.as_ref() else {
                return melee_hits(h, entity, victim, state.estimated_hit, state.secondary_hit);
            };
            let hits: Vec<Option<(i32, i32)>> = list
                .iter()
                .map(|target| match target {
                    None => None,
                    Some(Target::This) => Some((state.estimated_hit, state.secondary_hit)),
                    Some(Target::State(s)) => Some((s.estimated_hit, s.secondary_hit)),
                })
                .collect();
            for pair in hits {
                let (estimated, secondary) = pair.ok_or(Error::Null)?;
                melee_hits(h, entity, victim, estimated, secondary)?;
            }
            Ok(())
        }
        HandlerKind::Range => {
            if let Some(effect) = state.ammunition.as_ref().and_then(|a| a.effect) {
                if effect != BoltEffect::Diamond && h.bolt_can_fire(effect, state)? {
                    h.bolt_impact(effect, state)?;
                }
            }
            let hit = state.estimated_hit;
            let victim = victim.ok_or(Error::Null)?;
            impact::handle_impact(
                h,
                victim,
                entity,
                hit,
                Some(CombatStyle::Range),
                None,
                false,
            )?;
            if state.secondary_hit > -1 {
                h.submit(WorldPulse::new(
                    1,
                    vec![Some(victim.clone())],
                    Task::SecondHit {
                        entity: entity.cloned(),
                        victim: victim.clone(),
                        hit: state.secondary_hit,
                    },
                ))?;
            }
            Ok(())
        }
        HandlerKind::Magic => {
            let Some(list) = state.targets.as_ref() else {
                if state.estimated_hit > -1 {
                    let victim = victim.ok_or(Error::Null)?;
                    let hit = state.estimated_hit;
                    let style = Some(CombatStyle::Magic);
                    impact::handle_impact(h, victim, entity, hit, style, None, false)?;
                }
                return Ok(());
            };
            let hits: Vec<Option<(i32, Option<H::Entity>)>> = list
                .iter()
                .map(|target| match target {
                    None => None,
                    Some(Target::This) => Some((state.estimated_hit, state.victim.clone())),
                    Some(Target::State(s)) => Some((s.estimated_hit, s.victim.clone())),
                })
                .collect();
            for (hit, target) in hits.into_iter().flatten() {
                if hit < 0 {
                    continue;
                }
                let target = target.ok_or(Error::Null)?;
                let style = Some(CombatStyle::Magic);
                impact::handle_impact(h, &target, entity, hit, style, None, false)?;
            }
            Ok(())
        }
    }
}

/// `onImpact(entity, victim, state)`.
pub fn on_impact<H: Host>(
    h: &mut H,
    entity: Option<&H::Entity>,
    victim: Option<&H::Entity>,
    state: Option<&mut BattleState<H>>,
) -> Result<()> {
    if let (Some(e), Some(v)) = (entity, victim) {
        if h.kind(e).is_player() {
            h.degrade(e, v, true)?;
        }
    }
    let Some(state) = state else {
        return Ok(());
    };
    let nested = match &state.targets {
        Some(list) if !list.is_empty() => {
            !(list.len() == 1 && matches!(list[0], Some(Target::This)))
        }
        _ => false,
    };
    if nested {
        let count = state.targets.as_ref().map_or(0, Vec::len);
        for at in 0..count {
            let taken = match state.targets.as_mut().and_then(|list| list.get_mut(at)) {
                Some(entry @ Some(Target::State(_))) => entry.take(),
                _ => None,
            };
            if let Some(Target::State(mut s)) = taken {
                let target = s.victim.clone();
                let outcome = on_impact(h, entity, target.as_ref(), Some(&mut s));
                if let Some(entry) = state.targets.as_mut().and_then(|list| list.get_mut(at)) {
                    *entry = Some(Target::State(s));
                }
                outcome?;
            }
        }
        return Ok(());
    }
    h.entity_on_impact(victim.ok_or(Error::Null)?, entity, state)
}

/// `ArmourSet.effect(e, victim, state)`: the four overrides and the base
/// method the other sets inherit.
pub fn armour_set_effect<H: Host>(
    h: &mut H,
    set: ArmourSet,
    e: &H::Entity,
    victim: &H::Entity,
    state: &BattleState<H>,
) -> Result<bool> {
    match set {
        ArmourSet::Ahrim => {
            if random(h, 100)? < 25 && state.estimated_hit > -1 {
                h.update_level(victim, skill::STRENGTH, -5, 0)?;
                return Ok(true);
            }
            Ok(false)
        }
        ArmourSet::Guthan => {
            if random(h, 100)? < 25 {
                h.heal(e, state.estimated_hit)?;
                return Ok(true);
            }
            Ok(false)
        }
        ArmourSet::Karil => {
            if state.estimated_hit > 0 && random(h, 100)? < 25 {
                let levels = h.dynamic_levels(victim)?.ok_or(Error::Null)?;
                let agility = *levels.get(skill::AGILITY as usize).ok_or(Error::Index)?;
                h.update_level(victim, skill::AGILITY, (agility / 5).wrapping_neg(), 0)?;
                return Ok(true);
            }
            Ok(false)
        }
        ArmourSet::Torag => {
            if state.estimated_hit > 0 && random(h, 100)? < 25 {
                if h.kind(victim).is_player() {
                    h.update_run_energy(victim, 20.0)?;
                }
                return Ok(true);
            }
            Ok(false)
        }
        _ => Ok(false),
    }
}

/// `ArmourSet.visualize(e, victim)`.
pub fn armour_set_visualize<H: Host>(h: &mut H, set: ArmourSet, victim: &H::Entity) -> Result<()> {
    if let Some(graphic) = set.end_graphic() {
        h.graphics(victim, graphic)?;
    }
    Ok(())
}

/// `register(itemId, handler)`: answers false, with an error log, when the
/// item already has a special attack handler.
pub fn register<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    item_id: i32,
    special: &H::HandlerRef,
) -> Result<bool> {
    let existing = h
        .specials(r)
        .entries
        .get_or_insert_with(Vec::new)
        .iter()
        .find(|(id, _)| *id == item_id)
        .map(|(_, handler)| handler.clone());
    if let Some(old) = existing {
        let text = format!(
            "Already contained special attack handler for item {item_id} - [old={}, new={}].",
            h.handler_class_name(&old),
            h.handler_class_name(special)
        );
        let origin = h.handler_class_name(r);
        h.log_class_error(&origin, &text)?;
        return Ok(false);
    }
    if let Some(entries) = h.specials(r).entries.as_mut() {
        entries.push((item_id, special.clone()));
    }
    Ok(true)
}

/// `getSpecial(itemId)`.
pub fn special<H: Host>(h: &mut H, r: &H::HandlerRef, item_id: i32) -> Option<H::HandlerRef> {
    h.specials(r)
        .entries
        .get_or_insert_with(Vec::new)
        .iter()
        .find(|(id, _)| *id == item_id)
        .map(|(_, handler)| handler.clone())
}
