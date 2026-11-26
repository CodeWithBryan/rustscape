//! AntiMacro: the timer that schedules random events, and its companion
//! members. The events themselves (`RandomEvents`, `RandomEventNPC`) are
//! content behind [`Host`] services; the `::revent` command is not ported.
use super::{Host, Kind, Natives, State, get, put, random, random_between};
use crate::definitions::item_config::json::Json;
use crate::timers::{Handle, Manager, Result};

/// `AntiMacro.MIN_DELAY_TICKS` and `MAX_DELAY_TICKS`.
pub const DELAY: (i32, i32) = (3000, 9000);
/// `AntiMacro.EVENT_NPC`.
pub const EVENT_NPC: &str = "re-npc";

fn paused<H: Host>(ctx: &Natives<'_, H>, timer: &Handle) -> Result<bool> {
    ctx.fields(timer, |s| {
        matches!(s, State::AntiMacro { paused: true, .. })
    })
}

fn set_paused<H: Host>(ctx: &Natives<'_, H>, timer: &Handle, value: bool) -> Result<()> {
    ctx.fields(timer, |s| {
        if let State::AntiMacro { paused, .. } = s {
            *paused = value;
        }
    })
}

/// `setNextExecution()`.
fn set_next_execution<H: Host>(ctx: &mut Natives<'_, H>, timer: &Handle) -> Result<()> {
    let interval = random_between(ctx.host, DELAY.0, DELAY.1.wrapping_add(1))?;
    timer.write().run_interval = interval;
    let ticks = ctx.host.ticks()?;
    timer.write().next_execution = ticks.wrapping_add(interval);
    Ok(())
}

pub(super) fn on_register<S, H: Host>(
    ctx: &mut Natives<'_, H>,
    timer: &Handle,
    manager: &mut Manager,
    state: &mut S,
) -> Result<()> {
    let entity = ctx.entity.clone();
    let player = ctx.is_player();
    if !player || ctx.host.artificial(&entity)? {
        manager.remove(Some(timer), state, ctx)?;
    }
    if player && ctx.host.administrator(&entity)? {
        set_paused(ctx, timer, true)?;
    }
    if timer.read().run_interval == 0 {
        set_next_execution(ctx, timer)?;
    }
    Ok(())
}

/// `canSpawn(entity)`.
fn can_spawn<H: Host>(ctx: &mut Natives<'_, H>, timer: &Handle) -> Result<bool> {
    let entity = ctx.entity.clone();
    if ctx.host.random_events_restricted(&entity)? {
        return Ok(false);
    }
    if ctx.host.event_npc(&entity)?.is_some() {
        return Ok(false);
    }
    Ok(!paused(ctx, timer)?)
}

/// `rollEventPool(entity)`.
fn roll_event_pool<H: Host>(ctx: &mut Natives<'_, H>, timer: &Handle) -> Result<H::RandomEvent> {
    let forced = ctx.fields(timer, |s| match s {
        State::AntiMacro { next_random, .. } => next_random.take(),
        _ => None,
    })?;
    if let Some(event) = forced {
        return Ok(event);
    }
    let entity = ctx.entity.clone();
    let skill = ctx.host.skills(&entity).0.last_trained_skill;
    let skill_based = ctx.host.skill_random_event(skill)?;
    let normal = ctx.host.non_skill_random_event()?;
    let roll = random(ctx.host, 100)?;
    if roll >= 65
        && let Some(event) = skill_based
    {
        let ticks = ctx.host.ticks()?;
        let last = ctx.host.skills(&entity).0.last_xp_gain;
        if ticks.wrapping_sub(last) < 250 {
            return Ok(event);
        }
    }
    Ok(normal)
}

pub(super) fn run<H: Host>(ctx: &mut Natives<'_, H>, timer: &Handle) -> Result<bool> {
    if !ctx.is_player() {
        return Ok(false);
    }
    set_next_execution(ctx, timer)?;
    if !can_spawn(ctx, timer)? {
        return Ok(true);
    }
    let event = roll_event_pool(ctx, timer)?;
    let player = ctx.entity.clone();
    let npc = ctx.host.create_event_npc(&event, &player)?;
    if ctx.host.spawn_location_missing(&npc)? {
        ctx.debug("[AntiMacro] Attempted to spawn random, but spawnLoc was null.")?;
        return Ok(true);
    }
    ctx.host.init_event_npc(&npc)?;
    ctx.host.set_event_npc(&player, &npc)?;
    let name = ctx.host.random_event_name(&event);
    ctx.debug(&format!("[AntiMacro] Fired {name}."))?;
    Ok(true)
}

pub(super) fn save<H: Host>(
    ctx: &mut Natives<'_, H>,
    timer: &Handle,
    root: &mut Json,
) -> Result<()> {
    let next = timer.read().next_execution;
    let ticks = ctx.host.ticks()?;
    put(
        root,
        "ticksRemaining",
        Json::Text(next.wrapping_sub(ticks).to_string()),
    )
}

pub(super) fn parse<H: Host>(ctx: &mut Natives<'_, H>, timer: &Handle, data: &Json) -> Result<()> {
    // root["ticksRemaining"]?.toString()?.toIntOrNull() ?: 0
    let interval = match data.get("ticksRemaining") {
        None | Some(Json::Null) => None,
        Some(value) => rs09_game_core::java_string::parse_i32(&value.text()),
    }
    .unwrap_or(0);
    timer.write().run_interval = interval;
    let ticks = ctx.host.ticks()?;
    timer.write().next_execution = ticks.wrapping_add(interval);
    Ok(())
}

/// `getTimer<AntiMacro>(player)`.
fn timer(manager: &Manager) -> Result<Option<Handle>> {
    get(manager, Kind::AntiMacro)
}

/// `AntiMacro.pause(player)`.
pub fn pause<H: Host>(ctx: &mut Natives<'_, H>, manager: &Manager) -> Result<()> {
    if let Some(timer) = timer(manager)? {
        set_paused(ctx, &timer, true)?;
    }
    Ok(())
}

/// `AntiMacro.unpause(player)`.
pub fn unpause<H: Host>(ctx: &mut Natives<'_, H>, manager: &Manager) -> Result<()> {
    if let Some(timer) = timer(manager)? {
        set_paused(ctx, &timer, false)?;
    }
    Ok(())
}

/// `AntiMacro.forceEvent(player, event)`.
pub fn force_event<H: Host>(
    ctx: &mut Natives<'_, H>,
    manager: &Manager,
    event: Option<H::RandomEvent>,
) -> Result<()> {
    let Some(timer) = timer(manager)? else {
        return Ok(());
    };
    let ticks = ctx.host.ticks()?;
    timer.write().next_execution = ticks;
    ctx.fields(&timer, |s| {
        if let State::AntiMacro { next_random, .. } = s {
            *next_random = event;
        }
    })
}

/// `AntiMacro.getEventNpc(player)`.
pub fn event_npc<H: Host>(ctx: &mut Natives<'_, H>) -> Result<Option<H::EventNpc>> {
    let player = ctx.entity.clone();
    ctx.host.event_npc(&player)
}

/// `AntiMacro.terminateEventNpc(player)`.
pub fn terminate_event_npc<H: Host>(ctx: &mut Natives<'_, H>) -> Result<()> {
    if let Some(npc) = event_npc(ctx)? {
        ctx.host.terminate_event_npc(&npc)?;
    }
    Ok(())
}

/// `AntiMacro.rollEventLoot(player)`.
pub fn roll_event_loot<H: Host>(ctx: &mut Natives<'_, H>) -> Result<Vec<(i32, i32)>> {
    let Some(npc) = event_npc(ctx)? else {
        return Ok(Vec::new());
    };
    let player = ctx.entity.clone();
    Ok(ctx.host.roll_event_loot(&npc, &player)?.unwrap_or_default())
}

/// `Char.isWhitespace()` on the JVM: `Character.isWhitespace(c) ||
/// Character.isSpaceChar(c)`.
fn kotlin_whitespace(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\u{b}' | '\u{c}' | '\r' | '\u{1c}'..='\u{1f}' | ' ' | '\u{a0}' | '\u{1680}'
            | '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}'
    )
}

/// `AntiMacro.parseCommandArgs(args, commandName)`: the target player name
/// and event. `valueOf` is the host's (`Host::random_event_value_of`).
pub fn parse_command_args<H: Host>(
    host: &H,
    args: &str,
    command: &str,
) -> (String, Option<H::RandomEvent>) {
    let mut user = String::new();
    let mut event = String::new();
    let mut mode = "-p";
    for token in args.split(' ') {
        if token == command {
            continue;
        }
        if token == "-p" || token == "-e" {
            mode = if token == "-p" { "-p" } else { "-e" };
            continue;
        }
        match mode {
            "-p" => {
                user.push_str(token);
                user.push(' ');
            }
            _ => {
                event.push_str(token);
                event.push(' ');
            }
        }
    }
    let username = rs09_game_core::java_lower::lowercase_root(user.trim_matches(kotlin_whitespace))
        .replace(' ', "_");
    let name = event
        .trim_matches(kotlin_whitespace)
        .to_uppercase()
        .replace(' ', "_");
    (username, host.random_event_value_of(&name))
}
