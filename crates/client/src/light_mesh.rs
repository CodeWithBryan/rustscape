//! The terrain meshes a point light is drawn with: RT4
//! `SceneGraph.buildGlLighting`, `applyLightToTile`, `interpolateHeight` and
//! `LightMesh.addVertex`/`addFan`.
//!
//! A light gets one mesh, on its own level only. Tiles without a visible floor
//! are left out, a tile whose overlay hides part of it is lit over the visible
//! part, and a tile holding a diagonal wall (location shape 9) is cut along
//! the wall to the side the light's footprint reaches.
use std::collections::HashMap;

/// `SceneGraph.UNDERLAY_SHAPE_VERTICES`, named as the original names it.
pub const UNDERLAY_SHAPE_VERTICES: [&[i32]; 12] = [
    &[0, 128, 0, 0, 128, 0, 128, 128],
    &[0, 128, 0, 0, 128, 0],
    &[0, 0, 64, 128, 0, 128],
    &[128, 128, 64, 128, 128, 0],
    &[0, 0, 128, 0, 128, 128, 64, 128],
    &[0, 128, 0, 0, 128, 0, 64, 128],
    &[64, 128, 0, 128, 0, 0, 64, 0],
    &[0, 0, 64, 0, 0, 64],
    &[128, 0, 128, 128, 0, 128, 0, 64, 64, 0],
    &[0, 128, 0, 0, 32, 64, 64, 96, 128, 128],
    &[0, 0, 128, 0, 128, 128, 64, 96, 32, 64],
    &[0, 0, 128, 0, 96, 32, 32, 32],
];
/// `SceneGraph.OVERLAY_SHAPE_VERTICES`.
pub const OVERLAY_SHAPE_VERTICES: [&[i32]; 12] = [
    &[],
    &[128, 0, 128, 128, 0, 128],
    &[0, 0, 128, 0, 128, 128, 64, 128],
    &[0, 128, 0, 0, 128, 0, 64, 128],
    &[0, 0, 64, 128, 0, 128],
    &[128, 128, 64, 128, 128, 0],
    &[64, 0, 128, 0, 128, 128, 64, 128],
    &[128, 0, 128, 128, 0, 128, 0, 64, 64, 0],
    &[0, 0, 64, 0, 0, 64],
    &[0, 0, 128, 0, 128, 128, 64, 96, 32, 64],
    &[0, 128, 0, 0, 32, 64, 64, 96, 128, 128],
    &[0, 128, 0, 0, 32, 32, 96, 32, 128, 0, 128, 128],
];

/// One level of a scene `size` tiles wide and long (104 in the original).
/// Tile arrays are indexed `x * size[1] + y`; `heights` has one more entry on
/// each axis and is indexed `x * (size[1] + 1) + y`.
pub struct Level<'a> {
    pub size: [usize; 2],
    pub underlays: &'a [u8],
    pub overlays: &'a [u8],
    pub shapes: &'a [u8],
    pub rotations: &'a [u8],
    pub heights: &'a [i32],
}
impl Level<'_> {
    fn tile(&self, x: i32, y: i32) -> usize {
        x as usize * self.size[1] + y as usize
    }
    fn height(&self, x: i32, y: i32) -> i32 {
        self.heights[x as usize * (self.size[1] + 1) + y as usize]
    }
    /// The corner normals `buildScene` computes for the level, in the order
    /// `LightMesh.addVertex` takes them. The outer ring stays zero.
    pub fn normals(&self) -> Vec<[f32; 3]> {
        let stride = self.size[1] + 1;
        let mut normals = vec![[0.0f32; 3]; (self.size[0] + 1) * stride];
        for y in 1..self.size[1] as i32 {
            for x in 1..self.size[0] as i32 {
                let z = self.height(x, y + 1).wrapping_sub(self.height(x, y - 1));
                let p = self.height(x + 1, y).wrapping_sub(self.height(x - 1, y));
                let length = f64::from(
                    p.wrapping_mul(p)
                        .wrapping_add(z.wrapping_mul(z))
                        .wrapping_add(65536),
                )
                .sqrt() as f32;
                normals[x as usize * stride + y as usize] =
                    [p as f32 / length, -256.0 / length, z as f32 / length];
            }
        }
        normals
    }
}

/// A placed light. `tile` is the footprint's centre (`x >> 7`, `y >> 7` of the
/// original fields) and `position` the three fields `addVertex` subtracts the
/// vertex's fine x, height and fine y from, in that order.
#[derive(Clone, Debug)]
pub struct Source<'a> {
    pub level: usize,
    pub tile: [i32; 2],
    pub position: [i32; 3],
    pub radius: i32,
    pub spans: &'a [i16],
    pub color: u32,
}

/// What `LightMesh.upload` hands to the graphics library: a colour and a
/// position (fine x, height, fine y) per vertex, and triangle indices.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mesh {
    pub colors: Vec<[u8; 3]>,
    pub positions: Vec<[i32; 3]>,
    pub indices: Vec<u32>,
}

struct Builder<'a> {
    level: &'a Level<'a>,
    normals: &'a [[f32; 3]],
    light: &'a Source<'a>,
    mesh: Mesh,
    cache: HashMap<i64, u32>,
}
impl Builder<'_> {
    fn normal(&self, x: i32, y: i32) -> [f32; 3] {
        self.normals[x as usize * (self.level.size[1] + 1) + y as usize]
    }
    /// `LightMesh.addVertex`.
    fn vertex(&mut self, x: i32, y: i32, z: i32, normal: [f32; 3]) -> u32 {
        let mut key = 0i64;
        if x & 0x7f == 0 || z & 0x7f == 0 {
            key = i64::from(x.wrapping_add(z.wrapping_shl(16)));
            if let Some(&index) = self.cache.get(&key) {
                return index;
            }
        }
        let light = self.light;
        let d = [
            light.position[0].wrapping_sub(x) as f32,
            light.position[1].wrapping_sub(y) as f32,
            light.position[2].wrapping_sub(z) as f32,
        ];
        let distance = f64::from(d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt() as f32;
        let inverse = 1.0 / distance;
        let direction = d.map(|v| v * inverse);
        let attenuation = distance / ((light.radius << 7) + 64) as f32;
        let mut falloff = 1.0 - attenuation * attenuation;
        if falloff < 0.0 {
            falloff = 0.0;
        }
        let mut dot =
            direction[0] * normal[0] + direction[1] * normal[1] + direction[2] * normal[2];
        if dot < 0.0 {
            dot = 0.0;
        }
        let mut intensity = dot * falloff * 2.0;
        if intensity > 1.0 {
            intensity = 1.0;
        }
        let channel = |shift: u32| {
            let value = (intensity * (light.color >> shift & 0xff) as f32) as i32;
            value.min(255) as u8
        };
        let index = self.mesh.positions.len() as u32;
        self.mesh.colors.push([channel(16), channel(8), channel(0)]);
        self.mesh.positions.push([x, y, z]);
        // The first vertex stored under a key answers later lookups. Interior
        // vertices are all stored under key 0, which only the scene's corner
        // (fine 0, 0) ever looks up.
        self.cache.entry(key).or_insert(index);
        index
    }
    /// `applyLightToTile`.
    fn tile(&mut self, x: i32, y: i32, vertices: &[i32], rotation: i32) {
        let mut fan = Vec::with_capacity(vertices.len() / 2);
        for pair in vertices.chunks_exact(2) {
            let (mut vx, mut vy) = (pair[0], pair[1]);
            match rotation {
                1 => (vx, vy) = (vy, 128 - vx),
                2 => (vx, vy) = (128 - vx, 128 - vy),
                3 => (vx, vy) = (128 - vy, vx),
                _ => {}
            }
            let normal = match (vx, vy) {
                (0, 0) => self.normal(x, y),
                (128, 0) => self.normal(x + 1, y),
                (128, 128) => self.normal(x + 1, y + 1),
                (0, 128) => self.normal(x, y + 1),
                _ => {
                    let (a, b) = (self.normal(x, y), self.normal(x + 1, y));
                    let (c, d) = (self.normal(x, y + 1), self.normal(x + 1, y + 1));
                    let x_fraction = vx as f32 / 128.0;
                    let y_fraction = vy as f32 / 128.0;
                    std::array::from_fn(|i| {
                        let near = a[i] + (b[i] - a[i]) * x_fraction;
                        let far = c[i] + (d[i] - c[i]) * x_fraction;
                        near + (far - near) * y_fraction
                    })
                }
            };
            let level = self.level;
            // `interpolateHeight`.
            let near = (vx * level.height(x + 1, y) + (128 - vx) * level.height(x, y)) >> 7;
            let far = (level.height(x, y + 1) * (128 - vx) + level.height(x + 1, y + 1) * vx) >> 7;
            let height = (near * (128 - vy) + vy * far) >> 7;
            fan.push(self.vertex((x << 7) + vx, height, (y << 7) + vy, normal));
        }
        // `LightMesh.addFan`.
        for i in 1..fan.len().saturating_sub(1) {
            self.mesh.indices.extend([fan[0], fan[i], fan[i + 1]]);
        }
    }
}

/// The mesh of every light on `level` (None for a light on another level, or
/// one that lights no tile). `hidden_overlay` is `FloType.baseColor == -1` for
/// a one-based overlay ID; `diagonal_wall` is the rotation of a shape 9
/// location that `SceneGraph.getScenery` finds on the tile.
///
/// A light whose span profile is shorter than its radius requires would stop
/// the original's scene build with an exception; it gets no mesh here.
pub fn build(
    level_index: usize,
    level: &Level<'_>,
    lights: &[Source<'_>],
    hidden_overlay: impl Fn(u8) -> bool,
    diagonal_wall: impl Fn(i32, i32) -> Option<i32>,
) -> Vec<Option<Mesh>> {
    let normals = level.normals();
    let (last_x, last_y) = (level.size[0] as i32 - 1, level.size[1] as i32 - 1);
    lights
        .iter()
        .map(|light| {
            if light.level != level_index {
                return None;
            }
            let mut builder = Builder {
                level,
                normals: &normals,
                light,
                mesh: Mesh::default(),
                cache: HashMap::new(),
            };
            let min_x = light.tile[0] - light.radius;
            let mut min_y = light.tile[1] - light.radius;
            let mut offset = 0i32;
            if min_y < 0 {
                offset = -min_y;
                min_y = 0;
            }
            let max_y = (light.radius + light.tile[1]).min(last_y);
            let span = |offset: i32| -> Option<(i32, i32)> {
                let span = i32::from(*light.spans.get(usize::try_from(offset).ok()?)?);
                let start = min_x + (span >> 8);
                Some((start, start + (span & 0xff)))
            };
            for y in min_y..=max_y {
                let (start, end) = span(offset)?;
                let end_x = (end - 1).min(last_x);
                let start_x = start.max(0);
                for x in start_x..=end_x {
                    let tile = level.tile(x, y);
                    let (underlay, overlay) = (level.underlays[tile], level.overlays[tile]);
                    let shape = usize::from(level.shapes[tile]).min(11);
                    let mut rotation = i32::from(level.rotations[tile]);
                    let mut shaped = false;
                    if underlay == 0 {
                        if overlay == 0 || hidden_overlay(overlay) {
                            continue;
                        }
                        if shape != 0 {
                            builder.tile(x, y, UNDERLAY_SHAPE_VERTICES[shape], rotation);
                            continue;
                        }
                    } else if overlay != 0 {
                        if hidden_overlay(overlay) {
                            builder.tile(x, y, OVERLAY_SHAPE_VERTICES[shape], rotation);
                            continue;
                        }
                        shaped = shape != 0;
                    }
                    if let Some(wall) = diagonal_wall(x, y) {
                        // Whether the footprint reaches the tile from each
                        // side of the wall: along the row, else from the row
                        // the wall leans towards.
                        let mut first = x > start_x;
                        let mut second = end_x > x;
                        let covers =
                            |row: i32| span(row).is_some_and(|(start, end)| x > start && end > x);
                        let (first_row, second_row) = if wall & 1 == 0 { (1, -1) } else { (-1, 1) };
                        let in_rows = |step: i32| (min_y..=max_y).contains(&(y + step));
                        if !first && in_rows(first_row) {
                            first = covers(offset + first_row);
                        }
                        if !second && in_rows(second_row) {
                            second = covers(offset + second_row);
                        }
                        let vertices = if first && second {
                            Some(UNDERLAY_SHAPE_VERTICES[0])
                        } else if first {
                            rotation = if wall & 1 == 0 { 1 } else { 0 };
                            Some(UNDERLAY_SHAPE_VERTICES[1])
                        } else if second {
                            rotation = if wall & 1 == 0 { 3 } else { 2 };
                            Some(UNDERLAY_SHAPE_VERTICES[1])
                        } else {
                            None
                        };
                        if let Some(vertices) = vertices {
                            builder.tile(x, y, vertices, rotation);
                        }
                        continue;
                    }
                    if shaped {
                        builder.tile(x, y, OVERLAY_SHAPE_VERTICES[shape], rotation);
                        builder.tile(x, y, UNDERLAY_SHAPE_VERTICES[shape], rotation);
                    } else {
                        builder.tile(x, y, UNDERLAY_SHAPE_VERTICES[0], rotation);
                    }
                }
                offset += 1;
            }
            let mesh = builder.mesh;
            (!mesh.positions.is_empty() && !mesh.indices.is_empty()).then_some(mesh)
        })
        .collect()
}

/// `LightingManager.renderLightMeshes`' footprint walk: the tiles whose
/// visibility decides whether the mesh is drawn, inside the visible window
/// `[min, max)`. The mesh is drawn if any of them is visible.
pub fn footprint(light: &Source<'_>, min: [i32; 2], max: [i32; 2]) -> Vec<[i32; 2]> {
    let mut tiles = Vec::new();
    let mut offset = 0i32;
    let mut start_row = light.tile[1] - light.radius;
    let end_row = (light.tile[1] + light.radius).min(max[1] - 1);
    if start_row < min[1] {
        offset = min[1] - start_row;
        start_row = min[1];
    }
    for row in start_row..=end_row {
        let Some(&span) = usize::try_from(offset)
            .ok()
            .and_then(|i| light.spans.get(i))
        else {
            break;
        };
        offset += 1;
        let span = i32::from(span);
        let start = light.tile[0] + (span >> 8) - light.radius;
        let end = (start + (span & 0xff) - 1).min(max[0] - 1);
        for column in start.max(min[0])..=end {
            tiles.push([column, row]);
        }
    }
    tiles
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Flat {
        underlays: Vec<u8>,
        overlays: Vec<u8>,
        shapes: Vec<u8>,
        rotations: Vec<u8>,
        heights: Vec<i32>,
    }
    impl Flat {
        fn new() -> Self {
            Self {
                underlays: vec![1; 104 * 104],
                overlays: vec![0; 104 * 104],
                shapes: vec![0; 104 * 104],
                rotations: vec![0; 104 * 104],
                heights: vec![0; 105 * 105],
            }
        }
        fn level(&self) -> Level<'_> {
            Level {
                size: [104, 104],
                underlays: &self.underlays,
                overlays: &self.overlays,
                shapes: &self.shapes,
                rotations: &self.rotations,
                heights: &self.heights,
            }
        }
    }
    // A radius 1 light over tile (10, 10): rows of three tiles.
    const SPANS: [i16; 3] = [3, 3, 3];
    fn light(level: usize) -> Source<'static> {
        Source {
            level,
            tile: [10, 10],
            position: [10 * 128, -100, 10 * 128],
            radius: 1,
            spans: &SPANS,
            color: 0xff8040,
        }
    }

    // Transcription tests; the JVM differential is `tests/lighting_oracle.rs`.
    #[test]
    fn a_light_is_meshed_on_its_own_level_only() {
        let scene = Flat::new();
        let lights = [light(0), light(1)];
        let meshes = build(0, &scene.level(), &lights, |_| false, |_, _| None);
        let mesh = meshes[0].as_ref().unwrap();
        assert!(meshes[1].is_none());
        // Nine tiles, two triangles each; the 16 corners are shared.
        assert_eq!(mesh.indices.len(), 9 * 6);
        assert_eq!(mesh.positions.len(), 16);
        // Straight below the light the normal faces it and the falloff is
        // 1 - (100 / 192)^2, doubled and capped at full colour.
        let centre = mesh
            .positions
            .iter()
            .position(|p| *p == [10 * 128, 0, 10 * 128])
            .unwrap();
        assert_eq!(mesh.colors[centre], [255, 0x80, 0x40]);
    }

    #[test]
    fn tiles_without_a_visible_floor_are_skipped() {
        let mut scene = Flat::new();
        let hole = 10 * 104 + 10;
        scene.underlays[hole] = 0;
        // An invisible overlay over an underlay lights the part the overlay
        // leaves, which for shape 0 is nothing.
        let hidden = 9 * 104 + 10;
        scene.overlays[hidden] = 5;
        let meshes = build(0, &scene.level(), &[light(0)], |id| id == 5, |_, _| None);
        assert_eq!(meshes[0].as_ref().unwrap().indices.len(), 7 * 6);
        // A visible overlay with no underlay is lit.
        scene.overlays[hole] = 2;
        let meshes = build(0, &scene.level(), &[light(0)], |id| id == 5, |_, _| None);
        assert_eq!(meshes[0].as_ref().unwrap().indices.len(), 8 * 6);
    }

    #[test]
    fn a_diagonal_wall_cuts_the_tile_to_the_side_the_footprint_reaches() {
        let scene = Flat::new();
        let wall = |x, y| (x == 9 && y == 10).then_some(0);
        let meshes = build(0, &scene.level(), &[light(0)], |_| false, wall);
        // The footprint's west edge: nothing lies west in the row, and the
        // row to the north starts on the same column, so only the east half
        // is open: one triangle, rotated by 3.
        assert_eq!(meshes[0].as_ref().unwrap().indices.len(), 8 * 6 + 3);
        // In the middle both sides are open and the whole tile is lit.
        let wall = |x, y| (x == 10 && y == 10).then_some(1);
        let meshes = build(0, &scene.level(), &[light(0)], |_| false, wall);
        assert_eq!(meshes[0].as_ref().unwrap().indices.len(), 9 * 6);
    }

    #[test]
    fn a_short_span_profile_yields_no_mesh_instead_of_a_panic() {
        let scene = Flat::new();
        let mut bad = light(0);
        bad.spans = &SPANS[..1];
        assert!(build(0, &scene.level(), &[bad], |_| false, |_, _| None)[0].is_none());
    }

    #[test]
    fn the_footprint_is_clipped_to_the_visible_window() {
        let tiles = footprint(&light(0), [10, 10], [104, 11]);
        assert_eq!(tiles, [[10, 10], [11, 10]]);
    }
}
