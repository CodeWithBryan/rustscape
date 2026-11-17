//! The tables `MaterialManager.initNoiseTextures` and
//! `LiquidMaterialRenderer` build once at start: two 64x64x64
//! luminance-alpha noise volumes and a 256x64 turbulence table.
//!
//! Transcribed from `PerlinNoiseGenerator`, `RidgedNoiseGenerator`,
//! `RidgedNoiseTexture`, `FractalNoiseGenerator`, `FractalNoiseTexture`,
//! `TextureOpPerlinNoise.getNoiseRow` and
//! `LiquidMaterialRenderer.generateNoiseTable`. Java `int` arithmetic is
//! reproduced with wrapping operations. `tests/materials.rs` compares every
//! byte with the unchanged classes run on a JVM (`cargo xtask
//! materials-oracle`).
use crate::texture_graph::{permutation, smooth};

/// Samples along each axis of a volume.
pub const SIDE: usize = 64;
/// Bytes of one volume: luminance and alpha for each of 64^3 samples.
pub const VOLUME_BYTES: usize = SIDE * SIDE * SIDE * 2;
/// `LiquidMaterialRenderer`'s table: one row per low byte of the clock.
pub const LIQUID_ROWS: usize = 256;
pub const LIQUID_COLUMNS: usize = 64;

/// `PerlinNoiseGenerator.gradient`.
fn gradient(x: i32, y: i32, z: i32, hash: i32) -> i32 {
    let h = hash & 0xf;
    let v = if h >= 4 {
        if h == 12 || h == 14 { x } else { y }
    } else {
        z
    };
    let u = if h < 8 { x } else { z };
    (if h & 1 == 0 { u } else { u.wrapping_neg() }).wrapping_add(if h & 2 == 0 {
        v
    } else {
        v.wrapping_neg()
    })
}

/// `PerlinNoiseGenerator.generate`: `sample(noise, octave)` is `accumulate`
/// and `finish()` is `finishSample`, called in the original order (z outermost,
/// then y, then x, octaves innermost).
fn generate(
    seed: i32,
    octaves: usize,
    scale: [i32; 3],
    mut sample: impl FnMut(i32, usize),
    mut finish: impl FnMut(),
) {
    // `initAmplitudes`: `(short) Math.pow(2.0, i)`.
    let amplitudes: Vec<i32> = (0..octaves)
        .map(|i| 2f64.powi(i as i32) as i16 as i32)
        .collect();
    // `initPermutations` is `TextureOpVoronoi.getPermutationTable` on shorts.
    let perm = permutation(seed);
    let perm = |index: i32| perm[index as usize];
    let smooth: Vec<i32> = (0..4096).map(smooth).collect();
    let coords: Vec<i32> = (0..SIDE as i32).map(|i| (i << 12) / 64).collect();
    let [scale_x, scale_y, scale_z] = scale;
    for iz in 0..SIDE {
        for iy in 0..SIDE {
            for ix in 0..SIDE {
                for (octave, &amplitude) in amplitudes.iter().enumerate() {
                    let i = amplitude << 12;
                    let x_bound = scale_x.wrapping_mul(i) >> 12;
                    let mut scaled_z = i.wrapping_mul(coords[iz]) >> 12;
                    let y_bound = i.wrapping_mul(scale_y) >> 12;
                    let z_product = scaled_z.wrapping_mul(scale_z);
                    let z_bound = i.wrapping_mul(scale_z) >> 12;
                    let scaled_x = coords[ix].wrapping_mul(i) >> 12;
                    let x_product = scaled_x.wrapping_mul(scale_x);
                    let grid_x = x_product >> 12;
                    let frac_x = x_product & 0xfff;
                    let grid_z = z_product >> 12;
                    let frac_xm = frac_x - 4096;
                    let mut grid_x1 = grid_x + 1;
                    let mut grid_z1 = grid_z + 1;
                    let mut scaled_y = coords[iy].wrapping_mul(i) >> 12;
                    let smooth_x = smooth[frac_x as usize];
                    let y_product = scaled_y.wrapping_mul(scale_y);
                    let perm_z = grid_z & 0xff;
                    let perm_x = grid_x & 0xff;
                    if z_bound <= grid_z1 {
                        grid_z1 = 0;
                    } else {
                        grid_z1 &= 0xff;
                    }
                    let mut grid_y = y_product >> 12;
                    let perm_pz = perm(perm_z);
                    let perm_pz1 = perm(grid_z1);
                    let mut grid_y1 = grid_y + 1;
                    if y_bound > grid_y1 {
                        grid_y1 &= 0xff;
                    } else {
                        grid_y1 = 0;
                    }
                    scaled_y = y_product & 0xfff;
                    let perm_pz1y1 = perm(grid_y1 + perm_pz1);
                    scaled_z = z_product & 0xfff;
                    grid_y &= 0xff;
                    let smooth_z = smooth[scaled_z as usize];
                    let perm_pz1y = perm(grid_y + perm_pz1);
                    let perm_pzy = perm(perm_pz + grid_y);
                    if x_bound <= grid_x1 {
                        grid_x1 = 0;
                    } else {
                        grid_x1 &= 0xff;
                    }
                    let frac_ym = scaled_y - 4096;
                    let smooth_y = smooth[scaled_y as usize];
                    let frac_zm = scaled_z - 4096;
                    let perm_pzy1 = perm(perm_pz + grid_y1);
                    let lerp = |from: i32, to: i32, weight: i32| {
                        from.wrapping_add(weight.wrapping_mul(to.wrapping_sub(from)) >> 12)
                    };
                    let g000 = gradient(frac_x, scaled_z, scaled_y, perm(perm_pzy + perm_x));
                    let g100 = gradient(frac_xm, scaled_z, scaled_y, perm(grid_x1 + perm_pzy));
                    let interp_x0 = lerp(g000, g100, smooth_x);
                    let g010 = gradient(frac_x, scaled_z, frac_ym, perm(perm_pzy1 + perm_x));
                    let g110 = gradient(frac_xm, scaled_z, frac_ym, perm(grid_x1 + perm_pzy1));
                    let interp_x1 = lerp(g010, g110, smooth_x);
                    let interp_xy0 = lerp(interp_x0, interp_x1, smooth_y);
                    let g001 = gradient(frac_x, frac_zm, scaled_y, perm(perm_x + perm_pz1y));
                    let g101 = gradient(frac_xm, frac_zm, scaled_y, perm(grid_x1 + perm_pz1y));
                    let interp_x2 = lerp(g001, g101, smooth_x);
                    let g011 = gradient(frac_x, frac_zm, frac_ym, perm(perm_x + perm_pz1y1));
                    let g111 = gradient(frac_xm, frac_zm, frac_ym, perm(grid_x1 + perm_pz1y1));
                    let interp_x3 = lerp(g011, g111, smooth_x);
                    let interp_xy1 = lerp(interp_x2, interp_x3, smooth_y);
                    sample(lerp(interp_xy0, interp_xy1, smooth_z), octave);
                }
                finish();
            }
        }
    }
}

/// `new RidgedNoiseTexture().generateTexture()`: the volume the water, liquid
/// and underwater materials sample. Luminance is `value * 3 >> 5` and alpha
/// `value >> 2`, in z, y, x order.
pub fn ridged() -> Vec<u8> {
    // `super(8, 5, 8, 8, 2, 0.1F, 0.55F, 3.0F)`.
    let decay = (3.0f32 * 4096.0f32) as i32;
    let threshold = (0.55f32 * 4096.0f32) as i32;
    let initial_weight = (0.5f64.powf(-f64::from(0.1f32)) * 4096.0) as i32;
    let state = std::cell::RefCell::new((0i32, 0i32, initial_weight, Vec::new()));
    generate(
        8,
        5,
        [8, 8, 2],
        |noise, octave| {
            let mut state = state.borrow_mut();
            let (signal, accumulator, current_weight, _) = &mut *state;
            let ridge = |noise: i32| {
                let signal = threshold.wrapping_sub(noise.wrapping_abs());
                signal.wrapping_mul(signal) >> 12
            };
            if octave == 0 {
                *signal = ridge(noise);
                *accumulator = *signal;
                return;
            }
            let weight = (decay.wrapping_mul(*signal) >> 12).clamp(0, 4096);
            *signal = ridge(noise).wrapping_mul(weight) >> 12;
            *accumulator = accumulator.wrapping_add(current_weight.wrapping_mul(*signal) >> 12);
            *current_weight = initial_weight.wrapping_mul(*current_weight) >> 12;
        },
        || {
            let mut state = state.borrow_mut();
            let (_, accumulator, current_weight, out) = &mut *state;
            *current_weight = initial_weight;
            let value = (*accumulator >> 4).clamp(0, 255);
            out.extend([((value * 3) >> 5) as u8, (value >> 2) as u8]);
            *accumulator = 0;
        },
    );
    state.into_inner().3
}

/// `new FractalNoiseTexture().generateTexture()`: the volume the waterfall
/// material modulates with. Luminance and alpha are both
/// `(value >> 1) + 127`.
pub fn fractal() -> Vec<u8> {
    // `super(12, 5, 16, 2, 2, 0.45F)`: seed 12, five octaves, scales 16, 2, 2.
    let weights: Vec<i32> = (0..5)
        .map(|i| (f64::from(0.45f32).powi(i) * 4096.0) as i16 as i32)
        .collect();
    let state = std::cell::RefCell::new((0i32, Vec::new()));
    generate(
        12,
        5,
        [16, 2, 2],
        |noise, octave| {
            let mut state = state.borrow_mut();
            state.0 = state
                .0
                .wrapping_add(noise.wrapping_mul(weights[octave]) >> 12);
        },
        || {
            let mut state = state.borrow_mut();
            let mut accumulator = state.0.wrapping_abs();
            if accumulator >= 4096 {
                accumulator = 4095;
            }
            // `(byte) (accumulator >> 4)`, then `(sample >> 1 & 0x7F) + 127`.
            let sample = (accumulator >> 4) as u8 as i8 as i32;
            let value = ((sample >> 1 & 0x7f) + 127) as u8;
            state.1.extend([value, value]);
            state.0 = 0;
        },
    );
    state.into_inner().1
}

/// `LiquidMaterialRenderer.generateNoiseTable(0.4F)`: row `clock & 255`,
/// column `floor(frac(coordinate / 8192) * 64)`, in 1/4096 units.
///
/// The renderer builds the table twice, for the S and the T offset; both
/// calls give these same values.
pub fn liquid() -> Vec<i32> {
    // TextureOpPerlinNoise with three octaves, frequencies 8 by 4, seed 0,
    // no normalisation, at `Texture.setSize(256, 64)`.
    let persistence = (0.4f32 * 4096.0f32) as i32;
    let (frequency_x, frequency_y) = (8i32, 4i32);
    let amplitudes: Vec<i32> = (0..3)
        .map(|i| (f64::from(persistence as f32 / 4096.0f32).powi(i) * 4096.0) as i16 as i32)
        .collect();
    let perm = permutation(0);
    let perm = |index: i32| perm[index as usize] & 0xff;
    let gradient = |hash: i32, x: i32, y: i32| match hash & 3 {
        0 => y + x,
        1 => -x + y,
        2 => x - y,
        _ => -x + -y,
    };
    let mut table = vec![0i32; LIQUID_ROWS * LIQUID_COLUMNS];
    for row in 0..LIQUID_ROWS {
        let scaled_y = frequency_y * (((row as i32) << 12) / LIQUID_ROWS as i32);
        for (octave, &amplitude) in amplitudes.iter().enumerate() {
            // `postDecode` keeps every octave here: all amplitudes exceed 8.
            let scale = (2f64.powi(octave as i32) as i16 as i32) << 12;
            let mut y_pos = scale.wrapping_mul(scaled_y) >> 12;
            let cell_y = y_pos >> 12;
            let perm_y = perm(cell_y & 0xff);
            let scaled_frequency_x = frequency_x.wrapping_mul(scale) >> 12;
            let mut cell_y_next = cell_y + 1;
            y_pos &= 0xfff;
            let smooth_y = smooth(y_pos);
            let scaled_frequency_y = frequency_y.wrapping_mul(scale) >> 12;
            if cell_y_next >= scaled_frequency_y {
                cell_y_next = 0;
            }
            let perm_y_next = perm(cell_y_next & 0xff);
            for col in 0..LIQUID_COLUMNS {
                let raw_x = (((col as i32) << 12) / LIQUID_COLUMNS as i32) * frequency_x;
                // `sampleNoise`.
                let x_pos = raw_x.wrapping_mul(scale) >> 12;
                let y_frac_neg = y_pos - 4096;
                let cell_x = x_pos >> 12;
                let mut cell_x_next = cell_x + 1;
                let cell_x_masked = cell_x & 0xff;
                if cell_x_next >= scaled_frequency_x {
                    cell_x_next = 0;
                }
                let x_frac = x_pos & 0xfff;
                let smooth_x = smooth(x_frac);
                cell_x_next &= 0xff;
                let x_frac_neg = x_frac - 4096;
                let dot0 = gradient(perm(cell_x_masked + perm_y), x_frac, y_pos);
                let dot1 = gradient(perm(perm_y + cell_x_next), x_frac_neg, y_pos);
                let top = dot0 + ((dot1 - dot0).wrapping_mul(smooth_x) >> 12);
                let dot0 = gradient(perm(cell_x_masked + perm_y_next), x_frac, y_frac_neg);
                let dot1 = gradient(perm(perm_y_next + cell_x_next), x_frac_neg, y_frac_neg);
                let bottom = dot0 + ((dot1 - dot0).wrapping_mul(smooth_x) >> 12);
                let noise = top + (smooth_y.wrapping_mul(bottom - top) >> 12);
                let value = noise.wrapping_mul(amplitude) >> 12;
                let cell = &mut table[row * LIQUID_COLUMNS + col];
                *cell = if octave == 0 {
                    value
                } else {
                    cell.wrapping_add(value)
                };
            }
        }
    }
    table
}
