//! Integer scan conversion for the original non-jagged inventory icon path.
//! The UI oracle compares complete pixel buffers against Rasteriser.fillGouraudTriangle.
pub fn gouraud(
    pixels: &mut [u32],
    size: [usize; 2],
    palette: &[u32],
    points: [[i32; 2]; 3],
    colors: [i32; 3],
    alpha: i32,
) -> Result<(), String> {
    let [[xa, ya], [xb, yb], [xc, yc]] = points;
    let dx_ab = xb.wrapping_sub(xa);
    let dy_ab = yb.wrapping_sub(ya);
    let dx_ac = xc.wrapping_sub(xa);
    let dy_ac = yc.wrapping_sub(ya);
    let length = dx_ab
        .wrapping_mul(dy_ac)
        .wrapping_sub(dx_ac.wrapping_mul(dy_ab));
    if length == 0 {
        return Ok(());
    }
    let dc_ab = colors[1].wrapping_sub(colors[0]);
    let dc_ac = colors[2].wrapping_sub(colors[0]);
    let step_x = dc_ab
        .wrapping_mul(dy_ac)
        .wrapping_sub(dc_ac.wrapping_mul(dy_ab))
        .wrapping_shl(8)
        .wrapping_div(length);
    let step_y = dc_ac
        .wrapping_mul(dx_ab)
        .wrapping_sub(dc_ab.wrapping_mul(dx_ac))
        .wrapping_shl(8)
        .wrapping_div(length);
    let top = if ya <= yb && ya <= yc {
        0
    } else if yb <= yc {
        1
    } else {
        2
    };
    let base = colors[top]
        .wrapping_shl(8)
        .wrapping_add(step_x)
        .wrapping_sub(step_x.wrapping_mul(points[top][0]));
    let start = points[top][1].max(0);
    let end = ya.max(yb).max(yc).min(size[1] as i32);
    for y in start..end {
        let mut xs = [0; 3];
        let mut count = 0;
        for (a, b) in [(0, 1), (1, 2), (0, 2)] {
            let (mut a, mut b) = (points[a], points[b]);
            if a[1] > b[1] {
                std::mem::swap(&mut a, &mut b);
            }
            if a[1] <= y && y < b[1] {
                let slope = b[0]
                    .wrapping_sub(a[0])
                    .wrapping_shl(16)
                    .wrapping_div(b[1].wrapping_sub(a[1]));
                xs[count] = a[0]
                    .wrapping_shl(16)
                    .wrapping_add(slope.wrapping_mul(y.wrapping_sub(a[1])))
                    >> 16;
                count += 1;
            }
        }
        if count != 2 {
            continue;
        }
        let left = xs[0].min(xs[1]).max(0);
        let right = xs[0].max(xs[1]).min(size[0] as i32);
        if left >= right {
            continue;
        }
        let mut color = base
            .wrapping_add(step_y.wrapping_mul(y.wrapping_sub(points[top][1])))
            .wrapping_add(step_x.wrapping_mul(left));
        for x in left..right {
            let rgb = *palette
                .get((color >> 8) as usize)
                .ok_or("Gouraud palette index out of range")?;
            let offset = y as usize * size[0] + x as usize;
            pixels[offset] = if alpha == 0 {
                rgb
            } else {
                let inv = (256 - alpha) as u32;
                let a = alpha as u32;
                let old = pixels[offset];
                (((rgb & 0xff00ff).wrapping_mul(inv) >> 8) & 0xff00ff)
                    + (((rgb & 0xff00).wrapping_mul(inv) >> 8) & 0xff00)
                    + (((old & 0xff00ff).wrapping_mul(a) >> 8) & 0xff00ff)
                    + (((old & 0xff00).wrapping_mul(a) >> 8) & 0xff00)
            };
            color = color.wrapping_add(step_x);
        }
    }
    Ok(())
}

pub struct Texture<'a> {
    pub texels: &'a [u32],
    pub low_detail: bool,
    pub opaque: bool,
    pub average: i32,
}
pub struct TexturedTriangle {
    pub points: [[i32; 2]; 3],
    pub lights: [i32; 3],
    pub view: [[i32; 3]; 3],
    pub alpha: i32,
}
pub fn textured(
    pixels: &mut [u32],
    size: [usize; 2],
    palette: &[u32],
    texture: &Texture<'_>,
    face: &TexturedTriangle,
) -> Result<(), String> {
    if face.alpha > 10 || texture.texels.is_empty() {
        let colors = face.lights.map(|l| {
            (texture.average & 0xff80)
                + ((texture.average & 127).wrapping_mul(l) >> 7).clamp(2, 126)
        });
        return gouraud(pixels, size, palette, face.points, colors, face.alpha);
    }
    let [[xa, ya], [xb, yb], [xc, yc]] = face.points;
    let colors = face.lights;
    let dx_ab = xb.wrapping_sub(xa);
    let dy_ab = yb.wrapping_sub(ya);
    let dx_ac = xc.wrapping_sub(xa);
    let dy_ac = yc.wrapping_sub(ya);
    let length = dx_ab
        .wrapping_mul(dy_ac)
        .wrapping_sub(dx_ac.wrapping_mul(dy_ab));
    if length == 0 {
        return Ok(());
    }
    let dc_ab = colors[1].wrapping_sub(colors[0]);
    let dc_ac = colors[2].wrapping_sub(colors[0]);
    let step_x = dc_ab
        .wrapping_mul(dy_ac)
        .wrapping_sub(dc_ac.wrapping_mul(dy_ab))
        .wrapping_shl(9)
        .wrapping_div(length);
    let step_y = dc_ac
        .wrapping_mul(dx_ab)
        .wrapping_sub(dc_ab.wrapping_mul(dx_ac))
        .wrapping_shl(9)
        .wrapping_div(length);
    let top = if ya <= yb && ya <= yc {
        0
    } else if yb <= yc {
        1
    } else {
        2
    };
    let base = colors[top]
        .wrapping_shl(9)
        .wrapping_add(step_x)
        .wrapping_sub(step_x.wrapping_mul(face.points[top][0]));
    let a = face.view[0];
    let ab = std::array::from_fn::<_, 3, _>(|i| a[i].wrapping_sub(face.view[1][i]));
    let ac = std::array::from_fn::<_, 3, _>(|i| face.view[2][i].wrapping_sub(a[i]));
    fn cross(a: i32, b: i32, c: i32, d: i32, shift: u32) -> i32 {
        a.wrapping_mul(b)
            .wrapping_sub(c.wrapping_mul(d))
            .wrapping_shl(shift)
    }
    let vertical = [
        cross(ac[0], a[1], ac[1], a[0], 14),
        cross(ab[0], a[1], ab[1], a[0], 14),
        cross(ab[1], ac[0], ab[0], ac[1], 14),
    ];
    let horizontal = [
        cross(ac[1], a[2], ac[2], a[1], 8),
        cross(ab[1], a[2], ab[2], a[1], 8),
        cross(ab[2], ac[1], ab[1], ac[2], 8),
    ];
    let scan_step = [
        cross(ac[2], a[0], ac[0], a[2], 5),
        cross(ab[2], a[0], ab[0], a[2], 5),
        cross(ab[0], ac[2], ab[2], ac[0], 5),
    ];
    let start = face.points[top][1].max(0);
    let end = ya.max(yb).max(yc).min(size[1] as i32);
    for y in start..end {
        let mut xs = [0; 3];
        let mut count = 0;
        for (a, b) in [(0, 1), (1, 2), (0, 2)] {
            let (mut a, mut b) = (face.points[a], face.points[b]);
            if a[1] > b[1] {
                std::mem::swap(&mut a, &mut b);
            }
            if a[1] <= y && y < b[1] {
                let slope = b[0]
                    .wrapping_sub(a[0])
                    .wrapping_shl(16)
                    .wrapping_div(b[1].wrapping_sub(a[1]));
                xs[count] = a[0]
                    .wrapping_shl(16)
                    .wrapping_add(slope.wrapping_mul(y.wrapping_sub(a[1])))
                    >> 16;
                count += 1;
            }
        }
        if count != 2 {
            continue;
        }
        let left = xs[0].min(xs[1]).max(0);
        let right = xs[0].max(xs[1]).min(size[0] as i32);
        if left >= right {
            continue;
        }
        let row = base.wrapping_add(step_y.wrapping_mul(y.wrapping_sub(face.points[top][1])));
        let v =
            std::array::from_fn(|i| vertical[i].wrapping_add(scan_step[i].wrapping_mul(y - 16)));
        textured_span(
            pixels,
            y as usize * size[0],
            left,
            right,
            row,
            step_x,
            v,
            horizontal,
            texture,
        )?;
    }
    Ok(())
}
#[allow(clippy::too_many_arguments)]
fn textured_span(
    pixels: &mut [u32],
    offset: usize,
    left: i32,
    right: i32,
    light: i32,
    step: i32,
    mut vertical: [i32; 3],
    horizontal: [i32; 3],
    texture: &Texture<'_>,
) -> Result<(), String> {
    let delta = left - 16;
    for i in 0..3 {
        vertical[i] = vertical[i].wrapping_add((horizontal[i] >> 3).wrapping_mul(delta));
    }
    let (div_shift, pack_shift, index_shift, mask) = if texture.low_detail {
        (12, 20, 26, 0xfc0)
    } else {
        (14, 18, 25, 0x3f80)
    };
    fn uv(v: [i32; 3], shift: i32) -> [i32; 2] {
        let denominator = v[2] >> shift;
        if denominator == 0 {
            [0, 0]
        } else {
            [
                v[0].wrapping_div(denominator),
                v[1].wrapping_div(denominator),
            ]
        }
    }
    let mut begin = uv(vertical, div_shift);
    for i in 0..3 {
        vertical[i] = vertical[i].wrapping_add(horizontal[i]);
    }
    let mut end = uv(vertical, div_shift);
    let mut light = light.wrapping_add(step.wrapping_mul(left));
    let mut x = left;
    while x < right {
        let mut packed = begin[0].wrapping_shl(pack_shift).wrapping_add(begin[1]);
        let uv_step = (end[0].wrapping_sub(begin[0]) >> 3)
            .wrapping_shl(pack_shift)
            .wrapping_add(end[1].wrapping_sub(begin[1]) >> 3);
        let brightness = light >> 8;
        let count = (right - x).min(8);
        for _ in 0..count {
            let index = ((packed as u32 & mask) + (packed as u32 >> index_shift)) as usize;
            let color = *texture
                .texels
                .get(index)
                .ok_or("Texture raster index out of range")?;
            if texture.opaque || color != 0 {
                pixels[offset + x as usize] =
                    ((((color & 0xff00ff).wrapping_mul(brightness as u32) & 0xff00ff00)
                        .wrapping_add((color & 0xff00).wrapping_mul(brightness as u32) & 0xff0000))
                        as i32
                        >> 8) as u32;
            }
            x += 1;
            packed = packed.wrapping_add(uv_step);
        }
        begin = end;
        for i in 0..3 {
            vertical[i] = vertical[i].wrapping_add(horizontal[i]);
        }
        end = uv(vertical, div_shift);
        light = light.wrapping_add(step.wrapping_shl(3));
    }
    Ok(())
}
