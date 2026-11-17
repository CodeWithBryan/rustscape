//! RT4 FluType/FloType color decoding and the ten-tile underlay color window.
use rs09_protocol::{Error, Result, buffer::Reader};

#[derive(Clone, Debug)]
pub struct Underlay {
    pub weighted_hue: i32,
    pub saturation: i32,
    pub lightness: i32,
    pub chroma: i32,
    pub texture: i32,
    pub scale: i32,
    /// FluType.blockShadow; opcode 4 clears it.
    pub block_shadow: bool,
}
impl Default for Underlay {
    fn default() -> Self {
        Self {
            weighted_hue: 0,
            saturation: 0,
            lightness: 0,
            chroma: 0,
            texture: -1,
            scale: 128,
            block_shadow: true,
        }
    }
}
#[derive(Clone, Debug)]
pub struct Overlay {
    pub color: i32,
    pub secondary_color: i32,
    pub texture: i32,
    pub scale: i32,
    pub blend: bool,
    pub water_color: u32,
    pub water_opacity: u8,
    /// FloType.occludeUnderlay; opcode 5 clears it.
    pub occlude_underlay: bool,
    /// FloType.castsShadow; opcode 10 clears it.
    pub casts_shadow: bool,
    /// FloType.textureBrightness (opcode 11), part of the HD tile group key.
    pub texture_brightness: i32,
}
impl Default for Overlay {
    fn default() -> Self {
        Self {
            color: 0,
            secondary_color: -1,
            texture: -1,
            scale: 128,
            blend: false,
            water_color: 1190717,
            water_opacity: 16,
            occlude_underlay: true,
            casts_shadow: true,
            texture_brightness: 8,
        }
    }
}
fn hsl(rgb: i32) -> (f64, f64, f64) {
    let [r, g, b] = [
        (rgb >> 16 & 255) as f64 / 256.0,
        (rgb >> 8 & 255) as f64 / 256.0,
        (rgb & 255) as f64 / 256.0,
    ];
    let min = r.min(g).min(b);
    let max = r.max(g).max(b);
    let l = (min + max) / 2.0;
    if min == max {
        return (0.0, 0.0, l);
    }
    let s = if l < 0.5 {
        (max - min) / (min + max)
    } else {
        (max - min) / (2.0 - max - min)
    };
    let h = if max == r {
        (g - b) / (max - min)
    } else if max == g {
        (b - r) / (max - min) + 2.0
    } else {
        (r - g) / (max - min) + 4.0
    };
    (h / 6.0, s, l)
}
fn pack_hsl_value(lightness: i32, mut saturation: i32, hue: i32) -> i32 {
    if lightness > 243 {
        saturation >>= 4;
    } else if lightness > 217 {
        saturation >>= 3;
    } else if lightness > 192 {
        saturation >>= 2;
    } else if lightness > 179 {
        saturation >>= 1;
    }
    (lightness >> 1) + (saturation >> 5 << 7) + (hue >> 2 << 10)
}
pub fn pack_hsl(lightness: i32, saturation: i32, hue: i32) -> u16 {
    pack_hsl_value(lightness, saturation, hue) as u16
}
pub fn color(rgb: i32) -> i32 {
    if rgb == 0xff00ff {
        -1
    } else {
        let (h, s, l) = hsl(rgb);
        pack_hsl_value(
            ((l * 256.0) as i32).clamp(0, 255),
            ((s * 256.0) as i32).clamp(0, 255),
            (h * 256.0) as i32,
        )
    }
}
impl Underlay {
    pub fn decode(data: &[u8]) -> Result<Self> {
        let mut r = Reader::new(data);
        let mut out = Self::default();
        loop {
            match r.g1()? {
                0 => return Ok(out),
                1 => {
                    let (h, s, l) = hsl(r.g3()?);
                    out.chroma = (s * if l > 0.5 { 1.0 - l } else { l } * 512.0) as i32;
                    out.chroma = out.chroma.max(1);
                    out.saturation = ((s * 256.0) as i32).clamp(0, 255);
                    out.lightness = ((l * 256.0) as i32).clamp(0, 255);
                    out.weighted_hue = (f64::from(out.chroma) * h) as i32;
                }
                2 => {
                    out.texture = r.g2()?;
                    if out.texture == 65535 {
                        out.texture = -1;
                    }
                }
                3 => out.scale = r.g2()?,
                4 => out.block_shadow = false,
                _ => return Err(Error::new(r.position() - 1, "unknown underlay opcode")),
            }
        }
    }
}
impl Overlay {
    pub fn decode(data: &[u8]) -> Result<Self> {
        let mut r = Reader::new(data);
        let mut out = Self::default();
        loop {
            match r.g1()? {
                0 => return Ok(out),
                1 => out.color = color(r.g3()?),
                2 => out.texture = r.g1()?,
                3 => {
                    out.texture = r.g2()?;
                    if out.texture == 65535 {
                        out.texture = -1;
                    }
                }
                5 => out.occlude_underlay = false,
                8 => {}
                10 => out.casts_shadow = false,
                7 => out.secondary_color = color(r.g3()?),
                9 => out.scale = r.g2()?,
                11 => out.texture_brightness = r.g1()?,
                12 => out.blend = true,
                13 => out.water_color = r.g3()? as u32,
                14 => out.water_opacity = r.g1()? as u8,
                _ => return Err(Error::new(r.position() - 1, "unknown overlay opcode")),
            }
        }
    }
}
/// Prefix sums produce the same window as SceneGraph's add-at-+5/remove-at--5 scan.
pub fn blend(width: usize, height: usize, tiles: &[u8], types: &[Underlay]) -> Vec<u16> {
    let stride = height + 1;
    let mut prefix = vec![[0i32; 5]; (width + 1) * stride];
    for x in 0..width {
        for y in 0..height {
            let t = tiles[x * height + y];
            let values = if t == 0 {
                [0; 5]
            } else {
                let t = &types[t as usize - 1];
                [t.weighted_hue, t.saturation, t.lightness, t.chroma, 1]
            };
            for (i, &v) in values.iter().enumerate() {
                prefix[(x + 1) * stride + y + 1][i] =
                    v + prefix[x * stride + y + 1][i] + prefix[(x + 1) * stride + y][i]
                        - prefix[x * stride + y][i];
            }
        }
    }
    let mut colors = vec![0; width * height];
    for x in 0..width {
        for y in 0..height {
            let (x0, x1) = (x.saturating_sub(4), (x + 6).min(width));
            let (y0, y1) = (y.saturating_sub(4), (y + 6).min(height));
            let sum: [i32; 5] = std::array::from_fn(|i| {
                prefix[x1 * stride + y1][i]
                    - prefix[x0 * stride + y1][i]
                    - prefix[x1 * stride + y0][i]
                    + prefix[x0 * stride + y0][i]
            });
            if sum[4] > 0 {
                colors[x * height + y] =
                    pack_hsl(sum[2] / sum[4], sum[1] / sum[4], sum[0] * 256 / sum[3]);
            }
        }
    }
    colors
}
