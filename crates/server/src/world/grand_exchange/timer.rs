//! GrandExchangeTimer.kt: `RSTimer(500, "GE periodic poll", isSoft, isAuto)`.
//! Its registration is `startup::timers` (Kind::GrandExchange); this is its
//! `run` body.
use super::{Host, JINGLE, Result, records};
use crate::world::player_walk::NodeHandle;

/// The timer's run interval in ticks.
pub const INTERVAL: i32 = 500;

/// `run(entity)`: `None` is an entity that is not a Player (false, the timer
/// is removed). A pending update notification is cleared and, when slot 0
/// has something to collect, announced.
pub fn run<H: Host + ?Sized>(h: &mut H, entity: Option<&NodeHandle>) -> Result<bool> {
    let Some(player) = entity else {
        return Ok(false);
    };
    let records = records::get_instance(h, Some(player))?;
    let notify = records.fields().update_notification;
    if notify {
        records.fields().update_notification = false;
        if records::has_awaiting_collections(h, player)? {
            h.content_message(
                player,
                "One or more of your Grand Exchange offers have been updated.",
            )?;
            h.jingle(player, JINGLE)?;
        }
    }
    Ok(true)
}
