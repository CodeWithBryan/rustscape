//! The title screen's 3D scene as the hardware renderer shows it: which map
//! regions `LoginManager.setupLoadingScreenRegion` asks for, where
//! `LoginManager.loadRegion(.., fromLogin = true, ..)` puts the scene, the
//! NPCs `LoginManager.decodeNpcFiles` places from a region's `n` file, and
//! the wandering `client.mainUpdate` gives them without a server.
//!
//! Everything here is arithmetic over plain values; the scene itself, the
//! camera and the NPC bodies are the application's. Tiles are scene-local
//! (relative to `Camera.originX/Y`) unless a name says otherwise.
//!
//! The original keeps this state in game state 10. It enters game state 28
//! whenever the scene has to be built (`InterfaceList.resetToLoginScreen`,
//! the camera leaving the middle of the scene in `client.mainUpdate`, a
//! changed display setting) and leaves it for 10 at the end of
//! `LoginManager.rebuildMap`.
use rs09_game_core::collision::CollisionMap;

/// `LoginManager.setupLoadingScreenRegion`'s choice (LoginManager.java:
/// 603-646).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Selection {
    /// The zone (8 by 8 tiles) the camera is over: the new scene's centre.
    pub zone: [i32; 2],
    /// `regionBitPacked` in the original's order (x outer, y inner), each
    /// with whether its map files are asked for: only a region that has an
    /// NPC file (`n{x}_{y}`) is, and the others stay empty.
    pub regions: Vec<(i32, bool)>,
}
impl Selection {
    /// The regions whose files are loaded, as `[x, y]`.
    pub fn loaded(&self) -> impl Iterator<Item = [i32; 2]> + '_ {
        self.regions
            .iter()
            .filter(|(_, listed)| *listed)
            .map(|(packed, _)| [packed >> 8, packed & 0xff])
    }
}

/// `render` is `Camera.renderX/Y` (scene-local fine units) and `origin`
/// `Camera.originX/Y`. `has_spawns(x, y)` is whether the map archive has a
/// group named `n{x}_{y}`.
pub fn select(
    render: [i32; 2],
    origin: [i32; 2],
    mut has_spawns: impl FnMut(i32, i32) -> bool,
) -> Selection {
    let zone = [0, 1].map(|axis| (render[axis] >> 10).wrapping_add(origin[axis] >> 3));
    let mut regions = Vec::new();
    // Java's `/` truncates toward zero, as Rust's does.
    let range = |zone: i32| zone.wrapping_sub(6) / 8..=zone.wrapping_add(6) / 8;
    for x in range(zone[0]) {
        for y in range(zone[1]) {
            regions.push(((x << 8).wrapping_add(y), has_spawns(x, y)));
        }
    }
    Selection { zone, regions }
}

/// What a `loadRegion` call that did not return early changed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Loaded {
    /// The new `Camera.originX/Y`.
    pub origin: [i32; 2],
    /// New origin less the old one: the camera, its locked targets, the
    /// NPCs and everything else scene-local moves back by this.
    pub delta: [i32; 2],
    /// `FogManager.setInstantFade()`: the scene moved by more than its own
    /// width.
    pub instant_fade: bool,
}

/// `SceneGraph.centralZoneX/Y` and `Camera.originX/Y` while logged out.
/// `LoginManager.processLogout` zeroes all four, which is also their value
/// when the client starts.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Central {
    pub zone: [i32; 2],
    pub origin: [i32; 2],
}
impl Central {
    /// `LoginManager.loadRegion(0, zoneY, zoneX, 8, true, 8)`
    /// (LoginManager.java:698-827). None when the scene is already centred
    /// on `zone`: the original then returns before anything else (with the
    /// hardware renderer every level is visible, so the plane is not
    /// compared). Otherwise the game state becomes 28.
    pub fn load(&mut self, zone: [i32; 2]) -> Option<Loaded> {
        if self.zone == zone {
            return None;
        }
        self.zone = zone;
        let origin = [
            zone[0].wrapping_sub(6).wrapping_mul(8),
            zone[1].wrapping_mul(8).wrapping_sub(48),
        ];
        let delta = [0, 1].map(|axis| origin[axis].wrapping_sub(self.origin[axis]));
        self.origin = origin;
        Some(Loaded {
            origin,
            delta,
            instant_fade: delta.into_iter().any(|n| n.wrapping_abs() > 104),
        })
    }

    /// The local player's tile after the load, absolute:
    /// `PlayerList.self.teleport(8, false, 8)`. The sun's direction is taken
    /// from the map chunk this tile is in (`FogManager.setLightPosition` in
    /// `LoginManager.rebuildMap`).
    pub fn self_tile(&self) -> [i32; 2] {
        self.origin.map(|n| n.wrapping_add(8))
    }

    /// The middle of the 104 by 104 scene, absolute. Its region and the
    /// eight around it hold every region of [`Selection::regions`].
    pub fn middle(&self) -> [i32; 2] {
        self.zone.map(|n| n.wrapping_mul(8).wrapping_add(4))
    }
}

/// `loadRegion` with `fromLogin` keeps an NPC only while it stands inside
/// the new scene: both fine coordinates, already moved by the delta, from 0
/// to 13184 inclusive.
pub fn npc_kept(local_fine: [i32; 2]) -> bool {
    local_fine.into_iter().all(|n| (0..=13184).contains(&n))
}

/// `client.mainUpdate` (client.java:1208): the camera has left tiles 14 to
/// 89 of the scene on either axis, so the scene is set up again around it.
pub fn camera_outside(render: [i32; 2]) -> bool {
    render.into_iter().any(|n| !(14..90).contains(&(n >> 7)))
}

/// `LoginManager.decodedNpcRegions`: every region whose NPC file has been
/// decoded gets a slot for the life of the client, so that its NPCs keep
/// their indices from one title scene to the next.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Slots {
    regions: Vec<i32>,
}
impl Slots {
    /// The slot of a packed region, assigned on first use. The original's
    /// table has 64 entries and overruns on the 65th region; None then.
    pub fn slot(&mut self, packed: i32) -> Option<usize> {
        if let Some(slot) = self.regions.iter().position(|&region| region == packed) {
            return Some(slot);
        }
        if self.regions.len() == 64 {
            return None;
        }
        self.regions.push(packed);
        Some(self.regions.len() - 1)
    }
    pub fn len(&self) -> usize {
        self.regions.len()
    }
    pub fn is_empty(&self) -> bool {
        self.regions.is_empty()
    }
}

/// One record of an `n` file as `LoginManager.decodeNpcFiles` reads it
/// (LoginManager.java:1331-1339).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Spawn {
    /// `NpcList.npcs` index: the record's number in the file, shifted over
    /// the region's slot.
    pub index: usize,
    pub plane: i32,
    /// Scene-local tile.
    pub tile: [i32; 2],
    pub npc: i32,
}

/// The records of a region's NPC file: two bytes of packed position (plane,
/// then x and y within the region) and two of NPC type each, at most 511.
/// A trailing partial record is left out (the original would overrun its
/// buffer on one).
pub fn spawns(data: &[u8], packed_region: i32, slot: usize, origin: [i32; 2]) -> Vec<Spawn> {
    data.chunks_exact(4)
        .take(511)
        .enumerate()
        .map(|(number, record)| {
            let position = i32::from(u16::from_be_bytes([record[0], record[1]]));
            Spawn {
                index: number << 6 | slot,
                plane: position >> 14,
                tile: [
                    (position >> 7 & 0x3f) + (packed_region >> 8) * 64 - origin[0],
                    (position & 0x3f) + (packed_region & 0xff) * 64 - origin[1],
                ],
                npc: i32::from(u16::from_be_bytes([record[2], record[3]])),
            }
        })
        .collect()
}

/// The rest of `decodeNpcFiles`' test for a record whose index is free: the
/// type is one shown on the login screen (`loginscreenproperties & 1`), the
/// record is on the scene's plane (always 0 with the hardware renderer) and
/// the NPC fits inside the scene.
pub fn spawn_placed(spawn: &Spawn, properties: i32, size: i32) -> bool {
    properties & 1 > 0
        && spawn.plane == 0
        && spawn
            .tile
            .into_iter()
            .all(|n| n >= 0 && size.wrapping_add(n) < 104)
}

/// `PathingEntity.ANGLES[spawndirection]`, the angle a placed NPC faces.
/// None for a direction outside the table (the original overruns it).
pub fn spawn_angle(direction: i32) -> Option<i32> {
    usize::try_from(direction)
        .ok()
        .and_then(|direction| [768, 1024, 1280, 512, 1536, 256, 0, 1792].get(direction))
        .copied()
}

/// What one NPC's wander test did (client.java:1171-1192).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Wander {
    /// `movementQueueX/Y[0]` was overwritten with this tile. The original
    /// does so before it tests the step, so the queue's head moves even when
    /// the NPC then stays where it is.
    pub head: Option<[i32; 2]>,
    /// `movementQueueSize = 1`: the NPC walks one step to `head`.
    pub walk: bool,
    /// `continue nextNpc`: an NPC larger than one tile found its
    /// destination occupied, and the original then skips the rest of this
    /// cycle for it (movement, facing, animation, and marking its tiles in
    /// the collision map again).
    pub skip: bool,
}

/// The wander test of one title NPC for one cycle.
///
/// - `fine` is the NPC's `xFine/yFine`; `size` its footprint;
///   `properties` its type's `loginscreenproperties`; `idle` is
///   `movementQueueSize == 0`.
/// - `map` is the collision map of the scene's plane. Every title NPC marks
///   its own tiles in it at the end of each cycle ([`mark`]), and unmarks
///   them here before it tests a step.
/// - `random` is `Math.random()`.
///
/// Quirks kept: the NPC's tile is taken as `fine >> 7`, which for an NPC two
/// or three tiles wide is one tile north-east of its south-west corner; and
/// the step test is handed the destination with its coordinates exchanged
/// (`isPathClear(yFine >> 7, queueY[0], queueX[0], xFine >> 7)` against the
/// parameters `(srcY, destX, destY, srcX)`), so what it walks is the line
/// from the NPC's tile to the mirrored destination.
pub fn wander(
    map: &mut CollisionMap,
    fine: [i32; 2],
    size: i32,
    properties: i32,
    idle: bool,
    mut random: impl FnMut() -> f64,
) -> Wander {
    let mut out = Wander::default();
    if !(properties & 2 > 0 && idle && random() * 1000.0 < 10.0) {
        return out;
    }
    // (int) Math.round(Math.random() * 2.0 - 1.0): floor(x + 0.5).
    let mut step = || (random() * 2.0 - 1.0 + 0.5).floor() as i32;
    let dx = step();
    let dy = step();
    if dx == 0 && dy == 0 {
        return out;
    }
    let tile = fine.map(|n| n >> 7);
    let head = [dx.wrapping_add(tile[0]), dy.wrapping_add(tile[1])];
    out.head = Some(head);
    map.scenery((tile[0], tile[1]), (size, size), false, false);
    let limit = 104i32.wrapping_sub(size);
    if head.into_iter().all(|n| (0..=limit).contains(&n))
        && map.is_path_clear((tile[0], tile[1]), (head[1], head[0]))
    {
        if size > 1 {
            for x in head[0]..head[0].wrapping_add(size) {
                for y in head[1]..head[1].wrapping_add(size) {
                    if map.get(x, y).is_none_or(|flags| flags & 0x12401ff != 0) {
                        out.skip = true;
                        return out;
                    }
                }
            }
        }
        out.walk = true;
    }
    out
}

/// The end of a title NPC's cycle: `flagScenery(xFine >> 7, false,
/// yFine >> 7, size, size)`.
pub fn mark(map: &mut CollisionMap, fine: [i32; 2], size: i32) {
    map.scenery((fine[0] >> 7, fine[1] >> 7), (size, size), false, true);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Transcription test: LoginManager.java:604-646 and 698-717 by hand for
    /// the first camera position of title script 1240, tile (2498, 2855).
    #[test]
    fn the_first_scene_is_the_zone_under_the_locked_camera() {
        // resetToLoginScreen: renderX = lockedTargetX << 7 with the origin 0.
        let render = [2498 << 7, 2855 << 7];
        let selection = select(render, [0, 0], |x, y| [x, y] == [39, 44]);
        assert_eq!(selection.zone, [312, 356]);
        // (312 - 6) / 8 = 38 to (312 + 6) / 8 = 39; 43 to 45.
        assert_eq!(
            selection.regions,
            [
                (38 << 8 | 43, false),
                (38 << 8 | 44, false),
                (38 << 8 | 45, false),
                (39 << 8 | 43, false),
                (39 << 8 | 44, true),
                (39 << 8 | 45, false),
            ]
        );
        assert_eq!(selection.loaded().collect::<Vec<_>>(), [[39, 44]]);
        let mut central = Central::default();
        let loaded = central.load(selection.zone).unwrap();
        assert_eq!(loaded.origin, [2448, 2800]);
        assert_eq!(loaded.delta, [2448, 2800]);
        assert!(loaded.instant_fade);
        assert_eq!(central.self_tile(), [2456, 2808]);
        assert_eq!(central.middle(), [2500, 2852]);
        assert_eq!(central.load(selection.zone), None);
        // The camera is now at local tile (50, 55) and stays until it
        // reaches tile 90 or 13.
        let local = [render[0] - 2448 * 128, render[1] - 2800 * 128];
        assert!(!camera_outside(local));
        assert!(camera_outside([90 * 128, local[1]]));
        assert!(!camera_outside([90 * 128 - 1, 14 * 128]));
        assert!(camera_outside([50 * 128, 14 * 128 - 1]));
    }

    /// Transcription test: decodeNpcFiles' record layout and placement test.
    #[test]
    fn npc_records_are_placed_inside_the_scene_on_plane_0() {
        let mut slots = Slots::default();
        assert_eq!(slots.slot(10028), Some(0));
        assert_eq!(slots.slot(10284), Some(1));
        assert_eq!(slots.slot(10028), Some(0));
        // plane 1, x 3, y 5, type 258; then plane 0, x 63, y 0, type 7; and
        // a trailing odd byte.
        let data = [0x41, 0x85, 1, 2, 0x1f, 0x80, 0, 7, 9];
        let found = spawns(&data, 39 << 8 | 44, 1, [2448, 2800]);
        assert_eq!(
            found,
            [
                Spawn {
                    index: 1,
                    plane: 1,
                    tile: [3 + 48, 5 + 16],
                    npc: 258
                },
                Spawn {
                    index: 1 << 6 | 1,
                    plane: 0,
                    tile: [63 + 48, 16],
                    npc: 7
                },
            ]
        );
        assert!(!spawn_placed(&found[0], 1, 1));
        // x 111 is outside the scene's 104 tiles.
        assert!(!spawn_placed(&found[1], 1, 1));
        let inside = Spawn {
            tile: [102, 0],
            ..found[1]
        };
        assert!(spawn_placed(&inside, 3, 1));
        assert!(!spawn_placed(&inside, 2, 1));
        assert!(!spawn_placed(&inside, 1, 2));
        assert_eq!(spawn_angle(7), Some(1792));
        assert_eq!(spawn_angle(8), None);
    }
}
