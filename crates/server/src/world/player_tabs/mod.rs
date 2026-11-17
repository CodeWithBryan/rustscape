//! The player's core tab systems: the weapon tab (`WeaponInterface`), the
//! level-up reward and flashing skill icons (`LevelUp`, `LevelUpJingles`), the
//! emote tab (`EmoteManager`, `Emotes`), the music player (`MusicPlayer`,
//! `MusicZone.enter`) and the quest tab (`QuestRepository.syncronizeTab`,
//! `Quest.getConfig`, `forButtonId`, `forIndex`).
//!
//! Each is a library over the player's own state: the weapon tab's fields are
//! [`weapon::WeaponInterface`] and its component handle, the emote and music
//! unlocks are the persisted state in `player::persisted` (joined, not
//! copied), the quest stages are `persisted::quests::Repository`. Everything
//! the original reaches outside these classes (packets, varps, attributes,
//! Settings, Properties, equipment, item configuration, skills, locks, the
//! animator, the interface manager, content) is a required service of the
//! system's `Host`, called in the original order with the original
//! arguments. None has a default. Content stays behind services: each quest's
//! `getConfig` override and `updateVarps`, the SALUTE, GOBLIN_BOW and
//! GIVE_THANKS emote overrides, `Skillcape.trim`.
//!
//! Differentially tested against the unchanged classes by
//! `cargo xtask player-tabs-oracle` (fixtures/player-tabs). See
//! docs/player-tabs.md.
pub mod emotes;
pub mod level_up;
pub mod music;
pub mod quests;
pub mod weapon;

use crate::world::interfaces;

/// A failure of the original, or of a service.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// A null dereference (or null unboxing) in the original.
    Null,
    /// An array or list index outside its bounds in the original.
    Index,
    /// A failing cast (`ClassCastException`) in the original.
    Cast,
    /// An `IllegalStateException` the original throws, with its message.
    IllegalState(String),
    /// A required service failed; preceding effects are kept.
    Binding(String),
}

pub type Result<T> = std::result::Result<T, Error>;

impl From<crate::player::save::Error> for Error {
    /// The persisted state's failures: only null unboxing reaches here.
    fn from(error: crate::player::save::Error) -> Self {
        match error {
            crate::player::save::Error::Null => Self::Null,
            crate::player::save::Error::Index => Self::Index,
            crate::player::save::Error::Type => Self::Cast,
            other => Self::Binding(format!("{other:?}")),
        }
    }
}

/// A value these classes store in, or read from, the entity's attributes.
#[derive(Clone)]
pub enum Value {
    Int(i32),
    Bool(bool),
    /// The `Component` object `openAutocastSelect` stores.
    Component(interfaces::Handle),
}

impl std::fmt::Debug for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Int(v) => write!(f, "Int({v})"),
            Self::Bool(v) => write!(f, "Bool({v})"),
            Self::Component(c) => write!(f, "Component({})", c.read().id),
        }
    }
}

impl Value {
    /// `(boolean) value`: another type is a `ClassCastException`.
    pub fn boolean(&self) -> Result<bool> {
        match self {
            Self::Bool(v) => Ok(*v),
            _ => Err(Error::Cast),
        }
    }

    /// `(int) value`: another type is a `ClassCastException`.
    pub fn int(&self) -> Result<i32> {
        match self {
            Self::Int(v) => Ok(*v),
            _ => Err(Error::Cast),
        }
    }
}

/// `ContentAPI.removeAttributes(entity, keys...)`: each key without its
/// `/save:` prefix, through `entity.removeAttribute`.
pub(crate) fn remove_attributes(
    keys: &[&str],
    mut remove: impl FnMut(&str) -> Result<()>,
) -> Result<()> {
    for key in keys {
        remove(&key.replace("/save:", ""))?;
    }
    Ok(())
}
