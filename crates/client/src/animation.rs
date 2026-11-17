//! RT4 animation decoding, GL full/dual/masked poses, and unmasked software poses.
use crate::model::Model;
use rs09_protocol::{Error, Result, buffer::Reader};

#[derive(Clone, Debug)]
pub struct Base {
    pub id: u16,
    pub types: Vec<u8>,
    pub shadow: Vec<bool>,
    pub parts: Vec<u16>,
    pub labels: Vec<Vec<u8>>,
}
impl Base {
    pub fn decode(id: u16, data: &[u8]) -> Result<Self> {
        let mut r = Reader::new(data);
        let n = r.g1()? as usize;
        let types = r.take(n)?.to_vec();
        let shadow = r.take(n)?.iter().map(|&v| v == 1).collect();
        let parts = (0..n)
            .map(|_| r.g2().map(|n| n as u16))
            .collect::<Result<Vec<_>>>()?;
        let counts = r.take(n)?.to_vec();
        let mut labels = Vec::with_capacity(n);
        for count in counts {
            labels.push(r.take(count as usize)?.to_vec());
        }
        Ok(Self {
            id,
            types,
            shadow,
            parts,
            labels,
        })
    }
}

#[derive(Clone, Debug)]
pub struct Transform {
    pub index: usize,
    pub xyz: [i32; 3],
    pub previous_origin: Option<usize>,
    pub flags: u8,
}
#[derive(Clone, Debug)]
pub struct Frame {
    pub base: u16,
    pub transforms: Vec<Transform>,
}
impl Frame {
    pub fn decode(data: &[u8], base: &Base) -> Result<Self> {
        let mut h = Reader::new(data);
        let id = h.g2()? as u16;
        if id != base.id {
            return Err(Error::new(0, "animation base mismatch"));
        }
        let n = h.g1()? as usize;
        if n > base.types.len() {
            return Err(Error::new(2, "animation exceeds base"));
        }
        let attributes = h.take(n)?;
        let mut r = Reader::new(data);
        r.seek(3 + n)?;
        let (mut origin, mut used) = (None, None);
        let mut transforms = Vec::new();
        for (index, &a) in attributes.iter().enumerate() {
            let kind = base.types[index];
            if kind == 0 {
                origin = Some(index);
            }
            if a == 0 {
                continue;
            }
            if kind == 0 {
                used = Some(index);
            }
            let mut xyz = [if kind == 3 { 128 } else { 0 }; 3];
            for (axis, value) in xyz.iter_mut().enumerate() {
                if a & (1 << axis) != 0 {
                    *value = i32::from(r.gsmart()? as i16);
                }
                if kind == 2 {
                    *value = ((*value & 255) << 3) + ((*value >> 8) & 7);
                }
            }
            let previous_origin = if (1..=3).contains(&kind) && origin > used {
                used = origin;
                origin
            } else {
                None
            };
            transforms.push(Transform {
                index,
                xyz,
                previous_origin,
                flags: (a >> 3) & 3,
            });
        }
        if r.remaining() != 0 {
            return Err(Error::new(r.position(), "trailing animation data"));
        }
        Ok(Self {
            base: id,
            transforms,
        })
    }
}

#[derive(Clone, Debug)]
pub struct Sequence {
    pub frames: Vec<u32>,
    pub delays: Vec<u16>,
    pub replay_offset: Option<usize>,
    pub sounds: Vec<Vec<u32>>,
    pub transform_normals: bool,
    pub tween: bool,
    pub stretches: bool,
    pub frame_group: Option<Vec<bool>>,
    pub priority: i32,
    pub replay_count: i32,
    pub loop_type: i32,
    pub move_type: i32,
    pub exact_move: i32,
    pub mainhand: i32,
    pub offhand: i32,
    pub secondary_frames: Vec<u32>,
    pub update_shadows: bool,
}
impl Default for Sequence {
    fn default() -> Self {
        Self {
            frames: vec![],
            delays: vec![],
            replay_offset: None,
            sounds: vec![],
            transform_normals: false,
            tween: false,
            stretches: false,
            frame_group: None,
            priority: 5,
            replay_count: 99,
            loop_type: -1,
            move_type: -1,
            exact_move: 2,
            mainhand: -1,
            offhand: -1,
            secondary_frames: vec![],
            update_shadows: false,
        }
    }
}
impl Sequence {
    pub fn decode(data: &[u8]) -> Result<Self> {
        let mut r = Reader::new(data);
        let mut s = Self::default();
        loop {
            match r.g1()? {
                0 => {
                    if s.loop_type == -1 {
                        s.loop_type = if s.frame_group.is_some() { 2 } else { 0 };
                    }
                    if s.move_type == -1 {
                        s.move_type = if s.frame_group.is_some() { 2 } else { 0 };
                    }
                    return Ok(s);
                }
                1 => {
                    let n = r.g2()? as usize;
                    for _ in 0..n {
                        s.delays.push(r.g2()? as u16);
                    }
                    for _ in 0..n {
                        s.frames.push(r.g2()? as u32);
                    }
                    for frame in &mut s.frames {
                        *frame |= (r.g2()? as u32) << 16;
                    }
                }
                2 => s.replay_offset = Some(r.g2()? as usize),
                3 => {
                    let n = r.g1()? as usize;
                    let mut mask = vec![false; 256];
                    for _ in 0..n {
                        mask[r.g1()? as usize] = true;
                    }
                    s.frame_group = Some(mask);
                }
                4 => s.stretches = true,
                16 => s.update_shadows = true,
                14 => s.transform_normals = true,
                15 => s.tween = true,
                5 => s.priority = r.g1()?,
                6 => s.mainhand = r.g2()?,
                7 => s.offhand = r.g2()?,
                8 => s.replay_count = r.g1()?,
                9 => s.loop_type = r.g1()?,
                10 => s.move_type = r.g1()?,
                11 => s.exact_move = r.g1()?,
                12 => {
                    let n = r.g1()? as usize;
                    for _ in 0..n {
                        s.secondary_frames.push(r.g2()? as u32);
                    }
                    for frame in &mut s.secondary_frames {
                        *frame |= (r.g2()? as u32) << 16;
                    }
                }
                13 => {
                    let n = r.g2()? as usize;
                    for _ in 0..n {
                        let m = r.g1()? as usize;
                        let mut sounds = Vec::new();
                        if m > 0 {
                            sounds.push(r.g3()? as u32);
                            for _ in 1..m {
                                sounds.push(r.g2()? as u32);
                            }
                        }
                        s.sounds.push(sounds);
                    }
                }
                _ => return Err(Error::new(r.position() - 1, "unknown sequence opcode")),
            }
        }
    }

    /// Repeating preview cursor; action replay limits belong to the future entity state machine.
    pub fn preview_cursor(&self, tick: u64) -> Option<(usize, usize, i32, i32)> {
        let total: u64 = self.delays.iter().map(|&n| u64::from(n.max(1))).sum();
        if total == 0 || self.frames.len() != self.delays.len() {
            return None;
        }
        let mut t = tick % total;
        for (i, &delay) in self.delays.iter().enumerate() {
            let d = u64::from(delay.max(1));
            if t < d {
                return Some((i, (i + 1) % self.frames.len(), t as i32, d as i32));
            }
            t -= d;
        }
        None
    }
}

pub fn sin_cos(angle: i32) -> (i32, i32) {
    let t = f64::from(angle & 2047) * 0.0030679615;
    ((t.sin() * 65536.0) as i32, (t.cos() * 65536.0) as i32)
}

fn mix(a: i32, b: i32, t: i32, length: i32, period: i32) -> i32 {
    let mut delta = b - a;
    if period > 0 {
        delta &= period - 1;
        if delta >= period / 2 {
            delta -= period;
        }
    }
    let value = a + delta * t / length;
    if period > 0 {
        value & (period - 1)
    } else {
        value
    }
}

/// Existing software full-part path. Masked/BAS overlays use the GL path below.
pub fn animate(
    model: &Model,
    base: &Base,
    frame: &Frame,
    next: Option<&Frame>,
    delta: i32,
    length: i32,
) -> Model {
    animate_impl(
        model,
        base,
        frame,
        next,
        delta,
        length,
        Options {
            hd: None,
            groups: None,
            parts: 65535,
            matrix: None,
        },
        &mut false,
    )
}
#[derive(Clone, Copy)]
pub struct Pose<'a> {
    pub base: &'a Base,
    pub frame: &'a Frame,
    pub next: Option<&'a Frame>,
    pub delta: i32,
    pub length: i32,
}
struct Options<'a> {
    hd: Option<bool>,
    groups: Option<(&'a [bool], bool)>,
    parts: u16,
    matrix: Option<&'a [i32; 15]>,
}
/// GL retains four fractional bits until each complete animation call finishes.
pub fn animate_hd(
    model: &Model,
    base: &Base,
    frame: &Frame,
    next: Option<&Frame>,
    delta: i32,
    length: i32,
    normals: bool,
) -> Model {
    animate_hd_with_state(
        model,
        Pose {
            base,
            frame,
            next,
            delta,
            length,
        },
        normals,
        &mut false,
    )
}
pub fn animate_hd_with_state(
    model: &Model,
    pose: Pose<'_>,
    normals: bool,
    origin_valid: &mut bool,
) -> Model {
    animate_masked_hd(model, pose, 65535, None, normals, origin_valid)
}
pub fn animate_masked_hd(
    model: &Model,
    p: Pose<'_>,
    parts: u16,
    matrix: Option<&[i32; 15]>,
    normals: bool,
    origin_valid: &mut bool,
) -> Model {
    if model.vertex_labels.is_empty() || p.frame.base != p.base.id {
        return model.clone();
    }
    let mut scaled = model.clone();
    for v in &mut scaled.vertices {
        for n in v {
            *n = n.wrapping_shl(4);
        }
    }
    let mut result = animate_impl(
        &scaled,
        p.base,
        p.frame,
        p.next,
        p.delta,
        p.length,
        Options {
            hd: Some(normals),
            groups: None,
            parts,
            matrix,
        },
        origin_valid,
    );
    for v in &mut result.vertices {
        for n in v {
            *n = n.wrapping_add(7) >> 4;
        }
    }
    result
}
/// Action groups precede locomotion groups; the pivot resets between passes.
pub fn animate_dual_hd(
    model: &Model,
    primary: Pose<'_>,
    secondary: Option<Pose<'_>>,
    mask: Option<&[bool]>,
    normals: bool,
) -> Model {
    animate_dual_hd_with_state(model, primary, secondary, mask, normals, &mut false)
}
pub fn animate_dual_hd_with_state(
    model: &Model,
    primary: Pose<'_>,
    secondary: Option<Pose<'_>>,
    mask: Option<&[bool]>,
    normals: bool,
    origin_valid: &mut bool,
) -> Model {
    if model.vertex_labels.is_empty() {
        return model.clone();
    }
    let mut scaled = model.clone();
    for v in &mut scaled.vertices {
        for n in v {
            *n = n.wrapping_shl(4);
        }
    }
    let dual = secondary.zip(mask);
    let mut result = animate_impl(
        &scaled,
        primary.base,
        primary.frame,
        primary.next,
        primary.delta,
        primary.length,
        Options {
            hd: Some(normals),
            groups: dual.map(|(_, m)| (m, false)),
            parts: 65535,
            matrix: None,
        },
        origin_valid,
    );
    if let Some((second, mask)) = dual {
        result = animate_impl(
            &result,
            primary.base,
            second.frame,
            second.next,
            second.delta,
            second.length,
            Options {
                hd: Some(normals),
                groups: Some((mask, true)),
                parts: 65535,
                matrix: None,
            },
            origin_valid,
        );
    }
    for v in &mut result.vertices {
        for n in v {
            *n = n.wrapping_add(7) >> 4;
        }
    }
    result
}
fn apply_transform(
    model: &mut Model,
    origin: &mut crate::animation_parts::Origin,
    base: &Base,
    index: usize,
    xyz: [i32; 3],
    options: &Options<'_>,
) {
    let parts = options.parts & base.parts[index];
    if let Some(normals) = options.hd.filter(|_| parts != 65535) {
        crate::animation_parts::transform(
            model,
            origin,
            base.types[index],
            &base.labels[index],
            xyz,
            normals,
            parts,
            options.matrix,
        );
    } else {
        transform(
            model,
            &mut origin.xyz,
            base.types[index],
            &base.labels[index],
            xyz,
            options.hd,
        );
    }
}
#[allow(clippy::too_many_arguments)]
fn animate_impl(
    model: &Model,
    base: &Base,
    frame: &Frame,
    next: Option<&Frame>,
    delta: i32,
    length: i32,
    options: Options<'_>,
    origin_valid: &mut bool,
) -> Model {
    let mask = options.groups;
    let mut result = model.clone();
    if model.vertex_labels.is_empty() || frame.base != base.id {
        return result;
    }
    let mut origin = crate::animation_parts::Origin {
        valid: *origin_valid,
        ..Default::default()
    };
    let next = next.filter(|f| f.base == frame.base && delta != 0 && length > 0);
    if let Some(next) = next {
        let (mut a, mut b) = (0, 0);
        for i in 0..base.types.len() {
            let first = frame.transforms.get(a).filter(|t| t.index == i);
            let second = next.transforms.get(b).filter(|t| t.index == i);
            if first.is_none() && second.is_none() {
                continue;
            }
            let kind = base.types[i];
            if mask
                .is_some_and(|(m, value)| kind != 0 && m.get(i).copied().unwrap_or(false) != value)
            {
                a += usize::from(first.is_some());
                b += usize::from(second.is_some());
                continue;
            }
            let default = [if kind == 3 { 128 } else { 0 }; 3];
            let x = first.map_or(default, |t| t.xyz);
            let y = second.map_or(default, |t| t.xyz);
            let stop =
                first.is_some_and(|t| t.flags & 2 != 0) || second.is_some_and(|t| t.flags & 1 != 0);
            let mut xyz = x;
            if !stop {
                for axis in 0..3 {
                    xyz[axis] = mix(
                        x[axis],
                        y[axis],
                        delta,
                        length,
                        if kind == 2 {
                            2048
                        } else if kind == 7 && axis == 0 {
                            64
                        } else {
                            0
                        },
                    );
                }
            }
            if let Some(p) = first
                .and_then(|t| t.previous_origin)
                .or_else(|| second.and_then(|t| t.previous_origin))
            {
                apply_transform(&mut result, &mut origin, base, p, [0; 3], &options);
            }
            apply_transform(&mut result, &mut origin, base, i, xyz, &options);
            if first.is_some() {
                a += 1;
            }
            if second.is_some() {
                b += 1;
            }
        }
    } else {
        for t in &frame.transforms {
            if mask.is_some_and(|(m, value)| {
                base.types[t.index] != 0 && m.get(t.index).copied().unwrap_or(false) != value
            }) {
                continue;
            }
            if let Some(p) = t.previous_origin {
                apply_transform(&mut result, &mut origin, base, p, [0; 3], &options);
            }
            apply_transform(&mut result, &mut origin, base, t.index, t.xyz, &options);
        }
    }
    *origin_valid = origin.valid;
    result
}

fn transform(
    m: &mut Model,
    origin: &mut [i32; 3],
    kind: u8,
    labels: &[u8],
    mut xyz: [i32; 3],
    hd: Option<bool>,
) {
    if hd.is_some() && (kind == 0 || kind == 1) {
        xyz = xyz.map(|v| v.wrapping_shl(4));
    }
    if kind == 0 {
        let mut sum = [0i32; 3];
        let mut count = 0;
        for &label in labels {
            for (i, &bone) in m.vertex_labels.iter().enumerate() {
                if label == bone {
                    for (axis, value) in sum.iter_mut().enumerate() {
                        *value = value.wrapping_add(m.vertices[i][axis]);
                    }
                    count += 1;
                }
            }
        }
        for axis in 0..3 {
            origin[axis] = if count > 0 { sum[axis] / count } else { 0 };
            origin[axis] = origin[axis].wrapping_add(xyz[axis]);
        }
        return;
    }
    if matches!(kind, 1..=3) {
        for &label in labels {
            for (i, &bone) in m.vertex_labels.iter().enumerate() {
                if label != bone {
                    continue;
                }
                let v = &mut m.vertices[i];
                if kind == 1 {
                    for axis in 0..3 {
                        v[axis] = v[axis].wrapping_add(xyz[axis]);
                    }
                    continue;
                }
                for axis in 0..3 {
                    v[axis] = v[axis].wrapping_sub(origin[axis]);
                }
                if kind == 3 {
                    for axis in 0..3 {
                        v[axis] = if hd.is_some() {
                            v[axis].wrapping_mul(xyz[axis]) >> 7
                        } else {
                            v[axis].wrapping_mul(xyz[axis]) / 128
                        };
                    }
                } else {
                    rotate(v, xyz);
                    // Without high detail lighting the colours are packed
                    // from the rest pose's normals (see `sd_light`).
                    if hd == Some(true) && !crate::sd_light::enabled() {
                        for (face, surface) in m.faces.iter().zip(&mut m.surfaces) {
                            for (corner, &vertex) in face.iter().enumerate() {
                                if vertex == i {
                                    let n = &mut surface.normals[corner];
                                    let mut normal = [n[0], n[1], n[2]];
                                    crate::animation_parts::rotate_normal(&mut normal, xyz);
                                    for axis in 0..3 {
                                        n[axis] = normal[axis] as i16 as i32;
                                    }
                                }
                            }
                        }
                    }
                }
                for axis in 0..3 {
                    v[axis] = v[axis].wrapping_add(origin[axis]);
                }
            }
        }
    } else if kind == 5 || kind == 7 {
        for &label in labels {
            for (i, &bone) in m.face_labels.iter().enumerate() {
                if label != bone {
                    continue;
                }
                if kind == 5 {
                    m.alpha[i] = (i32::from(m.alpha[i]) + xyz[0] * 8).clamp(0, 255) as u8;
                } else {
                    let c = i32::from(m.colors[i]);
                    let h = ((c >> 10) + xyz[0]) & 63;
                    let s = (((c >> 7) & 7) + if hd.is_some() { xyz[1] / 4 } else { xyz[1] })
                        .clamp(0, 7);
                    let l = ((c & 127) + xyz[2]).clamp(0, 127);
                    m.colors[i] = ((h << 10) | (s << 7) | l) as u16;
                }
            }
        }
    }
}

fn rotate(v: &mut [i32; 3], xyz: [i32; 3]) {
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
        let (old_a, old_b) = (v[a], v[b]);
        let muladd = |x: i32, y: i32, z: i32, w: i32| {
            x.wrapping_mul(y)
                .wrapping_add(z.wrapping_mul(w))
                .wrapping_add(32767)
                >> 16
        };
        if axis == 0 {
            v[a] = muladd(old_a, c, old_b, -s);
            v[b] = muladd(old_a, s, old_b, c);
        } else {
            v[a] = muladd(old_a, c, old_b, s);
            v[b] = muladd(old_b, c, old_a, -s);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rotations_tween_across_zero_and_honor_flags() {
        assert_eq!(mix(2040, 8, 1, 2, 2048), 0);
        assert_eq!(mix(8, 2040, 1, 2, 2048), 0);
        assert_eq!(mix(63, 1, 1, 2, 64), 0);
    }
    #[test]
    fn implicit_origin_and_integer_rotation() {
        let base = Base::decode(7, &[2, 0, 2, 1, 1, 255, 255, 255, 255, 1, 1, 0, 1]).unwrap();
        let frame = Frame::decode(&[0, 7, 2, 0, 4, 192, 64], &base).unwrap();
        assert_eq!(frame.transforms[0].previous_origin, Some(0));
        assert_eq!(frame.transforms[0].xyz, [0, 0, 512]);
        let model = Model {
            vertices: vec![[10, 0, 0], [20, 0, 0]],
            vertex_labels: vec![0, 1],
            ..Model::default()
        };
        let pose = animate(&model, &base, &frame, None, 0, 1);
        assert_eq!(pose.vertices, vec![[10, 0, 0], [10, -10, 0]]);
    }
    #[test]
    fn preview_timing_uses_cache_delays() {
        let s = Sequence {
            frames: vec![1, 2],
            delays: vec![2, 3],
            ..Sequence::default()
        };
        assert_eq!(s.preview_cursor(1), Some((0, 1, 1, 2)));
        assert_eq!(s.preview_cursor(2), Some((1, 0, 0, 3)));
        assert_eq!(s.preview_cursor(5), Some((0, 1, 0, 2)));
    }
}
