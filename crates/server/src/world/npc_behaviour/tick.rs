//! Joins the `world::npc` tick kernel (`NPC.tick`/`handleTickActions`) to
//! this module. A binding's `world::npc::Context` hands each query and event
//! of that kernel to [`query`] and [`event`] first; those answer what the
//! original does through the NPC's behaviour, aggressive handler, runtime
//! class and `fullRestore`, and return `None`/`false` for the rest, which
//! stay the binding's (locks, pulses, walking, attributes, chat, teleport).
use super::{Error, Host, Result, ai, behaviour, class, lifecycle};
use crate::world::npc;

/// The tick kernel's error for one of this module's.
pub fn npc_error(error: Error) -> npc::Error {
    match error {
        Error::Null => npc::Error::Null,
        Error::Index => npc::Error::Index,
        Error::Binding(text) => npc::Error::Binding(text),
    }
}

/// Answers a tick query this module owns:
/// `Behavior` is `behavior.tick(this)`, `SelectTarget` is
/// `aggressiveHandler.selectTarget()`, `Attacking` is
/// `combatPulse.isAttacking()` and `InteractionLocked` is the lock query.
pub fn query<H: Host>(h: &mut H, npc: &H::Entity, query: npc::Query) -> Result<Option<bool>> {
    Ok(Some(match query {
        npc::Query::Behavior => {
            let behaviour = behaviour(h, npc)?;
            behaviour.tick(h, npc)?
        }
        npc::Query::SelectTarget => {
            let handler = ai(h, npc)?.handler.clone().ok_or(Error::Null)?;
            handler.select_target(h)?
        }
        npc::Query::Attacking => h.is_attacking(npc)?,
        npc::Query::InteractionLocked => h.interaction_locked(npc)?,
        _ => return Ok(None),
    }))
}

/// Runs a tick event this module owns and answers true; false leaves it to
/// the binding. `Inactivity` is `onRegionInactivity()`, `BehaviorRespawn`
/// `behavior.onRespawn(this)`, `Respawn` the class's `onRespawn()`,
/// `Restore` `fullRestore()` and `PauseAggression` the handler's
/// `setPauseTicks`.
pub fn event<H: Host>(h: &mut H, npc: &H::Entity, event: &npc::Event) -> Result<bool> {
    match event {
        npc::Event::Inactivity => lifecycle::on_region_inactivity(h, npc)?,
        npc::Event::BehaviorRespawn => {
            let behaviour = behaviour(h, npc)?;
            behaviour.on_respawn(h, npc)?;
        }
        npc::Event::Respawn => {
            let class = class(h, npc)?;
            class.on_respawn(h, npc)?;
        }
        npc::Event::Restore => lifecycle::full_restore(h, npc)?,
        npc::Event::PauseAggression(ticks) => {
            let handler = ai(h, npc)?.handler.clone().ok_or(Error::Null)?;
            handler.set_pause_ticks(h, *ticks)?;
        }
        _ => return Ok(false),
    }
    Ok(true)
}
