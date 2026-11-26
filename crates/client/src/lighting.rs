//! Original cache light decoding, flicker clocks and terrain light colors.
use rs09_protocol::{Error, Result, buffer::Reader};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Flicker {
    pub kind: i32,
    pub speed: i32,
    pub min: i32,
    pub max: i32,
}
impl Default for Flicker {
    fn default() -> Self {
        Self {
            kind: 0,
            speed: 2048,
            min: 2048,
            max: 0,
        }
    }
}
impl Flicker {
    pub fn decode(data: &[u8]) -> Result<Self> {
        let mut r = Reader::new(data);
        let mut f = Self::default();
        loop {
            match r.g1()? {
                0 => return Ok(f),
                1 => f.kind = r.g1()?,
                2 => f.speed = r.g2()?,
                3 => f.min = r.g2()?,
                4 => f.max = r.g2b()?,
                _ => return Err(Error::new(r.position() - 1, "unknown light type opcode")),
            }
        }
    }
    fn preset(p: i32) -> Self {
        let (kind, speed, min, max) = match p {
            2 => (1, 2048, 2048, 0),
            3 => (1, 4096, 2048, 0),
            4 => (4, 2048, 2048, 0),
            5 => (4, 8192, 2048, 0),
            12 => (2, 2048, 2048, 0),
            13 => (2, 8192, 2048, 0),
            10 => (3, 2048, 512, 1536),
            11 => (3, 4096, 512, 1536),
            6 => (3, 2048, 768, 1280),
            7 => (3, 4096, 768, 1280),
            8 => (3, 2048, 1024, 1024),
            9 => (3, 4096, 1024, 1024),
            14 => (1, 2048, 768, 1280),
            15 => (1, 4096, 512, 1536),
            16 => (1, 8192, 256, 1792),
            _ => (0, 2048, 2048, 0),
        };
        Self {
            kind,
            speed,
            min,
            max,
        }
    }
}
#[derive(Clone, Debug)]
pub struct Light {
    pub level: usize,
    pub extends_down: bool,
    pub extends_up: bool,
    /// Original field order, before the SceneGraph placement boundary.
    pub coordinates: [i32; 3],
    pub radius: i32,
    pub spans: Vec<i16>,
    pub color: u32,
    pub preset: i32,
    pub phase: i32,
    pub flicker: Flicker,
    pub quadratic_attenuation: f32,
}
#[derive(Clone, Copy, Debug)]
pub struct Sample {
    pub alpha: f32,
    pub diffuse: [f32; 3],
}
impl Light {
    pub fn decode(
        data: &[u8],
        palette: &[u32],
        mut custom: impl FnMut(i32) -> Result<Flicker>,
    ) -> Result<Self> {
        let mut r = Reader::new(data);
        let flags = r.g1()?;
        let coordinates = [r.g2()?, r.g2()?, r.g2()?];
        let radius = r.g1()?;
        let mut spans = Vec::new();
        for _ in 0..radius * 2 + 1 {
            spans.push(r.g2()? as i16);
        }
        let color = palette[r.g2()? as usize];
        let packed = r.g1()?;
        let preset = packed & 31;
        let flicker = if preset == 31 {
            custom(r.g2()?)?
        } else {
            Flicker::preset(preset)
        };
        let range = (radius << 7) + 64;
        Ok(Self {
            level: (flags & 7) as usize,
            extends_down: flags & 16 != 0,
            extends_up: flags & 8 != 0,
            coordinates,
            radius,
            spans,
            color,
            preset,
            phase: (packed & 224) << 3,
            flicker,
            quadratic_attenuation: 1.0 / (range * range) as f32,
        })
    }
    pub fn sample(&self, time: i32, disable_flicker: bool, noise: &[i32]) -> Sample {
        let t = self
            .phase
            .wrapping_add(time.wrapping_mul(self.flicker.speed) / 50)
            & 2047;
        let mut value = match self.flicker.kind {
            1 => (crate::animation::sin_cos(t).0 >> 6) + 1024,
            3 => noise[t as usize] >> 1,
            4 => (t >> 10) << 11,
            2 => t,
            5 => (if t < 1024 { t } else { 2048 - t }) << 1,
            _ => 2048,
        };
        if disable_flicker {
            value = 2048;
        }
        let alpha = self
            .flicker
            .max
            .wrapping_add(value.wrapping_mul(self.flicker.min) >> 11) as f32
            / 2048.0;
        let scale = alpha / 255.0;
        Sample {
            alpha,
            diffuse: [
                ((self.color >> 16) & 255) as f32 * scale,
                ((self.color >> 8) & 255) as f32 * scale,
                (self.color & 255) as f32 * scale,
            ],
        }
    }
    /// LightMesh.addVertex before flicker modulation. Coordinates use its original
    /// axis order; placement and graphics conversion belong at the scene boundary.
    pub fn floor_color(&self, position: [i32; 3], normal: [f32; 3]) -> [u8; 3] {
        let d: [f32; 3] = std::array::from_fn(|i| (self.coordinates[i] - position[i]) as f32);
        let distance = f64::from(d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt() as f32;
        let inv = 1.0 / distance;
        let direction = d.map(|v| v * inv);
        let attenuation = distance / ((self.radius << 7) + 64) as f32;
        let falloff = (1.0 - attenuation * attenuation).max(0.0);
        let mut dot =
            direction[0] * normal[0] + direction[1] * normal[1] + direction[2] * normal[2];
        if dot < 0.0 {
            dot = 0.0;
        }
        let mut intensity = dot * falloff * 2.0;
        if intensity > 1.0 {
            intensity = 1.0;
        }
        [16, 8, 0].map(|shift| (intensity * ((self.color >> shift) & 255) as f32) as u8)
    }
    pub fn contains(&self, plane: usize, x: i32, z: i32) -> bool {
        if plane < self.level && !self.extends_down || plane > self.level && !self.extends_up {
            return false;
        }
        let row = z - (self.coordinates[1] >> 7) + self.radius;
        let Some(&span) = usize::try_from(row).ok().and_then(|i| self.spans.get(i)) else {
            return false;
        };
        let start = (self.coordinates[0] >> 7) + (i32::from(span) >> 8) - self.radius;
        x >= start && x < start + (i32::from(span) & 255)
    }
}
pub fn noise() -> &'static [i32] {
    static NOISE: std::sync::OnceLock<Vec<i32>> = std::sync::OnceLock::new();
    NOISE.get_or_init(crate::texture_graph::light_noise)
}

/// Literal normal-map loader boundary, including its Y-coordinate overwrite.
pub fn pinned_position(
    raw: [i32; 3],
    region: [i32; 2],
    origin: [i32; 2],
    ground: i32,
) -> Option<[i32; 3]> {
    let x = raw[0].wrapping_add((region[0] - origin[0]) << 7);
    let y = raw[1].wrapping_add((region[1] - origin[1]) << 7);
    if !(0..104).contains(&(x >> 7)) || !(0..104).contains(&(y >> 7)) {
        return None;
    }
    Some([x, ground.wrapping_sub(y), raw[2]])
}

/// Four original light slots per tile, in cache insertion order.
pub fn grid(width: usize, height: usize, lights: &[Light]) -> Vec<[u8; 4]> {
    let mut grid = vec![[0; 4]; width * height * 4];
    for (i, light) in lights.iter().take(255).enumerate() {
        for plane in 0..4 {
            for z in 0..height {
                for x in 0..width {
                    if light.contains(plane, x as i32, z as i32)
                        && let Some(slot) = grid[(plane * height + z) * width + x]
                            .iter_mut()
                            .find(|v| **v == 0)
                    {
                        *slot = (i + 1) as u8;
                    }
                }
            }
        }
    }
    grid
}
