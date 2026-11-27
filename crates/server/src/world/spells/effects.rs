//! `fireEffect` of the combat spell classes: what a spell does to its victim
//! beyond the hit (`MagicSwingHandler.adjustBattleState` calls it for the
//! state, or for each target of a multi-target state).
//!
//! Ported from the pinned `BindSpell`, `CurseSpells`, `GodSpells`,
//! `TeleblockSpell`, `IceSpells`, `BloodSpells`, `ShadowSpells`,
//! `SmokeSpells` and `MiasmicSpells`; the other classes keep the empty
//! `CombatSpell.fireEffect`. Everything outside the spell object and the
//! battle state is a service of [`Effects`] (or of [`Host`]), called in the
//! original order. No JVM corpus covers these members: the native tests
//! (`tests/spell_effects.rs`) check them against a recording host, written
//! from the source. See docs/spells.md.
use super::{Error, Host, NodeKind, Result, SpellClass, SpellType};
use crate::world::combat_swing::CombatStyle;

/// `Skills.ATTACK`, `DEFENCE`, `STRENGTH`, `MAGIC`.
const ATTACK: i32 = 0;
const DEFENCE: i32 = 1;
const STRENGTH: i32 = 2;
const MAGIC: i32 = 6;

/// One argument of `spawnTimer(identifier, vararg args)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimerArg {
    Int(i32),
    Bool(bool),
}

/// The `BattleState` members `fireEffect` reads and writes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EffectState<S> {
    pub estimated_hit: i32,
    pub frozen: bool,
    /// `state.getSpell()`.
    pub spell: Option<S>,
    /// `state.getStyle()`.
    pub style: Option<CombatStyle>,
}

/// The services `fireEffect` reaches beyond those of [`Host`]. Each is one
/// original call; none has a default.
pub trait Effects: Host {
    /// `entity.getLocks().isMovementLocked()`.
    fn movement_locked(&mut self, entity: &Self::Node) -> Result<bool>;
    /// `entity.getWalkingQueue().reset()`.
    fn reset_walking(&mut self, entity: &Self::Node) -> Result<()>;
    /// `entity.getLocks().lockMovement(ticks)`.
    fn lock_movement(&mut self, entity: &Self::Node, ticks: i32) -> Result<()>;
    /// `entity.getSkills().drainLevel(slot, drain, maximum)`.
    fn drain_level(
        &mut self,
        entity: &Self::Node,
        slot: i32,
        drain: f64,
        maximum: f64,
    ) -> Result<()>;
    /// `entity.getSkills().decrementPrayerPoints(amount)`.
    fn decrement_prayer_points(&mut self, entity: &Self::Node, amount: f64) -> Result<()>;
    /// `entity.getSkills().getStaticLevel(slot)`.
    fn static_level(&mut self, entity: &Self::Node, slot: i32) -> Result<i32>;
    /// `entity.getSkills().updateLevel(slot, amount, maximum)`.
    fn update_level(
        &mut self,
        entity: &Self::Node,
        slot: i32,
        amount: i32,
        maximum: i32,
    ) -> Result<i32>;
    /// `entity.getSkills().heal(amount)`.
    fn heal(&mut self, entity: &Self::Node, amount: i32) -> Result<i32>;
    /// `style.getSwingHandler().isAccurateImpact(entity, victim)`.
    fn accurate_impact(
        &mut self,
        style: CombatStyle,
        entity: &Self::Node,
        victim: &Self::Node,
    ) -> Result<bool>;
    /// `player.getPrayer().get(PrayerType.PROTECT_FROM_MAGIC)`.
    fn protect_from_magic(&mut self, player: &Self::Node) -> Result<bool>;
    /// `registerTimer(entity, spawnTimer(identifier, *args))`: nothing is
    /// registered when the registry has no such timer.
    fn register_timer(
        &mut self,
        entity: &Self::Node,
        identifier: &str,
        args: &[TimerArg],
    ) -> Result<()>;
    /// `applyPoison(victim, source, severity)`.
    fn apply_poison(
        &mut self,
        victim: &Self::Node,
        source: &Self::Node,
        severity: i32,
    ) -> Result<()>;
}

/// `spell.fireEffect(entity, victim, state)`.
pub fn fire_effect<H: Effects>(
    h: &mut H,
    spell: &H::Spell,
    entity: &H::Node,
    victim: Option<&H::Node>,
    state: &mut EffectState<H::Spell>,
) -> Result<()> {
    let s = h.spell(spell).clone();
    match s.class {
        // CombatSpell.fireEffect: empty.
        SpellClass::Air
        | SpellClass::Water
        | SpellClass::Earth
        | SpellClass::Fire
        | SpellClass::CrumbleUndead
        | SpellClass::IbanBlast
        | SpellClass::MagicDart
        | SpellClass::SonicWave
        | SpellClass::Default => Ok(()),
        SpellClass::Bind => {
            if let Some(npc) = victim.filter(|v| h.kind(v) == NodeKind::Npc) {
                let name = h.name(npc)?.ok_or(Error::Null)?;
                if name.contains("impling") {
                    state.estimated_hit = -2;
                }
            }
            if state.estimated_hit == -1 {
                return Ok(());
            }
            let mut tick = 9;
            if s.kind == SpellType::Bind {
                state.estimated_hit = -2;
            }
            let cast = state.spell.clone().ok_or(Error::Null)?;
            let id = h.spell(&cast).spell_id;
            if id == 30 {
                tick = 17;
            } else if id == 56 {
                tick = 25;
            }
            let victim = victim.ok_or(Error::Null)?;
            if !h.movement_locked(victim)? && h.kind(victim) == NodeKind::Player {
                h.packet_message(victim, "A magical force stops you from moving!")?;
            }
            h.reset_walking(victim)?;
            h.lock_movement(victim, tick)?;
            let ticks = h.ticks()?;
            h.set_attribute(
                entity,
                "entangleDelay",
                super::Value::Int(ticks.wrapping_add(tick).wrapping_add(2)),
            )
        }
        SpellClass::Curse => {
            if state.estimated_hit == -1 {
                return Ok(());
            }
            state.estimated_hit = -2;
            let (slot, amount) = match s.kind {
                SpellType::Confuse => (ATTACK, 0.05),
                SpellType::Weaken => (STRENGTH, 0.05),
                SpellType::Curse => (DEFENCE, 0.05),
                SpellType::Vulnerability => (DEFENCE, 0.10),
                SpellType::Enfeeble => (STRENGTH, 0.10),
                SpellType::Stun => (ATTACK, 0.10),
                _ => return Ok(()),
            };
            h.drain_level(victim.ok_or(Error::Null)?, slot, amount, amount)
        }
        SpellClass::God => {
            // getSpellIndex(): by the amount of the second rune.
            let runes = s.runes.as_ref().ok_or(Error::Null)?;
            let second = runes.get(1).ok_or(Error::Index)?;
            match second.1 {
                2 => h.decrement_prayer_points(victim.ok_or(Error::Null)?, 1.0),
                1 => h.drain_level(victim.ok_or(Error::Null)?, DEFENCE, 0.05, 0.05),
                4 => h.drain_level(victim.ok_or(Error::Null)?, MAGIC, 0.05, 0.05),
                _ => Ok(()),
            }
        }
        SpellClass::Teleblock => {
            // hasTimerActive's entity is a non-null Kotlin parameter.
            let victim = victim.ok_or(Error::Null)?;
            if !h.timer_active(victim, "teleblock")?
                && h.kind(victim) == NodeKind::Player
                && h.accurate_impact(state.style.ok_or(Error::Null)?, entity, victim)?
            {
                let mut ticks = 500;
                if h.protect_from_magic(victim)? {
                    ticks /= 2;
                }
                h.register_timer(victim, "teleblock", &[TimerArg::Int(ticks)])
            } else if h.timer_active(victim, "teleblock")? {
                // entity.asPlayer(): a cast.
                if h.kind(entity) != NodeKind::Player {
                    return Err(Error::Cast);
                }
                h.send_message(entity, "Your target is already blocked from teleporting.")
            } else {
                Ok(())
            }
        }
        SpellClass::Ice => {
            if state.estimated_hit == -1 {
                return Ok(());
            }
            let ticks = s.kind.ordinal().wrapping_sub(4).wrapping_mul(8);
            let victim = victim.ok_or(Error::Null)?;
            if h.timer_active(victim, "frozen")? || h.timer_active(victim, "frozen:immunity")? {
                if s.kind == SpellType::Barrage {
                    state.frozen = true;
                }
                return Ok(());
            }
            h.register_timer(
                victim,
                "frozen",
                &[TimerArg::Int(ticks), TimerArg::Bool(true)],
            )
        }
        SpellClass::Blood => {
            if state.estimated_hit > -1 {
                let heal = state.estimated_hit / 4;
                if heal > 0 {
                    h.heal(entity, heal)?;
                    if h.kind(entity) == NodeKind::Player {
                        h.packet_message(entity, "You drain some of your opponent's health.")?;
                    }
                }
            }
            Ok(())
        }
        SpellClass::Shadow => {
            if state.estimated_hit > -1 {
                let victim = victim.ok_or(Error::Null)?;
                let level = h.static_level(victim, ATTACK)?;
                let tenth = f64::from(level) * 0.1;
                h.update_level(
                    victim,
                    ATTACK,
                    (-tenth) as i32,
                    (f64::from(level) - tenth) as i32,
                )?;
            }
            Ok(())
        }
        SpellClass::Smoke => {
            if state.estimated_hit > -1 {
                let severity = if s.kind.ordinal() >= SpellType::Blitz.ordinal() {
                    4
                } else {
                    2
                };
                // applyPoison's entity is a non-null Kotlin parameter.
                h.apply_poison(victim.ok_or(Error::Null)?, entity, severity)?;
            }
            Ok(())
        }
        SpellClass::Miasmic => {
            let victim = victim.ok_or(Error::Null)?;
            if !h.timer_active(victim, "miasmic:immunity")? {
                let ticks = s.spell_id.wrapping_sub(15).wrapping_mul(20);
                h.register_timer(victim, "miasmic", &[TimerArg::Int(ticks)])?;
            }
            Ok(())
        }
    }
}
