//! The special attacks that extend `MeleeSwingHandler`.
use super::{
    Host, Priority, Rights, Special, TimerArg, at, audio, drain_special, gfx, instance,
    is_attacking, multihit, player, random, random_between, show, skill, toggle_special_bar,
    transform,
};
use crate::world::combat_pulse::{handler as base, impact};
use crate::world::combat_swing::{BattleState, CombatStyle, Error, Result, Target, Weapon};
use rs09_game_core::direction::Direction;

const MELEE: Option<CombatStyle> = Some(CombatStyle::Melee);

/// The state parameter, which only the combat tab's instant special leaves null.
fn need<'a, H: Host>(state: &'a mut Option<&mut BattleState<H>>) -> Result<&'a mut BattleState<H>> {
    state.as_deref_mut().ok_or(Error::Null)
}

/// `isAccurateImpact(entity, victim, style, accuracyMod, defenceMod)`.
fn accurate<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    entity: Option<&H::Entity>,
    victim: Option<&H::Entity>,
    style: Option<CombatStyle>,
    accuracy: f64,
    defence: f64,
) -> Result<bool> {
    instance(h, r).is_accurate_impact(h, entity, victim, style, accuracy, defence)
}

/// `isAccurateImpact(entity, victim)`: the instance's `type`.
fn accurate_typed<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    entity: Option<&H::Entity>,
    victim: Option<&H::Entity>,
) -> Result<bool> {
    instance(h, r).is_accurate_impact_typed(h, entity, victim)
}

/// `calculateHit(entity, victim, modifier)`.
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

/// `super.swing(entity, victim, state)`: the melee class's own swing.
fn super_swing<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    entity: Option<&H::Entity>,
    victim: Option<&H::Entity>,
    state: &mut BattleState<H>,
) -> Result<i32> {
    let mut made = instance(h, r);
    let delay = made.swing(h, entity, victim, state);
    *h.handler(r) = made;
    delay
}

/// The shape most melee specials share: drain, the melee style, one accuracy
/// roll with the given modifiers and `random(calculateHit(modifier) + 1)`.
#[allow(clippy::too_many_arguments)]
fn plain<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    entity: Option<&H::Entity>,
    victim: Option<&H::Entity>,
    mut state: Option<&mut BattleState<H>>,
    energy: i32,
    accuracy: (f64, f64),
    modifier: f64,
) -> Result<i32> {
    let p = player(h, entity)?;
    if !drain_special(h, p, energy)? {
        return Ok(-1);
    }
    need(&mut state)?.style = MELEE;
    let mut hit = 0;
    if accurate(h, r, entity, victim, MELEE, accuracy.0, accuracy.1)? {
        hit = roll(h, r, entity, victim, modifier)?;
    }
    need(&mut state)?.estimated_hit = hit;
    Ok(1)
}

/// `skills.updateLevel(slot, amount, maximum)`.
fn update<H: Host>(h: &mut H, e: &H::Entity, slot: i32, amount: i32, maximum: i32) -> Result<i32> {
    h.update_level(e, slot, amount, maximum)
}

/// `swing(entity, victim, state)` of a melee special.
pub fn swing<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    special: Special,
    entity: Option<&H::Entity>,
    victim: Option<&H::Entity>,
    mut state: Option<&mut BattleState<H>>,
) -> Result<i32> {
    match special {
        Special::AncientMace => {
            let p = player(h, entity)?;
            if !drain_special(h, p, 100)? {
                return Ok(-1);
            }
            need(&mut state)?.style = MELEE;
            let mut hit = 0;
            if accurate(h, r, entity, victim, MELEE, 1.1, 0.98)? {
                hit = roll(h, r, entity, victim, 1.0)?;
                let points = h.prayer_points(p)?;
                if points < f64::from(h.static_level(p, skill::PRAYER)?) {
                    let points = h.prayer_points(p)?;
                    h.set_prayer_points(p, points + f64::from(hit))?;
                }
                let v = victim.ok_or(Error::Null)?;
                h.decrement_prayer_points(v, f64::from(hit))?;
            }
            need(&mut state)?.estimated_hit = hit;
            Ok(1)
        }
        Special::Backstab => {
            let p = player(h, entity)?;
            if !drain_special(h, p, 75)? {
                return Ok(-1);
            }
            need(&mut state)?.style = MELEE;
            let mut hit = 0;
            let v = victim.ok_or(Error::Null)?;
            if !is_attacking(h, v)? || accurate(h, r, entity, victim, MELEE, 1.0, 1.0)? {
                hit = roll(h, r, entity, victim, 1.0)?;
                let fixed = h.static_level(v, skill::DEFENCE)?;
                let levels = h.dynamic_levels(v)?.ok_or(Error::Null)?;
                let current = *levels.get(skill::DEFENCE as usize).ok_or(Error::Index)?;
                if fixed >= current {
                    update(h, v, skill::DEFENCE, hit.wrapping_neg(), 0)?;
                }
            }
            need(&mut state)?.estimated_hit = hit;
            Ok(1)
        }
        Special::Cleave => plain(h, r, entity, victim, state, 25, (1.0, 1.0), 1.25),
        Special::Clobber => {
            if h.osrs_dragon_axe() {
                clobber_osrs(h, r, entity)
            } else {
                clobber_authentic(h, r, entity, victim, state)
            }
        }
        Special::EnergyDrain => {
            let p = player(h, entity)?;
            if !drain_special(h, p, 50)? {
                return Ok(-1);
            }
            need(&mut state)?.style = MELEE;
            let mut hit = 0;
            if accurate(h, r, entity, victim, MELEE, 1.25, 1.0)? {
                hit = roll(h, r, entity, victim, 1.0)?;
            }
            if let Some(v) = victim.filter(|v| h.kind(v).is_player()) {
                h.update_run_energy(v, 10.0)?;
                h.update_run_energy(p, -10.0)?;
            }
            need(&mut state)?.estimated_hit = hit;
            Ok(1)
        }
        Special::Excalibur => {
            let p = player(h, entity)?;
            if !drain_special(h, p, 100)? {
                return Ok(-1);
            }
            h.send_chat(p, "For Camelot!")?;
            h.play_audio(p, 2539, 0)?;
            show(h, p, 1057, Some(gfx(247, 0, 0)))?;
            let weapon = h.equipment(p, 3)?.ok_or(Error::Null)?;
            match h.item_id(&weapon) {
                35 => {
                    let maximum = h.static_level(p, skill::DEFENCE)?.wrapping_add(8);
                    update(h, p, skill::DEFENCE, 8, maximum)?;
                }
                14632 => {
                    let args = [TimerArg::Int(3), TimerArg::Int(20), TimerArg::Int(4)];
                    let timer = h.spawn_timer("healovertime", &args)?;
                    if let Some(timer) = timer {
                        h.register_timer(p, timer)?;
                    }
                    let amount = (f64::from(h.static_level(p, skill::DEFENCE)?) * 0.15) as i32;
                    let maximum = (f64::from(h.static_level(p, skill::DEFENCE)?) * 1.15) as i32;
                    update(h, p, skill::DEFENCE, amount, maximum)?;
                }
                _ => {}
            }
            Ok(-1)
        }
        Special::Feint => {
            let p = player(h, entity)?;
            if !drain_special(h, p, 25)? {
                return Ok(-1);
            }
            need(&mut state)?.style = MELEE;
            let mut hit = 0;
            if accurate(h, r, entity, victim, MELEE, 1.0, 0.25)? {
                let minimum = hit_of(h, r, entity, victim, 0.2)?;
                let bound = hit_of(h, r, entity, victim, 1.0)?.wrapping_add(1);
                hit = minimum.wrapping_add(random(h, bound)?);
            }
            need(&mut state)?.estimated_hit = hit;
            Ok(1)
        }
        Special::HealingBlade => {
            let p = player(h, entity)?;
            if !drain_special(h, p, 50)? {
                return Ok(-1);
            }
            need(&mut state)?.style = MELEE;
            let mut hit = 0;
            if accurate(h, r, entity, victim, MELEE, 2.0, 1.0)? {
                hit = roll(h, r, entity, victim, 1.1)?;
                let mut health = hit / 2;
                let mut prayer = f64::from(hit) * 0.25;
                if health < 10 {
                    health = 10;
                }
                if prayer < 5.0 {
                    prayer = 5.0;
                }
                h.heal(p, health)?;
                h.increment_prayer_points(p, prayer)?;
            }
            need(&mut state)?.estimated_hit = hit;
            Ok(1)
        }
        Special::IceCleave => plain(h, r, entity, victim, state, 60, (2.0, 1.0), 1.1),
        Special::Impale => plain(h, r, entity, victim, state, 25, (1.1, 0.98), 1.1),
        Special::Judgement => {
            let p = player(h, entity)?;
            if !drain_special(h, p, 50)? {
                return Ok(-1);
            }
            need(&mut state)?.style = MELEE;
            let mut hit = 0;
            if accurate(h, r, entity, victim, MELEE, 2.0, 1.0)? {
                let max = (f64::from(hit_of(h, r, entity, victim, 1.1)?) * 1.25) as i32;
                hit = random(h, max.wrapping_add(1))?;
            }
            need(&mut state)?.estimated_hit = hit;
            Ok(1)
        }
        Special::Liquefy => {
            if let Some(e) = entity {
                let p = player(h, Some(e))?;
                h.player_send_message(p, "You need to be underwater to use this special attack.")?;
                let p = player(h, Some(e))?;
                let (settings, ctx) = h.settings(p);
                settings
                    .set_special_toggled(ctx, false)
                    .map_err(super::settings_error)?;
            }
            super_swing(h, r, entity, victim, need(&mut state)?)
        }
        Special::Powerstab | Special::SpearWall => {
            let p = player(h, entity)?;
            let energy = if special == Special::Powerstab {
                60
            } else {
                50
            };
            if !drain_special(h, p, energy)? {
                return Ok(-1);
            }
            let e = entity.ok_or(Error::Null)?;
            if !h.is_multi_zone(e)? {
                return super_swing(h, r, entity, victim, need(&mut state)?);
            }
            let v = victim.ok_or(Error::Null)?;
            let list = if special == Special::Powerstab {
                multihit::find_multihit_targets_for_d2h(h, v, e, CombatStyle::Melee)?
            } else {
                multihit::find_multihit_targets(h, v, e, CombatStyle::Melee)?
            };
            let mut targets = Vec::with_capacity(list.len());
            for target in list {
                let mut s = BattleState::new(Some(e.clone()), Some(target.clone()));
                let mut hit = 0;
                if special == Special::Powerstab {
                    if accurate_typed(h, r, entity, Some(&target))? {
                        let bound = hit_of(h, r, entity, Some(&target), 1.0)?.wrapping_add(1);
                        hit = random(h, bound)?;
                    }
                } else {
                    let bound = hit_of(h, r, entity, Some(&target), 1.0)?.wrapping_add(1);
                    hit = random(h, bound)?;
                }
                s.style = MELEE;
                s.estimated_hit = hit;
                targets.push(Some(Target::State(Box::new(s))));
            }
            need(&mut state)?.targets = Some(targets);
            Ok(1)
        }
        Special::Puncture => {
            let p = player(h, entity)?;
            if !drain_special(h, p, 25)? {
                return Ok(-1);
            }
            need(&mut state)?.style = MELEE;
            let mut hit = 0;
            if accurate(h, r, entity, victim, MELEE, 1.15, 1.0)? {
                hit = roll(h, r, entity, victim, 1.15)?;
            }
            need(&mut state)?.estimated_hit = hit;
            if accurate(h, r, entity, victim, MELEE, 1.15, 1.0)? {
                hit = roll(h, r, entity, victim, 1.15)?;
            } else {
                hit = 0;
            }
            need(&mut state)?.secondary_hit = hit;
            Ok(1)
        }
        Special::QuickSmash => quick_smash(h, r, entity, victim),
        Special::Rampage => {
            let p = player(h, entity)?;
            if !drain_special(h, p, 100)? {
                return Ok(-1);
            }
            h.send_chat(p, "Raarrrrrgggggghhhhhhh!")?;
            h.play_audio(p, 2538, 0)?;
            show(h, p, 1056, Some(gfx(246, 0, 0)))?;
            let mut boost: i32 = 0;
            for i in 0..7 {
                if i == skill::ATTACK
                    || i == skill::DEFENCE
                    || i == skill::RANGE
                    || i == skill::MAGIC
                {
                    let drain = (f64::from(h.level(p, i)?) * 0.1) as i32;
                    boost = boost.wrapping_add(drain);
                    update(h, p, i, drain.wrapping_neg(), 0)?;
                }
            }
            boost = 10i32.wrapping_add(boost / 4);
            let raised = h.static_level(p, skill::STRENGTH)?.wrapping_add(boost);
            let current = h.level(p, skill::STRENGTH)?;
            update(h, p, skill::STRENGTH, boost, raised.max(current))?;
            Ok(-1)
        }
        Special::SaradominsLightning => {
            let p = player(h, entity)?;
            if !drain_special(h, p, 100)? {
                return Ok(-1);
            }
            need(&mut state)?.style = Some(CombatStyle::Magic);
            let mut hit = 0;
            let mut secondary = 0;
            if accurate(h, r, entity, victim, MELEE, 1.10, 1.0)? {
                hit = roll(h, r, entity, victim, 1.1)?;
                secondary = 1i32.wrapping_add(random(h, 16)?);
            }
            let s = need(&mut state)?;
            s.estimated_hit = hit;
            s.secondary_hit = secondary;
            Ok(1)
        }
        Special::Sever => {
            let p = player(h, entity)?;
            if !drain_special(h, p, 55)? {
                return Ok(-1);
            }
            need(&mut state)?.style = MELEE;
            let mut hit = 0;
            if accurate(h, r, entity, victim, MELEE, 1.25, 1.0)? {
                hit = roll(h, r, entity, victim, 1.0)?;
                if let Some(v) = victim.filter(|v| h.kind(v).is_player()) {
                    for prayer in [
                        "PROTECT_FROM_MAGIC",
                        "PROTECT_FROM_MELEE",
                        "PROTECT_FROM_MISSILES",
                        "PROTECT_FROM_SUMMONING",
                    ] {
                        if h.prayer_on(v, prayer)? {
                            h.prayer_toggle(v, prayer)?;
                        }
                    }
                }
            }
            need(&mut state)?.estimated_hit = hit;
            Ok(1)
        }
        Special::Shatter => plain(h, r, entity, victim, state, 25, (1.25, 1.0), 1.5),
        Special::Shove => shove(h, entity, victim, need(&mut state)),
        Special::SliceAndDice => slice_and_dice(h, r, entity, victim, state),
        Special::Smash => {
            let p = player(h, entity)?;
            if !drain_special(h, p, 35)? {
                return Ok(-1);
            }
            need(&mut state)?.style = MELEE;
            let mut hit = 0;
            if accurate(h, r, entity, victim, MELEE, 1.0, 1.0)? {
                let max = hit_of(h, r, entity, victim, 1.0)?;
                hit = (max / 4).wrapping_add(random(h, max.wrapping_add(1))?);
                let v = victim.ok_or(Error::Null)?;
                let lower = (f64::from(h.level(v, skill::DEFENCE)?) * 0.30) as i32;
                update(h, v, skill::DEFENCE, lower.wrapping_neg(), 0)?;
            }
            need(&mut state)?.estimated_hit = hit;
            Ok(1)
        }
        Special::Sweep => sweep(h, r, entity, victim, state),
        Special::Warstrike => warstrike(h, r, entity, victim, need(&mut state)),
        Special::Weaken => {
            let p = player(h, entity)?;
            if !drain_special(h, p, 50)? {
                return Ok(-1);
            }
            need(&mut state)?.style = MELEE;
            let mut hit = 0;
            if accurate(h, r, entity, victim, MELEE, 1.0, 1.0)? {
                hit = roll(h, r, entity, victim, 1.0)?;
                let v = victim.ok_or(Error::Null)?;
                if h.kind(v).is_player() {
                    h.send_message(v, "You have been drained.")?;
                }
                for slot in [skill::DEFENCE, skill::ATTACK, skill::STRENGTH] {
                    let lower =
                        ((f64::from(h.static_level(v, slot)?) * 0.05) as i32).wrapping_add(1);
                    update(h, v, slot, lower.wrapping_neg(), 0)?;
                }
            }
            need(&mut state)?.estimated_hit = hit;
            Ok(hit)
        }
        _ => Err(Error::Binding(format!(
            "{special:?} is not a melee special"
        ))),
    }
}

/// Clobber's `swingAuthentic`: Kotlin's safe casts answer -1 instead of failing.
fn clobber_authentic<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    entity: Option<&H::Entity>,
    victim: Option<&H::Entity>,
    state: Option<&mut BattleState<H>>,
) -> Result<i32> {
    let Some(p) = entity.filter(|e| h.kind(e).is_player()) else {
        return Ok(-1);
    };
    let Some(v) = victim else { return Ok(-1) };
    let Some(state) = state else { return Ok(-1) };
    if !drain_special(h, p, 100)? {
        return Ok(-1);
    }
    state.style = MELEE;
    let item = h.equipment(p, 3)?;
    state.weapon = Some(Weapon::new(h, item));
    let mut hit = 0;
    if accurate(h, r, Some(p), Some(v), MELEE, 1.0, 1.0)? {
        let max = hit_of(h, r, Some(p), Some(v), 1.0)?;
        state.maximum_hit = max;
        hit = random(h, max.wrapping_add(1))?;
    }
    state.estimated_hit = hit;
    update(h, v, skill::DEFENCE, hit.wrapping_neg(), 0)?;
    update(h, v, skill::MAGIC, hit.wrapping_neg(), 0)?;
    h.play_audio(p, 2531, 20)?;
    Ok(1)
}

/// Clobber's `swingOSRS`.
fn clobber_osrs<H: Host>(h: &mut H, r: &H::HandlerRef, entity: Option<&H::Entity>) -> Result<i32> {
    let Some(p) = entity.filter(|e| h.kind(e).is_player()) else {
        return Ok(-1);
    };
    if !drain_special(h, p, 100)? {
        return Ok(-1);
    }
    h.send_chat(p, "Chop chop!")?;
    let maximum = h.static_level(p, skill::WOODCUTTING)?.wrapping_add(3);
    update(h, p, skill::WOODCUTTING, 3, maximum)?;
    clobber_visualize(h, p)?;
    let _ = r;
    h.play_audio(p, 2531, 20)?;
    Ok(-1)
}

fn clobber_visualize<H: Host>(h: &mut H, entity: &H::Entity) -> Result<()> {
    show(h, entity, 2876, Some(gfx(479, 96, 0)))
}

fn quick_smash<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    entity: Option<&H::Entity>,
    victim: Option<&H::Entity>,
) -> Result<i32> {
    let p = player(h, entity)?;
    let mut can_melee = base::can_melee(h, p, victim, 1)?;
    let victim = match victim {
        None => {
            let last = h.combat_pulse(p).last_victim.clone();
            let refused = match &last {
                None => true,
                Some(_) => {
                    let now = h.ticks()?;
                    now.wrapping_sub(h.combat_stop(p)?) > 2 || !can_melee
                }
            };
            if refused {
                h.send_message(
                    p,
                    "Warning: Since the maul's special is an instant attack, it will be wasted when used ",
                )?;
                h.send_message(p, "on a first strike.")?;
                return Ok(-1);
            }
            last.ok_or(Error::Null)?
        }
        Some(v) => {
            can_melee = can_melee && h.continue_attack(p, v, CombatStyle::Melee, true)?;
            v.clone()
        }
    };
    if !can_melee {
        return Ok(-1);
    }
    if h.is_dead(&victim)? {
        return Ok(-1);
    }
    if !drain_special(h, p, 50)? {
        return Ok(-1);
    }
    quick_smash_visualize(h, p, Some(&victim))?;
    let mut hit = 0;
    if accurate_typed(h, r, Some(p), Some(&victim))? {
        hit = roll(h, r, Some(p), Some(&victim), 1.0)?;
    }
    if h.has_protection_prayer(&victim, CombatStyle::Melee)? {
        let factor = if h.kind(&victim).is_player() {
            0.6
        } else {
            0.0
        };
        hit = (f64::from(hit) * factor) as i32;
    }
    let mut b = BattleState::default();
    let made = impact::handle_impact(h, &victim, Some(p), hit, MELEE, None, false)?;
    b.estimated_hit = made.ok_or(Error::Null)?.amount;
    base::add_experience(h, r, Some(p), Some(&victim), Some(&b))?;
    Ok(1)
}

fn quick_smash_visualize<H: Host>(
    h: &mut H,
    entity: &H::Entity,
    victim: Option<&H::Entity>,
) -> Result<()> {
    audio(h, entity, 2715)?;
    show(h, entity, 1667, Some(gfx(340, 96, 0)))?;
    let v = victim.ok_or(Error::Null)?;
    let animation = h.defence_animation(v)?;
    h.animate(v, animation.as_ref())
}

fn shove<H: Host>(
    h: &mut H,
    entity: Option<&H::Entity>,
    victim: Option<&H::Entity>,
    state: Result<&mut BattleState<H>>,
) -> Result<i32> {
    let v = victim.ok_or(Error::Null)?;
    if h.size(v)? > 1 {
        let p = player(h, entity)?;
        h.send_message(p, "That creature is too large to knock back!")?;
        toggle_special_bar(h, p)?;
        return Ok(-1);
    }
    let p = player(h, entity)?;
    if !drain_special(h, p, 25)? {
        return Ok(-1);
    }
    state?.estimated_hit = -1;
    let vx = at(h, v)?[0];
    let vy = at(h, v)?[1];
    let sx = at(h, p)?[0];
    let sy = at(h, p)?[1];
    let dir = if vx == sx && vy > sy {
        Some(Direction::North)
    } else if vx == sx && vy < sy {
        Some(Direction::South)
    } else if vx > sx && vy == sy {
        Some(Direction::East)
    } else if vx < sx && vy == sy {
        Some(Direction::West)
    } else if vx > sx && vy > sy {
        Some(Direction::NorthEast)
    } else if vx < sx && vy > sy {
        Some(Direction::NorthWest)
    } else if vx > sx && vy < sy {
        Some(Direction::SouthEast)
    } else if vx < sx && vy < sy {
        Some(Direction::SouthWest)
    } else {
        None
    };
    h.walking_reset(v)?;
    h.pulse_manager_clear(v)?;
    stun(h, v, 5)?;
    if let Some(dir) = dir {
        let (dx, dy) = dir.step();
        let dest = transform(at(h, v)?, dx, dy, 0);
        if h.dumb_path(v, dest)? {
            h.add_path(v, dest[0], dest[1])?;
        }
    }
    Ok(1)
}

/// ContentAPI `stun(entity, ticks)`, which is `stun(entity, ticks, true)`.
pub fn stun<H: Host>(h: &mut H, entity: &H::Entity, ticks: i32) -> Result<()> {
    h.walking_reset(entity)?;
    h.pulse_manager_clear(entity)?;
    h.lock_movement(entity, ticks)?;
    let now = h.ticks()?;
    h.set_stun_clock(entity, now.wrapping_add(ticks))?;
    h.graphics(entity, gfx(80, 96, 0))?;
    if h.kind(entity).is_player() {
        h.play_audio(entity, 2727, 0)?;
        let animation = h.new_animation(424, Priority::VeryHigh);
        h.animate(entity, Some(&animation))?;
        h.api_message(entity, "You have been stunned!")?;
    }
    Ok(())
}

fn slice_and_dice<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    entity: Option<&H::Entity>,
    victim: Option<&H::Entity>,
    mut state: Option<&mut BattleState<H>>,
) -> Result<i32> {
    let p = player(h, entity)?;
    if !drain_special(h, p, 50)? {
        return Ok(-1);
    }
    let maximum = hit_of(h, r, entity, victim, 1.0)?;
    let get_hit = |h: &mut H, max: i32, min: i32| -> Result<i32> {
        if accurate(h, r, entity, victim, MELEE, 1.0, 1.0)? {
            return random_between(h, min, max.wrapping_add(1));
        }
        Ok(0)
    };
    let hits: [i32; 4];
    let mut hit = get_hit(h, maximum.wrapping_sub(1), maximum / 2)?;
    if hit > 0 {
        hits = [hit, hit / 2, (hit / 2) / 2, ((hit / 2) / 2).wrapping_add(1)];
    } else {
        hit = get_hit(h, maximum.wrapping_mul(7) / 8, maximum.wrapping_mul(3) / 8)?;
        if hit > 0 {
            hits = [0, hit, hit / 2, (hit / 2).wrapping_add(1)];
        } else {
            hit = get_hit(h, maximum.wrapping_mul(3) / 4, maximum / 4)?;
            if hit > 0 {
                hits = [0, 0, hit, hit.wrapping_add(1)];
            } else {
                hit = get_hit(h, maximum.wrapping_mul(5) / 4, maximum / 4)?;
                if hit > 0 {
                    hits = [0, 0, 0, hit];
                } else {
                    hit = random(h, 2)?;
                    hits = [0, 0, hit, hit];
                }
            }
        }
    }
    let states = hits
        .iter()
        .map(|&hit| {
            let mut s = BattleState::new(entity.cloned(), victim.cloned());
            s.style = MELEE;
            s.estimated_hit = hit;
            Some(Target::State(Box::new(s)))
        })
        .collect();
    need(&mut state)?.targets = Some(states);
    Ok(1)
}

/// Sweep's `getTargets`.
fn sweep_targets<H: Host>(
    h: &mut H,
    entity: &H::Entity,
    victim: Option<&H::Entity>,
) -> Result<Vec<Option<Target<H>>>> {
    if !h.is_multi_zone(entity)? {
        return Ok(vec![Some(Target::This)]);
    }
    let v = victim.ok_or(Error::Null)?;
    if !h.is_multi_zone(v)? {
        return Ok(vec![Some(Target::This)]);
    }
    let mut list = vec![Some(Target::State(Box::new(BattleState::new(
        Some(entity.clone()),
        Some(v.clone()),
    ))))];
    for n in multihit::find_multihit_targets_for_dragon_halberd(h, v, entity)? {
        list.push(Some(Target::State(Box::new(BattleState::new(
            Some(entity.clone()),
            Some(n),
        )))));
    }
    Ok(list)
}

/// One Sweep target's rolls.
fn sweep_one<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    entity: &H::Entity,
    s: &mut BattleState<H>,
) -> Result<()> {
    s.style = MELEE;
    let target = s.victim.clone();
    let mut hit = 0;
    if accurate(h, r, Some(entity), target.as_ref(), MELEE, 1.0, 1.0)? {
        hit = roll(h, r, Some(entity), target.as_ref(), 1.1)?;
    }
    s.estimated_hit = hit;
    let target = s.victim.clone().ok_or(Error::Null)?;
    if h.size(&target)? > 1 {
        hit = 0;
        if accurate(h, r, Some(entity), Some(&target), MELEE, 0.75, 1.0)? {
            hit = roll(h, r, Some(entity), Some(&target), 1.1)?;
        }
        s.secondary_hit = hit;
    }
    Ok(())
}

fn sweep<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    entity: Option<&H::Entity>,
    victim: Option<&H::Entity>,
    state: Option<&mut BattleState<H>>,
) -> Result<i32> {
    let p = player(h, entity)?;
    if !drain_special(h, p, 30)? {
        return Ok(-1);
    }
    let targets = sweep_targets(h, p, victim)?;
    let state = state.ok_or(Error::Null)?;
    state.targets = Some(targets);
    let count = state.targets.as_ref().map_or(0, Vec::len);
    for at in 0..count {
        match state.targets.as_ref().and_then(|l| l.get(at)) {
            Some(Some(Target::This)) => sweep_one(h, r, p, state)?,
            Some(Some(Target::State(_))) => {
                let Some(Some(Target::State(mut s))) = state
                    .targets
                    .as_mut()
                    .and_then(|l| l.get_mut(at))
                    .map(Option::take)
                else {
                    unreachable!()
                };
                let outcome = sweep_one(h, r, p, &mut s);
                if let Some(slot) = state.targets.as_mut().and_then(|l| l.get_mut(at)) {
                    *slot = Some(Target::State(s));
                }
                outcome?;
            }
            _ => return Err(Error::Null),
        }
    }
    Ok(1)
}

fn warstrike<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    entity: Option<&H::Entity>,
    victim: Option<&H::Entity>,
    state: Result<&mut BattleState<H>>,
) -> Result<i32> {
    let p = player(h, entity)?;
    if !drain_special(h, p, 100)? {
        return Ok(-1);
    }
    let state = state?;
    state.style = MELEE;
    let mut hit = 0;
    if accurate(h, r, entity, victim, MELEE, 2.0, 1.0)? {
        let max = (f64::from(hit_of(h, r, entity, victim, 1.1)?) * 1.1) as i32;
        hit = random(h, max.wrapping_add(1))?;
    }
    state.estimated_hit = hit;
    let v = victim.ok_or(Error::Null)?;
    if h.kind(v).is_player() {
        h.send_message(v, "You have been drained.")?;
    }
    let mut left = update(h, v, skill::DEFENCE, hit.wrapping_neg(), 0)?.wrapping_neg();
    if left > 0 {
        left = update(h, v, skill::STRENGTH, left.wrapping_neg(), 0)?.wrapping_neg();
        if left > 0 {
            left = (-(h.prayer_points(v)? + f64::from(left))) as i32;
            h.decrement_prayer_points(v, f64::from(left))?;
            if left > 0 {
                left = update(h, v, skill::ATTACK, left.wrapping_neg(), 0)?.wrapping_neg();
                if left > 0 {
                    left = update(h, v, skill::MAGIC, left.wrapping_neg(), 0)?.wrapping_neg();
                    if left > 0 {
                        update(h, v, skill::RANGE, left.wrapping_neg(), 0)?;
                    }
                }
            }
        }
    }
    Ok(1)
}

/// `visualize(entity, victim, state)` of a melee special.
pub fn visualize<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    entity: &H::Entity,
    victim: Option<&H::Entity>,
    _state: &BattleState<H>,
) -> Result<()> {
    let special = h.special_of(r).ok_or(Error::Null)?;
    let (sound, animation, graphics) = match special {
        Special::AncientMace => (3592, 6147, gfx(1052, 0, 0)),
        Special::Backstab => (1084, 4198, gfx(704, 0, 0)),
        Special::Cleave => (2529, 1058, gfx(248, 96, 0)),
        Special::Clobber => return clobber_visualize(h, entity),
        Special::EnergyDrain => {
            audio(h, entity, 2713)?;
            let animation = h.new_animation(1658, Priority::High);
            h.animate(entity, Some(&animation))?;
            return h.graphics(victim.ok_or(Error::Null)?, gfx(341, 96, 0));
        }
        Special::Feint => {
            audio(h, entity, 2529)?;
            let animation = h.new_animation(10502, Priority::High);
            return h.animate(entity, Some(&animation));
        }
        Special::HealingBlade => (3857, 7071, gfx(1220, 0, 0)),
        Special::IceCleave => (3846, 7070, gfx(1221, 0, 0)),
        Special::Impale => {
            audio(h, entity, 2534)?;
            if h.rights(entity)? == Some(Rights::Administrator) {
                let animation = h.new_animation(2068, Priority::Mid);
                return h.animate(entity, Some(&animation));
            }
            return show(h, entity, 923, Some(gfx(274, 96, 0)));
        }
        Special::Judgement => (3865, 7074, gfx(1222, 0, 0)),
        Special::Powerstab => (2530, 3157, gfx(1225, 0, 0)),
        Special::Puncture => (2537, 1062, gfx(252, 96, 0)),
        Special::QuickSmash => return quick_smash_visualize(h, entity, victim),
        Special::SaradominsLightning => (3853, 7072, gfx(1224, 0, 0)),
        Special::Sever => (2540, 1872, gfx(347, 96, 0)),
        Special::Shatter => (2541, 1060, gfx(251, 96, 0)),
        Special::Shove => (2544, 1064, gfx(253, 96, 0)),
        Special::SliceAndDice => (2537, 10961, gfx(1950, 0, 0)),
        Special::Smash => (2520, 10501, gfx(1840, 0, 16)),
        Special::SpearWall => (2529, 10499, gfx(1835, 0, 0)),
        Special::Sweep => (2533, 1203, gfx(282, 96, 0)),
        Special::Warstrike => (3834, 7073, gfx(1223, 0, 0)),
        Special::Weaken => {
            show(h, entity, 2890, Some(gfx(483, 0, 0)))?;
            return audio(h, entity, 225);
        }
        other => return Err(Error::Binding(format!("{other:?} has no melee visualize"))),
    };
    audio(h, entity, sound)?;
    show(h, entity, animation, Some(graphics))
}

/// AncientMace's `getFormattedHit`: `formatHit(victim, hit)` alone.
pub fn ancient_mace_formatted_hit<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    _attacker: &H::Entity,
    victim: &H::Entity,
    _state: &BattleState<H>,
    hit: i32,
) -> Result<i32> {
    instance(h, r).format_hit(h, Some(victim), hit)
}

/// IceCleave's `adjustBattleState`.
pub fn ice_cleave_adjust<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    entity: &H::Entity,
    victim: &H::Entity,
    state: &mut BattleState<H>,
) -> Result<()> {
    base::adjust_battle_state_super(h, r, entity, victim, state)?;
    if state.estimated_hit > 0 {
        let timer = h.spawn_timer("frozen", &[TimerArg::Int(33), TimerArg::Bool(true)])?;
        if let Some(timer) = timer {
            h.register_timer(victim, timer)?;
        }
        h.graphics(victim, gfx(369, 0, 0))?;
    }
    Ok(())
}

/// One target's victim, hit and secondary hit; `None` is a null target.
type TargetHits<H> = Option<(
    Option<<H as crate::world::combat_swing::Types>::Entity>,
    i32,
    i32,
)>;

/// The target states' victims and hits, or `None` when there is no target
/// array.
fn target_list<H: Host>(state: &BattleState<H>) -> Option<Vec<TargetHits<H>>> {
    let list = state.targets.as_ref()?;
    Some(
        list.iter()
            .map(|t| match t {
                None => None,
                Some(Target::This) => Some((
                    state.victim.clone(),
                    state.estimated_hit,
                    state.secondary_hit,
                )),
                Some(Target::State(s)) => {
                    Some((s.victim.clone(), s.estimated_hit, s.secondary_hit))
                }
            })
            .collect(),
    )
}

/// Powerstab's and SpearWall's `impact`.
pub fn targets_impact<H: Host>(
    h: &mut H,
    _r: &H::HandlerRef,
    entity: Option<&H::Entity>,
    victim: Option<&H::Entity>,
    state: &mut BattleState<H>,
) -> Result<()> {
    if let Some(list) = target_list(state) {
        for (target, hit, _) in list.into_iter().flatten() {
            let target = target.ok_or(Error::Null)?;
            impact::handle_impact(h, &target, entity, hit, MELEE, None, false)?;
        }
        return Ok(());
    }
    let hit = state.estimated_hit;
    impact::handle_impact(
        h,
        victim.ok_or(Error::Null)?,
        entity,
        hit,
        MELEE,
        None,
        false,
    )?;
    Ok(())
}

/// Powerstab's, SpearWall's and Sweep's `visualizeImpact`: every target
/// plays the victim's defence animation.
pub fn targets_visualize_impact<H: Host>(
    h: &mut H,
    _r: &H::HandlerRef,
    _entity: Option<&H::Entity>,
    victim: Option<&H::Entity>,
    state: &mut BattleState<H>,
) -> Result<()> {
    if let Some(list) = target_list(state) {
        for (target, _, _) in list.into_iter().flatten() {
            // The receiver is null-checked after the argument is evaluated.
            let animation = h.defence_animation(victim.ok_or(Error::Null)?)?;
            let target = target.ok_or(Error::Null)?;
            h.animate(&target, animation.as_ref())?;
        }
        return Ok(());
    }
    let v = victim.ok_or(Error::Null)?;
    let animation = h.defence_animation(v)?;
    h.animate(v, animation.as_ref())
}

/// Puncture's `impact`: the second hit is flagged secondary.
pub fn puncture_impact<H: Host>(
    h: &mut H,
    _r: &H::HandlerRef,
    entity: Option<&H::Entity>,
    victim: Option<&H::Entity>,
    state: &mut BattleState<H>,
) -> Result<()> {
    let v = victim.ok_or(Error::Null)?;
    impact::handle_impact(h, v, entity, state.estimated_hit, MELEE, None, false)?;
    impact::handle_impact(h, v, entity, state.secondary_hit, MELEE, None, true)?;
    Ok(())
}

/// SaradominsLightning's `impact`: the secondary hit is magic.
pub fn lightning_impact<H: Host>(
    h: &mut H,
    _r: &H::HandlerRef,
    entity: Option<&H::Entity>,
    victim: Option<&H::Entity>,
    state: &mut BattleState<H>,
) -> Result<()> {
    let mut hit = state.estimated_hit;
    if hit > -1 {
        impact::handle_impact(
            h,
            victim.ok_or(Error::Null)?,
            entity,
            hit,
            MELEE,
            None,
            false,
        )?;
    }
    hit = state.secondary_hit;
    if hit > -1 {
        let v = victim.ok_or(Error::Null)?;
        h.graphics(v, gfx(1194, 0, 0))?;
        let style = Some(CombatStyle::Magic);
        impact::handle_impact(h, v, entity, hit, style, None, false)?;
    }
    Ok(())
}

/// Sweep's `adjustBattleState`: every target against its own victim.
pub fn sweep_adjust<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    entity: &H::Entity,
    victim: &H::Entity,
    state: &mut BattleState<H>,
) -> Result<()> {
    if state.targets.is_none() {
        return base::adjust_battle_state_super(h, r, entity, victim, state);
    }
    let count = state.targets.as_ref().map_or(0, Vec::len);
    for at in 0..count {
        match state.targets.as_ref().and_then(|l| l.get(at)) {
            Some(Some(Target::This)) => {
                let own = state.victim.clone().ok_or(Error::Null)?;
                base::adjust_battle_state_super(h, r, entity, &own, state)?;
            }
            Some(Some(Target::State(_))) => {
                let Some(Some(Target::State(mut s))) = state
                    .targets
                    .as_mut()
                    .and_then(|l| l.get_mut(at))
                    .map(Option::take)
                else {
                    unreachable!()
                };
                let outcome = match s.victim.clone() {
                    Some(own) => base::adjust_battle_state_super(h, r, entity, &own, &mut s),
                    None => Err(Error::Null),
                };
                if let Some(slot) = state.targets.as_mut().and_then(|l| l.get_mut(at)) {
                    *slot = Some(Target::State(s));
                }
                outcome?;
            }
            _ => {}
        }
    }
    Ok(())
}

/// Sweep's `impact`: both hits of every target on its own victim.
pub fn sweep_impact<H: Host>(
    h: &mut H,
    _r: &H::HandlerRef,
    entity: Option<&H::Entity>,
    victim: Option<&H::Entity>,
    state: &mut BattleState<H>,
) -> Result<()> {
    if let Some(list) = target_list(state) {
        for (target, hit, secondary) in list.into_iter().flatten() {
            let target = target.ok_or(Error::Null)?;
            impact::handle_impact(h, &target, entity, hit, MELEE, None, false)?;
            if secondary > -1 {
                impact::handle_impact(h, &target, entity, secondary, MELEE, None, false)?;
            }
        }
        return Ok(());
    }
    let v = victim.ok_or(Error::Null)?;
    impact::handle_impact(h, v, entity, state.estimated_hit, MELEE, None, false)?;
    if state.secondary_hit > -1 {
        impact::handle_impact(h, v, entity, state.secondary_hit, MELEE, None, false)?;
    }
    Ok(())
}
