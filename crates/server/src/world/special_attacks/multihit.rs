//! core/game/node/entity/combat/MultihitTargets.kt: the extra victims of the
//! multi-target attacks. `getLocalEntities` is a region service; every
//! `location` read is one recorded getter call, in Kotlin's evaluation order.
//! `filter` runs over the whole list before `take`.
use super::{Host, at, transform, within_distance};
use crate::world::combat_swing::{CombatStyle, Result};

/// The private `isAttackable(victim, attacker, style, wilderness)`.
fn attackable<H: Host>(
    h: &mut H,
    victim: &H::Entity,
    attacker: &H::Entity,
    style: CombatStyle,
    wilderness: bool,
) -> Result<bool> {
    if wilderness {
        return h.wilderness_continue_attack(attacker, victim, style, false);
    }
    h.entity_attackable(victim, attacker, Some(style), false)
}

/// `findMultihitTargets(target, attacker, style)`: the target and up to nine
/// attackable entities within one tile of it.
pub fn find_multihit_targets<H: Host>(
    h: &mut H,
    target: &H::Entity,
    attacker: &H::Entity,
    style: CombatStyle,
) -> Result<Vec<H::Entity>> {
    let here = at(h, target)?;
    let local = h.local_entities(here, 1)?;
    let mut targets = vec![];
    for it in local {
        if !h.same_entity(&it, attacker)
            && !h.same_entity(&it, target)
            && attackable(h, &it, attacker, style, false)?
        {
            targets.push(it);
        }
    }
    let mut out = vec![target.clone()];
    out.extend(targets.into_iter().take(9));
    Ok(out)
}

/// `findMultihitTargetsForDragonHalberd(target, attacker)`: the first
/// attackable entity on each side of the target, across the attack direction.
pub fn find_multihit_targets_for_dragon_halberd<H: Host>(
    h: &mut H,
    target: &H::Entity,
    attacker: &H::Entity,
) -> Result<Vec<H::Entity>> {
    let dx = at(h, target)?[0].wrapping_sub(at(h, attacker)?[0]);
    let dy = at(h, target)?[1].wrapping_sub(at(h, attacker)?[1]);
    let (step_x, step_y) = direction_step(dx, dy);
    let mut targets = vec![];
    for delta in [-1i32, 1] {
        let place = transform(
            at(h, target)?,
            delta.wrapping_mul(step_y),
            delta.wrapping_mul(step_x),
            0,
        );
        let add = h.local_entities(place, 0)?;
        let mut kept = vec![];
        for it in add {
            if !h.same_entity(&it, attacker)
                && attackable(h, &it, attacker, CombatStyle::Range, false)?
            {
                kept.push(it);
            }
        }
        targets.extend(kept.into_iter().take(1));
    }
    Ok(targets)
}

/// `findMultihitTargetsForD2h(target, attacker, style)`: the target and up to
/// fourteen attackable entities within one tile of the attacker.
pub fn find_multihit_targets_for_d2h<H: Host>(
    h: &mut H,
    target: &H::Entity,
    attacker: &H::Entity,
    style: CombatStyle,
) -> Result<Vec<H::Entity>> {
    let here = at(h, attacker)?;
    let local = h.local_entities(here, 1)?;
    let mut targets = vec![];
    for it in local {
        if !h.same_entity(&it, attacker)
            && !h.same_entity(&it, target)
            && attackable(h, &it, attacker, style, false)?
        {
            targets.push(it);
        }
    }
    let mut near = vec![];
    for it in targets {
        let there = at(h, &it)?;
        let here = at(h, attacker)?;
        if within_distance(there, here, 1) {
            near.push(it);
        }
    }
    let mut out = vec![target.clone()];
    out.extend(near.into_iter().take(14));
    Ok(out)
}

/// `findMultihitTargetsForChinchompa(target, attacker)`.
pub fn find_multihit_targets_for_chinchompa<H: Host>(
    h: &mut H,
    target: &H::Entity,
    attacker: &H::Entity,
) -> Result<Vec<H::Entity>> {
    let here = at(h, target)?;
    let all = h.local_entities(here, 1)?;
    let mut candidates = vec![];
    for it in &all {
        if !h.same_entity(it, attacker)
            && !h.same_entity(it, target)
            && attackable(h, it, attacker, CombatStyle::Range, false)?
        {
            candidates.push(it.clone());
        }
    }
    let mut targets = vec![];
    if h.kind(target).is_player() {
        let owners: Vec<H::Entity> = all
            .iter()
            .filter(|e| h.kind(e).is_player())
            .cloned()
            .collect();
        for it in candidates {
            let keep = (h.kind(&it).is_player()
                && attackable(h, &it, attacker, CombatStyle::Range, true)?)
                || is_familiar_of(h, &it, &owners, attacker)?;
            if keep {
                targets.push(it);
            }
        }
    } else {
        let npcs: Vec<H::Entity> = all.iter().filter(|e| h.kind(e).is_npc()).cloned().collect();
        for it in candidates {
            let keep = h.kind(&it).is_npc() || is_owner_of(h, &it, &npcs, attacker)?;
            if keep {
                targets.push(it);
            }
        }
    }
    let mut out = vec![target.clone()];
    out.extend(targets.into_iter().take(9));
    Ok(out)
}

/// The local `isFamiliarOf(entity, players)`.
fn is_familiar_of<H: Host>(
    h: &mut H,
    entity: &H::Entity,
    players: &[H::Entity],
    attacker: &H::Entity,
) -> Result<bool> {
    if h.kind(entity) != crate::world::combat_swing::Kind::Familiar {
        return Ok(false);
    }
    let owner = h.familiar_owner(entity)?;
    let known = owner
        .as_ref()
        .is_some_and(|o| players.iter().any(|p| h.same_entity(p, o)));
    if !known {
        return Ok(false);
    }
    let owner = h
        .familiar_owner(entity)?
        .ok_or(crate::world::combat_swing::Error::Null)?;
    attackable(h, &owner, attacker, CombatStyle::Range, true)
}

/// The local `isOwnerOf(entity, npcs)`.
fn is_owner_of<H: Host>(
    h: &mut H,
    entity: &H::Entity,
    npcs: &[H::Entity],
    attacker: &H::Entity,
) -> Result<bool> {
    if !h.kind(entity).is_player() {
        return Ok(false);
    }
    for npc in npcs {
        if h.kind(npc) == crate::world::combat_swing::Kind::Familiar {
            let owner = h.familiar_owner(npc)?;
            if owner.is_some_and(|o| h.same_entity(&o, entity)) {
                return attackable(h, entity, attacker, CombatStyle::Range, true);
            }
        }
    }
    Ok(false)
}

/// `Direction.getDirection(diffX, diffY)`'s step.
fn direction_step(dx: i32, dy: i32) -> (i32, i32) {
    let y = if dy < 0 {
        -1
    } else if dy > 0 || dx == 0 {
        1
    } else {
        0
    };
    (dx.signum(), y)
}
