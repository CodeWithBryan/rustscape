//! The live world: the tick driver ([`super::driver`]), the login
//! ([`super::login`]), the repository, social and the canonical players bound
//! to a running server (`rs09-server --world`).
//!
//! What this module is:
//!
//! - [`boot`]: the native start-up ([`crate::startup::sequence`]) over a world
//!   directory assembled from the data directory, the cache and the saves
//!   directory, and the canonical world built from what it loaded.
//! - [`host`]: the production [`super::driver::Host`],
//!   [`super::repository::Host`] and [`super::login::Host`] over that world
//!   (the shape of the test bindings in `tests/world_login.rs` and
//!   `tests/world_login/tick_driver.rs`, with every replayed stand-in replaced
//!   by a native library or by a named "not ported" answer).
//! - [`inner`]: the canonical state those hosts answer from, the interface
//!   context, the social services and the contexts of the player's own
//!   libraries (skills, Settings, equipment, interaction options, the save
//!   file).
//! - [`walk`]: walking on the canonical actor state (`player_walk`), the
//!   cache scene for collision, `Player.tick`'s entity part and the player
//!   update masks.
//! - [`server`]: the 600 ms worker loop, the transport gate
//!   ([`super::session::World`]), the saver thread and [`server::run`], which
//!   `main.rs` calls for `--world`.
//! - [`ground_items`]: ground items (the manager, the start-up's spawns,
//!   ground item options and pickup, drops).
//! - [`not_ported`]: the registry of named answers for systems without a
//!   port; each is listed in docs/live-world.md.
//!
//! This is a playable engine slice, not a complete server: content (login
//! listeners, plugins, quests, music, emotes, NPC behaviour, tutorial) is not
//! ported and is answered by name. See docs/live-world.md.
pub mod appearance;
pub mod bank;
pub mod boot;
pub mod buttons;
pub mod combat;
pub mod content;
pub mod destination;
pub mod door_claims;
pub mod doors;
pub mod dragons;
pub mod eating;
pub mod equipment;
pub mod faults;
pub mod follow;
pub mod global;
pub mod graves;
pub mod ground;
pub mod ground_items;
pub mod host;
pub mod inner;
pub mod items;
pub mod magic;
pub mod not_ported;
pub mod npc_life;
pub mod npcs;
pub mod region_zones;
pub mod regions;
pub mod round;
pub mod runtime_npcs;
pub mod scenery;
pub mod scripts;
pub mod server;
pub mod shopping;
pub mod subclasses;
pub mod tabs;
pub mod talk;
pub mod timers;
pub mod trading;
pub mod walk;

use super::player_walk::NodeHandle;
use std::hash::{Hash, Hasher};

/// A player of the live world: the canonical actor's retained node handle.
/// Equality is the handle's (one allocation per actor); the hash is its node
/// id, which equal handles share.
#[derive(Clone, PartialEq, Eq)]
pub struct P(pub NodeHandle);
impl Hash for P {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.node.hash(state);
    }
}
impl std::fmt::Debug for P {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "P({})", self.0.node)
    }
}
impl P {
    pub fn node(&self) -> usize {
        self.0.node
    }
}

/// A session of the transport, by number.
pub type SessionId = u64;

/// `System.currentTimeMillis()`.
pub fn now_millis() -> i64 {
    crate::runtime::wall_clock()
}
