//! `SalamanderSwingHandler`: a direct `CombatSwingHandler` subclass whose
//! private `style` follows the attacker's attack style (defensive cast:
//! magic, range accurate: range, otherwise melee) every time `canSwing`,
//! `calculateAccuracy` or `calculateHit` runs, and which hands most members
//! to that style's singleton.
use super::{Host, Subclass, subclass_of};
use crate::world::combat_pulse::{handler as base, skill};
use crate::world::combat_swing::{
    self as cs, ArmourSet, BattleState, CombatStyle, Error, Graphics, InteractionType, Result,
    STYLE_DEFENSIVE_CAST, STYLE_RANGE_ACCURATE, Weapon,
};
use crate::world::special_attacks::Priority;

/// `EquipmentSlot.AMMO.ordinal`.
const AMMO_SLOT: i32 = 13;

/// `getAmmoId(id)`: the tar each salamander burns, -1 for anything else.
pub fn ammo_id(id: i32) -> i32 {
    match id {
        10149 => 10142,
        10146 => 10143,
        10147 => 10144,
        10148 => 10145,
        _ => -1,
    }
}

/// `getSalamanderMagicDamageBonus(id)`.
pub fn magic_damage_bonus(id: i32) -> i32 {
    match id {
        10149 => 56,
        10146 => 59,
        10147 => 77,
        10148 => 92,
        _ => 0,
    }
}

/// The instance's current `style` field.
pub fn style_of<H: Host>(h: &H, r: &H::HandlerRef) -> Result<CombatStyle> {
    match subclass_of(h, r)? {
        Subclass::Salamander { style } => Ok(style),
        Subclass::Chinchompa => Err(Error::Binding("not a SalamanderSwingHandler".into())),
    }
}

/// `checkStyle(e)`: one attack style read, then the field.
fn check_style<H: Host>(h: &mut H, r: &H::HandlerRef, e: Option<&H::Entity>) -> Result<()> {
    let e = e.ok_or(Error::Null)?;
    let index = h.attack_style(e)?.ok_or(Error::Null)?.style;
    let style = match index {
        STYLE_DEFENSIVE_CAST => CombatStyle::Magic,
        STYLE_RANGE_ACCURATE => CombatStyle::Range,
        _ => CombatStyle::Melee,
    };
    subclass_of(h, r)?;
    h.set_salamander_style(r, style);
    Ok(())
}

/// `style.swingHandler`: the current style's singleton.
fn singleton<H: Host>(h: &H, r: &H::HandlerRef) -> Result<H::HandlerRef> {
    Ok(h.style_handler(style_of(h, r)?))
}

/// `canSwing`: reach of one tile (two at a moving victim, then a moving
/// interaction) between the centres, then `CombatSwingHandler.canSwing`.
pub fn can_swing<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    entity: &H::Entity,
    victim: &H::Entity,
) -> Result<InteractionType> {
    check_style(h, r, Some(entity))?;
    if !base::is_projectile_clipped(h, entity, Some(victim), false)? {
        return Ok(InteractionType::NoInteract);
    }
    let mut distance = 1;
    let mut good_range = in_reach(h, r, entity, victim, distance)?;
    let mut kind = InteractionType::StillInteract;
    if h.is_moving(victim)? && !good_range {
        distance += 1;
        good_range = in_reach(h, r, entity, victim, distance)?;
        kind = InteractionType::MoveInteract;
    }
    if good_range && base::is_attackable(h, r, entity, victim)? != InteractionType::NoInteract {
        if kind == InteractionType::StillInteract {
            h.walking_reset(entity)?;
        }
        return Ok(kind);
    }
    Ok(InteractionType::NoInteract)
}

/// `victim.centerLocation.withinDistance(entity.centerLocation,
/// getCombatDistance(entity, victim, distance))`.
fn in_reach<H: Host>(
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
    if ours[2] != theirs[2] {
        return Ok(false);
    }
    Ok(cs::distance(theirs, ours) <= f64::from(reach))
}

/// `swing(entity, victim, state)`: the salamander as weapon, its tar as
/// ammunition, an accuracy roll and `random(max + 1)` in the current style,
/// capped to the victim's lifepoints; one tar used; a delay of 1.
pub fn swing<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    entity: Option<&H::Entity>,
    victim: Option<&H::Entity>,
    state: &mut BattleState<H>,
) -> Result<i32> {
    swing_with(h, r, entity, victim, Some(state))
}

/// `swing` with a possibly null state.
pub fn swing_with<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    entity: Option<&H::Entity>,
    victim: Option<&H::Entity>,
    state: Option<&mut BattleState<H>>,
) -> Result<i32> {
    let state = state.ok_or(Error::Null)?;
    state.style = Some(style_of(h, r)?);
    if let Some(p) = entity.filter(|e| h.kind(e).is_player()) {
        let item = h.equipment(p, cs::slot::WEAPON)?;
        state.weapon = Some(Weapon::new(h, item));
    }
    if state.weapon.is_none() || !has_ammo(h, entity, state)? {
        h.stop_combat_pulse(entity.ok_or(Error::Null)?)?;
        return Ok(-1);
    }
    let mut hit = 0;
    let style = style_of(h, r)?;
    if base::is_accurate_impact(h, r, entity, victim, Some(style), 1.0, 1.0)? {
        let max = base::calculate_hit(h, r, entity, victim, 1.0)?;
        state.maximum_hit = max;
        hit = super::random(h, max.wrapping_add(1))?;
    }
    state.estimated_hit = hit;
    let Some(e) = entity else {
        return Ok(-1);
    };
    let v = victim.ok_or(Error::Null)?;
    if h.location(v)?.is_none() {
        return Ok(-1);
    }
    if state.estimated_hit > h.lifepoints(v)? {
        state.estimated_hit = h.lifepoints(v)?;
    }
    use_ammo(h, e, state)?;
    Ok(1)
}

/// The companion's `hasAmmo(e, state)`: the tar of the salamander in the
/// ammunition slot.
pub fn has_ammo<H: Host>(h: &mut H, e: Option<&H::Entity>, state: &BattleState<H>) -> Result<bool> {
    let Some(e) = e.filter(|e| h.kind(e).is_player()) else {
        return Ok(true);
    };
    let Some(ammo) = h.equipment(e, AMMO_SLOT)? else {
        h.send_message(e, "You do not have enough ammo left.")?;
        return Ok(false);
    };
    let weapon = state.weapon.as_ref().ok_or(Error::Null)?;
    if h.item_id(&ammo) == ammo_id(weapon.id) {
        return Ok(true);
    }
    h.send_message(
        e,
        "You can't use this type of ammunition with this salamander.",
    )?;
    Ok(false)
}

/// The companion's `useAmmo(e, state)`: one tar removed from the equipment.
pub fn use_ammo<H: Host>(h: &mut H, e: &H::Entity, state: &BattleState<H>) -> Result<()> {
    if !h.kind(e).is_player() {
        return Ok(());
    }
    let weapon = state.weapon.as_ref().ok_or(Error::Null)?;
    h.equipment_remove_value(e, (ammo_id(weapon.id), 1, 1000))?;
    Ok(())
}

/// `impact`: the current style's singleton's.
pub fn impact<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    entity: Option<&H::Entity>,
    victim: Option<&H::Entity>,
    state: &mut BattleState<H>,
) -> Result<()> {
    let singleton = singleton(h, r)?;
    base::impact(h, &singleton, entity, victim, state)
}

/// `visualize`: animation 5247 with graphic (952, 100).
pub fn visualize<H: Host>(
    h: &mut H,
    _: &H::HandlerRef,
    entity: &H::Entity,
    _victim: Option<&H::Entity>,
    _state: &BattleState<H>,
) -> Result<()> {
    let animation = h.new_animation(5247, Priority::Mid);
    let graphic = Graphics {
        id: 952,
        height: 100,
        delay: 0,
    };
    h.visualize_entity(entity, Some(&animation), Some(graphic))
}

/// `visualizeImpact`: the current style's singleton's.
pub fn visualize_impact<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    entity: Option<&H::Entity>,
    victim: Option<&H::Entity>,
    state: &mut BattleState<H>,
) -> Result<()> {
    let singleton = singleton(h, r)?;
    base::visualize_impact(h, &singleton, entity, victim, state)
}

/// `calculateAccuracy`: the style follows the attacker first.
pub fn calculate_accuracy<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    entity: Option<&H::Entity>,
) -> Result<i32> {
    check_style(h, r, entity)?;
    let singleton = singleton(h, r)?;
    base::calculate_accuracy(h, &singleton, entity)
}

/// `calculateHit`: 0 without an attacker; the style follows the attacker;
/// in magic the salamander's own formula with its magic damage bonus.
pub fn calculate_hit<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    entity: Option<&H::Entity>,
    victim: Option<&H::Entity>,
    modifier: f64,
) -> Result<i32> {
    let Some(e) = entity else {
        return Ok(0);
    };
    check_style(h, r, Some(e))?;
    if style_of(h, r)? == CombatStyle::Magic {
        let level = h.level_with(e, skill::MAGIC, true)?;
        let bonuses = h.bonuses(e)?.ok_or(Error::Null)?;
        let mut bonus = *bonuses.get(13).ok_or(Error::Index)?;
        if h.kind(e).is_player() {
            let weapon = h.equipment(e, cs::slot::WEAPON)?.ok_or(Error::Null)?;
            bonus = bonus.wrapping_add(magic_damage_bonus(h.item_id(&weapon)));
        }
        let mut cumulative = f64::from(level);
        cumulative *= base::set_multiplier(h, r, Some(e), skill::MAGIC)?;
        cumulative *= f64::from(bonus.wrapping_add(64));
        return Ok(((0.5 + cumulative.ceil() / 640.0) * modifier).floor() as i32);
    }
    let singleton = singleton(h, r)?;
    base::calculate_hit(h, &singleton, entity, victim, modifier)
}

/// `addExperience`: 1.33 hitpoints experience per damage and the style's
/// skill (magic 2, range 4, melee strength 4).
pub fn add_experience<H: Host>(
    h: &mut H,
    _: &H::HandlerRef,
    entity: Option<&H::Entity>,
    _victim: Option<&H::Entity>,
    state: Option<&BattleState<H>>,
) -> Result<()> {
    let e = entity.ok_or(Error::Null)?;
    let state = state.ok_or(Error::Null)?;
    let hit = f64::from(state.estimated_hit);
    h.add_experience(e, skill::HITPOINTS, hit * 1.33, true)?;
    if state.style == Some(CombatStyle::Magic) {
        h.add_experience(e, skill::MAGIC, hit * 2.0, true)?;
    }
    if state.style == Some(CombatStyle::Range) {
        h.add_experience(e, skill::RANGE, hit * 4.0, true)?;
    }
    if state.style == Some(CombatStyle::Melee) {
        h.add_experience(e, skill::STRENGTH, hit * 4.0, true)?;
    }
    Ok(())
}

/// `calculateDefence`: the current style's singleton's (no style check).
pub fn calculate_defence<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    victim: Option<&H::Entity>,
    attacker: Option<&H::Entity>,
) -> Result<i32> {
    let singleton = singleton(h, r)?;
    base::calculate_defence(h, &singleton, victim, attacker)
}

/// `getSetMultiplier`: the current style's singleton's.
pub fn set_multiplier<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    e: Option<&H::Entity>,
    skill_id: i32,
) -> Result<f64> {
    let singleton = singleton(h, r)?;
    base::set_multiplier(h, &singleton, e, skill_id)
}

/// `CombatSwingHandler.adjustBattleState`, which the class inherits.
pub fn base_adjust<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    entity: &H::Entity,
    victim: &H::Entity,
    state: &mut BattleState<H>,
) -> Result<()> {
    base::base_adjust(h, r, entity, victim, state)
}

/// `CombatSwingHandler.postSwing`: an empty hook.
pub fn no_post_swing<H: Host>(
    _: &mut H,
    _: &H::HandlerRef,
    _: Option<&H::Entity>,
    _: Option<&H::Entity>,
    _: &BattleState<H>,
) -> Result<()> {
    Ok(())
}

/// `CombatSwingHandler.getArmourSet`: null.
pub fn no_armour_set<H: Host>(
    _: &mut H,
    _: &H::HandlerRef,
    _: Option<&H::Entity>,
) -> Result<Option<ArmourSet>> {
    Ok(None)
}
