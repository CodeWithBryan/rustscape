//! Stateful bark packing and the line shapes present in the pinned texture corpus.
use super::Random;
pub(super) fn bark(p: [i32; 10], size: usize) -> Result<Vec<[i32; 3]>, String> {
    let s = size as i32;
    let (minw, maxw, minh, maxh) = (
        (s * p[1]) >> 12,
        (s * p[2]) >> 12,
        (s * p[3]) >> 12,
        (s * p[4]) >> 12,
    );
    let mut canvas = vec![[0; 3]; size * size];
    if maxh <= 1 {
        return Ok(canvas);
    }
    if minw <= 0 || maxw <= minw || maxh <= minh {
        return Err("invalid bark dimensions".into());
    }
    let corner = (s / 8 * p[5]) >> 12;
    let bands = (s / minw + 1) as usize;
    let mut prev = vec![[0; 3]; bands];
    let mut cur = prev.clone();
    let mut rng = Random::new(p[0]);
    let (mut x, mut prev_offset, mut delta, mut offset, mut prev_i, mut prev_count, mut cur_i) =
        (0, 0, 0, 0, 0usize, 0usize, 0usize);
    let (mut last, mut first) = (true, true);
    for _ in 0..size * size * 4 {
        let mut w = minw + rng.bounded(maxw - minw);
        let mut h = rng.bounded(maxh - minh) + minh;
        let end = (x + w).min(s);
        if end == s {
            w = s - x;
        }
        let y = if first {
            0
        } else {
            let mut scan = prev_i;
            let mut y = prev[prev_i][2];
            let mut spans = 0;
            let wrap = |mut v: i32| {
                if v < 0 {
                    v += s;
                }
                if v > s {
                    v -= s;
                }
                v
            };
            let wrapped_end = wrap(delta + end);
            loop {
                if wrapped_end >= prev[scan][0] && prev[scan][1] >= wrapped_end {
                    if prev_i != scan {
                        let wrapped_start = wrap(delta + x);
                        for j in 1..=spans {
                            y = y.max(prev[(prev_i + j) % prev_count][2]);
                        }
                        for j in 0..=spans {
                            let band = prev[(prev_i + j) % prev_count];
                            let gap_y = band[2];
                            if gap_y != y {
                                let (left, right) = if wrapped_end > wrapped_start {
                                    (wrapped_start.max(band[0]), wrapped_end.min(band[1]))
                                } else if band[0] == 0 {
                                    (0, wrapped_end.min(band[1]))
                                } else {
                                    (wrapped_start.max(band[0]), s)
                                };
                                branch(
                                    &mut canvas,
                                    size,
                                    p,
                                    corner,
                                    &mut rng,
                                    [gap_y, prev_offset + left, right - left, y - gap_y],
                                );
                            }
                        }
                    }
                    prev_i = scan;
                    break;
                }
                spans += 1;
                scan = (scan + 1) % prev_count;
            }
            y
        };
        if s >= y + h {
            last = false;
        } else {
            h = s - y;
        }
        branch(
            &mut canvas,
            size,
            p,
            corner,
            &mut rng,
            [y, offset + x, w, h],
        );
        if end == s {
            if last {
                return Ok(canvas);
            }
            last = true;
            prev_offset = offset;
            first = false;
            cur[cur_i] = [x, end, h + y];
            cur_i += 1;
            prev_i = 0;
            prev_count = cur_i;
            cur_i = 0;
            offset = rng.bounded(s);
            delta = offset - prev_offset;
            std::mem::swap(&mut prev, &mut cur);
            x = 0;
            let mut target = delta;
            if target < 0 {
                target += s;
            }
            if target > s {
                target -= s;
            }
            while !(prev[prev_i][0] <= target && prev[prev_i][1] >= target) {
                prev_i = (prev_i + 1) % prev_count;
            }
        } else {
            cur[cur_i] = [x, end, h + y];
            cur_i += 1;
            x = end;
        }
    }
    Err("bark packing did not converge".into())
}
fn branch(
    canvas: &mut [[i32; 3]],
    size: usize,
    p: [i32; 10],
    corner_size: i32,
    rng: &mut Random,
    [y, mut x, w, h]: [i32; 4],
) {
    let brightness = if p[8] > 0 {
        4096 - rng.bounded(p[8])
    } else {
        4096
    };
    let variation = (p[7] * corner_size) >> 12;
    let corner = corner_size
        - if variation > 0 {
            rng.bounded(variation)
        } else {
            0
        };
    if x >= size as i32 {
        x -= size as i32;
    }
    if w <= 0 || h <= 0 {
        return;
    }
    let cw = (w / 2).min(corner).max(0);
    let ch = (h / 2).min(corner).max(0);
    for row in 0..h {
        let from_bottom = h - row - 1;
        let fade = if row < ch {
            row * brightness / ch
        } else if from_bottom < ch {
            from_bottom * brightness / ch
        } else {
            brightness
        };
        let row_start = (y + row) as usize * size;
        for col in 0..cw {
            let side = col * brightness / cw;
            let value = if row < ch || from_bottom < ch {
                if p[6] == 0 {
                    (side * fade) >> 12
                } else {
                    side.min(fade)
                }
            } else {
                side
            };
            canvas[row_start + ((x + col) as usize & (size - 1))] = [value; 3];
            canvas[row_start + ((x + w - col - 1) as usize & (size - 1))] = [value; 3];
        }
        let inner_x = x + cw;
        let inner_w = w - cw * 2;
        if inner_x + inner_w <= size as i32 {
            for col in inner_x..inner_x + inner_w {
                canvas[row_start + col as usize] = [fade; 3];
            }
        } else {
            // RT4 starts the wrapped fill at zero even when inner_x itself has
            // crossed the right edge. This can overwrite one corner pixel.
            let first = size as i32 - inner_x;
            for col in inner_x..size as i32 {
                canvas[row_start + col as usize] = [fade; 3];
            }
            for col in 0..inner_w - first {
                canvas[row_start + col as usize] = [fade; 3];
            }
        }
    }
}
pub(super) fn shapes(points: &[Vec<i32>], size: usize, mono: bool) -> Vec<[i32; 3]> {
    let mut canvas = vec![0; size * size];
    let edge = size as i32 - 1;
    for p in points {
        let [mut x1, mut y1, mut x2, mut y2] = std::array::from_fn(|i| (p[i] * size as i32) >> 12);
        let color = p[4];
        let dx = x2 - x1;
        let dy = y2 - y1;
        if dx == 0 {
            if dy != 0 && (0..=edge).contains(&x1) {
                for y in y1.clamp(0, edge).min(y2.clamp(0, edge))
                    ..y1.clamp(0, edge).max(y2.clamp(0, edge))
                {
                    canvas[y as usize * size + x1 as usize] = color;
                }
            }
            continue;
        }
        if dy == 0 {
            if (0..=edge).contains(&y1) {
                for x in x1.clamp(0, edge).min(x2.clamp(0, edge))
                    ..x1.clamp(0, edge).max(x2.clamp(0, edge))
                {
                    canvas[y1 as usize * size + x as usize] = color;
                }
            }
            continue;
        }
        let slope = (dy << 12) / dx;
        let intercept = y1 - ((slope * x1) >> 12);
        if x1 < 0 {
            x1 = 0;
            y1 = intercept;
        } else if x1 > edge {
            x1 = edge;
            y1 = ((edge * slope) >> 12) + intercept;
        }
        if x2 < 0 {
            x2 = 0;
            y2 = intercept;
        } else if x2 > edge {
            x2 = edge;
            y2 = ((edge * slope) >> 12) + intercept;
        }
        if y2 < 0 {
            y2 = 0;
            x2 = (-intercept << 12) / slope;
        } else if y2 > edge {
            y2 = edge;
            x2 = ((edge - intercept) << 12) / slope;
        }
        if y1 < 0 {
            y1 = 0;
            x1 = (-intercept << 12) / slope;
        } else if y1 > edge {
            y1 = edge;
            x1 = ((edge - intercept) << 12) / slope;
        }
        plot(&mut canvas, size, color, x1, y1, x2, y2);
    }
    canvas
        .into_iter()
        .map(|c| {
            if mono {
                [c; 3]
            } else {
                [c >> 12 & 4080, c >> 4 & 4080, (c & 255) << 4]
            }
        })
        .collect()
}
fn plot(
    canvas: &mut [i32],
    size: usize,
    color: i32,
    mut x1: i32,
    mut y1: i32,
    mut x2: i32,
    mut y2: i32,
) {
    let dx = x2 - x1;
    let dy = y2 - y1;
    if dx == 0 {
        for y in y1.min(y2)..y1.max(y2) {
            canvas[y as usize * size + x1 as usize] = color;
        }
        return;
    }
    if dy == 0 {
        for x in x1.min(x2)..x1.max(x2) {
            canvas[y1 as usize * size + x as usize] = color;
        }
        return;
    }
    let steep = dx.abs() < dy.abs();
    if steep {
        std::mem::swap(&mut x1, &mut y1);
        std::mem::swap(&mut x2, &mut y2);
    }
    if x2 < x1 {
        std::mem::swap(&mut x1, &mut x2);
        std::mem::swap(&mut y1, &mut y2);
    }
    let mut y = y1;
    let dx = x2 - x1;
    let dy = (y2 - y1).abs();
    let mut error = -(dx >> 1);
    let step = if y2 <= y1 { -1 } else { 1 };
    for x in x1..=x2 {
        if steep {
            canvas[x as usize * size + y as usize] = color;
            error += dy;
        } else {
            error += dy;
            canvas[y as usize * size + x as usize] = color;
        }
        if error > 0 {
            y += step;
            error -= dx;
        }
    }
}
