//! Typed internal owners let State distinguish external references from its
//! ground -> dropper -> actor -> approach -> ground graph edges.
use super::*;
use crate::world::{ground_item as ground, ground_spawn as spawn};

pub(super) enum GroundTarget {
    Ordinary(ground::Handle),
    Spawn(spawn::Handle),
}
impl GroundTarget {
    pub fn new(item: ground::Handle, registry: &spawn::Registry) -> Result<Self> {
        if let Some(spawn) = registry.get(&item) {
            Ok(Self::Spawn(spawn))
        } else if item.has_owner() {
            Err(Error::Unsupported("ground subtype from another registry"))
        } else {
            Ok(Self::Ordinary(item))
        }
    }
    pub fn view(&self) -> ground::Handle {
        match self {
            Self::Ordinary(item) => item.clone(),
            Self::Spawn(spawn) => spawn::as_ground(spawn),
        }
    }
    pub fn external(&self) -> bool {
        match self {
            Self::Ordinary(item) => item.strong_count() > 1,
            // The state owns only the subtype. Its internal base is unleased;
            // exposed ground/Item views retain the subtype through an anchor.
            Self::Spawn(spawn) => {
                spawn.strong_count() > 1 || spawn.read().base().strong_count() > 1
            }
        }
    }
    pub fn active(&self, ticks: i32) -> bool {
        match self {
            Self::Spawn(_) => true,
            Self::Ordinary(item) => {
                let item = item.read();
                item.removed || ticks < item.decay_time
            }
        }
    }
}
