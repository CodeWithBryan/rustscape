//! The canonical player (`player::owned::Player`) an artificial player is.
//!
//! `new AIPlayer` constructs an ordinary `Player` from a `PlayerDetails`
//! whose user name is [`details_username`], and then sets the `artificial`
//! flag. Of the fields the engine's [`super::Host`] reads as plain fields,
//! the canonical profile holds three: `Node.name`, `Player.artificial` and
//! `Player.playing`. A binding answers `Host::name`, `Host::set_artificial`
//! and the repository port's `is_artificial` and `set_playing` with the
//! functions here; the corpus replay does (crates/server/tests/bots.rs).
//! The actor's location and its slot index are the canonical world state's,
//! and every other manager of a player stays a `Host` service.
use crate::player::owned::{Player, profile};

/// `"/aip" + (currentUID + 1) + ":" + name`: the user name of the
/// `PlayerDetails` an artificial player is constructed with, and so its
/// `Node.name`. The counter is read before it is incremented, so the first
/// bot of a world is `/aip2:...` and its uid is 1.
pub fn details_username(current_uid: i32, name: &str) -> String {
    format!("/aip{}:{}", current_uid.wrapping_add(1), name)
}

/// `Node.getName()`.
pub fn name(player: &Player) -> profile::Result<String> {
    player.with_profile(|profile| profile.name.clone())
}

/// `Player.artificial = artificial`.
pub fn set_artificial(player: &Player, artificial: bool) -> profile::Result<()> {
    player.with_profile(|profile| profile.flags.artificial = artificial)
}

/// `Player.isArtificial()`.
pub fn is_artificial(player: &Player) -> profile::Result<bool> {
    player.with_profile(|profile| profile.flags.artificial)
}

/// `Player.setPlaying(playing)`, as `finishClear` calls it.
pub fn set_playing(player: &Player, playing: bool) -> profile::Result<()> {
    player.with_profile(|profile| profile.flags.playing = playing)
}

/// `Player.isPlaying()`.
pub fn is_playing(player: &Player) -> profile::Result<bool> {
    player.with_profile(|profile| profile.flags.playing)
}
