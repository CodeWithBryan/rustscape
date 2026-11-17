//! Original SpotAnimType metadata, model transforms and actor attachment.
//! Animation/cache readiness and the actor's sampled terrain are caller services.
use crate::{
    animation::{Pose, Sequence, animate_hd, sin_cos},
    model::Model,
};
use rs09_protocol::{Result, buffer::Reader};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct Definition {
    pub model: u32,
    pub sequence: i32,
    pub resize: [i32; 2],
    pub angle: i32,
    pub ambient: i32,
    pub contrast: i32,
    pub align_to_terrain: bool,
    pub recolor: Vec<[u16; 2]>,
    pub retexture: Vec<[u16; 2]>,
}
impl Default for Definition {
    fn default() -> Self {
        Self {
            model: 0,
            sequence: -1,
            resize: [128; 2],
            angle: 0,
            ambient: 0,
            contrast: 0,
            align_to_terrain: false,
            recolor: vec![],
            retexture: vec![],
        }
    }
}
impl Definition {
    pub fn decode(data: &[u8]) -> Result<(Self, usize)> {
        let mut r = Reader::new(data);
        let mut d = Self::default();
        loop {
            match r.g1()? {
                0 => return Ok((d, r.position())),
                1 => d.model = r.g2()? as u32,
                2 => d.sequence = r.g2()?,
                4 => d.resize[0] = r.g2()?,
                5 => d.resize[1] = r.g2()?,
                6 => d.angle = r.g2()?,
                7 => d.ambient = r.g1()?,
                8 => d.contrast = r.g1()?,
                9 => d.align_to_terrain = true,
                op @ (40 | 41) => {
                    let pairs = if op == 40 {
                        &mut d.recolor
                    } else {
                        &mut d.retexture
                    };
                    pairs.clear();
                    for _ in 0..r.g1()? {
                        pairs.push([r.g2()? as u16, r.g2()? as u16]);
                    }
                }
                // The original consumes only an unknown opcode byte.
                _ => {}
            }
        }
    }
    pub fn assemble(&self, models: &BTreeMap<u32, Model>) -> std::result::Result<Model, String> {
        let mut m = models
            .get(&self.model)
            .ok_or_else(|| format!("Missing spot model {}", self.model))?
            .clone();
        m.recolor(&self.recolor);
        for &[from, to] in &self.retexture {
            for texture in &mut m.textures {
                if *texture == i32::from(from) {
                    *texture = i32::from(to);
                }
            }
        }
        m.lighting = [self.ambient + 64, self.contrast + 850];
        m.sort_by_priority = true;
        Ok(m)
    }
    /// SeqType.animateSpotAnim uses frameClock-1, supplied by Track::cursor.
    /// Resize and quarter turns alter positions, retaining GL normals and UVs.
    pub fn pose(&self, model: &Model, animation: Option<(&Sequence, Pose<'_>)>) -> Model {
        let mut m = if self.sequence != -1 {
            animation.map_or_else(
                || model.clone(),
                |(s, p)| {
                    animate_hd(
                        model,
                        p.base,
                        p.frame,
                        p.next,
                        p.delta,
                        p.length,
                        s.transform_normals,
                    )
                },
            )
        } else {
            model.clone()
        };
        if self.resize != [128; 2] {
            for v in &mut m.vertices {
                for (n, scale) in v
                    .iter_mut()
                    .zip([self.resize[0], self.resize[1], self.resize[0]])
                {
                    *n = n.wrapping_mul(scale) >> 7;
                }
            }
        }
        for v in &mut m.vertices {
            let [x, _, z] = *v;
            match self.angle {
                90 => {
                    v[0] = z;
                    v[2] = x.wrapping_neg();
                }
                180 => {
                    v[0] = x.wrapping_neg();
                    v[2] = z.wrapping_neg();
                }
                270 => {
                    v[0] = z.wrapping_neg();
                    v[2] = x;
                }
                _ => {}
            }
        }
        m
    }
    pub fn attach(&self, model: &mut Model, height: i32, terrain: Alignment) {
        translate_y(model, height.wrapping_neg());
        if self.align_to_terrain {
            terrain.apply(model);
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct Alignment {
    pub pitch: i32,
    pub roll: i32,
    pub offset: i32,
}
impl Alignment {
    /// PathingEntity.alignToTerrain, with a stable original signed-height grid.
    /// Sampling order is front-left/front-right/back-left/back-right.
    pub fn sample(
        dimensions: [i32; 2],
        yaw: i32,
        fine: [i32; 2],
        tile_height: i32,
        mut height: impl FnMut([i32; 2]) -> i32,
    ) -> Self {
        let [width, depth] = dimensions;
        if width == 0 || depth == 0 {
            return Self::default();
        }
        let (sin, cos) = sin_cos(yaw);
        let mut heights = [0; 4];
        for (i, [x, z]) in [
            [width.wrapping_neg() / 2, depth.wrapping_neg() / 2],
            [width / 2, depth.wrapping_neg() / 2],
            [width.wrapping_neg() / 2, depth / 2],
            [width / 2, depth / 2],
        ]
        .into_iter()
        .enumerate()
        {
            heights[i] = height([
                fine[0].wrapping_add(x.wrapping_mul(cos).wrapping_add(z.wrapping_mul(sin)) >> 16),
                fine[1].wrapping_add(z.wrapping_mul(cos).wrapping_sub(x.wrapping_mul(sin)) >> 16),
            ]);
        }
        let [a, b, c, d] = heights;
        Self {
            pitch: ((f64::from(a.min(b).wrapping_sub(c.min(d))).atan2(f64::from(depth)) * 325.95)
                as i32)
                & 2047,
            roll: ((f64::from(a.min(c).wrapping_sub(b.min(d))).atan2(f64::from(width)) * 325.95)
                as i32)
                & 2047,
            offset: (a.wrapping_add(d).min(b.wrapping_add(c)) >> 1).wrapping_sub(tile_height),
        }
    }
    pub fn apply(self, model: &mut Model) {
        for (axis, angle) in [(0, self.pitch), (2, self.roll)] {
            if angle == 0 {
                continue;
            }
            let (sin, cos) = sin_cos(angle);
            for v in &mut model.vertices {
                let [x, y, z] = *v;
                if axis == 0 {
                    v[1] = y.wrapping_mul(cos).wrapping_sub(z.wrapping_mul(sin)) >> 16;
                    v[2] = y.wrapping_mul(sin).wrapping_add(z.wrapping_mul(cos)) >> 16;
                } else {
                    v[0] = y.wrapping_mul(sin).wrapping_add(x.wrapping_mul(cos)) >> 16;
                    v[1] = y.wrapping_mul(cos).wrapping_sub(x.wrapping_mul(sin)) >> 16;
                }
            }
        }
        translate_y(model, self.offset);
    }
}
fn translate_y(model: &mut Model, y: i32) {
    for v in &mut model.vertices {
        v[1] = v[1].wrapping_add(y);
    }
}
