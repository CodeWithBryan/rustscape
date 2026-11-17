//! The remaining core player managers of `core.game.node.entity.player.link`:
//! [`skull`] (SkullManager with SkullIcon and its deep-wilderness container
//! listener), [`hint_icons`] (HintIconManager with HintIconContext and the
//! HintIcon packet), [`warnings`] (WarningMessages), [`assist`]
//! (AssistSession, the ASSIST request module) and [`diary`]
//! (AchievementDiaryManager and AchievementDiary over the persisted diary
//! state, with DiaryType's tables as data in [`diary_data`]).
//!
//! Each is a library over the player's own state, reached through its
//! `Host`'s accessors; no borrow of that state is held across a service call.
//! Everything the originals reach outside these classes (appearance, locks,
//! timers, attributes, containers, packets, the interface manager, saved
//! data, skills, names) is a required service of the module's `Host`, called
//! in the original order with the original arguments. None has a default.
//!
//! Differentially tested against the unchanged classes by
//! `cargo xtask player-managers-oracle` (fixtures/player-managers). See
//! docs/player-managers.md.
pub mod assist;
pub mod diary;
pub mod diary_data;
pub mod hint_icons;
pub mod skull;
pub mod warnings;

use crate::world::player_walk::NodeHandle;

/// A failure of the original, or of a service.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// A null dereference (or null unboxing) in the original.
    Null,
    /// An array or list index outside its bounds in the original.
    Index,
    /// A failing cast (`ClassCastException`) in the original.
    Cast,
    /// A required service failed; preceding effects are kept.
    Binding(String),
}

pub type Result<T> = std::result::Result<T, Error>;

impl From<crate::player::save::Error> for Error {
    /// The persisted diary parser's failures.
    fn from(error: crate::player::save::Error) -> Self {
        match error {
            crate::player::save::Error::Null => Self::Null,
            crate::player::save::Error::Index => Self::Index,
            crate::player::save::Error::Type => Self::Cast,
            other => Self::Binding(format!("{other:?}")),
        }
    }
}

/// A player handle these managers store (skull causes, the listener's
/// player, the assist partners). Equality is the original's object identity.
/// `retained_nodes` is ownership metadata for the stored edges, never a
/// gameplay callback.
pub trait PlayerRef: Clone + PartialEq + Send + Sync + 'static {
    fn retained_nodes(&self) -> Vec<NodeHandle> {
        Vec::new()
    }
}

impl PlayerRef for NodeHandle {
    fn retained_nodes(&self) -> Vec<NodeHandle> {
        vec![self.clone()]
    }
}

/// The containers a player's reward and risk checks read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Inventory,
    Equipment,
    Bank,
}

/// An item as these classes build it: `new Item(id, amount)`.
pub type Item = (i32, i32);

/// Java's `String.split(literal)` for a separator without regex
/// metacharacters: trailing empty strings are dropped, and an input without
/// the separator (the empty string included) is one element.
pub(crate) fn java_split<'a>(text: &'a str, separator: &str) -> Vec<&'a str> {
    if text.is_empty() {
        return vec![""];
    }
    let mut parts: Vec<&str> = text.split(separator).collect();
    while parts.len() > 1 && parts.last() == Some(&"") {
        parts.pop();
    }
    if parts.len() == 1 && parts[0].is_empty() {
        return Vec::new();
    }
    parts
}
