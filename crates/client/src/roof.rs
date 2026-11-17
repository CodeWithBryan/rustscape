//! Roof removal: which tiles of which levels the HD scene hides this frame.
//!
//! `ScriptRunner.updateRoofRemovalMode`, `buildAllRoofVisibilityGroups`,
//! `floodFillRoofVisibilityGroup` and `updateRoofVisibilityGroups`, with the
//! test `SceneGraph.setShapedTile` applies to their result. In HD every level
//! is built and drawn (`SceneGraph.allLevelsAreVisible`), and a tile of a level
//! above the player is hidden when the fill marked it.
//!
//! The fill reads three things: the map's render flags by physical plane, the
//! walls and scenery of the scene's tile array, whose columns under a bridge
//! are one level lower than their plane (`SceneGraph.bridgeTile`), and the
//! heights of the plane above the player for the bounds of each roof group.
//! [`Scene`] holds those; [`Roofs`] holds `ScriptRunner.roofVisibility`.

/// Tiles along one side of a scene.
pub const SIZE: usize = 104;
const AREA: usize = SIZE * SIZE;
/// `PathFinder.queueX.length`; the fill's queue wraps at this length.
const QUEUE: usize = 4096;
/// `API.TILE_FLAG_UNDER_ROOF`.
const UNDER_ROOF: u8 = 4;
/// `SceneGraph.WALL_ROTATION_TYPE1`.
const STRAIGHT: [i32; 4] = [1, 2, 4, 8];
/// `LoginManager.WALL_ROTATION_FLAGS`.
const CORNER: [i32; 4] = [16, 32, 64, 128];

/// `ScriptRunner.getRoofRemovalMode` in HD.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// 0: `neverRemoveRoofs`.
    Never,
    /// 1: every roof above a roofed tile of the player's plane.
    All,
    /// 2: only the roofs around the player and in front of the camera.
    Selective,
}
impl Mode {
    pub fn new(never_remove_roofs: bool, remove_roofs_selectively: bool) -> Self {
        if never_remove_roofs {
            Self::Never
        } else if remove_roofs_selectively {
            Self::Selective
        } else {
            Self::All
        }
    }
}

/// One location as `SceneGraph.addLoc` receives it at scene load.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Placement {
    pub plane: usize,
    /// Scene-local tile.
    pub tile: [i32; 2],
    pub shape: u8,
    pub rotation: u8,
    /// The type's width and length before rotation.
    pub size: [i32; 2],
    /// Whether `addLoc` has an entity to place; see [`has_entity`].
    pub entity: bool,
}

/// Whether `SceneGraph.addLoc` obtains an entity for a location in HD.
///
/// A type flagged by opcode 82 is not placed at all. A type with an animation,
/// alternatives or opcode 98 always gets a `Loc`. Any other gets the model
/// `LocType.getGlModel` finds for the shape, which is none when the type lists
/// shapes without this one, or lists none and the shape is not a centrepiece.
/// Models are taken to be present in the cache.
pub fn has_entity(
    shape: u8,
    shapes: Option<&[i32]>,
    models: usize,
    animated: bool,
    undrawn: bool,
) -> bool {
    if undrawn {
        return false;
    }
    if animated {
        return true;
    }
    let requested = match shape {
        11 => 10,
        5..=8 => 4,
        other => other,
    };
    match shapes {
        None => requested == 10 && models != 0,
        Some(list) => list.contains(&i32::from(requested)),
    }
}

#[derive(Clone, Copy, Default)]
struct Tile {
    /// `Wall.primaryFlags` and `Wall.secondaryFlags`.
    wall: Option<[i32; 2]>,
    scenery: [u32; 5],
    count: u8,
}
#[derive(Clone, Copy)]
struct Scenery {
    /// Bits 14..19 and 20..21 of `Scenery.key`.
    shape: u8,
    rotation: u8,
    min: [usize; 2],
    max: [usize; 2],
}

/// What the fill reads of a loaded scene.
#[derive(Clone)]
pub struct Scene {
    flags: Vec<u8>,
    heights: Vec<i32>,
    tiles: Vec<Tile>,
    scenery: Vec<Scenery>,
}
fn index(level: usize, x: usize, y: usize) -> usize {
    level * AREA + x * SIZE + y
}
impl Scene {
    /// `flag(plane, x, y)` is `SceneGraph.renderFlags` and `height(plane, x, y)`
    /// is `SceneGraph.tileHeights`, whose corners run to 104 inclusive.
    pub fn new(
        mut flag: impl FnMut(usize, usize, usize) -> u8,
        mut height: impl FnMut(usize, usize, usize) -> i32,
    ) -> Self {
        let mut flags = Vec::with_capacity(4 * AREA);
        let mut heights = Vec::with_capacity(4 * (SIZE + 1) * (SIZE + 1));
        for plane in 0..4 {
            for x in 0..SIZE {
                for y in 0..SIZE {
                    flags.push(flag(plane, x, y));
                }
            }
            for x in 0..=SIZE {
                for y in 0..=SIZE {
                    heights.push(height(plane, x, y));
                }
            }
        }
        Self {
            flags,
            heights,
            tiles: vec![Tile::default(); 4 * AREA],
            scenery: Vec::new(),
        }
    }
    pub fn flag(&self, plane: usize, x: usize, y: usize) -> u8 {
        self.flags[index(plane, x, y)]
    }
    /// The flag of a tile that may lie outside the scene, where there is none.
    fn flag_at(&self, plane: i32, x: i32, y: i32) -> u8 {
        if (0..4).contains(&plane) && (0..104).contains(&x) && (0..104).contains(&y) {
            self.flag(plane as usize, x as usize, y as usize)
        } else {
            0
        }
    }
    pub fn height(&self, plane: usize, x: usize, y: usize) -> i32 {
        self.heights[plane * (SIZE + 1) * (SIZE + 1) + x * (SIZE + 1) + y]
    }
    /// `SceneGraph.getTileHeight`.
    pub fn tile_height(&self, plane: i32, fine: [i32; 2]) -> i32 {
        crate::ground_geometry::tile_height(plane, fine, |p, x, y| {
            (
                self.height(p as usize, x as usize, y as usize),
                self.flag_at(p, x, y),
            )
        })
    }
    fn bridged(&self, x: usize, y: usize) -> bool {
        self.flag(1, x, y) & 2 == 2
    }
    /// The tile-array level holding what a map file places on `plane`, once
    /// [`Self::bridge`] has run. The ground under a bridge has none: it hangs
    /// off the bridge's own tile and is drawn with it.
    pub fn level(&self, plane: usize, x: usize, y: usize) -> Option<usize> {
        if self.bridged(x, y) {
            plane.checked_sub(1)
        } else {
            Some(plane)
        }
    }

    /// The tile bookkeeping of `SceneGraph.addLoc` for a location read from a
    /// map file, in file order: `readLocs` drops the scene's outer ring, walls
    /// replace the tile's wall (`setWall`) and scenery joins every tile it
    /// covers unless one is outside or already holds five (`addSceneryEntity`).
    /// Wall and ground decorations do not concern the fill.
    pub fn add(&mut self, p: Placement) {
        let [x, y] = p.tile;
        if !(1..=102).contains(&x) || !(1..=102).contains(&y) || p.plane > 3 || !p.entity {
            return;
        }
        let (x, y) = (x as usize, y as usize);
        let rotation = usize::from(p.rotation & 3);
        match p.shape {
            22 | 4..=8 => {}
            10 | 11 => {
                let [width, length] = if rotation & 1 == 1 {
                    [p.size[1], p.size[0]]
                } else {
                    p.size
                };
                self.add_scenery(p, [width, length]);
            }
            0 => self.tiles[index(p.plane, x, y)].wall = Some([STRAIGHT[rotation], 0]),
            1 | 3 => self.tiles[index(p.plane, x, y)].wall = Some([CORNER[rotation], 0]),
            2 => {
                self.tiles[index(p.plane, x, y)].wall =
                    Some([STRAIGHT[rotation], STRAIGHT[(rotation + 1) & 3]]);
            }
            _ => self.add_scenery(p, [1, 1]),
        }
    }
    fn add_scenery(&mut self, p: Placement, size: [i32; 2]) {
        let [x, y] = p.tile;
        for tx in x..x.saturating_add(size[0]) {
            for ty in y..y.saturating_add(size[1]) {
                if !(0..104).contains(&tx) || !(0..104).contains(&ty) {
                    return;
                }
                if self.tiles[index(p.plane, tx as usize, ty as usize)].count >= 5 {
                    return;
                }
            }
        }
        if size[0] <= 0 || size[1] <= 0 {
            // The original links such a record to no tile.
            return;
        }
        let id = self.scenery.len() as u32;
        self.scenery.push(Scenery {
            shape: p.shape & 0x3f,
            rotation: p.rotation & 3,
            min: [x as usize, y as usize],
            max: [(x + size[0] - 1) as usize, (y + size[1] - 1) as usize],
        });
        for tx in x..x + size[0] {
            for ty in y..y + size[1] {
                let tile = &mut self.tiles[index(p.plane, tx as usize, ty as usize)];
                tile.scenery[usize::from(tile.count)] = id;
                tile.count += 1;
            }
        }
    }
    /// The end of `SceneGraph.buildScene`: `bridgeTile` for every column whose
    /// plane 1 carries flag 2. Call once, after every [`Self::add`].
    pub fn bridge(&mut self) {
        for x in 0..SIZE {
            for y in 0..SIZE {
                if self.bridged(x, y) {
                    for level in 0..3 {
                        self.tiles[index(level, x, y)] = self.tiles[index(level + 1, x, y)];
                    }
                    self.tiles[index(3, x, y)] = Tile::default();
                }
            }
        }
    }
    /// Every wall and scenery link of the tile array, one per line, for
    /// comparison with the original's.
    pub fn dump(&self) -> String {
        use std::fmt::Write;
        let mut out = String::new();
        for level in 0..4 {
            for x in 0..SIZE {
                for y in 0..SIZE {
                    let tile = &self.tiles[index(level, x, y)];
                    if let Some([primary, secondary]) = tile.wall {
                        writeln!(out, "w {level} {x} {y} {primary} {secondary}").unwrap();
                    }
                    for (i, &id) in tile.scenery[..usize::from(tile.count)].iter().enumerate() {
                        let s = &self.scenery[id as usize];
                        writeln!(
                            out,
                            "s {level} {x} {y} {i} {} {} {} {} {} {}",
                            s.shape, s.rotation, s.min[0], s.max[0], s.min[1], s.max[1]
                        )
                        .unwrap();
                    }
                }
            }
        }
        out
    }

    /// Whether the floor of `plane` at a tile is hidden: `GlTile` draws a face
    /// when the tile of its render level is visible.
    pub fn floor_hidden(&self, mask: &Mask, plane: usize, tile: [i32; 2]) -> bool {
        let [x, y] = tile;
        if plane > 3 || !(0..104).contains(&x) || !(0..104).contains(&y) {
            return false;
        }
        let level = crate::terrain::render_level(
            plane,
            self.flag(plane, x as usize, y as usize),
            self.flag(1, x as usize, y as usize),
        );
        mask.hidden(level, x, y)
    }
    /// Whether a location is hidden. Walls and decorations are drawn with
    /// their tile. Scenery is drawn once from any visible tile it covers
    /// (`SceneGraph.renderTile`), so it is hidden only when all of them are.
    /// Tiles outside the scene are never hidden.
    pub fn location_hidden(
        &self,
        mask: &Mask,
        plane: usize,
        tile: [i32; 2],
        shape: u8,
        rotation: u8,
        size: [i32; 2],
    ) -> bool {
        if plane > 3 {
            return false;
        }
        let size = match shape {
            10 | 11 if rotation & 1 == 1 => [size[1], size[0]],
            10 | 11 => size,
            _ => [1, 1],
        };
        if size[0] <= 0 || size[1] <= 0 {
            return false;
        }
        for x in tile[0]..tile[0].saturating_add(size[0]) {
            for y in tile[1]..tile[1].saturating_add(size[1]) {
                if !(0..104).contains(&x) || !(0..104).contains(&y) {
                    return false;
                }
                let shown = self
                    .level(plane, x as usize, y as usize)
                    .is_none_or(|level| !mask.hidden(level, x, y));
                if shown {
                    return false;
                }
            }
        }
        true
    }
}

/// `SceneGraph.getLocCollisionMask`: the wall flag a roof edge of this shape
/// and rotation stands for.
fn edge_mask(shape_rotation: i32) -> i32 {
    let shape = shape_rotation & 0x3f;
    let rotation = (shape_rotation >> 6 & 3) as usize;
    if shape == 18 {
        STRAIGHT[rotation]
    } else if shape == 19 || shape == 21 {
        CORNER[rotation]
    } else {
        0
    }
}

/// `Camera.cameraType`, `renderX`, `renderY`, `renderZ` and `cameraPitch`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Camera {
    /// `cameraType == 1`: the camera follows the player.
    pub follow: bool,
    /// Scene-local fine x and y, and the height, negative upward.
    pub position: [i32; 3],
    pub pitch: i32,
}

/// The tiles hidden in one frame, by tile-array level.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Mask {
    bits: Vec<bool>,
}
impl Mask {
    pub fn hidden(&self, level: usize, x: i32, y: i32) -> bool {
        level < 4
            && (0..104).contains(&x)
            && (0..104).contains(&y)
            && self
                .bits
                .get(index(level, x as usize, y as usize))
                .is_some_and(|&bit| bit)
    }
    pub fn is_empty(&self) -> bool {
        !self.bits.contains(&true)
    }
    /// `[level, x, y]` of every hidden tile, in that order.
    pub fn tiles(&self) -> Vec<[usize; 3]> {
        self.bits
            .iter()
            .enumerate()
            .filter(|&(_, &bit)| bit)
            .map(|(i, _)| [i / AREA, i % AREA / SIZE, i % SIZE])
            .collect()
    }
}

/// `ScriptRunner.roofGroupMaxHeight`, `MinX`, `MaxX`, `MinZ` and `MaxZ`. Only
/// the original's occluders read them; they are kept so that the whole fill
/// can be compared.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Groups {
    pub max_height: Vec<i32>,
    pub min_x: Vec<i32>,
    pub max_x: Vec<i32>,
    pub min_z: Vec<i32>,
    pub max_z: Vec<i32>,
}
impl Groups {
    fn allocate(size: usize) -> Self {
        Self {
            max_height: vec![0; size],
            min_x: vec![0; size],
            max_x: vec![0; size],
            min_z: vec![0; size],
            max_z: vec![0; size],
        }
    }
}

/// `ScriptRunner.roofVisibility`, the render cycle that stamps it and the roof
/// group bounds.
pub struct Roofs {
    /// `None` is the original's null array (mode 0).
    marks: Option<Vec<u8>>,
    /// `ScriptRunner.renderCycle`; the caller adds one before each
    /// [`Self::frame`], as `ScriptRunner.drawScene` does.
    pub cycle: i32,
    pub groups: Groups,
    queue_x: Vec<i32>,
    queue_y: Vec<i32>,
}
impl Default for Roofs {
    fn default() -> Self {
        Self {
            marks: None,
            cycle: 0,
            // The field initialisers: five arrays of eight.
            groups: Groups::allocate(8),
            queue_x: vec![0; QUEUE],
            queue_y: vec![0; QUEUE],
        }
    }
}
impl Roofs {
    /// `updateRoofRemovalMode`: the original calls it after a scene is built,
    /// when the minimap follows the player to another plane and when one of
    /// the two preferences changes.
    pub fn update_mode(&mut self, mode: Mode, scene: &Scene, plane: i32) {
        match mode {
            Mode::Never => {
                self.marks = None;
                self.groups = Groups::allocate(0);
            }
            Mode::All => {
                self.fill(0);
                self.groups = Groups::allocate(512);
                // buildAllRoofVisibilityGroups
                let mut group = 0;
                for x in 0..SIZE {
                    for y in 0..SIZE {
                        if self.flood(scene, true, x, y, group, plane) {
                            group += 1;
                        }
                        if group >= 512 {
                            return;
                        }
                    }
                }
            }
            Mode::Selective => {
                self.fill((self.cycle.wrapping_sub(4) & 0xff) as u8);
                // API.GetRoofVisibilityGroupLimit without a plugin handler.
                self.groups = Groups::allocate(2);
            }
        }
    }
    /// `fillRoofVisibility`.
    fn fill(&mut self, value: u8) {
        match &mut self.marks {
            Some(marks) => marks.fill(value),
            None => self.marks = Some(vec![value; 4 * AREA]),
        }
    }

    /// `updateRoofVisibilityGroups`, once per drawn frame. `player` is the
    /// local player's scene-local fine position.
    ///
    /// The original indexes its arrays without checks. Where it would throw,
    /// this reads a tile outside the scene as unroofed, and does nothing when
    /// the camera stands on the player's tile (a division by zero there).
    pub fn frame(
        &mut self,
        mode: Mode,
        scene: &Scene,
        plane: i32,
        player: [i32; 2],
        camera: &Camera,
    ) {
        if mode != Mode::Selective {
            return;
        }
        let stale = (self.cycle.wrapping_sub(4) & 0xff) as u8;
        let marks = self.marks.get_or_insert_with(|| vec![stale; 4 * AREA]);
        let column = self.cycle.rem_euclid(104) as usize;
        for level in 0..4 {
            for y in 0..SIZE {
                marks[index(level, column, y)] = stale;
            }
        }
        if plane == 3 {
            return;
        }
        for i in 0..self.groups.max_height.len() {
            self.groups.max_height[i] = -1000000;
            self.groups.min_x[i] = 1000000;
            self.groups.max_x[i] = 0;
            self.groups.min_z[i] = 1000000;
            self.groups.max_z[i] = 0;
        }
        let roofed = |x: i32, y: i32| scene.flag_at(plane, x, y) & UNDER_ROOF != 0;
        let [camera_x, camera_y, camera_z] = camera.position;
        if !camera.follow {
            let ground = scene.tile_height(plane, [camera_x, camera_y]);
            if ground.wrapping_sub(camera_z) < 800 && roofed(camera_x >> 7, camera_y >> 7) {
                self.flood(
                    scene,
                    false,
                    (camera_x >> 7) as usize,
                    (camera_y >> 7) as usize,
                    1,
                    plane,
                );
            }
            return;
        }
        let (player_x, player_y) = (player[0] >> 7, player[1] >> 7);
        if roofed(player_x, player_y) {
            self.flood(scene, false, player_x as usize, player_y as usize, 0, plane);
        }
        if camera.pitch >= 310 {
            return;
        }
        let (mut x, mut y) = (camera_x >> 7, camera_y >> 7);
        let delta_y = (y.wrapping_sub(player_y)).wrapping_abs();
        let delta_x = (x.wrapping_sub(player_x)).wrapping_abs();
        let mut accumulator = 32768i32;
        if delta_x <= delta_y {
            if delta_y == 0 {
                return;
            }
            let step = delta_x.wrapping_mul(65536) / delta_y;
            while y != player_y {
                if y < player_y {
                    y += 1;
                } else if y > player_y {
                    y -= 1;
                }
                if roofed(x, y) {
                    self.flood(scene, false, x as usize, y as usize, 1, plane);
                    break;
                }
                accumulator = accumulator.wrapping_add(step);
                if accumulator >= 65536 {
                    if player_x > x {
                        x += 1;
                    } else if player_x < x {
                        x -= 1;
                    }
                    accumulator -= 65536;
                    if roofed(x, y) {
                        self.flood(scene, false, x as usize, y as usize, 1, plane);
                        break;
                    }
                }
            }
            return;
        }
        let step = delta_y.wrapping_mul(65536) / delta_x;
        while player_x != x {
            if player_x > x {
                x += 1;
            } else if x > player_x {
                x -= 1;
            }
            if roofed(x, y) {
                self.flood(scene, false, x as usize, y as usize, 1, plane);
                break;
            }
            accumulator = accumulator.wrapping_add(step);
            if accumulator >= 65536 {
                if y < player_y {
                    y += 1;
                } else if player_y < y {
                    y -= 1;
                }
                accumulator -= 65536;
                if roofed(x, y) {
                    self.flood(scene, false, x as usize, y as usize, 1, plane);
                    break;
                }
            }
        }
    }

    /// `floodFillRoofVisibilityGroup`. The queue entries carry, above the
    /// tile, up to three roof-edge shape-and-rotation codes naming the sides a
    /// fringe tile was reached from; a level whose wall or roof edge faces one
    /// of them keeps that tile.
    fn flood(
        &mut self,
        scene: &Scene,
        fill_all: bool,
        start_x: usize,
        start_y: usize,
        group: usize,
        plane: i32,
    ) -> bool {
        if !(0..3).contains(&plane) {
            return false;
        }
        let plane = plane as usize;
        let fill = if fill_all {
            1
        } else {
            (self.cycle & 0xff) as u8
        };
        let Some(marks) = &mut self.marks else {
            return false;
        };
        if fill == marks[index(plane, start_x, start_y)] {
            return false;
        }
        if scene.flag(plane, start_x, start_y) & UNDER_ROOF == 0 {
            return false;
        }
        let (queue_x, queue_y) = (&mut self.queue_x, &mut self.queue_y);
        let groups = &mut self.groups;
        let mut head = 0usize;
        queue_x[0] = start_x as i32;
        let mut tail = 1usize;
        queue_y[0] = start_y as i32;
        marks[index(plane, start_x, start_y)] = fill;
        let open = |x: usize, y: usize| scene.flag(plane, x, y) & UNDER_ROOF == 0;
        while head != tail {
            let side1 = queue_x[head] >> 16 & 0xff;
            let side2 = queue_x[head] >> 24 & 0xff;
            let x = (queue_x[head] & 0xffff) as usize;
            let side3 = queue_y[head] >> 16 & 0xff;
            let mut y = (queue_y[head] & 0xffff) as usize;
            head = (head + 1) & 0xfff;
            let outdoors = open(x, y);
            let mut roof_above = false;
            'levels: for upper in plane + 1..=3 {
                if scene.flag(upper, x, y) & 8 != 0 {
                    continue;
                }
                let tile = scene.tiles[index(upper, x, y)];
                let scenery = &tile.scenery[..usize::from(tile.count)];
                if outdoors {
                    if let Some([primary, secondary]) = tile.wall {
                        let mask = edge_mask(side1);
                        if primary == mask || secondary == mask {
                            continue;
                        }
                        if side2 != 0 {
                            let mask = edge_mask(side2);
                            if mask == primary || secondary == mask {
                                continue;
                            }
                        }
                        if side3 != 0 {
                            let mask = edge_mask(side3);
                            if mask == primary || mask == secondary {
                                continue;
                            }
                        }
                    }
                    for &id in scenery {
                        let s = &scene.scenery[id as usize];
                        let mut shape = i32::from(s.shape);
                        if shape == 21 {
                            shape = 19;
                        }
                        let code = shape | i32::from(s.rotation) << 6;
                        if code == side1
                            || (side2 != 0 && code == side2)
                            || (side3 != 0 && side3 == code)
                        {
                            continue 'levels;
                        }
                    }
                }
                roof_above = true;
                for &id in scenery {
                    let s = &scene.scenery[id as usize];
                    if s.max != s.min {
                        for mark_x in s.min[0]..=s.max[0] {
                            for mark_y in s.min[1]..=s.max[1] {
                                marks[index(upper, mark_x, mark_y)] = fill;
                            }
                        }
                    }
                }
                marks[index(upper, x, y)] = fill;
            }
            if roof_above && group < groups.max_height.len() {
                let height = scene.height(plane + 1, x, y);
                if height > groups.max_height[group] {
                    groups.max_height[group] = height;
                }
                let fine = (x << 7) as i32;
                if fine < groups.min_x[group] {
                    groups.min_x[group] = fine;
                } else if groups.max_x[group] < fine {
                    groups.max_x[group] = fine;
                }
                let fine = (y << 7) as i32;
                if groups.min_z[group] > fine {
                    groups.min_z[group] = fine;
                } else if groups.max_z[group] < fine {
                    groups.max_z[group] = fine;
                }
            }
            if outdoors {
                continue;
            }
            let mut push = |marks: &mut Vec<u8>, x: usize, y: usize, high: u32, sides: i32| {
                queue_x[tail] = x as i32 | high as i32;
                queue_y[tail] = y as i32 | sides;
                tail = (tail + 1) & 0xfff;
                marks[index(plane, x, y)] = fill;
            };
            if x >= 1 && marks[index(plane, x - 1, y)] != fill {
                push(marks, x - 1, y, 0x120000 | 0xD3000000, 0x130000);
            }
            y += 1;
            if y < 104 {
                if x >= 1
                    && fill != marks[index(plane, x - 1, y)]
                    && open(x, y)
                    && open(x - 1, y - 1)
                {
                    push(marks, x - 1, y, 0x52000000 | 0x120000, 0x130000);
                }
                if fill != marks[index(plane, x, y)] {
                    push(marks, x, y, 0x13000000 | 0x520000, 0x530000);
                }
                if x + 1 < 104
                    && marks[index(plane, x + 1, y)] != fill
                    && open(x, y)
                    && open(x + 1, y - 1)
                {
                    push(marks, x + 1, y, 0x92000000 | 0x520000, 0x530000);
                }
            }
            y -= 1;
            if x + 1 < 104 && fill != marks[index(plane, x + 1, y)] {
                push(marks, x + 1, y, 0x920000 | 0x53000000, 0x930000);
            }
            if y >= 1 {
                let y = y - 1;
                if x >= 1
                    && marks[index(plane, x - 1, y)] != fill
                    && open(x, y)
                    && open(x - 1, y + 1)
                {
                    push(marks, x - 1, y, 0xD20000 | 0x12000000, 0xD30000);
                }
                if fill != marks[index(plane, x, y)] {
                    push(marks, x, y, 0xD20000 | 0x93000000, 0xD30000);
                }
                if x + 1 < 104
                    && marks[index(plane, x + 1, y)] != fill
                    && open(x, y)
                    && open(x + 1, y + 1)
                {
                    push(marks, x + 1, y, 0xD2000000 | 0x920000, 0x930000);
                }
            }
        }
        if group < groups.max_height.len() && groups.max_height[group] != -1000000 {
            groups.max_height[group] += 10;
            groups.min_x[group] -= 50;
            groups.max_x[group] += 50;
            groups.max_z[group] += 50;
            groups.min_z[group] -= 50;
        }
        true
    }

    /// The test in `SceneGraph.setShapedTile`, with the level and flag
    /// `ScriptRunner.drawScene` passes: a tile of a level above the player is
    /// hidden when its mark is this frame's (mode 2) or 1 (mode 1).
    pub fn hidden(&self, mode: Mode, plane: i32, level: usize, x: usize, y: usize) -> bool {
        let Some(marks) = &self.marks else {
            return false;
        };
        let flag = if mode == Mode::Selective {
            (self.cycle & 0xff) as u8
        } else {
            1
        };
        level as i32 > plane && marks[index(level, x, y)] == flag
    }
    pub fn mask(&self, mode: Mode, plane: i32) -> Mask {
        let mut bits = vec![false; 4 * AREA];
        if self.marks.is_some() {
            for level in 0..4 {
                for x in 0..SIZE {
                    for y in 0..SIZE {
                        bits[index(level, x, y)] = self.hidden(mode, plane, level, x, y);
                    }
                }
            }
        }
        Mask { bits }
    }
    /// The raw marks, for comparison with the original's array.
    pub fn marks(&self) -> Option<&[u8]> {
        self.marks.as_deref()
    }
}
