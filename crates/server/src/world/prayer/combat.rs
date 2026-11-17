//! The prayer reads of the combat path, answered from native prayer state.
//!
//! A world binding answers these `combat_swing::Host` and
//! `combat_pulse::Host` services with the functions here:
//!
//! | Service | Original | Function |
//! | --- | --- | --- |
//! | `combat_swing::Host::prayer_skill_bonus` | `player.prayer.getSkillBonus(slot)` | [`skill_bonus`] |
//! | `combat_swing::Host::protects_from_summoning` | `prayer[PROTECT_FROM_SUMMONING]` | [`protects_from_summoning`] |
//! | `combat_swing::Host::has_protection_prayer` | `Player`/`NPC.hasProtectionPrayer(style)` | [`player_has_protection_prayer`], [`npc_has_protection_prayer`] |
//! | `combat_pulse::Host::prayer_active` | `prayer.get(SMITE)`, `prayer.get(REDEMPTION)` | [`active`] |
//! | `combat_pulse::Host::start_redemption` | `prayer.startRedemption()` | [`super::start_redemption`] |
//! | `combat_pulse::Host::entity_formatted_hit` | `Entity.getFormattedHit(state, hit)` | [`formatted_hit`] |
//!
//! Prayer points (`prayer_points`, `decrement_prayer_points`) are the
//! native `player::skills::Skills` fields and methods, not prayer state.
use super::{PrayerState, PrayerType};
use crate::world::combat_pulse::Prayer as PulsePrayer;
use crate::world::combat_swing::{ArmourSet, BattleState, CombatStyle, Kind, Result, Types};

/// `Prayer.getSkillBonus(slot)`: the multiplier the swing formulas add,
/// `floor(level + bonus * level)`.
pub fn skill_bonus(state: &PrayerState, slot: i32) -> f64 {
    state.skill_bonus(slot)
}

/// `prayer.get(PROTECT_FROM_SUMMONING)`.
pub fn protects_from_summoning(state: &PrayerState) -> bool {
    state.get(PrayerType::ProtectFromSummoning)
}

/// `Player.hasProtectionPrayer(style)`: false for a null style.
pub fn player_has_protection_prayer(state: &PrayerState, style: Option<CombatStyle>) -> bool {
    match style {
        None => false,
        Some(style) => state.get(PrayerType::protection(style)),
    }
}

/// `NPC.hasProtectionPrayer(style)`: `properties.protectStyle == style`, so
/// two nulls compare equal.
pub fn npc_has_protection_prayer(
    protect_style: Option<CombatStyle>,
    style: Option<CombatStyle>,
) -> bool {
    protect_style == style
}

/// `prayer.get(type)` for the prayers the combat pulse queries.
pub fn active(state: &PrayerState, prayer: PulsePrayer) -> bool {
    state.get(match prayer {
        PulsePrayer::Smite => PrayerType::Smite,
        PulsePrayer::Redemption => PrayerType::Redemption,
    })
}

/// `Entity.isIgnoreProtection(style)`: no class of the pinned server
/// overrides it.
pub fn is_ignore_protection(_style: CombatStyle) -> bool {
    false
}

/// `Entity.getFormattedHit(state, hit)`. It reads the state's attacker and
/// victim, not the entity it is called on. A protected hit is multiplied by
/// 0.6 between two players and by 0 otherwise, as the int compound
/// assignment `hit *= ...` does: truncated toward zero, then widened.
/// `has_protection` is the victim's `hasProtectionPrayer(style)`.
pub fn formatted_hit<T: Types>(
    state: &BattleState<T>,
    hit: i32,
    kind: impl Fn(&T::Entity) -> Kind,
    mut has_protection: impl FnMut(&T::Entity, CombatStyle) -> Result<bool>,
) -> Result<f64> {
    let (Some(entity), Some(victim), Some(style)) = (&state.entity, &state.victim, state.style)
    else {
        return Ok(f64::from(hit));
    };
    if state.armour_effect != Some(ArmourSet::Verac)
        && !is_ignore_protection(style)
        && has_protection(victim, style)?
    {
        let factor = if kind(entity) == Kind::Player && kind(victim) == Kind::Player {
            0.6
        } else {
            0.0
        };
        return Ok(f64::from((f64::from(hit) * factor) as i32));
    }
    Ok(f64::from(hit))
}
