//! Exact GL masked-bone math. GPU upload and model cache ownership are caller services.
use crate::{animation::sin_cos, model::Model};
#[derive(Clone, Debug, Default)]
pub struct Origin {
    pub xyz: [i32; 3],
    pub valid: bool,
    pub colors_dirty: bool,
    pub normals_dirty: bool,
}
fn dot(a: [i32; 3], b: [i32; 3]) -> i32 {
    a.into_iter()
        .zip(b)
        .fold(0i32, |s, (x, y)| s.wrapping_add(x.wrapping_mul(y)))
        .wrapping_add(16384)
        >> 15
}
fn row(m: &[i32], i: usize) -> [i32; 3] {
    [m[i * 3], m[i * 3 + 1], m[i * 3 + 2]]
}
fn col(m: &[i32], i: usize) -> [i32; 3] {
    [m[i], m[3 + i], m[6 + i]]
}
fn mv(m: &[i32], v: [i32; 3]) -> [i32; 3] {
    std::array::from_fn(|i| dot(row(m, i), v))
}
fn add(a: [i32; 3], b: [i32; 3]) -> [i32; 3] {
    std::array::from_fn(|i| a[i].wrapping_add(b[i]))
}
fn rotation(xyz: [i32; 3]) -> [i32; 9] {
    let [sx, cx] = {
        let (s, c) = sin_cos(xyz[0]);
        [s >> 1, c >> 1]
    };
    let [sy, cy] = {
        let (s, c) = sin_cos(xyz[1]);
        [s >> 1, c >> 1]
    };
    let [sz, cz] = {
        let (s, c) = sin_cos(xyz[2]);
        [s >> 1, c >> 1]
    };
    let a = dot([sx, 0, 0], [cz, 0, 0]);
    let b = dot([sx, 0, 0], [sz, 0, 0]);
    [
        dot([cy, sy, 0], [cz, b, 0]),
        dot([cy.wrapping_neg(), sy, 0], [sz, a, 0]),
        dot([sy, 0, 0], [cx, 0, 0]),
        dot([cx, 0, 0], [sz, 0, 0]),
        dot([cx, 0, 0], [cz, 0, 0]),
        sx.wrapping_neg(),
        dot([sy.wrapping_neg(), cy, 0], [cz, b, 0]),
        dot([sy, cy, 0], [sz, a, 0]),
        dot([cy, 0, 0], [cx, 0, 0]),
    ]
}
pub fn basis(t: [i32; 6]) -> [i32; 15] {
    let mut m = [0; 15];
    let xyz = [t[3], t[4], t[5]];
    if xyz == [0; 3] {
        m[0] = 32768;
        m[4] = 32768;
        m[8] = 32768;
        for i in 0..3 {
            m[12 + i] = t[i].wrapping_neg();
        }
    } else {
        m[..9].copy_from_slice(&rotation(xyz));
        let neg = [
            t[0].wrapping_neg(),
            t[1].wrapping_neg(),
            t[2].wrapping_neg(),
        ];
        for i in 0..3 {
            m[12 + i] = dot(col(&m, i), neg);
        }
    }
    m[9..12].copy_from_slice(&t[..3]);
    m
}
fn rotate(v: &mut [i32; 3], xyz: [i32; 3], short: bool) {
    for axis in [2, 0, 1] {
        if xyz[axis] == 0 {
            continue;
        }
        let (s, c) = sin_cos(xyz[axis]);
        let (a, b) = match axis {
            2 => (0, 1),
            0 => (1, 2),
            _ => (0, 2),
        };
        let (x, y) = (v[a], v[b]);
        let q = |a: i32, b: i32, c: i32, d: i32| {
            a.wrapping_mul(b)
                .wrapping_add(c.wrapping_mul(d))
                .wrapping_add(32767)
                >> 16
        };
        if axis == 0 {
            v[a] = q(x, c, y, s.wrapping_neg());
            v[b] = q(x, s, y, c);
        } else {
            v[a] = q(x, c, y, s);
            v[b] = q(y, c, x, s.wrapping_neg());
        }
        if short {
            v[a] = v[a] as i16 as i32;
            v[b] = v[b] as i16 as i32;
        }
    }
}
fn selected(sources: &[u16], index: usize, mask: u16) -> bool {
    sources.is_empty() || sources[index] & mask != 0
}
#[allow(clippy::too_many_arguments)]
pub fn transform(
    m: &mut Model,
    o: &mut Origin,
    kind: u8,
    labels: &[u8],
    mut xyz: [i32; 3],
    normals: bool,
    mask: u16,
    matrix: Option<&[i32; 15]>,
) {
    if kind == 0 {
        let (mut sum, mut count) = ([0i32; 3], 0i32);
        for &label in labels {
            for (i, &bone) in m.vertex_labels.iter().enumerate() {
                if label == bone && selected(&m.vertex_sources, i, mask) {
                    sum = add(sum, m.vertices[i]);
                    count += 1;
                }
            }
        }
        let shift = xyz.map(|v| v.wrapping_shl(4));
        o.xyz = if count > 0 {
            o.valid = true;
            add(sum.map(|n| n / count), shift)
        } else {
            shift
        };
        return;
    }
    if kind == 1 {
        if let Some(b) = matrix {
            xyz = mv(b, xyz);
        }
        xyz = xyz.map(|v| v.wrapping_shl(4));
        for &label in labels {
            for (i, &bone) in m.vertex_labels.iter().enumerate() {
                if label == bone && selected(&m.vertex_sources, i, mask) {
                    m.vertices[i] = add(m.vertices[i], xyz);
                }
            }
        }
        return;
    }
    if matches!(kind, 2 | 3) {
        if let Some(b) = matrix {
            let local = [
                b[12].wrapping_shl(4),
                b[13].wrapping_shl(4),
                b[14].wrapping_shl(4),
            ];
            let global = [
                b[9].wrapping_shl(4),
                b[10].wrapping_shl(4),
                b[11].wrapping_shl(4),
            ];
            if o.valid {
                o.xyz = std::array::from_fn(|i| dot(col(b, i), o.xyz).wrapping_add(local[i]));
                o.valid = false;
            }
            let r = if kind == 2 {
                rotation(xyz)
            } else {
                let mut s = [0; 9];
                for i in 0..3 {
                    s[i * 4] = xyz[i].wrapping_shl(15) >> 7;
                }
                s
            };
            let pivot = add(mv(&r, o.xyz.map(i32::wrapping_neg)), o.xyz);
            let mut composed = [0; 9];
            for i in 0..3 {
                for j in 0..3 {
                    composed[i * 3 + j] = dot(row(&r, i), row(b, j));
                }
            }
            let mut final_matrix = [0; 9];
            for i in 0..3 {
                for j in 0..3 {
                    final_matrix[i * 3 + j] = dot(row(b, i), col(&composed, j));
                }
            }
            let translation = add(mv(b, add(mv(&r, local), pivot)), global);
            for &label in labels {
                for (i, &bone) in m.vertex_labels.iter().enumerate() {
                    if label == bone && selected(&m.vertex_sources, i, mask) {
                        m.vertices[i] = add(mv(&final_matrix, m.vertices[i]), translation);
                    }
                }
            }
        } else {
            for &label in labels {
                for (i, &bone) in m.vertex_labels.iter().enumerate() {
                    if label != bone || !selected(&m.vertex_sources, i, mask) {
                        continue;
                    }
                    let v = &mut m.vertices[i];
                    for (a, n) in v.iter_mut().enumerate() {
                        *n = n.wrapping_sub(o.xyz[a]);
                    }
                    if kind == 2 {
                        rotate(v, xyz, false);
                    } else {
                        for a in 0..3 {
                            v[a] = v[a].wrapping_mul(xyz[a]) >> 7;
                        }
                    }
                    *v = add(*v, o.xyz);
                }
            }
            // Without high detail lighting the colours are packed from the
            // rest pose's normals (see `sd_light`).
            if kind == 2 && normals && !m.surfaces.is_empty() && !crate::sd_light::enabled() {
                for &label in labels {
                    for (i, &bone) in m.vertex_labels.iter().enumerate() {
                        if label != bone || !selected(&m.vertex_sources, i, mask) {
                            continue;
                        }
                        for (face, surface) in m.faces.iter().zip(&mut m.surfaces) {
                            for (corner, &vertex) in face.iter().enumerate() {
                                if vertex == i {
                                    let n = &mut surface.normals[corner];
                                    let mut v = [n[0], n[1], n[2]];
                                    rotate(&mut v, xyz, true);
                                    n[..3].copy_from_slice(&v);
                                }
                            }
                        }
                    }
                }
                o.normals_dirty = true;
            }
        }
    } else if kind == 5 || kind == 7 {
        if m.face_labels.is_empty() || (kind == 5 && m.alpha.is_empty()) {
            return;
        }
        for &label in labels {
            let mut any = false;
            for (i, &bone) in m.face_labels.iter().enumerate() {
                if label != bone {
                    continue;
                }
                any = true;
                if !selected(&m.face_sources, i, mask) {
                    continue;
                }
                if kind == 5 {
                    m.alpha[i] = i32::from(m.alpha[i])
                        .wrapping_add(xyz[0].wrapping_mul(8))
                        .clamp(0, 255) as u8;
                } else {
                    let c = i32::from(m.colors[i]);
                    let h = (c >> 10).wrapping_add(xyz[0]) & 63;
                    let s = ((c >> 7) & 7).wrapping_add(xyz[1] / 4).clamp(0, 7);
                    let l = (c & 127).wrapping_add(xyz[2]).clamp(0, 127);
                    m.colors[i] = ((h << 10) | (s << 7) | l) as u16;
                }
            }
            if any {
                o.colors_dirty = true;
            }
        }
    }
}

/// GL truncates a normal to signed16 bits after each axis, not only after all axes.
pub(crate) fn rotate_normal(v: &mut [i32; 3], xyz: [i32; 3]) {
    rotate(v, xyz, true);
}
