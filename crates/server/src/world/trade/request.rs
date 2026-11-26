//! RequestManager.kt with RequestType.kt and RequestModule.open dispatch.
//! There is no timeout: `lastRequest` stays on the requester and the
//! manager's `target` stays set until an acceptance clears it; the only timed
//! window is the two-tick `busy` attribute written at acceptance.
use super::{Host, RequestType, Result, module, ui::Ui};
use crate::world::interfaces;

/// `RequestManager.request(target, type)`.
pub fn request<H: Host>(
    h: &mut H,
    player: &H::Player,
    target: &H::Player,
    kind: RequestType,
) -> Result<bool> {
    if !can_request(h, player, kind, target)? {
        return Ok(false);
    }
    if accept_existing(h, player, target, kind)? {
        return Ok(true);
    }
    h.message(player, kind.message())?;
    let name = h.username(player)?;
    h.message(target, &format!("{name}{}", kind.request_message()))?;
    h.set_last_request(player, kind)?;
    h.set_request_target(player, Some(target.clone()))?;
    if h.is_artificial(target)? {
        let name = super::lowercase(&h.username(target)?);
        if let Some(bot) = h.bot(&name)?
            && bot.doubling
        {
            h.bot_trade_received(&name, player)?;
        }
    }
    Ok(true)
}

fn can_request<H: Host>(
    h: &mut H,
    player: &H::Player,
    kind: RequestType,
    target: &H::Player,
) -> Result<bool> {
    if target == player {
        return Ok(false);
    }
    let there = h.location(target)?;
    let here = h.location(player)?;
    if !there.within_distance(here, 15) {
        let name = h.username(target)?;
        h.message(player, &format!("Unable to find {name}."))?;
        return Ok(false);
    }
    let busy_ui = if !h.is_active(target)? {
        true
    } else {
        let m = h.interface_manager(target)?;
        interfaces::is_opened(&m)
    };
    if busy_ui {
        h.message(player, "Other player is busy at the moment.")?;
        return Ok(false);
    }
    let ticks = h.ticks()?;
    let busy = h.attribute_int(target, "busy", 0)? > ticks || {
        let ticks = h.ticks()?;
        h.attribute_int(player, "busy", 0)? > ticks
    };
    if busy {
        h.message(player, "Other player is busy at the moment.")?;
        return Ok(false);
    }
    if !h.zone_can_request(player, kind, target)? {
        Ok(false)
    } else {
        Ok(kind.can_request())
    }
}

fn accept_existing<H: Host>(
    h: &mut H,
    player: &H::Player,
    target: &H::Player,
    kind: RequestType,
) -> Result<bool> {
    let last = h.last_request(target)?;
    if last == Some(kind) && h.request_target(target)?.as_ref() == Some(player) {
        close(h, player)?;
        clear(h, player)?;
        h.set_request_target(target, None)?;
        let ticks = h.ticks()?;
        h.set_attribute_int(player, "busy", ticks.wrapping_add(2))?;
        let ticks = h.ticks()?;
        h.set_attribute_int(target, "busy", ticks.wrapping_add(2))?;
        match kind {
            RequestType::Trade => module::open(h, player, target)?,
            RequestType::Assist => h.assist_open(player, target)?,
        }
        return Ok(true);
    }
    close(h, player)?;
    Ok(false)
}

/// `RequestManager.close(player)`: dialogue, main interface, chatbox.
fn close<H: Host>(h: &mut H, player: &H::Player) -> Result<()> {
    h.dialogue_close(player)?;
    let m = h.interface_manager(player)?;
    interfaces::close(&mut Ui::new(h, player), &m)?;
    let m = h.interface_manager(player)?;
    interfaces::close_chatbox(&mut Ui::new(h, player), &m)?;
    Ok(())
}

/// `RequestManager.clear()`.
pub fn clear<H: Host>(h: &mut H, player: &H::Player) -> Result<()> {
    h.set_request_target(player, None)
}
