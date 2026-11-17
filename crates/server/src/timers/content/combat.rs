//! Poison, Disease, Frozen, FrozenImmunity, HealOverTime, Miasmic,
//! DragonFireImmunity and PoisonImmunity.
use super::{
    Arg, Error, Host, Kind, Natives, State, UNINITIALIZED, colorize, has, integer, put, random,
    random_between, skills_error,
};
use crate::definitions::item_config::json::Json;
use crate::timers::{Handle, Manager, Result};
use crate::world::combat_pulse::HitsplatType;

/// `Sounds.CLOCK_TICK_1_3120`.
pub const CLOCK_TICK: i32 = 3120;
/// `Sounds.DRAGON_POTION_FINISHED_2607`.
pub const POTION_FINISHED: i32 = 2607;
/// `Skills.SLAYER`.
const SLAYER: i32 = 18;

fn location_text(location: Option<[i32; 3]>) -> String {
    location.map_or_else(|| "null".to_owned(), |[x, y, z]| format!("[{x}, {y}, {z}]"))
}

/// `damageSource`; reading it while unset throws.
fn source<H: Host>(ctx: &Natives<'_, H>, timer: &Handle) -> Result<H::Entity> {
    ctx.fields(timer, |s| match s {
        State::Poison { source, .. } => source.clone(),
        _ => None,
    })?
    .ok_or_else(|| Error::Binding(UNINITIALIZED.into()))
}

fn severity<H: Host>(ctx: &Natives<'_, H>, timer: &Handle) -> Result<i32> {
    ctx.fields(timer, |s| match s {
        State::Poison { severity, .. } => *severity,
        _ => 0,
    })
}

/// The slayer check of `Poison.onRegister`: `entity is NPC && entity.task !=
/// null && entity.task.levelReq > slayer.skills.getLevel(Skills.SLAYER)`.
fn below_task<H: Host>(ctx: &mut Natives<'_, H>, slayer: &H::Entity) -> Result<bool> {
    if !ctx.kind().is_npc() {
        return Ok(false);
    }
    let entity = ctx.entity.clone();
    if ctx.host.npc_task(&entity)?.is_none() {
        return Ok(false);
    }
    let required = ctx.host.npc_task(&entity)?.ok_or(Error::Null)?;
    let (skills, context) = ctx.host.skills(slayer);
    let level = skills.level(SLAYER, false, context).map_err(skills_error)?;
    Ok(required > level)
}

fn removed_message<H: Host>(ctx: &mut Natives<'_, H>) -> Result<String> {
    let entity = ctx.entity.clone();
    let name = ctx.host.name(&entity)?;
    let location = ctx.host.location(&entity)?;
    Ok(format!(
        "[Poison] -> Removed poison from {} {} due to Slayer reqs!",
        name.as_deref().unwrap_or("null"),
        location_text(location)
    ))
}

pub(super) fn poison_on_register<S, H: Host>(
    ctx: &mut Natives<'_, H>,
    timer: &Handle,
    manager: &mut Manager,
    state: &mut S,
) -> Result<()> {
    if ctx.is_player() {
        ctx.message("You have been poisoned.")?;
        let severity = severity(ctx, timer)?;
        ctx.debug(&format!("[Poison] -> Received for {severity} severity."))?;
    }
    let from = source(ctx, timer)?;
    if ctx.host.kind(&from).is_player() {
        let severity = severity(ctx, timer)?;
        ctx.host.debug(
            &from,
            &format!("[Poison] -> Applied for {severity} severity."),
        )?;
        if below_task(ctx, &from)? {
            let message = removed_message(ctx)?;
            ctx.host.debug(&from, &message)?;
            manager.remove(Some(timer), state, ctx)?;
        }
    }
    let from = source(ctx, timer)?;
    if ctx.host.kind(&from) == super::EntityKind::Familiar {
        if let Some(owner) = ctx.host.familiar_owner(&from)? {
            if below_task(ctx, &owner)? {
                let message = removed_message(ctx)?;
                ctx.host.debug(&owner, &message)?;
                manager.remove(Some(timer), state, ctx)?;
            }
        }
    }
    Ok(())
}

pub(super) fn poison_run<H: Host>(ctx: &mut Natives<'_, H>, timer: &Handle) -> Result<bool> {
    let entity = ctx.entity.clone();
    ctx.host.remove_weak_scripts(&entity)?;
    let from = source(ctx, timer)?;
    // getDamageFromSeverity(severity--): the old value, then the decrement.
    let old = ctx.fields(timer, |s| match s {
        State::Poison { severity, .. } => {
            let old = *severity;
            *severity = old.wrapping_sub(1);
            old
        }
        _ => 0,
    })?;
    let damage = old.wrapping_add(4) / 5;
    ctx.host
        .manual_hit(&entity, &from, damage, HitsplatType::Poison)?;
    let now = severity(ctx, timer)?;
    if now == 0 && ctx.is_player() {
        ctx.message("The poison has worn off.")?;
    }
    Ok(now > 0)
}

pub(super) fn poison_save<H: Host>(
    ctx: &mut Natives<'_, H>,
    timer: &Handle,
    root: &mut Json,
) -> Result<()> {
    let from = source(ctx, timer)?;
    let uid = if ctx.host.kind(&from).is_player() {
        ctx.host.uid(&from)?
    } else {
        -1
    };
    put(root, "source-uid", Json::Int(i64::from(uid)))?;
    let severity = severity(ctx, timer)?;
    put(root, "severity", Json::Text(severity.to_string()))
}

pub(super) fn poison_parse<H: Host>(
    ctx: &mut Natives<'_, H>,
    timer: &Handle,
    data: &Json,
) -> Result<()> {
    let uid = integer(data, "source-uid")?;
    let from = match ctx.host.player_by_uid(uid)? {
        Some(player) => player,
        None => ctx.entity.clone(),
    };
    ctx.fields(timer, |s| {
        if let State::Poison { source, .. } = s {
            *source = Some(from);
        }
    })?;
    let value = integer(data, "severity")?;
    ctx.fields(timer, |s| {
        if let State::Poison { severity, .. } = s {
            *severity = value;
        }
    })
}

pub(super) fn disease_before_register<S, H: Host>(
    ctx: &mut Natives<'_, H>,
    timer: &Handle,
    manager: &mut Manager,
    state: &mut S,
) -> Result<()> {
    if has(manager, Kind::Disease)? {
        manager.remove(Some(timer), state, ctx)
    } else if ctx.is_player() {
        ctx.message("You have become diseased.")
    } else {
        Ok(())
    }
}

pub(super) fn disease_run<H: Host>(ctx: &mut Natives<'_, H>, timer: &Handle) -> Result<bool> {
    let entity = ctx.entity.clone();
    let damage = random_between(ctx.host, 1, 5)?;
    ctx.host
        .manual_hit(&entity, &entity, damage, HitsplatType::Disease)?;
    let mut skill = random(ctx.host, 24)?;
    if skill == 3 {
        skill -= 1;
    }
    let (skills, context) = ctx.host.skills(&entity);
    skills
        .update_level(skill, damage.wrapping_neg(), 0, context)
        .map_err(skills_error)?;
    let left = ctx.fields(timer, |s| match s {
        State::Disease { hits_left } => {
            *hits_left = hits_left.wrapping_sub(1);
            *hits_left
        }
        _ => 0,
    })?;
    if left == 0 && ctx.is_player() {
        ctx.message("The disease has wore off.")?;
    }
    Ok(left > 0)
}

/// DragonFireImmunity.run.
pub(super) fn dragonfire_run<H: Host>(ctx: &mut Natives<'_, H>, timer: &Handle) -> Result<bool> {
    let left = decrement(ctx, timer)?;
    let entity = ctx.entity.clone();
    // secondsToTicks(30)
    if ctx.is_player() && left == 50 {
        ctx.message(&colorize(
            "%RYou have 30 seconds remaining on your antifire potion.",
        ))?;
        ctx.host.play_audio(&entity, CLOCK_TICK, 0, 3)?;
    } else if ctx.is_player() && left == 0 {
        ctx.message(&colorize("%RYour antifire potion has expired."))?;
        ctx.host.play_audio(&entity, POTION_FINISHED, 0, 1)?;
    }
    Ok(left > 0)
}

/// PoisonImmunity.run.
pub(super) fn poison_immunity_run<H: Host>(
    ctx: &mut Natives<'_, H>,
    timer: &Handle,
) -> Result<bool> {
    let left = decrement(ctx, timer)?;
    if ctx.is_player() && left == 0 {
        let entity = ctx.entity.clone();
        ctx.message(&colorize("%RYour poison immunity has expired."))?;
        ctx.host.play_audio(&entity, POTION_FINISHED, 0, 1)?;
    }
    Ok(left > 0)
}

fn decrement<H: Host>(ctx: &Natives<'_, H>, timer: &Handle) -> Result<i32> {
    ctx.fields(timer, |s| match s {
        State::Remaining { ticks } => {
            *ticks = ticks.wrapping_sub(1);
            *ticks
        }
        _ => 0,
    })
}

pub(super) fn frozen_before_register<S, H: Host>(
    ctx: &mut Natives<'_, H>,
    timer: &Handle,
    manager: &mut Manager,
    state: &mut S,
) -> Result<()> {
    if has(manager, Kind::FrozenImmunity)? {
        return manager.remove(Some(timer), state, ctx);
    }
    if has(manager, Kind::Frozen)? {
        return manager.remove(Some(timer), state, ctx);
    }
    if ctx.is_player() {
        ctx.message("You have been frozen!")?;
    }
    Ok(())
}

pub(super) fn frozen_run<S, H: Host>(
    ctx: &mut Natives<'_, H>,
    timer: &Handle,
    manager: &mut Manager,
    state: &mut S,
) -> Result<bool> {
    let apply = ctx.fields(timer, |s| {
        matches!(
            s,
            State::Frozen {
                apply_immunity: true
            }
        )
    })?;
    if apply {
        let immunity = ctx.spawn(manager, Kind::FrozenImmunity, &[Arg::Int(7)], state)?;
        ctx.register(manager, Some(&immunity), state)?;
    } else if ctx.is_player() {
        ctx.debug("Can't apply immunity")?;
    }
    Ok(false)
}

pub(super) fn frozen_immunity_on_register<S, H: Host>(
    ctx: &mut Natives<'_, H>,
    timer: &Handle,
    manager: &mut Manager,
    state: &mut S,
) -> Result<()> {
    if has(manager, Kind::Frozen)? {
        ctx.remove_class(manager, Kind::Frozen, state)?;
    }
    if ctx.is_player() {
        let interval = timer.read().run_interval;
        ctx.debug(&format!("Applied frozen immunity for {interval} ticks."))?;
    }
    Ok(())
}

pub(super) fn heal_on_register<S, H: Host>(
    ctx: &mut Natives<'_, H>,
    timer: &Handle,
    manager: &mut Manager,
    state: &mut S,
) -> Result<()> {
    if has(manager, Kind::MiasmicImmunity)? {
        manager.remove(Some(timer), state, ctx)?;
    }
    if has(manager, Kind::Miasmic)? {
        manager.remove(Some(timer), state, ctx)?;
    }
    Ok(())
}

pub(super) fn heal_run<H: Host>(ctx: &mut Natives<'_, H>, timer: &Handle) -> Result<bool> {
    let (amount, left) = ctx.fields(timer, |s| match s {
        State::Heal {
            remaining,
            per_tick,
        } => {
            let amount = (*remaining).min(*per_tick);
            *remaining = remaining.wrapping_sub(amount);
            (amount, *remaining)
        }
        _ => (0, 0),
    })?;
    let entity = ctx.entity.clone();
    let (skills, context) = ctx.host.skills(&entity);
    skills.heal(amount, context).map_err(skills_error)?;
    Ok(left > 0)
}

pub(super) fn miasmic_before_register<S, H: Host>(
    ctx: &mut Natives<'_, H>,
    timer: &Handle,
    manager: &mut Manager,
    state: &mut S,
) -> Result<()> {
    if has(manager, Kind::MiasmicImmunity)? {
        manager.remove(Some(timer), state, ctx)?;
    }
    if has(manager, Kind::Miasmic)? {
        manager.remove(Some(timer), state, ctx)?;
    }
    Ok(())
}
