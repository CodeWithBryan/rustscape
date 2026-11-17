//! SkillRestore and its two prayer hooks.
use super::{
    EventClass, Hook, Host, Kind, Natives, State, get_or_start, settings_error, skills_error,
};
use crate::timers::{Handle, Manager, Result};
use crate::world::prayer::PrayerType;

/// `Items.REGEN_BRACELET_11133`.
pub const REGEN_BRACELET: i32 = 11133;
/// `EquipmentSlot.HANDS.ordinal`.
const HANDS: i32 = 9;
const HITPOINTS: usize = 3;
const PRAYER: usize = 5;
const SUMMONING: usize = 23;

pub(super) fn on_register<H: Host>(ctx: &mut Natives<'_, H>) -> Result<()> {
    let entity = ctx.entity.clone();
    ctx.host
        .hook(&entity, EventClass::PrayerActivated, Hook::PrayerActivated)?;
    ctx.host.hook(
        &entity,
        EventClass::PrayerDeactivated,
        Hook::PrayerDeactivated,
    )?;
    if ctx.is_player() {
        ctx.debug("Registered skill restoration timer.")?;
    }
    Ok(())
}

/// `ticksSinceLastRestore[slot]++ >= restoreTicks[slot]`.
fn due<H: Host>(ctx: &Natives<'_, H>, timer: &Handle, slot: usize) -> Result<bool> {
    ctx.fields(timer, |s| match s {
        State::SkillRestore { since, restore } => {
            let old = since[slot];
            since[slot] = old.wrapping_add(1);
            old >= restore[slot]
        }
        _ => false,
    })
}

fn restart<H: Host>(ctx: &Natives<'_, H>, timer: &Handle, slot: usize) -> Result<()> {
    ctx.fields(timer, |s| {
        if let State::SkillRestore { since, .. } = s {
            since[slot] = 0;
        }
    })
}

/// `getHealAmount(entity)`.
fn heal_amount<H: Host>(ctx: &mut Natives<'_, H>) -> Result<i32> {
    if !ctx.is_player() {
        return Ok(1);
    }
    let entity = ctx.entity.clone();
    Ok(match ctx.host.worn(&entity, HANDS)? {
        Some(REGEN_BRACELET) => 2,
        _ => 1,
    })
}

pub(super) fn run<H: Host>(ctx: &mut Natives<'_, H>, timer: &Handle) -> Result<bool> {
    let entity = ctx.entity.clone();
    for slot in 0..24 {
        if slot == PRAYER || slot == SUMMONING {
            continue;
        }
        if !due(ctx, timer, slot)? {
            continue;
        }
        let low = slot == HITPOINTS && {
            let (skills, context) = ctx.host.skills(&entity);
            let lifepoints = skills.lifepoints;
            lifepoints < skills.maximum_lifepoints(context).map_err(skills_error)?
        };
        if low {
            let amount = heal_amount(ctx)?;
            let (skills, context) = ctx.host.skills(&entity);
            skills.heal(amount, context).map_err(skills_error)?;
        } else {
            let (skills, context) = ctx.host.skills(&entity);
            let max = skills.static_levels[slot];
            let current = skills
                .level(slot as i32, false, context)
                .map_err(skills_error)?;
            if current != max {
                let step = if current < max { 1 } else { -1 };
                skills
                    .update_level(slot as i32, step, max, context)
                    .map_err(skills_error)?;
            }
        }
        restart(ctx, timer, slot)?;
    }
    // ticksSinceLastRestore[24]++ >= 50 (not restoreTicks[24]).
    let special = ctx.is_player()
        && ctx.fields(timer, |s| match s {
            State::SkillRestore { since, .. } => {
                let old = since[24];
                since[24] = old.wrapping_add(1);
                old >= 50
            }
            _ => false,
        })?;
    if special {
        let (settings, context) = ctx.host.settings(&entity);
        let energy = settings.fields.read().special_energy;
        settings
            .set_special_energy(context, 100.min(energy.wrapping_add(10)))
            .map_err(settings_error)?;
        restart(ctx, timer, 24)?;
    }
    Ok(true)
}

/// `PrayerActivatedHook.process` (`active`) and `PrayerDeactivatedHook.process`.
pub(super) fn prayer<H: Host>(
    ctx: &mut Natives<'_, H>,
    manager: &mut Manager,
    prayer: PrayerType,
    active: bool,
) -> Result<()> {
    let restore = get_or_start(ctx, manager, Kind::SkillRestore, &[])?;
    let ticks = if active { 50 } else { 100 };
    ctx.fields(&restore, |s| {
        let State::SkillRestore { since, restore } = s else {
            return;
        };
        match prayer {
            PrayerType::RapidHeal => {
                restore[HITPOINTS] = ticks;
                since[HITPOINTS] = 0;
            }
            PrayerType::RapidRestore => {
                for slot in 0..24 {
                    if slot == HITPOINTS || slot == PRAYER || slot == SUMMONING {
                        continue;
                    }
                    restore[slot] = ticks;
                    since[slot] = 0;
                }
            }
            _ => {}
        }
    })
}
