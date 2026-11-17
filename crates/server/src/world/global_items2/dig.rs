//! `SpadeDigOptionPlugin`, `SpadeDigListener` and
//! `core.game.global.action.DigSpadeHandler`: the spade's dig option and the
//! two location registries content fills (Kotlin listeners first, then the
//! older DigActions run a tick later from a world pulse).
use super::{Anim, Error, Host, LogLevel, Result, Task};

/// `Items.SPADE_952`.
pub const SPADE: i32 = 952;
/// `Sounds.DIGSPADE_1470`.
pub const DIG_SOUND: i32 = 1470;
/// `DigSpadeHandler.ANIMATION` (`Animation.create(830)`).
pub const ANIMATION: Anim = Anim::new(830);
/// The class DigSpadeHandler.register logs as.
pub const LOG_ORIGIN: &str = "core.game.system.communication.CommunicationInfo";

/// `SpadeDigOptionPlugin.handle`: the dig sound, then `DigSpadeHandler.dig`;
/// nothing found is a message. Always true.
pub fn spade<H: Host + ?Sized>(h: &mut H, player: &H::Node) -> Result<bool> {
    h.play_audio(player, DIG_SOUND, 0, 1, None, 8)?;
    if !dig(h, player)? {
        h.send_message(player, "You dig but find nothing.")?;
    }
    Ok(true)
}

/// `DigSpadeHandler.dig(player)`: the action is looked up before the
/// animation and the lock; a SpadeDigListener at the location wins, else
/// the action runs from a world pulse of delay 1.
pub fn dig<H: Host + ?Sized>(h: &mut H, player: &H::Node) -> Result<bool> {
    let location = h.location(player)?;
    let action = location.and_then(|at| {
        h.statics()
            .dig_actions
            .iter()
            .find(|(key, _)| *key == at)
            .map(|(_, action)| action.clone())
    });
    h.animate(player, ANIMATION)?;
    h.lock(player, 1)?;
    // `runListener(location: Location, ...)`: Kotlin's parameter check.
    let location = h.location(player)?.ok_or(Error::Null)?;
    if run_listener(h, location, player)? {
        return Ok(true);
    }
    if let Some(action) = action {
        h.submit_world(
            Task::Dig {
                player: player.clone(),
                action,
            },
            1,
        )?;
        return Ok(true);
    }
    Ok(false)
}

/// The DigSpadeHandler pulse's `pulse()`: runs the action, then finishes.
pub fn dig_pulse<H: Host + ?Sized>(
    h: &mut H,
    player: &H::Node,
    action: &H::Callback,
) -> Result<bool> {
    h.run_dig_action(player, action)?;
    Ok(true)
}

/// `DigSpadeHandler.register(location, action)`: a location already taken
/// logs an error and refuses.
pub fn register<H: Host + ?Sized>(
    h: &mut H,
    location: [i32; 3],
    action: H::Callback,
) -> Result<bool> {
    if h.statics()
        .dig_actions
        .iter()
        .any(|(key, _)| *key == location)
    {
        let [x, y, z] = location;
        h.log(
            LOG_ORIGIN,
            LogLevel::Err,
            &format!("Already contained dig reward for location [{x}, {y}, {z}]."),
        )?;
        return Ok(false);
    }
    h.statics().dig_actions.push((location, action));
    Ok(true)
}

/// `SpadeDigListener.registerListener(location, method)`: `putIfAbsent`.
pub fn register_listener<H: Host + ?Sized>(h: &mut H, location: [i32; 3], method: H::Callback) {
    let listeners = &mut h.statics().spade_listeners;
    if !listeners.iter().any(|(key, _)| *key == location) {
        listeners.push((location, method));
    }
}

/// `SpadeDigListener.runListener(location, player)`.
pub fn run_listener<H: Host + ?Sized>(
    h: &mut H,
    location: [i32; 3],
    player: &H::Node,
) -> Result<bool> {
    let method = h
        .statics()
        .spade_listeners
        .iter()
        .find(|(key, _)| *key == location)
        .map(|(_, method)| method.clone());
    match method {
        Some(method) => {
            h.run_spade_listener(player, &method)?;
            Ok(true)
        }
        None => Ok(false),
    }
}
