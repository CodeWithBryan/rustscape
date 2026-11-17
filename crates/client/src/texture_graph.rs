//! RT4 fixed-point texture graph interpreter. Image evaluation is independent of the GPU.
use crate::{pack::Pack, texture::Sprite};
use rs09_protocol::{Error, Result, buffer::Reader};
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

mod sources;

type Pixels = Rc<Vec<[i32; 3]>>;
#[derive(Clone, Debug)]
struct Op {
    kind: u8,
    cache_height: usize,
    mono: bool,
    p: [i32; 10],
    points: Vec<Vec<i32>>,
    inputs: Vec<usize>,
}
#[derive(Debug)]
pub struct Graph {
    ops: Vec<Op>,
    color: usize,
    alpha: usize,
    pub transpose: bool,
    pub wrap: [bool; 2],
    pub scroll: [i8; 2],
    pub blend_mode: u8,
    pub blurred: bool,
    /// `GlTexture.mipmapMode`: 0 none, 1 regenerated per level, 2 generated.
    pub mipmap: u8,
}
impl Graph {
    pub fn decode(data: &[u8]) -> Result<Self> {
        let mut r = Reader::new(data);
        let count = r.g1()? as usize;
        let mut ops = Vec::new();
        for _ in 0..count {
            r.g1()?;
            let kind = r.g1()? as u8;
            let cache_height = r.g1()? as usize;
            let n = r.g1()?;
            let (input_count, mono, defaults): (usize, bool, &[i32]) = match kind {
                0 => (0, true, &[4096]),
                1 => (0, false, &[0]),
                2 | 3 | 13 => (0, true, &[]),
                4 => (0, true, &[4, 8, 409, 204, 1024, 0, 81, 1024]),
                5 => (1, false, &[1, 1]),
                6 => (1, false, &[0, 4096]),
                7 => (2, false, &[6]),
                8 => (1, true, &[0]),
                9 => (1, false, &[1, 1]),
                10 => (1, false, &[1]),
                11 => (1, false, &[4096, 4096, 4096]),
                12 => (0, true, &[0, 0, 1]),
                14 => (0, true, &[585]),
                15 => (0, true, &[5, 0, 2048, 2, 1, 5]),
                16 => (0, true, &[1, 1, 204]),
                17 => (1, false, &[0, 0, 0]),
                18 | 36 | 39 => (0, false, &[-1]),
                19 => (3, false, &[32768]),
                20 => (1, false, &[4, 4]),
                21 => (3, false, &[]),
                22 => (1, false, &[]),
                24 => (1, true, &[]),
                26 => (1, true, &[0, 4096]),
                27 => (0, true, &[10, 2048, 0]),
                28 => (0, true, &[0, 1024, 2048, 409, 819, 1024, 0, 1024, 1024]),
                29 => (0, true, &[]),
                30 => (1, false, &[1024, 3072]),
                32 => (1, true, &[4096, 3216, 3216]),
                34 => (0, true, &[1, 4, 1638, 4, 0, 4]),
                38 => (0, true, &[0, 2000, 16, 0, 4096]),
                _ => {
                    return Err(Error::new(
                        r.position() - 3,
                        "unsupported texture operation",
                    ));
                }
            };
            let mut op = Op {
                kind,
                cache_height,
                mono,
                p: [0; 10],
                points: Vec::new(),
                inputs: Vec::new(),
            };
            op.p[..defaults.len()].copy_from_slice(defaults);
            for _ in 0..n {
                let code = r.g1()? as usize;
                match (kind, code) {
                    (0, 0) => op.p[0] = (r.g1()? << 12) / 255,
                    (4, 0..=1) | (38, 0 | 2) => op.p[code] = r.g1()?,
                    (4, 2..=7) | (38, 1 | 3 | 4) => op.p[code] = r.g2()?,
                    (12, 0..=1) | (16, 0..=1) => op.p[code] = r.g1()?,
                    (12, 3) => op.p[2] = r.g1()?,
                    (12, _) => {}
                    (14, 0) | (16, 2) => op.p[code] = r.g2()?,
                    (1, 0) => op.p[0] = r.g3()?,
                    (5 | 9 | 20, 0..=1) | (7, 0) | (27, 0 | 2) => op.p[code] = r.g1()?,
                    (5 | 6 | 9 | 30, 2) | (7 | 19, 1) | (21 | 22, 0) => op.mono = r.g1()? == 1,
                    (6 | 26 | 30, 0..=1) | (11 | 32, 0..=2) | (18 | 36 | 39, 0) | (27, 1) => {
                        op.p[code] = r.g2()?
                    }
                    (19, 0) => op.p[0] = r.g2()? << 4,
                    (28, 0 | 6) => op.p[code] = r.g1()?,
                    (28, 1..=5 | 7..=8) => op.p[code] = r.g2()?,
                    (29, 0) => {
                        let n = r.g1()?;
                        for _ in 0..n {
                            let kind = r.g1()?;
                            if kind != 0 {
                                return Err(Error::new(
                                    r.position() - 1,
                                    "unsupported raster shape",
                                ));
                            }
                            op.points.push(vec![
                                r.g2()? as i16 as i32,
                                r.g2()? as i16 as i32,
                                r.g2()? as i16 as i32,
                                r.g2()? as i16 as i32,
                                r.g3()?,
                                r.g1()?,
                            ]);
                        }
                    }
                    (29, 1) => op.mono = r.g1()? == 1,
                    (8, 0) => {
                        op.p[0] = r.g1()?;
                        let count = r.g1()?;
                        for _ in 0..count {
                            op.points.push(vec![r.g2()?, r.g2()?]);
                        }
                    }
                    (10, 0) => {
                        op.p[0] = r.g1()?;
                        if op.p[0] == 0 {
                            let count = r.g1()?;
                            for _ in 0..count {
                                op.points.push(vec![
                                    r.g2()?,
                                    r.g1()? << 4,
                                    r.g1()? << 4,
                                    r.g1()? << 4,
                                ]);
                            }
                        }
                    }
                    (15, 0) => {
                        op.p[0] = r.g1()?;
                        op.p[5] = op.p[0];
                    }
                    (15, 1 | 3 | 4) => op.p[code] = r.g1()?,
                    (15, 2) => op.p[2] = r.g2()?,
                    (15, 5) => op.p[0] = r.g1()?,
                    (15, 6) => op.p[5] = r.g1()?,
                    (17, 0) => op.p[0] = r.g2()? as i16 as i32,
                    (17, 1..=2) => op.p[code] = ((r.g1()? as i8 as i32) << 12) / 100,
                    (34, 0 | 1 | 4) => op.p[code] = r.g1()?,
                    (34, 2) => {
                        op.p[2] = r.g2()? as i16 as i32;
                        if op.p[2] < 0 {
                            for _ in 0..op.p[1] {
                                op.points.push(vec![r.g2()? as i16 as i32]);
                            }
                        }
                    }
                    (34, 3) => {
                        op.p[3] = r.g1()?;
                        op.p[5] = op.p[3];
                    }
                    (34, 5) => op.p[3] = r.g1()?,
                    (34, 6) => op.p[5] = r.g1()?,
                    _ => {
                        return Err(Error::new(
                            r.position() - 1,
                            "unsupported texture parameter",
                        ));
                    }
                }
            }
            for _ in 0..input_count {
                op.inputs.push(r.g1()? as usize);
            }
            ops.push(op);
        }
        let color = r.g1()? as usize;
        let alpha = r.g1()? as usize;
        let blurred = r.g1()? == 1;
        let transpose = r.g1()? == 1;
        let wrap = [r.g1()? == 1, r.g1()? == 1];
        let blend_mode = r.g1()? as u8 & 3;
        let scroll = [r.g1()? as i8, r.g1()? as i8];
        let mipmap = r.g1()? as u8 >> 4 & 15;
        r.g1()?;
        Ok(Self {
            ops,
            color,
            alpha,
            transpose,
            wrap,
            scroll,
            blend_mode,
            blurred,
            mipmap,
        })
    }
    fn source(
        &self,
        pack: &Pack,
        index: usize,
        size: usize,
    ) -> std::result::Result<Pixels, String> {
        let op = self.ops.get(index).ok_or("invalid texture node")?;
        if !op.inputs.is_empty() {
            return Err("texture source has inputs".into());
        }
        let p = op.p;
        let mut out = vec![[0; 3]; size * size];
        let mask = size as i32 - 1;
        let fraction = |v: usize| ((v << 12) / size) as i32;
        match op.kind {
            0 => out.fill([p[0]; 3]),
            1 => out.fill([(p[0] >> 12) & 4080, (p[0] >> 4) & 4080, (p[0] & 255) << 4]),
            2 | 3 => {
                for y in 0..size {
                    for x in 0..size {
                        out[y * size + x] = [fraction(if op.kind == 2 { x } else { y }); 3];
                    }
                }
            }
            4 => out = brickwork(p, size)?,
            13 => {
                for y in 0..size {
                    for x in 0..size {
                        let h = fraction(x).wrapping_add(fraction(y).wrapping_mul(57));
                        let h = h ^ (h << 1);
                        let v = h
                            .wrapping_mul(
                                h.wrapping_mul(h).wrapping_mul(15731).wrapping_add(789221),
                            )
                            .wrapping_add(1376312589)
                            & i32::MAX;
                        out[y * size + x] = [(4096 - v / 262144) % 4096; 3];
                    }
                }
            }
            12 | 14 | 16 => {
                for y in 0..size {
                    for x in 0..size {
                        let (fx, fy) = (fraction(x), fraction(y));
                        let value = match op.kind {
                            12 => {
                                let mut v = if p[0] == 0 {
                                    (fx - fy) * p[2]
                                } else {
                                    let hx = (fx - 2048) >> 1;
                                    let hy = (fy - 2048) >> 1;
                                    let sq = (hx * hx + hy * hy) >> 12;
                                    let r = (f64::from(sq as f32 / 4096.0).sqrt() * 4096.0) as i32;
                                    (f64::from(r * p[2]) * std::f64::consts::PI) as i32
                                };
                                v &= 4095;
                                match p[1] {
                                    0 => (trig(v >> 4).0 + 4096) >> 1,
                                    2 => (2048 - (v - 2048).abs()) << 1,
                                    _ => v,
                                }
                            }
                            14 => {
                                let w = p[0];
                                if fx > w && 4096 - w > fx && 2048 - w < fy && fy < w + 2048 {
                                    4096 - ((2048 - fx).abs() << 12) / (2048 - w)
                                } else if fx > 2048 - w && fx < w + 2048 {
                                    (((fy - 2048).abs() - w) << 12) / (2048 - w)
                                } else if fy < w || 4096 - w < fy {
                                    (((fx - 2048).abs() - w) << 12) / (2048 - w)
                                } else if w <= fx && fx <= 4096 - w {
                                    0
                                } else {
                                    4096 - ((2048 - fy).abs() << 12) / (2048 - w)
                                }
                            }
                            16 => {
                                if p[0] == 0 || p[1] == 0 {
                                    return Err("zero grid count".into());
                                }
                                let cx = (p[0] * fx) >> 12;
                                let cy = (p[1] * fy) >> 12;
                                let rx = p[0] * (fx % (4096 / p[0]));
                                let ry = (fy % (4096 / p[1])) * p[1];
                                let cell = (cx - cy).rem_euclid(4);
                                if (ry < p[2] && (cell != 1 || rx < p[2]))
                                    || (rx < p[2] && cell > 0)
                                {
                                    0
                                } else {
                                    4096
                                }
                            }
                            _ => unreachable!(),
                        };
                        out[y * size + x] = [value; 3];
                    }
                }
            }
            15 => out = voronoi(p, size),
            18 | 39 => {
                if p[0] >= 0 {
                    let sprite = Sprite::decode(pack.get(&format!("sprite/{}", p[0]))?)
                        .map_err(|e| e.to_string())?;
                    for y in 0..size {
                        for x in 0..size {
                            let off = if op.kind == 18 {
                                (y % sprite.height) * sprite.height + x % sprite.width
                            } else {
                                (y * sprite.height / size) * sprite.width + x * sprite.width / size
                            };
                            let c = sprite
                                .rgba
                                .get(off * 4..off * 4 + 3)
                                .ok_or("sprite texture index")?;
                            out[y * size + x] = [
                                i32::from(c[0]) << 4,
                                i32::from(c[1]) << 4,
                                i32::from(c[2]) << 4,
                            ];
                        }
                    }
                }
            }
            27 => {
                if p[0] == 0 {
                    return Err("zero stripe count".into());
                }
                let step = 4096 / p[0];
                let width = (step * p[1]) >> 12;
                for y in 0..size {
                    for x in 0..size {
                        let (x, y) = (fraction(x), fraction(y));
                        let coord = match p[2] {
                            0 => y,
                            1 => x,
                            2 => ((x + y - 4096) >> 1) + 2048,
                            3 => ((x - y) >> 1) + 2048,
                            _ => 0,
                        };
                        let value = if (0..p[0]).any(|i| {
                            coord >= i * step
                                && coord < if i == p[0] - 1 { 4096 } else { (i + 1) * step }
                                && coord < i * step + width
                        }) {
                            4096
                        } else {
                            0
                        };
                        out[(y as usize * size / 4096) * size + x as usize * size / 4096] =
                            [value; 3];
                    }
                }
            }
            28 => out = sources::bark(p, size)?,
            29 => out = sources::shapes(&op.points, size, op.mono),
            38 => {
                let mut rng = Random::new(p[0]);
                for _ in 0..p[1] {
                    let angle = if p[4] > 0 {
                        p[3] + rng.bounded(p[4]) - (p[4] >> 1)
                    } else {
                        p[3]
                    };
                    let mut x0 = rng.bounded(size as i32);
                    let (s, c) = trig(angle >> 4);
                    let mut y0 = rng.bounded(size as i32);
                    let mut x1 = x0 + ((p[2] * c) >> 12);
                    let mut y1 = y0 + ((s * p[2]) >> 12);
                    let dx = (x1 - x0).abs();
                    let dy = (y1 - y0).abs();
                    if dx == 0 && dy == 0 {
                        continue;
                    }
                    let steep = dy > dx;
                    if steep {
                        std::mem::swap(&mut x0, &mut y0);
                        std::mem::swap(&mut x1, &mut y1);
                    }
                    if x0 > x1 {
                        std::mem::swap(&mut x0, &mut x1);
                        std::mem::swap(&mut y0, &mut y1);
                    }
                    let dx = x1 - x0;
                    let dy = (y1 - y0).abs();
                    let mut y = y0;
                    let mut error = -dx / 2;
                    let base = 1024 - (rng.bounded(4096) >> 2);
                    let step = if y1 <= y0 { -1 } else { 1 };
                    let intensity = 2048 / dx;
                    for x in x0..x1 {
                        error += dy;
                        let value = intensity * (x - x0) + base + 1024;
                        let my = y & mask;
                        if error > 0 {
                            error -= dx;
                            y += step;
                        }
                        let mx = x & mask;
                        let index = if steep {
                            my as usize * size + mx as usize
                        } else {
                            mx as usize * size + my as usize
                        };
                        out[index] = [value; 3];
                    }
                }
            }
            _ => return Err(format!("unsupported texture source {}", op.kind)),
        }
        Ok(Rc::new(out))
    }
    pub fn rgba(
        &self,
        pack: &Pack,
        size: usize,
        brightness: f64,
        flip: bool,
        depth: usize,
    ) -> std::result::Result<Vec<u8>, String> {
        if size == 0 || !size.is_power_of_two() || size > 256 {
            return Err("invalid texture size".into());
        }
        if depth > 32 {
            return Err("cyclic texture dependency".into());
        }
        let mut evaluator = Evaluator::new(self, pack, size, brightness, depth);
        let mut rgba = vec![0; size * size * 4];
        for y in 0..size {
            let color = evaluator.row(self.color, y, 0)?;
            // getPixels (the non-flipped dependency path) never evaluates alpha.
            let alpha = if flip {
                Some(evaluator.row(self.alpha, y, 0)?)
            } else {
                None
            };
            let mut gamma = [0u8; 256];
            for (i, v) in gamma.iter_mut().enumerate() {
                *v = ((i as f64 / 255.0).powf(evaluator.brightness) * 255.0).min(255.0) as u8;
            }
            let color = color.borrow();
            let alpha = alpha.as_ref().map(|a| a.borrow());
            for x in 0..size {
                let sx = if flip { size - 1 - x } else { x };
                let dest = if self.transpose {
                    x * size + y
                } else {
                    y * size + x
                };
                let c = color[sx].map(|v| gamma[(v >> 4).clamp(0, 255) as usize]);
                let a = if c == [0; 3] {
                    0
                } else {
                    alpha
                        .as_ref()
                        .map_or(255, |a| (a[sx][0] >> 4).clamp(0, 255) as u8)
                };
                rgba[dest * 4..dest * 4 + 4].copy_from_slice(&[c[0], c[1], c[2], a]);
            }
        }
        Ok(rgba)
    }
    /// GlTexture.getAnimatedPixels: reverse X, omit alpha, and optionally blur.
    pub fn software_pixels(
        &self,
        pack: &Pack,
        size: usize,
        brightness: f64,
    ) -> std::result::Result<Vec<u32>, String> {
        if size != 64 && size != 128 {
            return Err("Invalid software texture size".into());
        }
        let mut evaluator = Evaluator::new(self, pack, size, brightness, 0);
        let mut pixels = vec![0u32; size * size];
        let gamma: Vec<u32> = (0..256)
            .map(|i| ((i as f64 / 255.0).powf(brightness) * 255.0).min(255.0) as u32)
            .collect();
        for y in 0..size {
            let color = evaluator.row(self.color, y, 0)?;
            let row = color.borrow();
            for x in 0..size {
                let c = row[size - 1 - x].map(|v| gamma[(v >> 4).clamp(0, 255) as usize]);
                let dest = if self.transpose {
                    x * size + y
                } else {
                    y * size + x
                };
                pixels[dest] = c[0] << 16 | c[1] << 8 | c[2];
            }
        }
        if self.blurred {
            let mut out = vec![0; pixels.len()];
            for y in 0..size {
                for x in 0..size {
                    let mut sums = [0u32; 3];
                    for dy in [-1i32, 0, 1] {
                        for dx in [-1i32, 0, 1] {
                            let value = pixels[((y as i32 + dy) & (size as i32 - 1)) as usize
                                * size
                                + ((x as i32 + dx) & (size as i32 - 1)) as usize];
                            sums[0] += value >> 16 & 255;
                            sums[1] += value >> 8 & 255;
                            sums[2] += value & 255;
                        }
                    }
                    out[y * size + x] =
                        ((sums[0] / 9) << 16) | ((sums[1] / 9) << 8) | (sums[2] / 9);
                }
            }
            pixels = out;
        }
        Ok(pixels)
    }
}

type Row = Rc<RefCell<Vec<[i32; 3]>>>;
struct RowCache {
    capacity: usize,
    rows: Vec<Option<usize>>,
    buffers: Vec<Row>,
    recent: Vec<usize>,
}
impl RowCache {
    fn new(op: &Op, size: usize) -> Self {
        Self {
            capacity: if op.cache_height == 255 {
                size
            } else {
                op.cache_height.max(1)
            },
            rows: vec![None; size],
            buffers: Vec::new(),
            recent: Vec::new(),
        }
    }
    fn get(&mut self, y: usize, width: usize) -> (Row, bool) {
        if let Some(slot) = self.rows[y] {
            self.recent.retain(|&n| n != slot);
            self.recent.push(slot);
            return (self.buffers[slot].clone(), false);
        }
        let slot = if self.buffers.len() < self.capacity {
            let slot = self.buffers.len();
            self.buffers
                .push(Rc::new(RefCell::new(vec![[0; 3]; width])));
            slot
        } else {
            let slot = self.recent.remove(0);
            for row in &mut self.rows {
                if *row == Some(slot) {
                    *row = None;
                }
            }
            slot
        };
        self.rows[y] = Some(slot);
        self.recent.push(slot);
        (self.buffers[slot].clone(), true)
    }
}
/// RT4 row caches are observable: a child can overwrite a previously returned
/// row while evaluating another input, and noise can accumulate in a reused row.
/// Shared mutable row buffers preserve both behaviors without cache-specific hacks.
struct Evaluator<'a> {
    graph: &'a Graph,
    pack: &'a Pack,
    size: usize,
    depth: usize,
    brightness: f64,
    caches: Vec<RowCache>,
    generated: BTreeMap<usize, Pixels>,
    hsl_previous: Vec<[i32; 3]>,
    curves: BTreeMap<usize, [i32; 257]>,
    gradients: BTreeMap<usize, [[i32; 3]; 257]>,
}
impl<'a> Evaluator<'a> {
    fn new(graph: &'a Graph, pack: &'a Pack, size: usize, brightness: f64, depth: usize) -> Self {
        Self {
            graph,
            pack,
            size,
            brightness,
            depth,
            caches: graph.ops.iter().map(|o| RowCache::new(o, size)).collect(),
            generated: BTreeMap::new(),
            hsl_previous: vec![[0; 3]; graph.ops.len()],
            curves: BTreeMap::new(),
            gradients: BTreeMap::new(),
        }
    }
    fn row(&mut self, index: usize, y: usize, depth: usize) -> std::result::Result<Row, String> {
        if depth > 128 {
            return Err("cyclic texture graph".into());
        }
        let (out, invalid) = self
            .caches
            .get_mut(index)
            .ok_or("invalid texture node")?
            .get(y, self.size);
        if !invalid {
            return Ok(out);
        }
        let op = self.graph.ops[index].clone();
        let p = op.p;
        let size = self.size;
        let mask = size - 1;
        let child =
            |this: &mut Self, which: usize, row: usize| this.row(op.inputs[which], row, depth + 1);
        let sample = |row: &Row, x: usize, a: usize| row.borrow()[x][if op.mono { 0 } else { a }];
        match op.kind {
            5 => {
                let inv_x = 65536 / (p[0] * 2 + 1);
                let inv_y = 65536 / (p[1] * 2 + 1);
                let mut rows = Vec::new();
                for dy in -p[1]..=p[1] {
                    let src = child(self, 0, (y as i32 + dy) as usize & mask)?;
                    let mut row = vec![[0; 3]; size];
                    for (x, v) in row.iter_mut().enumerate() {
                        for (a, v) in v.iter_mut().enumerate() {
                            let sum = (-p[0]..=p[0]).fold(0i32, |sum, dx| {
                                sum.wrapping_add(sample(&src, (x as i32 + dx) as usize & mask, a))
                            });
                            *v = sum.wrapping_mul(inv_x) >> 16;
                        }
                    }
                    rows.push(row);
                }
                for x in 0..size {
                    for a in 0..3 {
                        out.borrow_mut()[x][a] = rows
                            .iter()
                            .fold(0i32, |sum, row| sum.wrapping_add(row[x][a]))
                            .wrapping_mul(inv_y)
                            >> 16;
                    }
                }
            }
            6 | 7 | 11 | 21 | 22 | 24 | 26 | 30 => {
                let mut inputs = Vec::new();
                for i in 0..op.inputs.len() {
                    inputs.push(child(self, i, y)?);
                }
                for x in 0..size {
                    let c = inputs[0].borrow()[x];
                    for a in 0..3 {
                        let v = sample(&inputs[0], x, a);
                        out.borrow_mut()[x][a] = match op.kind {
                            6 => {
                                if v < p[0] {
                                    p[0]
                                } else if v > p[1] {
                                    p[1]
                                } else {
                                    v
                                }
                            }
                            7 => combine(p[0], v, sample(&inputs[1], x, a)),
                            11 => {
                                if c[0] == c[1] && c[1] == c[2] {
                                    v.wrapping_mul(p[a]) >> 12
                                } else {
                                    p[a]
                                }
                            }
                            21 => {
                                let t = inputs[2].borrow()[x][0];
                                t.wrapping_mul(v)
                                    .wrapping_add((4096 - t).wrapping_mul(sample(&inputs[1], x, a)))
                                    >> 12
                            }
                            22 => 4096 - v,
                            24 => (c[0] + c[1] + c[2]) / 3,
                            26 => {
                                if v >= p[0] && v <= p[1] {
                                    4096
                                } else {
                                    0
                                }
                            }
                            30 => p[0] + (v.wrapping_mul(p[1] - p[0]) >> 12),
                            _ => unreachable!(),
                        };
                    }
                }
            }
            8 => {
                let src = child(self, 0, y)?;
                if let std::collections::btree_map::Entry::Vacant(entry) = self.curves.entry(index)
                {
                    entry.insert(curve_table(&op)?);
                }
                let table = &self.curves[&index];
                for x in 0..size {
                    out.borrow_mut()[x] =
                        [table[(src.borrow()[x][0] >> 4).clamp(0, 256) as usize]; 3];
                }
            }
            10 => {
                let src = child(self, 0, y)?;
                if let std::collections::btree_map::Entry::Vacant(entry) =
                    self.gradients.entry(index)
                {
                    entry.insert(gradient_table(&op)?);
                }
                let table = &self.gradients[&index];
                for x in 0..size {
                    out.borrow_mut()[x] = table[(src.borrow()[x][0] >> 4).clamp(0, 256) as usize];
                }
            }
            9 => {
                let sy = if p[1] == 1 { size - 1 - y } else { y };
                let src = child(self, 0, sy)?;
                for x in 0..size {
                    let sx = if p[0] == 1 { size - 1 - x } else { x };
                    for a in 0..3 {
                        out.borrow_mut()[x][a] = sample(&src, sx, a);
                    }
                }
            }
            17 => {
                let src = child(self, 0, y)?;
                for x in 0..size {
                    let c = hsl_adjust(src.borrow()[x], p, self.hsl_previous[index]);
                    self.hsl_previous[index] = c;
                    out.borrow_mut()[x] = c;
                }
            }
            19 => {
                let angle = child(self, 1, y)?;
                let radius = child(self, 2, y)?;
                for x in 0..size {
                    let a = if op.mono {
                        angle.borrow()[x][0] >> 4
                    } else {
                        angle.borrow()[x][0].wrapping_mul(255) >> 12
                    };
                    let r = radius.borrow()[x][0].wrapping_mul(p[0]) >> 12;
                    let (s, c) = trig(a);
                    let dx = r.wrapping_mul(c) >> 12;
                    let dy = r.wrapping_mul(s) >> 12;
                    let sx = ((dx >> 12) + x as i32) as usize & mask;
                    let sy = ((dy >> 12) + y as i32) as usize & mask;
                    let src = child(self, 0, sy)?;
                    for a in 0..3 {
                        out.borrow_mut()[x][a] = sample(&src, sx, a);
                    }
                }
            }
            20 => {
                if p[0] == 0 || p[1] == 0 {
                    return Err("zero texture tiling".into());
                }
                let tw = size / p[0] as usize;
                let th = size / p[1] as usize;
                #[allow(clippy::manual_checked_ops)]
                let sy = if th == 0 { 0 } else { y % th * size / th };
                let src = child(self, 0, sy)?;
                for x in 0..size {
                    #[allow(clippy::manual_checked_ops)]
                    let sx = if tw == 0 { 0 } else { x % tw * size / tw };
                    for a in 0..3 {
                        out.borrow_mut()[x][a] = sample(&src, sx, a);
                    }
                }
            }
            32 => {
                let prev = child(self, 0, y.wrapping_sub(1) & mask)?;
                let cur = child(self, 0, y)?;
                let next = child(self, 0, (y + 1) & mask)?;
                let elev = f64::from(p[2] as f32 / 4096.0);
                let az = f64::from(p[1] as f32 / 4096.0);
                let mut light = [
                    (elev.cos() * 4096.0 * az.sin()) as i32,
                    (az.cos() * elev.cos() * 4096.0) as i32,
                    (elev.sin() * 4096.0) as i32,
                ];
                let length = (((((light[0] * light[0]) >> 12)
                    + ((light[1] * light[1]) >> 12)
                    + ((light[2] * light[2]) >> 12))
                    >> 12) as f64)
                    .sqrt();
                let length = (length * 4096.0) as i32;
                if length != 0 {
                    for v in &mut light {
                        *v = (*v << 12) / length;
                    }
                }
                let strength = ((if size == 64 { 2048 } else { 4096 }) * p[0]) >> 12;
                let lookup = normal_lookup();
                for x in 0..size {
                    let dx = (cur.borrow()[x.wrapping_sub(1) & mask][0]
                        - cur.borrow()[(x + 1) & mask][0])
                        .wrapping_mul(strength)
                        >> 12;
                    let dy =
                        (next.borrow()[x][0] - prev.borrow()[x][0]).wrapping_mul(strength) >> 12;
                    let ax = (dx >> 4).abs().min(255);
                    let ay = (dy >> 4).abs().min(255);
                    let nz = lookup[(((ay * (ay + 1)) >> 1) + ax) as usize];
                    let nx = (nz * dx) >> 8;
                    let ny = (dy * nz) >> 8;
                    out.borrow_mut()[x] = [((light[0] * nx) >> 12)
                        + ((ny * light[1]) >> 12)
                        + ((((nz * 4096) >> 8) * light[2]) >> 12);
                        3];
                }
            }
            34 => {
                self.generated.entry(index).or_insert_with(|| {
                    let mut raw = op.clone();
                    raw.p[0] = 0;
                    raw.cache_height = 255;
                    Rc::new(perlin(&raw, size))
                });
                let overwrite = op.p[1] == 1
                    || if op.p[2] > 0 {
                        true
                    } else {
                        op.points[0][0].abs() > 8
                    };
                let raw = &self.generated[&index];
                for x in 0..size {
                    let v = raw[y * size + x][0].wrapping_add(if overwrite {
                        0
                    } else {
                        out.borrow()[x][0]
                    });
                    out.borrow_mut()[x] = [if p[0] == 1 { (v >> 1) + 2048 } else { v }; 3];
                }
            }
            36 => {
                if let std::collections::btree_map::Entry::Vacant(e) = self.generated.entry(index) {
                    let graph = Graph::decode(self.pack.get(&format!("texture/{}", p[0]))?)
                        .map_err(|e| e.to_string())?;
                    let materials = crate::texture::materials(self.pack.get("materials")?)
                        .map_err(|e| e.to_string())?;
                    let input_size = if materials[p[0] as usize]
                        .as_ref()
                        .is_some_and(crate::texture::low_detail)
                    {
                        64
                    } else {
                        128
                    };
                    self.brightness = 1.0;
                    let input = graph.rgba(self.pack, input_size, 1.0, false, self.depth + 1)?;
                    let mut pixels = vec![[0; 3]; size * size];
                    for y in 0..size {
                        for x in 0..size {
                            let i =
                                ((y * input_size / size) * input_size + x * input_size / size) * 4;
                            pixels[y * size + x] = [
                                i32::from(input[i]) << 4,
                                i32::from(input[i + 1]) << 4,
                                i32::from(input[i + 2]) << 4,
                            ];
                        }
                    }
                    e.insert(Rc::new(pixels));
                }
                out.borrow_mut()
                    .copy_from_slice(&self.generated[&index][y * size..(y + 1) * size]);
            }
            _ => {
                if !op.inputs.is_empty() {
                    return Err(format!("unsupported row operation {}", op.kind));
                }
                if let std::collections::btree_map::Entry::Vacant(e) = self.generated.entry(index) {
                    let pixels = self.graph.source(self.pack, index, size)?;
                    e.insert(pixels);
                }
                out.borrow_mut()
                    .copy_from_slice(&self.generated[&index][y * size..(y + 1) * size]);
            }
        }
        Ok(out)
    }
}
fn curve_table(op: &Op) -> std::result::Result<[i32; 257], String> {
    let points = if op.points.is_empty() {
        vec![vec![0, 0], vec![4096, 4096]]
    } else {
        op.points.clone()
    };
    if points.len() < 2 {
        return Err("curve requires two markers".into());
    }
    let mut table = [0; 257];
    for (i, v) in table.iter_mut().enumerate() {
        let value = (i as i32) << 4;
        let mut m = 1;
        while m < points.len() - 1 && points[m][0] <= value {
            m += 1;
        }
        let a = &points[m - 1];
        let b = &points[m];
        if a[0] == b[0] {
            return Err("duplicate curve marker".into());
        }
        let t = ((value - a[0]) << 12) / (b[0] - a[0]);
        let result = match op.p[0] {
            1 => {
                let w = (4096 - trig(t >> 5).1) >> 1;
                (w * b[1] + (4096 - w) * a[1]) >> 12
            }
            2 => {
                let before = if m >= 2 {
                    points[m - 2][1]
                } else {
                    2 * points[0][1] - points[1][1]
                };
                let after = if m + 1 < points.len() {
                    points[m + 1][1]
                } else {
                    2 * points[points.len() - 2][1] - points[points.len() - 1][1]
                };
                let ca = after + a[1] - b[1] - before;
                let cb = before - a[1] - ca;
                let t2 = t.wrapping_mul(t) >> 12;
                a[1] + (t2.wrapping_mul(cb) >> 12)
                    + (t2.wrapping_mul(t.wrapping_mul(ca) >> 12) >> 12)
                    + ((b[1] - before).wrapping_mul(t) >> 12)
            }
            _ => (b[1] * t + (4096 - t) * a[1]) >> 12,
        };
        *v = result.clamp(-32767, 32767);
    }
    Ok(table)
}
fn gradient_table(op: &Op) -> std::result::Result<[[i32; 3]; 257], String> {
    let points = if op.points.is_empty() {
        gradient_preset(op.p[0])?
    } else {
        op.points.clone()
    };
    let mut table = [[0; 3]; 257];
    for (i, pixel) in table.iter_mut().enumerate() {
        let value = (i as i32) << 4;
        let next = points
            .iter()
            .position(|p| p[0] > value)
            .unwrap_or(points.len());
        for a in 0..3 {
            let v = if next == 0 {
                points[0][a + 1]
            } else if next == points.len() {
                points[next - 1][a + 1]
            } else {
                let before = &points[next - 1];
                let after = &points[next];
                let t = ((value - before[0]) << 12) / (after[0] - before[0]);
                ((4096 - t) * before[a + 1] + t * after[a + 1]) >> 12
            };
            pixel[a] = (v >> 4).clamp(0, 255) << 4;
        }
    }
    Ok(table)
}

fn combine(function: i32, a: i32, b: i32) -> i32 {
    match function {
        1 => a.wrapping_add(b),
        2 => a.wrapping_sub(b),
        3 => a.wrapping_mul(b) >> 12,
        4 => {
            if b == 0 {
                4096
            } else {
                a.wrapping_shl(12).wrapping_div(b)
            }
        }
        5 => 4096 - ((4096 - a).wrapping_mul(4096 - b) >> 12),
        6 => {
            if b >= 2048 {
                4096 - ((4096 - a).wrapping_mul(4096 - b) >> 11)
            } else {
                b.wrapping_mul(a) >> 11
            }
        }
        7 => {
            if a == 4096 {
                4096
            } else {
                b.wrapping_shl(12).wrapping_div(4096 - a)
            }
        }
        8 => {
            if a == 0 {
                0
            } else {
                4096 - (4096 - b).wrapping_shl(12).wrapping_div(a)
            }
        }
        9 => a.min(b),
        10 => a.max(b),
        11 => a.wrapping_sub(b).wrapping_abs(),
        12 => b.wrapping_add(a).wrapping_sub(a.wrapping_mul(b) >> 11),
        _ => 0,
    }
}
fn trig(angle: i32) -> (i32, i32) {
    let radians = f64::from(angle & 255) / 255.0 * std::f64::consts::TAU;
    (
        (radians.sin() * 4096.0) as i32,
        (radians.cos() * 4096.0) as i32,
    )
}
pub(crate) fn smooth(x: i32) -> i32 {
    let cubed = x.wrapping_mul(x.wrapping_mul(x) >> 12) >> 12;
    let linear = x * 6 - 61440;
    let factor = (x.wrapping_mul(linear) >> 12) + 40960;
    cubed.wrapping_mul(factor) >> 12
}
struct Random(u64);
impl Random {
    fn new(seed: i32) -> Self {
        Self((seed as u64 ^ 0x5deece66d) & ((1 << 48) - 1))
    }
    fn next(&mut self) -> i32 {
        self.0 = (self.0.wrapping_mul(0x5deece66d).wrapping_add(11)) & ((1 << 48) - 1);
        (self.0 >> 16) as i32
    }
    fn bounded(&mut self, bound: i32) -> i32 {
        if bound.count_ones() == 1 {
            return (((self.next() as u32 as u64) * bound as u64) >> 32) as i32;
        }
        let threshold = i32::MIN.wrapping_sub(((1u64 << 32) % bound as u64) as i32);
        let mut sample = self.next();
        while threshold <= sample {
            sample = self.next();
        }
        let correction = (bound - 1) & (sample >> 31);
        correction + sample.wrapping_add((sample as u32 >> 31) as i32) % bound
    }
}
pub(crate) fn permutation(seed: i32) -> [i32; 512] {
    let mut table = [0; 512];
    for (i, v) in table.iter_mut().enumerate().take(255) {
        *v = i as i32;
    }
    let mut rng = Random::new(seed);
    for i in 0..255 {
        let remaining = 255 - i;
        let j = rng.bounded(remaining as i32) as usize;
        let tmp = table[j];
        table.swap(j, remaining);
        table[511 - i] = tmp;
    }
    table
}
fn perlin(op: &Op, size: usize) -> Vec<[i32; 3]> {
    perlin_rows(op, size, size)
}
pub(crate) fn light_noise() -> Vec<i32> {
    let op = Op {
        kind: 34,
        cache_height: 255,
        mono: true,
        p: [1, 4, 1638, 8, 35, 8, 0, 0, 0, 0],
        points: Vec::new(),
        inputs: Vec::new(),
    };
    perlin_rows(&op, 2048, 1)
        .into_iter()
        .map(|v| v[0])
        .collect()
}
fn perlin_rows(op: &Op, size: usize, rows: usize) -> Vec<[i32; 3]> {
    let p = op.p;
    let perm = permutation(p[4]);
    let mut amplitudes = Vec::new();
    // The octave number is also the exponent.
    #[allow(clippy::needless_range_loop)]
    for i in 0..p[1] as usize {
        amplitudes.push(if p[2] > 0 {
            ((f64::from(p[2] as f32 / 4096.0).powi(i as i32) * 4096.0) as i32) as i16 as i32
        } else {
            op.points[i][0]
        });
    }
    while amplitudes.len() > 1 && amplitudes.last().unwrap().abs() <= 8 {
        amplitudes.pop();
    }
    let mut out = vec![[0; 3]; size * rows];
    let grad = |hash: i32, x: i32, y: i32| match hash & 3 {
        0 => x + y,
        1 => -x + y,
        2 => x - y,
        _ => -x - y,
    };
    let capacity = if op.cache_height == 255 {
        size
    } else {
        op.cache_height.max(1)
    };
    for y in 0..rows {
        for x in 0..size {
            let mut acc = if amplitudes[0].abs() <= 8 && amplitudes.len() > 1 && y >= capacity {
                out[(y - capacity) * size + x][0]
            } else {
                0
            };
            for (i, &amplitude) in amplitudes.iter().enumerate() {
                if amplitude.abs() <= 8 && amplitudes.len() != 1 {
                    continue;
                }
                let freq = 1i32 << i;
                let xp = p[3] * ((x << 12) / size) as i32 * freq;
                let yp = p[5] * ((y << 12) / size) as i32 * freq;
                let cx = (xp >> 12) & 255;
                let cy = (yp >> 12) & 255;
                let nx = if (xp >> 12) + 1 >= p[3] * freq {
                    0
                } else {
                    ((xp >> 12) + 1) & 255
                };
                let ny = if (yp >> 12) + 1 >= p[5] * freq {
                    0
                } else {
                    ((yp >> 12) + 1) & 255
                };
                let xf = xp & 4095;
                let yf = yp & 4095;
                let sx = smooth(xf);
                let sy = smooth(yf);
                let py = perm[cy as usize];
                let pn = perm[ny as usize];
                let a = grad(perm[(cx + py) as usize], xf, yf);
                let b = grad(perm[(nx + py) as usize], xf - 4096, yf);
                let c = grad(perm[(cx + pn) as usize], xf, yf - 4096);
                let d = grad(perm[(nx + pn) as usize], xf - 4096, yf - 4096);
                let top = a + (((b - a) * sx) >> 12);
                let bottom = c + (((d - c) * sx) >> 12);
                let value = top + (((bottom - top) * sy) >> 12);
                acc += (value * amplitude) >> 12;
                if p[0] == 1 && i == amplitudes.len() - 1 {
                    acc = (acc >> 1) + 2048;
                }
            }
            out[y * size + x] = [acc; 3];
        }
    }
    out
}
fn voronoi(p: [i32; 10], size: usize) -> Vec<[i32; 3]> {
    let perm = permutation(p[1]);
    let mut rng = Random::new(p[1]);
    let mut offsets = [0; 512];
    if p[2] > 0 {
        for v in &mut offsets {
            *v = rng.bounded(p[2]) as i16 as i32;
        }
    }
    let mut out = vec![[0; 3]; size * size];
    for y in 0..size {
        let sy = p[5] * ((y << 12) / size) as i32 + 2048;
        let cy = sy >> 12;
        for x in 0..size {
            let sx = p[0] * ((x << 12) / size) as i32 + 2048;
            let cx = sx >> 12;
            let mut nearest = [i32::MAX; 4];
            for iy in cy - 1..=cy + 1 {
                let py = perm[((if iy >= p[5] { iy - p[5] } else { iy }) & 255) as usize];
                for ix in cx - 1..=cx + 1 {
                    let pi = perm[((if ix >= p[0] { ix - p[0] } else { ix }) + py) as usize & 255]
                        as usize
                        * 2;
                    let dx = sx - (ix << 12) - offsets[pi];
                    let dy = sy - (iy << 12) - offsets[pi + 1];
                    let square = dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy));
                    let distance = match p[4] {
                        1 => square >> 12,
                        2 => dx.abs() + dy.abs(),
                        3 => dx.abs().max(dy.abs()),
                        4 => {
                            let ax = (f64::from(dx.abs() as f32 / 4096.0).sqrt() * 4096.0) as i32;
                            let ay = (f64::from(dy.abs() as f32 / 4096.0).sqrt() * 4096.0) as i32;
                            (ax + ay).wrapping_mul(ax + ay) >> 12
                        }
                        5 => (f64::from(square as f32 / 16777216.0).sqrt().sqrt() * 4096.0) as i32,
                        _ => (f64::from(square as f32 / 16777216.0).sqrt() * 4096.0) as i32,
                    };
                    if let Some(i) = nearest.iter().position(|&v| v > distance) {
                        for j in (i + 1..4).rev() {
                            nearest[j] = nearest[j - 1];
                        }
                        nearest[i] = distance;
                    }
                }
            }
            out[y * size + x] = [match p[3] {
                0 => nearest[0],
                1 => nearest[1],
                2 => nearest[1] - nearest[0],
                3 => nearest[2],
                4 => nearest[3],
                _ => 0,
            }; 3];
        }
    }
    out
}
fn normal_lookup() -> &'static [i32] {
    static LOOKUP: std::sync::OnceLock<Vec<i32>> = std::sync::OnceLock::new();
    LOOKUP.get_or_init(|| {
        let mut table = Vec::with_capacity(32896);
        for i in 0..256 {
            for j in 0..=i {
                table.push(
                    (255.0 / f64::from((j * j + i * i + 65535) as f32 / 65535.0).sqrt()) as i32
                        & 255,
                );
            }
        }
        table
    })
}
fn brickwork(p: [i32; 10], size: usize) -> std::result::Result<Vec<[i32; 3]>, String> {
    if p[0] <= 0 || p[1] <= 0 {
        return Err("zero brick count".into());
    }
    let (cols, rows) = (p[0] as usize, p[1] as usize);
    let rh = 4096 / p[1];
    let cw = 4096 / p[0];
    let mortar = p[6] / 2;
    let mut rng = Random::new(p[1]);
    let mut ys = vec![0; rows + 1];
    let mut xs = vec![vec![0; cols + 1]; rows];
    let mut colors = vec![vec![0; cols]; rows];
    for r in 0..rows {
        if r > 0 {
            let jitter = ((rng.bounded(4096) - 2048) * p[3]) >> 12;
            ys[r] = ys[r - 1] + rh + ((jitter * (rh / 2)) >> 12);
        }
        for c in 0..cols {
            if c > 0 {
                let jitter = ((rng.bounded(4096) - 2048) * p[2]) >> 12;
                xs[r][c] = xs[r][c - 1] + cw + (((cw / 2) * jitter) >> 12);
            }
            colors[r][c] = if p[7] <= 0 {
                4096
            } else {
                4096 - rng.bounded(p[7])
            };
        }
        xs[r][cols] = 4096;
    }
    ys[rows] = 4096;
    let wrap = |mut x: i32| {
        while x < 0 {
            x += 4096;
        }
        while x > 4096 {
            x -= 4096;
        }
        x
    };
    let mut out = vec![[0; 3]; size * size];
    for y in 0..size {
        let fy = wrap(((y << 12) / size) as i32 + p[5]);
        let mut ri = 0;
        while ri < rows && fy >= ys[ri] {
            ri += 1;
        }
        if ri == 0 || fy <= ys[ri - 1] + mortar || fy >= ys[ri] - mortar {
            continue;
        }
        for x in 0..size {
            let fx = wrap(
                ((x << 12) / size) as i32 + ((cw * if ri & 1 == 0 { p[4] } else { -p[4] }) >> 12),
            );
            let mut ci = 0;
            while ci < cols && fx >= xs[ri - 1][ci] {
                ci += 1;
            }
            if ci > 0 && fx > xs[ri - 1][ci - 1] + mortar && fx < xs[ri - 1][ci] - mortar {
                out[y * size + x] = [colors[ri - 1][ci - 1]; 3];
            }
        }
    }
    Ok(out)
}
fn hsl_adjust(c: [i32; 3], p: [i32; 10], previous: [i32; 3]) -> [i32; 3] {
    let [r, g, b] = c;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;
    let mut hue = if delta > 0 {
        let dg = ((max - g) << 12) / delta;
        let dr = ((max - r) << 12) / delta;
        let db = ((max - b) << 12) / delta;
        (if r == max {
            if min == g { db + 20480 } else { 4096 - dg }
        } else if g == max {
            if min == b { dr + 4096 } else { 12288 - db }
        } else if min == r {
            dg + 12288
        } else {
            20480 - dr
        }) / 6
    } else {
        0
    };
    let l = (min + max) / 2;
    let s = if l > 0 && l < 4096 {
        (delta << 12) / if l > 2048 { 8192 - l * 2 } else { l * 2 }
    } else {
        0
    };
    let l = (l + p[2]).clamp(0, 4096);
    let s = (s + p[1]).clamp(0, 4096);
    hue += p[0];
    while hue < 0 {
        hue += 4096;
    }
    while hue > 4096 {
        hue -= 4096;
    }
    let q = if l <= 2048 {
        (l * (s + 4096)) >> 12
    } else {
        l + s - ((l * s) >> 12)
    };
    if q <= 0 {
        return [l; 3];
    }
    let h = hue * 6;
    let a = l * 2 - q;
    let sector = h >> 12;
    let chroma = ((q - a) << 12) / q;
    let frac = h - (sector << 12);
    let ramp = (((q * chroma) >> 12) * frac) >> 12;
    match sector {
        0 => [q, ramp + a, a],
        1 => [q - ramp, q, a],
        2 => [a, q, ramp + a],
        3 => [a, q - ramp, q],
        4 => [ramp + a, a, q],
        5 => [q, a, q - ramp],
        _ => previous,
    }
}
fn gradient_preset(id: i32) -> std::result::Result<Vec<Vec<i32>>, String> {
    match id {
        1 => Ok(vec![vec![0, 0, 0, 0], vec![4096, 4096, 4096, 4096]]),
        _ => Err(format!("unsupported gradient preset {id}")),
    }
}
