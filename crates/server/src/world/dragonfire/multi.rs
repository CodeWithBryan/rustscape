//! `MultiSwingHandler`: a direct subclass of `CombatSwingHandler` (type
//! RANGE until a swing sets it) that picks one of its `SwitchAttack`s per
//! swing, and the three MixedSwingHandlers.kt objects.
use super::{
    DragonProjectile, Host, Multi, Subclass, SwitchAttack, attack_style, random, randomize,
};
use crate::world::combat_pulse::{handler as base, impact};
use crate::world::combat_swing::{
    self as cs, BattleState, CombatStyle, Error, Handler, HandlerKind, InteractionType, Result,
    Target,
};

/// One of the MixedSwingHandlers.kt objects.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mixed {
    /// Ranged offence against magic defence.
    MagicalRanged,
    /// Melee offence against magic defence.
    MagicalMelee,
    /// Melee offence against ranged defence.
    RangedMelee,
}

impl Mixed {
    pub const ALL: [Self; 3] = [Self::MagicalRanged, Self::MagicalMelee, Self::RangedMelee];

    pub fn class_name(self) -> &'static str {
        match self {
            Self::MagicalRanged => "MagicalRangedSwingHandler",
            Self::MagicalMelee => "MagicalMeleeSwingHandler",
            Self::RangedMelee => "RangedMeleeSwingHandler",
        }
    }

    /// The object's base fields: its class (`RangeSwingHandler()` or
    /// `MeleeSwingHandler()`, no flags) and the singleton whose
    /// `calculateDefence` it answers with.
    pub fn instance(self) -> Handler {
        match self {
            Self::MagicalRanged => Handler::mixed(HandlerKind::Range, HandlerKind::Magic),
            Self::MagicalMelee => Handler::mixed(HandlerKind::Melee, HandlerKind::Magic),
            Self::RangedMelee => Handler::mixed(HandlerKind::Melee, HandlerKind::Range),
        }
    }
}

/// The instance's fields.
fn fields<H: Host>(h: &H, r: &H::HandlerRef) -> Result<Multi<H>> {
    match h.subclass(r) {
        Some(Subclass::Multi(m)) => Ok(m.clone()),
        _ => Err(Error::Binding("not a MultiSwingHandler".into())),
    }
}

fn fields_mut<'a, H: Host>(h: &'a mut H, r: &H::HandlerRef) -> Result<&'a mut Multi<H>> {
    match h.subclass_mut(r) {
        Some(Subclass::Multi(m)) => Ok(m),
        _ => Err(Error::Binding("not a MultiSwingHandler".into())),
    }
}

/// The attack in `current`.
fn current<H: Host>(h: &H, r: &H::HandlerRef) -> Result<SwitchAttack<H>> {
    let m = fields(h, r)?;
    m.attacks.get(m.current).cloned().ok_or(Error::Index)
}

/// `current.handler`, failing on a null handler as the virtual call does.
fn current_handler<H: Host>(h: &H, r: &H::HandlerRef) -> Result<H::HandlerRef> {
    current(h, r)?.handler.ok_or(Error::Null)
}

/// `new MultiSwingHandler(meleeDistance, attacks...)`; the vararg-only
/// constructor passes `true`. The first attack is current and next, and
/// melee distance is dropped when the only attack is melee.
pub fn new<H: Host>(
    h: &mut H,
    melee_distance: bool,
    attacks: Vec<SwitchAttack<H>>,
) -> Result<H::HandlerRef> {
    let first = attacks.first().ok_or(Error::Index)?;
    let only_melee = attacks.len() == 1 && attack_style(h, first)? == Some(CombatStyle::Melee);
    let made = Multi {
        attacks,
        melee_distance: melee_distance && !only_melee,
        current: 0,
        next: 0,
    };
    Ok(h.new_handler(Handler::new(HandlerKind::Range, &[]), Subclass::Multi(made)))
}

/// `canSwing`: the range singleton's in melee distance mode, otherwise the
/// next attack's handler's.
pub fn can_swing<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    entity: &H::Entity,
    victim: &H::Entity,
) -> Result<InteractionType> {
    let m = fields(h, r)?;
    if m.melee_distance {
        let range = h.style_handler(CombatStyle::Range);
        return base::can_swing(h, &range, entity, victim);
    }
    let next = m.attacks.get(m.next).ok_or(Error::Index)?;
    let handler = next.handler.clone().ok_or(Error::Null)?;
    base::can_swing(h, &handler, entity, victim)
}

/// `swing(entity, victim, state)` of any handler instance: the overriding
/// body, or the base class's on a copy of its fields written back after.
fn virtual_swing<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    entity: Option<&H::Entity>,
    victim: Option<&H::Entity>,
    state: &mut BattleState<H>,
) -> Result<i32> {
    if let Some(member) = h.overrides(r).and_then(|o| o.swing) {
        return member(h, r, entity, victim, state);
    }
    let mut instance = h.handler(r).clone();
    let delay = instance.swing(h, entity, victim, state);
    *h.handler(r) = instance;
    delay
}

/// `getNext(entity, victim, state, index)`: the attack at the drawn index,
/// drawing again while it cannot be selected.
fn next_index<H: Host>(h: &mut H, attacks: &[SwitchAttack<H>], index: i32) -> Result<usize> {
    let mut at = usize::try_from(index).map_err(|_| Error::Index)?;
    while !attacks.get(at).ok_or(Error::Index)?.can_select() {
        let size = i32::try_from(attacks.len()).map_err(|_| Error::Index)?;
        at = usize::try_from(randomize(h, size)?).map_err(|_| Error::Index)?;
    }
    Ok(at)
}

/// `swing(entity, victim, state)`: the next attack becomes current (the first
/// non-melee attack instead when melee is out of reach in melee distance
/// mode), the handler's type becomes its style, the next attack is drawn with
/// `randomize(attacks.size)`; an attack that uses its handler swings with it,
/// otherwise an accuracy roll and `random(max)` with `1 + ceil(distance *
/// 0.5 or 0.3)` ticks for a non-melee style.
pub fn swing<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    entity: Option<&H::Entity>,
    victim: Option<&H::Entity>,
    state: &mut BattleState<H>,
) -> Result<i32> {
    let m = fields(h, r)?;
    fields_mut(h, r)?.current = m.next;
    let current = m.attacks.get(m.next).ok_or(Error::Index)?;
    if m.melee_distance && attack_style(h, current)? == Some(CombatStyle::Melee) {
        let melee = h.style_handler(CombatStyle::Melee);
        let (e, v) = (entity.ok_or(Error::Null)?, victim.ok_or(Error::Null)?);
        if base::can_swing(h, &melee, e, v)? != InteractionType::StillInteract {
            for (at, attack) in m.attacks.iter().enumerate() {
                if attack_style(h, attack)? != Some(CombatStyle::Melee) {
                    fields_mut(h, r)?.current = at;
                    break;
                }
            }
        }
    }
    let current = self::current(h, r)?;
    let style = attack_style(h, &current)?;
    h.handler(r).style = style;
    let size = i32::try_from(m.attacks.len()).map_err(|_| Error::Index)?;
    let index = randomize(h, size)?;
    let pick = next_index(h, &m.attacks, index)?;
    fields_mut(h, r)?.next = pick;
    if current.use_handler {
        let handler = current.handler.clone().ok_or(Error::Null)?;
        return virtual_swing(h, &handler, entity, victim, state);
    }
    let mut ticks: i32 = 1;
    if style != Some(CombatStyle::Melee) {
        let e = entity.ok_or(Error::Null)?;
        let here = h.location(e)?.ok_or(Error::Null)?;
        let there = h.location(victim.ok_or(Error::Null)?)?.ok_or(Error::Null)?;
        let factor = if h.handler(r).style == Some(CombatStyle::Magic) {
            0.5
        } else {
            0.3
        };
        ticks = ticks.wrapping_add((cs::distance(here, there) * factor).ceil() as i32);
    }
    let mut hit = 0;
    if base::is_accurate_impact(h, r, entity, victim, style, 1.0, 1.0)? {
        let max = base::calculate_hit(h, r, entity, victim, 1.0)?;
        state.maximum_hit = max;
        hit = random(h, max)?;
    }
    state.estimated_hit = hit;
    state.style = style;
    Ok(ticks)
}

/// `visualize`: the current attack's handler's when it uses its handler,
/// otherwise its animation and start graphic and its projectile transformed
/// from the entity (speed modifier 10 for magic, 5 otherwise).
pub fn visualize<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    entity: &H::Entity,
    victim: Option<&H::Entity>,
    state: &BattleState<H>,
) -> Result<()> {
    let current = current(h, r)?;
    if current.use_handler {
        let handler = current.handler.clone().ok_or(Error::Null)?;
        return base::visualize(h, &handler, entity, victim, state);
    }
    h.visualize_entity(entity, current.animation.as_ref(), current.start_graphic)?;
    if let Some(template) = &current.projectile {
        let npc = h.kind(entity).is_npc();
        let modified_speed = if attack_style(h, &current)? == Some(CombatStyle::Magic) {
            10
        } else {
            5
        };
        h.send_dragon_projectile(DragonProjectile::Transform {
            template,
            source: entity,
            victim,
            npc,
            base_speed: 46,
            modified_speed,
        })?;
    }
    Ok(())
}

/// `impact`: the current attack's handler's when it uses its handler,
/// otherwise both hits of each target state (the state alone without
/// targets) in the current attack's style against the victim.
pub fn impact<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    entity: Option<&H::Entity>,
    victim: Option<&H::Entity>,
    state: &mut BattleState<H>,
) -> Result<()> {
    let current = current(h, r)?;
    if current.use_handler {
        let handler = current.handler.clone().ok_or(Error::Null)?;
        return base::impact(h, &handler, entity, victim, state);
    }
    let hits: Vec<Option<(i32, i32)>> = match &state.targets {
        None => vec![Some((state.estimated_hit, state.secondary_hit))],
        Some(list) => list
            .iter()
            .map(|target| match target {
                None => None,
                Some(Target::This) => Some((state.estimated_hit, state.secondary_hit)),
                Some(Target::State(s)) => Some((s.estimated_hit, s.secondary_hit)),
            })
            .collect(),
    };
    for pair in hits {
        let (estimated, secondary) = pair.ok_or(Error::Null)?;
        for hit in [estimated, secondary] {
            if hit > -1 {
                let victim = victim.ok_or(Error::Null)?;
                let style = attack_style(h, &current)?;
                impact::handle_impact(h, victim, entity, hit, style, None, false)?;
            }
        }
    }
    Ok(())
}

/// `adjustBattleState`: the current attack's handler's when it uses its
/// handler, otherwise `CombatSwingHandler.adjustBattleState`.
pub fn adjust_battle_state<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    entity: &H::Entity,
    victim: &H::Entity,
    state: &mut BattleState<H>,
) -> Result<()> {
    let current = current(h, r)?;
    if current.use_handler {
        let handler = current.handler.clone().ok_or(Error::Null)?;
        return base::adjust_battle_state(h, &handler, entity, victim, state);
    }
    base::base_adjust(h, r, entity, victim, state)
}

/// `addExperience`: the current attack's handler's.
pub fn add_experience<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    entity: Option<&H::Entity>,
    victim: Option<&H::Entity>,
    state: Option<&BattleState<H>>,
) -> Result<()> {
    let handler = current_handler(h, r)?;
    base::add_experience(h, &handler, entity, victim, state)
}

/// `postSwing`: the current attack's handler's.
pub fn post_swing<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    entity: Option<&H::Entity>,
    victim: Option<&H::Entity>,
    state: &BattleState<H>,
) -> Result<()> {
    let handler = current_handler(h, r)?;
    base::post_swing(h, &handler, entity, victim, state)
}

/// `visualizeImpact`: the current attack's handler's when it uses its
/// handler, otherwise the victim's defence animation with the end graphic.
pub fn visualize_impact<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    entity: Option<&H::Entity>,
    victim: Option<&H::Entity>,
    state: &mut BattleState<H>,
) -> Result<()> {
    let current = current(h, r)?;
    if current.use_handler {
        let handler = current.handler.clone().ok_or(Error::Null)?;
        return base::visualize_impact(h, &handler, entity, victim, state);
    }
    let victim = victim.ok_or(Error::Null)?;
    let animation = h.defence_animation(victim)?;
    h.visualize_entity(victim, animation.as_ref(), current.end_graphic)
}

pub fn calculate_accuracy<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    entity: Option<&H::Entity>,
) -> Result<i32> {
    let handler = current_handler(h, r)?;
    base::calculate_accuracy(h, &handler, entity)
}

/// `calculateHit`: the current attack's maximum hit when set, otherwise its
/// handler's.
pub fn calculate_hit<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    entity: Option<&H::Entity>,
    victim: Option<&H::Entity>,
    modifier: f64,
) -> Result<i32> {
    let current = current(h, r)?;
    if current.maximum_hit > -1 {
        return Ok(current.maximum_hit);
    }
    let handler = current.handler.ok_or(Error::Null)?;
    base::calculate_hit(h, &handler, entity, victim, modifier)
}

pub fn calculate_defence<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    victim: Option<&H::Entity>,
    attacker: Option<&H::Entity>,
) -> Result<i32> {
    let handler = current_handler(h, r)?;
    base::calculate_defence(h, &handler, victim, attacker)
}

pub fn set_multiplier<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    e: Option<&H::Entity>,
    skill_id: i32,
) -> Result<f64> {
    let handler = current_handler(h, r)?;
    base::set_multiplier(h, &handler, e, skill_id)
}
