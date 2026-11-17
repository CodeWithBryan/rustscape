//! RT4's scenery shadow sprites and eight-tile shadow texture filtering.
//! Coordinates and masks remain on the CPU, independently of the graphics backend.
use crate::{model::Model, surface, texture::Material};

/// Default player shadow: three nine-sided rings, bounded by the animated body.
/// The animation transform operates on the entire shadow, without HD fractions.
pub fn character(
    body: &Model,
    seed_fine: [i32; 2],
    angle: i32,
    stretched: bool,
    animation: Option<(&crate::animation::Base, &crate::animation::Frame)>,
) -> Model {
    character_styled(
        body,
        seed_fine,
        angle,
        stretched,
        animation,
        CharacterStyle::default(),
    )
}
#[derive(Clone, Copy)]
pub struct CharacterStyle {
    pub size: i32,
    pub colors: [u16; 2],
    pub alpha: [u8; 2],
}
impl Default for CharacterStyle {
    fn default() -> Self {
        Self {
            size: 1,
            colors: [0; 2],
            alpha: [160, 240],
        }
    }
}
pub fn character_styled(
    body: &Model,
    seed_fine: [i32; 2],
    angle: i32,
    stretched: bool,
    animation: Option<(&crate::animation::Base, &crate::animation::Frame)>,
    style: CharacterStyle,
) -> Model {
    let mut min = [i32::MAX; 2];
    let mut max = [i32::MIN; 2];
    for v in &body.vertices {
        for i in 0..2 {
            min[i] = min[i].min(v[i * 2]);
            max[i] = max[i].max(v[i * 2]);
        }
    }
    let mut bounds_min = [-(style.size * 64 - 1); 2];
    let mut bounds_max = [style.size * 64 - 1; 2];
    if stretched {
        if !(384..=1664).contains(&angle) {
            bounds_min[1] -= 128;
        }
        if angle > 1152 && angle < 1920 {
            bounds_max[0] += 128;
        }
        if angle > 640 && angle < 1408 {
            bounds_max[1] += 128;
        }
        if angle > 128 && angle < 896 {
            bounds_min[0] -= 128;
        }
    }
    for i in 0..2 {
        min[i] = min[i].max(bounds_min[i]);
        max[i] = max[i].min(bounds_max[i]);
    }
    let mut result = Model {
        lighting: [64, 768],
        vertices: vec![[0; 3]],
        ..Model::default()
    };
    let segments = match style.size {
        1 => 9,
        2 => 12,
        3 => 15,
        4 => 18,
        _ => 21,
    };
    for radius in [64, 96, 128] {
        for i in 0..segments {
            let (sin, cos) = crate::animation::sin_cos(((i << 11) / segments) as i32);
            result.vertices.push([
                (seed_fine[0] + sin * radius) >> 16,
                0,
                (seed_fine[1] + cos * radius) >> 16,
            ]);
        }
    }
    for ring in 0..3 {
        let weight = (ring * 256 + 128) / 3;
        let alpha = ((usize::from(style.alpha[1]) * weight
            + usize::from(style.alpha[0]) * (256 - weight))
            >> 8) as u8;
        let [inner, outer] = style.colors.map(usize::from);
        let inv = 256 - weight;
        let color = ((((outer & 127) * inv + (inner & 127) * weight) & 0x7f00)
            + ((inv * (outer & 0x380) + weight * (inner & 0x380)) & 0x38000)
            + ((weight * (inner & 0xfc00) + (outer & 0xfc00) * inv) & 0xfc0000))
            >> 8;
        for i in 0..segments {
            let a = 1 + ring * segments + i;
            let b = 1 + ring * segments + (i + 1) % segments;
            let faces = if ring == 0 {
                vec![[0, b, a]]
            } else {
                vec![[a - segments, b - segments, b], [a - segments, b, a]]
            };
            for face in faces {
                result.faces.push(face);
                result.alpha.push(alpha);
                result.colors.push(color as u16);
            }
        }
    }
    result.textures = vec![-1; result.faces.len()];
    result.texture_indices = vec![-1; result.faces.len()];
    result.face_info = vec![1; result.faces.len()];
    result.priorities = vec![0; result.faces.len()];
    result.surfaces = surface::prepare(&result, &[]);
    for v in &mut result.vertices {
        for i in 0..2 {
            v[i * 2] = ((v[i * 2] * ((max[i] - min[i]) / 2)) >> 7) + (min[i] + max[i]) / 2;
        }
    }
    if let Some((base, frame)) = animation {
        for v in &mut result.vertices {
            *v = v.map(|n| n.wrapping_shl(4));
        }
        let mut origin = [0i32; 3];
        for t in &frame.transforms {
            if !base.shadow[t.index] {
                continue;
            }
            if t.previous_origin.is_some() {
                shadow_transform(&mut result, &mut origin, 0, [0; 3]);
            }
            shadow_transform(&mut result, &mut origin, base.types[t.index], t.xyz);
        }
        for v in &mut result.vertices {
            *v = v.map(|n| n.wrapping_add(7) >> 4);
        }
    }
    let (sin, cos) = crate::animation::sin_cos(angle);
    for v in &mut result.vertices {
        let x = v[0];
        v[0] = v[2].wrapping_mul(sin).wrapping_add(x.wrapping_mul(cos)) >> 16;
        v[2] = v[2].wrapping_mul(cos).wrapping_sub(x.wrapping_mul(sin)) >> 16;
    }
    result
}
fn shadow_transform(model: &mut Model, origin: &mut [i32; 3], kind: u8, xyz: [i32; 3]) {
    if kind == 0 {
        *origin = [0; 3];
        for v in &model.vertices {
            for i in 0..3 {
                origin[i] = origin[i].wrapping_add(v[i]);
            }
        }
        for i in 0..3 {
            origin[i] = origin[i] / model.vertices.len() as i32 + xyz[i];
        }
    } else if (1..=3).contains(&kind) {
        for v in &mut model.vertices {
            if kind == 1 {
                for i in 0..3 {
                    v[i] = v[i].wrapping_add(xyz[i]);
                }
                continue;
            }
            for i in 0..3 {
                v[i] = v[i].wrapping_sub(origin[i]);
            }
            if kind == 3 {
                for i in 0..3 {
                    v[i] = v[i].wrapping_mul(xyz[i]) / 128;
                }
            } else {
                for axis in [2, 0, 1] {
                    if xyz[axis] == 0 {
                        continue;
                    }
                    let (s, c) = crate::animation::sin_cos(xyz[axis]);
                    let (a, b) = match axis {
                        2 => (0, 1),
                        0 => (1, 2),
                        _ => (0, 2),
                    };
                    let (x, y) = (v[a], v[b]);
                    let sum = |x: i32, a: i32, y: i32, b: i32| {
                        x.wrapping_mul(a)
                            .wrapping_add(y.wrapping_mul(b))
                            .wrapping_add(32767)
                            >> 16
                    };
                    if axis == 0 {
                        v[a] = sum(x, c, y, -s);
                        v[b] = sum(x, s, y, c);
                    } else {
                        v[a] = sum(x, c, y, s);
                        v[b] = sum(y, c, x, -s);
                    }
                }
            }
            for i in 0..3 {
                v[i] = v[i].wrapping_add(origin[i]);
            }
        }
    } else if kind == 5 {
        for alpha in &mut model.alpha {
            *alpha = (i32::from(*alpha) + xyz[0] * 8).clamp(0, 255) as u8;
        }
    } else if kind == 7 {
        for color in &mut model.colors {
            let v = i32::from(*color);
            *color = ((((v >> 10) + xyz[0]) & 63) << 10
                | ((v >> 7 & 7) + xyz[1] / 4).clamp(0, 7) << 7
                | ((v & 127) + xyz[2]).clamp(0, 127)) as u16;
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Sprite {
    pub offset: [i32; 2],
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
}

pub fn light_projection(direction: [i32; 3]) -> [i32; 2] {
    [direction[0], direction[2]].map(|n| (n as f32 * 256.0 / direction[1] as f32) as i32)
}

/// GlRenderer.setDepthBias projection coefficients, in a consistent length unit.
pub fn depth_bias(near: f32, far: f32, distance: f32, bias: f32) -> [f32; 2] {
    let z_scale = -(far + near) / (far - near);
    let z_offset = -(near * 2.0 * far) / (far - near);
    if bias == 0.0 {
        return [z_scale, z_offset];
    }
    let scale = distance / (bias + distance);
    let square = scale * scale;
    let adjust = -z_offset * (1.0 - scale) * (1.0 - scale) / bias;
    [z_scale + adjust, z_offset * square]
}

/// The original scan converter uses 16.16 edges and excludes the final scanline
/// and right-hand pixel. Restarting the short edge at the middle vertex matters.
pub fn triangle(pixels: &mut [u8], width: usize, mut points: [[i32; 2]; 3]) {
    points.sort_by_key(|p| p[1]);
    let [a, b, c] = points;
    let slope = |p: [i32; 2], q: [i32; 2]| {
        if p[1] == q[1] {
            0
        } else {
            q[0].wrapping_sub(p[0]).wrapping_shl(16) / (q[1] - p[1])
        }
    };
    let ac = slope(a, c);
    let ab = slope(a, b);
    let bc = slope(b, c);
    for y in a[1].max(0)..c[1] {
        let long = a[0]
            .wrapping_shl(16)
            .wrapping_add(ac.wrapping_mul(y - a[1]));
        let (start, step) = if y < b[1] { (a, ab) } else { (b, bc) };
        let short = start[0]
            .wrapping_shl(16)
            .wrapping_add(step.wrapping_mul(y - start[1]));
        let left = (long.min(short) >> 16).max(0) as usize;
        let right = (long.max(short) >> 16).max(0) as usize;
        let row = y as usize * width;
        if row >= pixels.len() {
            break;
        }
        let end = (row + right.min(width)).min(pixels.len());
        if row + left < end {
            pixels[row + left..end].fill(1);
        }
    }
}

pub fn project(model: &Model, materials: &[Option<Material>], light: [i32; 2]) -> Option<Sprite> {
    if !model
        .faces
        .iter()
        .enumerate()
        .any(|(i, _)| surface::visible(model, i, materials))
    {
        return None;
    }
    let mut min = [32767; 3];
    let mut max = [-32768; 3];
    for v in &model.vertices {
        for axis in 0..3 {
            min[axis] = min[axis].min(v[axis]);
            max[axis] = max[axis].max(v[axis]);
        }
    }
    let min = min.map(|n| i32::from(n as i16));
    let max = max.map(|n| i32::from(n as i16));
    let projected = |p: i32, y: i32, l: i32| p.wrapping_sub(y.wrapping_mul(l) >> 8) >> 3;
    let offset = std::array::from_fn(|i| {
        projected(
            min[i * 2],
            if light[i] > 0 { max[1] } else { min[1] },
            light[i],
        )
    });
    let end: [i32; 2] = std::array::from_fn(|i| {
        projected(
            max[i * 2],
            if light[i] > 0 { min[1] } else { max[1] },
            light[i],
        )
    });
    let width = usize::try_from(end[0] + 1 - offset[0]).ok()?;
    let height = usize::try_from(end[1] + 1 - offset[1]).ok()?;
    if width == 0 || height == 0 || width.checked_mul(height)? > 16_777_216 {
        return None;
    }
    let mut sprite = Sprite {
        offset,
        width,
        height,
        pixels: vec![0; width * height],
    };
    let points: Vec<_> = model
        .vertices
        .iter()
        .map(|v| {
            [
                projected(v[0], v[1], light[0]) - offset[0],
                projected(v[2], v[1], light[1]) - offset[1],
            ]
        })
        .collect();
    for (i, face) in model.faces.iter().enumerate() {
        if !surface::visible(model, i, materials) {
            continue;
        }
        // triangleAlpha is a signed Java byte: its <= 128 check accepts every byte.
        let [a, b, c] = face.map(|v| points[v]);
        if (a[0] - b[0])
            .wrapping_mul(b[1] - c[1])
            .wrapping_sub((b[1] - a[1]).wrapping_mul(c[0] - b[0]))
            > 0
        {
            triangle(&mut sprite.pixels, width, [a, b, c]);
        }
    }
    Some(sprite)
}

pub struct Map {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
    dirty: Vec<bool>,
    hashes: Vec<i32>,
}
impl Map {
    pub fn new(columns: usize, rows: usize) -> Self {
        let width = columns * 128 + 2;
        let height = rows * 128 + 2;
        Self {
            width,
            height,
            pixels: vec![0; width * height],
            dirty: vec![true; columns * rows],
            hashes: vec![-1; columns * rows],
        }
    }
    /// Adds/removes an entire model mask, preserving byte-count wrapping and the
    /// one-pixel empty border used by ShadowManager's clipping routine.
    pub fn stamp(&mut self, sprite: &Sprite, position: [i32; 2], subtract: bool) {
        let origin = [
            position[0] + sprite.offset[0] + 1,
            position[1] + sprite.offset[1] + 1,
        ];
        let x0 = origin[0].max(1);
        let y0 = origin[1].max(1);
        let x1 = (origin[0] + sprite.width as i32).min(self.width as i32 - 1);
        let y1 = (origin[1] + sprite.height as i32).min(self.height as i32 - 1);
        if x0 >= x1 || y0 >= y1 {
            return;
        }
        for y in y0..y1 {
            for x in x0..x1 {
                let src = sprite.pixels
                    [(y - origin[1]) as usize * sprite.width + (x - origin[0]) as usize];
                let dst = &mut self.pixels[y as usize * self.width + x as usize];
                *dst = if subtract {
                    dst.wrapping_sub(src)
                } else {
                    dst.wrapping_add(src)
                };
            }
        }
        let columns = (self.width - 2) / 128;
        for y in (y0 - 1) as usize / 128..=(y1 - 2) as usize / 128 {
            for x in (x0 - 1) as usize / 128..=(x1 - 2) as usize / 128 {
                self.dirty[y * columns + x] = true;
            }
        }
    }
    /// Returns only changed 128-square texture tiles, with original occupied
    /// alpha 68 and four-neighbor edge alpha 17. The original row hash is retained.
    pub fn updates(&mut self) -> Vec<([usize; 2], Vec<u8>)> {
        let columns = (self.width - 2) / 128;
        let mut updates = Vec::new();
        for tile in 0..self.dirty.len() {
            if !std::mem::take(&mut self.dirty[tile]) {
                continue;
            }
            let x = tile % columns * 128;
            let y = tile / columns * 128;
            let mut hash = 0i32;
            for row in 0..128 {
                hash = hash.wrapping_shl(8).wrapping_sub(hash);
                for col in 0..128 {
                    hash = hash.wrapping_add(i32::from(
                        self.pixels[(y + row + 1) * self.width + x + col + 1] != 0,
                    ));
                }
            }
            if hash == self.hashes[tile] {
                continue;
            }
            self.hashes[tile] = hash;
            let mut rgba = vec![0; 128 * 128 * 4];
            for row in 0..128 {
                for col in 0..128 {
                    let i = (y + row + 1) * self.width + x + col + 1;
                    rgba[(row * 128 + col) * 4 + 3] = if self.pixels[i] != 0 {
                        68
                    } else {
                        [i - 1, i + 1, i - self.width, i + self.width]
                            .into_iter()
                            .filter(|&j| self.pixels[j] != 0)
                            .count() as u8
                            * 17
                    };
                }
            }
            updates.push(([x, y], rgba));
        }
        updates
    }
}
