//! Shared lighting between neighbouring scenery (location opcode 22):
//! `SceneGraph.mergeSceneNormals`, `mergeEntityNormals`,
//! `mergeGroundDecorNormals` and `GlModel.mergeNormals`/`createModel`, with
//! the tile slots `setWall`, `addSceneryEntity` and `setGroundDecor` fill and
//! the part of `GlModel.alignToTerrain` (hill skew type 1) that decides where
//! a model's vertices are when they are compared.
//!
//! Where two mergeable models have a vertex at the same place, each adds the
//! other's vertex normal to its own, so the lighting runs smoothly across
//! the seam. GPU independent.
use crate::model::Model;
use std::collections::HashMap;

/// A scene entity as merging reads it: the fields of a `GlModel`.
#[derive(Clone, Debug, Default)]
pub struct Body {
    /// Model-local vertices, relative to the entity's centre and base height.
    pub vertices: Vec<[i32; 3]>,
    /// `vertexLookup`: for each vertex, its packed vertices in the order
    /// the model created them.
    pub lookup: Vec<Vec<u32>>,
    /// How many drawn face corners use each vertex: the lookup slots
    /// `GlModel` reserves for it.
    pub uses: Vec<u32>,
    /// `normalX/Y/Z` and `normalMagnitude` of each packed vertex.
    pub normals: Vec<[i16; 4]>,
    /// `triangleCount`.
    pub triangles: usize,
    /// `mergeable`: the type has opcode 22 and the entity is a static model.
    pub mergeable: bool,
    /// `GlModelRecoloring`: the running sums, created on the first merge.
    sums: Option<Vec<[i16; 4]>>,
    bounds: Option<[[i16; 3]; 2]>,
}
impl Body {
    /// An entity that occupies its slot but never merges: an animated or
    /// multi-location `Loc`, or a model whose type lacks opcode 22.
    pub fn solid() -> Self {
        Self::default()
    }
    /// The packed vertices of a prepared, placed model. `faces` lists the
    /// drawn faces; the result also maps each of their corners to its packed
    /// vertex.
    ///
    /// `surface::prepare` numbered the packed vertices as `GlModel` creates
    /// them. Mirroring a placed model swaps the first and last corner of
    /// every face without the numbers following, so the corner order is
    /// taken as whichever of the two keeps each number on a single vertex.
    pub fn from_model(model: &Model, faces: &[usize], mergeable: bool) -> (Self, Vec<[u32; 3]>) {
        let consistent = |swapped: bool| {
            let mut owner = HashMap::new();
            faces.iter().all(|&face| {
                (0..3).all(|corner| {
                    let id = model.surfaces[face].packed[if swapped { 2 - corner } else { corner }];
                    *owner.entry(id).or_insert(model.faces[face][corner])
                        == model.faces[face][corner]
                })
            })
        };
        let swapped = !consistent(false) && consistent(true);
        let mut body = Self {
            vertices: model.vertices.clone(),
            lookup: vec![Vec::new(); model.vertices.len()],
            uses: vec![0; model.vertices.len()],
            triangles: faces.len(),
            mergeable,
            ..Self::default()
        };
        // Creation order is ascending number; drawn faces need not use all.
        let mut numbers = std::collections::BTreeMap::new();
        for &face in faces {
            for corner in 0..3 {
                let from = if swapped { 2 - corner } else { corner };
                let vertex = model.faces[face][corner];
                body.uses[vertex] += 1;
                numbers
                    .entry(model.surfaces[face].packed[from])
                    .or_insert((vertex, model.surfaces[face].normals[corner]));
            }
        }
        let mut index = HashMap::new();
        for (&number, &(vertex, normal)) in &numbers {
            index.insert(number, body.normals.len() as u32);
            body.lookup[vertex].push(body.normals.len() as u32);
            body.normals.push(normal.map(|n| n as i16));
        }
        let corners = faces
            .iter()
            .map(|&face| {
                std::array::from_fn(|corner| {
                    index[&model.surfaces[face].packed[if swapped { 2 - corner } else { corner }]]
                })
            })
            .collect();
        (body, corners)
    }
    /// `GlModel.calculateBounds`, as far as merging and alignment read it.
    fn bounds(&mut self) -> [[i16; 3]; 2] {
        *self.bounds.get_or_insert_with(|| {
            let mut min = [32767i32; 3];
            let mut max = [-32768i32; 3];
            for v in &self.vertices {
                for axis in 0..3 {
                    min[axis] = min[axis].min(v[axis]);
                    max[axis] = max[axis].max(v[axis]);
                }
            }
            [min.map(|v| v as i16), max.map(|v| v as i16)]
        })
    }
    /// `GlModel.alignToTerrain` for hill skew type 1: every vertex follows
    /// the ground under it. `heights` reads a corner of the entity's level
    /// and `size` is the corner grid (105 x 105 in the original); `fine` is
    /// the entity's centre and `base` its averaged ground height.
    pub fn align(
        &mut self,
        heights: impl Fn(i32, i32) -> i32,
        size: [i32; 2],
        fine: [i32; 2],
        base: i32,
    ) {
        let [low, high] = self.bounds();
        let min_x = fine[0] + i32::from(low[0]);
        let max_x = fine[0] + i32::from(high[0]);
        let min_z = fine[1] + i32::from(low[2]);
        let max_z = fine[1] + i32::from(high[2]);
        if min_x < 0 || (max_x + 128) >> 7 >= size[0] || min_z < 0 || (max_z + 128) >> 7 >= size[1]
        {
            return;
        }
        let (x0, x1, z0, z1) = (
            min_x >> 7,
            (max_x + 127) >> 7,
            min_z >> 7,
            (max_z + 127) >> 7,
        );
        if [(x0, z0), (x1, z0), (x0, z1), (x1, z1)]
            .iter()
            .all(|&(x, z)| heights(x, z) == base)
        {
            return;
        }
        for v in &mut self.vertices {
            let wx = v[0] + fine[0];
            let wz = v[2] + fine[1];
            let (fx, fz) = (wx & 0x7f, wz & 0x7f);
            let (tx, tz) = (wx >> 7, wz >> 7);
            let near = (heights(tx, tz) * (128 - fx) + heights(tx + 1, tz) * fx) >> 7;
            let far = (heights(tx, tz + 1) * (128 - fx) + heights(tx + 1, tz + 1) * fx) >> 7;
            v[1] = v[1] + ((near * (128 - fz) + far * fz) >> 7) - base;
        }
        self.bounds = None;
    }
    fn can_merge(&self) -> bool {
        self.mergeable
    }
    /// The packed vertex whose normal stands for a vertex: the first with a
    /// face count. When all are flat it is the last one, but only if the
    /// vertex's lookup slots are full; otherwise the scan runs into an empty
    /// slot and the vertex is skipped.
    fn representative(&self, vertex: usize) -> Option<usize> {
        let lookup = &self.lookup[vertex];
        lookup
            .iter()
            .find(|&&index| self.normals[index as usize][3] != 0)
            .or_else(|| {
                lookup
                    .last()
                    .filter(|_| lookup.len() as u32 >= self.uses[vertex])
            })
            .map(|&index| index as usize)
    }
    /// `GlModel.mergeNormals`. `offset` is where `other` stands relative to
    /// this entity.
    fn merge(&mut self, other: &mut Body, offset: [i32; 3]) {
        if self.triangles == 0 || other.triangles == 0 {
            return;
        }
        let [low, high] = other.bounds();
        for i in 0..self.vertices.len() {
            let v = self.vertices[i];
            let d: [i32; 3] = std::array::from_fn(|axis| v[axis] - offset[axis]);
            // The original tests y, then x, then z.
            if (0..3).any(|a| d[a] < i32::from(low[a]) || d[a] > i32::from(high[a])) {
                continue;
            }
            let Some(mine) = self.representative(i) else {
                continue;
            };
            for j in 0..other.vertices.len() {
                if other.vertices[j] != d {
                    continue;
                }
                let Some(theirs) = other.representative(j) else {
                    continue;
                };
                let add = |sums: &mut Option<Vec<[i16; 4]>>,
                           normals: &[[i16; 4]],
                           lookup: &[u32],
                           n: [i16; 4]| {
                    let sums = sums.get_or_insert_with(|| normals.to_vec());
                    for &index in lookup {
                        let sum = &mut sums[index as usize];
                        if sum[3] != 0 {
                            for axis in 0..4 {
                                sum[axis] = sum[axis].wrapping_add(n[axis]);
                            }
                        }
                    }
                };
                // Both sums exist before either is added to.
                self.sums.get_or_insert_with(|| self.normals.clone());
                add(
                    &mut other.sums,
                    &other.normals,
                    &other.lookup[j],
                    self.normals[mine],
                );
                add(
                    &mut self.sums,
                    &self.normals,
                    &self.lookup[i],
                    other.normals[theirs],
                );
            }
        }
    }
    /// `GlModel.createModel`: the sums become the normals and the entity
    /// stops taking part.
    fn finish(&mut self) {
        self.mergeable = false;
        if let Some(sums) = self.sums.take() {
            self.normals = sums;
        }
    }
}

pub type Entity = usize;

#[derive(Clone, Debug)]
struct Scenery {
    entity: Entity,
    min: [i32; 2],
    max: [i32; 2],
}
#[derive(Clone, Debug, Default)]
struct Tile {
    wall: Option<[Option<Entity>; 2]>,
    scenery: Vec<Scenery>,
    ground: Option<Entity>,
}

/// The entity slots of a scene's tiles and the entities in them.
pub struct Scene {
    width: i32,
    depth: i32,
    tiles: Vec<Tile>,
    pub bodies: Vec<Body>,
}
impl Scene {
    /// The original scene is 104 x 104 tiles on four levels.
    pub fn new(width: usize, depth: usize) -> Self {
        Self {
            width: width as i32,
            depth: depth as i32,
            tiles: vec![Tile::default(); 4 * width * depth],
            bodies: Vec::new(),
        }
    }
    pub fn add(&mut self, body: Body) -> Entity {
        self.bodies.push(body);
        self.bodies.len() - 1
    }
    fn index(&self, level: usize, x: i32, y: i32) -> Option<usize> {
        (level < 4 && x >= 0 && y >= 0 && x < self.width && y < self.depth)
            .then(|| (level * self.width as usize + x as usize) * self.depth as usize + y as usize)
    }
    /// `setWall`: the last wall placed on a tile is the one it keeps.
    pub fn set_wall(
        &mut self,
        level: usize,
        x: i32,
        y: i32,
        primary: Option<Entity>,
        secondary: Option<Entity>,
    ) {
        if primary.is_none() && secondary.is_none() {
            return;
        }
        if let Some(index) = self.index(level, x, y) {
            self.tiles[index].wall = Some([primary, secondary]);
        }
    }
    /// `setGroundDecor`.
    pub fn set_ground_decor(&mut self, level: usize, x: i32, y: i32, entity: Option<Entity>) {
        if let (Some(entity), Some(index)) = (entity, self.index(level, x, y)) {
            self.tiles[index].ground = Some(entity);
        }
    }
    /// `addEntity`/`addSceneryEntity`: fails when the footprint leaves the
    /// scene or a tile under it already holds five entities.
    pub fn add_scenery(
        &mut self,
        level: usize,
        x: i32,
        y: i32,
        size: [i32; 2],
        entity: Option<Entity>,
    ) -> bool {
        let Some(entity) = entity else {
            return true;
        };
        let tiles = || (x..x + size[0]).flat_map(|tx| (y..y + size[1]).map(move |ty| (tx, ty)));
        for (tx, ty) in tiles() {
            match self.index(level, tx, ty) {
                Some(index) if self.tiles[index].scenery.len() < 5 => {}
                _ => return false,
            }
        }
        for (tx, ty) in tiles() {
            let index = self.index(level, tx, ty).unwrap();
            self.tiles[index].scenery.push(Scenery {
                entity,
                min: [x, y],
                max: [x + size[0] - 1, y + size[1] - 1],
            });
        }
        true
    }
    /// The slot `addLoc` gives a location of `shape`. `size` is the type's
    /// footprint, swapped here for rotations 1 and 3. Wall decorations
    /// (shapes 4 to 8) never merge and are not kept. Returns whether a
    /// centrepiece (shapes 10 and 11) with an entity was added.
    pub fn place(
        &mut self,
        level: usize,
        [x, y]: [i32; 2],
        shape: u8,
        rotation: u8,
        mut size: [i32; 2],
        pieces: [Option<Entity>; 2],
    ) -> bool {
        if rotation & 1 != 0 {
            size.swap(0, 1);
        }
        match shape {
            22 => self.set_ground_decor(level, x, y, pieces[0]),
            10 | 11 => {
                return pieces[0].is_some() && self.add_scenery(level, x, y, size, pieces[0]);
            }
            12.. => {
                self.add_scenery(level, x, y, [1, 1], pieces[0]);
            }
            0 | 1 | 3 => self.set_wall(level, x, y, pieces[0], None),
            2 => self.set_wall(level, x, y, pieces[0], pieces[1]),
            9 => {
                self.add_scenery(level, x, y, [1, 1], pieces[0]);
            }
            _ => {}
        }
        false
    }
    fn pair(&mut self, a: Entity, b: Entity) -> Option<(&mut Body, &mut Body)> {
        if a == b {
            return None;
        }
        let (low, high) = (a.min(b), a.max(b));
        let (head, tail) = self.bodies.split_at_mut(high);
        let (low, high) = (&mut head[low], &mut tail[0]);
        Some(if a < b { (low, high) } else { (high, low) })
    }
    fn merge_pair(&mut self, entity: Entity, other: Entity, offset: [i32; 3]) {
        if !self.bodies[other].can_merge() {
            return;
        }
        if let Some((a, b)) = self.pair(entity, other) {
            a.merge(b, offset);
        }
    }
    /// `mergeEntityNormals`.
    fn merge_entity(
        &mut self,
        heights: &impl Fn(usize, i32, i32) -> i32,
        entity: Entity,
        level: usize,
        [x, y]: [i32; 2],
        [x_size, y_size]: [i32; 2],
    ) {
        let average = |level: usize, x: i32, y: i32| {
            (heights(level, x, y)
                + heights(level, x + 1, y)
                + heights(level, x, y + 1)
                + heights(level, x + 1, y + 1))
                / 4
        };
        let mut same_level = true;
        let mut start_x = x;
        let end_x = x + x_size;
        let start_y = y - 1;
        let end_y = y + y_size;
        for lvl in level..=level + 1 {
            if lvl == 4 {
                continue;
            }
            for tx in start_x..=end_x {
                for ty in start_y..=end_y {
                    if !(!same_level || tx >= end_x || ty >= end_y || ty < y && tx != x) {
                        continue;
                    }
                    let Some(index) = self.index(lvl, tx, ty) else {
                        continue;
                    };
                    let tile = self.tiles[index].clone();
                    if tile.wall.is_none() && tile.scenery.is_empty() {
                        continue;
                    }
                    let lift = average(lvl, tx, ty) - average(level, x, y);
                    if let Some(wall) = tile.wall {
                        let offset = [
                            (tx - x) * 128 + (1 - x_size) * 64,
                            lift,
                            (ty - y) * 128 + (1 - y_size) * 64,
                        ];
                        for piece in wall.into_iter().flatten() {
                            self.merge_pair(entity, piece, offset);
                        }
                    }
                    for s in &tile.scenery {
                        if (tx == s.min[0] || tx == start_x) && (ty == s.min[1] || ty == start_y) {
                            let width = s.max[0] + 1 - s.min[0];
                            let length = s.max[1] + 1 - s.min[1];
                            let offset = [
                                (s.min[0] - x) * 128 + (width - x_size) * 64,
                                lift,
                                (s.min[1] - y) * 128 + (length - y_size) * 64,
                            ];
                            self.merge_pair(entity, s.entity, offset);
                        }
                    }
                }
            }
            start_x -= 1;
            same_level = false;
        }
    }
    /// `mergeSceneNormals`: every mergeable entity, in tile order, merges
    /// with the mergeable entities around it that have not had their turn.
    /// `heights` reads `tileHeights[level][x][y]`.
    pub fn merge(&mut self, heights: impl Fn(usize, i32, i32) -> i32) {
        for level in 0..4 {
            for x in 0..self.width {
                for y in 0..self.depth {
                    let Some(index) = self.index(level, x, y) else {
                        continue;
                    };
                    let tile = self.tiles[index].clone();
                    // A wall with only a secondary piece fails in the original.
                    if let Some([Some(primary), secondary]) = tile.wall {
                        if self.bodies[primary].can_merge() {
                            self.merge_entity(&heights, primary, level, [x, y], [1, 1]);
                            if let Some(secondary) = secondary {
                                if self.bodies[secondary].can_merge() {
                                    self.merge_entity(&heights, secondary, level, [x, y], [1, 1]);
                                    self.merge_pair(primary, secondary, [0; 3]);
                                    self.bodies[secondary].finish();
                                }
                            }
                            self.bodies[primary].finish();
                        }
                    }
                    for s in &tile.scenery {
                        if self.bodies[s.entity].can_merge() {
                            let size = [s.max[0] + 1 - s.min[0], s.max[1] - s.min[1] + 1];
                            self.merge_entity(&heights, s.entity, level, [x, y], size);
                            self.bodies[s.entity].finish();
                        }
                    }
                    if let Some(ground) = tile.ground {
                        if self.bodies[ground].can_merge() {
                            // mergeGroundDecorNormals: east, north, north-east
                            // and south-east neighbours on the same level.
                            for (dx, dy) in [(1, 0), (0, 1), (1, 1), (1, -1)] {
                                let neighbour = self
                                    .index(level, x + dx, y + dy)
                                    .and_then(|index| self.tiles[index].ground);
                                if let Some(neighbour) = neighbour {
                                    self.merge_pair(ground, neighbour, [dx * 128, 0, dy * 128]);
                                }
                            }
                            self.bodies[ground].finish();
                        }
                    }
                }
            }
        }
    }
}

/// What a tile holds, for inspection: the wall's two pieces, each scenery
/// entity with the first and last tile of its footprint, the ground decoration.
pub type Slots = (
    Option<[Option<Entity>; 2]>,
    Vec<(Entity, [i32; 2], [i32; 2])>,
    Option<Entity>,
);
impl Scene {
    pub fn slots(&self, level: usize, x: i32, y: i32) -> Slots {
        let Some(index) = self.index(level, x, y) else {
            return (None, Vec::new(), None);
        };
        let tile = &self.tiles[index];
        (
            tile.wall,
            tile.scenery
                .iter()
                .map(|s| (s.entity, s.min, s.max))
                .collect(),
            tile.ground,
        )
    }
}

/// The scene entity `addLoc` makes for one piece of a location.
#[derive(Clone, Debug, Default)]
pub enum Piece {
    /// No model: nothing is placed.
    #[default]
    None,
    /// An animated or multi-location entity (`Loc`).
    Animated,
    /// A static model.
    Model(Body),
}

/// One landscape placement as `addLoc` sees it.
pub struct Placed {
    pub level: usize,
    pub tile: [i32; 2],
    pub shape: u8,
    pub rotation: u8,
    /// The type's footprint, before the rotation swap.
    pub size: [i32; 2],
    /// `LocType.active`.
    pub active: bool,
    /// The wall's second piece exists only for shape 2.
    pub pieces: [Piece; 2],
}

/// `SceneGraph.addLoc` as far as the tile slots and the shadow map go: the
/// entities take their slots and an active wall or centrepiece shades the
/// floor. Returns the entities of the two pieces.
pub fn add_loc(
    scene: &mut Scene,
    shade: &mut crate::tile_shade::ShadowMap,
    placed: Placed,
) -> [Option<Entity>; 2] {
    use crate::tile_shade::{Caster, length_xz};
    let caster = match &placed.pieces[0] {
        Piece::None => None,
        Piece::Animated => Some(Caster::Animated),
        Piece::Model(body) => Some(Caster::Model(length_xz(&body.vertices))),
    };
    let entities = placed.pieces.map(|piece| match piece {
        Piece::None => None,
        Piece::Animated => Some(scene.add(Body::solid())),
        Piece::Model(body) => Some(scene.add(body)),
    });
    let [x, y] = placed.tile;
    let added = scene.place(
        placed.level,
        placed.tile,
        placed.shape,
        placed.rotation,
        placed.size,
        entities,
    );
    if placed.active {
        match (placed.shape, caster) {
            (0 | 1 | 3, _) => shade.wall(placed.level, x, y, placed.shape, placed.rotation),
            (10 | 11, Some(caster)) if added => {
                let mut size = placed.size;
                if placed.rotation & 1 != 0 {
                    size.swap(0, 1);
                }
                shade.centrepiece(placed.level, x, y, size, caster);
            }
            _ => {}
        }
    }
    entities
}

/// Stands a model on the ground as `LocType.getStaticEntity` does for a type
/// with hill skew 1. `heights` reads a corner of the location's level,
/// `grid` is the tile count per side (104) and `size` the type's footprint
/// before the rotation swap.
pub fn stand(
    body: &mut Body,
    heights: impl Fn(i32, i32) -> i32,
    grid: [i32; 2],
    [x, y]: [i32; 2],
    rotation: u8,
    mut size: [i32; 2],
) {
    if rotation & 1 != 0 {
        size.swap(0, 1);
    }
    let base = base_height(&heights, [x, y], size, grid);
    body.align(
        heights,
        [grid[0] + 1, grid[1] + 1],
        [(size[0] << 6) + (x << 7), (size[1] << 6) + (y << 7)],
        base,
    );
}

/// `addLoc`'s `averageY`: the ground height a location's model stands on.
/// `size` is the footprint after the rotation swap and `limit` the scene's
/// tile count per side (104).
pub fn base_height(
    heights: impl Fn(i32, i32) -> i32,
    [x, y]: [i32; 2],
    [width, length]: [i32; 2],
    limit: [i32; 2],
) -> i32 {
    let (west, east) = if x + width <= limit[0] {
        (x + (width >> 1), x + ((width + 1) >> 1))
    } else {
        (x, x + 1)
    };
    let (south, north) = if length + y > limit[1] {
        (y, y + 1)
    } else {
        ((length >> 1) + y, y + ((length + 1) >> 1))
    };
    (heights(west, north) + heights(east, south) + heights(west, south) + heights(east, north)) >> 2
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body(vertices: Vec<[i32; 3]>, normals: Vec<[i16; 4]>, lookup: Vec<Vec<u32>>) -> Body {
        Body {
            vertices,
            uses: lookup.iter().map(|l| l.len() as u32).collect(),
            lookup,
            normals,
            triangles: 1,
            mergeable: true,
            ..Body::default()
        }
    }

    // Transcription test: two walls on neighbouring tiles share the vertex
    // on their common edge; expected sums follow GlModel.mergeNormals by hand.
    #[test]
    fn neighbouring_walls_add_each_others_normal_at_shared_vertices() {
        let mut scene = Scene::new(8, 8);
        let west = scene.add(body(
            vec![[64, -100, 0], [-64, -100, 0]],
            vec![[10, 20, 30, 2], [1, 1, 1, 1], [7, 7, 7, 0]],
            vec![vec![2, 0], vec![1]],
        ));
        let east = scene.add(body(
            vec![[-64, -100, 0], [64, -100, 0]],
            vec![[-5, 4, 3, 1], [9, 9, 9, 3]],
            vec![vec![0], vec![1]],
        ));
        scene.place(0, [2, 2], 0, 0, [1, 1], [Some(west), None]);
        scene.place(0, [3, 2], 0, 0, [1, 1], [Some(east), None]);
        scene.merge(|_, _, _| 0);
        // The flat packed vertex (face count 0) is passed over as the
        // representative and receives nothing.
        assert_eq!(
            scene.bodies[west].normals,
            [[5, 24, 33, 3], [1, 1, 1, 1], [7, 7, 7, 0]]
        );
        assert_eq!(scene.bodies[east].normals, [[5, 24, 33, 3], [9, 9, 9, 3]]);
        assert!(!scene.bodies[west].mergeable && !scene.bodies[east].mergeable);
    }

    #[test]
    fn a_sixth_entity_on_a_tile_is_refused_and_walls_replace() {
        let mut scene = Scene::new(8, 8);
        let ids: Vec<_> = (0..7).map(|_| scene.add(Body::solid())).collect();
        for &id in &ids[..5] {
            assert!(scene.place(0, [1, 1], 10, 0, [1, 1], [Some(id), None]));
        }
        assert!(!scene.place(0, [0, 1], 10, 1, [1, 2], [Some(ids[5]), None]));
        assert!(!scene.place(0, [7, 7], 10, 0, [2, 1], [Some(ids[5]), None]));
        scene.place(0, [4, 4], 0, 0, [1, 1], [Some(ids[5]), None]);
        scene.place(0, [4, 4], 2, 0, [1, 1], [Some(ids[6]), None]);
        assert_eq!(
            scene.tiles[scene.index(0, 4, 4).unwrap()].wall,
            Some([Some(ids[6]), None])
        );
    }

    #[test]
    fn base_height_averages_the_footprint_centre() {
        let h = |x: i32, y: i32| -(x * 8 + y);
        assert_eq!(
            base_height(h, [2, 3], [1, 1], [104; 2]),
            (-19 - 28 - 27 - 20) >> 2
        );
        assert_eq!(
            base_height(h, [2, 3], [3, 2], [104; 2]),
            -((3 * 8 + 4) * 2 + (4 * 8 + 4) * 2) >> 2
        );
    }
}
