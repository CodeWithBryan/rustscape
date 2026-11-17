//! FamiliarManager's persisted values: pet details per pet item id in the
//! HashMap's order, and the current pet or familiar with its ticks, special
//! points, lifepoints and beast-of-burden container. Only the saving side is
//! native. FamiliarManager.parse constructs familiars from the FAMILIARS
//! registry and pets from the Pets enum, and migrates older formats; it stays
//! a required service, as do summoning and the familiar NPCs.
use super::IntHashMap;
use crate::player::save::{ActiveFamiliar, Familiars, PetDetail};

pub struct Manager {
    pub pets: IntHashMap<Vec<PetDetail>>,
    pub active: ActiveFamiliar,
}
impl Default for Manager {
    fn default() -> Self {
        Self {
            pets: IntHashMap::default(),
            active: ActiveFamiliar::None,
        }
    }
}
impl Manager {
    pub fn saved(&self) -> Familiars {
        Familiars {
            pets: self.pets.iter().map(|(id, d)| (id, d.clone())).collect(),
            active: match &self.active {
                ActiveFamiliar::None => ActiveFamiliar::None,
                ActiveFamiliar::Pet { item_id } => ActiveFamiliar::Pet { item_id: *item_id },
                ActiveFamiliar::Familiar {
                    original_id,
                    ticks,
                    special_points,
                    burden,
                    lifepoints,
                } => ActiveFamiliar::Familiar {
                    original_id: *original_id,
                    ticks: *ticks,
                    special_points: *special_points,
                    burden: burden.clone(),
                    lifepoints: *lifepoints,
                },
            },
        }
    }
}
