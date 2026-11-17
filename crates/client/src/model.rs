//! RT4 RawModel::decodeOld/decodeNew. Coordinates stay in integer cache units.
use rs09_protocol::{Error, Result, buffer::Reader};

#[derive(Clone, Debug, Default)]
pub struct Model {
    pub lighting: [i32; 2],
    pub has_priorities: bool,
    pub sort_by_priority: bool,
    pub draw_order: Vec<usize>,
    pub surfaces: Vec<crate::surface::Surface>,
    pub vertices: Vec<[i32; 3]>,
    pub faces: Vec<[usize; 3]>,
    pub colors: Vec<u16>,
    pub alpha: Vec<u8>,
    pub vertex_labels: Vec<u8>,
    /// Original RawModel assembly source bits, empty for an uncombined model.
    pub vertex_sources: Vec<u16>,
    pub face_sources: Vec<u16>,
    /// Per-part BAS inverse/forward basis used by slot animation overlays.
    pub part_matrices: Vec<Option<[i32; 15]>>,
    pub face_labels: Vec<u8>,
    pub textures: Vec<i32>,
    pub texture_indices: Vec<i32>,
    pub texture_faces: Vec<[usize; 3]>,
    pub texture_types: Vec<u8>,
    pub texture_scales: Vec<[u16; 3]>,
    pub texture_rotation: Vec<u8>,
    pub texture_direction: Vec<u8>,
    pub texture_speed: Vec<i8>,
    pub texture_translation: Vec<[i8; 2]>,
    pub face_info: Vec<u8>,
    pub priorities: Vec<u8>,
}

fn at(data: &[u8], offset: usize) -> Result<Reader<'_>> {
    let mut r = Reader::new(data);
    r.seek(offset)?;
    Ok(r)
}

impl Model {
    /// RT4's model assembler deduplicates equal coordinates in face traversal order.
    /// A shared mesh is required before animating: origins can span body parts.
    pub fn combine(models: &[Self]) -> Self {
        Self::combine_slots(&models.iter().map(Some).collect::<Vec<_>>())
    }
    /// Null slots retain their original source-bit position. Welded vertices OR
    /// every contributing slot, including texture-only vertex references.
    pub fn combine_slots(models: &[Option<&Self>]) -> Self {
        use std::collections::BTreeMap;
        let present = models.iter().filter_map(|m| *m).collect::<Vec<_>>();
        let mut result = Self::default();
        let mut vertices = BTreeMap::new();
        result.has_priorities = present.iter().any(|m| m.has_priorities)
            || present
                .windows(2)
                .any(|m| m[0].priorities.first() != m[1].priorities.first());
        let face_labels = present.iter().any(|m| !m.face_labels.is_empty());
        fn vertex(
            result: &mut Model,
            known: &mut BTreeMap<[i32; 3], usize>,
            m: &Model,
            index: usize,
            source: u16,
        ) -> usize {
            let point = m.vertices[index];
            if let Some(&v) = known.get(&point) {
                result.vertex_sources[v] |= source;
                return v;
            }
            let v = result.vertices.len();
            result.vertices.push(point);
            result.vertex_sources.push(source);
            result
                .vertex_labels
                .push(m.vertex_labels.get(index).copied().unwrap_or(0));
            known.insert(point, v);
            v
        }
        for (slot, model) in models.iter().enumerate() {
            let Some(model) = model else { continue };
            let source = 1u32.wrapping_shl(slot as u32) as u16;
            let texture_offset = result.texture_faces.len() as i32;
            for (i, face) in model.faces.iter().enumerate() {
                let face = face.map(|v| vertex(&mut result, &mut vertices, model, v, source));
                result.faces.push(face);
                result.face_sources.push(source);
                result.colors.push(model.colors[i]);
                result.alpha.push(model.alpha[i]);
                result.face_info.push(model.face_info[i]);
                result.priorities.push(model.priorities[i]);
                if face_labels {
                    result
                        .face_labels
                        .push(model.face_labels.get(i).copied().unwrap_or(0));
                }
                result.textures.push(model.textures[i]);
                let t = model.texture_indices[i];
                result
                    .texture_indices
                    .push(if t < 0 { -1 } else { t + texture_offset });
            }
            for (i, &face) in model.texture_faces.iter().enumerate() {
                let kind = model.texture_types[i];
                let face = if kind == 0 {
                    face.map(|v| vertex(&mut result, &mut vertices, model, v, source))
                } else {
                    face
                };
                result.texture_faces.push(face);
                result.texture_types.push(kind);
                result.texture_scales.push(model.texture_scales[i]);
                result.texture_rotation.push(model.texture_rotation[i]);
                result.texture_direction.push(model.texture_direction[i]);
                result.texture_speed.push(model.texture_speed[i]);
                result
                    .texture_translation
                    .push(model.texture_translation[i]);
            }
        }
        result
    }

    pub fn decode(data: &[u8]) -> Result<Self> {
        if data.len() < 18 {
            return Err(Error::new(0, "truncated model footer"));
        }
        let new = data.ends_with(&[255, 255]);
        let footer = if new { 23 } else { 18 };
        if data.len() < footer {
            return Err(Error::new(0, "truncated model footer"));
        }
        let mut h = at(data, data.len() - footer)?;
        let nv = h.g2()? as usize;
        let nf = h.g2()? as usize;
        let nt = h.g1()? as usize;
        let info = h.g1()?;
        let priority = h.g1()?;
        let alpha = h.g1()? == 1;
        let face_bones = h.g1()? == 1;
        let has_textures = new && h.g1()? == 1;
        let vertex_bones = h.g1()? == 1;
        let dx_len = h.g2()? as usize;
        let dy_len = h.g2()? as usize;
        let dz_len = h.g2()? as usize;
        let index_len = h.g2()? as usize;
        let tex_index_len = if new { h.g2()? as usize } else { 0 };
        let mut m = Self {
            has_priorities: priority == 255,
            vertices: Vec::with_capacity(nv),
            faces: Vec::with_capacity(nf),
            colors: vec![0; nf],
            alpha: vec![0; nf],
            textures: vec![-1; nf],
            texture_indices: vec![-1; nf],
            face_info: vec![0; nf],
            priorities: vec![priority as u8; nf],
            ..Self::default()
        };
        if vertex_bones {
            m.vertex_labels = vec![0; nv];
        }
        if face_bones {
            m.face_labels = vec![0; nf];
        }
        m.texture_types = if new {
            data.get(..nt)
                .ok_or(Error::new(0, "truncated texture types"))?
                .to_vec()
        } else {
            vec![0; nt]
        };
        let simple = m.texture_types.iter().filter(|&&t| t == 0).count();
        let complex = m
            .texture_types
            .iter()
            .filter(|&&t| (1..=3).contains(&t))
            .count();
        let cube = m.texture_types.iter().filter(|&&t| t == 2).count();
        let mut off = if new { nv + nt } else { nv };
        let (
            info_off,
            type_off,
            pri_off,
            bones_off,
            vertex_off,
            alpha_off,
            index_off,
            tex_off,
            tex_index_off,
            color_off,
            pmn_off,
            dx_off,
            dy_off,
            dz_off,
            complex_off,
        );
        if new {
            info_off = off;
            if info & 1 != 0 {
                off += nf;
            }
            type_off = off;
            off += nf;
            pri_off = off;
            if priority == 255 {
                off += nf;
            }
            bones_off = off;
            if face_bones {
                off += nf;
            }
            vertex_off = off;
            if vertex_bones {
                off += nv;
            }
            alpha_off = off;
            if alpha {
                off += nf;
            }
            index_off = off;
            off += index_len;
            tex_off = off;
            if has_textures {
                off += nf * 2;
            }
            tex_index_off = off;
            off += tex_index_len;
            color_off = off;
            off += nf * 2;
            dx_off = off;
            off += dx_len;
            dy_off = off;
            off += dy_len;
            dz_off = off;
            off += dz_len;
            pmn_off = off;
            off += simple * 6;
            complex_off = off;
            off += complex * 6 + complex * 6 + complex * 3 + cube * 2;
        } else {
            type_off = off;
            off += nf;
            pri_off = off;
            if priority == 255 {
                off += nf;
            }
            bones_off = off;
            if face_bones {
                off += nf;
            }
            info_off = off;
            if info == 1 {
                off += nf;
            }
            vertex_off = off;
            if vertex_bones {
                off += nv;
            }
            alpha_off = off;
            if alpha {
                off += nf;
            }
            index_off = off;
            off += index_len;
            color_off = off;
            off += nf * 2;
            pmn_off = off;
            off += nt * 6;
            dx_off = off;
            off += dx_len;
            dy_off = off;
            off += dy_len;
            dz_off = off;
            off += dz_len;
            tex_off = off;
            tex_index_off = off;
            complex_off = off;
        }
        if off > data.len() - footer {
            return Err(Error::new(off, "model streams exceed payload"));
        }
        let mut flags = at(data, if new { nt } else { 0 })?;
        let mut xs = at(data, dx_off)?;
        let mut ys = at(data, dy_off)?;
        let mut zs = at(data, dz_off)?;
        let mut labels = at(data, vertex_off)?;
        let mut previous = [0i32; 3];
        for i in 0..nv {
            let f = flags.g1()?;
            for (axis, stream) in [&mut xs, &mut ys, &mut zs].into_iter().enumerate() {
                if f & (1 << axis) != 0 {
                    previous[axis] = previous[axis].wrapping_add(stream.gsmart()?);
                }
            }
            m.vertices.push(previous);
            if vertex_bones {
                m.vertex_labels[i] = labels.g1()? as u8;
            }
        }
        let mut colors = at(data, color_off)?;
        let mut infos = at(data, info_off)?;
        let mut priorities = at(data, pri_off)?;
        let mut alphas = at(data, alpha_off)?;
        let mut bones = at(data, bones_off)?;
        let mut textures = at(data, tex_off)?;
        let mut tex_indices = at(data, tex_index_off)?;
        for i in 0..nf {
            m.colors[i] = colors.g2()? as u16;
            if (new && info & 1 != 0) || (!new && info == 1) {
                let flags = infos.g1()? as u8;
                m.face_info[i] = if new { flags } else { flags & 1 };
                if !new && flags & 2 != 0 {
                    m.textures[i] = i32::from(m.colors[i] as i16);
                    m.colors[i] = 127;
                    m.texture_indices[i] = i32::from(flags >> 2);
                }
            }
            if priority == 255 {
                m.priorities[i] = priorities.g1()? as u8;
            }
            if alpha {
                m.alpha[i] = alphas.g1()? as u8;
            }
            if face_bones {
                m.face_labels[i] = bones.g1()? as u8;
            }
            if has_textures {
                m.textures[i] = textures.g2()? - 1;
                if nt > 0 && m.textures[i] != -1 {
                    m.texture_indices[i] = tex_indices.g1()? - 1;
                }
            }
        }
        let mut types = at(data, type_off)?;
        let mut indices = at(data, index_off)?;
        let (mut a, mut b, mut c, mut last) = (0i32, 0i32, 0i32, 0i32);
        for _ in 0..nf {
            match types.g1()? {
                1 => {
                    a = last.wrapping_add(indices.gsmart()?);
                    b = a.wrapping_add(indices.gsmart()?);
                    c = b.wrapping_add(indices.gsmart()?);
                }
                2 => {
                    b = c;
                    c = last.wrapping_add(indices.gsmart()?);
                }
                3 => {
                    a = c;
                    c = last.wrapping_add(indices.gsmart()?);
                }
                4 => {
                    std::mem::swap(&mut a, &mut b);
                    c = last.wrapping_add(indices.gsmart()?);
                }
                _ => {
                    return Err(Error::new(
                        types.position() - 1,
                        "invalid triangle strip opcode",
                    ));
                }
            }
            last = c;
            if [a, b, c].iter().any(|&v| v < 0 || v as usize >= nv) {
                return Err(Error::new(
                    indices.position(),
                    "model face vertex out of bounds",
                ));
            }
            m.faces.push([a as usize, b as usize, c as usize]);
        }
        let mut simple_data = at(data, pmn_off)?;
        let mut complex_data = at(data, complex_off)?;
        let mut scales = at(data, complex_off + complex * 6)?;
        let mut rotations = at(data, complex_off + complex * 12)?;
        let mut directions = at(data, complex_off + complex * 13)?;
        let mut speeds = at(data, complex_off + complex * 14)?;
        for &kind in &m.texture_types {
            let r = if kind == 0 {
                &mut simple_data
            } else {
                &mut complex_data
            };
            m.texture_faces
                .push([r.g2()? as usize, r.g2()? as usize, r.g2()? as usize]);
            if kind == 0 {
                m.texture_scales.push([0; 3]);
                m.texture_rotation.push(0);
                m.texture_direction.push(0);
                m.texture_speed.push(0);
                m.texture_translation.push([0; 2]);
            } else {
                m.texture_scales.push([
                    scales.g2()? as u16,
                    scales.g2()? as u16,
                    scales.g2()? as u16,
                ]);
                m.texture_rotation.push(rotations.g1()? as u8);
                m.texture_direction.push(directions.g1()? as u8);
                m.texture_speed.push(speeds.g1()? as i8);
                m.texture_translation.push(if kind == 2 {
                    [speeds.g1()? as i8, speeds.g1()? as i8]
                } else {
                    [0; 2]
                });
            }
        }
        // decodeOld removes redundant projections. GL assigns its default UV
        // orientation to these faces; retaining the basis flips those textures.
        if !new {
            for (i, index) in m.texture_indices.iter_mut().enumerate() {
                if *index >= 0 && m.texture_faces.get(*index as usize) == Some(&m.faces[i]) {
                    *index = -1;
                }
            }
        }
        Ok(m)
    }

    pub fn recolor(&mut self, pairs: &[[u16; 2]]) {
        for c in &mut self.colors {
            for &[from, to] in pairs {
                if *c == from {
                    *c = to;
                    break;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn malformed_models_are_errors() {
        for data in [vec![], vec![255; 22], vec![255; 23], vec![1; 18]] {
            assert!(Model::decode(&data).is_err());
        }
    }
}
