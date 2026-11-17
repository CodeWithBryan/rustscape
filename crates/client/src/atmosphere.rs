//! RT4 map environments, FogManager transitions and fixed-function fog parameters.
use rs09_protocol::{Result, buffer::Reader};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Environment {
    pub color: i32,
    pub ambient: f32,
    pub diffuse: [f32; 2],
    pub direction: [i32; 3],
    pub fog_color: i32,
    pub fog_depth: i32,
}
impl Default for Environment {
    fn default() -> Self {
        Self {
            color: 16777215,
            ambient: 1.1523438,
            diffuse: [0.69921875, 1.2],
            direction: [-50, -60, -50],
            fog_color: 13156520,
            fog_depth: 0,
        }
    }
}
impl Environment {
    pub fn decode(r: &mut Reader<'_>) -> Result<Self> {
        let flags = r.g1()?;
        let mut e = Self::default();
        if flags & 1 != 0 {
            e.color = r.g4()?;
        }
        if flags & 2 != 0 {
            e.ambient = r.g2()? as f32 / 256.0;
        }
        if flags & 4 != 0 {
            e.diffuse[0] = r.g2()? as f32 / 256.0;
        }
        if flags & 8 != 0 {
            e.diffuse[1] = r.g2()? as f32 / 256.0;
        }
        if flags & 16 != 0 {
            e.direction = [r.g2b()?, r.g2b()?, r.g2b()?];
        }
        if flags & 32 != 0 {
            e.fog_color = r.g4()?;
        }
        if flags & 64 != 0 {
            e.fog_depth = r.g2()?;
        }
        Ok(e)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Parameters {
    pub color: i32,
    pub ambient: f32,
    pub diffuse: [f32; 2],
    pub fog_color: i32,
    pub fog_depth: i32,
}
#[derive(Clone, Debug)]
pub struct Atmosphere {
    pub current: Parameters,
    pub target: Parameters,
    pub previous: Parameters,
    pub fade_progress: i32,
    pub instant: bool,
}
impl Default for Atmosphere {
    fn default() -> Self {
        Self {
            current: Parameters::default(),
            target: Parameters::default(),
            previous: Parameters::default(),
            fade_progress: 0,
            instant: true,
        }
    }
}
fn blend_color(a: i32, b: i32, previous_weight: i32, target_weight: i32) -> i32 {
    let rb = (a & 0xff00ff)
        .wrapping_mul(previous_weight)
        .wrapping_add((b & 0xff00ff).wrapping_mul(target_weight))
        & 0xff00ff00u32 as i32;
    let g = (a & 0xff00)
        .wrapping_mul(previous_weight)
        .wrapping_add((b & 0xff00).wrapping_mul(target_weight))
        & 0xff0000;
    rb.wrapping_add(g) >> 8
}
impl Atmosphere {
    /// Delta is the number of 20ms client ticks since the preceding rendered frame.
    pub fn update(
        &mut self,
        e: Environment,
        mut delta: i32,
        brightness: i32,
        fog_enabled: bool,
    ) -> Parameters {
        if self.instant {
            delta = 1000000;
            self.instant = false;
        }
        let target = Parameters {
            color: e.color,
            ambient: (brightness as f32 * 0.1 + 0.7) * e.ambient,
            diffuse: e.diffuse,
            fog_color: e.fog_color,
            fog_depth: if fog_enabled { e.fog_depth } else { 0 },
        };
        if target != self.target {
            self.target = target;
            self.previous = self.current;
            self.fade_progress = 0;
        }
        if self.fade_progress < 65536 {
            self.fade_progress = self.fade_progress.wrapping_add(delta.wrapping_mul(250));
            if self.fade_progress >= 65536 {
                self.fade_progress = 65536;
            }
            let progress = self.fade_progress as f32 / 65536.0;
            let target_weight = self.fade_progress >> 8;
            let previous_weight = (65536 - self.fade_progress) >> 8;
            let previous_progress = (65536 - self.fade_progress) as f32 / 65536.0;
            self.current = Parameters {
                color: blend_color(
                    self.previous.color,
                    self.target.color,
                    previous_weight,
                    target_weight,
                ),
                ambient: previous_progress * self.previous.ambient + progress * self.target.ambient,
                diffuse: [
                    self.previous.diffuse[0] * previous_progress
                        + progress * self.target.diffuse[0],
                    progress * self.target.diffuse[1]
                        + previous_progress * self.previous.diffuse[1],
                ],
                fog_color: blend_color(
                    self.previous.fog_color,
                    self.target.fog_color,
                    previous_weight,
                    target_weight,
                ),
                fog_depth: target_weight
                    .wrapping_mul(self.target.fog_depth)
                    .wrapping_add(previous_weight.wrapping_mul(self.previous.fog_depth))
                    >> 8,
            };
        }
        self.current
    }
}

impl Parameters {
    /// Original values are in cache units (128 per tile), including the minimum 50.
    pub fn fog_range(&self, view_distance: i32, fade_distance: f32) -> [f32; 2] {
        [
            view_distance
                .wrapping_sub((fade_distance * 2.0) as i32)
                .wrapping_sub(self.fog_depth)
                .max(50) as f32,
            view_distance as f32 - fade_distance,
        ]
    }
    /// Ambient and both directional lights share the environment's RGB tint.
    pub fn light_colors(&self) -> [[f32; 3]; 3] {
        let rgb = rgb(self.color);
        [
            rgb.map(|v| self.ambient * v),
            rgb.map(|v| self.diffuse[0] * v),
            rgb.map(|v| -self.diffuse[1] * v),
        ]
    }
}
pub fn rgb(color: i32) -> [f32; 3] {
    [
        (color >> 16 & 255) as f32 / 255.0,
        (color >> 8 & 255) as f32 / 255.0,
        (color & 255) as f32 / 255.0,
    ]
}
