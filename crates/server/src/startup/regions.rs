//! The per-region configuration the start-up loaders write into
//! `RegionManager`'s regions: scenery overrides (`Region.addSceneries` /
//! `removeSceneries`), the region-wide music track and tile music zones.
//!
//! `RegionManager.forId` creates and caches a `Region` for any id it is asked
//! for, so [`Regions::for_id`] does too, and [`Regions::ids`] is the set of
//! region ids the original's `REGION_CACHE` holds because of these loaders.
//! Loading a region's map, collision and objects is `world::region` /
//! `world::scene`; the two are not joined yet (see docs/startup.md).
use crate::player::attributes::AttributeLocation;
use std::{collections::HashMap, sync::Arc};

/// `new Scenery(id, location, type, rotation)` as the override parser builds it,
/// after the constructor's adjustments (an id below 1 makes the type 22).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SceneryOverride {
    pub id: i32,
    /// `Location(x, y, z)`: 15-bit signed x and y, z modulo 4.
    pub location: [i32; 3],
    pub kind: i32,
    pub rotation: i32,
}
impl SceneryOverride {
    pub fn new(id: i32, x: i32, y: i32, z: i32, kind: i32, rotation: i32) -> Self {
        let location = AttributeLocation::new(x, y, z).coordinates();
        Self {
            id,
            location,
            kind: if id < 1 { 22 } else { kind },
            rotation: if rotation < 0 { rotation + 4 } else { rotation },
        }
    }
    /// `Location.getRegionId()`.
    pub fn region_id(&self) -> i32 {
        ((self.location[0] >> 6) << 8) | (self.location[1] >> 6)
    }
}

/// `core.game.world.map.zone.ZoneBorders` as the music loader constructs it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ZoneBorders {
    pub south_west_x: i32,
    pub south_west_y: i32,
    pub north_east_x: i32,
    pub north_east_y: i32,
    pub plane: i32,
    /// `null` until the first `addException`.
    pub exceptions: Option<Vec<ZoneBorders>>,
}
impl ZoneBorders {
    /// `ZoneBorders(x1, y1, x2, y2)`.
    pub fn new(x1: i32, y1: i32, x2: i32, y2: i32) -> Self {
        Self {
            south_west_x: x1.min(x2),
            south_west_y: y1.min(y2),
            north_east_x: x1.max(x2),
            north_east_y: y1.max(y2),
            plane: 0,
            exceptions: None,
        }
    }
    pub fn add_exception(&mut self, exception: ZoneBorders) {
        self.exceptions.get_or_insert_with(Vec::new).push(exception);
    }
    /// `getRegionIds()`, in its x-then-y order.
    pub fn region_ids(&self) -> Vec<i32> {
        let mut ids = Vec::new();
        let ne_x = (self.north_east_x >> 6).wrapping_add(1);
        let ne_y = (self.north_east_y >> 6).wrapping_add(1);
        let mut x = self.south_west_x >> 6;
        while x < ne_x {
            let mut y = self.south_west_y >> 6;
            while y < ne_y {
                ids.push(y | (x << 8));
                y += 1;
            }
            x += 1;
        }
        ids
    }
}

/// `MusicZone(musicId, borders)`. One zone object is shared by every region
/// its borders touch.
#[derive(Debug, PartialEq, Eq)]
pub struct MusicZone {
    pub music_id: i32,
    pub borders: ZoneBorders,
}

/// The fields of one `Region` that the loaders write.
#[derive(Clone, Debug)]
pub struct RegionConfig {
    pub add_sceneries: Vec<SceneryOverride>,
    pub remove_sceneries: Vec<SceneryOverride>,
    /// `-1` when the region has no region-wide track.
    pub music: i32,
    pub music_zones: Vec<Arc<MusicZone>>,
}
impl Default for RegionConfig {
    fn default() -> Self {
        Self {
            add_sceneries: Vec::new(),
            remove_sceneries: Vec::new(),
            music: -1,
            music_zones: Vec::new(),
        }
    }
}

/// The regions the start-up loaders created, by id.
#[derive(Clone, Debug, Default)]
pub struct Regions {
    regions: HashMap<i32, RegionConfig>,
}
impl Regions {
    /// `RegionManager.forId(id)`: the cached region, created on first use.
    pub fn for_id(&mut self, id: i32) -> &mut RegionConfig {
        self.regions.entry(id).or_default()
    }
    pub fn get(&self, id: i32) -> Option<&RegionConfig> {
        self.regions.get(&id)
    }
    /// Every region id created so far, ascending.
    pub fn ids(&self) -> Vec<i32> {
        let mut ids: Vec<i32> = self.regions.keys().copied().collect();
        ids.sort_unstable();
        ids
    }
    pub fn len(&self) -> usize {
        self.regions.len()
    }
    pub fn is_empty(&self) -> bool {
        self.regions.is_empty()
    }
    /// Clears what the loaders wrote, keeping the regions (the oracle's case
    /// reset does the same between authored inputs).
    pub fn clear_configuration(&mut self) {
        for region in self.regions.values_mut() {
            *region = RegionConfig::default();
        }
    }
}
