//! `CombatPulse`: the per-tick combat state machine of one entity, the
//! companion `swing` that runs one attack, and the pulses combat submits to
//! the world runner. The base `Pulse` fields and `update` these classes
//! inherit are kept here beside their own fields.
use super::{HitsplatType, Host, handler, impact, slot};
use crate::world::combat_swing::{
    self as cs, BattleState, CombatStyle, Error, InteractionType, Kind, Result,
};

/// The fields of one `CombatPulse`, including the `Pulse` fields it inherits.
pub struct PulseState<H: Host> {
    /// `Pulse.running`; a combat pulse is created stopped.
    pub running: bool,
    /// `Pulse.delay`.
    pub delay: i32,
    /// `Pulse.ticksPassed`.
    pub ticks_passed: i32,
    /// `Pulse.checks`: the entity, then the current victim.
    pub checks: [Option<H::Entity>; 2],
    pub victim: Option<H::Entity>,
    pub style: CombatStyle,
    pub temporary_handler: Option<H::HandlerRef>,
    /// `handler`: initialised to the melee singleton and only ever read or
    /// written from outside the class.
    pub handler: H::HandlerRef,
    pub last_victim: Option<H::Entity>,
    pub next_attack: i32,
    pub combat_time_out: i32,
    pub last_sent_attack: i32,
    pub last_received_attack: i32,
}

impl<H: Host> PulseState<H> {
    /// `CombatPulse(entity)`. `melee` is `CombatStyle.MELEE.swingHandler`.
    pub fn new(entity: H::Entity, melee: H::HandlerRef) -> Self {
        Self {
            running: false,
            delay: 1,
            ticks_passed: 0,
            checks: [Some(entity), None],
            victim: None,
            style: CombatStyle::Melee,
            temporary_handler: None,
            handler: melee,
            last_victim: None,
            next_attack: -1,
            combat_time_out: 0,
            last_sent_attack: 0,
            last_received_attack: 0,
        }
    }
}

/// What a pulse submitted by combat does when it fires.
pub enum Task<H: Host> {
    /// The anonymous pulse of `CombatPulse.swing`: delivers the impact.
    Impact {
        handler: H::HandlerRef,
        entity: H::Entity,
        victim: H::Entity,
        state: Box<BattleState<H>>,
        /// Its `impact` field: the impact visuals have been shown.
        impact: bool,
    },
    /// The anonymous pulse of `RangeSwingHandler.impact`: the second arrow.
    SecondHit {
        entity: Option<H::Entity>,
        victim: H::Entity,
        hit: i32,
    },
    /// The anonymous pulse of the delayed `ImpactHandler.manualHit`.
    ManualHit {
        owner: H::Entity,
        source: Option<H::Entity>,
        hit: i32,
        kind: Option<HitsplatType>,
    },
}

/// A pulse handed to `GameWorld.Pulser.submit`, with its `Pulse` fields.
pub struct WorldPulse<H: Host> {
    pub running: bool,
    pub delay: i32,
    pub ticks_passed: i32,
    pub checks: Vec<Option<H::Entity>>,
    pub task: Task<H>,
}

impl<H: Host> WorldPulse<H> {
    /// `Pulse(delay, checks...)`.
    pub fn new(delay: i32, checks: Vec<Option<H::Entity>>, task: Task<H>) -> Self {
        Self {
            running: true,
            delay,
            ticks_passed: 0,
            checks,
            task,
        }
    }

    /// `Pulse.update()`: true when the runner must drop the pulse. A failure
    /// inside the body is printed and stops the pulse, as the original's
    /// `catch (Exception)` does.
    pub fn update(&mut self, h: &mut H) -> Result<bool> {
        for at in 0..self.checks.len() {
            if let Some(node) = self.checks[at].clone() {
                if !h.is_active(&node)? {
                    self.running = false;
                    return Ok(true);
                }
            }
        }
        if !self.running {
            return Ok(true);
        }
        self.ticks_passed = self.ticks_passed.wrapping_add(1);
        if self.ticks_passed >= self.delay {
            self.ticks_passed = 0;
            match self.body(h) {
                Ok(true) => {
                    self.running = false;
                    return Ok(true);
                }
                Ok(false) => {}
                Err(error) => {
                    h.print_stack_trace(&error)?;
                    self.running = false;
                    return Ok(true);
                }
            }
            return Ok(!self.running);
        }
        Ok(false)
    }

    /// `pulse()` of the anonymous class.
    fn body(&mut self, h: &mut H) -> Result<bool> {
        let Self { delay, task, .. } = self;
        match task {
            Task::Impact {
                handler: r,
                entity,
                victim,
                state,
                impact: shown,
            } => {
                let state: &mut BattleState<H> = state;
                if h.is_dead(victim)? || h.is_dead(entity)? {
                    return Ok(true);
                }
                if h.kind(entity).is_npc() {
                    h.before_attack_finalized(entity, victim, state)?;
                }
                if *shown || *delay == 0 {
                    if state.estimated_hit != 0 && h.kind(victim).is_npc() {
                        if let Some(id) = h.npc_audio(victim, 1)? {
                            let at = h.location(victim)?;
                            h.play_global_audio(at.ok_or(Error::Null)?, id)?;
                        }
                    } else if state.estimated_hit != 0 && h.kind(victim).is_player() {
                        h.play_hurt_audio(victim)?;
                    }
                    handler::impact(h, r, Some(entity), Some(victim), state)?;
                    handler::on_impact(h, Some(entity), Some(victim), Some(state))?;
                    return Ok(true);
                }
                *delay = 1;
                *shown = true;
                handler::visualize_impact(h, r, Some(entity), Some(victim), state)?;
                Ok(false)
            }
            Task::SecondHit {
                entity,
                victim,
                hit,
            } => {
                let style = Some(CombatStyle::Range);
                impact::handle_impact(h, victim, entity.as_ref(), *hit, style, None, false)?;
                Ok(true)
            }
            Task::ManualHit {
                owner,
                source,
                hit,
                kind,
            } => {
                impact::manual_hit(h, owner, source.as_ref(), *hit, *kind)?;
                Ok(true)
            }
        }
    }
}

/// `Pulse.hasInactiveNode()` over the combat pulse's live check array.
fn has_inactive_node<H: Host>(h: &mut H, entity: &H::Entity) -> Result<bool> {
    for at in 0..2 {
        if let Some(node) = h.combat_pulse(entity).checks[at].clone() {
            if !h.is_active(&node)? {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

/// `Pulse.update()` of the combat pulse: what the runner calls once a tick.
pub fn update<H: Host>(h: &mut H, entity: &H::Entity) -> Result<bool> {
    if has_inactive_node(h, entity)? {
        stop(h, entity)?;
        return Ok(true);
    }
    if !h.combat_pulse(entity).running {
        return Ok(true);
    }
    let state = h.combat_pulse(entity);
    state.ticks_passed = state.ticks_passed.wrapping_add(1);
    if state.ticks_passed >= state.delay {
        state.ticks_passed = 0;
        let body = (|| -> Result<bool> {
            if pulse(h, entity)? {
                stop(h, entity)?;
                return Ok(true);
            }
            Ok(false)
        })();
        match body {
            Ok(true) => return Ok(true),
            Ok(false) => {}
            Err(error) => {
                h.print_stack_trace(&error)?;
                stop(h, entity)?;
                return Ok(true);
            }
        }
        return Ok(!h.combat_pulse(entity).running);
    }
    Ok(false)
}

/// `Location.regionId`.
fn region_id(at: [i32; 3]) -> i32 {
    ((at[0] >> 6) << 8) | (at[1] >> 6)
}

/// `CombatPulse.pulse()`.
pub fn pulse<H: Host>(h: &mut H, entity: &H::Entity) -> Result<bool> {
    let Some(victim) = h.combat_pulse(entity).victim.clone() else {
        return Ok(true);
    };
    if h.is_dead(entity)? || h.is_dead(&victim)? {
        return Ok(true);
    }
    let here = h.location(entity)?.ok_or(Error::Null)?;
    if !h.region_active(region_id(here))? {
        return Ok(true);
    }
    let victim = h.combat_pulse(entity).victim.clone().ok_or(Error::Null)?;
    let there = h.location(&victim)?.ok_or(Error::Null)?;
    if !h.region_active(region_id(there))? {
        return Ok(true);
    }
    if !interactable(h, entity)? {
        if h.is_moving(entity)? {
            return Ok(false);
        }
        let state = h.combat_pulse(entity);
        let waited = state.combat_time_out;
        state.combat_time_out = waited.wrapping_add(1);
        return Ok(waited > h.combat_time_out(entity)?);
    }
    h.combat_pulse(entity).combat_time_out = 0;
    let victim = h.combat_pulse(entity).victim.clone();
    h.face(entity, victim.as_ref())?;
    let next_attack = h.combat_pulse(entity).next_attack;
    if next_attack <= h.ticks()? {
        let Some(v) = h.combat_pulse(entity).victim.clone() else {
            return Ok(false);
        };
        let r = match h.combat_pulse(entity).temporary_handler.clone() {
            Some(temporary) => temporary,
            None => h.swing_handler(entity, true)?,
        };
        let style = h.handler(&r).style;
        if !h.entity_attackable(&v, entity, style, true)? {
            let event = h.random_event_npc(&v)?;
            if !event.is_some_and(|npc| h.same_entity(entity, &npc)) {
                return Ok(true);
            }
        }
        let victim = h.combat_pulse(entity).victim.clone();
        if !swing(h, entity, victim.as_ref(), &r)? {
            h.combat_pulse(entity).temporary_handler = None;
            update_style(h, entity)?;
            return Ok(false);
        }
        let mut speed = h.attack_speed(entity)?;
        let magic = h.handler(&r).style == Some(CombatStyle::Magic);
        let salamander = h.is_salamander(&r);
        if h.kind(entity).is_player() && magic {
            speed = 5;
        } else if h.attack_style(entity)?.ok_or(Error::Null)?.style == cs::STYLE_RAPID
            || (salamander
                && h.attack_style(entity)?.ok_or(Error::Null)?.style == cs::STYLE_RANGE_ACCURATE)
        {
            speed = speed.wrapping_sub(1);
        }
        if !magic && h.miasmic_active(entity)? {
            speed = (f64::from(speed) * 1.5) as i32;
        }
        set_next_attack(h, entity, speed)?;
        h.combat_pulse(entity).temporary_handler = None;
        set_combat_flags(h, entity, Some(&v))?;
    }
    if let Some(victim) = h.combat_pulse(entity).victim.clone() {
        if h.lifepoints(&victim)? < 1 {
            return Ok(true);
        }
    }
    Ok(h.lifepoints(entity)? < 1)
}

/// `setCombatFlags(victim)`.
pub fn set_combat_flags<H: Host>(
    h: &mut H,
    entity: &H::Entity,
    victim: Option<&H::Entity>,
) -> Result<()> {
    let Some(victim) = victim else {
        return Ok(());
    };
    if h.kind(entity).is_player() && !h.has_attribute(entity, "keepDialogueAlive")? {
        h.interface_close(entity)?;
        h.interface_close_chatbox(entity)?;
    }
    if h.kind(victim).is_player() {
        if h.kind(entity).is_player() && h.is_wilderness(entity)? {
            h.check_skull(entity, victim)?;
        }
        if !h.has_attribute(victim, "keepDialogueAlive")? {
            h.interface_close_chatbox(victim)?;
            h.interface_close(victim)?;
        }
    }
    if !h.pulse_manager_is_moving(victim)? {
        h.pulse_manager_clear(victim)?;
    }
    let until = h.current_time_millis()?.wrapping_add(10000);
    h.set_combat_time(victim, until)?;
    h.set_combat_attacker(victim, entity)
}

/// `interactable()`.
fn interactable<H: Host>(h: &mut H, entity: &H::Entity) -> Result<bool> {
    let Some(victim) = h.combat_pulse(entity).victim.clone() else {
        return Ok(false);
    };
    let (ours, theirs) = (h.kind(entity), h.kind(&victim));
    if ours.is_npc() && theirs.is_player() && h.npc_hidden(entity, &victim)? {
        stop(h, entity)?;
        return Ok(false);
    }
    if theirs.is_npc() && ours.is_player() && h.npc_hidden(&victim, entity)? {
        stop(h, entity)?;
        return Ok(false);
    }
    if ours.is_npc() && !h.npc_can_start_combat(entity, &victim)? {
        stop(h, entity)?;
        return Ok(false);
    }
    let kind = can_interact(h, entity)?;
    if kind == InteractionType::StillInteract {
        return Ok(true);
    }
    if h.combat_pulse(entity).victim.is_none() || h.movement_locked(entity)? {
        return Ok(false);
    }
    h.movement_update_path(entity)?;
    Ok(kind == InteractionType::MoveInteract)
}

/// `updateStyle()`.
pub fn update_style<H: Host>(h: &mut H, entity: &H::Entity) -> Result<()> {
    if h.kind(entity).is_player() {
        if h.spell(entity)?.is_some() || h.autocast_spell(entity)?.is_some() {
            h.combat_pulse(entity).style = CombatStyle::Magic;
            return Ok(());
        }
        let style = match h.attack_style(entity)?.ok_or(Error::Null)?.bonus_type {
            cs::BONUS_MAGIC => CombatStyle::Magic,
            cs::BONUS_RANGE => CombatStyle::Range,
            _ => CombatStyle::Melee,
        };
        h.combat_pulse(entity).style = style;
    }
    Ok(())
}

/// `attack(victim)`.
pub fn attack<H: Host>(h: &mut H, entity: &H::Entity, victim: Option<&H::Entity>) -> Result<()> {
    let Some(victim) = victim else {
        return Ok(());
    };
    if h.interaction_locked(entity)? {
        return Ok(());
    }
    let current = h.combat_pulse(entity).victim.clone();
    let same = current.is_some_and(|c| h.same_entity(&c, victim));
    if same && is_attacking(h, entity)? {
        return Ok(());
    }
    if h.kind(victim).is_player() && (h.id(entity)? == 4474 || h.id(entity)? == 7891) {
        return Ok(());
    }
    if h.kind(entity).is_player()
        && (h.id(victim)? == 4474 || h.id(victim)? == 7891)
        && h.combat_level(entity)? >= 30
    {
        h.player_send_message(
            entity,
            "You are too experienced to gain anything from these.",
        )?;
        return Ok(());
    }
    if h.kind(victim).is_npc() {
        let last = h.combat_pulse(entity).last_victim.clone();
        if h.kind(entity).is_player() && !same && !last.is_some_and(|l| h.same_entity(&l, victim)) {
            if let Some(mask) = h.equipment(entity, slot::HAT)? {
                let id = h.item_id(&mask);
                if (8901..8920).contains(&id) && random(h, 50)? == 0 {
                    let charges = if id == 8919 {
                        "no".to_owned()
                    } else {
                        ((8920 - id) / 2).to_string()
                    };
                    h.send_message(
                        entity,
                        &format!(
                            "Your black mask startles your enemy, you have {charges} charges left."
                        ),
                    )?;
                    h.equipment_replace(entity, Some((id + 2, 1, 1000)), slot::HAT)?;
                    let mut drain = 3 + h.level(victim, cs::skill::DEFENCE)? / 14;
                    if drain > 10 {
                        drain = 10;
                    }
                    let maximum = h
                        .static_level(victim, cs::skill::DEFENCE)?
                        .wrapping_sub(drain);
                    h.update_level(victim, cs::skill::DEFENCE, -drain, maximum)?;
                }
            }
        }
        if !h.movement_locked(victim)? {
            h.walking_reset(victim)?;
        }
    }
    set_victim(h, entity, Some(victim))?;
    h.on_attack(entity, victim)?;
    if is_attacking(h, entity)? {
        h.remove_weak_scripts(victim)?;
    }
    if !is_attacking(h, entity)? {
        h.pulse_manager_run(entity)?;
    }
    Ok(())
}

/// `RandomFunction.random(maxValue)`.
fn random<H: Host>(h: &mut H, max_value: i32) -> Result<i32> {
    if max_value <= 0 {
        return Ok(0);
    }
    h.next_int(max_value)
}

/// `setVictim(victim)`.
pub fn set_victim<H: Host>(
    h: &mut H,
    entity: &H::Entity,
    victim: Option<&H::Entity>,
) -> Result<()> {
    h.combat_pulse(entity).checks[1] = victim.cloned();
    h.movement_set_last(entity)?;
    h.movement_set_destination(entity, victim)?;
    let state = h.combat_pulse(entity);
    state.victim = victim.cloned();
    state.combat_time_out = 0;
    Ok(())
}

/// `setNextAttack(ticks)`.
pub fn set_next_attack<H: Host>(h: &mut H, entity: &H::Entity, ticks: i32) -> Result<()> {
    let now = h.ticks()?;
    h.combat_pulse(entity).next_attack = now.wrapping_add(ticks);
    Ok(())
}

/// `delayNextAttack(ticks)`.
pub fn delay_next_attack<H: Host>(h: &mut H, entity: &H::Entity, ticks: i32) {
    let state = h.combat_pulse(entity);
    state.next_attack = state.next_attack.wrapping_add(ticks);
}

/// `canInteract()`.
pub fn can_interact<H: Host>(h: &mut H, entity: &H::Entity) -> Result<InteractionType> {
    if h.combat_pulse(entity).victim.is_none() {
        return Ok(InteractionType::NoInteract);
    }
    let r = match h.combat_pulse(entity).temporary_handler.clone() {
        Some(temporary) => temporary,
        None => h.swing_handler(entity, false)?,
    };
    let victim = h.combat_pulse(entity).victim.clone().ok_or(Error::Null)?;
    handler::can_swing(h, &r, entity, &victim)
}

/// `start()`: what the pulse manager calls when it runs the pulse.
pub fn start<H: Host>(h: &mut H, entity: &H::Entity) -> Result<()> {
    h.combat_pulse(entity).running = true;
    let victim = h.combat_pulse(entity).victim.clone();
    h.face(entity, victim.as_ref())?;
    h.walking_reset(entity)
}

/// `stop()`.
pub fn stop<H: Host>(h: &mut H, entity: &H::Entity) -> Result<()> {
    h.combat_pulse(entity).running = false;
    let now = h.ticks()?;
    h.set_combat_stop(entity, now)?;
    let state = h.combat_pulse(entity);
    if state.victim.is_some() {
        state.last_victim = state.victim.clone();
    }
    state.victim = None;
    state.checks[1] = None;
    h.face(entity, None)?;
    h.clear_spell(entity)
}

/// `isAttacking`.
pub fn is_attacking<H: Host>(h: &mut H, entity: &H::Entity) -> Result<bool> {
    let Some(victim) = h.combat_pulse(entity).victim.clone() else {
        return Ok(false);
    };
    Ok(h.is_active(&victim)? && h.combat_pulse(entity).running)
}

/// `isInCombat`.
pub fn is_in_combat<H: Host>(h: &mut H, entity: &H::Entity) -> Result<bool> {
    match h.combat_attacker(entity)? {
        Some(attacker) => is_attacking(h, &attacker),
        None => Ok(false),
    }
}

/// The companion `swing(entity, victim, handler)`: one attack, from the
/// battle state to the submitted impact pulse. False when the handler's
/// swing refuses.
pub fn swing<H: Host>(
    h: &mut H,
    entity: &H::Entity,
    victim: Option<&H::Entity>,
    r: &H::HandlerRef,
) -> Result<bool> {
    let mut state = BattleState::new(Some(entity.clone()), victim.cloned());
    let set = handler::armour_set(h, r, Some(entity))?;
    h.set_armour_set(entity, set)?;
    let delay = match h.overrides(r).and_then(|o| o.swing) {
        Some(swing) => swing(h, r, Some(entity), victim, &mut state),
        None => {
            let mut instance = h.handler(r).clone();
            let delay = instance.swing(h, Some(entity), victim, &mut state);
            *h.handler(r) = instance;
            delay
        }
    };
    let delay = delay?;
    if delay < 0 {
        return Ok(false);
    }
    if victim.is_none() {
        h.face_temporary(entity, None, 1)?;
    }
    let victim = victim.ok_or(Error::Null)?;
    handler::adjust_battle_state(h, r, entity, victim, &mut state)?;
    handler::add_experience(h, r, Some(entity), Some(victim), Some(&state))?;
    handler::post_swing(h, r, Some(entity), Some(victim), &state)?;
    handler::visualize(h, r, entity, Some(victim), &state)?;
    if delay.wrapping_sub(1) < 1 {
        handler::visualize_impact(h, r, Some(entity), Some(victim), &mut state)?;
    }
    handler::visualize_audio(h, r, entity, &state)?;
    if let Some(set) = set {
        if handler::armour_set_effect(h, set, entity, victim, &state)? {
            handler::armour_set_visualize(h, set, victim)?;
        }
    }
    h.submit(WorldPulse::new(
        delay.wrapping_sub(1),
        vec![Some(entity.clone()), Some(victim.clone())],
        Task::Impact {
            handler: r.clone(),
            entity: entity.clone(),
            victim: victim.clone(),
            state: Box::new(state),
            impact: false,
        },
    ))?;
    Ok(true)
}

/// Whether `entity` is a familiar: the class test `ImpactHandler` makes.
pub(super) fn is_familiar<H: Host>(h: &H, entity: &H::Entity) -> bool {
    h.kind(entity) == Kind::Familiar
}
