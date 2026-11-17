//! Bind-pose normals, GL texture projections and material colors from pinned RT4.
use crate::{model::Model, texture::Material};

#[derive(Clone, Debug, Default)]
pub struct Surface {
    pub uv: [[f32; 2]; 3],
    /// Accumulated integer normal and its face count (zero for flat normals).
    pub normals: [[i32; 4]; 3],
    /// The packed GlModel vertex of each corner as [prepare] met them,
    /// numbered in creation order. Corners with one number share uv and normal.
    pub packed: [u32; 3],
}
fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|i| a[i] - b[i])
}
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn apply(matrix: [f32; 9], v: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|i| {
        v[0] * matrix[i * 3] + v[1] * matrix[i * 3 + 1] + v[2] * matrix[i * 3 + 2]
    })
}
pub fn visible(m: &Model, i: usize, materials: &[Option<Material>]) -> bool {
    m.face_info[i] != 2
        && (m.textures[i] < 0
            || !materials
                .get(m.textures[i] as usize)
                .and_then(Option::as_ref)
                .is_some_and(|t| t.flipped))
}
pub fn prepare(m: &Model, materials: &[Option<Material>]) -> Vec<Surface> {
    let mut vertex_normals = vec![[0i32; 4]; m.vertices.len()];
    let mut face_normals = vec![[0i32; 4]; m.faces.len()];
    for (i, &face) in m.faces.iter().enumerate() {
        let a = m.vertices[face[0]];
        let ab = std::array::from_fn::<_, 3, _>(|j| m.vertices[face[1]][j].wrapping_sub(a[j]));
        let ac = std::array::from_fn::<_, 3, _>(|j| m.vertices[face[2]][j].wrapping_sub(a[j]));
        let mut n = [
            ab[1]
                .wrapping_mul(ac[2])
                .wrapping_sub(ac[1].wrapping_mul(ab[2])),
            ab[2]
                .wrapping_mul(ac[0])
                .wrapping_sub(ac[2].wrapping_mul(ab[0])),
            ab[0]
                .wrapping_mul(ac[1])
                .wrapping_sub(ac[0].wrapping_mul(ab[1])),
        ];
        while n.iter().any(|&v| !(-8192..=8192).contains(&v)) {
            for v in &mut n {
                *v >>= 1;
            }
        }
        let length = f64::from(n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt() as i32;
        let n = n.map(|v| v * 256 / length.max(1));
        if m.face_info[i] == 0 {
            for v in face {
                for (a, &n) in n.iter().enumerate() {
                    vertex_normals[v][a] = vertex_normals[v][a].wrapping_add(n);
                }
                vertex_normals[v][3] += 1;
            }
        } else if m.face_info[i] == 1 {
            face_normals[i] = [n[0], n[1], n[2], 0];
        }
    }
    let nt = m.texture_faces.len();
    let mut min = vec![[i32::MAX; 3]; nt];
    let mut max = vec![[-2147483647; 3]; nt];
    for (i, &face) in m.faces.iter().enumerate() {
        let t = m.texture_indices[i];
        if t < 0 || !visible(m, i, materials) {
            continue;
        }
        let t = t as usize;
        for v in face {
            for axis in 0..3 {
                min[t][axis] = min[t][axis].min(m.vertices[v][axis]);
                max[t][axis] = max[t][axis].max(m.vertices[v][axis]);
            }
        }
    }
    let centers: Vec<_> = min
        .iter()
        .zip(&max)
        .map(|(a, b)| std::array::from_fn::<_, 3, _>(|i| a[i].wrapping_add(b[i]) / 2))
        .collect();
    let matrices: Vec<_> = m
        .texture_types
        .iter()
        .enumerate()
        .map(|(i, &kind)| {
            let s = m.texture_scales[i];
            let scale = match kind {
                1 => {
                    let sx = s[0] as i16;
                    match sx.cmp(&0) {
                        std::cmp::Ordering::Equal => [1.0, 64.0 / f32::from(s[1]), 1.0],
                        std::cmp::Ordering::Greater => {
                            [1.0, 64.0 / f32::from(s[1]), f32::from(sx) / 1024.0]
                        }
                        std::cmp::Ordering::Less => {
                            [-f32::from(sx) / 1024.0, 64.0 / f32::from(s[1]), 1.0]
                        }
                    }
                }
                2 => s.map(|v| 64.0 / f32::from(v)),
                3 => s.map(|v| f32::from(v as i16) / 1024.0),
                _ => [1.0; 3],
            };
            matrix(
                m.texture_faces[i].map(|v| v as i16 as i32),
                m.texture_rotation[i],
                scale,
            )
        })
        .collect();
    let mut keys = vec![[0i64; 3]; m.faces.len()];
    let mut surfaces: Vec<_> = m
        .faces
        .iter()
        .enumerate()
        .map(|(i, &face)| {
            let normals = if m.face_info[i] == 0 {
                face.map(|v| vertex_normals[v])
            } else {
                [face_normals[i]; 3]
            };
            let mut wraps = [0i64; 3];
            let mut projection_axis = 0i32;
            let mut uv = [[0.0; 2]; 3];
            let texture = m.textures[i];
            let index = m.texture_indices[i];
            if texture >= 0 {
                uv = [[0.0, 1.0], [1.0, 1.0], [0.0, 0.0]];
                if index < 0 {
                    wraps = [0, 1, 2];
                }
                if index >= 0 {
                    let index = index as usize;
                    let kind = m.texture_types[index];
                    let f = face.map(|v| m.vertices[v].map(|v| v as f32));
                    if kind == 0 {
                        let mapping = m.texture_faces[index];
                        if mapping.iter().all(|&v| v < m.vertices.len()) {
                            let origin = m.vertices[mapping[0]].map(|v| v as f32);
                            let a = sub(m.vertices[mapping[1]].map(|v| v as f32), origin);
                            let b = sub(m.vertices[mapping[2]].map(|v| v as f32), origin);
                            let n = cross(a, b);
                            let row = cross(b, n);
                            let inv = 1.0 / dot(row, a);
                            let row2 = cross(a, n);
                            let inv2 = 1.0 / dot(row2, b);
                            for j in 0..3 {
                                let delta = sub(f[j], origin);
                                uv[j] = [dot(row, delta) * inv, dot(row2, delta) * inv2];
                            }
                        }
                    } else {
                        let center = centers[index].map(|v| v as f32);
                        let mat = matrices[index];
                        let dir = m.texture_direction[index];
                        let speed = f32::from(m.texture_speed[index]) / 256.0;
                        let scale = f32::from(m.texture_scales[index][2]) / 1024.0;
                        let transformed = f.map(|v| apply(mat, sub(v, center)));
                        let axis = if kind == 2 {
                            let normal = cross(sub(f[1], f[0]), sub(f[2], f[0]));
                            let mut normal = apply(mat, normal);
                            for (i, v) in normal.iter_mut().enumerate() {
                                *v /= 64.0 / f32::from(m.texture_scales[index][i]);
                            }
                            dominant(normal)
                        } else {
                            0
                        };
                        projection_axis = axis as i32;
                        for j in 0..3 {
                            let [x, y, z] = transformed[j];
                            let raw = match kind {
                                1 => {
                                    let mut u = (f64::from(x).atan2(f64::from(z)) as f32)
                                        / std::f32::consts::TAU
                                        + 0.5;
                                    if scale != 1.0 {
                                        u *= scale;
                                    }
                                    [u, y + speed + 0.5]
                                }
                                2 => {
                                    let tu = f32::from(m.texture_translation[index][0]) / 256.0;
                                    let tv = f32::from(m.texture_translation[index][1]) / 256.0;
                                    match axis {
                                        0 => [x + speed + 0.5, tv + 0.5 - z],
                                        1 => [x + speed + 0.5, z + tv + 0.5],
                                        2 => [speed + 0.5 - x, tu + 0.5 - y],
                                        3 => [x + speed + 0.5, tu + 0.5 - y],
                                        4 => [z + tv + 0.5, tu + 0.5 - y],
                                        _ => [tv + 0.5 - z, tu + 0.5 - y],
                                    }
                                }
                                3 => {
                                    let length = f64::from(x * x + y * y + z * z).sqrt() as f32;
                                    [
                                        (f64::from(x).atan2(f64::from(z)) as f32)
                                            / std::f32::consts::TAU
                                            + 0.5,
                                        (f64::from(y / length).asin() as f32)
                                            / std::f32::consts::PI
                                            + speed
                                            + 0.5,
                                    ]
                                }
                                _ => [0.0; 2],
                            };
                            uv[j] = direction(raw, dir);
                        }
                        if kind == 1 || kind == 3 {
                            let period = if kind == 1 { scale } else { 1.0 };
                            let axis = (dir & 1) as usize;
                            for j in 1..3 {
                                if uv[j][axis] - uv[0][axis] > period / 2.0 {
                                    uv[j][axis] -= period;
                                    wraps[j] = 1;
                                } else if uv[0][axis] - uv[j][axis] > period / 2.0 {
                                    uv[j][axis] += period;
                                    wraps[j] = 2;
                                }
                            }
                        }
                    }
                }
            }
            let high = (i64::from(projection_axis.wrapping_shl(24))
                + i64::from(i32::from(m.colors[i]).wrapping_shl(8)))
                + i64::from(m.alpha[i]);
            let low = if m.face_info[i] == 1 {
                let n = normals[0];
                index
                    .wrapping_shl(2)
                    .wrapping_add(if n[0] > 0 { 1024 } else { 2048 })
                    .wrapping_add((n[1] + 256).wrapping_shl(12))
                    .wrapping_add((n[2] + 256).wrapping_shl(22))
            } else {
                index.wrapping_shl(2)
            };
            let key = i64::from(low).wrapping_add(high.wrapping_shl(32));
            keys[i] = wraps.map(|w| key.wrapping_add(w));
            Surface {
                uv,
                normals: normals.map(|n| n.map(|v| v as i16 as i32)),
                packed: [0; 3],
            }
        })
        .collect();
    // GlModel shares packed vertices by this key, including its flat-normal
    // collision behavior. First use follows the original material sort order.
    let order = face_order(m, materials);
    let mut shared = std::collections::BTreeMap::new();
    for i in order {
        for j in 0..3 {
            let key = (m.faces[i][j], keys[i][j]);
            let original = (
                surfaces[i].uv[j],
                surfaces[i].normals[j],
                shared.len() as u32,
            );
            let (uv, normal, packed) = *shared.entry(key).or_insert(original);
            surfaces[i].uv[j] = uv;
            surfaces[i].normals[j] = normal;
            surfaces[i].packed[j] = packed;
        }
    }
    surfaces
}

/// GlModel constructor order, retained from the bind pose during alpha animation.
pub fn face_order(m: &Model, materials: &[Option<Material>]) -> Vec<usize> {
    let mut order: Vec<_> = (0..m.faces.len())
        .filter(|&i| visible(m, i, materials))
        .collect();
    order.sort_by_key(|&i| {
        let tex = m.textures[i];
        let material = if tex < 0 {
            None
        } else {
            materials.get(tex as usize).and_then(Option::as_ref)
        };
        let transparent = m.alpha[i] != 0 || material.is_some_and(|m| !m.opaque);
        let mut high = if (m.sort_by_priority || transparent) && m.has_priorities {
            (m.priorities[i] as i8 as i32) << 17
        } else {
            0
        };
        if transparent {
            high += 65536;
        }
        if let Some(m) = material {
            high += i32::from(m.material_type) << 8;
            high += i32::from(m.animation_type);
        }
        let low = ((tex & 65535) << 16).wrapping_add(i as i32 & 65535);
        (i64::from(high) << 32).wrapping_add(i64::from(low))
    });
    order
}
fn dominant([x, y, z]: [f32; 3]) -> usize {
    if y.abs() > x.abs() && y.abs() > z.abs() {
        if y > 0.0 { 0 } else { 1 }
    } else if z.abs() > x.abs() && z.abs() > y.abs() {
        if z > 0.0 { 2 } else { 3 }
    } else if x > 0.0 {
        4
    } else {
        5
    }
}
fn direction([u, v]: [f32; 2], dir: u8) -> [f32; 2] {
    match dir {
        1 => [-v, u],
        2 => [-u, -v],
        3 => [v, -u],
        _ => [u, v],
    }
}
pub fn matrix([p, m, n]: [i32; 3], rotation: u8, scale: [f32; 3]) -> [f32; 9] {
    let angle = f64::from(f32::from(rotation) * 0.024543693);
    let cos = angle.cos() as f32;
    let sin = angle.sin() as f32;
    let yaw = [cos, 0.0, sin, 0.0, 1.0, 0.0, -sin, 0.0, cos];
    let mut result = [0.0; 9];
    let ny = m as f32 / 32767.0;
    let tilt = -(f64::from(1.0 - ny * ny).sqrt() as f32);
    let one = 1.0 - ny;
    let mag = f64::from(p.wrapping_mul(p).wrapping_add(n.wrapping_mul(n))).sqrt() as f32;
    if mag == 0.0 && ny == 0.0 {
        result = yaw;
    } else {
        let (nz, nx) = if mag == 0.0 {
            (1.0, 0.0)
        } else {
            (-(n as f32) / mag, p as f32 / mag)
        };
        let t = [
            ny + nz * nz * one,
            nx * tilt,
            nx * nz * one,
            -nx * tilt,
            ny,
            nz * tilt,
            nz * nx * one,
            -nz * tilt,
            ny + nx * nx * one,
        ];
        for i in 0..3 {
            for j in 0..3 {
                result[i * 3 + j] =
                    yaw[i * 3] * t[j] + yaw[i * 3 + 1] * t[3 + j] + yaw[i * 3 + 2] * t[6 + j];
            }
        }
    }
    for i in 0..9 {
        result[i] *= scale[i / 3];
    }
    result
}
pub fn palette(brightness: f32) -> Vec<u32> {
    (0..65536)
        .map(|color| {
            let h = f64::from(color >> 10) / 64.0 + 0.0078125;
            let s = f64::from((color >> 7) & 7) / 8.0 + 0.0625;
            let l = f64::from(color & 127) / 128.0;
            let q = if l < 0.5 {
                l * (s + 1.0)
            } else {
                l + s - l * s
            };
            let p = l * 2.0 - q;
            let hue = |mut t: f64| {
                if t > 1.0 {
                    t -= 1.0;
                }
                if t < 0.0 {
                    t += 1.0;
                }
                if t * 6.0 < 1.0 {
                    p + (q - p) * 6.0 * t
                } else if t * 2.0 < 1.0 {
                    q
                } else if t * 3.0 < 2.0 {
                    p + (q - p) * (2.0 / 3.0 - t) * 6.0
                } else {
                    p
                }
            };
            let channels = [hue(h + 1.0 / 3.0), hue(h), hue(h - 1.0 / 3.0)]
                .map(|v| (v.powf(f64::from(brightness)) * 256.0) as u32);
            ((channels[0] << 16) + (channels[1] << 8) + channels[2]).max(1)
        })
        .collect()
}
pub fn pack_color(
    color: u16,
    material: Option<&Material>,
    lightness: i32,
    alpha: u8,
    palette: &[u32],
) -> [u8; 4] {
    let light = ((lightness * i32::from(color & 127)) >> 7).clamp(2, 126);
    let mut rgb = palette[usize::from(color & 0xff80) + light as usize];
    if let Some(m) = material {
        let brightness = u32::from(m.brightness);
        if brightness != 0 {
            let white = if lightness < 0 {
                0
            } else if lightness > 127 {
                0xffffff
            } else {
                lightness as u32 * 131586
            };
            let blend = 256 - brightness;
            rgb = (((white & 0xff00ff).wrapping_mul(brightness)
                + (rgb & 0xff00ff).wrapping_mul(blend))
                & 0xff00ff00)
                .wrapping_add(
                    ((white & 0xff00).wrapping_mul(brightness)
                        + (rgb & 0xff00).wrapping_mul(blend))
                        & 0xff0000,
                )
                >> 8;
        }
        if m.speed != 0 {
            let factor = u32::from(m.speed) + 256;
            let r = ((rgb >> 16 & 255) * factor).min(65535);
            let g = ((rgb >> 8 & 255) * factor).min(65535);
            let b = ((rgb & 255) * factor).min(65535);
            rgb = ((r & 0xff00) << 8) + (g & 0xff00) + (b >> 8);
        }
    }
    [(rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8, 255 - alpha]
}
