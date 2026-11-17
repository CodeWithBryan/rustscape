//! What the original builds of the title screen's scene: the 104 by 104
//! tiles `LoginManager.loadRegion` puts at `Camera.originX/Y`, cut out of the
//! regions `LoginManager.setupLoadingScreenRegion` asks for
//! (`title_scene::select`), and nothing else.
//!
//! - `setupLoadingScreenRegion` (LoginManager.java:603-646) gives a region
//!   without an NPC file the group id -1 for all four map files, `m`, `l`,
//!   `um` and `ul` alike, and fills the unused slots of its 18 with -1 too.
//!   `rebuildMap` (LoginManager.java:890-921) then fetches nothing for them:
//!   such a region has no terrain, no locations and no underwater files
//!   ([`unread`]). With the GL renderer the `um`/`ul` groups of the listed
//!   regions are fetched with the zero key, like the `m` group.
//! - `readStaticTerrain` (LoginManager.java:1501-1536) reads each region's
//!   tiles into the 104-tile scene; `readTile` (SceneGraph.java:464-479)
//!   skips every tile outside it. A region without terrain is cleared only
//!   while `centralZoneY < 800` (`clearTerrainRegion`, SceneGraph.java:
//!   5070-5119, [`Window::clear_heights`]); otherwise its heights keep the
//!   zero `SceneGraph.init` allocated them with (SceneGraph.java:654).
//!   Corner row and column 104 of the 105 by 105 height grid are never
//!   written, so they stay zero ([`Window::corner_height`]).
//! - `readLocs` (SceneGraph.java:2854-2856) adds a location only when its
//!   tile is 1 to 102 on both axes, for the surface and underwater alike
//!   ([`Window::keeps_location`]).
//!
//! Whatever lies outside the 104 tiles is not part of the scene: the frame
//! shows the cleared background there (black in a cave, the fog colour
//! outside).
use crate::{
    atmosphere::Environment,
    terrain::{Region, Tile},
    underwater::Source,
};

/// Tiles on each side of the original's scene (`SceneGraph.width/length`).
pub const SIZE: i32 = 104;

/// The title scene as the original builds it.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Window {
    /// `Camera.originX/Y`: the absolute tile of scene tile (0, 0).
    pub base: [i32; 2],
    /// `SceneGraph.centralZoneX/Y`.
    pub zone: [i32; 2],
    /// The regions of the scene without terrain, as `[x, y]` in
    /// `regionBitPacked` order (x outer, y inner).
    pub unread: Vec<[i32; 2]>,
}

/// A region of the title scene whose files are not read: no tiles, every
/// height zero, and no underwater files.
pub fn unread(x: i32, z: i32) -> Region {
    Region {
        x,
        z,
        tiles: vec![Tile::default(); 4 * 64 * 64],
        environment: Environment::default(),
        point_lights: Vec::new(),
        chunk_environments: Vec::new(),
        occlusion: Vec::new(),
        underwater: Source::Absent,
    }
}

impl Window {
    /// Scene-local tile of an absolute one.
    pub fn local(&self, [x, z]: [i32; 2]) -> [i32; 2] {
        [x.wrapping_sub(self.base[0]), z.wrapping_sub(self.base[1])]
    }

    /// `readStaticTerrain`'s condition for clearing a region without
    /// terrain: the scene is on the surface map (`centralZoneY < 800`).
    pub fn clears(&self) -> bool {
        self.zone[1] < 800
    }

    /// `readLocs`: the location at absolute tile `(x, z)` is added.
    pub fn keeps_location(&self, x: i32, z: i32) -> bool {
        let [lx, lz] = self.local([x, z]);
        lx > 0 && lz > 0 && lx < SIZE - 1 && lz < SIZE - 1
    }

    /// `buildScene`'s floor loop (SceneGraph.java:949-951): the floors, and
    /// with them the upper levels' floor shadows (`addFloorShadow`, 993 and
    /// 1075), of tiles 1 to 102 on both axes.
    pub fn builds_floor(&self, x: i32, z: i32) -> bool {
        self.keeps_location(x, z)
    }

    /// `addLoc` hands a centrepiece (shapes 10 and 11) to `addEntity` with
    /// its footprint (SceneGraph.java:2509-2521), and `addSceneryEntity`
    /// turns it away when a tile of the footprint lies outside the 104 tiles
    /// (1331-1335). `size` is the footprint after the quarter turn; every
    /// other shape is added on its own tile or not through `addEntity`. The
    /// shadow `getStaticEntity` cast before (2511-2514) stays.
    pub fn keeps_scenery(&self, shape: i32, tile: [i32; 2], size: [i32; 2]) -> bool {
        if !matches!(shape, 10 | 11) {
            return true;
        }
        let [x, z] = self.local(tile);
        x >= 0 && z >= 0 && x + size[0] <= SIZE && z + size[1] <= SIZE
    }

    /// `tileHeights[level][x][y]` for scene-local corner `(x, y)` of the
    /// 105 by 105 grid, given the height the regions hold there.
    pub fn corner_height(&self, [x, y]: [usize; 2], height: i32) -> i32 {
        if x as i32 >= SIZE || y as i32 >= SIZE {
            0
        } else {
            height
        }
    }

    /// The height half of `clearTerrainRegion(level, ..)` for every level of
    /// every region in [`Window::unread`], in the original's order, when the
    /// scene clears ([`Window::clears`]). `regions` must hold every tile of
    /// the scene. The shade half is `tile_shade::ShadowMap::clear_region`.
    pub fn clear_heights(&self, regions: &mut [Region]) {
        if !self.clears() {
            return;
        }
        let mut grid = Grid::read(self.base, regions);
        for &[rx, rz] in &self.unread {
            let origin = self.local([rx * 64, rz * 64]);
            for level in 0..4 {
                grid.clear(level, origin);
            }
        }
        grid.write(self.base, regions);
    }
}

/// `tileHeights` over the scene's 104 by 104 tiles, `[level][x * 104 + y]`.
struct Grid {
    heights: [Vec<i32>; 4],
}

impl Grid {
    fn read(base: [i32; 2], regions: &[Region]) -> Self {
        let mut heights: [Vec<i32>; 4] = std::array::from_fn(|_| vec![0; (SIZE * SIZE) as usize]);
        for (level, grid) in heights.iter_mut().enumerate() {
            for x in 0..SIZE {
                for y in 0..SIZE {
                    if let Some(tile) = find(regions, base[0] + x, base[1] + y)
                        .map(|(region, i)| regions[region].tiles[level * 4096 + i])
                    {
                        grid[(x * SIZE + y) as usize] = tile.height;
                    }
                }
            }
        }
        Self { heights }
    }

    fn write(&self, base: [i32; 2], regions: &mut [Region]) {
        for (level, grid) in self.heights.iter().enumerate() {
            for x in 0..SIZE {
                for y in 0..SIZE {
                    if let Some((region, i)) = find(regions, base[0] + x, base[1] + y) {
                        regions[region].tiles[level * 4096 + i].height =
                            grid[(x * SIZE + y) as usize];
                    }
                }
            }
        }
    }

    fn at(&self, level: usize, x: i32, y: i32) -> i32 {
        self.heights[level][(x * SIZE + y) as usize]
    }

    fn put(&mut self, level: usize, x: i32, y: i32, value: i32) {
        self.heights[level][(x * SIZE + y) as usize] = value;
    }

    /// SceneGraph.java:5080-5118 for a 64 by 64 region at scene tile
    /// `(x, y)`. The original's parameters are named the other way round
    /// (`clearTerrainRegion(level, baseY, baseX, 64, 64)` from
    /// LoginManager.java:1532 into `(level, x, y, ..)`), and its arrays are
    /// indexed `[level][y-parameter][x-parameter]`; here `x` is always the
    /// first index.
    fn clear(&mut self, level: usize, [x, y]: [i32; 2]) {
        let inside = |n: i32| (0..SIZE).contains(&n);
        for tx in x..x + 64 {
            for ty in y..y + 64 {
                if inside(tx) && inside(ty) {
                    let value = if level == 0 {
                        0
                    } else {
                        self.at(level - 1, tx, ty)
                    };
                    self.put(level, tx, ty, value);
                }
            }
        }
        // The region's west column continues its west neighbour's heights,
        // its south row its south neighbour's, both without the corner.
        if x > 0 && x < SIZE {
            for ty in y + 1..y + 64 {
                if inside(ty) {
                    let value = self.at(level, x - 1, ty);
                    self.put(level, x, ty, value);
                }
            }
        }
        if y > 0 && y < SIZE {
            for tx in x + 1..x + 64 {
                if inside(tx) {
                    let value = self.at(level, tx, y - 1);
                    self.put(level, tx, y, value);
                }
            }
        }
        if !inside(x) || !inside(y) {
            return;
        }
        if level == 0 {
            if x > 0 && self.at(0, x - 1, y) != 0 {
                let value = self.at(0, x - 1, y);
                self.put(0, x, y, value);
            } else if y > 0 && self.at(0, x, y - 1) != 0 {
                let value = self.at(0, x, y - 1);
                self.put(0, x, y, value);
            } else if x > 0 && y > 0 && self.at(0, x - 1, y - 1) != 0 {
                let value = self.at(0, x - 1, y - 1);
                self.put(0, x, y, value);
            }
        } else if x > 0 && self.at(level - 1, x - 1, y) != self.at(level, x - 1, y) {
            let value = self.at(level, x - 1, y);
            self.put(level, x, y, value);
        } else if y > 0 && self.at(level, x, y - 1) != self.at(level - 1, x, y - 1) {
            let value = self.at(level, x, y - 1);
            self.put(level, x, y, value);
        } else if x > 0 && y > 0 && self.at(level, x - 1, y - 1) != self.at(level - 1, x - 1, y - 1)
        {
            let value = self.at(level, x - 1, y - 1);
            self.put(level, x, y, value);
        }
    }
}

/// The region holding absolute tile `(x, z)` and the tile's index within one
/// level of it.
fn find(regions: &[Region], x: i32, z: i32) -> Option<(usize, usize)> {
    let index = regions
        .iter()
        .position(|r| r.x == x.div_euclid(64) && r.z == z.div_euclid(64))?;
    Some((index, (x.rem_euclid(64) * 64 + z.rem_euclid(64)) as usize))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn region(x: i32, z: i32, height: impl Fn(usize, i32, i32) -> i32) -> Region {
        let mut region = unread(x, z);
        for level in 0..4 {
            for lx in 0..64 {
                for lz in 0..64 {
                    region.tiles[level * 4096 + (lx * 64 + lz) as usize].height =
                        height(level, lx, lz);
                }
            }
        }
        region
    }

    fn height(regions: &[Region], level: usize, x: i32, z: i32) -> i32 {
        let (r, i) = find(regions, x, z).unwrap();
        regions[r].tiles[level * 4096 + i].height
    }

    /// Level 0 of the clear is the same `clearTerrainRegion` the underwater
    /// scene runs (`underwater::Scene::clear`), which the terrain oracle's
    /// JVM differential reproduces; the two must agree on any heights.
    #[test]
    fn level_0_clears_as_the_underwater_scene_does() {
        let mut seed = 12345u32;
        let mut next = || {
            seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
            seed >> 16
        };
        for origin in [
            [-40, -40],
            [24, -40],
            [-40, 24],
            [24, 24],
            [0, 0],
            [88, 40],
            [103, 103],
            [104, 5],
        ] {
            let mut grid = Grid {
                heights: std::array::from_fn(|_| vec![0; (SIZE * SIZE) as usize]),
            };
            let size = SIZE as usize;
            let mut scene = crate::underwater::Scene::new(size, size, vec![0; size * size]);
            for x in 0..SIZE {
                for y in 0..SIZE {
                    // A third of the corners are zero.
                    let value = if next() % 3 == 0 {
                        0
                    } else {
                        -((next() % 400) as i32)
                    };
                    grid.put(0, x, y, value);
                    scene.heights[(x * (SIZE + 1) + y) as usize] = value;
                }
            }
            grid.clear(0, origin);
            scene.clear(origin);
            for x in 0..SIZE {
                for y in 0..SIZE {
                    assert_eq!(
                        grid.at(0, x, y),
                        scene.heights[(x * (SIZE + 1) + y) as usize],
                        "{origin:?} at {x},{y}"
                    );
                }
            }
        }
    }

    /// Transcription test: readLocs' bounds (SceneGraph.java:2856) and the
    /// zero corners of `tileHeights` (SceneGraph.java:464, 654).
    #[test]
    fn the_scene_keeps_locations_on_tiles_1_to_102() {
        let window = Window {
            base: [2976, 9904],
            zone: [378, 1244],
            unread: vec![],
        };
        assert!(window.keeps_location(2977, 9905));
        assert!(window.keeps_location(2976 + 102, 9904 + 102));
        assert!(!window.keeps_location(2976, 9950));
        assert!(!window.keeps_location(2976 + 103, 9950));
        assert!(!window.keeps_location(3000, 9904 + 103));
        assert!(!window.keeps_location(3136, 9950));
        assert_eq!(window.corner_height([103, 103], -80), -80);
        assert_eq!(window.corner_height([104, 3], -80), 0);
        assert_eq!(window.corner_height([3, 104], -80), 0);
        // Zone 1244 is under the surface map: nothing is cleared.
        assert!(!window.clears());
    }

    /// Transcription test: `addSceneryEntity`'s bounds (SceneGraph.java:
    /// 1331-1335) for the footprint `addLoc` gives a centrepiece
    /// (2509-2521), and `buildScene`'s floor loop (949-951).
    #[test]
    fn scenery_past_tile_103_is_turned_away() {
        let window = Window {
            base: [2976, 9904],
            zone: [378, 1244],
            unread: vec![],
        };
        let at = |x: i32, z: i32| [2976 + x, 9904 + z];
        // A 1 by 1 block on tile 102 fits; 2 by 2 on 102 reaches tile 103.
        assert!(window.keeps_scenery(10, at(102, 50), [1, 1]));
        assert!(window.keeps_scenery(10, at(102, 50), [2, 2]));
        // Three tiles from 102 reach tile 104, in either direction.
        assert!(!window.keeps_scenery(10, at(102, 50), [3, 1]));
        assert!(!window.keeps_scenery(11, at(50, 102), [1, 3]));
        assert!(window.keeps_scenery(10, at(101, 101), [3, 3]));
        // Walls, decorations, diagonal walls and roofs are not checked.
        for shape in [0, 1, 2, 3, 4, 9, 12, 21, 22] {
            assert!(window.keeps_scenery(shape, at(102, 102), [3, 3]));
        }
        assert!(window.builds_floor(2977, 9905));
        assert!(window.builds_floor(2976 + 102, 9904 + 102));
        assert!(!window.builds_floor(2976 + 103, 9950));
        assert!(!window.builds_floor(2976, 9950));
    }

    /// Transcription test: an unread region is the original's untouched
    /// arrays: no tiles, zero heights, and no underwater files.
    #[test]
    fn an_unread_region_has_nothing() {
        let region = unread(59, 44);
        assert!(
            region
                .tiles
                .iter()
                .all(|t| t.height == 0 && t.underlay == 0 && t.overlay == 0 && t.flags == 0)
        );
        assert!(matches!(region.underwater, Source::Absent));
        assert!(region.point_lights.is_empty() && region.occlusion.is_empty());
    }

    /// Transcription test: SceneGraph.clearTerrainRegion's height loops on
    /// level 0 and level 1, worked by hand. The scene starts 8 tiles into
    /// region (58, 43); region (59, 44) has terrain, (59, 43) and (58, 44) do
    /// not.
    #[test]
    fn a_cleared_region_continues_its_neighbours() {
        let base = [58 * 64 + 40, 43 * 64 + 40];
        let window = Window {
            base,
            zone: [475, 355],
            unread: vec![[58, 43], [58, 44], [59, 43]],
        };
        // Every listed tile: level 0 at -100 - x, level 1 240 above it.
        let read = |level: usize, x: i32, _z: i32| -100 - x - 240 * level as i32;
        let mut regions = vec![
            unread(58, 43),
            unread(58, 44),
            unread(59, 43),
            region(59, 44, read),
            // Outside the scene: untouched.
            region(60, 44, |_, _, _| -7),
        ];
        window.clear_heights(&mut regions);
        // (59, 43) lies at scene x 24..88, y -40..24: its south row is off
        // the scene, its west column (x 24) copies x 23 of (58, 43), cleared
        // to 0 before it.
        assert_eq!(height(&regions, 0, 59 * 64, 43 * 64 + 50), 0);
        // (58, 44) lies at x -40..24, y 24..88: its south row (y 24) copies
        // y 23, which (58, 43) cleared to 0; nothing of (59, 44) is changed.
        assert_eq!(height(&regions, 0, 58 * 64 + 63, 44 * 64 + 5), 0);
        assert_eq!(height(&regions, 0, 59 * 64, 44 * 64), -100);
        assert_eq!(height(&regions, 1, 59 * 64 + 3, 44 * 64), -343);
        assert_eq!(height(&regions, 0, 60 * 64, 44 * 64), -7);
        // A cleared region east of (59, 44) would continue it: its west
        // column copies (59, 44)'s east column, its corner tile too.
        let window = Window {
            base: [59 * 64 - 20, 44 * 64 - 20],
            zone: [475, 355],
            unread: vec![[60, 44]],
        };
        let mut regions = vec![
            unread(58, 43),
            unread(58, 44),
            unread(59, 43),
            unread(60, 43),
            region(59, 44, read),
            unread(60, 44),
        ];
        window.clear_heights(&mut regions);
        for z in 1..20 {
            assert_eq!(height(&regions, 0, 60 * 64, 44 * 64 + z), -163);
            assert_eq!(height(&regions, 1, 60 * 64, 44 * 64 + z), -403);
        }
        // Level 0's corner: the west neighbour is not zero, so it is copied.
        assert_eq!(height(&regions, 0, 60 * 64, 44 * 64), -163);
        // Level 1's corner: the west neighbour differs from the level below
        // (-403 against -163), so it is copied.
        assert_eq!(height(&regions, 1, 60 * 64, 44 * 64), -403);
        // Further in, the region is level 0 at 0 and level 1 on level 0.
        assert_eq!(height(&regions, 0, 60 * 64 + 1, 44 * 64 + 1), 0);
        assert_eq!(height(&regions, 1, 60 * 64 + 1, 44 * 64 + 1), 0);
        // On the surface map the same scene keeps its zero heights.
        let window = Window {
            zone: [475, 1244],
            ..window
        };
        let mut regions = vec![region(59, 44, read), unread(60, 44)];
        window.clear_heights(&mut regions);
        assert_eq!(height(&regions, 0, 60 * 64, 44 * 64 + 5), 0);
    }
}
