//! The combat members of `Entity`, `NPC` and `Player`: attackability,
//! `continueAttack`, `checkImpact`, `onImpact` with its retaliation,
//! `sendImpact`, `inCombat`, invisibility and the NPC's swing handler.
use super::{
    BattleState, CombatStyle, Error, Hitsplat, Host, Result, SLAYER, ai, behaviour, brain, class,
};
use crate::world::combat_swing::Kind;

/// `Entity.inCombat()`: `combat-time` against the wall clock.
pub fn in_combat<H: Host>(h: &mut H, entity: &H::Entity) -> Result<bool> {
    let time = h.attribute_long(entity, "combat-time", 0)?;
    Ok(time > h.current_time_millis()?)
}

/// `NPC.isInvisible()`.
pub fn npc_is_invisible<H: Host>(h: &mut H, npc: &H::Entity) -> Result<bool> {
    if !h.is_active(npc)? {
        return Ok(true);
    }
    let respawn = brain(h, npc)?.fields.read().respawn_tick;
    if respawn > h.ticks()? {
        return Ok(true);
    }
    Ok(ai(h, npc)?.invisible)
}

/// `NPC.isHidden(player)`: the player is not consulted.
pub fn npc_is_hidden<H: Host>(h: &mut H, npc: &H::Entity, _player: &H::Entity) -> Result<bool> {
    npc_is_invisible(h, npc)
}

/// `entity.isInvisible()` by runtime class: `NPC.isInvisible` for an NPC,
/// `Player.isInvisible` for a player.
pub fn is_invisible<H: Host>(h: &mut H, entity: &H::Entity) -> Result<bool> {
    match h.kind(entity) {
        Kind::Npc | Kind::Familiar => npc_is_invisible(h, entity),
        _ => h.player_invisible(entity),
    }
}

/// `entity.canSelectTarget(target)`: an NPC's runtime class answers, any
/// other entity takes `Entity`'s `true`.
pub fn can_select_target<H: Host>(
    h: &mut H,
    entity: &H::Entity,
    target: &H::Entity,
) -> Result<bool> {
    if h.kind(entity).is_npc() {
        let class = class(h, entity)?;
        return class.can_select_target(h, entity, target);
    }
    Ok(true)
}

/// `NPC.continueAttack(target, style, message)`.
pub fn npc_continue_attack<H: Host>(
    h: &mut H,
    npc: &H::Entity,
    _target: &H::Entity,
    _style: Option<CombatStyle>,
    _message: bool,
) -> Result<bool> {
    Ok(!npc_is_invisible(h, npc)?)
}

/// `Entity.isAttackable(entity, style, message)`.
pub fn entity_is_attackable<H: Host>(
    h: &mut H,
    victim: &H::Entity,
    attacker: &H::Entity,
    style: Option<CombatStyle>,
    message: bool,
) -> Result<bool> {
    if h.is_dead(victim)? {
        return Ok(false);
    }
    if !h.zone_continue_attack(attacker, victim, style, message)? {
        return Ok(false);
    }
    if !h.zone_continue_attack(victim, attacker, style, message)? {
        return Ok(false);
    }
    Ok(true)
}

/// `NPC.isAttackable(entity, style, message)`.
pub fn npc_is_attackable<H: Host>(
    h: &mut H,
    npc: &H::Entity,
    attacker: &H::Entity,
    style: Option<CombatStyle>,
    message: bool,
) -> Result<bool> {
    if npc_is_invisible(h, npc)? {
        return Ok(false);
    }
    let task = ai(h, npc)?.task;
    if let Some(task) = task
        && h.kind(attacker).is_player()
        && task.level_req > h.level(attacker, SLAYER)?
        && message
    {
        h.send_message(
            attacker,
            "You need a higher slayer level to know how to wound this monster.",
        )?;
    }
    let behaviour = behaviour(h, npc)?;
    let hook_style = style.ok_or(Error::Null)?;
    if !behaviour.can_be_attacked_by(h, npc, attacker, hook_style, message)? {
        return Ok(false);
    }
    entity_is_attackable(h, npc, attacker, style, message)
}

/// `victim.isAttackable(attacker, style, message)` by runtime class: the
/// NPC's class (by default `NPC.isAttackable`), `Player.isAttackable`, or
/// `Entity.isAttackable` for anything else.
pub fn is_attackable<H: Host>(
    h: &mut H,
    victim: &H::Entity,
    attacker: &H::Entity,
    style: Option<CombatStyle>,
    message: bool,
) -> Result<bool> {
    match h.kind(victim) {
        Kind::Npc | Kind::Familiar => {
            let class = class(h, victim)?;
            class.is_attackable(h, victim, attacker, style, message)
        }
        Kind::Player => player_is_attackable(h, victim, attacker, style, message),
        Kind::Other => entity_is_attackable(h, victim, attacker, style, message),
    }
}

/// `Player.isAttackable(entity, style, message)`.
pub fn player_is_attackable<H: Host>(
    h: &mut H,
    player: &H::Entity,
    attacker: &H::Entity,
    style: Option<CombatStyle>,
    message: bool,
) -> Result<bool> {
    if h.kind(attacker).is_npc() && !h.has_action(attacker, "attack")? {
        let class = class(h, attacker)?;
        if !class.is_ignore_attack_restrictions(h, attacker, player)? {
            return Ok(false);
        }
    }
    if h.kind(attacker).is_player() {
        if h.skull_wilderness(attacker)? && h.skull_wilderness(player)? {
            if !h.settings()? {
                return Err(Error::Null);
            }
            if !h.wild_pvp_enabled()? {
                return Ok(false);
            }
            if h.wilderness_protection(attacker)? {
                return Ok(false);
            }
            if h.wilderness_protection(player)? {
                return Ok(false);
            }
            return entity_is_attackable(h, player, attacker, style, message);
        }
        return h.zone_is_pvpable(player, attacker, player, style, message);
    }
    entity_is_attackable(h, player, attacker, style, message)
}

/// `Player.continueAttack(target, style, message)`.
pub fn player_continue_attack<H: Host>(
    h: &mut H,
    player: &H::Entity,
    target: &H::Entity,
    style: Option<CombatStyle>,
    message: bool,
) -> Result<bool> {
    if h.kind(target).is_npc() {
        let shown = h.shown_npc(target, player)?;
        let can_attack = h.has_action(&shown, "attack")? || h.has_action(&shown, "smash-ice")?;
        if !can_attack {
            let class = class(h, target)?;
            if !class.is_ignore_attack_restrictions(h, target, player)? {
                return Ok(false);
            }
        }
    }
    if h.kind(target).is_player() {
        if h.skull_wilderness(target)? && h.skull_wilderness(player)? {
            if !h.settings()? {
                return Ok(false);
            }
            if !h.settings()? {
                return Err(Error::Null);
            }
            if !h.wild_pvp_enabled()? {
                return Ok(false);
            }
            if h.wilderness_protection(target)? {
                return Ok(false);
            }
            return Ok(!h.wilderness_protection(player)?);
        }
        return h.zone_is_pvpable(player, target, player, style, message);
    }
    Ok(true)
}

/// `Entity.checkImpact(state)`, with the safespot rule for NPCs.
pub fn entity_check_impact<H: Host>(
    h: &mut H,
    entity: &H::Entity,
    state: &mut BattleState<H>,
) -> Result<()> {
    let now = h.ticks()?;
    h.set_last_received_attack(entity, now)?;
    let now = h.ticks()?;
    let ticks = now.wrapping_sub(h.last_sent_attack(entity)?);
    if ticks > 10 && h.kind(entity).is_npc() && h.config_bool(entity, "safespot", false)? {
        let attacker = state.entity.clone();
        let here = h.location(entity);
        h.path_find(attacker.as_ref(), here)?;
        h.path_walk(attacker.as_ref())?;
        let victim = state.victim.clone();
        let attacker = attacker.ok_or(Error::Null)?;
        let there = h.location(&attacker);
        h.path_find(victim.as_ref(), there)?;
        h.path_walk(victim.as_ref())?;
        if ticks > 40 {
            h.move_step(&attacker)?;
            h.combat_stop(&attacker)?;
            let victim = victim.ok_or(Error::Null)?;
            h.move_step(&victim)?;
            h.combat_stop(&victim)?;
        }
    }
    Ok(())
}

/// `NPC.checkImpact(state)`.
pub fn npc_check_impact<H: Host>(
    h: &mut H,
    npc: &H::Entity,
    state: &mut BattleState<H>,
) -> Result<()> {
    entity_check_impact(h, npc, state)?;
    let attacker = state.entity.clone();
    let behaviour = behaviour(h, npc)?;
    behaviour.before_damage_received(h, npc, attacker.as_ref().ok_or(Error::Null)?, state)?;
    let attacker = attacker.ok_or(Error::Null)?;
    let task = ai(h, npc)?.task;
    if let Some(task) = task
        && h.kind(&attacker).is_player()
        && task.level_req > h.level(&attacker, SLAYER)?
    {
        neutralize(state);
    }
    let task = ai(h, npc)?.task;
    if let Some(task) = task
        && h.kind(&attacker) == Kind::Familiar
    {
        let owner = h.familiar_owner(&attacker)?.ok_or(Error::Null)?;
        if task.level_req > h.level(&owner, SLAYER)? {
            neutralize(state);
        }
    }
    Ok(())
}

fn neutralize<H: Host>(state: &mut BattleState<H>) {
    state.neutralize_hits();
    state.poison_damage = -1;
    if state.style == Some(CombatStyle::Magic) {
        state.estimated_hit = -1;
    }
}

/// `entity.checkImpact(state)` by runtime class.
pub fn check_impact<H: Host>(
    h: &mut H,
    entity: &H::Entity,
    state: &mut BattleState<H>,
) -> Result<()> {
    if h.kind(entity).is_npc() {
        npc_check_impact(h, entity, state)
    } else {
        entity_check_impact(h, entity, state)
    }
}

/// `Entity.onImpact(entity, state)`: auto-retaliation for players and NPCs.
pub fn entity_on_impact<H: Host>(
    h: &mut H,
    this: &H::Entity,
    attacker: Option<&H::Entity>,
    state: &mut BattleState<H>,
) -> Result<()> {
    if h.is_dead(this)? {
        state.neutralize_hits();
    }
    if h.kind(this).is_npc() {
        let behaviour = behaviour(h, this)?;
        behaviour.after_damage_received(h, this, attacker.ok_or(Error::Null)?, state)?;
    }
    if h.is_retaliating(this)?
        && !h.is_attacking(this)?
        && !h.interaction_locked(this)?
        && h.next_attack(this)? < h.ticks()?
        && ((!h.walking_has_path(this)? && !h.pulse_manager_is_moving(this)?)
            || h.kind(this).is_npc())
    {
        h.combat_attack(this, attacker)?;
    }
    Ok(())
}

/// `Player.onImpact(entity, state)`: retaliation, then vengeance, the ring of
/// recoil and armour degradation.
pub fn player_on_impact<H: Host>(
    h: &mut H,
    player: &H::Entity,
    attacker: Option<&H::Entity>,
    state: &mut BattleState<H>,
) -> Result<()> {
    entity_on_impact(h, player, attacker, state)?;
    let recoil = h.ring_id(player)? == 2550;
    if state.estimated_hit > 0 {
        if h.attribute_bool(player, "vengeance", false)? {
            h.remove_attribute(player, "vengeance")?;
            let hit = (f64::from(state.estimated_hit) * 0.75) as i32;
            h.send_chat(player, "Taste vengeance!")?;
            if hit > -1 {
                let target = attacker.ok_or(Error::Null)?;
                h.manual_hit(target, player, hit, Hitsplat::Normal)?;
            }
        }
        if recoil {
            h.handle_recoil_effect(player, attacker, state.estimated_hit)?;
        }
    }
    if state.secondary_hit > 0 && recoil {
        h.handle_recoil_effect(player, attacker, state.secondary_hit)?;
    }
    h.check_armour_degrades(player)
}

/// `victim.onImpact(attacker, state)` by runtime class.
pub fn on_impact<H: Host>(
    h: &mut H,
    victim: &H::Entity,
    attacker: Option<&H::Entity>,
    state: &mut BattleState<H>,
) -> Result<()> {
    if h.kind(victim).is_player() {
        player_on_impact(h, victim, attacker, state)
    } else {
        entity_on_impact(h, victim, attacker, state)
    }
}

/// `Entity.sendImpact(state)`.
pub fn send_impact<H: Host>(h: &mut H, entity: &H::Entity) -> Result<()> {
    let now = h.ticks()?;
    h.set_last_sent_attack(entity, now)
}

/// `Entity.isIgnoreMultiBoundaries(victim)`.
pub fn is_ignore_multi_boundaries<H: Host>(
    h: &mut H,
    entity: &H::Entity,
    victim: &H::Entity,
) -> Result<bool> {
    if h.kind(entity).is_npc() {
        let behaviour = behaviour(h, entity)?;
        return behaviour.should_ignore_multi_restrictions(h, entity, victim);
    }
    Ok(false)
}

/// `NPC.getSwingHandler(swing)`: the combat pulse style's handler, offered
/// to the behaviour.
pub fn npc_swing_handler<H: Host>(h: &mut H, npc: &H::Entity) -> Result<H::Handler> {
    let style = h.combat_style(npc)?.ok_or(Error::Null)?;
    let original = h.style_handler(style);
    let behaviour = behaviour(h, npc)?;
    behaviour.swing_handler_override(h, npc, original)
}
