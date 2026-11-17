//! Original Component.getSprite image preparation before its graphics upload.
use super::sprite::Sheet;
#[derive(Clone, Copy, Debug, Default, serde::Deserialize, serde::Serialize)]
pub struct Options {
    pub has_alpha: bool,
    pub h_flip: bool,
    pub v_flip: bool,
    pub outline: i32,
    pub shadow: i32,
}
impl Options {
    /// Preserve the original wrapping long sum, including overlapping bits.
    pub fn key(self, id: i32) -> u64 {
        (u64::from(self.v_flip) << 38)
            .wrapping_add(u64::from(self.has_alpha) << 35)
            .wrapping_add(id as i64 as u64)
            .wrapping_add((self.outline as i64 as u64) << 36)
            .wrapping_add(u64::from(self.h_flip) << 39)
            .wrapping_add((self.shadow as i64 as u64) << 40)
    }
}
#[derive(Clone, Debug, serde::Serialize)]
pub struct Image {
    pub inner: [usize; 2],
    pub offset: [usize; 2],
    pub size: [usize; 2],
    pub pixels: Vec<u32>,
    pub alpha: bool,
}
impl Image {
    pub fn prepare(sheet: &Sheet, options: Options) -> Result<Self, String> {
        let f = sheet.frames.first().ok_or("Empty widget sprite")?;
        let alpha = options.has_alpha && f.has_alpha;
        if (0..2).any(|i| {
            f.offset[i]
                .checked_add(f.size[i])
                .is_none_or(|n| n > sheet.size[i])
        }) {
            return Err("Widget sprite bounds exceed inner canvas".into());
        }
        let pixels = f
            .indices
            .iter()
            .enumerate()
            .map(|(i, &p)| {
                sheet.palette[p as usize]
                    | if alpha {
                        u32::from(f.alpha[i]) << 24
                    } else {
                        0
                    }
            })
            .collect();
        let mut image = Self {
            inner: sheet.size,
            offset: f.offset,
            size: f.size,
            pixels,
            alpha,
        };
        if options.v_flip {
            image.flip_vertical();
        }
        if options.h_flip {
            image.flip_horizontal();
        }
        if options.outline > 0 {
            image.pad(options.outline as usize)?;
        }
        if options.outline >= 1 {
            image.outline(1);
        }
        if options.outline >= 2 {
            image.outline(0xffffff);
        }
        if options.shadow != 0 {
            image.shadow(options.shadow as u32);
        }
        Ok(image)
    }
    fn flip_vertical(&mut self) {
        let [w, h] = self.size;
        for y in 0..h / 2 {
            for x in 0..w {
                self.pixels.swap(y * w + x, (h - 1 - y) * w + x);
            }
        }
        self.offset[1] = self.inner[1] - h - self.offset[1];
    }
    fn flip_horizontal(&mut self) {
        let [w, h] = self.size;
        for y in 0..h {
            self.pixels[y * w..(y + 1) * w].reverse();
        }
        self.offset[0] = self.inner[0] - w - self.offset[0];
    }
    fn pad(&mut self, padding: usize) -> Result<(), String> {
        if self.size == self.inner {
            return Ok(());
        }
        let left = padding.min(self.offset[0]);
        let top = padding.min(self.offset[1]);
        let right = padding.min(self.inner[0] - self.offset[0] - self.size[0]);
        let bottom = padding.min(self.inner[1] - self.offset[1] - self.size[1]);
        let [w, h] = [self.size[0] + left + right, self.size[1] + top + bottom];
        let count = w
            .checked_mul(h)
            .filter(|&n| n <= 4 * 1024 * 1024)
            .ok_or("Widget sprite padding exceeds pixel limit")?;
        let mut pixels = vec![0; count];
        for y in 0..self.size[1] {
            let from = y * self.size[0];
            let to = (y + top) * w + left;
            pixels[to..to + self.size[0]].copy_from_slice(&self.pixels[from..from + self.size[0]]);
        }
        self.pixels = pixels;
        self.size = [w, h];
        self.offset[0] -= left;
        self.offset[1] -= top;
        Ok(())
    }
    fn outline(&mut self, color: u32) {
        let [w, h] = self.size;
        let mut pixels = self.pixels.clone();
        for y in 0..h {
            for x in 0..w {
                let i = y * w + x;
                if self.pixels[i] == 0
                    && ((x > 0 && self.pixels[i - 1] != 0)
                        || (y > 0 && self.pixels[i - w] != 0)
                        || (x + 1 < w && self.pixels[i + 1] != 0)
                        || (y + 1 < h && self.pixels[i + w] != 0))
                {
                    pixels[i] = color;
                }
            }
        }
        self.pixels = pixels;
    }
    fn shadow(&mut self, color: u32) {
        let [w, h] = self.size;
        for y in (1..h).rev() {
            for x in (1..w).rev() {
                let i = y * w + x;
                if self.pixels[i] == 0 && self.pixels[i - w - 1] != 0 {
                    self.pixels[i] = color;
                }
            }
        }
    }
    /// Original GlSprite.renderResized integer destination bounds. UVs still
    /// address the complete trimmed image, as in the original GL draw call.
    pub fn resized_rect(&self, rect: [i32; 4]) -> Result<Option<[i32; 4]>, String> {
        resized_rect(self.inner, self.offset, self.size, rect)
    }
    /// Original power-of-two GL backing, including transparent trailing pixels.
    pub fn backing_rgba(&self) -> Result<([usize; 2], Vec<u8>), String> {
        let size = self
            .size
            .map(|n| if n == 0 { 0 } else { n.next_power_of_two() });
        let count = size[0]
            .checked_mul(size[1])
            .filter(|&n| n <= 4 * 1024 * 1024)
            .ok_or("Widget backing exceeds pixel limit")?;
        let rgba = self.rgba();
        let mut pixels = vec![0; count * 4];
        for y in 0..self.size[1] {
            let from = y * self.size[0] * 4;
            let to = y * size[0] * 4;
            pixels[to..to + self.size[0] * 4].copy_from_slice(&rgba[from..from + self.size[0] * 4]);
        }
        Ok((size, pixels))
    }
    pub fn rgba(&self) -> Vec<u8> {
        self.pixels
            .iter()
            .flat_map(|&p| {
                [
                    (p >> 16) as u8,
                    (p >> 8) as u8,
                    p as u8,
                    if self.alpha {
                        (p >> 24) as u8
                    } else if p == 0 {
                        0
                    } else {
                        255
                    },
                ]
            })
            .collect()
    }
}

pub fn resized_rect(
    inner: [usize; 2],
    offset: [usize; 2],
    size: [usize; 2],
    rect: [i32; 4],
) -> Result<Option<[i32; 4]>, String> {
    let [mut x, mut y, mut w, mut h] = rect;
    if w <= 0 || h <= 0 {
        return Ok(None);
    }
    let div = |a: i32, b: i32| {
        if b == 0 {
            Err("Widget resize scale is zero".to_string())
        } else {
            Ok(a.wrapping_div(b))
        }
    };
    let sx = div((inner[0] as i32).wrapping_shl(16), w)?;
    let sy = div((inner[1] as i32).wrapping_shl(16), h)?;
    let (mut ox, mut oy) = (0i32, 0i32);
    if offset[0] > 0 {
        let shifted = (offset[0] as i32).wrapping_shl(16);
        let adj = div(shifted.wrapping_add(sx).wrapping_sub(1), sx)?;
        x = x.wrapping_add(adj);
        ox = adj.wrapping_mul(sx).wrapping_sub(shifted);
    }
    if offset[1] > 0 {
        let shifted = (offset[1] as i32).wrapping_shl(16);
        let adj = div(shifted.wrapping_add(sy).wrapping_sub(1), sy)?;
        y = y.wrapping_add(adj);
        oy = adj.wrapping_mul(sy).wrapping_sub(shifted);
    }
    if size[0] < inner[0] {
        w = div(
            (size[0] as i32)
                .wrapping_shl(16)
                .wrapping_add(sx)
                .wrapping_sub(ox)
                .wrapping_sub(1),
            sx,
        )?;
    }
    if size[1] < inner[1] {
        h = div(
            (size[1] as i32)
                .wrapping_shl(16)
                .wrapping_add(sy)
                .wrapping_sub(oy)
                .wrapping_sub(1),
            sy,
        )?;
    }
    Ok(Some([x, y, w, h]))
}

#[derive(Clone, Copy, Debug)]
pub struct DrawInput {
    pub if3: bool,
    pub tiling: bool,
    pub angle: i32,
    pub inner: [i32; 2],
    pub size: [i32; 2],
    pub rect: [i32; 4],
    pub alpha: i32,
    pub active: bool,
    /// Original inherited raster clip, in left/top/right/bottom order.
    pub clip: [i32; 4],
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Draw {
    Sprite(bool),
    Clip([i32; 4]),
    Natural([i32; 2]),
    AlphaNatural([i32; 3]),
    Resized([i32; 4]),
    AlphaResize([i32; 5]),
    /// Original argument order: y, angle, zoom, x.
    Rotated([i32; 4]),
    Tiled([i32; 4]),
    AlphaTile([i32; 5]),
    /// Native plugin rendering remains a caller service.
    Plugin([i32; 2]),
}
/// Complete original type5 GL routing for an already-ready ordinary sprite.
/// Image selection/availability and concrete plugin callbacks are caller services.
pub fn draw_commands(input: DrawInput, mut emit: impl FnMut(Draw)) -> Result<(), String> {
    let DrawInput {
        if3,
        tiling,
        angle,
        inner: [iw, ih],
        size: [sw, sh],
        rect: [x, y, w, h],
        alpha,
        active,
        clip,
    } = input;
    emit(Draw::Sprite(!if3 && active));
    if !if3 {
        emit(Draw::Natural([x, y]));
        return Ok(());
    }
    if iw <= 0 || ih <= 0 {
        return Err("Widget sprite inner size is zero".into());
    }
    let opacity = 256 - (alpha & 255);
    if tiling {
        let nx = iw.wrapping_add(w).wrapping_sub(1).wrapping_div(iw);
        let ny = ih.wrapping_add(h).wrapping_sub(1).wrapping_div(ih);
        emit(Draw::Clip([x, y, x.wrapping_add(w), y.wrapping_add(h)]));
        let px = sw > 0 && (sw as u32).is_power_of_two();
        let py = sh > 0 && (sh as u32).is_power_of_two();
        let cols = if px { 1 } else { nx };
        let rows = if py { 1 } else { ny };
        if i64::from(cols) * i64::from(rows) > 131072 {
            emit(Draw::Clip(clip));
            return Err("Widget tile draw limit exceeded".into());
        }
        for col in 0..cols {
            for row in 0..rows {
                let a = x.wrapping_add(col.wrapping_mul(iw));
                let b = y.wrapping_add(row.wrapping_mul(ih));
                if px || py {
                    let tx = if px { nx } else { 1 };
                    let ty = if py { ny } else { 1 };
                    if alpha == 0 {
                        emit(Draw::Tiled([a, b, tx, ty]));
                    } else {
                        emit(Draw::AlphaTile([a, b, opacity, tx, ty]));
                    }
                } else if alpha == 0 {
                    emit(Draw::Natural([a, b]));
                } else {
                    emit(Draw::AlphaNatural([a, b, opacity]));
                }
            }
        }
        emit(Draw::Plugin(if !px && !py {
            [x.wrapping_add(iw), y.wrapping_add(ih)]
        } else {
            [x, y]
        }));
        emit(Draw::Clip(clip));
    } else {
        let zoom = w.wrapping_mul(4096).wrapping_div(iw);
        if angle != 0 {
            emit(Draw::Rotated([
                y.wrapping_add(h / 2),
                angle,
                zoom,
                x.wrapping_add(w / 2),
            ]));
        } else if alpha != 0 {
            emit(Draw::AlphaResize([x, y, w, h, opacity]));
        } else if iw == w && ih == h {
            emit(Draw::Natural([x, y]));
        } else {
            emit(Draw::Resized([x, y, w, h]));
        }
        emit(Draw::Plugin([x, y]));
    }
    Ok(())
}
