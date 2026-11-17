//! `ChinchompaSwingHandler`: a `RangeSwingHandler` subclass whose swing in
//! a multi-combat area hits up to ten targets around the victim with one
//! shared accuracy roll.
use super::Host;
use crate::world::combat_pulse::{ProjectileRequest, handler as base, skill};
use crate::world::combat_swing::{
    self as cs, BattleState, CombatStyle, Error, Graphics, Result, STYLE_LONG_RANGE, Target, Weapon,
};
use crate::world::special_attacks::multihit;

/// `END_GRAPHIC`: `new Graphics(157, 96)`.
pub const END_GRAPHIC: Graphics = Graphics {
    id: 157,
    height: 96,
    delay: 0,
};

fn need<'a, H: Host>(state: &'a mut Option<&mut BattleState<H>>) -> Result<&'a mut BattleState<H>> {
    state.as_deref_mut().ok_or(Error::Null)
}

/// The `swing` override with the pulse's state.
pub fn swing<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    entity: Option<&H::Entity>,
    victim: Option<&H::Entity>,
    state: &mut BattleState<H>,
) -> Result<i32> {
    swing_with(h, r, entity, victim, Some(state))
}

/// `swing(entity, victim, state)`: outside multi combat the range swing with
/// the state as its only target; inside, every target the chinchompa
/// finder answers gets its own state with a hit of `random(calculateHit + 1)`
/// when the one accuracy roll succeeded, the ammunition is used once and the
/// delay is `2 + ceil(distance * 0.3)`.
pub fn swing_with<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    entity: Option<&H::Entity>,
    victim: Option<&H::Entity>,
    mut state: Option<&mut BattleState<H>>,
) -> Result<i32> {
    let e = entity.ok_or(Error::Null)?;
    let multi = h.is_multi_zone(e)? && h.is_multi_zone(victim.ok_or(Error::Null)?)?;
    need(&mut state)?.style = Some(CombatStyle::Range);
    if !multi {
        let mut made = h.handler(r).clone();
        let ticks = made.swing(h, entity, victim, need(&mut state)?);
        *h.handler(r) = made;
        let ticks = ticks?;
        need(&mut state)?.targets = Some(vec![Some(Target::This)]);
        return Ok(ticks);
    }
    if !h.kind(e).is_player() {
        return Err(Error::Binding(
            "ClassCastException: (Player) entity of a non-player".into(),
        ));
    }
    let p = e;
    let held = h.equipment_new(p, cs::slot::WEAPON)?;
    let Some(rw) = h.range_weapon(h.item_id(&held)) else {
        return Ok(-1);
    };
    let item = h.equipment(p, cs::slot::WEAPON)?;
    let ammunition = h.equipment_new(p, rw.ammunition_slot)?;
    let mut w = Weapon::with_ammunition(h, item, rw.ammunition_slot, Some(ammunition));
    w.kind = Some(rw.kind()?);
    let state = need(&mut state)?;
    state.range_weapon = Some(rw);
    let ammunition_id = w
        .ammunition
        .as_ref()
        .map(|a| h.item_id(a))
        .ok_or(Error::Null)?;
    state.ammunition = h.ammunition(ammunition_id);
    state.weapon = Some(w);
    if !cs::Handler::has_ammo(h, entity, state)? {
        h.stop_combat_pulse(e)?;
        return Ok(-1);
    }
    let accurate =
        base::is_accurate_impact(h, r, entity, victim, Some(CombatStyle::Range), 1.0, 1.0)?;
    let v = victim.ok_or(Error::Null)?;
    let candidates = multihit::find_multihit_targets_for_chinchompa(h, v, e)?;
    let mut targets = Vec::with_capacity(candidates.len());
    for target in candidates {
        let mut s = BattleState::new(Some(e.clone()), Some(target.clone()));
        s.style = Some(CombatStyle::Range);
        let mut hit = 0;
        if accurate {
            let max = base::calculate_hit(h, r, entity, Some(&target), 1.0)?;
            hit = super::random(h, max.wrapping_add(1))?;
        }
        s.estimated_hit = hit;
        targets.push(Some(Target::State(Box::new(s))));
    }
    state.targets = Some(targets);
    base::use_ammo(h, e, state, None)?;
    let here = h.location(e)?.ok_or(Error::Null)?;
    let there = h.location(v)?.ok_or(Error::Null)?;
    let gap = cs::distance(here, there);
    Ok(2i32.wrapping_add((gap * 0.3).ceil() as i32))
}

/// The targets of a state: `getTargets()` failing on null.
fn target_count<H: Host>(state: &BattleState<H>) -> Result<usize> {
    Ok(state.targets.as_ref().ok_or(Error::Null)?.len())
}

/// What one element of the targets array is.
enum Element {
    Null,
    This,
    Nested,
}

fn element<H: Host>(state: &BattleState<H>, at: usize) -> Element {
    match state.targets.as_ref().and_then(|t| t.get(at)) {
        Some(Some(Target::This)) => Element::This,
        Some(Some(Target::State(_))) => Element::Nested,
        _ => Element::Null,
    }
}

/// Runs `body` on one nested state taken out of the array and puts it back.
fn with_nested<H: Host, T>(
    h: &mut H,
    state: &mut BattleState<H>,
    at: usize,
    body: impl FnOnce(&mut H, &mut BattleState<H>) -> Result<T>,
) -> Result<T> {
    let slot = state
        .targets
        .as_mut()
        .and_then(|t| t.get_mut(at))
        .ok_or(Error::Index)?;
    let Some(Target::State(mut nested)) = slot.take() else {
        return Err(Error::Null);
    };
    let outcome = body(h, &mut nested);
    if let Some(entry) = state.targets.as_mut().and_then(|t| t.get_mut(at)) {
        *entry = Some(Target::State(nested));
    }
    outcome
}

/// `adjustBattleState`: the range adjustment of every target's own state
/// against its own victim; the state itself only when it is its own target.
pub fn adjust_battle_state<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    entity: &H::Entity,
    _victim: &H::Entity,
    state: &mut BattleState<H>,
) -> Result<()> {
    for at in 0..target_count(state)? {
        match element(state, at) {
            Element::Null => {}
            Element::This => {
                let v = state.victim.clone().ok_or(Error::Null)?;
                base::adjust_battle_state_super(h, r, entity, &v, state)?;
            }
            Element::Nested => with_nested(h, state, at, |h, s| {
                let v = s.victim.clone().ok_or(Error::Null)?;
                base::adjust_battle_state_super(h, r, entity, &v, s)
            })?,
        }
    }
    Ok(())
}

/// `addExperience`: the targets' positive hits, 1.33 hitpoints experience
/// each, then range and defence (long range) or range alone.
pub fn add_experience<H: Host>(
    h: &mut H,
    _: &H::HandlerRef,
    entity: Option<&H::Entity>,
    _victim: Option<&H::Entity>,
    state: Option<&BattleState<H>>,
) -> Result<()> {
    let state = state.ok_or(Error::Null)?;
    let mut hit: i32 = 0;
    for target in state.targets.as_ref().ok_or(Error::Null)? {
        let estimated = match target {
            None => continue,
            Some(Target::This) => state.estimated_hit,
            Some(Target::State(s)) => s.estimated_hit,
        };
        if estimated > 0 {
            hit = hit.wrapping_add(estimated);
        }
    }
    let e = entity.ok_or(Error::Null)?;
    h.add_experience(e, skill::HITPOINTS, f64::from(hit) * 1.33, true)?;
    if h.attack_style(e)?.ok_or(Error::Null)?.style == STYLE_LONG_RANGE {
        h.add_experience(e, skill::RANGE, f64::from(hit.wrapping_mul(2)), true)?;
        h.add_experience(e, skill::DEFENCE, f64::from(hit.wrapping_mul(2)), true)?;
    } else {
        h.add_experience(e, skill::RANGE, f64::from(hit.wrapping_mul(4)), true)?;
    }
    Ok(())
}

/// `visualize`: the ammunition's projectile at speed 5 and its start
/// graphic, with the range weapon's animation for the default attack
/// animations 422 and 423.
pub fn visualize<H: Host>(
    h: &mut H,
    _: &H::HandlerRef,
    entity: &H::Entity,
    victim: Option<&H::Entity>,
    state: &BattleState<H>,
) -> Result<()> {
    let mut start = None;
    if let Some(ammunition) = state.ammunition.clone() {
        let visuals = h.ammunition_visuals(ammunition.item_id);
        if visuals.projectile_id.is_some() {
            start = visuals.start_graphics;
            if let Some(v) = victim {
                h.send_projectile(ProjectileRequest::AmmunitionCopy {
                    ammunition_id: ammunition.item_id,
                    source: entity,
                    victim: Some(v),
                    speed: 5.0,
                })?;
            }
        }
    }
    let current = h.attack_animation(entity)?.ok_or(Error::Null)?;
    let anim = h.animation_id(&current);
    if anim == 422 || anim == 423 {
        let id = state.weapon.as_ref().ok_or(Error::Null)?.id;
        if id > 0 && h.range_weapon(id).is_some() {
            let animation = h.range_weapon_animation(id);
            return h.visualize_entity(entity, animation.as_ref(), start);
        }
    }
    let animation = h.attack_animation(entity)?;
    h.visualize_entity(entity, animation.as_ref(), start)
}

/// `impact`: the range impact of every target's own state on its victim.
pub fn impact<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    entity: Option<&H::Entity>,
    _victim: Option<&H::Entity>,
    state: &mut BattleState<H>,
) -> Result<()> {
    for at in 0..target_count(state)? {
        match element(state, at) {
            Element::Null => {}
            Element::This => {
                let v = state.victim.clone();
                base::impact_super(h, r, entity, v.as_ref(), state)?;
            }
            Element::Nested => with_nested(h, state, at, |h, s| {
                let v = s.victim.clone();
                base::impact_super(h, r, entity, v.as_ref(), s)
            })?,
        }
    }
    Ok(())
}

/// `visualizeImpact`: the explosion on the victim, then every target's
/// victim plays its defence animation.
pub fn visualize_impact<H: Host>(
    h: &mut H,
    _: &H::HandlerRef,
    _entity: Option<&H::Entity>,
    victim: Option<&H::Entity>,
    state: &mut BattleState<H>,
) -> Result<()> {
    h.graphics(victim.ok_or(Error::Null)?, END_GRAPHIC)?;
    for at in 0..target_count(state)? {
        let v = match state.targets.as_ref().and_then(|t| t.get(at)) {
            None | Some(None) => continue,
            Some(Some(Target::This)) => state.victim.clone(),
            Some(Some(Target::State(s))) => s.victim.clone(),
        };
        let v = v.ok_or(Error::Null)?;
        let animation = h.defence_animation(&v)?;
        h.animate(&v, animation.as_ref())?;
    }
    Ok(())
}
