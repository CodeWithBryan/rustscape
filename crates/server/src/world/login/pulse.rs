//! The world-side finish of a login: the body of LoginParser's `Pulse(1)`,
//! which the world pulse queue runs on the tick after the admission, and
//! `LoginParser.reconnect`.
use super::{Error, Host, Result, admission::flag, configuration, ip};
use crate::world::repository::Repository;
use rs09_protocol::login::AuthResponse;
use std::hash::Hash;

/// LoginParser's anonymous `Pulse(1)` and what it captured: the player, the
/// details' session, the reconnect flag and the `lateinit` parser, which is
/// unset when `PlayerParser.parse` failed.
pub struct LoginPulse<P, Parser, Session> {
    pub player: P,
    pub session: Session,
    pub reconnect: bool,
    pub parser: Option<Parser>,
}

/// `Pulse(1)`: the delay the original submits it with.
pub const DELAY: i32 = 1;

/// `pulse()`: always answers true (the pulse runs once). A `Throwable` from
/// the body is printed, the player removed and ErrorLoadingProfile flagged;
/// a failure of that recovery leaves the pulse, as it leaves the original's.
pub fn run<P: Clone + Eq + Hash, H: Host<P>>(
    host: &mut H,
    repository: &Repository<P>,
    login_listeners: &[String],
    pulse: &LoginPulse<P, H::Parser, H::Session>,
) -> Result<bool> {
    match body(host, repository, login_listeners, pulse) {
        Ok(()) => {}
        Err(error) if error.is_throwable() => {
            host.print_stack(&error);
            repository.remove_player(host, &pulse.player)?;
            let session = details_session(host, &pulse.player)?;
            flag(
                host,
                repository,
                &session,
                AuthResponse::ErrorLoadingProfile,
            )?;
        }
        Err(error) => return Err(error),
    }
    Ok(true)
}

fn body<P: Clone + Eq + Hash, H: Host<P>>(
    host: &mut H,
    repository: &Repository<P>,
    login_listeners: &[String],
    pulse: &LoginPulse<P, H::Parser, H::Session>,
) -> Result<()> {
    let player = &pulse.player;
    let session = details_session(host, player)?;
    if !host.session_active(&session) {
        repository.remove_player(host, player)?;
        return Ok(());
    }
    let home = host.constants().home_location;
    let spawn = host.saved_spawn_location(player, home)?;
    host.set_spawn_location(player, spawn)?;
    for listener in login_listeners {
        host.login_listener(listener, player)?;
    }
    let parser = pulse.parser.as_ref().ok_or_else(|| {
        Error::Exception(
            "UninitializedPropertyAccessException: lateinit property parser has not been initialized"
                .into(),
        )
    })?;
    host.run_content_hooks(player, parser)?;
    let session = details_session(host, player)?;
    host.set_session_object(&session, Some(player.clone()));
    // A Kotlin setter of a non-null String refuses Java's null.
    host.with_details(player, |details| match ip(details) {
        Some(address) => {
            crate::auth::storage::lock(&details.account).last_used_ip = address;
            Ok(())
        }
        None => Err(Error::Exception(
            "NullPointerException: accountInfo.lastUsedIp".into(),
        )),
    })??;
    if pulse.reconnect {
        reconnect(host, repository, pulse)
    } else {
        let session = details_session(host, player)?;
        flag(host, repository, &session, AuthResponse::Success)?;
        configuration::player_init(host, repository, player)?;
        host.reinit_varps(player)
    }
}

/// `LoginParser.reconnect(player)`.
fn reconnect<P: Clone + Eq + Hash, H: Host<P>>(
    host: &mut H,
    repository: &Repository<P>,
    pulse: &LoginPulse<P, H::Parser, H::Session>,
) -> Result<()> {
    let player = &pulse.player;
    let name = super::username(host, player)?;
    repository.disconnection_queue().remove(Some(&name));
    host.init_reconnect(player)?;
    host.set_active(player, true);
    let session = details_session(host, player)?;
    flag(host, repository, &session, AuthResponse::Success)?;
    host.update_scene_graph(player, true)?;
    host.reinit_varps(player)?;
    configuration::configure_game_world(host, repository, player)
}

/// `details.session`, read where LoginParser reads it.
fn details_session<P: Clone + Eq + Hash, H: Host<P>>(
    host: &mut H,
    player: &P,
) -> Result<H::Session> {
    host.session(player)
        .ok_or_else(|| Error::Exception("NullPointerException: details.session".into()))
}
