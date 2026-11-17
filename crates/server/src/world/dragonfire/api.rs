//! The dragonfire protection checks: ContentAPI
//! `hasDragonfireShieldProtection` and `calculateDragonfireMaxHit`, and
//! `Player.getDragonfireProtection`.
use super::{Host, Value};
use crate::world::combat_pulse::slot;
use crate::world::combat_swing::Result;

pub const ELEMENTAL_SHIELD: i32 = 2890;
pub const MIND_SHIELD: i32 = 9731;
pub const ANTI_DRAGON_SHIELD: i32 = 1540;
pub const DRAGONFIRE_SHIELD: i32 = 11283;
pub const DRAGONFIRE_SHIELD_UNCHARGED: i32 = 11284;

/// `PrayerType.PROTECT_FROM_MAGIC`.
const PROTECT_FROM_MAGIC: &str = "PROTECT_FROM_MAGIC";

/// `hasDragonfireShieldProtection(player, wyvern)`: wyvern fire ignores the
/// anti-dragon shield but not the elemental and mind shields.
pub fn has_dragonfire_shield_protection<H: Host>(
    h: &mut H,
    player: &H::Entity,
    wyvern: bool,
) -> Result<bool> {
    let Some(shield) = h.equipment(player, slot::SHIELD)? else {
        return Ok(false);
    };
    Ok(match h.item_id(&shield) {
        ELEMENTAL_SHIELD | MIND_SHIELD => wyvern,
        ANTI_DRAGON_SHIELD => !wyvern,
        DRAGONFIRE_SHIELD | DRAGONFIRE_SHIELD_UNCHARGED => true,
        _ => false,
    })
}

/// `calculateDragonfireMaxHit(entity, maxDamage, wyvern, unprotectableDamage,
/// sendMessage)`: a player's shield, antifire potion and magic prayer, or a
/// non-player's protection tokens (0x2 shield, 0x4 potion, 0x8 prayer).
pub fn calculate_dragonfire_max_hit<H: Host>(
    h: &mut H,
    entity: &H::Entity,
    max_damage: i32,
    wyvern: bool,
    unprotectable_damage: i32,
    send_message: bool,
) -> Result<i32> {
    let (has_shield, has_potion, has_prayer);
    if h.kind(entity).is_player() {
        has_shield = has_dragonfire_shield_protection(h, entity, wyvern)?;
        has_potion = !wyvern && h.dragonfire_immunity(entity)?;
        has_prayer = h.prayer_on(entity, PROTECT_FROM_MAGIC)?;
        if send_message {
            let breath = if wyvern { "icy" } else { "fiery" };
            let message = if has_shield && has_potion {
                format!("Your potion and shield fully absorb the {breath} breath.")
            } else if has_shield {
                format!("Your shield absorbs most of the {breath} breath.")
            } else if has_potion {
                "Your potion absorbs some of the fiery breath.".to_owned()
            } else if has_prayer {
                format!("Your prayer absorbs some of the {breath} breath.")
            } else {
                format!("You are horribly burnt by the {breath} breath.")
            };
            h.api_message(entity, &message)?;
        }
    } else {
        let tokens = h.npc_dragonfire_protection(entity, !wyvern)?;
        has_shield = tokens & 0x2 != 0;
        has_potion = tokens & 0x4 != 0;
        has_prayer = tokens & 0x8 != 0;
    }
    let mut effective = f64::from(max_damage);
    if has_prayer && !has_shield && !has_potion {
        effective -= 0.6 * f64::from(max_damage);
    } else {
        if has_shield {
            effective -= 0.9 * f64::from(max_damage);
        }
        if has_potion {
            effective -= 0.1 * f64::from(max_damage);
        }
    }
    Ok(unprotectable_damage.max(effective as i32))
}

/// `Player.getDragonfireProtection(fire)`: 0x2 fire resistance (fire only),
/// 0x4 a protecting shield, 0x8 the magic prayer; stored as
/// `fire_resistance`.
pub fn player_dragonfire_protection<H: Host>(
    h: &mut H,
    player: &H::Entity,
    fire: bool,
) -> Result<i32> {
    let mut value = 0;
    if fire && h.has_fire_resistance(player)? {
        value |= 0x2;
    }
    let item = h.equipment(player, slot::SHIELD)?;
    if let Some(item) = item {
        let id = h.item_id(&item);
        if id == DRAGONFIRE_SHIELD
            || id == DRAGONFIRE_SHIELD_UNCHARGED
            || (fire && id == ANTI_DRAGON_SHIELD)
            || (!fire && (id == ELEMENTAL_SHIELD || id == MIND_SHIELD))
        {
            value |= 0x4;
        }
    }
    if h.prayer_on(player, PROTECT_FROM_MAGIC)? {
        value |= 0x8;
    }
    h.set_attribute_value(player, "fire_resistance", Value::Int(value))?;
    Ok(value)
}
