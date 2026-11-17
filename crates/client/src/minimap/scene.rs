use super::SIZE;
use crate::{floor::Overlay, interaction::LocationType, texture::Material, ui::sprite::Sheet};
use rs09_protocol::{Error, Result, buffer::Reader};

#[derive(Clone, Debug, Default)]
pub struct MapScene {
    pub sprite: i32,
    pub tint: u32,
    pub stretch: bool,
}
impl MapScene {
    pub fn decode(data: &[u8]) -> Result<Self> {
        let mut r = Reader::new(data);
        let mut out = Self::default();
        loop {
            match r.g1()? {
                0 => return Ok(out),
                1 => out.sprite = r.g2()?,
                2 => out.tint = r.g3()? as u32,
                3 => out.stretch = true,
                4 => out.sprite = -1,
                _ => {}
            }
        }
    }
}

/// Indexed MsiType art retains the original cropped bounds and first frame only.
#[derive(Clone, Debug)]
pub struct Indexed {
    pub size: [i32; 2],
    pub inner: [i32; 2],
    pub offset: [i32; 2],
    pub indices: Vec<u8>,
    pub palette: Vec<u32>,
}
impl Indexed {
    pub fn first(sheet: &Sheet) -> Self {
        let f = &sheet.frames[0];
        Self {
            size: f.size.map(|v| v as i32),
            inner: sheet.size.map(|v| v as i32),
            offset: f.offset.map(|v| v as i32),
            indices: f.indices.clone(),
            palette: sheet.palette.clone(),
        }
    }
    pub fn adjust_palette(&mut self, delta: [i32; 3]) {
        for color in &mut self.palette {
            *color = (0..3).fold(0, |rgb, i| {
                let shift = 16 - i * 8;
                rgb | (((((*color >> shift) & 255) as i32 + delta[i]).clamp(0, 255) as u32)
                    << shift)
            });
        }
    }
    /// SoftwareIndexedSprite.flipVertical rotates a quarter-turn despite its name.
    pub fn rotate(&mut self) {
        let [w, h] = self.size;
        let mut pixels = Vec::with_capacity(self.indices.len());
        for x in 0..w {
            for y in (0..h).rev() {
                pixels.push(self.indices[(x + y * w) as usize]);
            }
        }
        self.indices = pixels;
        self.offset = [self.inner[1] - h - self.offset[1], self.offset[0]];
        self.size = [h, w];
        self.inner.swap(0, 1);
    }
    pub fn trim(&mut self) {
        if self.size == self.inner {
            return;
        }
        let mut data = vec![0; (self.inner[0] * self.inner[1]) as usize];
        for y in 0..self.size[1] {
            for x in 0..self.size[0] {
                data[(x + self.offset[0] + (y + self.offset[1]) * self.inner[0]) as usize] =
                    self.indices[(x + y * self.size[0]) as usize];
            }
        }
        self.indices = data;
        self.size = self.inner;
        self.offset = [0, 0];
    }
    /// Original scaled indexed blit and tint arithmetic into a 512-square map target.
    pub fn scaled(
        &self,
        pixels: &mut [u32],
        position: [i32; 2],
        draw: [i32; 2],
        tint: u32,
    ) -> Result<()> {
        if draw.iter().any(|&v| v <= 0) {
            return Err(Error::new(0, "invalid mapscene draw size"));
        }
        let [mut x, mut y] = position;
        let [mut w, mut h] = draw;
        let [sw, sh] = self.size;
        let [iw, ih] = self.inner;
        let sx_step = (iw << 16) / w;
        let sy_step = (ih << 16) / h;
        if sx_step == 0 || sy_step == 0 {
            return Err(Error::new(0, "invalid mapscene scale"));
        }
        let (mut sx, mut sy) = (0, 0);
        if self.offset[0] > 0 {
            let clip = ((self.offset[0] << 16) + sx_step - 1) / sx_step;
            x += clip;
            sx = clip * sx_step - (self.offset[0] << 16);
        }
        if self.offset[1] > 0 {
            let clip = ((self.offset[1] << 16) + sy_step - 1) / sy_step;
            y += clip;
            sy = clip * sy_step - (self.offset[1] << 16);
        }
        if sw < iw {
            w = ((sw << 16) + sx_step - sx - 1) / sx_step;
        }
        if sh < ih {
            h = ((sh << 16) + sy_step - sy - 1) / sy_step;
        }
        let mut dest = x + y * SIZE as i32;
        let mut pad = SIZE as i32 - w;
        if y + h > SIZE as i32 {
            h -= y + h - SIZE as i32;
        }
        if y < 0 {
            h += y;
            dest -= y * SIZE as i32;
            sy -= sy_step * y;
        }
        if x + w > SIZE as i32 {
            let clipped = x + w - SIZE as i32;
            w -= clipped;
            pad += clipped;
        }
        if x < 0 {
            w += x;
            dest -= x;
            sx -= sx_step * x;
            pad -= x;
        }
        let reset = sx;
        for _ in 0..h {
            let row = (sy >> 16) * sw;
            for _ in 0..w {
                let index = *self
                    .indices
                    .get(((sx >> 16) + row) as usize)
                    .ok_or(Error::new(0, "mapscene sprite bounds"))?;
                if index != 0 {
                    let mut color = self.palette[index as usize];
                    if tint != 0 {
                        color = (0..3).fold(0, |rgb, i| {
                            let shift = i * 8;
                            rgb | (((((color >> shift) & 255) * ((tint >> shift) & 255)) >> 8)
                                << shift)
                        });
                    }
                    *pixels
                        .get_mut(dest as usize)
                        .ok_or(Error::new(0, "mapscene raster bounds"))? = color;
                }
                dest += 1;
                sx += sx_step;
            }
            sy += sy_step;
            sx = reset;
            dest += pad;
        }
        Ok(())
    }
}

/// SceneGraph's minimap flat RGB. The two signed jitter values retain their original names.
pub fn floor_rgb(
    underlay: Option<u16>,
    overlay: Option<&Overlay>,
    materials: &[Option<Material>],
    jitter: [i32; 2],
    palette: &[u32],
) -> [u32; 2] {
    let level = (jitter[1] >> 2) << 10;
    let light = jitter[0] >> 1;
    let adjusted =
        |hsl: i32| (hsl & 0x380) + ((hsl + level) & 0xfc00) + ((hsl & 127) + light).clamp(0, 127);
    let rgb = |hsl: i32| {
        let l = (((hsl & 127) * 96) >> 7).clamp(2, 126);
        palette[((hsl & 0xff80) + l) as usize]
    };
    let under = underlay.map_or(0, |hsl| rgb(adjusted(i32::from(hsl))));
    let over = overlay.map_or(0, |o| {
        let texture = (o.texture >= 0)
            .then(|| materials.get(o.texture as usize).and_then(Option::as_ref))
            .flatten()
            .filter(|m| m.repeating);
        let mut color = if let Some(t) = texture {
            rgb(i32::from(t.average_color))
        } else if o.color == -1 {
            0
        } else {
            rgb(adjusted(o.color))
        };
        if o.secondary_color >= 0 {
            color = rgb(adjusted(o.secondary_color));
        }
        color
    });
    [under, over]
}

/// No-mapscene wall/diagonal pixels from MiniMap.renderMapTileFeatures.
pub fn stroke(
    pixels: &mut [u32],
    tile: [usize; 2],
    layer: u8,
    shape: u8,
    rotation: u8,
    positive_key: bool,
    colors: [u32; 2],
) {
    let base = (103 - tile[1]) * 2048 + tile[0] * 4 + 24624;
    let wall = if positive_key { colors[1] } else { colors[0] };
    let edge = |pixels: &mut [u32], angle: u8| {
        for i in 0..4 {
            let offset = match angle {
                0 => i * SIZE,
                1 => i,
                2 => i * SIZE + 3,
                _ => 1536 + i,
            };
            pixels[base + offset] = wall;
        }
    };
    if layer == 0 {
        if shape == 0 || shape == 2 {
            edge(pixels, rotation);
        }
        if shape == 3 {
            pixels[base + [0, 3, 1539, 1536][rotation as usize]] = wall;
        }
        if shape == 2 {
            edge(pixels, (rotation + 1) & 3);
        }
    } else if layer == 2 && shape == 9 {
        let color = if positive_key { 15597568 } else { 15658734 };
        for i in 0..4 {
            let offset = if rotation == 0 || rotation == 2 {
                (3 - i) * SIZE + i
            } else {
                i * SIZE + i
            };
            pixels[base + offset] = color;
        }
    }
}

pub fn scenery(
    pixels: &mut [u32],
    tile: [i32; 2],
    loc: &LocationType,
    rotation: i32,
    msi: &MapScene,
    sprite: &Indexed,
) -> Result<()> {
    if msi.sprite == -1 {
        return Ok(());
    }
    let angle = if loc.map_scene_rotated {
        (rotation + loc.map_scene_angle) & 3
    } else {
        0
    };
    let mut size = loc.size;
    if angle & 1 == 1 {
        size.swap(0, 1);
    }
    let draw = if msi.stretch {
        size.map(|v| v * 4)
    } else {
        sprite.inner
    };
    sprite.scaled(
        pixels,
        [tile[0] * 4 + 48, (-size[1] - tile[1] + 104) * 4 + 48],
        draw,
        msi.tint,
    )
}
