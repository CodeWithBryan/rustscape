//! The AbstractNPC classes a world registers: the `npc_classes::Class`
//! slice and the dragon classes of `world::dragonfire::dragons`, as one
//! [`WorldClass`] the registry (`npc_classes::registry`) holds.
use crate::world::dragonfire::dragons::DragonClass;
use crate::world::npc_classes::{self as nc, Abstract, Class, Constructor};

/// One AbstractNPC subclass a world can register.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum WorldClass {
    Npc(Class),
    Dragon(DragonClass),
}

impl WorldClass {
    /// `getClass().getName()`.
    pub fn name(self) -> &'static str {
        match self {
            Self::Npc(c) => c.name(),
            Self::Dragon(d) => Abstract::name(&d),
        }
    }

    /// The class of a scanned name.
    pub fn named(name: &str) -> Option<Self> {
        Class::named(name).map(Self::Npc).or_else(|| {
            DragonClass::ALL
                .into_iter()
                .find(|d| Abstract::name(d) == name)
                .map(Self::Dragon)
        })
    }

    /// `getWalkRadius()` where the class overrides it.
    pub fn walk_radius(self) -> Option<i32> {
        match self {
            Self::Npc(c) => c.walk_radius(),
            Self::Dragon(_) => None,
        }
    }

    /// The `npc_classes::Class` slice, when it is one.
    pub fn npc(self) -> Option<Class> {
        match self {
            Self::Npc(c) => Some(c),
            Self::Dragon(_) => None,
        }
    }
}

impl From<Class> for WorldClass {
    fn from(c: Class) -> Self {
        Self::Npc(c)
    }
}

impl From<DragonClass> for WorldClass {
    fn from(d: DragonClass) -> Self {
        Self::Dragon(d)
    }
}

impl Abstract for WorldClass {
    fn name(&self) -> &'static str {
        WorldClass::name(*self)
    }
    fn ids(&self) -> Vec<i32> {
        match self {
            Self::Npc(c) => Abstract::ids(c),
            Self::Dragon(d) => Abstract::ids(d),
        }
    }
    fn prototype(&self) -> (i32, Constructor) {
        match self {
            Self::Npc(c) => Abstract::prototype(c),
            Self::Dragon(d) => Abstract::prototype(d),
        }
    }
    fn construct(&self, location: Option<[i32; 3]>) -> nc::Result<Option<Constructor>> {
        match self {
            Self::Npc(c) => Abstract::construct(c, location),
            Self::Dragon(d) => Abstract::construct(d, location),
        }
    }
}
