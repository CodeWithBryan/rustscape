//! ContentAPI's attributes, timers and poison, locks, pulses, queued scripts,
//! clocks and stun.
use super::{
    Anim, Arg, Error, Gfx, Host, Kind, Member as M, Priority, Pulse, PulseBody, PulseTarget,
    QueuedScript, Result, ScriptBody, Value, b, n, nd, unit, world,
};
use crate::world::content_api::{
    services::{LogLevel, log},
    ui::send_message,
    visuals::play_audio,
};

/// `QueueStrength`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueueStrength {
    Weak,
    Normal,
    Strong,
    Soft,
}
impl QueueStrength {
    pub fn name(self) -> &'static str {
        ["WEAK", "NORMAL", "STRONG", "SOFT"][self as usize]
    }
}

/// `Clocks`.
pub mod clocks {
    pub const MOVEMENT: i32 = 0;
    pub const ANIMATION_END: i32 = 1;
    pub const NEXT_EAT: i32 = 2;
    pub const NEXT_CONSUME: i32 = 3;
    pub const NEXT_DRINK: i32 = 4;
    pub const NEXT_ATTACK: i32 = 5;
    pub const STUN: i32 = 6;
    pub const SKILLING: i32 = 7;
}

// ---------------------------------------------------------------- attributes

/// `getAttribute(entity, attribute, default)`.
pub fn get_attribute<H: Host + ?Sized>(
    h: &mut H,
    e: &H::Node,
    key: &str,
    default: Value<H::Node>,
) -> Result<Value<H::Node>> {
    let stored = h
        .call(
            M::EntityGetAttribute,
            vec![nd(e), Arg::text(key), Arg::value(&default)],
        )?
        .value()?;
    Ok(stored.unwrap_or(default))
}

/// `setAttribute(entity, attribute, value)`.
pub fn set_attribute<H: Host + ?Sized>(
    h: &mut H,
    e: &H::Node,
    key: &str,
    value: &Value<H::Node>,
) -> Result<()> {
    unit(
        h,
        M::EntitySetAttribute,
        vec![nd(e), Arg::text(key), Arg::value(value)],
    )
}

/// `removeAttribute(entity, attribute)`: without any `/save:`.
pub fn remove_attribute<H: Host + ?Sized>(h: &mut H, e: &H::Node, key: &str) -> Result<()> {
    unit(
        h,
        M::EntityRemoveAttribute,
        vec![nd(e), Arg::Text(key.replace("/save:", ""))],
    )
}

/// `removeAttributes(entity, attributes...)`.
pub fn remove_attributes<H: Host + ?Sized>(h: &mut H, e: &H::Node, keys: &[&str]) -> Result<()> {
    for key in keys {
        remove_attribute(h, e, key)?;
    }
    Ok(())
}

/// `getUsedOption(player)`: the `interact:option` attribute, "INVALID"
/// without one; another type fails the String cast.
pub fn get_used_option<H: Host + ?Sized>(h: &mut H, p: &H::Node) -> Result<String> {
    match get_attribute(h, p, "interact:option", Value::Text("INVALID".into()))? {
        Value::Text(t) => Ok(t),
        Value::Null => Err(Error::Null),
        _ => Err(Error::Cast),
    }
}

// ---------------------------------------------------------------- timers

/// `registerTimer(entity, timer)`.
pub fn register_timer<H: Host + ?Sized>(
    h: &mut H,
    e: &H::Node,
    timer: Option<&H::Node>,
) -> Result<()> {
    let Some(timer) = timer else {
        return Ok(());
    };
    unit(h, M::TimerRegister, vec![nd(e), nd(timer)])
}

/// `getTimer(entity, identifier)`.
pub fn get_timer<H: Host + ?Sized>(
    h: &mut H,
    e: &H::Node,
    identifier: &str,
) -> Result<Option<H::Node>> {
    n(h, M::TimerGet, vec![nd(e), Arg::text(identifier)])
}

/// `getTimer<T>(entity)`: the reified cast of the answer to `class`.
pub fn get_timer_of<H: Host + ?Sized>(
    h: &mut H,
    e: &H::Node,
    class: &str,
) -> Result<Option<H::Node>> {
    let timer = n(h, M::TimerGet, vec![nd(e), Arg::text(class)])?;
    if let Some(t) = &timer
        && h.timer_class(t) != class
    {
        return Err(Error::Cast);
    }
    Ok(timer)
}

/// `spawnTimer(identifier, args...)`.
pub fn spawn_timer<H: Host + ?Sized>(
    h: &mut H,
    identifier: &str,
    args: &[Value<H::Node>],
) -> Result<Option<H::Node>> {
    n(
        h,
        M::TimerInstance,
        vec![
            Arg::text(identifier),
            Arg::List(args.iter().map(Arg::value).collect()),
        ],
    )
}

/// `spawnTimer<T>(args...)`: `getTimerInstance<T>(*args)!!`.
pub fn spawn_timer_of<H: Host + ?Sized>(
    h: &mut H,
    class: &str,
    args: &[Value<H::Node>],
) -> Result<H::Node> {
    let timer = spawn_timer(h, class, args)?.ok_or(Error::Null)?;
    if h.timer_class(&timer) != class {
        return Err(Error::Cast);
    }
    Ok(timer)
}

/// `getOrStartTimer(entity, identifier, args...)`.
pub fn get_or_start_timer<H: Host + ?Sized>(
    h: &mut H,
    e: &H::Node,
    identifier: &str,
    args: &[Value<H::Node>],
) -> Result<Option<H::Node>> {
    if let Some(existing) = get_timer(h, e, identifier)? {
        return Ok(Some(existing));
    }
    let timer = spawn_timer(h, identifier, args)?;
    register_timer(h, e, timer.as_ref())?;
    Ok(timer)
}

/// `getOrStartTimer<T>(entity, args...)`.
pub fn get_or_start_timer_of<H: Host + ?Sized>(
    h: &mut H,
    e: &H::Node,
    class: &str,
    args: &[Value<H::Node>],
) -> Result<H::Node> {
    if let Some(existing) = get_timer_of(h, e, class)? {
        return Ok(existing);
    }
    let timer = spawn_timer_of(h, class, args)?;
    register_timer(h, e, Some(&timer))?;
    Ok(timer)
}

/// `hasTimerActive<T>(entity)`.
pub fn has_timer_active_of<H: Host + ?Sized>(h: &mut H, e: &H::Node, class: &str) -> Result<bool> {
    Ok(get_timer_of(h, e, class)?.is_some())
}

/// `hasTimerActive(entity, identifier)`.
pub fn has_timer_active<H: Host + ?Sized>(
    h: &mut H,
    e: &H::Node,
    identifier: &str,
) -> Result<bool> {
    Ok(get_timer(h, e, identifier)?.is_some())
}

/// `removeTimer<T>(entity)` (by class) and `removeTimer(entity, identifier)`.
pub fn remove_timer<H: Host + ?Sized>(
    h: &mut H,
    e: &H::Node,
    class_or_identifier: &str,
) -> Result<()> {
    unit(
        h,
        M::TimerRemove,
        vec![nd(e), Arg::text(class_or_identifier)],
    )
}

/// `removeTimer(entity, timer)`.
pub fn remove_timer_instance<H: Host + ?Sized>(
    h: &mut H,
    e: &H::Node,
    timer: &H::Node,
) -> Result<()> {
    unit(h, M::TimerRemove, vec![nd(e), nd(timer)])
}

/// `applyPoison(entity, source, severity)`.
pub fn apply_poison<H: Host + ?Sized>(
    h: &mut H,
    e: &H::Node,
    source: &H::Node,
    severity: i32,
) -> Result<()> {
    if has_timer_active_of(h, e, "PoisonImmunity")? {
        return Ok(());
    }
    if b(h, M::EntityPoisonImmune, vec![nd(e)])? {
        return Ok(());
    }
    match get_timer_of(h, e, "Poison")? {
        Some(existing) => {
            if h.poison_severity(&existing) > severity {
                return Ok(());
            }
            unit(
                h,
                M::PoisonSetSeverity,
                vec![nd(&existing), Arg::Int(severity)],
            )?;
            h.set_poison_source(&existing, source);
            Ok(())
        }
        None => {
            let timer = spawn_timer_of(
                h,
                "Poison",
                &[Value::Node(source.clone()), Value::Int(severity)],
            )?;
            register_timer(h, e, Some(&timer))
        }
    }
}

/// `isPoisoned(entity)`.
pub fn is_poisoned<H: Host + ?Sized>(h: &mut H, e: &H::Node) -> Result<bool> {
    Ok(get_timer_of(h, e, "Poison")?.is_some())
}

/// `curePoison(entity)`.
pub fn cure_poison<H: Host + ?Sized>(h: &mut H, e: &H::Node) -> Result<()> {
    if !has_timer_active_of(h, e, "Poison")? {
        return Ok(());
    }
    remove_timer(h, e, "Poison")?;
    if h.kind(e) == Kind::Player {
        send_message(h, e, "Your poison has been cured.")?;
    }
    Ok(())
}

// ---------------------------------------------------------------- locks

/// `lock(entity, duration)`.
pub fn lock<H: Host + ?Sized>(h: &mut H, e: &H::Node, duration: i32) -> Result<()> {
    unit(h, M::EntityLock, vec![nd(e), Arg::Int(duration)])
}

/// `lockInteractions(entity, duration)`.
pub fn lock_interactions<H: Host + ?Sized>(h: &mut H, e: &H::Node, duration: i32) -> Result<()> {
    unit(h, M::LockInteractions, vec![nd(e), Arg::Int(duration)])
}

/// `unlock(entity)`.
pub fn unlock<H: Host + ?Sized>(h: &mut H, e: &H::Node) -> Result<()> {
    unit(h, M::EntityUnlock, vec![nd(e)])
}

// ---------------------------------------------------------------- pulses

/// `submitWorldPulse(pulse)`.
pub fn submit_world_pulse<H: Host + ?Sized>(h: &mut H, pulse: Pulse<H::Node>) -> Result<()> {
    h.submit_pulse(PulseTarget::World, pulse).map(|_| ())
}

/// `runWorldTask(task)`: the submitted pulse.
pub fn run_world_task<H: Host + ?Sized>(h: &mut H, task: &H::Node) -> Result<H::Node> {
    h.submit_pulse(
        PulseTarget::World,
        Pulse {
            delay: 1,
            body: PulseBody::WorldTask { task: task.clone() },
        },
    )
}

/// `submitIndividualPulse(entity, pulse)`.
pub fn submit_individual_pulse<H: Host + ?Sized>(
    h: &mut H,
    e: &H::Node,
    pulse: Pulse<H::Node>,
) -> Result<()> {
    h.submit_pulse(PulseTarget::Entity(e), pulse).map(|_| ())
}

/// `runTask(entity, delay = 0, repeatTimes = 1, task)`.
pub fn run_task<H: Host + ?Sized>(
    h: &mut H,
    e: &H::Node,
    delay: i32,
    repeat: i32,
    task: &H::Node,
) -> Result<()> {
    let body = PulseBody::RunTask {
        cycles: repeat,
        task: task.clone(),
    };
    h.submit_pulse(PulseTarget::Entity(e), Pulse { delay, body })
        .map(|_| ())
}

/// One `pulse()` of a pulse ContentAPI built (or a content pulse).
pub fn run_pulse<H: Host + ?Sized>(h: &mut H, pulse: &mut Pulse<H::Node>) -> Result<bool> {
    match &mut pulse.body {
        PulseBody::PoofClear { npc, counter } => {
            let npc = npc.clone();
            world::run_poof_clear(h, &npc, counter)
        }
        PulseBody::TransformRestore { npc } => {
            unit(h, M::NpcReTransform, vec![nd(npc)])?;
            Ok(true)
        }
        PulseBody::RunTask { cycles, task } => {
            unit(h, M::Callback, vec![nd(task)])?;
            *cycles = cycles.wrapping_sub(1);
            Ok(*cycles <= 0)
        }
        PulseBody::WorldTask { task } => {
            unit(h, M::Callback, vec![nd(task)])?;
            Ok(true)
        }
        PulseBody::Content(p) => b(h, M::CallbackBool, vec![nd(p)]),
    }
}

// ---------------------------------------------------------------- scripts and clocks

/// `finishedMoving(entity)`.
pub fn finished_moving<H: Host + ?Sized>(h: &mut H, e: &H::Node) -> Result<bool> {
    Ok(h.clock(e, clocks::MOVEMENT)? < h.ticks())
}

/// `entity.scripts.getActiveScript()`.
fn active<H: Host + ?Sized>(h: &mut H, e: &H::Node) -> Result<Option<H::Node>> {
    n(h, M::ActiveScript, vec![nd(e)])
}

/// `delayScript(entity, ticks)`: always false.
pub fn delay_script<H: Host + ?Sized>(h: &mut H, e: &H::Node, ticks: i32) -> Result<bool> {
    if let Some(s) = active(h, e)? {
        let next = h.ticks().wrapping_add(ticks);
        h.set_script_next_execution(&s, next);
    }
    Ok(false)
}

/// `delayEntity(entity, ticks)`: the script delay, then `lock`.
pub fn delay_entity<H: Host + ?Sized>(h: &mut H, e: &H::Node, ticks: i32) -> Result<()> {
    let delay = h.ticks().wrapping_add(ticks);
    unit(h, M::ScriptsDelay, vec![nd(e), Arg::Int(delay)])?;
    lock(h, e, ticks)
}

/// `apRange(entity, apRange)`.
pub fn ap_range<H: Host + ?Sized>(h: &mut H, e: &H::Node, range: i32) -> Result<()> {
    unit(h, M::ScriptsApRange, vec![nd(e), Arg::Int(range)])?;
    unit(h, M::ScriptsApRangeCalled, vec![nd(e), Arg::Bool(true)])
}

/// `animationFinished(entity)`.
pub fn animation_finished<H: Host + ?Sized>(h: &mut H, e: &H::Node) -> Result<bool> {
    Ok(h.clock(e, clocks::ANIMATION_END)? < h.ticks())
}

/// `clearScripts(entity)`: always true.
pub fn clear_scripts<H: Host + ?Sized>(h: &mut H, e: &H::Node) -> Result<bool> {
    unit(h, M::ScriptsReset, vec![nd(e)])?;
    Ok(true)
}

/// `log(entity.scripts.getActiveScript()!!::class.java, ERR, message)`: the
/// active script is read again.
fn log_active<H: Host + ?Sized>(h: &mut H, e: &H::Node, message: &str) -> Result<()> {
    let s = active(h, e)?.ok_or(Error::Null)?;
    let class = h.script_class(&s);
    log(h, &class, LogLevel::Err, message)
}

/// `restartScript(entity)`.
pub fn restart_script<H: Host + ?Sized>(h: &mut H, e: &H::Node) -> Result<bool> {
    let persist = active(h, e)?.is_some_and(|s| h.script_persist(&s));
    if !persist {
        log_active(
            h,
            e,
            "Tried to call restartScript on a non-persistent script! Either use stopExecuting() or make the script persistent.",
        )?;
        return clear_scripts(h, e);
    }
    Ok(true)
}

/// `keepRunning(entity)`: next execution next tick; always false.
pub fn keep_running<H: Host + ?Sized>(h: &mut H, e: &H::Node) -> Result<bool> {
    if let Some(s) = active(h, e)? {
        let next = h.ticks().wrapping_add(1);
        h.set_script_next_execution(&s, next);
    }
    Ok(false)
}

/// `stopExecuting(entity)`.
pub fn stop_executing<H: Host + ?Sized>(h: &mut H, e: &H::Node) -> Result<bool> {
    let persist = active(h, e)?.is_some_and(|s| h.script_persist(&s));
    if persist {
        log_active(
            h,
            e,
            "Tried to call stopExecuting() on a persistent script! To halt execution of a persistent script, you MUST call clearScripts()!",
        )?;
        return clear_scripts(h, e);
    }
    Ok(true)
}

/// `queueScript(entity, delay = 1, strength = WEAK, persist = false, script)`.
pub fn queue_script<H: Host + ?Sized>(
    h: &mut H,
    e: &H::Node,
    delay: i32,
    strength: QueueStrength,
    persist: bool,
    body: ScriptBody<H::Node>,
) -> Result<()> {
    let next_execution = h.ticks().wrapping_add(delay);
    h.add_to_queue(
        e,
        QueuedScript {
            body,
            strength,
            persist,
            next_execution,
        },
    )
}

/// One execution of a queued script ContentAPI built (or a content body).
pub fn run_script<H: Host + ?Sized>(
    h: &mut H,
    script: &mut QueuedScript<H::Node>,
    stage: i32,
) -> Result<bool> {
    match &mut script.body {
        ScriptBody::SendChat { entity, message } => {
            let (entity, message) = (entity.clone(), message.clone());
            b(h, M::EntitySendChat, vec![nd(&entity), Arg::Text(message)])?;
            stop_executing(h, &entity)
        }
        ScriptBody::TransformHp { npc, ticks } => {
            let (npc, ticks) = (npc.clone(), *ticks);
            if stage == 0 {
                return delay_script(h, &npc, ticks);
            }
            unit(h, M::NpcReTransform, vec![nd(&npc)])?;
            stop_executing(h, &npc)
        }
        ScriptBody::ForceMove(m) => world::run_force_move(h, m),
        ScriptBody::Content(f) => b(h, M::CallbackBool, vec![nd(f), Arg::Int(stage)]),
    }
}

/// `delayClock(entity, clock, ticks, delayScript = false)`.
pub fn delay_clock<H: Host + ?Sized>(
    h: &mut H,
    e: &H::Node,
    clock: i32,
    ticks: i32,
    delay: bool,
) -> Result<bool> {
    let value = h.ticks().wrapping_add(ticks);
    h.set_clock(e, clock, value)?;
    if delay {
        return delay_script(h, e, ticks);
    }
    Ok(false)
}

/// `clockReady(entity, clock)`.
pub fn clock_ready<H: Host + ?Sized>(h: &mut H, e: &H::Node, clock: i32) -> Result<bool> {
    Ok(h.clock(e, clock)? <= h.ticks())
}

/// `delayAttack(entity, ticks)`: the combat pulse is delayed by 3 whatever
/// `ticks` is.
pub fn delay_attack<H: Host + ?Sized>(h: &mut H, e: &H::Node, ticks: i32) -> Result<()> {
    unit(h, M::DelayNextAttack, vec![nd(e), Arg::Int(3)])?;
    let value = h.ticks().wrapping_add(ticks);
    h.set_clock(e, clocks::NEXT_ATTACK, value)
}

/// `stun(entity, ticks, sendMessage = true)`.
pub fn stun<H: Host + ?Sized>(h: &mut H, e: &H::Node, ticks: i32, message: bool) -> Result<()> {
    unit(h, M::WalkingReset, vec![nd(e)])?;
    unit(h, M::PulseManagerClear, vec![nd(e)])?;
    unit(h, M::LockMovement, vec![nd(e), Arg::Int(ticks)])?;
    let value = h.ticks().wrapping_add(ticks);
    h.set_clock(e, clocks::STUN, value)?;
    b(
        h,
        M::EntityGraphics,
        vec![
            nd(e),
            Arg::gfx(Gfx {
                id: 80,
                height: 96,
                delay: 0,
            }),
        ],
    )?;
    if h.kind(e) == Kind::Player {
        play_audio(h, e, 2727, 0, 1, None, super::visuals::DEFAULT_AUDIO_RADIUS)?;
        b(
            h,
            M::EntityAnimate,
            vec![
                nd(e),
                Arg::anim(&Anim::<H::Node>::with(424, Priority::VeryHigh)),
            ],
        )?;
        if message {
            send_message(h, e, "You have been stunned!")?;
        }
    }
    Ok(())
}

/// `isStunned(entity)`.
pub fn is_stunned<H: Host + ?Sized>(h: &mut H, e: &H::Node) -> Result<bool> {
    Ok(h.clock(e, clocks::STUN)? >= h.ticks())
}

/// `setCurrentScriptState(entity, state)`: one below, so the next execution
/// runs the state; without a script, a warning and (in dev mode) a failure.
pub fn set_current_script_state<H: Host + ?Sized>(
    h: &mut H,
    e: &H::Node,
    state: i32,
) -> Result<()> {
    match active(h, e)? {
        None => {
            log(
                h,
                "ContentAPI",
                LogLevel::Warn,
                "Tried to set a script state when no script was being ran!",
            )?;
            if h.dev_mode() != Some(true) {
                return Ok(());
            }
            Err(Error::State(
                "Script execution mistake - check stack trace and above warning log!".into(),
            ))
        }
        Some(s) => {
            h.set_script_state(&s, state.wrapping_sub(1));
            Ok(())
        }
    }
}
