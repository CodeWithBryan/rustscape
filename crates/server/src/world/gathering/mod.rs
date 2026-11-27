//! Gathering skills: the pinned node/resource/tool tables, woodcutting,
//! mining and fishing, ported from `content.global.skill.gather`,
//! `content.global.skill.fishing` and `SkillingTool`.
//!
//! This is a library of original bodies over explicit required services
//! ([`service::Context`]); only the woodcutting pulse runs in the live world
//! (`world::live::items::woodcutting`). See
//! docs/gathering.md for what the JVM corpus does and does not establish.
pub mod binding;
pub mod fishing;
pub mod mining;
pub mod nodes;
pub mod random;
pub mod service;
pub mod tables;
pub mod tools;
pub mod woodcutting;

/// World inputs the original reads from static configuration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    /// `ServerConstants.MAX_PLAYERS`, the divisor of every respawn duration.
    pub max_players: i32,
}
