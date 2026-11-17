//! RT4's occluders: what the original leaves undrawn (and so unpicked)
//! because a long wall or an upper floor stands in front of it.
//!
//! While a scene is loaded `SceneGraph.addLoc` and `buildScene` mark tile
//! edges and tiles in `occludeFlags`: bit 1 for an occluding wall on a tile's
//! west edge, bit 2 for one on its south edge, bit 4 for a level floor or a
//! roof piece above the ground level. The end of `buildScene` turns runs of
//! marks into rectangles ([`Flags::build`]). Every frame
//! `buildActiveOccluders` keeps the rectangles that face the camera, lie in
//! view and do not belong to a building whose roof is removed, with the
//! slopes of the camera's sight lines to their edges ([`Active`]).
//! `isOccluded` then says whether a point lies in a rectangle's shadow, and
//! `isTileOccluded`, `isTileVisible`, `isWallVisible` and `isAreaVisible`
//! (the last three answer "hidden" despite their names) ask it about the
//! corners of a tile, a wall, a wall decoration and scenery ([`Frame`]).
//!
//! Units are the original's: 128 to the tile, heights negative upwards, and
//! `x`, `y` the two ground axes. Integer widths, wrapping and division are
//! Java's.

/// The scene's tile corners per side (`occludeFlags` is `[4][105][105]`).
pub const EDGE: usize = 105;
/// `SceneGraph.occluders` has room for this many; the original would throw
/// on the next one, and this port stops adding.
pub const CAPACITY: usize = 500;

/// An `Occluder` as `Occluder.add` leaves it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Occluder {
    /// 1: a wall along the y axis (constant x). 2: a wall along the x axis.
    /// 4: a floor.
    pub kind: u8,
    pub min_x: i32,
    pub max_x: i32,
    pub min_y: i32,
    pub max_y: i32,
    /// The upper edge: heights are negative upwards.
    pub min_z: i32,
    pub max_z: i32,
}

/// `SceneGraph.occludeFlags` while a scene is built.
#[derive(Clone, Debug)]
pub struct Flags {
    flags: Vec<u8>,
}

impl Default for Flags {
    fn default() -> Self {
        Self::new()
    }
}

impl Flags {
    pub fn new() -> Self {
        Self {
            flags: vec![0; 4 * EDGE * EDGE],
        }
    }

    fn slot(level: usize, x: i32, y: i32) -> Option<usize> {
        (level < 4 && (0..EDGE as i32).contains(&x) && (0..EDGE as i32).contains(&y))
            .then(|| (level * EDGE + x as usize) * EDGE + y as usize)
    }

    /// The marks of a tile corner; 0 outside the scene.
    pub fn get(&self, level: usize, x: i32, y: i32) -> u8 {
        Self::slot(level, x, y).map_or(0, |slot| self.flags[slot])
    }

    fn mark(&mut self, level: usize, x: i32, y: i32, bit: u8) {
        if let Some(slot) = Self::slot(level, x, y) {
            self.flags[slot] |= bit;
        }
    }

    fn has(&self, level: usize, x: i32, y: i32, bit: u8) -> bool {
        self.get(level, x, y) & bit != 0
    }

    fn clear(&mut self, level: usize, x: i32, y: i32, bit: u8) {
        if let Some(slot) = Self::slot(level, x, y) {
            self.flags[slot] &= !bit;
        }
    }

    /// `SceneGraph.addLoc` for a location of the loaded map (its `lowmem`
    /// argument is true there and false for locations added afterwards,
    /// which mark nothing). `occlude` is the type's opcode 23. A type hidden
    /// in HD (opcode 82) never reaches this: `addLoc` returns before.
    pub fn location(
        &mut self,
        level: usize,
        [x, y]: [i32; 2],
        shape: i32,
        rotation: i32,
        occlude: bool,
    ) {
        match shape {
            // A straight wall: its own edge.
            0 if occlude => match rotation & 3 {
                0 => self.mark(level, x, y, 1),
                1 => self.mark(level, x, y + 1, 2),
                2 => self.mark(level, x + 1, y, 1),
                _ => self.mark(level, x, y, 2),
            },
            // A corner wall: both of its edges.
            2 if occlude => match rotation & 3 {
                0 => {
                    self.mark(level, x, y, 1);
                    self.mark(level, x, y + 1, 2);
                }
                1 => {
                    self.mark(level, x, y + 1, 2);
                    self.mark(level, x + 1, y, 1);
                }
                2 => {
                    self.mark(level, x + 1, y, 1);
                    self.mark(level, x, y, 2);
                }
                _ => {
                    self.mark(level, x, y, 2);
                    self.mark(level, x, y, 1);
                }
            },
            // Roof pieces above the ground level, whatever their type says;
            // not the diagonal one with a roof edge (13).
            12 | 14..=17 if level > 0 => self.mark(level, x, y, 4),
            _ => {}
        }
    }

    /// `SceneGraph.buildScene` for a tile with an underlay or an overlay:
    /// above the ground level a level floor hides what is under it, unless
    /// its overlay lets the underlay through (`FloType.occludeUnderlay`
    /// false) or it has an overlay shape and no underlay. `shape` is the
    /// map's `tileShapes` value; `heights` are the corners south-west,
    /// south-east, north-east, north-west.
    #[allow(clippy::too_many_arguments)]
    pub fn floor(
        &mut self,
        level: usize,
        [x, y]: [i32; 2],
        underlay: u8,
        overlay: u8,
        shape: u8,
        overlay_occludes: bool,
        heights: [i32; 4],
    ) {
        if (underlay == 0 && overlay == 0) || level == 0 {
            return;
        }
        let mut occludes = underlay != 0 || shape == 0;
        if overlay > 0 && !overlay_occludes {
            occludes = false;
        }
        let [sw, se, ne, nw] = heights;
        if occludes && sw == se && sw == ne && nw == sw {
            self.mark(level, x, y, 4);
        }
    }

    /// The end of `SceneGraph.buildScene`: marks become occluders. A wall
    /// run is taken with the levels above and below that repeat it and
    /// kept when it covers eight tile faces; a floor rectangle when it
    /// covers four tiles. `height` is `tileHeights[level][x][y]`.
    pub fn build(mut self, height: impl Fn(usize, i32, i32) -> i32) -> Vec<Occluder> {
        let mut occluders = Vec::new();
        let mut add = |occluder: Occluder| {
            if occluders.len() < CAPACITY {
                occluders.push(occluder);
            }
        };
        for level in 0..4 {
            for col in 0..=104 {
                for i in 0..=104 {
                    if self.has(level, i, col, 1) {
                        let (mut j, mut k) = (col, col);
                        while j > 0 && self.has(level, i, j - 1, 1) {
                            j -= 1;
                        }
                        while k < 104 && self.has(level, i, k + 1, 1) {
                            k += 1;
                        }
                        let (mut low, mut high) = (level, level);
                        while low > 0 && (j..=k).all(|n| self.has(low - 1, i, n, 1)) {
                            low -= 1;
                        }
                        while high < 3 && (j..=k).all(|n| self.has(high + 1, i, n, 1)) {
                            high += 1;
                        }
                        if (high + 1 - low) as i32 * (k - j + 1) >= 8 {
                            add(Occluder {
                                kind: 1,
                                min_x: i * 128,
                                max_x: i * 128,
                                min_y: j * 128,
                                max_y: k * 128 + 128,
                                min_z: height(high, i, j).wrapping_sub(240),
                                max_z: height(low, i, j),
                            });
                            for r in low..=high {
                                for s in j..=k {
                                    self.clear(r, i, s, 1);
                                }
                            }
                        }
                    }
                    if self.has(level, i, col, 2) {
                        let (mut j, mut k) = (i, i);
                        while j > 0 && self.has(level, j - 1, col, 2) {
                            j -= 1;
                        }
                        while k < 104 && self.has(level, k + 1, col, 2) {
                            k += 1;
                        }
                        let (mut low, mut high) = (level, level);
                        while low > 0 && (j..=k).all(|n| self.has(low - 1, n, col, 2)) {
                            low -= 1;
                        }
                        while high < 3 && (j..=k).all(|n| self.has(high + 1, n, col, 2)) {
                            high += 1;
                        }
                        if (k + 1 - j) * (high + 1 - low) as i32 >= 8 {
                            add(Occluder {
                                kind: 2,
                                min_x: j * 128,
                                max_x: k * 128 + 128,
                                min_y: col * 128,
                                max_y: col * 128,
                                min_z: height(high, j, col).wrapping_sub(240),
                                max_z: height(low, j, col),
                            });
                            for r in low..=high {
                                for s in j..=k {
                                    self.clear(r, s, col, 2);
                                }
                            }
                        }
                    }
                    if self.has(level, i, col, 4) {
                        let (mut j, mut k) = (i, i);
                        let (mut south, mut north) = (col, col);
                        while south > 0 && self.has(level, i, south - 1, 4) {
                            south -= 1;
                        }
                        while north < 104 && self.has(level, i, north + 1, 4) {
                            north += 1;
                        }
                        while j > 0 && (south..=north).all(|n| self.has(level, j - 1, n, 4)) {
                            j -= 1;
                        }
                        while k < 104 && (south..=north).all(|n| self.has(level, k + 1, n, 4)) {
                            k += 1;
                        }
                        if (k + 1 - j) * (north - (south - 1)) >= 4 {
                            let z = height(level, j, south);
                            add(Occluder {
                                kind: 4,
                                min_x: j * 128,
                                max_x: k * 128 + 128,
                                min_y: south * 128,
                                max_y: north * 128 + 128,
                                min_z: z,
                                max_z: z,
                            });
                            for p in j..=k {
                                for q in south..=north {
                                    self.clear(level, p, q, 4);
                                }
                            }
                        }
                    }
                }
            }
        }
        occluders
    }
}

/// An occluder this frame's camera can be hidden by.
#[derive(Clone, Copy, Debug)]
struct Shade {
    occluder: Occluder,
    /// `Occluder.direction`: 1 and 2 for a wall of kind 1 seen from its
    /// greater and lesser x, 3 and 4 the same for kind 2 and y, 5 a floor.
    direction: u8,
    /// `farXSlope`, `nearXSlope`: the sight lines to `minX` and `maxX`.
    x: [i32; 2],
    /// `nearYSlope`, `farYSlope`: to `minY` and `maxY`.
    y: [i32; 2],
    /// `nearZSlope`, `farZSlope`: to `minZ` and `maxZ`.
    z: [i32; 2],
}

/// The bounds of the roof groups roof removal hides this frame
/// (`ScriptRunner.roofGroupMaxHeight`, `MinX`, `MaxX`, `MinZ`, `MaxZ`).
#[derive(Clone, Copy, Debug, Default)]
pub struct Removed<'a> {
    pub max_height: &'a [i32],
    pub min_x: &'a [i32],
    pub max_x: &'a [i32],
    pub min_z: &'a [i32],
    pub max_z: &'a [i32],
}

impl<'a> From<&'a crate::roof::Groups> for Removed<'a> {
    fn from(groups: &'a crate::roof::Groups) -> Self {
        Self {
            max_height: &groups.max_height,
            min_x: &groups.min_x,
            max_x: &groups.max_x,
            min_z: &groups.min_z,
            max_z: &groups.max_z,
        }
    }
}

impl Removed<'_> {
    /// `buildActiveOccluders`' first test: the occluder reaches into a
    /// removed roof group's box.
    fn holds(&self, o: &Occluder) -> bool {
        (0..self.max_height.len()).any(|i| {
            let [Some(&max_x), Some(&min_x), Some(&max_z), Some(&min_z)] = [
                self.max_x.get(i),
                self.min_x.get(i),
                self.max_z.get(i),
                self.min_z.get(i),
            ] else {
                return false;
            };
            let height = self.max_height[i];
            height != -1_000_000
                && (o.min_z <= height || o.max_z <= height)
                && (o.min_x <= max_x || o.max_x <= max_x)
                && (o.min_x >= min_x || o.max_x >= min_x)
                && (o.min_y <= max_z || o.max_y <= max_z)
                && (o.min_y >= min_z || o.max_y >= min_z)
        })
    }
}

/// `SceneGraph.activeOccluders` of one frame, with the camera they were
/// made for.
#[derive(Clone, Debug, Default)]
pub struct Active {
    shades: Vec<Shade>,
    /// `cameraX` and `cameraY`, which `isWallVisible` reads.
    eye: [i32; 2],
}

impl Active {
    /// `SceneGraph.buildActiveOccluders`. `eye` is the camera's fine x,
    /// height and y (`setPlainTile` keeps x and y inside the scene);
    /// `visibility` is the tile radius of the visibility map and `visible`
    /// reads `visibleTiles[column][row]`, both indices from 0 to twice the
    /// radius.
    pub fn new(
        occluders: &[Occluder],
        removed: Removed,
        eye: [i32; 3],
        visibility: i32,
        visible: impl Fn(i32, i32) -> bool,
    ) -> Self {
        let camera = [
            eye[0].clamp(0, 104 * 128 - 1),
            eye[1],
            eye[2].clamp(0, 104 * 128 - 1),
        ];
        let tile = [camera[0] / 128, camera[2] / 128];
        let span = visibility.wrapping_add(visibility);
        let slope = |edge: i32, eye: i32, distance: i32| {
            edge.wrapping_sub(eye)
                .wrapping_shl(8)
                .wrapping_div(distance)
        };
        let mut shades = Vec::new();
        for o in occluders {
            if removed.holds(o) {
                continue;
            }
            let tiles = [
                [o.min_x / 128, o.max_x / 128],
                [o.min_y / 128, o.max_y / 128],
            ];
            // Where the occluder's tiles fall in the visibility map.
            let window = |axis: usize| {
                (
                    (tiles[axis][0] + visibility - tile[axis]).max(0),
                    (tiles[axis][1] + visibility - tile[axis]).min(span),
                )
            };
            match o.kind {
                1 => {
                    let column = tiles[0][0] + visibility - tile[0];
                    if !(0..=span).contains(&column) {
                        continue;
                    }
                    let (from, to) = window(1);
                    if !(from..=to).any(|row| visible(column, row)) {
                        continue;
                    }
                    let mut distance = camera[0].wrapping_sub(o.min_x);
                    let direction = if distance > 32 {
                        1
                    } else if distance >= -32 {
                        continue;
                    } else {
                        distance = -distance;
                        2
                    };
                    shades.push(Shade {
                        occluder: *o,
                        direction,
                        x: [0; 2],
                        y: [
                            slope(o.min_y, camera[2], distance),
                            slope(o.max_y, camera[2], distance),
                        ],
                        z: [
                            slope(o.min_z, camera[1], distance),
                            slope(o.max_z, camera[1], distance),
                        ],
                    });
                }
                2 => {
                    let row = tiles[1][0] + visibility - tile[1];
                    if !(0..=span).contains(&row) {
                        continue;
                    }
                    let (from, to) = window(0);
                    if !(from..=to).any(|column| visible(column, row)) {
                        continue;
                    }
                    let mut distance = camera[2].wrapping_sub(o.min_y);
                    let direction = if distance > 32 {
                        3
                    } else if distance >= -32 {
                        continue;
                    } else {
                        distance = -distance;
                        4
                    };
                    shades.push(Shade {
                        occluder: *o,
                        direction,
                        x: [
                            slope(o.min_x, camera[0], distance),
                            slope(o.max_x, camera[0], distance),
                        ],
                        y: [0; 2],
                        z: [
                            slope(o.min_z, camera[1], distance),
                            slope(o.max_z, camera[1], distance),
                        ],
                    });
                }
                4 => {
                    let below = o.min_z.wrapping_sub(camera[1]);
                    if below <= 128 {
                        continue;
                    }
                    let (row_from, row_to) = window(1);
                    if row_from > row_to {
                        continue;
                    }
                    let (column_from, column_to) = window(0);
                    if !(column_from..=column_to)
                        .any(|column| (row_from..=row_to).any(|row| visible(column, row)))
                    {
                        continue;
                    }
                    shades.push(Shade {
                        occluder: *o,
                        direction: 5,
                        x: [
                            slope(o.min_x, camera[0], below),
                            slope(o.max_x, camera[0], below),
                        ],
                        y: [
                            slope(o.min_y, camera[2], below),
                            slope(o.max_y, camera[2], below),
                        ],
                        z: [0; 2],
                    });
                }
                _ => {}
            }
        }
        Self {
            shades,
            eye: [camera[0], camera[2]],
        }
    }

    /// How many occluders are active.
    pub fn len(&self) -> usize {
        self.shades.len()
    }

    pub fn is_empty(&self) -> bool {
        self.shades.is_empty()
    }

    /// Each active occluder with its `direction` and the four slopes that
    /// direction uses, in the order `isOccluded` reads them: the two of the
    /// first bounded axis, then the two of the second.
    pub fn shades(&self) -> impl Iterator<Item = (Occluder, u8, [i32; 4])> + '_ {
        self.shades.iter().map(|shade| {
            let [a, b] = match shade.direction {
                1 | 2 => [shade.y, shade.z],
                3 | 4 => [shade.x, shade.z],
                _ => [shade.x, shade.y],
            };
            (shade.occluder, shade.direction, [a[0], a[1], b[0], b[1]])
        })
    }

    /// `SceneGraph.isOccluded`: a point at fine `x`, height `z`, fine `y`
    /// lies behind an active occluder.
    pub fn occluded(&self, x: i32, z: i32, y: i32) -> bool {
        let grow = |slope: i32, delta: i32| slope.wrapping_mul(delta) >> 8;
        let within = |value: i32, bounds: [i32; 2], slopes: [i32; 2], delta: i32| {
            value >= bounds[0].wrapping_add(grow(slopes[0], delta))
                && value <= bounds[1].wrapping_add(grow(slopes[1], delta))
        };
        self.shades.iter().any(|shade| {
            let o = &shade.occluder;
            let (xs, ys, zs) = ([o.min_x, o.max_x], [o.min_y, o.max_y], [o.min_z, o.max_z]);
            let delta = match shade.direction {
                1 => o.min_x.wrapping_sub(x),
                2 => x.wrapping_sub(o.min_x),
                3 => o.min_y.wrapping_sub(y),
                4 => y.wrapping_sub(o.min_y),
                _ => z.wrapping_sub(o.min_z),
            };
            if delta <= 0 {
                return false;
            }
            match shade.direction {
                1 | 2 => within(y, ys, shade.y, delta) && within(z, zs, shade.z, delta),
                3 | 4 => within(x, xs, shade.x, delta) && within(z, zs, shade.z, delta),
                _ => within(x, xs, shade.x, delta) && within(y, ys, shade.y, delta),
            }
        })
    }
}

/// The occlusion queries of one walk over the scene. `SceneGraph`
/// remembers `isTileOccluded`'s answers in `occlusionCache` for the frame,
/// and `isAreaVisible` reads that memory: scenery on several tiles is drawn
/// once one of them has been found in view, whatever its own corners say,
/// and tiles nobody has asked about yet do not count.
pub struct Frame<'a, H> {
    active: &'a Active,
    /// 0 not asked, 1 hidden, 2 in view.
    cache: Vec<u8>,
    height: H,
}

impl<'a, H: Fn(usize, i32, i32) -> i32> Frame<'a, H> {
    /// `height` is `tileHeights[level][x][y]`.
    pub fn new(active: &'a Active, height: H) -> Self {
        Self {
            active,
            cache: if active.is_empty() {
                Vec::new()
            } else {
                vec![0; 4 * EDGE * EDGE]
            },
            height,
        }
    }

    /// The four corners of a tile, each a unit inside it and `lift` above
    /// the ground (negative is up), lie behind occluders.
    fn corners(&self, level: usize, x: i32, y: i32, lift: i32) -> bool {
        let (fx, fy) = (x.wrapping_shl(7), y.wrapping_shl(7));
        let at = |dx: i32, dy: i32| (self.height)(level, x + dx, y + dy).wrapping_add(lift);
        self.active.occluded(fx + 1, at(0, 0), fy + 1)
            && self.active.occluded(fx + 128 - 1, at(1, 0), fy + 1)
            && self.active.occluded(fx + 128 - 1, at(1, 1), fy + 128 - 1)
            && self.active.occluded(fx + 1, at(0, 1), fy + 128 - 1)
    }

    /// `SceneGraph.isTileOccluded`: the tile's ground is hidden.
    pub fn tile(&mut self, level: usize, x: i32, y: i32) -> bool {
        // Nothing is hidden without occluders; the memory is not needed.
        if self.active.is_empty() {
            return false;
        }
        let Some(slot) = Flags::slot(level, x, y) else {
            return false;
        };
        match self.cache[slot] {
            1 => true,
            2 => false,
            _ => {
                let hidden = self.corners(level, x, y, 0);
                self.cache[slot] = if hidden { 1 } else { 2 };
                hidden
            }
        }
    }

    /// `SceneGraph.isTileVisible`: something on the tile reaching `height`
    /// (a model's `getMinY()`, negative upwards) is hidden.
    pub fn object(&mut self, level: usize, x: i32, y: i32, height: i32) -> bool {
        self.tile(level, x, y) && self.corners(level, x, y, height)
    }

    /// `SceneGraph.isWallVisible`: a wall piece with the side or corner
    /// mask `flags` is hidden.
    pub fn wall(&mut self, level: usize, x: i32, y: i32, flags: i32) -> bool {
        if !self.tile(level, x, y) {
            return false;
        }
        let (fx, fy) = (x.wrapping_shl(7), y.wrapping_shl(7));
        let foot = (self.height)(level, x, y).wrapping_sub(1);
        let (middle, top, peak) = (
            foot.wrapping_sub(120),
            foot.wrapping_sub(230),
            foot.wrapping_sub(238),
        );
        let hidden = |x: i32, z: i32, y: i32| self.active.occluded(x, z, y);
        // A side: its two ends at the foot (when seen from outside the
        // tile), halfway up (above the ground level) and at the top.
        let side = |outside: bool, a: [i32; 2], b: [i32; 2]| {
            if outside && !(hidden(a[0], foot, a[1]) && hidden(b[0], foot, b[1])) {
                return false;
            }
            if level > 0 && !(hidden(a[0], middle, a[1]) && hidden(b[0], middle, b[1])) {
                return false;
            }
            hidden(a[0], top, a[1]) && hidden(b[0], top, b[1])
        };
        match flags {
            1 => side(fx > self.active.eye[0], [fx, fy], [fx, fy + 128]),
            2 => side(
                fy < self.active.eye[1],
                [fx, fy + 128],
                [fx + 128, fy + 128],
            ),
            4 => side(
                fx < self.active.eye[0],
                [fx + 128, fy],
                [fx + 128, fy + 128],
            ),
            8 => side(fy > self.active.eye[1], [fx, fy], [fx + 128, fy]),
            _ => {
                if !hidden(fx + 64, peak, fy + 64) {
                    return false;
                }
                match flags {
                    16 => hidden(fx, top, fy + 128),
                    32 => hidden(fx + 128, top, fy + 128),
                    64 => hidden(fx + 128, top, fy),
                    128 => hidden(fx, top, fy),
                    _ => true,
                }
            }
        }
    }

    /// `SceneGraph.isAreaVisible`: scenery over the tiles `min..=max`
    /// reaching `height` is hidden.
    pub fn area(&mut self, level: usize, min: [i32; 2], max: [i32; 2], height: i32) -> bool {
        if min == max {
            return self.object(level, min[0], min[1], height);
        }
        if self.active.is_empty() {
            return false;
        }
        for x in min[0]..=max[0] {
            for y in min[1]..=max[1] {
                if Flags::slot(level, x, y).is_some_and(|slot| self.cache[slot] == 2) {
                    return false;
                }
            }
        }
        // The original's own corners: a unit and two units inside the first
        // tile, and a unit short of the last tile's near edge.
        let west = min[0].wrapping_shl(7) + 1;
        let south = min[1].wrapping_shl(7) + 2;
        let east = max[0].wrapping_shl(7) - 1;
        let north = max[1].wrapping_shl(7) - 1;
        let z = (self.height)(level, min[0], min[1]).wrapping_add(height);
        self.active.occluded(west, z, south)
            && self.active.occluded(east, z, south)
            && self.active.occluded(west, z, north)
            && self.active.occluded(east, z, north)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A wall of eight tiles along the y axis at x = 50 tiles, 240 high.
    fn wall() -> Vec<Occluder> {
        let mut flags = Flags::new();
        for y in 40..48 {
            flags.location(0, [50, y], 0, 0, true);
        }
        flags.build(|_, _, _| 0)
    }

    #[test]
    fn a_run_of_eight_wall_faces_becomes_one_occluder_and_seven_do_not() {
        assert_eq!(
            wall(),
            [Occluder {
                kind: 1,
                min_x: 6400,
                max_x: 6400,
                min_y: 40 * 128,
                max_y: 48 * 128,
                min_z: -240,
                max_z: 0,
            }]
        );
        let mut short = Flags::new();
        for y in 40..47 {
            short.location(0, [50, y], 0, 0, true);
        }
        assert!(short.clone().build(|_, _, _| 0).is_empty());
        // Four tiles repeated on the level above make eight faces.
        let mut stacked = Flags::new();
        for level in 0..2 {
            for y in 40..44 {
                stacked.location(level, [50, y], 0, 0, true);
            }
        }
        let built = stacked.build(|level, _, _| -(level as i32) * 240);
        assert_eq!(built.len(), 1);
        assert_eq!((built[0].min_z, built[0].max_z), (-480, 0));
        // A type that does not occlude marks nothing.
        let mut plain = Flags::new();
        for y in 40..60 {
            plain.location(0, [50, y], 0, 0, false);
        }
        assert!(plain.build(|_, _, _| 0).is_empty());
    }

    #[test]
    fn each_wall_rotation_marks_its_own_edge() {
        let mut flags = Flags::new();
        for rotation in 0..4 {
            flags.location(1, [10 + rotation * 10, 20], 0, rotation, true);
        }
        assert_eq!(flags.get(1, 10, 20), 1);
        assert_eq!(flags.get(1, 20, 21), 2);
        assert_eq!(flags.get(1, 31, 20), 1);
        assert_eq!(flags.get(1, 40, 20), 2);
        let mut corners = Flags::new();
        for rotation in 0..4 {
            corners.location(2, [10 + rotation * 10, 20], 2, rotation, true);
        }
        assert_eq!([corners.get(2, 10, 20), corners.get(2, 10, 21)], [1, 2]);
        assert_eq!([corners.get(2, 20, 21), corners.get(2, 21, 20)], [2, 1]);
        assert_eq!([corners.get(2, 31, 20), corners.get(2, 30, 20)], [1, 2]);
        assert_eq!(corners.get(2, 40, 20), 3);
    }

    #[test]
    fn roofs_and_level_floors_above_the_ground_make_floor_occluders() {
        let mut flags = Flags::new();
        for shape in [12, 13, 14, 15, 16, 17, 18] {
            flags.location(1, [shape, 5], shape, 0, false);
            flags.location(0, [shape, 6], shape, 0, true);
        }
        let marked: Vec<i32> = (12..19).filter(|&x| flags.get(1, x, 5) == 4).collect();
        assert_eq!(marked, [12, 14, 15, 16, 17]);
        assert!((12..19).all(|x| flags.get(0, x, 6) == 0));
        let mut floors = Flags::new();
        // Level with an underlay; sloped; overlay shape without underlay;
        // an overlay that shows the underlay; the ground level.
        floors.floor(1, [1, 1], 5, 0, 0, true, [-240; 4]);
        floors.floor(1, [2, 1], 5, 0, 0, true, [-240, -240, -240, -232]);
        floors.floor(1, [3, 1], 0, 7, 3, true, [-240; 4]);
        floors.floor(1, [4, 1], 5, 7, 0, false, [-240; 4]);
        floors.floor(0, [5, 1], 5, 0, 0, true, [0; 4]);
        floors.floor(1, [6, 1], 0, 7, 0, true, [-240; 4]);
        floors.floor(1, [7, 1], 0, 0, 0, true, [-240; 4]);
        let marked: Vec<i32> = (1..8).filter(|&x| floors.get(1, x, 1) == 4).collect();
        assert_eq!(marked, [1, 6]);
        // Two by two tiles are enough for a floor occluder; three in a row are not.
        let mut square = Flags::new();
        for [x, y] in [
            [30, 30],
            [31, 30],
            [30, 31],
            [31, 31],
            [40, 40],
            [41, 40],
            [42, 40],
        ] {
            square.floor(2, [x, y], 1, 0, 0, true, [-480; 4]);
        }
        assert_eq!(
            square.build(|_, _, _| -480),
            [Occluder {
                kind: 4,
                min_x: 30 * 128,
                max_x: 32 * 128,
                min_y: 30 * 128,
                max_y: 32 * 128,
                min_z: -480,
                max_z: -480,
            }]
        );
    }

    #[test]
    fn points_behind_a_wall_are_hidden_and_those_beside_or_above_are_not() {
        let occluders = wall();
        // The camera east of the wall, level with its middle.
        let eye = [58 * 128, -120, 44 * 128];
        let active = Active::new(&occluders, Removed::default(), eye, 25, |_, _| true);
        assert_eq!(active.len(), 1);
        assert!(active.occluded(45 * 128, -100, 44 * 128));
        assert!(!active.occluded(45 * 128, -100, 30 * 128));
        assert!(!active.occluded(45 * 128, -900, 44 * 128));
        assert!(!active.occluded(52 * 128, -100, 44 * 128));
        // Within 32 units of the wall's plane it is not used at all.
        let close = Active::new(
            &occluders,
            Removed::default(),
            [6400 + 20, -120, 44 * 128],
            25,
            |_, _| true,
        );
        assert!(close.is_empty());
        // Nor when none of its tiles is in view, or its building's roof is off.
        let unseen = Active::new(&occluders, Removed::default(), eye, 25, |_, _| false);
        assert!(unseen.is_empty());
        // The group's box: the wall's top reaches above its height (heights
        // are negative upwards) and its ends lie inside.
        let group = ([-100], [6000], [7000], [5000], [6500]);
        let removed = Removed {
            max_height: &group.0,
            min_x: &group.1,
            max_x: &group.2,
            min_z: &group.3,
            max_z: &group.4,
        };
        assert!(Active::new(&occluders, removed, eye, 25, |_, _| true).is_empty());
        let unused = [-1_000_000];
        let removed = Removed {
            max_height: &unused,
            ..removed
        };
        assert_eq!(
            Active::new(&occluders, removed, eye, 25, |_, _| true).len(),
            1
        );
    }

    #[test]
    fn tiles_walls_and_scenery_behind_the_wall_are_hidden() {
        let occluders = wall();
        let eye = [58 * 128, -120, 44 * 128];
        let active = Active::new(&occluders, Removed::default(), eye, 25, |_, _| true);
        let mut frame = Frame::new(&active, |_, _, _| 0);
        assert!(frame.tile(0, 45, 44));
        assert!(!frame.tile(0, 55, 44));
        assert!(!frame.tile(0, 45, 20));
        // A low object is hidden, one taller than the wall's shadow is not.
        assert!(frame.object(0, 45, 44, -100));
        assert!(!frame.object(0, 45, 44, -2000));
        for flags in [1, 2, 4, 8, 16, 32, 64, 128] {
            assert!(frame.wall(0, 45, 44, flags), "{flags}");
            assert!(!frame.wall(0, 55, 44, flags), "{flags}");
        }
        // Scenery on several tiles: hidden by its own corners until one of
        // its tiles has been found in view.
        assert!(frame.area(0, [44, 42], [46, 45], -100));
        assert!(!frame.area(0, [44, 42], [46, 45], -2000));
        // A spike of ground in the middle is in view although the corners
        // of the scenery standing over it are not.
        let spike = |_: usize, x: i32, y: i32| if [x, y] == [45, 44] { -5000 } else { 0 };
        let mut frame = Frame::new(&active, spike);
        assert!(frame.area(0, [44, 42], [46, 45], -100));
        assert!(!frame.tile(0, 45, 44));
        assert!(!frame.area(0, [44, 42], [46, 45], -100));
    }
}
