//! The shade walls and scenery bake into the floor: `SceneGraph.shadowmap` as
//! `addLoc` writes it while a scene loads, `clearTerrainRegion`'s fill for a
//! region without terrain, and the `tileLightness` `buildScene` derives from
//! it in HD with high detail lighting.
//!
//! Corners are x-major, `x * (depth + 1) + y`; the original map is 105 x 105.
//! A write outside the map is skipped where the original would fail the load.

/// `GlModel.calculateBounds`' cylinder radius, which is `getLengthXZ()`.
pub fn length_xz(vertices: &[[i32; 3]]) -> i32 {
    let squared = vertices.iter().fold(0, |max: i32, v| {
        max.max(
            v[0].wrapping_mul(v[0])
                .wrapping_add(v[2].wrapping_mul(v[2])),
        )
    });
    i32::from((f64::from(squared).sqrt() + 0.99) as i32 as i16)
}

/// What stands on a tile, as far as shade is concerned.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Caster {
    /// A location whose scene entity is a model: its `getLengthXZ()`.
    Model(i32),
    /// An animated or multi-location entity (`Loc`), which is not a `Model`.
    Animated,
}

#[derive(Clone, Debug)]
pub struct ShadowMap {
    pub width: usize,
    pub depth: usize,
    /// One `(width + 1) * (depth + 1)` map per level.
    pub levels: [Vec<i8>; 4],
}
impl ShadowMap {
    pub fn new(width: usize, depth: usize) -> Self {
        Self {
            width,
            depth,
            levels: std::array::from_fn(|_| vec![0; (width + 1) * (depth + 1)]),
        }
    }
    fn cell(&mut self, level: usize, x: i32, y: i32) -> Option<&mut i8> {
        if x < 0 || y < 0 || x as usize > self.width || y as usize > self.depth {
            return None;
        }
        self.levels
            .get_mut(level)?
            .get_mut(x as usize * (self.depth + 1) + y as usize)
    }
    fn set(&mut self, level: usize, x: i32, y: i32) {
        if let Some(cell) = self.cell(level, x, y) {
            *cell = 50;
        }
    }
    /// `addLoc` for a straight wall (shape 0) or a corner (shapes 1 and 3)
    /// whose type is active (location opcode 64 clears that). The write does
    /// not depend on the wall having a model. Other shapes write nothing here.
    pub fn wall(&mut self, level: usize, x: i32, y: i32, shape: u8, rotation: u8) {
        match (shape, rotation & 3) {
            (0, 0) => {
                self.set(level, x, y);
                self.set(level, x, y + 1);
            }
            (0, 1) => {
                self.set(level, x, y + 1);
                self.set(level, x + 1, y + 1);
            }
            (0, 2) => {
                self.set(level, x + 1, y);
                self.set(level, x + 1, y + 1);
            }
            (0, _) => {
                self.set(level, x, y);
                self.set(level, x + 1, y);
            }
            (1 | 3, 0) => self.set(level, x, y + 1),
            (1 | 3, 1) => self.set(level, x + 1, y + 1),
            (1 | 3, 2) => self.set(level, x + 1, y),
            (1 | 3, _) => self.set(level, x, y),
            _ => {}
        }
    }
    /// `addLoc` for a centrepiece (shapes 10 and 11) of an active type that
    /// was added to the scene: every corner of its footprint is raised to
    /// the caster's strength. `size` is the footprint after the rotation swap.
    pub fn centrepiece(&mut self, level: usize, x: i32, y: i32, size: [i32; 2], caster: Caster) {
        let strength = match caster {
            Caster::Animated => 15,
            Caster::Model(length) => (length / 4).min(30),
        };
        for sx in 0..=size[0] {
            for sy in 0..=size[1] {
                if let Some(cell) = self.cell(level, x + sx, y + sy)
                    && i32::from(*cell) < strength
                {
                    *cell = strength as i8;
                }
            }
        }
    }
    /// `clearTerrainRegion`'s first loop: a region with no terrain is fully
    /// shaded, corners on both far edges included.
    pub fn clear_region(&mut self, level: usize, x: i32, y: i32, width: i32, depth: i32) {
        for cx in x..=x + width {
            for cy in y..=y + depth {
                // The original bounds this fill by the 104 tiles of the scene.
                if cx >= 0
                    && cy >= 0
                    && (cx as usize) < self.width
                    && (cy as usize) < self.depth
                    && let Some(cell) = self.cell(level, cx, cy)
                {
                    *cell = 127;
                }
            }
        }
    }
    /// `tileLightness` of one level on the tile grid (`x * depth + y`), for
    /// corners `1..=width - 2` by `1..=depth - 2`. The rest stays 0, as the
    /// original leaves it.
    pub fn lightness(&self, level: usize) -> Vec<i32> {
        let (w, d) = (self.width, self.depth);
        let map = &self.levels[level];
        let s = |x: usize, y: usize| i32::from(map[x * (d + 1) + y]);
        let mut out = vec![0; w * d];
        for y in 1..d.saturating_sub(1) {
            for x in 1..w.saturating_sub(1) {
                let shade = (s(x + 1, y) >> 3)
                    + (s(x - 1, y) >> 2)
                    + (s(x, y - 1) >> 2)
                    + (s(x, y + 1) >> 3)
                    + (s(x, y) >> 1);
                out[x * d + y] = 74 - shade;
            }
        }
        out
    }
    /// `tileLightness` without high detail lighting (`buildScene` 866-883):
    /// the slope of the terrain against the light direction around 96, less
    /// 1.7 times the shade. `heights` are the level's corner heights
    /// (`x * (depth + 1) + y`) and `sun` is `FogManager.light0Position`.
    ///
    /// A light direction of length zero divides by zero in the original;
    /// here the slope term is then left out.
    pub fn lightness_sd(&self, level: usize, heights: &[i32], sun: [i32; 3]) -> Vec<i32> {
        let (w, d) = (self.width, self.depth);
        let map = &self.levels[level];
        let s = |x: usize, y: usize| i32::from(map[x * (d + 1) + y]);
        let h = |x: usize, y: usize| heights[x * (d + 1) + y];
        let [i, j, k] = sun;
        let length = f64::from(
            j.wrapping_mul(j)
                .wrapping_add(i.wrapping_mul(i))
                .wrapping_add(k.wrapping_mul(k)),
        )
        .sqrt() as i32;
        let m = length.wrapping_mul(1024) >> 8;
        let mut out = vec![0; w * d];
        for y in 1..d.saturating_sub(1) {
            for x in 1..w.saturating_sub(1) {
                let q = h(x + 1, y).wrapping_sub(h(x - 1, y));
                let r = h(x, y + 1).wrapping_sub(h(x, y - 1));
                let slope = f64::from(
                    q.wrapping_mul(q)
                        .wrapping_add(r.wrapping_mul(r))
                        .wrapping_add(65536),
                )
                .sqrt() as i32;
                let shade = (s(x, y + 1) >> 3)
                    + (s(x, y - 1) >> 2)
                    + (s(x - 1, y) >> 2)
                    + (s(x + 1, y) >> 3)
                    + (s(x, y) >> 1);
                let lit = if slope == 0 || m == 0 {
                    96
                } else {
                    let t = q.wrapping_shl(8).wrapping_div(slope);
                    let v = -65536 / slope;
                    let u = r.wrapping_shl(8).wrapping_div(slope);
                    k.wrapping_mul(u)
                        .wrapping_add(i.wrapping_mul(t))
                        .wrapping_add(v.wrapping_mul(j))
                        .wrapping_div(m)
                        .wrapping_add(96)
                };
                out[x * d + y] = lit.wrapping_sub((shade as f32 * 1.7f32) as i32);
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Transcription tests: the expected values are worked by hand from
    // SceneGraph.addLoc and buildScene.
    #[test]
    fn a_wall_shades_its_own_corners_and_bleeds_into_the_neighbours() {
        let mut map = ShadowMap::new(8, 8);
        map.wall(0, 3, 3, 0, 0);
        let light = map.lightness(0);
        assert_eq!(light[3 * 8 + 3], 74 - 25 - 6);
        assert_eq!(light[3 * 8 + 4], 74 - 25 - 12);
        assert_eq!(light[2 * 8 + 3], 74 - 6);
        assert_eq!(light[4 * 8 + 3], 74 - 12);
        assert_eq!(light[5 * 8 + 5], 74);
        assert_eq!(light[0], 0);
    }

    #[test]
    fn a_centrepiece_only_raises_shade_and_is_capped() {
        let mut map = ShadowMap::new(8, 8);
        map.wall(0, 2, 2, 1, 3);
        map.centrepiece(0, 2, 2, [1, 2], Caster::Model(400));
        assert_eq!(map.levels[0][2 * 9 + 2], 50);
        assert_eq!(map.levels[0][3 * 9 + 4], 30);
        map.centrepiece(0, 2, 2, [1, 1], Caster::Animated);
        assert_eq!(map.levels[0][3 * 9 + 3], 30);
        assert_eq!(length_xz(&[[3, 99, 4], [0, 0, 0]]), 5);
        assert_eq!(length_xz(&[[3, 0, 5]]), 6);
    }
}
