//! The answers this library gives to the spell services of
//! [`combat_swing::Host`](crate::world::combat_swing::Host) and
//! [`combat_pulse::Host`](crate::world::combat_pulse::Host), in their shapes
//! and error type. A combat host whose spell handle is this library's
//! `Host::Spell` answers those services by calling these functions.
//!
//! | combat service | answer |
//! | --- | --- |
//! | `spell_meets_requirements` | [`meets_requirements`] |
//! | `spell_maximum_impact` | [`maximum_impact`] |
//! | `spell_targets` | [`targets`] |
//! | `spell_is_blitz` | [`is_blitz`] |
//! | `spell_id` | [`spell_id`] |
//! | `spell_add_experience` | [`add_experience`] |
//! | `spell_visualize` | [`visualize`] |
//! | `spell_visualize_impact` | [`visualize_impact`] |
//! | `select_auto_spell` | [`select_auto_spell`] |
//!
//! `spell_fire_effect` (each spell's `fireEffect`) is a required service of
//! the combat host, which can answer it with [`super::effects::fire_effect`].
//! The manual spell, the autocast spell and `same_spell` are the combat
//! host's own state.
use super::{Host, SpellType, StateView};
use crate::world::combat_swing::{self as cs, BattleState, Types};

/// `spell.meetsRequirements(caster, message, remove)`.
pub fn meets_requirements<H: Host>(
    h: &mut H,
    spell: &H::Spell,
    caster: &H::Node,
    message: bool,
    remove: bool,
) -> cs::Result<bool> {
    Ok(super::meets_requirements(
        h, spell, caster, message, remove,
    )?)
}

/// `spell.getMaximumImpact(entity, victim, state)`; no spell reads the state.
pub fn maximum_impact<H: Host>(
    h: &mut H,
    spell: &H::Spell,
    entity: &H::Node,
    victim: Option<&H::Node>,
) -> cs::Result<i32> {
    Ok(super::maximum_impact(h, spell, Some(entity), victim)?)
}

/// `spell.getTargets(entity, victim)`: fresh `BattleState(entity, target)`
/// values; the array is never null and holds no null element.
#[allow(clippy::type_complexity)]
pub fn targets<H: Host, T: Types<Entity = H::Node>>(
    h: &mut H,
    spell: &H::Spell,
    entity: &H::Node,
    victim: Option<&H::Node>,
) -> cs::Result<Option<Vec<Option<BattleState<T>>>>> {
    let list = super::get_targets(h, spell, entity, victim)?;
    Ok(Some(
        list.into_iter()
            .map(|(e, v)| Some(BattleState::new(Some(e), v)))
            .collect(),
    ))
}

/// `spell.getType() === SpellType.BLITZ`.
pub fn is_blitz<H: Host>(h: &mut H, spell: &H::Spell) -> bool {
    h.spell(spell).kind == SpellType::Blitz
}

/// `spell.getSpellId()`.
pub fn spell_id<H: Host>(h: &mut H, spell: &H::Spell) -> i32 {
    h.spell(spell).spell_id
}

/// `spell.addExperience(entity, hit)`.
pub fn add_experience<H: Host>(
    h: &mut H,
    spell: &H::Spell,
    entity: &H::Node,
    hit: i32,
) -> cs::Result<()> {
    Ok(super::add_experience(h, spell, entity, hit)?)
}

/// `spell.visualize(entity, victim)`.
pub fn visualize<H: Host>(
    h: &mut H,
    spell: &H::Spell,
    entity: &H::Node,
    victim: Option<&H::Node>,
) -> cs::Result<()> {
    Ok(super::visualize(h, spell, entity, victim)?)
}

/// `spell.visualizeImpact(entity, victim, state)`: reads the state's
/// estimated hit and frozen flag and writes nothing to it.
pub fn visualize_impact<H: Host, T: Types>(
    h: &mut H,
    spell: &H::Spell,
    entity: Option<&H::Node>,
    victim: Option<&H::Node>,
    state: &BattleState<T>,
) -> cs::Result<()> {
    let view = StateView {
        estimated_hit: state.estimated_hit,
        frozen: state.frozen,
    };
    Ok(super::visualize_impact(h, spell, entity, victim, view)?)
}

/// `WeaponInterface.selectAutoSpell(button, adjustAttackStyle, forgetAutocast)`.
pub fn select_auto_spell<H: Host>(
    h: &mut H,
    player: &H::Node,
    button: i32,
    adjust_attack_style: bool,
    forget_autocast: bool,
) -> cs::Result<()> {
    Ok(super::autocast::select_auto_spell(
        h,
        player,
        button,
        adjust_attack_style,
        forget_autocast,
    )?)
}
