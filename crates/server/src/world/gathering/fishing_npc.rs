//! `FishingNPC` and `FishSpots` (`content.global.skill.fishing`): the NPC
//! behaviour that moves a fishing spot between its area's tiles.
//!
//! The behaviour is registered for `FishingSpot.getAllIds()`. `onCreation`
//! stores the spot's area (`fishing:spot`, [`FishSpots::for_location`] of the
//! NPC's location) and `fishing:switchdelay = 0`; `tick` moves the spot once
//! the delay is below the world's tick count and always answers `false`, so
//! `NPC.handleTickActions` does nothing else for a fishing spot.
//!
//! `moveSpot`'s `null` branch (toggling `isInvisible`) is unreachable: `tick`
//! returns before it when the attribute is null, and `forLocation` never
//! answers null. It is not ported.
//!
//! No JVM corpus covers this module; the unit tests below check the port
//! against the source's control flow only.
use std::{collections::HashMap, sync::OnceLock};

use super::fishing::FishingSpot;

/// A location as `[x, y, z]`.
pub type Tile = [i32; 3];

/// `FishSpots`: each area's tiles in declaration order, repeats included.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FishSpots {
    Catherby,
    Entrana,
    EntranaPlatform,
    Seers,
    Draynor,
    Barbarian,
    Lumbridge,
    LumbridgeSwamp,
    LumbridgeMarsh,
    Alkharid,
    FishingGuild,
    Gnome,
    Castlewar,
    Shilo,
    Karamja,
    Piscatoris,
    TutorialIsland,
    WildernessArea,
}

impl FishSpots {
    /// `FishSpots.values()`.
    pub const VALUES: [FishSpots; 18] = [
        FishSpots::Catherby,
        FishSpots::Entrana,
        FishSpots::EntranaPlatform,
        FishSpots::Seers,
        FishSpots::Draynor,
        FishSpots::Barbarian,
        FishSpots::Lumbridge,
        FishSpots::LumbridgeSwamp,
        FishSpots::LumbridgeMarsh,
        FishSpots::Alkharid,
        FishSpots::FishingGuild,
        FishSpots::Gnome,
        FishSpots::Castlewar,
        FishSpots::Shilo,
        FishSpots::Karamja,
        FishSpots::Piscatoris,
        FishSpots::TutorialIsland,
        FishSpots::WildernessArea,
    ];
    /// `locations`.
    pub fn locations(self) -> &'static [Tile] {
        match self {
            FishSpots::Catherby => &[
                [2844, 3429, 0],
                [2839, 3431, 0],
                [2836, 3431, 0],
                [2837, 3431, 0],
                [2838, 3431, 0],
                [2839, 3431, 0],
                [2840, 3431, 0],
                [2844, 3429, 0],
                [2845, 3429, 0],
                [2846, 3429, 0],
                [2853, 3423, 0],
                [2854, 3423, 0],
                [2855, 3423, 0],
                [2859, 3426, 0],
                [2860, 3426, 0],
            ],
            FishSpots::Entrana => &[
                [2842, 3359, 0],
                [2843, 3359, 0],
                [2844, 3359, 0],
                [2847, 3361, 0],
                [2848, 3361, 0],
                [2849, 3361, 0],
                [2850, 3361, 0],
            ],
            FishSpots::EntranaPlatform => &[
                [2879, 3334, 0],
                [2879, 3335, 0],
                [2879, 3338, 0],
                [2879, 3339, 0],
                [2877, 3342, 0],
                [2876, 3342, 0],
                [2875, 3342, 0],
                [2875, 3331, 0],
                [2876, 3331, 0],
                [2877, 3331, 0],
            ],
            FishSpots::Seers => &[
                [2714, 3533, 0],
                [2714, 3532, 0],
                [2726, 3524, 0],
                [2727, 3524, 0],
                [2728, 3524, 0],
            ],
            FishSpots::Draynor => &[[3085, 3230, 0], [3086, 3227, 0]],
            FishSpots::Barbarian => &[
                [3110, 3432, 0],
                [3110, 3433, 0],
                [3110, 3434, 0],
                [3104, 3425, 0],
                [3104, 3424, 0],
            ],
            FishSpots::Lumbridge => &[
                [3239, 3243, 0],
                [3239, 3241, 0],
                [3239, 3242, 0],
                [3238, 3251, 0],
                [3238, 3252, 0],
                [3238, 3253, 0],
            ],
            FishSpots::LumbridgeSwamp => &[
                [3242, 3143, 0],
                [3242, 3148, 0],
                [3241, 3148, 0],
                [3240, 3147, 0],
                [3240, 3146, 0],
                [3244, 3150, 0],
                [3245, 3152, 0],
                [3246, 3155, 0],
                [3246, 3156, 0],
                [3239, 3241, 0],
            ],
            FishSpots::LumbridgeMarsh => &[
                [3169, 3266, 0],
                [3168, 3266, 0],
                [3174, 3274, 0],
                [3173, 3274, 0],
            ],
            FishSpots::Alkharid => &[
                [3267, 3148, 0],
                [3268, 3147, 0],
                [3276, 3140, 0],
                [3275, 3140, 0],
            ],
            FishSpots::FishingGuild => &[
                [2612, 3411, 0],
                [2602, 3423, 0],
                [2602, 3411, 0],
                [2602, 3412, 0],
                [2602, 3414, 0],
                [2602, 3415, 0],
                [2603, 3417, 0],
                [2604, 3417, 0],
                [2605, 3416, 0],
                [2606, 3416, 0],
                [2607, 3416, 0],
                [2608, 3416, 0],
                [2609, 3416, 0],
                [2610, 3416, 0],
                [2611, 3416, 0],
                [2612, 3415, 0],
                [2612, 3414, 0],
                [2607, 3410, 0],
                [2606, 3410, 0],
                [2605, 3410, 0],
                [2599, 3419, 0],
                [2600, 3419, 0],
                [2602, 3419, 0],
                [2603, 3419, 0],
                [2604, 3419, 0],
                [2605, 3420, 0],
                [2605, 3421, 0],
                [2604, 3422, 0],
                [2603, 3422, 0],
                [2602, 3422, 0],
                [2601, 3422, 0],
                [2605, 3424, 0],
                [2605, 3425, 0],
                [2602, 3426, 0],
                [2603, 3426, 0],
                [2604, 3426, 0],
            ],
            FishSpots::Gnome => &[
                [2388, 3424, 0],
                [2390, 3422, 0],
                [2391, 3421, 0],
                [2382, 3413, 0],
                [2382, 3414, 0],
                [2382, 3415, 0],
            ],
            FishSpots::Castlewar => &[
                [2465, 3156, 0],
                [2464, 3155, 0],
                [2468, 3157, 0],
                [2469, 3157, 0],
                [2471, 3158, 0],
                [2472, 3156, 0],
                [2462, 3145, 0],
                [2461, 3150, 0],
                [2461, 3151, 0],
            ],
            FishSpots::Shilo => &[
                [2460, 3144, 0],
                [2461, 3150, 0],
                [2461, 3151, 0],
                [2853, 2977, 0],
                [2854, 2977, 0],
                [2855, 2977, 0],
                [2856, 2977, 0],
                [2858, 2976, 0],
                [2859, 2976, 0],
                [2860, 2976, 0],
                [2863, 2975, 0],
                [2863, 2975, 0],
                [2864, 2975, 0],
                [2865, 2972, 0],
                [2860, 2972, 0],
                [2855, 2974, 0],
                [2852, 2973, 0],
            ],
            FishSpots::Karamja => &[
                [2923, 3178, 0],
                [2923, 3179, 0],
                [2923, 3180, 0],
                [2924, 3181, 0],
                [2925, 3181, 0],
                [2926, 3180, 0],
                [2926, 3179, 0],
                [2926, 3178, 0],
                [2926, 3177, 0],
                [2926, 3176, 0],
            ],
            FishSpots::Piscatoris => &[
                [2341, 3702, 0],
                [2342, 3702, 0],
                [2343, 3702, 0],
                [2344, 3702, 0],
                [2345, 3702, 0],
                [2346, 3702, 0],
                [2347, 3702, 0],
                [2349, 3702, 0],
                [2352, 3703, 0],
            ],
            FishSpots::TutorialIsland => &[[3101, 3092, 0]],
            FishSpots::WildernessArea => &[
                [3187, 3927, 0],
                [3185, 3926, 0],
                [3183, 3926, 0],
                [3181, 3926, 0],
            ],
        }
    }
    /// `FishSpots.forLocation`: the area owning the tile (the last declared
    /// area wins a tile two areas list), `TUTORIAL_ISLAND` for any other.
    pub fn for_location(tile: Tile) -> FishSpots {
        static MAP: OnceLock<HashMap<Tile, FishSpots>> = OnceLock::new();
        MAP.get_or_init(|| {
            let mut map = HashMap::new();
            for value in Self::VALUES {
                for tile in value.locations() {
                    map.insert(*tile, value);
                }
            }
            map
        })
        .get(&tile)
        .copied()
        .unwrap_or(FishSpots::TutorialIsland)
    }
}

/// Whether `FishingNPC` is the behaviour of this NPC id.
pub fn is_spot(id: i32) -> bool {
    static IDS: OnceLock<Vec<i32>> = OnceLock::new();
    IDS.get_or_init(|| {
        let mut ids = FishingSpot::all_ids();
        ids.sort_unstable();
        ids.dedup();
        ids
    })
    .binary_search(&id)
    .is_ok()
}

/// What the behaviour asks of the world.
pub trait Host {
    type Npc;
    type Error;
    /// `getAttribute(self, "fishing:spot", null)`.
    fn spot(&mut self, npc: &Self::Npc) -> Result<Option<FishSpots>, Self::Error>;
    /// `setAttribute(self, "fishing:spot", spot)`.
    fn set_spot(&mut self, npc: &Self::Npc, spot: FishSpots) -> Result<(), Self::Error>;
    /// `getAttribute(self, "fishing:switchdelay", 0)`.
    fn switch_delay(&mut self, npc: &Self::Npc) -> Result<i32, Self::Error>;
    /// `setAttribute(self, "fishing:switchdelay", value)`.
    fn set_switch_delay(&mut self, npc: &Self::Npc, value: i32) -> Result<(), Self::Error>;
    /// `self.location`.
    fn location(&mut self, npc: &Self::Npc) -> Result<Tile, Self::Error>;
    /// `getWorldTicks()`.
    fn world_ticks(&mut self) -> Result<i32, Self::Error>;
    /// `RandomFunction.random(max)`.
    fn random(&mut self, max: i32) -> Result<i32, Self::Error>;
    /// `RandomFunction.random(a, b)`.
    fn random_range(&mut self, a: i32, b: i32) -> Result<i32, Self::Error>;
    /// `findLocalNPCs(tile, 0).isEmpty()`.
    fn no_npc_at(&mut self, tile: Tile) -> Result<bool, Self::Error>;
    /// `teleport(self, tile)` (instant: `properties.teleportLocation = tile`).
    fn teleport(&mut self, npc: &Self::Npc, tile: Tile) -> Result<(), Self::Error>;
}

/// `FishingNPC.onCreation`.
pub fn on_creation<H: Host>(h: &mut H, npc: &H::Npc) -> Result<(), H::Error> {
    let at = h.location(npc)?;
    h.set_spot(npc, FishSpots::for_location(at))?;
    h.set_switch_delay(npc, 0)
}

/// `FishingNPC.tick`: always `false`.
pub fn tick<H: Host>(h: &mut H, npc: &H::Npc) -> Result<bool, H::Error> {
    let Some(spot) = h.spot(npc)? else {
        return Ok(false);
    };
    if h.switch_delay(npc)? < h.world_ticks()? {
        move_spot(h, npc, spot)?;
    }
    Ok(false)
}

/// `FishingNPC.moveSpot` for a non-null area.
fn move_spot<H: Host>(h: &mut H, npc: &H::Npc, spot: FishSpots) -> Result<(), H::Error> {
    if spot == FishSpots::TutorialIsland {
        return Ok(());
    }
    let locations = spot.locations();
    let tile = locations[h.random(locations.len() as i32)? as usize];
    if h.no_npc_at(tile)? {
        h.teleport(npc, tile)?;
    }
    // getWorldTicks() + getRandomDelay().
    let delay = h.world_ticks()?.wrapping_add(h.random_range(200, 390)?);
    h.set_switch_delay(npc, delay)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct World {
        ticks: i32,
        spot: Option<FishSpots>,
        delay: i32,
        at: Tile,
        occupied: Vec<Tile>,
        draws: Vec<i32>,
        asked: Vec<(i32, i32)>,
        teleports: Vec<Tile>,
    }
    impl Host for World {
        type Npc = ();
        type Error = ();
        fn spot(&mut self, _: &()) -> Result<Option<FishSpots>, ()> {
            Ok(self.spot)
        }
        fn set_spot(&mut self, _: &(), spot: FishSpots) -> Result<(), ()> {
            self.spot = Some(spot);
            Ok(())
        }
        fn switch_delay(&mut self, _: &()) -> Result<i32, ()> {
            Ok(self.delay)
        }
        fn set_switch_delay(&mut self, _: &(), value: i32) -> Result<(), ()> {
            self.delay = value;
            Ok(())
        }
        fn location(&mut self, _: &()) -> Result<Tile, ()> {
            Ok(self.at)
        }
        fn world_ticks(&mut self) -> Result<i32, ()> {
            Ok(self.ticks)
        }
        fn random(&mut self, max: i32) -> Result<i32, ()> {
            self.asked.push((0, max));
            Ok(self.draws.remove(0))
        }
        fn random_range(&mut self, a: i32, b: i32) -> Result<i32, ()> {
            self.asked.push((a, b));
            Ok(self.draws.remove(0))
        }
        fn no_npc_at(&mut self, tile: Tile) -> Result<bool, ()> {
            Ok(!self.occupied.contains(&tile))
        }
        fn teleport(&mut self, _: &(), tile: Tile) -> Result<(), ()> {
            self.teleports.push(tile);
            Ok(())
        }
    }

    #[test]
    fn table_matches_the_source() {
        assert_eq!(FishSpots::VALUES.len(), 18);
        let total: usize = FishSpots::VALUES.iter().map(|v| v.locations().len()).sum();
        assert_eq!(total, 160);
        assert_eq!(
            FishSpots::Draynor.locations(),
            &[[3085, 3230, 0], [3086, 3227, 0]]
        );
        assert_eq!(FishSpots::for_location([3086, 3227, 0]), FishSpots::Draynor);
        // Two Castle Wars tiles are listed again under SHILO, which is
        // declared later and so owns them.
        assert_eq!(FishSpots::for_location([2461, 3151, 0]), FishSpots::Shilo);
        // Any other tile, another plane included, is TUTORIAL_ISLAND.
        assert_eq!(
            FishSpots::for_location([3086, 3227, 1]),
            FishSpots::TutorialIsland
        );
        assert_eq!(
            FishSpots::for_location([1, 1, 0]),
            FishSpots::TutorialIsland
        );
    }

    #[test]
    fn ids_are_the_fishing_spots() {
        assert!(is_spot(316));
        assert!(is_spot(309));
        assert!(!is_spot(1));
    }

    #[test]
    fn creation_then_first_tick_moves_to_a_free_tile() {
        let mut w = World {
            ticks: 5,
            at: [3104, 3424, 0],
            draws: vec![1, 250],
            ..World::default()
        };
        on_creation(&mut w, &()).unwrap();
        assert_eq!((w.spot, w.delay), (Some(FishSpots::Barbarian), 0));
        assert_eq!(tick(&mut w, &()), Ok(false));
        assert_eq!(w.asked, vec![(0, 5), (200, 390)]);
        assert_eq!(w.teleports, vec![[3110, 3433, 0]]);
        assert_eq!(w.delay, 255);
        // Not due again until the delay is below the tick count.
        w.ticks = 255;
        assert_eq!(tick(&mut w, &()), Ok(false));
        assert_eq!(w.asked.len(), 2);
    }

    #[test]
    fn an_occupied_tile_keeps_the_spot_but_resets_the_delay() {
        let mut w = World {
            ticks: 9,
            spot: Some(FishSpots::Draynor),
            occupied: vec![[3085, 3230, 0]],
            draws: vec![0, 200],
            ..World::default()
        };
        assert_eq!(tick(&mut w, &()), Ok(false));
        assert!(w.teleports.is_empty());
        assert_eq!(w.delay, 209);
    }

    #[test]
    fn tutorial_island_never_draws_or_resets() {
        let mut w = World {
            ticks: 9,
            spot: Some(FishSpots::TutorialIsland),
            ..World::default()
        };
        assert_eq!(tick(&mut w, &()), Ok(false));
        assert!(w.asked.is_empty());
        assert_eq!(w.delay, 0);
    }

    #[test]
    fn no_area_attribute_does_nothing() {
        let mut w = World {
            ticks: 9,
            ..World::default()
        };
        assert_eq!(tick(&mut w, &()), Ok(false));
        assert!(w.asked.is_empty());
    }
}
