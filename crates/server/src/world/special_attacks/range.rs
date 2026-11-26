//! The special attacks that extend `RangeSwingHandler`.
use super::{
    Host, Priority, Special, SpecialProjectile, SpecialPulse, SpecialTask, SpecialTimer, TimerBody,
    at, audio, distance, drain_special, gfx, instance, is_attacking, player, random, show, skill,
    toggle_special_bar, within_distance,
};
use crate::world::combat_pulse::{ProjectileRequest, handler as base, impact};
use crate::world::combat_swing::{
    BattleState, CombatStyle, Error, Graphics, Handler, Result, Target, Weapon, WeaponType,
};

const RANGE: Option<CombatStyle> = Some(CombatStyle::Range);

/// The static templates `DescentOfDarknessSpecialHandler` copies:
/// `Projectile.create(null, null, id, 40, 36, 41, speed, angle, 11)`.
const DRAGON: [i32; 7] = [1099, 40, 36, 41, 46, 5, 11];
const DRAGON_1: [i32; 7] = [1099, 40, 36, 41, 55, 25, 11];
const DARKNESS: [i32; 7] = [1101, 40, 36, 41, 46, 5, 11];
const DARKNESS_1: [i32; 7] = [1101, 40, 36, 41, 55, 25, 11];

/// Chainhit's door supports.
const DOOR_SUPPORTS: [[i32; 3]; 3] = [[2545, 10145, 0], [2545, 10141, 0], [2543, 10143, 0]];

fn need<'a, H: Host>(state: &'a mut Option<&mut BattleState<H>>) -> Result<&'a mut BattleState<H>> {
    state.as_deref_mut().ok_or(Error::Null)
}

/// `RandomFunction.random(calculateHit(entity, victim, modifier) + 1)`.
fn roll<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    entity: Option<&H::Entity>,
    victim: Option<&H::Entity>,
    modifier: f64,
) -> Result<i32> {
    let bound = hit_of(h, r, entity, victim, modifier)?.wrapping_add(1);
    random(h, bound)
}

fn hit_of<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    entity: Option<&H::Entity>,
    victim: Option<&H::Entity>,
    modifier: f64,
) -> Result<i32> {
    instance(h, r).calculate_hit(h, entity, victim, modifier)
}

/// A copy of a battle state that a pulse captures; the original shares the
/// object, which neither holder changes afterwards.
pub(crate) fn copy_state<H: Host>(s: &BattleState<H>) -> BattleState<H> {
    BattleState {
        entity: s.entity.clone(),
        victim: s.victim.clone(),
        estimated_hit: s.estimated_hit,
        maximum_hit: s.maximum_hit,
        secondary_hit: s.secondary_hit,
        recoil_damage: s.recoil_damage,
        poison_damage: s.poison_damage,
        frozen: s.frozen,
        targets: s.targets.as_ref().map(|list| {
            list.iter()
                .map(|t| match t {
                    None => None,
                    Some(Target::This) => Some(Target::This),
                    Some(Target::State(nested)) => {
                        Some(Target::State(Box::new(copy_state(nested))))
                    }
                })
                .collect()
        }),
        weapon: s.weapon.as_ref().map(|w| Weapon {
            id: w.id,
            item: w.item.clone(),
            name: w.name.clone(),
            ammunition_slot: w.ammunition_slot,
            ammunition: w.ammunition.clone(),
            kind: w.kind,
        }),
        range_weapon: s.range_weapon.clone(),
        ammunition: s.ammunition.clone(),
        spell: s.spell.clone(),
        style: s.style,
        armour_effect: s.armour_effect,
    }
}

/// `configureRangeData(p, state)` and the ammunition check every range
/// special opens with: false when the special answers -1.
fn configured<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    p: &H::Entity,
    state: &mut BattleState<H>,
) -> Result<bool> {
    instance(h, r).configure_range_data(h, Some(p), state)?;
    if state.weapon.is_none() || !Handler::has_ammo(h, Some(p), state)? {
        h.stop_combat_pulse(p)?;
        toggle_special_bar(h, p)?;
        return Ok(false);
    }
    Ok(true)
}

/// `Companion.useAmmo(entity, state, victim.getLocation())`.
fn use_ammo<H: Host>(
    h: &mut H,
    entity: &H::Entity,
    victim: Option<&H::Entity>,
    state: &BattleState<H>,
) -> Result<()> {
    let location = h.location(victim.ok_or(Error::Null)?)?;
    base::use_ammo(h, entity, state, location)
}

/// `1 + (int) Math.ceil(entity.getLocation().getDistance(victim.getLocation()) * 0.3)`.
fn travel<H: Host>(h: &mut H, entity: &H::Entity, victim: Option<&H::Entity>) -> Result<i32> {
    let gap = distance(h, entity, victim.ok_or(Error::Null)?)?;
    Ok(1i32.wrapping_add((gap * 0.3).ceil() as i32))
}

/// The nine-argument `Projectile.create(...).send()`.
#[allow(clippy::too_many_arguments)]
fn create<H: Host>(
    h: &mut H,
    source: &H::Entity,
    victim: Option<&H::Entity>,
    id: i32,
    start_height: i32,
    end_height: i32,
    start_delay: i32,
    speed: i32,
    angle: i32,
) -> Result<()> {
    h.send_special_projectile(SpecialProjectile::Create {
        source,
        victim,
        id,
        start_height,
        end_height,
        start_delay,
        speed,
        angle,
        distance: 11,
    })
}

/// `swing(entity, victim, state)` of a range special.
pub fn swing<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    special: Special,
    entity: Option<&H::Entity>,
    victim: Option<&H::Entity>,
    mut state: Option<&mut BattleState<H>>,
) -> Result<i32> {
    match special {
        Special::Chainhit => {
            let e = entity.ok_or(Error::Null)?;
            if h.chain_victims(e)?.is_some() {
                let p = player(h, entity)?;
                h.send_message(p, "You're already using the chain-hit special attack.")?;
                toggle_special_bar(h, p)?;
                return Ok(-1);
            }
            let p = player(h, entity)?;
            if !drain_special(h, p, 10)? {
                return Ok(-1);
            }
            need(&mut state)?.style = RANGE;
            if let Some(v) = victim.filter(|v| h.kind(v).is_npc())
                && h.id(v)? == 2440
            {
                for l in DOOR_SUPPORTS {
                    let Some(n) = h.find_npc(l)? else { continue };
                    if h.is_dead(&n)? || h.id(&n)? != h.original_id(&n)? {
                        continue;
                    }
                    let speed = (32.0 + distance(h, v, &n)? * 5.0) as i32;
                    create(h, v, Some(&n), 258, 40, 36, 32, speed, 5)?;
                    h.heal(&n, 100)?;
                    h.submit_special(SpecialPulse::new(
                        3,
                        vec![],
                        SpecialTask::ChainDoor {
                            npc: n.clone(),
                            victim: v.clone(),
                        },
                    ))?;
                }
            }
            let mut made = instance(h, r);
            let delay = made.swing(h, entity, victim, need(&mut state)?);
            *h.handler(r) = made;
            delay
        }
        Special::DescentOfDarkness => descent(h, r, entity, victim, state),
        Special::Hamstring | Special::PhantomStrike => {
            let p = player(h, entity)?;
            let state = need(&mut state)?;
            if !configured(h, r, p, state)? {
                return Ok(-1);
            }
            if !drain_special(h, p, 50)? {
                return Ok(-1);
            }
            state.style = RANGE;
            let hamstring = special == Special::Hamstring;
            let max = hit_of(h, r, entity, victim, if hamstring { 1.2 } else { 1.0 })?;
            state.maximum_hit = max;
            let mut hit = 0;
            if instance(h, r).is_accurate_impact_typed(h, entity, victim)? {
                if hamstring {
                    let minimum = hit_of(h, r, entity, victim, 0.2)?;
                    let bound = hit_of(h, r, entity, victim, 1.0)?.wrapping_add(1);
                    hit = minimum.wrapping_add(random(h, bound)?);
                } else {
                    hit = random(h, max.wrapping_add(1))?;
                }
            }
            state.estimated_hit = hit;
            use_ammo(h, p, victim, state)?;
            let v = victim.ok_or(Error::Null)?;
            let timer = if hamstring {
                SpecialTimer {
                    run_interval: 100,
                    identifier: "hamstrung",
                    is_soft: true,
                    is_auto: false,
                    flags: &["ClearOnDeath"],
                    body: TimerBody::Hamstrung,
                }
            } else {
                SpecialTimer {
                    run_interval: 3,
                    identifier: "phantom-strike",
                    is_soft: true,
                    is_auto: false,
                    flags: &["ClearOnDeath"],
                    body: TimerBody::PhantomStrike {
                        attacker: p.clone(),
                        remaining_damage: hit,
                    },
                }
            };
            h.register_special_timer(v, timer)?;
            travel(h, p, victim)
        }
        Special::Powershot => {
            let p = player(h, entity)?;
            let state = need(&mut state)?;
            if !configured(h, r, p, state)? {
                return Ok(-1);
            }
            if !drain_special(h, p, 35)? {
                return Ok(-1);
            }
            state.style = RANGE;
            let hit = roll(h, r, entity, victim, 1.0)?;
            state.estimated_hit = hit;
            use_ammo(h, p, victim, state)?;
            travel(h, p, victim)
        }
        Special::Seercull => {
            let p = player(h, entity)?;
            let state = need(&mut state)?;
            if !configured(h, r, p, state)? {
                return Ok(-1);
            }
            if !drain_special(h, p, 100)? {
                return Ok(-1);
            }
            let hit = roll(h, r, entity, victim, 1.0)?;
            let v = victim.ok_or(Error::Null)?;
            if h.level(v, skill::MAGIC)? >= h.static_level(v, skill::MAGIC)? {
                h.update_level(v, skill::MAGIC, hit.wrapping_neg(), 0)?;
            }
            use_ammo(h, p, victim, state)?;
            state.estimated_hit = hit;
            Ok(1)
        }
        Special::Snapshot => {
            let p = player(h, entity)?;
            let state = need(&mut state)?;
            if !configured(h, r, p, state)? {
                return Ok(-1);
            }
            if !drain_special(h, p, 55)? {
                return Ok(-1);
            }
            state.style = RANGE;
            let max = hit_of(h, r, entity, victim, 1.0)?;
            state.maximum_hit = max;
            let melee = Some(CombatStyle::Melee);
            let mut hit = 0;
            if instance(h, r).is_accurate_impact(h, entity, victim, melee, 1.43, 1.0)? {
                hit = random(h, max.wrapping_add(1))?;
            }
            state.estimated_hit = hit;
            hit = 0;
            if instance(h, r).is_accurate_impact(h, entity, victim, melee, 1.43, 1.0)? {
                hit = random(h, max.wrapping_add(1))?;
            }
            state.secondary_hit = hit;
            use_ammo(h, p, victim, state)?;
            travel(h, p, victim)
        }
        Special::Snipe => {
            let p = player(h, entity)?;
            let state = need(&mut state)?;
            if !configured(h, r, p, state)? {
                return Ok(-1);
            }
            if !drain_special(h, p, 75)? {
                return Ok(-1);
            }
            state.style = RANGE;
            let mut hit = 0;
            let v = victim.ok_or(Error::Null)?;
            if !is_attacking(h, v)?
                || instance(h, r).is_accurate_impact(h, entity, victim, RANGE, 1.0, 1.0)?
            {
                hit = roll(h, r, entity, victim, 1.0)?;
                let fixed = h.static_level(v, skill::DEFENCE)?;
                let levels = h.dynamic_levels(v)?.ok_or(Error::Null)?;
                let current = *levels.get(skill::DEFENCE as usize).ok_or(Error::Index)?;
                if fixed >= current {
                    h.update_level(v, skill::DEFENCE, hit.wrapping_neg(), 0)?;
                }
            }
            use_ammo(h, p, victim, state)?;
            state.estimated_hit = hit;
            Ok(1)
        }
        _ => Err(Error::Binding(format!(
            "{special:?} is not a range special"
        ))),
    }
}

fn descent<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    entity: Option<&H::Entity>,
    victim: Option<&H::Entity>,
    state: Option<&mut BattleState<H>>,
) -> Result<i32> {
    let p = player(h, entity)?;
    let held = h.equipment_new(p, 3)?;
    let Some(rw) = h.range_weapon(h.item_id(&held)) else {
        return Ok(-1);
    };
    let item = h.equipment(p, 3)?;
    let ammunition = h.equipment_new(p, rw.ammunition_slot)?;
    let mut w = Weapon::with_ammunition(h, item, rw.ammunition_slot, Some(ammunition));
    w.kind = Some(rw.kind()?);
    let state = state.ok_or(Error::Null)?;
    state.style = RANGE;
    state.range_weapon = Some(rw.clone());
    let ammunition_id = w
        .ammunition
        .as_ref()
        .map(|a| h.item_id(a))
        .ok_or(Error::Null)?;
    state.ammunition = h.ammunition(ammunition_id);
    state.weapon = Some(w);
    if !Handler::has_ammo(h, Some(p), state)? {
        h.stop_combat_pulse(p)?;
        toggle_special_bar(h, p)?;
        return Ok(-1);
    }
    if !drain_special(h, p, 55)? {
        return Ok(-1);
    }
    let ammo_id = state.ammunition.as_ref().ok_or(Error::Null)?.item_id;
    state.frozen = h.definition_name(ammo_id)?.contains("ragon arrow");
    let modifier = if state.frozen { 1.5 } else { 1.3 };
    let minimum: i32 = if state.frozen { 8 } else { 5 };
    let max = hit_of(h, r, entity, victim, modifier)?;
    state.maximum_hit = max;
    let mut hit = minimum;
    if instance(h, r).is_accurate_impact(h, entity, victim, RANGE, 1.15, 1.0)? {
        hit = hit.wrapping_add(random(h, max.wrapping_sub(minimum).wrapping_add(1))?);
    }
    state.estimated_hit = hit;
    // `w.getType()`: the weapon object the state holds, which hasAmmo may
    // have turned from a double shot into a default one.
    if state.weapon.as_ref().and_then(|w| w.kind) == Some(WeaponType::DoubleShot) {
        hit = minimum;
        if instance(h, r).is_accurate_impact(h, entity, victim, RANGE, 1.15, 1.0)? {
            hit = hit.wrapping_add(random(h, max.wrapping_sub(minimum).wrapping_add(1))?);
        }
        state.secondary_hit = hit;
    }
    use_ammo(h, p, victim, state)?;
    travel(h, p, victim)
}

/// `visualize(entity, victim, state)` of a range special.
pub fn visualize<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    entity: &H::Entity,
    victim: Option<&H::Entity>,
    state: &BattleState<H>,
) -> Result<()> {
    let special = h.special_of(r).ok_or(Error::Null)?;
    match special {
        Special::Chainhit => {
            audio(h, entity, 2528)?;
            show(h, entity, 1068, Some(gfx(257, 96, 0)))?;
            let speed = (32.0 + distance(h, entity, victim.ok_or(Error::Null)?)? * 5.0) as i32;
            create(h, entity, victim, 258, 40, 36, 32, speed, 5)
        }
        Special::DescentOfDarkness => descent_visualize(h, entity, victim, state),
        Special::Powershot => {
            audio(h, entity, 2536)?;
            let rw = state.range_weapon.as_ref().ok_or(Error::Null)?;
            let animation = h.range_weapon_animation(rw.item_id);
            h.visualize_entity(entity, animation.as_ref(), Some(gfx(250, 96, 0)))?;
            let speed = (46.0 + distance(h, entity, victim.ok_or(Error::Null)?)? * 5.0) as i32;
            create(h, entity, victim, 249, 40, 36, 45, speed, 5)
        }
        Special::Seercull => {
            audio(h, entity, 2546)?;
            let v = victim.ok_or(Error::Null)?;
            h.graphics(v, gfx(474, 0, 0))?;
            let speed = (35.0 + distance(h, entity, v)? * 10.0) as i32;
            let animation = h.attack_animation(entity)?;
            h.visualize_entity(entity, animation.as_ref(), Some(gfx(472, 96, 0)))?;
            create(h, entity, victim, 473, 40, 40, 40, speed, 15)
        }
        Special::Snapshot => {
            audio(h, entity, 2545)?;
            show(h, entity, 1074, Some(gfx(249, 96, 0)))?;
            let v = victim.ok_or(Error::Null)?;
            let mut speed = (27.0 + distance(h, entity, v)? * 5.0) as i32;
            create(h, entity, victim, 249, 40, 36, 20, speed, 15)?;
            speed = (32.0 + distance(h, entity, v)? * 10.0) as i32;
            create(h, entity, victim, 249, 40, 36, 50, speed, 15)
        }
        Special::Snipe => {
            audio(h, entity, 1080)?;
            let animation = h.new_animation(4230, Priority::High);
            h.animate(entity, Some(&animation))?;
            h.send_special_projectile(SpecialProjectile::CreateShort {
                source: entity,
                victim,
                id: 698,
                start_height: 36,
                end_height: 25,
                start_delay: 35,
                speed: 72,
            })
        }
        other => Err(Error::Binding(format!("{other:?} has no range visualize"))),
    }
}

fn copy<H: Host>(
    h: &mut H,
    template: [i32; 7],
    source: &H::Entity,
    victim: Option<&H::Entity>,
    speed: f64,
) -> Result<()> {
    h.send_special_projectile(SpecialProjectile::Copy {
        template,
        source,
        victim,
        speed,
    })
}

fn descent_visualize<H: Host>(
    h: &mut H,
    entity: &H::Entity,
    victim: Option<&H::Entity>,
    state: &BattleState<H>,
) -> Result<()> {
    let mut start: Option<Graphics> = None;
    if let Some(ammunition) = state.ammunition.clone() {
        let visuals = h.ammunition_visuals(ammunition.item_id);
        start = visuals.start_graphics;
        if state.frozen {
            copy(h, DRAGON, entity, victim, 5.0)?;
        } else {
            if visuals.projectile_id.is_none() {
                return Err(Error::Null);
            }
            h.send_projectile(ProjectileRequest::AmmunitionCopy {
                ammunition_id: ammunition.item_id,
                source: entity,
                victim,
                speed: 5.0,
            })?;
            copy(h, DARKNESS, entity, victim, 5.0)?;
        }
        let weapon = state.weapon.as_ref().ok_or(Error::Null)?;
        if weapon.kind == Some(WeaponType::DoubleShot) && visuals.dark_bow_graphics.is_some() {
            start = visuals.dark_bow_graphics;
            if state.frozen {
                copy(h, DRAGON_1, entity, victim, 10.0)?;
            } else {
                let gap = distance(h, entity, victim.ok_or(Error::Null)?)?;
                let speed = (55.0 + gap * 10.0) as i32;
                let id = visuals.projectile_id.ok_or(Error::Null)?;
                h.send_projectile(ProjectileRequest::Create {
                    source: entity,
                    victim,
                    id,
                    start_height: 40,
                    end_height: 36,
                    start_delay: 41,
                    speed,
                    angle: 25,
                })?;
                copy(h, DARKNESS_1, entity, victim, 10.0)?;
            }
        }
    }
    let animation = h.attack_animation(entity)?;
    h.visualize_entity(entity, animation.as_ref(), start)
}

/// DescentOfDarkness's `visualizeImpact`.
pub fn descent_visualize_impact<H: Host>(
    h: &mut H,
    _r: &H::HandlerRef,
    _entity: Option<&H::Entity>,
    victim: Option<&H::Entity>,
    state: &mut BattleState<H>,
) -> Result<()> {
    let v = victim.ok_or(Error::Null)?;
    let here = at(h, v)?;
    h.play_global_audio_delayed(here, 3736, 20)?;
    let here = at(h, v)?;
    h.play_global_audio_delayed(here, 3737, 40)?;
    let animation = h.defence_animation(v)?;
    let graphics = if state.frozen {
        gfx(1100, 96, 0)
    } else {
        gfx(1103, 96, 0)
    };
    h.visualize_entity(v, animation.as_ref(), Some(graphics))
}

/// Chainhit's `visualizeImpact`: `handleHit`, then the range class's own.
pub fn chainhit_visualize_impact<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    entity: Option<&H::Entity>,
    victim: Option<&H::Entity>,
    state: &mut BattleState<H>,
) -> Result<()> {
    let p = player(h, entity)?;
    let e = entity.ok_or(Error::Null)?;
    let v = victim.ok_or(Error::Null)?;
    handle_hit(h, r, e, v, p, state)?;
    base::visualize_impact_super(h, r, entity, victim, state)
}

/// Chainhit's `getVictimsList(e, victim)`.
fn victims_list<H: Host>(h: &mut H, e: &H::Entity, victim: &H::Entity) -> Result<()> {
    if h.chain_victims(e)?.is_some() {
        return Ok(());
    }
    let here = at(h, e)?;
    let made = if h.kind(victim).is_npc() {
        h.local_npcs(here, 5)?
    } else {
        h.local_players(here, 5)?
    };
    h.set_chain_victims(e, made)?;
    for removed in [e, victim] {
        let list = h
            .chain_victims_mut(e)
            .map(|l| l.clone())
            .unwrap_or_default();
        if let Some(found) = list.iter().position(|x| h.same_entity(x, removed))
            && let Some(stored) = h.chain_victims_mut(e)
        {
            stored.remove(found);
        }
    }
    Ok(())
}

/// Chainhit's `handleHit(entity, victim, player, state)`.
pub fn handle_hit<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    entity: &H::Entity,
    victim: &H::Entity,
    player: &H::Entity,
    state: &BattleState<H>,
) -> Result<()> {
    h.submit_special(SpecialPulse::new(
        1,
        vec![Some(player.clone()), Some(victim.clone())],
        SpecialTask::ChainImpact {
            handler: r.clone(),
            entity: entity.clone(),
            victim: victim.clone(),
            player: player.clone(),
            state: Box::new(copy_state(state)),
        },
    ))?;
    if h.id(victim)? == 2440 || h.id(victim)? == 2446 || h.id(victim)? == 2443 {
        return Ok(());
    }
    if h.is_multi_zone(victim)? {
        let (settings, _) = h.settings(player);
        let energy = settings.fields.read().special_energy;
        if energy >= 10 {
            victims_list(h, player, victim)?;
            loop {
                let next = match h.chain_victims_mut(player) {
                    Some(list) if !list.is_empty() => list.remove(0),
                    _ => break,
                };
                let style = Some(CombatStyle::Range);
                if !h.entity_attackable(&next, player, style, false)? || !h.is_multi_zone(&next)? {
                    continue;
                }
                let gap = distance(h, victim, &next)?;
                let speed = (32.0 + gap * 5.0) as i32;
                create(h, victim, Some(&next), 258, 40, 36, 32, speed, 5)?;
                h.submit_special(SpecialPulse::new(
                    (gap / 3.0) as i32,
                    vec![
                        Some(entity.clone()),
                        Some(victim.clone()),
                        Some(next.clone()),
                    ],
                    SpecialTask::ChainBounce {
                        handler: r.clone(),
                        entity: entity.clone(),
                        victim: victim.clone(),
                        target: next,
                        player: player.clone(),
                    },
                ))?;
                let (settings, ctx) = h.settings(player);
                let value = settings.fields.read().special_energy.wrapping_sub(10);
                settings
                    .set_special_energy(ctx, value)
                    .map_err(super::settings_error)?;
                return Ok(());
            }
        }
    }
    h.remove_attribute(player, "chain-hit_v")
}

/// The body of the pulse that carries the chain to its next victim.
pub fn chainhit_bounce<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    entity: &H::Entity,
    victim: &H::Entity,
    target: &H::Entity,
    player: &H::Entity,
) -> Result<()> {
    let mut bs = BattleState::new(Some(entity.clone()), Some(target.clone()));
    bs.maximum_hit = hit_of(h, r, Some(player), Some(target), 1.0)?;
    bs.estimated_hit = random(h, bs.maximum_hit.wrapping_add(1))?;
    handle_hit(h, r, victim, target, player, &bs)?;
    base::visualize_impact_super(h, r, Some(player), Some(target), &mut bs)
}

/// Seercull's `impact`: one range hit.
pub fn seercull_impact<H: Host>(
    h: &mut H,
    _r: &H::HandlerRef,
    entity: Option<&H::Entity>,
    victim: Option<&H::Entity>,
    state: &mut BattleState<H>,
) -> Result<()> {
    let hit = state.estimated_hit;
    impact::handle_impact(
        h,
        victim.ok_or(Error::Null)?,
        entity,
        hit,
        RANGE,
        None,
        false,
    )?;
    Ok(())
}

/// Snapshot's `impact`: the second arrow lands at once within two tiles, or
/// a tick later.
pub fn snapshot_impact<H: Host>(
    h: &mut H,
    _r: &H::HandlerRef,
    entity: Option<&H::Entity>,
    victim: Option<&H::Entity>,
    state: &mut BattleState<H>,
) -> Result<()> {
    let v = victim.ok_or(Error::Null)?;
    let hit = state.estimated_hit;
    impact::handle_impact(h, v, entity, hit, RANGE, None, false)?;
    if state.secondary_hit > -1 {
        let second = state.secondary_hit;
        let here = at(h, v)?;
        let e = entity.ok_or(Error::Null)?;
        let there = at(h, e)?;
        if within_distance(here, there, 2) {
            impact::handle_impact(h, v, entity, second, RANGE, None, false)?;
            return Ok(());
        }
        h.submit_special(SpecialPulse::new(
            1,
            vec![Some(v.clone())],
            SpecialTask::SnapshotSecond {
                entity: e.clone(),
                victim: v.clone(),
                hit: second,
            },
        ))?;
    }
    Ok(())
}
