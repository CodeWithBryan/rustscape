//! RT4 `LightingManager`'s choice of up to four point lights for each drawn
//! object, and the per-vertex key the renderer bakes so its shader can repeat
//! that choice.
//!
//! [`Selector`] is the original state machine: `updateLightsForTile`,
//! `updateLightsForRegion`, `updateLightsForWallPiece`, `pruneInactiveLights`,
//! `resetActiveLights` and the cache reset of `updateAllLightAnimations`. Its
//! log is the sequence of GL light changes the original would issue.
//!
//! [`resolve`] is the stateless form the shader uses. It reproduces the set of
//! lights the scene traversal leaves active for one object (the slot a light
//! occupies does not change fixed-function lighting). It does not reproduce
//! the two places where the original's result depends on what was drawn
//! before: a wall piece whose neighbour tile is outside the scene keeps the
//! previous object's lights, and a region query that repeats the previous
//! rectangle after a prune keeps the pruned set.

/// `SceneGraph.WALL_DRAW_FLAGS`: wall pieces drawn after the tile's contents,
/// indexed by the camera's side of the tile.
pub const WALL_DRAW_FLAGS: [i32; 9] = [76, 8, 137, 4, 0, 1, 38, 2, 19];
/// `SceneGraph.WALL_VISIBILITY_FLAGS`: wall pieces drawn before the contents.
pub const WALL_VISIBILITY_FLAGS: [i32; 9] = [19, 55, 38, 155, 255, 110, 137, 205, 76];
/// `SceneGraph.WALL_UNCULL_FLAGS`: corner pieces drawn once scenery is done.
pub const WALL_UNCULL_FLAGS: [i32; 9] = [160, 192, 80, 96, 0, 144, 80, 48, 160];

/// A GL light change: `Enable` is `glEnable`, `Set` the parameters of a light
/// placed in a slot (issued whether or not the slot was already enabled).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Event {
    Enable(usize),
    Disable(usize),
    Set(usize, i32),
}

/// The light grid as the original packs it: four one-based light numbers per
/// tile, lowest byte first.
pub trait Grid {
    fn packed(&self, plane: i32, x: i32, y: i32) -> u32;
}
impl<F: Fn(i32, i32, i32) -> u32> Grid for F {
    fn packed(&self, plane: i32, x: i32, y: i32) -> u32 {
        self(plane, x, y)
    }
}

#[derive(Clone, Debug)]
pub struct Selector {
    pub active: [i32; 4],
    pub enabled: [bool; 4],
    retained: [bool; 4],
    candidates: [i32; 4],
    cached: [i32; 5],
    pub width: i32,
    pub length: i32,
    pub log: Vec<Event>,
}
impl Selector {
    /// The state after `initGlLightParameters` and one
    /// `updateAllLightAnimations`: no light active, nothing cached.
    pub fn new(width: i32, length: i32) -> Self {
        Self {
            active: [-1; 4],
            enabled: [false; 4],
            retained: [false; 4],
            candidates: [0; 4],
            cached: [-1; 5],
            width,
            length,
            log: Vec::new(),
        }
    }
    fn disable(&mut self, slot: usize) {
        if self.enabled[slot] {
            self.enabled[slot] = false;
            self.log.push(Event::Disable(slot));
        }
    }
    fn enable(&mut self, slot: usize, light: i32) {
        if !self.enabled[slot] {
            self.log.push(Event::Enable(slot));
            self.enabled[slot] = true;
        }
        self.log.push(Event::Set(slot, light));
    }
    /// `resetActiveLights`.
    pub fn reset(&mut self) {
        for slot in 0..4 {
            self.active[slot] = -1;
            self.disable(slot);
        }
    }
    /// The cache reset at the end of `updateAllLightAnimations`.
    pub fn invalidate(&mut self) {
        self.cached = [-1; 5];
    }
    fn place_candidates(&mut self, count: usize) {
        for candidate in 0..count {
            for slot in 0..4 {
                if !self.retained[slot] {
                    self.active[slot] = self.candidates[candidate];
                    self.retained[slot] = true;
                    self.enable(slot, self.candidates[candidate]);
                    break;
                }
            }
        }
        for slot in 0..4 {
            if !self.retained[slot] {
                self.active[slot] = -1;
                self.disable(slot);
            }
        }
    }
    /// `updateLightsForTile`.
    pub fn tile(&mut self, grid: &impl Grid, plane: i32, x: i32, y: i32) {
        if self.cached == [plane, x, y, x, y] {
            return;
        }
        self.retained = [false; 4];
        let mut count = 0;
        let mut packed = grid.packed(plane, x, y);
        'light: while packed != 0 {
            let light = (packed & 0xff) as i32 - 1;
            packed >>= 8;
            for slot in 0..4 {
                if light == self.active[slot] {
                    self.retained[slot] = true;
                    continue 'light;
                }
            }
            self.candidates[count] = light;
            count += 1;
        }
        self.place_candidates(count);
        self.cached = [plane, x, y, x, y];
    }
    /// `updateLightsForRegion`: the first four distinct lights over the
    /// rectangle, columns outermost.
    pub fn region(&mut self, grid: &impl Grid, plane: i32, start: [i32; 2], end: [i32; 2]) {
        if self.cached == [plane, start[0], start[1], end[0], end[1]] {
            return;
        }
        self.retained = [false; 4];
        let mut matched = 0;
        let mut count = 0;
        'scan: for x in start[0]..=end[0] {
            for y in start[1]..=end[1] {
                let mut packed = grid.packed(plane, x, y);
                'light: while packed != 0 {
                    let light = (packed & 0xff) as i32 - 1;
                    packed >>= 8;
                    if self.candidates[..count].contains(&light) {
                        continue;
                    }
                    for slot in 0..4 {
                        if light == self.active[slot] {
                            if !self.retained[slot] {
                                self.retained[slot] = true;
                                matched += 1;
                                if matched == 4 {
                                    break 'scan;
                                }
                            }
                            continue 'light;
                        }
                    }
                    self.candidates[count] = light;
                    count += 1;
                    matched += 1;
                    if matched == 4 {
                        break 'scan;
                    }
                }
            }
        }
        self.place_candidates(count);
        self.cached = [plane, start[0], start[1], end[0], end[1]];
    }
    /// `updateLightsForWallPiece`: the tile on the far side of the piece, or
    /// no change at all when that tile is outside the scene.
    pub fn wall_piece(&mut self, grid: &impl Grid, flags: i32, plane: i32, x: i32, y: i32) {
        if let Some([dx, dy]) = wall_neighbour(flags) {
            let (nx, ny) = (x + dx, y + dy);
            let inside = |d: i32, n: i32, size: i32| match d {
                -1 => n >= 0,
                1 => n < size,
                _ => true,
            };
            if inside(dx, nx, self.width) && inside(dy, ny, self.length) {
                self.tile(grid, plane, nx, ny);
            }
        }
    }
    /// `pruneInactiveLights`: drops every active light found on neither tile.
    /// The cached query is left as it was.
    pub fn prune(&mut self, grid: &impl Grid, plane: i32, a: [i32; 2], b: [i32; 2]) {
        'slot: for slot in 0..4 {
            if self.active[slot] != -1 {
                for tile in [a, b] {
                    let mut packed = grid.packed(plane, tile[0], tile[1]);
                    while packed != 0 {
                        let light = (packed & 0xff) as i32 - 1;
                        packed >>= 8;
                        if light == self.active[slot] {
                            continue 'slot;
                        }
                    }
                }
            }
            self.active[slot] = -1;
            self.disable(slot);
        }
    }
}

/// The tile a wall piece with exactly this flag borders, as an offset.
pub fn wall_neighbour(flags: i32) -> Option<[i32; 2]> {
    Some(match flags {
        1 => [-1, 0],
        4 => [1, 0],
        8 => [0, -1],
        2 => [0, 1],
        16 => [-1, 1],
        32 => [1, 1],
        128 => [-1, -1],
        64 => [1, -1],
        _ => return None,
    })
}

/// The index `renderTile` forms from the camera's tile and the drawn tile.
pub fn camera_side(camera: [i32; 2], tile: [i32; 2]) -> usize {
    let mut index = 0;
    if camera[0] == tile[0] {
        index += 1;
    } else if camera[0] < tile[0] {
        index += 2;
    }
    if camera[1] == tile[1] {
        index += 3;
    } else if camera[1] > tile[1] {
        index += 6;
    }
    index
}

/// What a drawn object is, as far as its lights are concerned.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Object {
    /// Scenery and actors: the first four lights over `size` tiles from the
    /// south-west tile. One tile for decorations and item piles.
    Area { size: [i32; 2] },
    /// A wall piece carrying one flag of `WALL_ROTATION_TYPE1` or
    /// `WALL_ROTATION_FLAGS`. `diagonal` is location shape 1 with its
    /// rotation, whose corner piece has its own camera test.
    Wall {
        flag: i32,
        diagonal: Option<i32>,
        /// The tile's level is one above the level it is drawn on (a bridge).
        raised: bool,
    },
    /// Location shape 9 with its rotation.
    DiagonalScenery { rotation: i32 },
}

const KEY_AREA: u32 = 1 << 10;

/// The 11-bit vertex key for an object whose south-west tile is `tile`, for a
/// vertex on `vertex_tile`. Zero means no key: the vertex's own tile is used.
/// Objects whose vertices lie outside the encodable offsets get no key.
pub fn key(object: Object, tile: [i32; 2], vertex_tile: [i32; 2]) -> u32 {
    let offset = [tile[0] - vertex_tile[0], tile[1] - vertex_tile[1]];
    match object {
        Object::Area { size } => {
            if offset.iter().any(|d| !(-6..=1).contains(d)) {
                return 0;
            }
            let size = size.map(|n| n.clamp(1, 4) as u32 - 1);
            KEY_AREA
                | ((offset[0] + 6) as u32) << 7
                | ((offset[1] + 6) as u32) << 4
                | size[0] << 2
                | size[1]
        }
        Object::Wall { .. } | Object::DiagonalScenery { .. } => {
            if offset.iter().any(|d| !(-2..=1).contains(d)) {
                return 0;
            }
            let (code, raised) = match object {
                Object::Wall {
                    diagonal: Some(rotation),
                    raised,
                    ..
                } => (8 + (rotation & 3) as u32, raised),
                Object::Wall { flag, raised, .. } => {
                    let Some(index) = (0..8u32).find(|i| flag == 1 << i) else {
                        return 0;
                    };
                    (index, raised)
                }
                Object::DiagonalScenery { rotation } => (12 + (rotation & 3) as u32, false),
                Object::Area { .. } => unreachable!(),
            };
            // Bit 0 keeps the key non-zero for code 0 at offset (-2, -2).
            ((offset[0] + 2) as u32) << 8
                | ((offset[1] + 2) as u32) << 6
                | code << 2
                | u32::from(raised) << 1
                | 1
        }
    }
}

fn distinct(cells: impl IntoIterator<Item = u32>) -> [u8; 4] {
    let mut out = [0u8; 4];
    let mut count = 0;
    for mut packed in cells {
        while packed != 0 && count < 4 {
            let light = (packed & 0xff) as u8;
            packed >>= 8;
            if !out[..count].contains(&light) {
                out[count] = light;
                count += 1;
            }
        }
    }
    out
}

/// The lights (one-based, zero for none) a keyed vertex is lit by. `plane` is
/// the level the object is drawn on, `camera` the eye in scene units of 128
/// per tile, and `size` the grid's extent. This is the shader's rule.
pub fn resolve(
    grid: &impl Grid,
    size: [i32; 2],
    key: u32,
    plane: i32,
    vertex_tile: [i32; 2],
    camera: [i32; 2],
) -> [u8; 4] {
    let cell = |plane: i32, x: i32, y: i32| {
        if x < 0 || y < 0 || x >= size[0] || y >= size[1] || !(0..4).contains(&plane) {
            0
        } else {
            grid.packed(plane, x, y)
        }
    };
    if key == 0 {
        return distinct([cell(plane, vertex_tile[0], vertex_tile[1])]);
    }
    if key & KEY_AREA != 0 {
        let x = vertex_tile[0] + (key >> 7 & 7) as i32 - 6;
        let y = vertex_tile[1] + (key >> 4 & 7) as i32 - 6;
        let (w, l) = ((key >> 2 & 3) as i32 + 1, (key & 3) as i32 + 1);
        return distinct((x..x + w).flat_map(|x| (y..y + l).map(move |y| cell(plane, x, y))));
    }
    let x = vertex_tile[0] + (key >> 8 & 3) as i32 - 2;
    let y = vertex_tile[1] + (key >> 6 & 3) as i32 - 2;
    let code = (key >> 2 & 15) as i32;
    let raised = key >> 1 & 1 != 0;
    let own = distinct([cell(plane, x, y)]);
    let camera_tile = camera.map(|n| n >> 7);
    let side = [(camera_tile[0] - x).signum(), (camera_tile[1] - y).signum()];
    // Fine offsets of the tile's centre from the eye, as `renderTile` forms them.
    let (fx, fy) = ((x << 7) + 64 - camera[0], (y << 7) + 64 - camera[1]);
    if code >= 12 {
        let (a, b) = if code & 1 == 1 {
            if fy > -fx {
                ([x, y - 1], [x - 1, y])
            } else {
                ([x, y + 1], [x + 1, y])
            }
        } else if fy > fx {
            ([x, y - 1], [x + 1, y])
        } else {
            ([x, y + 1], [x - 1, y])
        };
        let both = [cell(plane, a[0], a[1]), cell(plane, b[0], b[1])];
        let mut out = [0u8; 4];
        let mut count = 0;
        for light in own {
            let kept = light != 0
                && both
                    .iter()
                    .any(|&packed| packed.to_le_bytes().contains(&light));
            if kept {
                out[count] = light;
                count += 1;
            }
        }
        return out;
    }
    let flag = if code >= 8 {
        16 << (code - 8)
    } else {
        1 << code
    };
    let [dx, dy] = wall_neighbour(flag).unwrap();
    let late = if dx != 0 && dy != 0 {
        side == [dx, dy]
    } else if dx != 0 {
        side[0] == dx
    } else {
        side[1] == dy
    };
    if late {
        let (nx, ny) = (x + dx, y + dy);
        if nx < 0 || ny < 0 || nx >= size[0] || ny >= size[1] {
            return own;
        }
        return distinct([cell(plane + i32::from(raised), nx, ny)]);
    }
    if code >= 8 {
        let dot = dx * side[0] + dy * side[1];
        let deferred = match side[0].abs() + side[1].abs() {
            2 => dot == 0,
            1 => dot == 1,
            _ => false,
        };
        let beyond = match code - 8 {
            0 => fy + 64 < fx - 64,
            1 => fy + 64 < -(fx + 64),
            2 => fy - 64 > fx + 64,
            _ => fy - 64 > -(fx - 64),
        };
        let (nx, ny) = (x + dx, y + dy);
        if deferred && beyond && nx >= 0 && ny >= 0 && nx < size[0] && ny < size[1] {
            return distinct([cell(plane, nx, ny)]);
        }
    }
    own
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sorted(mut lights: [u8; 4]) -> [u8; 4] {
        lights.sort_unstable_by(|a, b| b.cmp(a));
        lights
    }
    fn active(selector: &Selector) -> [u8; 4] {
        sorted(selector.active.map(|light| (light + 1) as u8))
    }
    /// A grid with a different light mix on every tile.
    fn grid(plane: i32, x: i32, y: i32) -> u32 {
        let n = (x * 7 + y * 13 + plane * 3) as u32;
        match n % 5 {
            0 => 0,
            1 => 1 + n % 4,
            2 => (1 + n % 3) | (5 + n % 2) << 8,
            3 => 9 | 2 << 8 | (6 + n % 3) << 16,
            _ => 3 | 4 << 8 | 7 << 16 | 8 << 24,
        }
    }

    // Transcription tests: the expected sets come from running the ported
    // state machine the way `renderTile` calls it for each kind of object.
    #[test]
    fn the_wall_tables_split_every_flag_between_the_three_passes() {
        for side in 0..9 {
            let (early, late, deferred) = (
                WALL_VISIBILITY_FLAGS[side],
                WALL_DRAW_FLAGS[side],
                WALL_UNCULL_FLAGS[side],
            );
            assert_eq!(early & late, 0);
            assert_eq!(early & deferred, 0);
            assert_eq!(late & deferred, 0);
            assert_eq!(early | late | deferred, 255);
        }
    }

    #[test]
    fn area_keys_resolve_to_the_region_query() {
        for (tile, size) in [([10, 10], [1, 1]), ([20, 31], [3, 2]), ([7, 9], [4, 4])] {
            let mut selector = Selector::new(104, 104);
            selector.region(
                &grid,
                1,
                tile,
                [tile[0] + size[0] - 1, tile[1] + size[1] - 1],
            );
            for vertex in [[0, 0], [1, 0], [-1, 2], [3, 3]] {
                let vertex_tile = [tile[0] + vertex[0], tile[1] + vertex[1]];
                let key = key(Object::Area { size }, tile, vertex_tile);
                assert_ne!(key, 0);
                assert_eq!(
                    sorted(resolve(&grid, [104, 104], key, 1, vertex_tile, [0, 0])),
                    active(&selector)
                );
            }
        }
        // Too far from the object: no key, so the vertex's own tile is used.
        assert_eq!(key(Object::Area { size: [1, 1] }, [10, 10], [17, 10]), 0);
    }

    /// The lights `renderTile` leaves for one wall piece from one eye position.
    fn wall_reference(
        flag: i32,
        diagonal: Option<i32>,
        raised: bool,
        tile: [i32; 2],
        camera: [i32; 2],
    ) -> [u8; 4] {
        let side = camera_side(camera.map(|n| n >> 7), tile);
        let mut selector = Selector::new(104, 104);
        let plane = 1;
        if flag & WALL_VISIBILITY_FLAGS[side] != 0 {
            selector.tile(&grid, plane, tile[0], tile[1]);
        } else if flag & WALL_DRAW_FLAGS[side] != 0 {
            // No change outside the scene: the reference starts from the
            // tile's own lights so the comparison has a defined state.
            selector.tile(&grid, plane, tile[0], tile[1]);
            selector.wall_piece(&grid, flag, plane + i32::from(raised), tile[0], tile[1]);
        } else {
            let (mut i, mut j) = (
                (tile[0] << 7) + 64 - camera[0],
                (tile[1] << 7) + 64 - camera[1],
            );
            let mut target = tile;
            match diagonal {
                Some(0) => {
                    i -= 64;
                    j += 64;
                    if j < i {
                        target = [tile[0] - 1, tile[1] + 1];
                    }
                }
                Some(1) => {
                    i += 64;
                    j += 64;
                    if j < -i {
                        target = [tile[0] + 1, tile[1] + 1];
                    }
                }
                Some(2) => {
                    i += 64;
                    j -= 64;
                    if j > i {
                        target = [tile[0] + 1, tile[1] - 1];
                    }
                }
                Some(_) => {
                    i -= 64;
                    j -= 64;
                    if j > -i {
                        target = [tile[0] - 1, tile[1] - 1];
                    }
                }
                None => {}
            }
            selector.tile(&grid, plane, target[0], target[1]);
        }
        active(&selector)
    }

    #[test]
    fn wall_keys_follow_the_camera_as_the_three_wall_passes_do() {
        let tile = [40, 50];
        let cameras: Vec<[i32; 2]> = [-3, 0, 4]
            .into_iter()
            .flat_map(|dx| {
                [-2, 0, 5]
                    .into_iter()
                    .flat_map(move |dy| [[5, 100], [120, 9]].map(|f| (dx, dy, f)))
            })
            .map(|(dx, dy, fine)| [((40 + dx) << 7) + fine[0], ((50 + dy) << 7) + fine[1]])
            .collect();
        for camera in cameras {
            for raised in [false, true] {
                for index in 0..8 {
                    let flag = 1 << index;
                    let diagonals: &[Option<i32>] = if index >= 4 {
                        &[None, Some(index - 4)]
                    } else {
                        &[None]
                    };
                    for &diagonal in diagonals {
                        let key = key(
                            Object::Wall {
                                flag,
                                diagonal,
                                raised,
                            },
                            tile,
                            [41, 49],
                        );
                        assert_ne!(key, 0);
                        assert_eq!(
                            sorted(resolve(&grid, [104, 104], key, 1, [41, 49], camera)),
                            wall_reference(flag, diagonal, raised, tile, camera),
                            "flag {flag} diagonal {diagonal:?} raised {raised} camera {camera:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn diagonal_scenery_keeps_the_lights_of_the_two_tiles_on_the_eye_side() {
        let tile = [30, 30];
        for rotation in 0..4 {
            for camera in [[3000, 4500], [4500, 3000], [3900, 3800], [3800, 3900]] {
                let mut selector = Selector::new(104, 104);
                selector.tile(&grid, 0, 30, 30);
                let (fx, fy) = ((30 << 7) + 64 - camera[0], (30 << 7) + 64 - camera[1]);
                if rotation == 1 || rotation == 3 {
                    if fy > -fx {
                        selector.prune(&grid, 0, [30, 29], [29, 30]);
                    } else {
                        selector.prune(&grid, 0, [30, 31], [31, 30]);
                    }
                } else if fy > fx {
                    selector.prune(&grid, 0, [30, 29], [31, 30]);
                } else {
                    selector.prune(&grid, 0, [30, 31], [29, 30]);
                }
                let key = key(Object::DiagonalScenery { rotation }, tile, [30, 30]);
                assert_eq!(
                    sorted(resolve(&grid, [104, 104], key, 0, [30, 30], camera)),
                    active(&selector)
                );
            }
        }
    }

    #[test]
    fn slots_are_kept_for_lights_that_stay_and_reused_for_new_ones() {
        let cells = |_: i32, x: i32, _: i32| match x {
            0 => 1 | 2 << 8 | 3 << 16,
            1 => 3 | 4 << 8,
            _ => 0,
        };
        let mut selector = Selector::new(104, 104);
        selector.tile(&cells, 0, 0, 0);
        assert_eq!(selector.active, [0, 1, 2, -1]);
        selector.tile(&cells, 0, 1, 0);
        // Light 3 (index 2) keeps slot 2; light 4 takes the first freed slot.
        assert_eq!(selector.active, [3, -1, 2, -1]);
        let before = selector.log.len();
        selector.tile(&cells, 0, 1, 0);
        assert_eq!(selector.log.len(), before, "a repeated query is cached");
        selector.invalidate();
        selector.tile(&cells, 0, 2, 0);
        assert_eq!(selector.active, [-1; 4]);
        assert_eq!(selector.enabled, [false; 4]);
    }
}
