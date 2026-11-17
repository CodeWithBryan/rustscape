//! Instanced ("dynamic") scenes: REBUILD_REGION copies 8x8 chunks from source
//! map files into a 104x104 scene, each with its own rotation and source level.
//!
//! Transcribed from RT4 `LoginManager.readDynamicTerrain`/`readDynamicLocs`,
//! `SceneGraph.readDynamicTerrain`, `readTile`, `clearTerrainRegion` and the
//! chunk rotation helpers. Coordinates here are scene-local tiles.
use crate::atmosphere::Environment;
use crate::terrain::{Region, Tile, tile_height};
use rs09_protocol::{Result, buffer::Reader};
use std::collections::BTreeMap;

pub const SIZE: usize = 104;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RawHeight {
    /// Opcode 0: procedural on level 0, 240 above the level below otherwise.
    Derived,
    /// Opcode 1 with its byte; 1 means 0.
    Explicit(i32),
}

/// One tile's commands from a map file, before heights are resolved against
/// the scene they are placed into.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RawTile {
    pub height: RawHeight,
    /// Overlay ID, shape and angle, when the tile sets them.
    pub overlay: Option<[u8; 3]>,
    pub underlay: Option<u8>,
    pub flags: u8,
}

/// The 4 x 64 x 64 tile stream of a map file, indexed `(level * 64 + x) * 64 + y`.
pub fn raw_tiles(data: &[u8]) -> Result<Vec<RawTile>> {
    let mut r = Reader::new(data);
    let mut tiles = Vec::with_capacity(4 * 64 * 64);
    for _ in 0..4 * 64 * 64 {
        let mut tile = RawTile {
            height: RawHeight::Derived,
            overlay: None,
            underlay: None,
            flags: 0,
        };
        loop {
            match r.g1()? {
                0 => break,
                1 => {
                    tile.height = RawHeight::Explicit(r.g1()?);
                    break;
                }
                op @ 2..=49 => {
                    tile.overlay = Some([r.g1()? as u8, ((op - 2) / 4) as u8, ((op - 2) & 3) as u8])
                }
                op @ 50..=81 => tile.flags = (op - 49) as u8,
                op => tile.underlay = Some((op - 81) as u8),
            }
        }
        tiles.push(tile);
    }
    Ok(tiles)
}

/// `SceneGraph.rotateX(rotation, y, x)` and `rotateY(rotation, x, y)` for a
/// tile inside its 8x8 chunk.
fn rotate(rotation: i32, x: i32, y: i32) -> [i32; 2] {
    match rotation & 3 {
        0 => [x, y],
        1 => [y, 7 - x],
        2 => [7 - x, 7 - y],
        _ => [7 - y, x],
    }
}

/// `LoginManager.rotateLocX`/`rotateLocY`: the south-west tile of a location
/// of `size` (width, length) after its chunk is rotated.
fn rotate_location(rotation: i32, loc_rotation: i32, size: [i32; 2], x: i32, y: i32) -> [i32; 2] {
    let [mut width, mut length] = size;
    if loc_rotation & 1 == 1 {
        std::mem::swap(&mut width, &mut length);
    }
    match rotation & 3 {
        0 => [x, y],
        1 => [y, 8 - x - width],
        2 => [8 - x - width, 8 - length - y],
        _ => [8 - y - length, x],
    }
}

/// The chunk fields of a packed REBUILD_REGION entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Chunk {
    pub level: usize,
    pub rotation: i32,
    /// Source chunk coordinates in 8-tile units.
    pub zone: [i32; 2],
}
impl Chunk {
    pub fn unpack(packed: i32) -> Option<Self> {
        (packed != -1).then_some(Self {
            level: (packed >> 24 & 3) as usize,
            rotation: packed >> 1 & 3,
            zone: [packed >> 14 & 0x3ff, packed >> 3 & 0x7ff],
        })
    }
    /// The source region's packed ID, as the rebuild lists them.
    pub fn region(self) -> i32 {
        ((self.zone[0] / 8) << 8) + self.zone[1] / 8
    }
}

/// A composed scene's tiles, indexed `(level * 104 + x) * 104 + y`.
pub struct Terrain {
    pub tiles: Vec<Tile>,
}
impl Terrain {
    fn index(level: usize, x: usize, y: usize) -> usize {
        (level * SIZE + x) * SIZE + y
    }
    pub fn tile(&self, level: usize, x: usize, y: usize) -> Tile {
        self.tiles[Self::index(level, x, y)]
    }
    fn height(&self, level: usize, x: i32, y: i32) -> i32 {
        self.tiles[Self::index(level, x as usize, y as usize)].height
    }
    fn set_height(&mut self, level: usize, x: i32, y: i32, height: i32) {
        self.tiles[Self::index(level, x as usize, y as usize)].height = height;
    }

    /// `clearTerrainRegion` for the 8x8 chunk whose south-west tile is (x, y):
    /// a missing chunk takes the heights around it so it does not open a pit.
    fn clear(&mut self, level: usize, x: i32, y: i32) {
        let inside = |n: i32| (0..SIZE as i32).contains(&n);
        for a in x..x + 8 {
            for b in y..y + 8 {
                if inside(a) && inside(b) {
                    let below = if level == 0 {
                        0
                    } else {
                        self.height(level - 1, a, b)
                    };
                    self.set_height(level, a, b, below);
                }
            }
        }
        if x > 0 && x < SIZE as i32 {
            for b in y + 1..y + 8 {
                if inside(b) {
                    let west = self.height(level, x - 1, b);
                    self.set_height(level, x, b, west);
                }
            }
        }
        if y > 0 && y < SIZE as i32 {
            for a in x + 1..x + 8 {
                if inside(a) {
                    let south = self.height(level, a, y - 1);
                    self.set_height(level, a, y, south);
                }
            }
        }
        if !inside(x) || !inside(y) {
            return;
        }
        // The corner takes the first neighbour that differs from flat ground
        // (level 0) or from the level below it (upper levels).
        let differs = |terrain: &Self, a: i32, b: i32| {
            let height = terrain.height(level, a, b);
            if level == 0 {
                height != 0
            } else {
                height != terrain.height(level - 1, a, b)
            }
        };
        let neighbour = if x > 0 && differs(self, x - 1, y) {
            Some([x - 1, y])
        } else if y > 0 && differs(self, x, y - 1) {
            Some([x, y - 1])
        } else if x > 0 && y > 0 && differs(self, x - 1, y - 1) {
            Some([x - 1, y - 1])
        } else {
            None
        };
        if let Some([a, b]) = neighbour {
            let height = self.height(level, a, b);
            self.set_height(level, x, y, height);
        }
    }

    /// `readTile` for one source tile placed at scene tile (x, y).
    fn place(&mut self, level: usize, x: i32, y: i32, rotation: i32, raw: RawTile) {
        if !(0..SIZE as i32).contains(&x) || !(0..SIZE as i32).contains(&y) {
            return;
        }
        let below = if level == 0 {
            0
        } else {
            self.height(level - 1, x, y)
        };
        let tile = &mut self.tiles[Self::index(level, x as usize, y as usize)];
        tile.flags = raw.flags;
        if let Some([id, shape, angle]) = raw.overlay {
            tile.overlay = id;
            tile.shape = shape;
            tile.rotation = ((i32::from(angle) + rotation) & 3) as u8;
        }
        if let Some(underlay) = raw.underlay {
            tile.underlay = underlay;
        }
        tile.height = match (raw.height, level) {
            // The noise is sampled at the scene tile, not the source tile.
            (RawHeight::Derived, 0) => -tile_height(y + 556_238, x + 932_731) * 8,
            (RawHeight::Derived, _) => below - 240,
            (RawHeight::Explicit(height), _) => {
                let height = if height == 1 { 0 } else { height };
                below - height * 8
            }
        };
    }

    /// Levels are filled in ascending order, so an upper level's heights build
    /// on the composed level below it. `chunks` is `[level][x][y]`, 13 per axis;
    /// `sources` maps a packed region ID to its tile stream.
    pub fn compose(chunks: &[i32], sources: &BTreeMap<i32, Vec<RawTile>>) -> Self {
        let mut terrain = Self {
            tiles: vec![Tile::default(); 4 * SIZE * SIZE],
        };
        for level in 0..4 {
            for chunk_x in 0..13 {
                for chunk_y in 0..13 {
                    let (x, y) = (chunk_x as i32 * 8, chunk_y as i32 * 8);
                    let source = chunks
                        .get((level * 13 + chunk_x) * 13 + chunk_y)
                        .copied()
                        .and_then(Chunk::unpack)
                        .and_then(|chunk| Some((chunk, sources.get(&chunk.region())?)));
                    let Some((chunk, tiles)) = source else {
                        terrain.clear(level, x, y);
                        continue;
                    };
                    let from = [(chunk.zone[0] & 7) * 8, (chunk.zone[1] & 7) * 8];
                    for k in from[0]..from[0] + 8 {
                        for l in from[1]..from[1] + 8 {
                            let raw = tiles[(chunk.level * 64 + k as usize) * 64 + l as usize];
                            let [dx, dy] = rotate(chunk.rotation, k & 7, l & 7);
                            terrain.place(level, x + dx, y + dy, chunk.rotation, raw);
                        }
                    }
                }
            }
        }
        terrain
    }

    /// The scene as map regions at absolute coordinates, with `base` the
    /// absolute tile of scene tile (0, 0). Tiles outside the scene are blank.
    /// `environments` is the scene's 13 x 13 chunk atmosphere from
    /// [`environments`]; chunks outside the scene keep the default.
    pub fn regions(&self, base: [i32; 2], environments: &[Environment]) -> Vec<Region> {
        let mut regions = BTreeMap::<[i32; 2], Region>::new();
        for level in 0..4 {
            for x in 0..SIZE {
                for y in 0..SIZE {
                    let (ax, ay) = (base[0] + x as i32, base[1] + y as i32);
                    let key = [ax.div_euclid(64), ay.div_euclid(64)];
                    let region = regions.entry(key).or_insert_with(|| Region {
                        x: key[0],
                        z: key[1],
                        tiles: vec![Tile::default(); 4 * 64 * 64],
                        // The middle chunk stands for the region where one
                        // value is wanted (the shadow direction).
                        environment: environments.get(6 * 13 + 6).copied().unwrap_or_default(),
                        point_lights: vec![],
                        chunk_environments: vec![Environment::default(); 64],
                        occlusion: vec![],
                        underwater: Default::default(),
                    });
                    if let Some(&environment) = environments.get((x / 8) * 13 + y / 8) {
                        region.chunk_environments
                            [(ax.rem_euclid(64) / 8 * 8 + ay.rem_euclid(64) / 8) as usize] =
                            environment;
                    }
                    region.tiles[level * 4096
                        + ax.rem_euclid(64) as usize * 64
                        + ay.rem_euclid(64) as usize] = self.tile(level, x, y);
                }
            }
        }
        regions.into_values().collect()
    }
}

/// `FogManager.chunksAtmosphere` of an instanced scene, indexed
/// `chunk_x * 13 + chunk_y`: each copied chunk brings its source region's
/// environment. Levels are applied in ascending order, so where several levels
/// copy a chunk into the same place the highest one decides. `sources` maps a
/// packed region ID to that region's environment.
pub fn environments(chunks: &[i32], sources: &BTreeMap<i32, Environment>) -> Vec<Environment> {
    let mut out = vec![Environment::default(); 13 * 13];
    for level in 0..4 {
        for (place, environment) in out.iter_mut().enumerate() {
            if let Some(source) = chunks
                .get(level * 169 + place)
                .copied()
                .and_then(Chunk::unpack)
                .and_then(|chunk| sources.get(&chunk.region()))
            {
                *environment = *source;
            }
        }
    }
    out
}

/// Locations of the composed scene as `[id, x, y, level, shape, rotation]` in
/// scene-local tiles. `sources` maps a packed region ID to that region's
/// placements with region-local coordinates; `size` is a location type's
/// (width, length). As in the original, a location whose rotated south-west
/// tile lands on the scene's outer ring (0 or 103) is dropped.
pub fn locations(
    chunks: &[i32],
    sources: &BTreeMap<i32, Vec<[i32; 6]>>,
    mut size: impl FnMut(i32) -> [i32; 2],
) -> Vec<[i32; 6]> {
    let mut out = vec![];
    for level in 0..4 {
        for chunk_x in 0..13 {
            for chunk_y in 0..13 {
                let Some((chunk, placed)) = chunks
                    .get((level * 13 + chunk_x) * 13 + chunk_y)
                    .copied()
                    .and_then(Chunk::unpack)
                    .and_then(|chunk| Some((chunk, sources.get(&chunk.region())?)))
                else {
                    continue;
                };
                let from = [(chunk.zone[0] & 7) * 8, (chunk.zone[1] & 7) * 8];
                for &[id, x, y, source_level, shape, rotation] in placed {
                    if source_level != chunk.level as i32
                        || !(from[0]..from[0] + 8).contains(&x)
                        || !(from[1]..from[1] + 8).contains(&y)
                    {
                        continue;
                    }
                    let [dx, dy] =
                        rotate_location(chunk.rotation, rotation, size(id), x & 7, y & 7);
                    let (x, y) = (chunk_x as i32 * 8 + dx, chunk_y as i32 * 8 + dy);
                    if x > 0 && y > 0 && x < 103 && y < 103 {
                        out.push([
                            id,
                            x,
                            y,
                            level as i32,
                            shape,
                            (rotation + chunk.rotation) & 3,
                        ]);
                    }
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pack(level: i32, zone: [i32; 2], rotation: i32) -> i32 {
        (level << 24) | (zone[0] << 14) | (zone[1] << 3) | (rotation << 1)
    }
    /// A map file whose level-0 tiles carry height `x + 2` and overlay ID
    /// `y + 1` with shape 3 and angle 1; other levels are derived.
    fn source() -> Vec<RawTile> {
        let mut data = vec![];
        for level in 0..4 {
            for x in 0..64u8 {
                for y in 0..64u8 {
                    if level == 0 {
                        // Overlay opcode 2 + shape * 4 + angle, then flags 2,
                        // underlay 5, then the explicit height.
                        data.extend([2 + 3 * 4 + 1, y + 1, 49 + 2, 81 + 5, 1, x + 2]);
                    } else {
                        data.push(0);
                    }
                }
            }
        }
        raw_tiles(&data).unwrap()
    }
    fn chunks(entries: &[(usize, usize, usize, i32)]) -> Vec<i32> {
        let mut chunks = vec![-1; 4 * 13 * 13];
        for &(level, x, y, packed) in entries {
            chunks[(level * 13 + x) * 13 + y] = packed;
        }
        chunks
    }

    #[test]
    fn raw_tile_stream_keeps_the_last_command_of_each_kind() {
        let tiles = source();
        assert_eq!(tiles.len(), 4 * 64 * 64);
        assert_eq!(
            tiles[3 * 64 + 9],
            RawTile {
                height: RawHeight::Explicit(5),
                overlay: Some([10, 3, 1]),
                underlay: Some(5),
                flags: 2,
            }
        );
        assert_eq!(tiles[4096].height, RawHeight::Derived);
        assert!(raw_tiles(&[2]).is_err());
    }

    #[test]
    fn chunk_fields_and_region() {
        let chunk = Chunk::unpack(pack(2, [401, 410], 3)).unwrap();
        assert_eq!(
            chunk,
            Chunk {
                level: 2,
                rotation: 3,
                zone: [401, 410]
            }
        );
        assert_eq!(chunk.region(), (50 << 8) + 51);
        assert_eq!(Chunk::unpack(-1), None);
    }

    #[test]
    fn tiles_rotate_inside_their_chunk_and_overlay_angles_turn_with_them() {
        // Source chunk (1, 2) of region 50,50: source tiles x 8..16, y 16..24.
        let region = (50 << 8) + 50;
        let sources = BTreeMap::from([(region, source())]);
        for (rotation, at) in [
            // Where source tile (8 + 1, 16 + 2) lands in scene chunk (3, 4).
            (0, [24 + 1, 32 + 2]),
            (1, [24 + 2, 32 + 6]),
            (2, [24 + 6, 32 + 5]),
            (3, [24 + 5, 32 + 1]),
        ] {
            let terrain = Terrain::compose(
                &chunks(&[(0, 3, 4, pack(0, [400 + 1, 400 + 2], rotation))]),
                &sources,
            );
            let tile = terrain.tile(0, at[0] as usize, at[1] as usize);
            // Height byte x + 2 = 11, overlay y + 1 = 19.
            assert_eq!(tile.height, -11 * 8, "rotation {rotation}");
            assert_eq!((tile.overlay, tile.shape), (19, 3));
            assert_eq!(i32::from(tile.rotation), (1 + rotation) & 3);
            assert_eq!((tile.underlay, tile.flags), (5, 2));
        }
    }

    #[test]
    fn a_source_level_can_be_placed_on_another_level_and_stacks_on_it() {
        let region = (50 << 8) + 50;
        let sources = BTreeMap::from([(region, source())]);
        // Level 0 of the source placed on scene levels 0 and 1; source level 1
        // (derived heights) on scene level 2.
        let terrain = Terrain::compose(
            &chunks(&[
                (0, 0, 0, pack(0, [400, 400], 0)),
                (1, 0, 0, pack(0, [400, 400], 0)),
                (2, 0, 0, pack(1, [400, 400], 0)),
            ]),
            &sources,
        );
        // Source tile (3, 0) has height byte 5.
        assert_eq!(terrain.tile(0, 3, 0).height, -40);
        assert_eq!(terrain.tile(1, 3, 0).height, -80, "relative to level 0");
        assert_eq!(terrain.tile(2, 3, 0).height, -80 - 240);
        // Level 3 has no chunk: it takes the level below.
        assert_eq!(terrain.tile(3, 3, 0).height, -80 - 240);
    }

    #[test]
    fn derived_level_zero_height_is_sampled_at_the_scene_tile() {
        let region = (50 << 8) + 50;
        // A file of nothing but opcode 0.
        let sources = BTreeMap::from([(region, raw_tiles(&vec![0; 4 * 4096]).unwrap())]);
        let terrain = Terrain::compose(&chunks(&[(0, 2, 5, pack(0, [403, 407], 0))]), &sources);
        let (x, y) = (2 * 8 + 3, 5 * 8 + 4);
        assert_eq!(
            terrain.tile(0, x, y).height,
            -tile_height(y as i32 + 556_238, x as i32 + 932_731) * 8
        );
    }

    #[test]
    fn a_missing_chunk_takes_its_surroundings() {
        let region = (50 << 8) + 50;
        let sources = BTreeMap::from([(region, source())]);
        // Chunk (0, 0) present, its neighbours (1, 0) and (0, 1) missing.
        let terrain = Terrain::compose(&chunks(&[(0, 0, 0, pack(0, [400, 400], 0))]), &sources);
        // East neighbour: its west column copies the column beside it, and its
        // corner copies the tile to the west.
        assert_eq!(terrain.tile(0, 7, 3).height, -(7 + 2) * 8);
        for y in 0..8 {
            assert_eq!(terrain.tile(0, 8, y).height, -(7 + 2) * 8, "y {y}");
        }
        assert_eq!(terrain.tile(0, 9, 3).height, 0, "the interior is flat");
        // North neighbour: its south row copies the row below it.
        assert_eq!(terrain.tile(0, 3, 8).height, -(3 + 2) * 8);
        assert_eq!(terrain.tile(0, 3, 9).height, 0);
        // A missing source file behaves like a missing chunk.
        let empty = Terrain::compose(&chunks(&[(0, 0, 0, pack(0, [0, 0], 0))]), &BTreeMap::new());
        assert!(empty.tiles.iter().all(|tile| tile.height == 0));
    }

    #[test]
    fn regions_place_the_scene_at_its_absolute_base() {
        let region = (50 << 8) + 50;
        let sources = BTreeMap::from([(region, source())]);
        let terrain = Terrain::compose(&chunks(&[(0, 0, 0, pack(0, [400, 400], 0))]), &sources);
        // Base 6392 = 99 * 64 + 56: the scene straddles regions 99..=101.
        let regions = terrain.regions([6392, 6400], &[]);
        let keys: Vec<[i32; 2]> = regions.iter().map(|r| [r.x, r.z]).collect();
        assert_eq!(
            keys,
            [
                [99, 100],
                [99, 101],
                [100, 100],
                [100, 101],
                [101, 100],
                [101, 101]
            ]
        );
        // Scene tile (3, 0) is absolute (6395, 6400): region 99,100 local (59, 0).
        let west = regions.iter().find(|r| [r.x, r.z] == [99, 100]).unwrap();
        assert_eq!(west.tile(0, 59, 0).height, -40);
        // Scene tile (9, 0) is in region 100,100 at local (1, 0).
        let east = regions.iter().find(|r| [r.x, r.z] == [100, 100]).unwrap();
        assert_eq!(east.tile(0, 1, 0).height, terrain.tile(0, 9, 0).height);
    }

    #[test]
    fn each_chunk_takes_its_source_regions_atmosphere() {
        let (first, second) = ((50 << 8) + 50, (51 << 8) + 50);
        let fog = |fog_depth| Environment {
            fog_depth,
            ..Environment::default()
        };
        let sources = BTreeMap::from([(first, fog(100)), (second, fog(200))]);
        let table = chunks(&[
            (0, 0, 0, pack(0, [400, 400], 0)),
            // A higher level copied into the same place decides.
            (1, 0, 0, pack(0, [408, 400], 0)),
            (0, 1, 0, pack(0, [400, 400], 0)),
            // A chunk whose region has no file leaves the default.
            (0, 2, 0, pack(0, [0, 0], 0)),
        ]);
        let scene = environments(&table, &sources);
        assert_eq!(scene.len(), 169);
        assert_eq!(scene[0].fog_depth, 200);
        assert_eq!(scene[13].fog_depth, 100);
        assert_eq!(scene[26], Environment::default());
        assert_eq!(scene[1], Environment::default());

        let regions = Terrain::compose(&table, &BTreeMap::new()).regions([6392, 6400], &scene);
        // Scene chunk (0, 0) is absolute (6392.., 6400..): region 99,100,
        // region chunk (7, 0). Scene chunk (1, 0) starts region 100,100.
        let west = regions.iter().find(|r| [r.x, r.z] == [99, 100]).unwrap();
        assert_eq!(west.environment_at(59, 0).fog_depth, 200);
        assert_eq!(west.environment_at(55, 0), Environment::default());
        let east = regions.iter().find(|r| [r.x, r.z] == [100, 100]).unwrap();
        assert_eq!(east.environment_at(7, 7).fog_depth, 100);
        assert_eq!(east.environment_at(8, 0), Environment::default());
        // An ordinary region answers with its single environment everywhere.
        let mut plain = vec![0u8; 4 * 4096];
        plain.extend([0, 64, 0, 77]); // environment entry, flag 64: fog depth 77
        let region = Region::decode(50, 50, &plain).unwrap();
        assert_eq!(region.environment_at(63, 63).fog_depth, 77);
    }

    #[test]
    fn locations_rotate_by_their_footprint() {
        let region = (50 << 8) + 50;
        // A 3 x 1 location (width 3, length 1) at source local (9, 18), level 0,
        // inside source chunk (1, 2); another on level 1 that must be skipped;
        // and one outside the chunk.
        let sources = BTreeMap::from([(
            region,
            vec![
                [100, 9, 18, 0, 10, 0],
                [100, 9, 18, 1, 10, 0],
                [100, 20, 18, 0, 10, 0],
            ],
        )]);
        let size = |_| [3, 1];
        for (rotation, expected) in [
            // In-chunk position (1, 2), footprint 3 x 1.
            (0, [24 + 1, 32 + 2]),
            (1, [24 + 2, 32 + 8 - 1 - 3]),
            (2, [24 + 8 - 1 - 3, 32 + 8 - 1 - 2]),
            (3, [24 + 8 - 2 - 1, 32 + 1]),
        ] {
            let placed = locations(
                &chunks(&[(2, 3, 4, pack(0, [401, 402], rotation))]),
                &sources,
                size,
            );
            assert_eq!(
                placed,
                [[100, expected[0], expected[1], 2, 10, rotation]],
                "rotation {rotation}"
            );
        }
        // A location already turned a quarter swaps its footprint first.
        let sources = BTreeMap::from([(region, vec![[100, 9, 18, 0, 10, 1]])]);
        let placed = locations(
            &chunks(&[(0, 3, 4, pack(0, [401, 402], 2))]),
            &sources,
            size,
        );
        // Width and length swap to 1 x 3: x = 8 - 1 - 1, y = 8 - 3 - 2.
        assert_eq!(placed, [[100, 24 + 6, 32 + 3, 0, 10, 3]]);
    }

    #[test]
    fn locations_on_the_outer_ring_are_dropped() {
        let region = (50 << 8) + 50;
        let sources = BTreeMap::from([(
            region,
            vec![
                [100, 0, 0, 0, 10, 0],
                [101, 1, 1, 0, 10, 0],
                [102, 7, 7, 0, 10, 0],
            ],
        )]);
        // Scene chunk (0, 0): tile (0, 0) is on the ring.
        let placed = locations(
            &chunks(&[(0, 0, 0, pack(0, [400, 400], 0))]),
            &sources,
            |_| [1, 1],
        );
        assert_eq!(placed, [[101, 1, 1, 0, 10, 0], [102, 7, 7, 0, 10, 0]]);
        // Scene chunk (12, 12): tile (103, 103) is on the ring.
        let placed = locations(
            &chunks(&[(0, 12, 12, pack(0, [400, 400], 0))]),
            &sources,
            |_| [1, 1],
        );
        assert_eq!(placed, [[100, 96, 96, 0, 10, 0], [101, 97, 97, 0, 10, 0]]);
    }
}
