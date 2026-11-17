//! `IdleAbstractNPC` (NPCs that stay idle until disturbed: rock crabs,
//! krakens, ...), the "disturb" option handler `NPCDisturbPlugin` and the
//! hairdresser's `HairCutPlugin`.
use super::{CombatStyle, Error, Host, InteractionType, Kind, Result, Value, as_npc};

/// `Skills.SLAYER`.
pub const SLAYER: i32 = 18;

/// An `IdleAbstractNPC`'s own fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Idle {
    pub active_id: i32,
    pub idle: bool,
    /// `timeout`: 30 ticks (18 seconds) before it goes idle again.
    pub timeout: i32,
}
impl Idle {
    /// `IdleAbstractNPC(idleId, activeId, location)`'s field initialisers.
    pub fn new(active_id: i32) -> Self {
        Self {
            active_id,
            idle: true,
            timeout: 30,
        }
    }
}

fn state<H: Host + ?Sized>(h: &mut H, npc: &H::Npc) -> Result<Idle> {
    h.idle(npc)?.ok_or(Error::Cast)
}

/// `handleTickActions()`: after the NPC's own, an active NPC no longer
/// attacking goes idle once `timeout` ticks (600 ms each) have passed since
/// its `combat-time`.
pub fn handle_tick_actions<H: Host + ?Sized>(h: &mut H, npc: &H::Npc) -> Result<()> {
    h.base_tick_actions(npc)?;
    if !state(h, npc)?.idle && !h.is_attacking(npc)? {
        let time = h.get_attribute(npc, "combat-time")?;
        let time = Value::long(time, 0)?;
        let now = h.current_time_millis()?;
        let timeout = state(h, npc)?.timeout;
        if now.wrapping_sub(time) > i64::from(timeout.wrapping_mul(600)) {
            go_idle(h, npc)?;
        }
    }
    Ok(())
}

/// `isAttackable(entity, style, message)`.
pub fn is_attackable<H: Host + ?Sized>(
    h: &mut H,
    npc: &H::Npc,
    entity: Option<&H::Npc>,
    style: Option<CombatStyle>,
    message: bool,
) -> Result<bool> {
    if h.is_invisible(npc)? {
        return Ok(false);
    }
    if h.task_level(npc)?.is_some() {
        if let Some(e) = entity.filter(|e| h.kind(e) == Kind::Player) {
            let required = h.task_level(npc)?.ok_or(Error::Null)?;
            if required > h.level(e, SLAYER)? && message {
                h.packet_message(
                    e,
                    "You need a higher slayer level to know how to wound this monster.",
                )?;
            }
        }
    }
    if h.is_dead(npc)? {
        return Ok(false);
    }
    let entity = entity.ok_or(Error::Null)?;
    if !h.continue_attack(entity, npc, style, message)? {
        return Ok(false);
    }
    Ok(true)
}

/// `inDisturbingRange(disturber)`.
pub fn in_disturbing_range<H: Host + ?Sized>(
    h: &mut H,
    npc: &H::Npc,
    disturber: Option<&H::Npc>,
) -> Result<bool> {
    if state(h, npc)?.idle {
        let disturber = disturber.ok_or(Error::Null)?;
        let handler = h.swing_handler(disturber, false)?;
        if h.can_swing(&handler, disturber, npc)? != Some(InteractionType::NoInteract) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// `canDisturb(disturber)`.
pub fn can_disturb<H: Host + ?Sized>(h: &mut H, npc: &H::Npc) -> Result<bool> {
    Ok(state(h, npc)?.idle)
}

/// `disturb(disturber)`: combat flags (or a combat time 10 seconds ahead
/// without a disturber), the attack, the active form, then not idle.
pub fn disturb<H: Host + ?Sized>(
    h: &mut H,
    npc: &H::Npc,
    disturber: Option<&H::Npc>,
) -> Result<()> {
    match disturber {
        Some(d) => h.set_combat_flags(d, npc)?,
        None => {
            let at = h.current_time_millis()?.wrapping_add(10000);
            h.set_attribute(npc, "combat-time", Value::Long(at))?;
        }
    }
    h.combat_attack(npc, disturber)?;
    let active = state(h, npc)?.active_id;
    h.transform(npc, active)?;
    let mut s = state(h, npc)?;
    s.idle = false;
    h.set_idle(npc, s)
}

/// `goIdle()`.
pub fn go_idle<H: Host + ?Sized>(h: &mut H, npc: &H::Npc) -> Result<()> {
    let mut s = state(h, npc)?;
    if s.idle {
        return Ok(());
    }
    s.idle = true;
    h.set_idle(npc, s)?;
    h.re_transform(npc)
}

/// `NPCDisturbPlugin.handle(player, node, option)`.
pub fn disturb_option<H: Host + ?Sized>(
    h: &mut H,
    player: &H::Npc,
    node: Option<&H::Npc>,
) -> Result<bool> {
    if let Some(npc) = node.filter(|n| h.kind(n) == Kind::Idle) {
        if can_disturb(h, npc)? {
            disturb(h, npc, Some(player))?;
        }
        return Ok(true);
    }
    Ok(false)
}

/// `NPCDisturbPlugin.isWalk(player, node)`.
pub fn disturb_option_walks<H: Host + ?Sized>(
    h: &mut H,
    player: &H::Npc,
    node: Option<&H::Npc>,
) -> Result<bool> {
    if let Some(npc) = node.filter(|n| h.kind(n) == Kind::Idle) {
        return Ok(!in_disturbing_range(h, npc, Some(player))?);
    }
    Ok(false)
}

/// `HairCutPlugin.handle(player, node, option)`: the hairdresser's dialogue
/// with the Java `(NPC)` cast (null passes).
pub fn hair_cut<H: Host + ?Sized>(
    h: &mut H,
    player: &H::Npc,
    node: Option<&H::Npc>,
) -> Result<bool> {
    let npc = match node {
        None => None,
        Some(n) => Some(as_npc(h, Some(n))?.clone()),
    };
    h.open_dialogue_id(
        player,
        598,
        &[super::Arg::Node(npc), super::Arg::Bool(true)],
    )?;
    Ok(true)
}
