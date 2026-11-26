//! FamiliarManager's persisted values: pet details per pet item id in the
//! HashMap's order, and the current pet or familiar with its ticks, special
//! points, lifepoints and beast-of-burden container.
//!
//! The pet details are the original's `PetDetails` objects, shared: the
//! manager's lists and a `Pet` that took one hold the same object, so a
//! pet's hunger and growth reach the save without copying. The world's
//! familiar manager (`world::familiars::manager`) works on this same
//! [`PetStore`], and its `parse` is `FamiliarManager.parse`; the active
//! familiar of a live world is read off its manager when saving
//! (`world::familiars::Manager::saved_active`).
use super::IntHashMap;
use crate::player::save::{ActiveFamiliar, Familiars, PetDetail};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

/// `PetDetails`: hunger and growth, both doubles. One object, shared by
/// reference wherever the original shares it.
#[derive(Debug, Default)]
pub struct PetDetails {
    hunger: AtomicU64,
    growth: AtomicU64,
}
/// A reference to one `PetDetails` object.
pub type Details = Arc<PetDetails>;

impl PetDetails {
    /// `new PetDetails(growth)`: hunger starts at 0.0.
    pub fn new(growth: f64) -> Details {
        Arc::new(Self {
            hunger: AtomicU64::new(0f64.to_bits()),
            growth: AtomicU64::new(growth.to_bits()),
        })
    }
    pub fn hunger(&self) -> f64 {
        f64::from_bits(self.hunger.load(Ordering::Relaxed))
    }
    pub fn growth(&self) -> f64 {
        f64::from_bits(self.growth.load(Ordering::Relaxed))
    }
    /// `setHunger`.
    pub fn set_hunger(&self, value: f64) {
        self.hunger.store(value.to_bits(), Ordering::Relaxed);
    }
    /// `setGrowth`.
    pub fn set_growth(&self, value: f64) {
        self.growth.store(value.to_bits(), Ordering::Relaxed);
    }
    /// `updateHunger`: add, then raise anything below 0.0 to 0.0.
    pub fn update_hunger(&self, amount: f64) {
        let mut hunger = self.hunger() + amount;
        if hunger < 0.0 {
            hunger = 0.0;
        }
        self.set_hunger(hunger);
    }
    /// `updateGrowth`: add, then clamp to 0.0..=100.0.
    #[allow(clippy::manual_clamp)] // the original order of checks, kept
    pub fn update_growth(&self, amount: f64) {
        let mut growth = self.growth() + amount;
        if growth < 0.0 {
            growth = 0.0;
        } else if growth > 100.0 {
            growth = 100.0;
        }
        self.set_growth(growth);
    }
    /// The values the save writes.
    pub fn value(&self) -> PetDetail {
        PetDetail {
            hunger: self.hunger(),
            growth: self.growth(),
        }
    }
}

/// `FamiliarManager.petDetails`, a `HashMap<Integer, ArrayList<PetDetails>>`
/// that is only put into, read and never removed from by key. Clones share
/// the one map.
#[derive(Clone, Debug, Default)]
pub struct PetStore(Arc<Mutex<IntHashMap<Vec<Details>>>>);

impl PetStore {
    fn lock(&self) -> MutexGuard<'_, IntHashMap<Vec<Details>>> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
    /// Runs `f` on the map. Callers never call out while inside `f`.
    pub fn with<R>(&self, f: impl FnOnce(&mut IntHashMap<Vec<Details>>) -> R) -> R {
        f(&mut self.lock())
    }
    pub fn len(&self) -> usize {
        self.lock().len()
    }
    pub fn is_empty(&self) -> bool {
        self.lock().is_empty()
    }
    /// Whether two stores are the same map.
    pub fn same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
    /// The entries' values in the map's iteration order (a copy).
    pub fn iter(&self) -> std::vec::IntoIter<(i32, Vec<PetDetail>)> {
        self.saved().into_iter()
    }
    /// `put(itemId, list)` of new `PetDetails` objects with these values.
    pub fn put(&self, item_id: i32, details: Vec<PetDetail>) {
        let list = details
            .into_iter()
            .map(|d| {
                let object = PetDetails::new(d.growth);
                object.set_hunger(d.hunger);
                object
            })
            .collect();
        self.with(|map| map.put(item_id, list));
    }
    /// The entries in the map's iteration order, as the save writes them.
    pub fn saved(&self) -> Vec<(i32, Vec<PetDetail>)> {
        self.with(|map| {
            map.iter()
                .map(|(id, list)| (id, list.iter().map(|d| d.value()).collect()))
                .collect()
        })
    }
}

pub struct Manager {
    pub pets: PetStore,
    pub active: ActiveFamiliar,
}
impl Default for Manager {
    fn default() -> Self {
        Self {
            pets: PetStore::default(),
            active: ActiveFamiliar::None,
        }
    }
}
impl Manager {
    pub fn saved(&self) -> Familiars {
        Familiars {
            pets: self.pets.saved(),
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
