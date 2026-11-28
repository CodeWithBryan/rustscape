//! Gathering skills: the pinned node/resource/tool tables, woodcutting,
//! mining and fishing, ported from `content.global.skill.gather`,
//! `content.global.skill.fishing` and `SkillingTool`.
//!
//! This is a library of original bodies over explicit required services
//! ([`service::Context`]); woodcutting (the pulse and the listener's
//! script) and mining (the listener's script and the prospect option) run
//! in the live world (`world::live::items::{woodcutting, mining}`). See
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
