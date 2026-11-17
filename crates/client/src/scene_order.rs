//! The order in which RT4 draws a scene.
//!
//! `SceneGraph.setShapedTile` and `renderTile` walk the tile grid from the
//! far corners toward the camera, level by level, drawing each tile's ground,
//! walls, decorations, item pile and scenery, and holding scenery back until
//! every tile it stands on is ready. The original picks while it draws, so
//! this order is the order of its pick results (the later entry wins the
//! left click) and of its walk-destination test (the last tile drawn under
//! the mouse wins).
//!
//! This module keeps the tile structure the original keeps and replays the
//! walk, reporting what would be drawn instead of drawing it. Parts are the
//! caller's own numbers. The walk asks an [`Occlusion`] the questions the
//! original asks `isTileOccluded`, `isWallVisible`, `isTileVisible` and
//! `isAreaVisible`, at the same moments (their answers depend on which tiles
//! have been asked about before); `crate::occlusion` answers them.
use std::collections::VecDeque;

/// What the walk asks about hidden things. `level` is the tile's own level
/// (`Tile.level`, which a bridge does not lower).
pub trait Occlusion {
    /// `isTileOccluded`: the tile's ground is hidden.
    fn tile(&mut self, level: usize, x: i32, y: i32) -> bool;
    /// `isWallVisible`: a wall piece with the side or corner mask `flags`
    /// is hidden.
    fn wall(&mut self, level: usize, x: i32, y: i32, flags: i32) -> bool;
    /// `isTileVisible` with the height of the wall decoration `part`.
    fn decor(&mut self, level: usize, x: i32, y: i32, part: u32) -> bool;
    /// `isAreaVisible` with the height of the scenery `part` over the tiles
    /// `min..=max`.
    fn area(&mut self, level: usize, min: [i32; 2], max: [i32; 2], part: u32) -> bool;
    /// A model the walk reached and left undrawn because of an answer above
    /// (or because its tile has no ground to stand on).
    fn culled(&mut self, _part: u32) {}
}

/// No occluders: nothing is hidden.
pub struct Open;

impl Occlusion for Open {
    fn tile(&mut self, _: usize, _: i32, _: i32) -> bool {
        false
    }
    fn wall(&mut self, _: usize, _: i32, _: i32, _: i32) -> bool {
        false
    }
    fn decor(&mut self, _: usize, _: i32, _: i32, _: u32) -> bool {
        false
    }
    fn area(&mut self, _: usize, _: [i32; 2], _: [i32; 2], _: u32) -> bool {
        false
    }
}

/// `SceneGraph.width` and `length`.
pub const SIZE: i32 = 104;
/// `SceneGraph.levels`.
pub const LEVELS: usize = 4;

const WALL_DRAW_FLAGS: [i32; 9] = [76, 8, 137, 4, 0, 1, 38, 2, 19];
const WALL_SCENERY_DIR_TYPE1: [i32; 9] = [0, 0, 2, 0, 0, 2, 1, 1, 0];
const WALL_SCENERY_DIR_TYPE2: [i32; 9] = [2, 0, 0, 2, 0, 0, 0, 4, 4];
const WALL_SCENERY_DIR_TYPE3: [i32; 9] = [0, 4, 4, 8, 0, 0, 8, 0, 0];
const WALL_SCENERY_DIR_TYPE4: [i32; 9] = [1, 1, 0, 0, 0, 8, 0, 0, 8];
const WALL_UNCULL_FLAGS: [i32; 9] = [160, 192, 80, 96, 0, 144, 80, 48, 160];
const WALL_VISIBILITY_FLAGS: [i32; 9] = [19, 55, 38, 155, 255, 110, 137, 205, 76];
/// `SceneGraph.WALL_ROTATION_TYPE1`: a straight wall's side by rotation.
pub const WALL_SIDES: [i32; 4] = [1, 2, 4, 8];
/// `LoginManager.WALL_ROTATION_FLAGS`: a corner pillar's corner by rotation.
pub const WALL_CORNERS: [i32; 4] = [16, 32, 64, 128];

/// What a tile has for ground.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Ground {
    #[default]
    None,
    /// `PlainTile`; one without a colour is drawn only for the walk test, on
    /// levels up to the walker's.
    Plain { coloured: bool },
    /// `ShapedTile`.
    Shaped,
}

/// One thing the original would draw, in its order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Draw {
    /// A tile's ground, with heights taken from `level`.
    Ground { level: u8, tile: [u8; 2] },
    /// A model: the part number given when it was placed.
    Model(u32),
}

#[derive(Clone, Copy, Debug)]
struct Wall {
    primary: u32,
    secondary: Option<u32>,
    primary_flags: i32,
    secondary_flags: i32,
}

#[derive(Clone, Copy, Debug)]
struct WallDecor {
    primary: u32,
    secondary: Option<u32>,
    flags: i32,
    /// The value the original keeps in `WallDecor.zFine`.
    orientation: i32,
}

#[derive(Clone, Copy, Debug)]
struct ObjStack {
    /// Secondary, tertiary, primary: the original's drawing order.
    parts: [Option<u32>; 3],
    raised: bool,
}

#[derive(Clone, Copy, Debug)]
struct Scenery {
    part: u32,
    level: usize,
    min: [i32; 2],
    max: [i32; 2],
    fine: [i32; 2],
    location: bool,
    cycle: u32,
    priority: i32,
}

#[derive(Clone, Debug, Default)]
struct Tile {
    level: usize,
    draw_level: usize,
    x: i32,
    y: i32,
    ground: Ground,
    wall: Option<Wall>,
    wall_decor: Option<WallDecor>,
    /// Part, and whether its key has bit 31 (drawn before the walls).
    ground_decor: Option<(u32, bool)>,
    obj_stack: Option<ObjStack>,
    scenery: Vec<(u32, i32)>,
    all_interior: i32,
    linked: Option<u32>,
    visible: bool,
    /// `Tile.checkedBelowLevel`.
    pending: bool,
    /// `Tile.hasUpdated`.
    updated: bool,
    scenery_draw: i32,
    scenery_drawn_dir: i32,
    scenery_skip_dir: i32,
    wall_draw: i32,
    /// Its place in the queue, or 0: adding a queued tile moves it to the tail.
    ticket: u64,
}

/// The camera of one walk.
#[derive(Clone, Copy, Debug)]
struct Frame {
    eye: [i32; 2],
    tile: [i32; 2],
    min: [i32; 2],
    max: [i32; 2],
    walk_level: Option<usize>,
}

const NONE: u32 = u32::MAX;

/// `SceneGraph.tiles` and what hangs from it.
#[derive(Clone, Debug)]
pub struct Scene {
    tiles: Vec<Tile>,
    grid: Vec<u32>,
    scenery: Vec<Scenery>,
    free: Vec<u32>,
    temporary: Vec<u32>,
    cycle: u32,
    queue: VecDeque<(u32, u64)>,
    tickets: u64,
}

impl Default for Scene {
    fn default() -> Self {
        Self::new()
    }
}

impl Scene {
    pub fn new() -> Self {
        Self {
            tiles: Vec::new(),
            grid: vec![NONE; LEVELS * (SIZE * SIZE) as usize],
            scenery: Vec::new(),
            free: Vec::new(),
            temporary: Vec::new(),
            cycle: 0,
            queue: VecDeque::new(),
            tickets: 0,
        }
    }

    fn slot(level: usize, x: i32, y: i32) -> Option<usize> {
        (level < LEVELS && (0..SIZE).contains(&x) && (0..SIZE).contains(&y))
            .then(|| level * (SIZE * SIZE) as usize + (x * SIZE + y) as usize)
    }

    fn at(&self, level: usize, x: i32, y: i32) -> Option<usize> {
        let index = self.grid[Self::slot(level, x, y)?];
        (index != NONE).then_some(index as usize)
    }

    fn ensure(&mut self, level: usize, x: i32, y: i32) -> Option<usize> {
        let slot = Self::slot(level, x, y)?;
        if self.grid[slot] == NONE {
            self.grid[slot] = self.tiles.len() as u32;
            self.tiles.push(Tile {
                level,
                draw_level: level,
                x,
                y,
                ..Tile::default()
            });
        }
        Some(self.grid[slot] as usize)
    }

    /// Tiles on `level` and every level below it, as the placing calls make them.
    fn ensure_down(&mut self, level: usize, x: i32, y: i32) -> Option<usize> {
        for below in 0..level {
            self.ensure(below, x, y)?;
        }
        self.ensure(level, x, y)
    }

    /// `SceneGraph.setBaseLevel`: every tile of a level exists.
    pub fn fill_level(&mut self, level: usize) {
        for x in 0..SIZE {
            for y in 0..SIZE {
                self.ensure(level, x, y);
            }
        }
    }

    /// `SceneGraph.setTile`.
    pub fn set_ground(&mut self, level: usize, x: i32, y: i32, ground: Ground) {
        if let Some(tile) = self.ensure_down(level, x, y) {
            self.tiles[tile].ground = ground;
        }
    }

    /// `SceneGraph.setWall`, with the two side masks of its models.
    #[allow(clippy::too_many_arguments)]
    pub fn set_wall(
        &mut self,
        level: usize,
        x: i32,
        y: i32,
        primary: u32,
        secondary: Option<u32>,
        primary_flags: i32,
        secondary_flags: i32,
    ) {
        if let Some(tile) = self.ensure_down(level, x, y) {
            self.tiles[tile].wall = Some(Wall {
                primary,
                secondary,
                primary_flags,
                secondary_flags,
            });
        }
    }

    /// `SceneGraph.setWallDecor`. `flags` is a side mask, or 256 for the
    /// diagonal kinds, whose side then comes from `orientation`.
    #[allow(clippy::too_many_arguments)]
    pub fn set_wall_decor(
        &mut self,
        level: usize,
        x: i32,
        y: i32,
        primary: u32,
        secondary: Option<u32>,
        flags: i32,
        orientation: i32,
    ) {
        if let Some(tile) = self.ensure_down(level, x, y) {
            self.tiles[tile].wall_decor = Some(WallDecor {
                primary,
                secondary,
                flags,
                orientation,
            });
        }
    }

    /// `SceneGraph.setGroundDecor`. `early` is bit 31 of its key (a
    /// definition with opcode 96): drawn before the tile's walls.
    pub fn set_ground_decor(&mut self, level: usize, x: i32, y: i32, part: u32, early: bool) {
        if let Some(tile) = self.ensure(level, x, y) {
            self.tiles[tile].ground_decor = Some((part, early));
        }
    }

    /// `SceneGraph.setObjStack`: primary, secondary and tertiary models, and
    /// whether scenery on the tile lifts the pile (`heightOffset != 0`).
    pub fn set_obj_stack(
        &mut self,
        level: usize,
        x: i32,
        y: i32,
        [primary, secondary, tertiary]: [Option<u32>; 3],
        raised: bool,
    ) {
        if let Some(tile) = self.ensure(level, x, y) {
            self.tiles[tile].obj_stack = Some(ObjStack {
                parts: [secondary, tertiary, primary],
                raised,
            });
        }
    }

    /// `SceneGraph.removeObjStack`.
    pub fn remove_obj_stack(&mut self, level: usize, x: i32, y: i32) {
        if let Some(tile) = self.at(level, x, y) {
            self.tiles[tile].obj_stack = None;
        }
    }

    /// `SceneGraph.addSceneryEntity`: scenery covering `size` tiles from
    /// `min`. False when it leaves the scene or a tile already holds five.
    /// `location` marks a map object (key type 2); `temporary` scenery is
    /// removed by `clear_temporary`.
    #[allow(clippy::too_many_arguments)]
    pub fn add_scenery(
        &mut self,
        level: usize,
        min: [i32; 2],
        size: [i32; 2],
        fine: [i32; 2],
        part: u32,
        location: bool,
        temporary: bool,
    ) -> bool {
        if level >= LEVELS {
            return false;
        }
        let end = [
            min[0].saturating_add(size[0]),
            min[1].saturating_add(size[1]),
        ];
        for x in min[0]..end[0] {
            for y in min[1]..end[1] {
                if !(0..SIZE).contains(&x) || !(0..SIZE).contains(&y) {
                    return false;
                }
                if self
                    .at(level, x, y)
                    .is_some_and(|tile| self.tiles[tile].scenery.len() >= 5)
                {
                    return false;
                }
            }
        }
        let scenery = Scenery {
            part,
            level,
            min,
            max: [end[0] - 1, end[1] - 1],
            fine,
            location,
            cycle: 0,
            priority: 0,
        };
        let index = match self.free.pop() {
            Some(index) => {
                self.scenery[index as usize] = scenery;
                index
            }
            None => {
                self.scenery.push(scenery);
                self.scenery.len() as u32 - 1
            }
        };
        for x in min[0]..end[0] {
            for y in min[1]..end[1] {
                let mut flags = 0;
                if x > min[0] {
                    flags += 1;
                }
                if x < end[0] - 1 {
                    flags += 4;
                }
                if y > min[1] {
                    flags += 8;
                }
                if y < end[1] - 1 {
                    flags += 2;
                }
                if let Some(tile) = self.ensure_down(level, x, y) {
                    let tile = &mut self.tiles[tile];
                    tile.scenery.push((index, flags));
                    tile.all_interior |= flags;
                }
            }
        }
        if temporary {
            self.temporary.push(index);
        }
        true
    }

    /// `SceneGraph.add`: temporary scenery for an actor at a fine position,
    /// covering the tiles within `radius`, widened toward where it faces
    /// when `extend` (a stretching sequence) is set.
    pub fn add_actor(
        &mut self,
        level: usize,
        fine: [i32; 2],
        radius: i32,
        part: u32,
        orientation: i32,
        extend: bool,
    ) -> bool {
        let mut min = [fine[0].wrapping_sub(radius), fine[1].wrapping_sub(radius)];
        let mut max = [fine[0].wrapping_add(radius), fine[1].wrapping_add(radius)];
        if extend {
            if orientation > 640 && orientation < 1408 {
                max[1] = max[1].wrapping_add(128);
            }
            if orientation > 1152 && orientation < 1920 {
                max[0] = max[0].wrapping_add(128);
            }
            if !(384..=1664).contains(&orientation) {
                min[1] = min[1].wrapping_sub(128);
            }
            if orientation > 128 && orientation < 896 {
                min[0] = min[0].wrapping_sub(128);
            }
        }
        let (min, max) = (min.map(|n| n / 128), max.map(|n| n / 128));
        self.add_scenery(
            level,
            min,
            [max[0] + 1 - min[0], max[1] - min[1] + 1],
            fine,
            part,
            false,
            true,
        )
    }

    /// `SceneGraph.removeScenery(Scenery)`.
    fn remove_scenery(&mut self, index: u32) {
        let scenery = self.scenery[index as usize];
        for x in scenery.min[0]..=scenery.max[0] {
            for y in scenery.min[1]..=scenery.max[1] {
                if let Some(tile) = self.at(scenery.level, x, y) {
                    let tile = &mut self.tiles[tile];
                    if let Some(at) = tile.scenery.iter().position(|&(s, _)| s == index) {
                        tile.scenery.remove(at);
                    }
                    tile.all_interior = tile.scenery.iter().fold(0, |all, &(_, f)| all | f);
                }
            }
        }
        self.free.push(index);
    }

    /// `ScriptRunner.clearSceneScenery`: remove this frame's actors.
    pub fn clear_temporary(&mut self) {
        for index in std::mem::take(&mut self.temporary) {
            self.remove_scenery(index);
        }
    }

    /// `SceneGraph.bridgeTile`: the tiles above a bridge move down a level
    /// and the old ground hangs below the new ground tile.
    pub fn bridge(&mut self, x: i32, y: i32) {
        let Some(base) = Self::slot(0, x, y) else {
            return;
        };
        let step = (SIZE * SIZE) as usize;
        let ground = self.grid[base];
        for level in 0..LEVELS - 1 {
            let moved = self.grid[base + (level + 1) * step];
            self.grid[base + level * step] = moved;
            if moved != NONE {
                let tile = &mut self.tiles[moved as usize];
                tile.draw_level = tile.draw_level.saturating_sub(1);
                for i in 0..tile.scenery.len() {
                    let scenery = &mut self.scenery[tile.scenery[i].0 as usize];
                    if scenery.location && scenery.min == [x, y] {
                        scenery.level = scenery.level.saturating_sub(1);
                    }
                }
            }
        }
        self.grid[base + (LEVELS - 1) * step] = NONE;
        if let Some(top) = self.ensure(0, x, y) {
            self.tiles[top].linked = (ground != NONE).then_some(ground);
        }
    }

    /// The parts `SceneGraph.isLocValid` compares keys against on a tile:
    /// its wall, wall decoration, ground decoration and scenery.
    pub fn parts_at(&self, level: usize, x: i32, y: i32) -> Vec<u32> {
        let Some(tile) = self.at(level, x, y) else {
            return Vec::new();
        };
        let tile = &self.tiles[tile];
        tile.wall
            .iter()
            .map(|wall| wall.primary)
            .chain(tile.wall_decor.iter().map(|decor| decor.primary))
            .chain(tile.ground_decor.iter().map(|decor| decor.0))
            .chain(
                tile.scenery
                    .iter()
                    .map(|&(s, _)| self.scenery[s as usize].part),
            )
            .collect()
    }

    fn push(&mut self, tile: usize) {
        self.tickets += 1;
        self.tiles[tile].ticket = self.tickets;
        self.queue.push_back((tile as u32, self.tickets));
    }

    fn pop(&mut self) -> Option<usize> {
        while let Some((tile, ticket)) = self.queue.pop_front() {
            let tile = tile as usize;
            if self.tiles[tile].ticket == ticket {
                self.tiles[tile].ticket = 0;
                return Some(tile);
            }
        }
        None
    }

    fn pending_at(&self, level: usize, x: i32, y: i32) -> bool {
        self.at(level, x, y)
            .is_some_and(|tile| self.tiles[tile].pending)
    }

    fn push_pending(&mut self, level: usize, x: i32, y: i32) {
        if let Some(tile) = self.at(level, x, y) {
            if self.tiles[tile].pending {
                self.push(tile);
            }
        }
    }

    /// One frame: `setPlainTile`'s camera, then `setShapedTile`'s marking and
    /// its two sweeps. `eye` is the camera's fine x and y; `visible` says
    /// whether the tile in a grid cell is inside the view and not hidden by
    /// roof removal; `walk_level` is the level a pending walk click belongs
    /// to. `emit` receives everything drawn, in order.
    pub fn draw(
        &mut self,
        eye: [i32; 2],
        visibility: i32,
        visible: impl FnMut(usize, i32, i32) -> bool,
        walk_level: Option<usize>,
        emit: impl FnMut(Draw),
    ) {
        self.draw_occluded(eye, visibility, visible, walk_level, &mut Open, emit);
    }

    /// [`Self::draw`] with the frame's occlusion answers.
    pub fn draw_occluded(
        &mut self,
        eye: [i32; 2],
        visibility: i32,
        mut visible: impl FnMut(usize, i32, i32) -> bool,
        walk_level: Option<usize>,
        cull: &mut impl Occlusion,
        mut emit: impl FnMut(Draw),
    ) {
        let eye = eye.map(|n| n.clamp(0, SIZE * 128 - 1));
        let tile = eye.map(|n| n / 128);
        let frame = Frame {
            eye,
            tile,
            min: tile.map(|n| (n - visibility).max(0)),
            max: tile.map(|n| (n + visibility).min(SIZE)),
            walk_level,
        };
        self.cycle = self.cycle.wrapping_add(1);
        self.queue.clear();
        let mut count = 0i32;
        for level in 0..LEVELS {
            for x in frame.min[0]..frame.max[0] {
                for y in frame.min[1]..frame.max[1] {
                    let Some(index) = self.at(level, x, y) else {
                        continue;
                    };
                    let shown = visible(level, x, y);
                    let tile = &mut self.tiles[index];
                    if shown {
                        tile.visible = true;
                        tile.pending = true;
                        tile.updated = !tile.scenery.is_empty();
                        count += 1;
                    } else {
                        tile.visible = false;
                        tile.pending = false;
                        tile.scenery_draw = 0;
                    }
                }
            }
        }
        for check_interior in [true, false] {
            for level in 0..LEVELS {
                for dx in -visibility..=0 {
                    let (near_x, far_x) = (tile[0] + dx, tile[0] - dx);
                    if near_x < frame.min[0] && far_x >= frame.max[0] {
                        continue;
                    }
                    for dy in -visibility..=0 {
                        let (near_y, far_y) = (tile[1] + dy, tile[1] - dy);
                        let mut corners = [None; 4];
                        if near_x >= frame.min[0] {
                            if near_y >= frame.min[1] {
                                corners[0] = Some([near_x, near_y]);
                            }
                            if far_y < frame.max[1] {
                                corners[1] = Some([near_x, far_y]);
                            }
                        }
                        if far_x < frame.max[0] {
                            if near_y >= frame.min[1] {
                                corners[2] = Some([far_x, near_y]);
                            }
                            if far_y < frame.max[1] {
                                corners[3] = Some([far_x, far_y]);
                            }
                        }
                        for [x, y] in corners.into_iter().flatten() {
                            if let Some(start) = self.at(level, x, y) {
                                if self.tiles[start].visible {
                                    self.render_tile(
                                        start,
                                        check_interior,
                                        &frame,
                                        &mut count,
                                        cull,
                                        &mut emit,
                                    );
                                }
                            }
                        }
                        if count == 0 {
                            return;
                        }
                    }
                }
            }
        }
    }

    /// `SceneGraph.renderTile`.
    fn render_tile(
        &mut self,
        start: usize,
        mut check_interior: bool,
        frame: &Frame,
        count: &mut i32,
        cull: &mut impl Occlusion,
        emit: &mut impl FnMut(Draw),
    ) {
        self.push(start);
        loop {
            // A tile leaves this loop once its scenery has nothing left to wait for.
            let index = loop {
                let index = loop {
                    let Some(index) = self.pop() else {
                        return;
                    };
                    let tile = &self.tiles[index];
                    if !tile.pending {
                        continue;
                    }
                    if !tile.visible {
                        break index;
                    }
                    let (x, y, level, interior) =
                        (tile.x, tile.y, tile.draw_level, tile.all_interior);
                    if check_interior {
                        if level > 0 && self.pending_at(level - 1, x, y) {
                            continue;
                        }
                        let blocked = |scene: &Self, x: i32, y: i32, side: i32| {
                            scene.at(level, x, y).is_some_and(|other| {
                                let other = &scene.tiles[other];
                                other.pending && (other.visible || interior & side == 0)
                            })
                        };
                        if x <= frame.tile[0] && x > frame.min[0] && blocked(self, x - 1, y, 1) {
                            continue;
                        }
                        if x >= frame.tile[0] && x < frame.max[0] - 1 && blocked(self, x + 1, y, 4)
                        {
                            continue;
                        }
                        if y <= frame.tile[1] && y > frame.min[1] && blocked(self, x, y - 1, 8) {
                            continue;
                        }
                        if y >= frame.tile[1] && y < frame.max[1] - 1 && blocked(self, x, y + 1, 2)
                        {
                            continue;
                        }
                    } else {
                        check_interior = true;
                    }
                    self.tiles[index].visible = false;
                    self.draw_tile(index, frame, cull, emit);
                    break index;
                };
                if self.tiles[index].scenery_draw != 0 {
                    let tile = &self.tiles[index];
                    let waiting = tile.scenery.iter().any(|&(scenery, interior)| {
                        self.scenery[scenery as usize].cycle != self.cycle
                            && interior & tile.scenery_draw == tile.scenery_drawn_dir
                    });
                    if !waiting {
                        if let Some(wall) = tile.wall {
                            if cull.wall(tile.level, tile.x, tile.y, wall.primary_flags) {
                                cull.culled(wall.primary);
                            } else {
                                emit(Draw::Model(wall.primary));
                            }
                        }
                        self.tiles[index].scenery_draw = 0;
                    }
                }
                if !self.tiles[index].updated {
                    break index;
                }
                // A missing tile under scenery is the original's caught exception.
                if self.draw_scenery(index, frame, cull, emit).is_none() {
                    self.tiles[index].updated = false;
                    break index;
                }
                if !self.tiles[index].updated {
                    break index;
                }
            };
            let tile = &self.tiles[index];
            let (x, y, level) = (tile.x, tile.y, tile.draw_level);
            if !tile.pending || tile.scenery_draw != 0 {
                continue;
            }
            if x <= frame.tile[0] && x > frame.min[0] && self.pending_at(level, x - 1, y) {
                continue;
            }
            if x >= frame.tile[0] && x < frame.max[0] - 1 && self.pending_at(level, x + 1, y) {
                continue;
            }
            if y <= frame.tile[1] && y > frame.min[1] && self.pending_at(level, x, y - 1) {
                continue;
            }
            if y >= frame.tile[1] && y < frame.max[1] - 1 && self.pending_at(level, x, y + 1) {
                continue;
            }
            self.tiles[index].pending = false;
            *count -= 1;
            let tile = &self.tiles[index];
            if let Some(stack) = tile.obj_stack.filter(|stack| stack.raised) {
                for part in stack.parts.into_iter().flatten() {
                    emit(Draw::Model(part));
                }
            }
            if tile.wall_draw != 0 {
                let own = tile.level;
                if let Some(decor) = tile.wall_decor {
                    let hidden = cull.decor(own, x, y, decor.primary);
                    let part = if decor.flags & tile.wall_draw != 0 {
                        Some(decor.primary)
                    } else if decor.flags == 256 {
                        let (across, along) = decor_sides(&decor, tile, frame);
                        if along >= across {
                            Some(decor.primary)
                        } else {
                            decor.secondary
                        }
                    } else {
                        None
                    };
                    if let Some(part) = part {
                        if hidden {
                            cull.culled(part);
                        } else {
                            emit(Draw::Model(part));
                        }
                    }
                }
                if let Some(wall) = tile.wall {
                    for (flags, part) in [
                        (wall.secondary_flags, wall.secondary),
                        (wall.primary_flags, Some(wall.primary)),
                    ] {
                        let Some(part) = part.filter(|_| flags & tile.wall_draw != 0) else {
                            continue;
                        };
                        if cull.wall(own, x, y, flags) {
                            cull.culled(part);
                        } else {
                            emit(Draw::Model(part));
                        }
                    }
                }
            }
            if level < LEVELS - 1 {
                self.push_pending(level + 1, x, y);
            }
            if x < frame.tile[0] {
                self.push_pending(level, x + 1, y);
            }
            if y < frame.tile[1] {
                self.push_pending(level, x, y + 1);
            }
            if x > frame.tile[0] {
                self.push_pending(level, x - 1, y);
            }
            if y > frame.tile[1] {
                self.push_pending(level, x, y - 1);
            }
        }
    }

    /// The first visit to a visible tile: bridge underside, ground, ground
    /// decoration, the walls facing the camera, wall decoration, item pile.
    fn draw_tile(
        &mut self,
        index: usize,
        frame: &Frame,
        cull: &mut impl Occlusion,
        emit: &mut impl FnMut(Draw),
    ) {
        let tile = &self.tiles[index];
        let (x, y, draw_level, own) = (tile.x, tile.y, tile.draw_level, tile.level);
        let place = [x as u8, y as u8];
        if let Some(linked) = tile.linked {
            let linked = &self.tiles[linked as usize];
            if linked.ground != Ground::None {
                // The answer only reaches the software renderer, but asking
                // is remembered.
                cull.tile(0, x, y);
                emit(Draw::Ground {
                    level: 0,
                    tile: place,
                });
            }
            if let Some(wall) = linked.wall {
                emit(Draw::Model(wall.primary));
            }
            for &(scenery, _) in &linked.scenery {
                emit(Draw::Model(self.scenery[scenery as usize].part));
            }
        }
        let ground = Draw::Ground {
            level: tile.level as u8,
            tile: place,
        };
        // A hidden tile still takes the walk test (and a plain one takes it
        // whatever its colour), but nothing standing on its ground is drawn.
        let has_ground = match tile.ground {
            Ground::None => false,
            Ground::Shaped => {
                let hidden = cull.tile(own, x, y);
                emit(ground);
                !hidden
            }
            Ground::Plain { coloured } => {
                let hidden = cull.tile(own, x, y);
                if hidden || coloured || frame.walk_level.is_some_and(|level| draw_level <= level) {
                    emit(ground);
                }
                !hidden
            }
        };
        if let Some((part, true)) = tile.ground_decor {
            if has_ground {
                emit(Draw::Model(part));
            } else {
                cull.culled(part);
            }
        }
        let mut side = 0;
        let mut facing = 0;
        let mut wall_draw = tile.wall_draw;
        if tile.wall.is_some() || tile.wall_decor.is_some() {
            if frame.tile[0] == x {
                side += 1;
            } else if frame.tile[0] < x {
                side += 2;
            }
            if frame.tile[1] == y {
                side += 3;
            } else if frame.tile[1] > y {
                side += 6;
            }
            facing = WALL_VISIBILITY_FLAGS[side];
            wall_draw = WALL_DRAW_FLAGS[side];
        }
        let mut scenery_flags = (
            tile.scenery_draw,
            tile.scenery_drawn_dir,
            tile.scenery_skip_dir,
        );
        if let Some(wall) = tile.wall {
            scenery_flags = if wall.primary_flags & WALL_UNCULL_FLAGS[side] == 0 {
                (0, scenery_flags.1, scenery_flags.2)
            } else {
                let (all, drawn) = match wall.primary_flags {
                    16 => (3, WALL_SCENERY_DIR_TYPE1[side]),
                    32 => (6, WALL_SCENERY_DIR_TYPE2[side]),
                    64 => (12, WALL_SCENERY_DIR_TYPE3[side]),
                    _ => (9, WALL_SCENERY_DIR_TYPE4[side]),
                };
                (all, drawn, all - drawn)
            };
            for (flags, part) in [
                (wall.primary_flags, Some(wall.primary)),
                (wall.secondary_flags, wall.secondary),
            ] {
                let Some(part) = part.filter(|_| flags & facing != 0) else {
                    continue;
                };
                if cull.wall(own, x, y, flags) {
                    cull.culled(part);
                } else {
                    emit(Draw::Model(part));
                }
            }
        }
        if let Some(decor) = tile.wall_decor {
            let hidden = cull.decor(own, x, y, decor.primary);
            let part = if decor.flags & facing != 0 {
                Some(decor.primary)
            } else if decor.flags == 256 {
                let (across, along) = decor_sides(&decor, tile, frame);
                let (shown, other) = if along < across {
                    (Some(decor.primary), decor.secondary)
                } else {
                    (decor.secondary, Some(decor.primary))
                };
                // The other model follows when the tile is finished, except
                // on the camera's own tile, which has no near side: there it
                // is not drawn at all.
                if wall_draw == 0 {
                    if let Some(other) = other {
                        cull.culled(other);
                    }
                }
                shown
            } else {
                None
            };
            if let Some(part) = part {
                if hidden {
                    cull.culled(part);
                } else {
                    emit(Draw::Model(part));
                }
            }
        }
        if let Some((part, false)) = tile.ground_decor {
            if has_ground {
                emit(Draw::Model(part));
            } else {
                cull.culled(part);
            }
        }
        if let Some(stack) = tile.obj_stack.filter(|stack| !stack.raised) {
            for part in stack.parts.into_iter().flatten() {
                if has_ground {
                    emit(Draw::Model(part));
                } else {
                    cull.culled(part);
                }
            }
        }
        let interior = tile.all_interior;
        let tile = &mut self.tiles[index];
        tile.wall_draw = wall_draw;
        (
            tile.scenery_draw,
            tile.scenery_drawn_dir,
            tile.scenery_skip_dir,
        ) = scenery_flags;
        if interior != 0 {
            if x < frame.tile[0] && interior & 4 != 0 {
                self.push_pending(draw_level, x + 1, y);
            }
            if y < frame.tile[1] && interior & 2 != 0 {
                self.push_pending(draw_level, x, y + 1);
            }
            if x > frame.tile[0] && interior & 1 != 0 {
                self.push_pending(draw_level, x - 1, y);
            }
            if y > frame.tile[1] && interior & 8 != 0 {
                self.push_pending(draw_level, x, y - 1);
            }
        }
    }

    /// Draw the tile's scenery whose every tile is ready, farthest first.
    /// None when a covered tile does not exist.
    fn draw_scenery(
        &mut self,
        index: usize,
        frame: &Frame,
        cull: &mut impl Occlusion,
        emit: &mut impl FnMut(Draw),
    ) -> Option<()> {
        let (x, y, level, own) = {
            let tile = &self.tiles[index];
            (tile.x, tile.y, tile.draw_level, tile.level)
        };
        let skip = self.tiles[index].scenery_skip_dir;
        let list: Vec<u32> = self.tiles[index].scenery.iter().map(|&(s, _)| s).collect();
        self.tiles[index].updated = false;
        let mut ready = Vec::with_capacity(list.len());
        'scenery: for scenery_index in list {
            let scenery = self.scenery[scenery_index as usize];
            if scenery.cycle == self.cycle {
                continue;
            }
            for sx in scenery.min[0]..=scenery.max[0] {
                for sy in scenery.min[1]..=scenery.max[1] {
                    let covered = &self.tiles[self.at(level, sx, sy)?];
                    if covered.visible {
                        self.tiles[index].updated = true;
                        continue 'scenery;
                    }
                    if covered.scenery_draw != 0 {
                        let mut side = 0;
                        if sx > scenery.min[0] {
                            side += 1;
                        }
                        if sx < scenery.max[0] {
                            side += 4;
                        }
                        if sy > scenery.min[1] {
                            side += 8;
                        }
                        if sy < scenery.max[1] {
                            side += 2;
                        }
                        if side & covered.scenery_draw == skip {
                            self.tiles[index].updated = true;
                            continue 'scenery;
                        }
                    }
                }
            }
            ready.push(scenery_index);
            let across = (frame.tile[0] - scenery.min[0]).max(scenery.max[0] - frame.tile[0]);
            let south = frame.tile[1] - scenery.min[1];
            let north = scenery.max[1] - frame.tile[1];
            self.scenery[scenery_index as usize].priority = if north > south {
                across + north
            } else {
                across + south
            };
        }
        let distance = |scenery: &Scenery| {
            let dx = scenery.fine[0].wrapping_sub(frame.eye[0]);
            let dy = scenery.fine[1].wrapping_sub(frame.eye[1]);
            dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy))
        };
        while !ready.is_empty() {
            let mut priority = -50;
            let mut best: Option<u32> = None;
            for &candidate_index in &ready {
                let candidate = &self.scenery[candidate_index as usize];
                if candidate.cycle == self.cycle {
                    continue;
                }
                if candidate.priority > priority {
                    priority = candidate.priority;
                    best = Some(candidate_index);
                } else if candidate.priority == priority {
                    // An entry at priority -50 has nothing to be compared with.
                    let current = &self.scenery[best? as usize];
                    if distance(candidate) > distance(current) {
                        best = Some(candidate_index);
                    }
                }
            }
            let Some(best) = best else {
                break;
            };
            self.scenery[best as usize].cycle = self.cycle;
            let selected = self.scenery[best as usize];
            if cull.area(own, selected.min, selected.max, selected.part) {
                cull.culled(selected.part);
            } else {
                emit(Draw::Model(selected.part));
            }
            for sx in selected.min[0]..=selected.max[0] {
                for sy in selected.min[1]..=selected.max[1] {
                    let covered = self.at(level, sx, sy)?;
                    if self.tiles[covered].scenery_draw != 0
                        || ((sx != x || sy != y) && self.tiles[covered].pending)
                    {
                        self.push(covered);
                    }
                }
            }
        }
        Some(())
    }
}

/// The diagonal wall decoration's camera test: the camera's offset from the
/// tile centre, mirrored by the decoration's orientation.
fn decor_sides(decor: &WallDecor, tile: &Tile, frame: &Frame) -> (i32, i32) {
    let dx = (tile.x * 128 + 64).wrapping_sub(frame.eye[0]);
    let dy = (tile.y * 128 + 64).wrapping_sub(frame.eye[1]);
    (
        if decor.orientation == 1 || decor.orientation == 2 {
            dx.wrapping_neg()
        } else {
            dx
        },
        if decor.orientation == 2 || decor.orientation == 3 {
            dy.wrapping_neg()
        } else {
            dy
        },
    )
}

/// `SceneGraph.setPlainTile`'s frustum test: which tile columns can be seen.
#[derive(Clone, Debug)]
pub struct Frustum {
    tile: [i32; 2],
    visibility: i32,
    tiles: Vec<bool>,
}

impl Frustum {
    /// `ground` gives the height of a scene tile corner on level 0 and
    /// `top` on level 3.
    pub fn new(
        view: &crate::picking::View,
        visibility: i32,
        ground: impl Fn(i32, i32) -> i32,
        top: impl Fn(i32, i32) -> i32,
    ) -> Self {
        let eye = [view.eye[0], view.eye[2]].map(|n| n.clamp(0, SIZE * 128 - 1));
        let tile = eye.map(|n| n / 128);
        let side = (visibility * 2 + 2) as usize;
        let mut points = vec![false; side * side];
        for vx in 0..side as i32 {
            for vy in 0..side as i32 {
                let (x, y) = (tile[0] + vx - visibility, tile[1] + vy - visibility);
                if !(0..SIZE).contains(&x) || !(0..SIZE).contains(&y) {
                    continue;
                }
                let dx = ((vx - visibility) << 7) - (eye[0] & 0x7f);
                let dy = ((vy - visibility) << 7) - (eye[1] & 0x7f);
                let high = ground(x, y).wrapping_add(128).wrapping_sub(view.eye[1]);
                let low = top(x, y).wrapping_sub(view.eye[1]).wrapping_sub(1000);
                points[vx as usize * side + vy as usize] =
                    project(view, dx, low, high, dy, crate::picking::VIEW_DISTANCE);
            }
        }
        let width = side - 1;
        let tiles = (0..width * width)
            .map(|i| {
                let (vx, vy) = (i / width, i % width);
                points[vx * side + vy]
                    || points[(vx + 1) * side + vy]
                    || points[vx * side + vy + 1]
                    || points[(vx + 1) * side + vy + 1]
            })
            .collect();
        Self {
            tile,
            visibility,
            tiles,
        }
    }

    /// `visibleTiles` for a scene tile.
    pub fn contains(&self, x: i32, y: i32) -> bool {
        let width = self.visibility * 2 + 1;
        let (vx, vy) = (
            x + self.visibility - self.tile[0],
            y + self.visibility - self.tile[1],
        );
        (0..width).contains(&vx)
            && (0..width).contains(&vy)
            && self.tiles[(vx * width + vy) as usize]
    }
}

/// `SceneGraph.projectToScreen`.
fn project(
    view: &crate::picking::View,
    x: i32,
    near: i32,
    far: i32,
    y: i32,
    max_depth: i32,
) -> bool {
    let mix =
        |a: i32, b: i32, c: i32, d: i32| a.wrapping_mul(b).wrapping_add(c.wrapping_mul(d)) >> 16;
    let across = mix(y, view.sin_yaw, x, view.cos_yaw);
    let ahead = mix(y, view.cos_yaw, x.wrapping_neg(), view.sin_yaw);
    let screen = |height: i32| {
        let depth = mix(height, view.sin_pitch, ahead, view.cos_pitch).max(1);
        let down = mix(height, view.cos_pitch, ahead.wrapping_neg(), view.sin_pitch);
        (
            depth,
            across.wrapping_shl(9) / depth,
            down.wrapping_shl(9) / depth,
        )
    };
    let (near_depth, near_x, near_y) = screen(near);
    let (far_depth, far_x, far_y) = screen(far);
    if (near_depth < 50 && far_depth < 50)
        || (near_depth > max_depth && far_depth > max_depth)
        || (near_x < view.lower[0] && far_x < view.lower[0])
        || (near_x > view.upper[0] && far_x > view.upper[0])
        || (near_y < view.lower[1] && far_y < view.lower[1])
    {
        return false;
    }
    near_y <= view.upper[1] || far_y <= view.upper[1]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drawn(scene: &mut Scene, eye: [i32; 2], visibility: i32) -> Vec<Draw> {
        let mut out = Vec::new();
        scene.draw(eye, visibility, |_, _, _| true, Some(0), |d| out.push(d));
        out
    }

    fn models(draws: &[Draw]) -> Vec<u32> {
        draws
            .iter()
            .filter_map(|d| match d {
                Draw::Model(part) => Some(*part),
                Draw::Ground { .. } => None,
            })
            .collect()
    }

    fn flat(scene: &mut Scene) {
        for x in 0..SIZE {
            for y in 0..SIZE {
                scene.set_ground(0, x, y, Ground::Plain { coloured: true });
            }
        }
    }

    #[test]
    fn tiles_are_drawn_from_far_to_near_and_each_once() {
        let mut scene = Scene::new();
        flat(&mut scene);
        let draws = drawn(&mut scene, [50 * 128 + 64, 50 * 128 + 64], 3);
        // The window is the 6 by 6 tiles from camera - 3 to camera + 2.
        assert_eq!(draws.len(), 36);
        let position = |x: u8, y: u8| {
            draws
                .iter()
                .position(|d| {
                    *d == Draw::Ground {
                        level: 0,
                        tile: [x, y],
                    }
                })
                .unwrap()
        };
        assert_eq!(position(47, 47), 0);
        assert_eq!(position(50, 50), 35);
        for x in 47..50 {
            assert!(position(x, 50) < position(x + 1, 50));
            assert!(position(50, x) < position(50, x + 1));
        }
        assert!(position(52, 50) < position(51, 50));
    }

    #[test]
    fn scenery_waits_for_all_its_tiles_and_far_scenery_comes_first() {
        let mut scene = Scene::new();
        flat(&mut scene);
        // A 2x2 object north-west of the camera and two single-tile objects.
        assert!(scene.add_scenery(0, [48, 51], [2, 2], [49 * 128, 52 * 128], 7, true, false));
        assert!(scene.add_scenery(
            0,
            [50, 52],
            [1, 1],
            [50 * 128 + 64, 52 * 128 + 64],
            8,
            true,
            false
        ));
        assert!(scene.add_scenery(
            0,
            [50, 51],
            [1, 1],
            [50 * 128 + 64, 51 * 128 + 64],
            9,
            true,
            false
        ));
        let draws = drawn(&mut scene, [50 * 128 + 64, 50 * 128 + 64], 4);
        // In the camera's own column the far tile's object precedes the near one.
        let order = models(&draws);
        assert_eq!(order.len(), 3);
        let place = |part| order.iter().position(|&p| p == part).unwrap();
        assert!(place(8) < place(9));
        let ground = |x, y| {
            draws
                .iter()
                .position(|d| {
                    *d == Draw::Ground {
                        level: 0,
                        tile: [x, y],
                    }
                })
                .unwrap()
        };
        let big = draws.iter().position(|d| *d == Draw::Model(7)).unwrap();
        for (x, y) in [(48, 51), (49, 51), (48, 52), (49, 52)] {
            assert!(ground(x, y) < big);
        }
    }

    #[test]
    fn a_tile_holds_five_pieces_of_scenery_and_temporary_ones_leave() {
        let mut scene = Scene::new();
        flat(&mut scene);
        for part in 0..5 {
            assert!(scene.add_actor(0, [6464, 6464], 60, part, 0, false));
        }
        assert!(!scene.add_actor(0, [6464, 6464], 60, 5, 0, false));
        assert!(!scene.add_scenery(0, [103, 103], [2, 1], [0, 0], 6, true, false));
        assert_eq!(scene.parts_at(0, 50, 50), [0, 1, 2, 3, 4]);
        // Equal priority and distance: the first placed is drawn first.
        assert_eq!(models(&drawn(&mut scene, [6464, 6000], 5)), [0, 1, 2, 3, 4]);
        scene.clear_temporary();
        assert!(scene.parts_at(0, 50, 50).is_empty());
        // A radius reaching the next tile makes the actor stand on both.
        assert!(scene.add_actor(0, [6464 + 60, 6464], 60, 9, 0, false));
        assert_eq!(scene.parts_at(0, 51, 50), [9]);
        assert_eq!(scene.parts_at(0, 50, 50), [9]);
        // A stretching sequence adds the tile the actor faces.
        scene.clear_temporary();
        assert!(scene.add_actor(0, [6464, 6464], 60, 3, 1024, true));
        assert_eq!(scene.parts_at(0, 50, 51), [3]);
        assert!(scene.parts_at(0, 50, 49).is_empty());
    }

    #[test]
    fn walls_facing_the_camera_come_before_the_tile_contents() {
        let mut scene = Scene::new();
        flat(&mut scene);
        // A west wall (side 1) on a tile north-east of the camera faces it; an
        // east wall (side 4) there is on the far side.
        scene.set_wall(0, 52, 52, 1, None, WALL_SIDES[0], 0);
        scene.set_ground_decor(0, 52, 52, 2, false);
        scene.set_obj_stack(0, 52, 52, [Some(3), None, None], false);
        scene.set_wall(0, 53, 53, 4, None, WALL_SIDES[2], 0);
        scene.set_ground_decor(0, 53, 53, 5, false);
        let draws = models(&drawn(&mut scene, [50 * 128 + 64, 50 * 128 + 64], 5));
        // Tile 53,53 is farther: its far wall first, then its decoration.
        assert_eq!(draws, [4, 5, 2, 3, 1]);
    }

    /// Nothing hidden; remembers what the walk says it left out.
    struct Notes(Vec<u32>);
    impl Occlusion for Notes {
        fn tile(&mut self, _: usize, _: i32, _: i32) -> bool {
            false
        }
        fn wall(&mut self, _: usize, _: i32, _: i32, _: i32) -> bool {
            false
        }
        fn decor(&mut self, _: usize, _: i32, _: i32, _: u32) -> bool {
            false
        }
        fn area(&mut self, _: usize, _: [i32; 2], _: [i32; 2], _: u32) -> bool {
            false
        }
        fn culled(&mut self, part: u32) {
            self.0.push(part);
        }
    }

    /// `WALL_VISIBILITY_FLAGS`, `WALL_DRAW_FLAGS` and `WALL_UNCULL_FLAGS`
    /// decide when a wall piece or a wall decoration is drawn (before or
    /// after what stands on its tile), not whether: from each of the nine
    /// places the camera's tile can be relative to the wall's, every piece is
    /// drawn exactly once. The one exception is a diagonal decoration on the
    /// camera's own tile, of whose two models only one is drawn.
    #[test]
    fn every_wall_piece_is_drawn_once_from_every_side() {
        let sides = WALL_SIDES;
        let all: Vec<i32> = sides.iter().chain(&WALL_CORNERS).copied().collect();
        for dx in -1..=1 {
            for dy in -1..=1 {
                let eye = [(50 + dx) * 128 + 40, (50 + dy) * 128 + 90];
                let own = dx == 0 && dy == 0;
                let check = |scene: &mut Scene, expected: &[u32], left: usize, what: &str| {
                    let mut notes = Notes(Vec::new());
                    let mut out = Vec::new();
                    scene.draw_occluded(eye, 6, |_, _, _| true, None, &mut notes, |d| out.push(d));
                    let mut drawn = models(&out);
                    drawn.sort_unstable();
                    assert_eq!(drawn.len() + left, expected.len(), "{what} from {dx},{dy}");
                    assert!(drawn.iter().all(|part| expected.contains(part)), "{what}");
                    drawn.dedup();
                    assert_eq!(
                        drawn.len() + left,
                        expected.len(),
                        "{what} twice from {dx},{dy}"
                    );
                    assert_eq!(notes.0.len(), left, "{what} from {dx},{dy}");
                };
                for &flags in &all {
                    // A wall piece by itself, and with scenery on its tile
                    // that a corner post waits for.
                    for scenery in [false, true] {
                        let mut scene = Scene::new();
                        flat(&mut scene);
                        scene.set_wall(0, 50, 50, 1, None, flags, 0);
                        let mut expected = vec![1];
                        if scenery {
                            assert!(scene.add_scenery(
                                0,
                                [49, 49],
                                [3, 3],
                                [6464; 2],
                                2,
                                true,
                                false
                            ));
                            expected.push(2);
                        }
                        check(&mut scene, &expected, 0, &format!("wall {flags}"));
                    }
                    for &second in &sides {
                        let mut scene = Scene::new();
                        flat(&mut scene);
                        scene.set_wall(0, 50, 50, 1, Some(2), flags, second);
                        check(&mut scene, &[1, 2], 0, &format!("walls {flags} {second}"));
                    }
                }
                for &flags in &sides {
                    let mut scene = Scene::new();
                    flat(&mut scene);
                    scene.set_wall_decor(0, 50, 50, 1, None, flags, 0);
                    check(&mut scene, &[1], 0, &format!("decoration {flags}"));
                }
                for orientation in 0..4 {
                    let mut scene = Scene::new();
                    flat(&mut scene);
                    scene.set_wall_decor(0, 50, 50, 1, Some(2), 256, orientation);
                    let left = usize::from(own);
                    check(
                        &mut scene,
                        &[1, 2],
                        left,
                        &format!("diagonal {orientation}"),
                    );
                }
            }
        }
    }

    #[test]
    fn a_bridge_moves_the_upper_tile_down_and_keeps_the_water_below() {
        let mut scene = Scene::new();
        flat(&mut scene);
        scene.set_ground(1, 50, 52, Ground::Shaped);
        assert!(scene.add_scenery(1, [50, 52], [1, 1], [6464, 6720], 11, true, false));
        assert!(scene.add_scenery(0, [50, 52], [1, 1], [6464, 6720], 12, true, false));
        scene.bridge(50, 52);
        assert_eq!(scene.parts_at(0, 50, 52), [11]);
        assert!(scene.parts_at(1, 50, 52).is_empty());
        let draws = drawn(&mut scene, [6464, 6464], 4);
        let at = |d: Draw| draws.iter().position(|x| *x == d).unwrap();
        // The old ground and what stood on it, then the deck with level 1 heights.
        let water = at(Draw::Ground {
            level: 0,
            tile: [50, 52],
        });
        let deck = at(Draw::Ground {
            level: 1,
            tile: [50, 52],
        });
        assert!(water < at(Draw::Model(12)));
        assert!(at(Draw::Model(12)) < deck);
        assert!(deck < at(Draw::Model(11)));
    }

    #[test]
    fn hidden_tiles_and_their_contents_are_not_drawn() {
        let mut scene = Scene::new();
        flat(&mut scene);
        scene.set_ground(1, 51, 51, Ground::Plain { coloured: true });
        scene.set_ground_decor(1, 51, 51, 4, false);
        scene.set_ground_decor(0, 51, 51, 5, false);
        let mut out = Vec::new();
        scene.draw(
            [6464, 6464],
            4,
            |level, _, _| level == 0,
            Some(0),
            |d| out.push(d),
        );
        assert_eq!(models(&out), [5]);
        assert_eq!(models(&drawn(&mut scene, [6464, 6464], 4)), [5, 4]);
        // A colourless plain tile is drawn only up to the walker's level.
        scene.set_ground(1, 51, 51, Ground::Plain { coloured: false });
        let draws = drawn(&mut scene, [6464, 6464], 4);
        assert!(!draws.contains(&Draw::Ground {
            level: 1,
            tile: [51, 51]
        }));
        assert_eq!(models(&draws), [5, 4]);
    }

    #[test]
    fn the_frustum_keeps_what_is_ahead_of_the_camera() {
        let mut view = crate::picking::View::new([6464, -800, 6464], 200, 0);
        view.lower = [-256, -167];
        view.upper = [256, 167];
        let frustum = Frustum::new(&view, 25, |_, _| 0, |_, _| -720);
        assert!(frustum.contains(50, 56));
        assert!(!frustum.contains(50, 40));
        assert!(!frustum.contains(70, 52));
        assert!(!frustum.contains(50, 80));
    }
}
