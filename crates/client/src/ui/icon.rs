//! Native inventory art: integer item camera, original lighting and software raster kernels.
use super::{
    item::{Catalog, ItemType},
    raster::{self, Texture, TexturedTriangle},
};
use crate::{
    animation::sin_cos,
    model::Model,
    pack::Pack,
    surface,
    texture::{self, Material},
};
use std::collections::BTreeMap;
pub struct Icons {
    pub palette: Vec<u32>,
    /// `Rasteriser.brightness`: the palette and the textures both use it.
    brightness: f32,
    /// `client.objRecolorPalette`, the script palette of CS2 5401 that
    /// `recol_p` entries index (`ObjType` 555-561); all zero until a script
    /// sets an entry.
    pub recolor: [u16; 256],
    materials: Vec<Option<Material>>,
    textures: BTreeMap<i32, Vec<u32>>,
    models: BTreeMap<i32, Model>,
}
impl Icons {
    pub fn new(visual: &Pack, brightness: f32) -> Result<Self, String> {
        Ok(Self {
            palette: surface::palette(brightness),
            brightness,
            recolor: [0; 256],
            materials: texture::materials(visual.get("materials")?).map_err(|e| e.to_string())?,
            textures: BTreeMap::new(),
            models: BTreeMap::new(),
        })
    }
    #[allow(clippy::too_many_arguments)] // Original inventory sprite inputs.
    pub fn render(
        &mut self,
        catalog: &mut Catalog,
        ui: &Pack,
        visual: &Pack,
        id: i32,
        count: i32,
        outline: i32,
        shadow: u32,
    ) -> Result<Vec<u32>, String> {
        self.render_inner(
            catalog,
            ui,
            visual,
            [id, count, outline, shadow as i32],
            false,
            0,
        )
    }
    fn render_inner(
        &mut self,
        catalog: &mut Catalog,
        ui: &Pack,
        visual: &Pack,
        args: [i32; 4],
        cert: bool,
        depth: usize,
    ) -> Result<Vec<u32>, String> {
        if depth > 64 {
            return Err("Cyclic item icon".into());
        }
        let [id, count, outline, shadow] = args;
        let mut item = catalog.get(ui, id)?;
        let variant = item.count_variant(count);
        if variant != item.id {
            item = catalog.get(ui, variant)?;
        }
        let linked = if item.certtemplate != -1 {
            Some(self.render_inner(
                catalog,
                ui,
                visual,
                [item.certlink, 10, 1, 0],
                true,
                depth + 1,
            )?)
        } else if item.lentTemplate != -1 {
            Some(self.render_inner(
                catalog,
                ui,
                visual,
                [item.lentLink, count, outline, shadow],
                false,
                depth + 1,
            )?)
        } else {
            None
        };
        let mut pixels = self.model(ui, visual, &item, outline, cert)?;
        fn border(pixels: &mut Vec<u32>, color: u32) {
            let mut out = vec![0; pixels.len()];
            for y in 0..32 {
                for x in 0..36 {
                    let i = y * 36 + x;
                    out[i] = if pixels[i] != 0 {
                        pixels[i]
                    } else if (x > 0 && pixels[i - 1] != 0)
                        || (y > 0 && pixels[i - 36] != 0)
                        || (x < 35 && pixels[i + 1] != 0)
                        || (y < 31 && pixels[i + 36] != 0)
                    {
                        color
                    } else {
                        0
                    };
                }
            }
            *pixels = out;
        }
        if outline >= 1 {
            border(&mut pixels, 1);
        }
        if outline >= 2 {
            border(&mut pixels, 0xffffff);
        }
        if shadow != 0 {
            for y in (1..32).rev() {
                for x in (1..36).rev() {
                    let i = y * 36 + x;
                    if pixels[i] == 0 && pixels[i - 37] != 0 {
                        pixels[i] = shadow as u32;
                    }
                }
            }
        }
        if let Some(mut linked) = linked {
            if item.certtemplate != -1 {
                for (i, &p) in linked.iter().enumerate() {
                    if p != 0 {
                        pixels[i] = p;
                    }
                }
            } else {
                for (i, &p) in pixels.iter().enumerate() {
                    if p != 0 {
                        linked[i] = p;
                    }
                }
                pixels = linked;
            }
        }
        Ok(pixels)
    }
    pub fn model(
        &mut self,
        ui: &Pack,
        visual: &Pack,
        item: &ItemType,
        outline: i32,
        cert: bool,
    ) -> Result<Vec<u32>, String> {
        if let std::collections::btree_map::Entry::Vacant(entry) = self.models.entry(item.model) {
            entry.insert(
                Model::decode(ui.get(&format!("model/{}", item.model))?)
                    .map_err(|e| e.to_string())?,
            );
        }
        let mut model = self.models[&item.model].clone();
        if let (Some(src), Some(dst)) = (&item.recol_s, &item.recol_d) {
            for (i, (&a, &b)) in src.iter().zip(dst).enumerate() {
                let color = item
                    .recol_p
                    .as_ref()
                    .and_then(|p| p.get(i))
                    .map_or(b as u16, |&p| self.recolor[p as u8 as usize]);
                model.recolor(&[[a as u16, color]]);
            }
        }
        if let (Some(src), Some(dst)) = (&item.retex_s, &item.retex_d) {
            for (&a, &b) in src.iter().zip(dst) {
                for texture in &mut model.textures {
                    if *texture as i16 == a as i16 {
                        *texture = b as i16 as i32;
                    }
                }
            }
        }
        let surfaces = surface::prepare(&model, &self.materials);
        let mut colors = vec![[0i32; 3]; model.faces.len()];
        let mut kinds = model.face_info.clone();
        let mut textures = model.textures.clone();
        let contrast = ((item.contrast + 768) * 71) >> 8;
        for i in 0..model.faces.len() {
            if !self
                .materials
                .get(textures[i] as usize)
                .and_then(Option::as_ref)
                .is_some_and(|m| m.repeating)
            {
                textures[i] = -1;
            }
            if model.texture_indices[i] >= 0
                && model.texture_types[model.texture_indices[i] as usize] != 0
            {
                textures[i] = -1;
            }
            if model.alpha[i] == 254 {
                kinds[i] = 3;
            }
            if model.alpha[i] == 255 {
                kinds[i] = 2;
            }
            let shade = |normal: [i32; 4]| {
                let dot = normal[0]
                    .wrapping_mul(-50)
                    .wrapping_add(normal[1].wrapping_mul(-10))
                    .wrapping_add(normal[2].wrapping_mul(-50));
                let divisor = if normal[3] == 0 {
                    contrast + contrast / 2
                } else {
                    contrast * normal[3]
                };
                item.ambient + 64 + if divisor == 0 { 0 } else { dot / divisor }
            };
            if kinds[i] == 3 && textures[i] < 0 {
                colors[i] = [128; 3];
                kinds[i] = 1;
                continue;
            }
            for j in 0..3 {
                let light = shade(surfaces[i].normals[j]);
                colors[i][j] = if textures[i] < 0 {
                    (i32::from(model.colors[i]) & 0xff80)
                        + (((i32::from(model.colors[i]) & 127) * light) >> 7).clamp(2, 126)
                } else {
                    light.clamp(2, 126)
                };
            }
            if kinds[i] == 1 {
                colors[i] = [colors[i][0]; 3];
            }
        }
        for v in &mut model.vertices {
            for (axis, scale) in [item.resizeX, item.resizeY, item.resizeZ]
                .into_iter()
                .enumerate()
            {
                v[axis] = v[axis].wrapping_mul(scale) / 128;
            }
        }
        let min_y = model.vertices.iter().map(|v| v[1]).min().unwrap_or(32767) as i16 as i32;
        let radius = (model
            .vertices
            .iter()
            .map(|v| {
                v[0].wrapping_mul(v[0])
                    .wrapping_add(v[1].wrapping_mul(v[1]))
                    .wrapping_add(v[2].wrapping_mul(v[2]))
            })
            .max()
            .unwrap_or(0)
            .max(0) as f64)
            .sqrt()
            + 0.99;
        let radius = radius as i32 as i16 as i32;
        let mut pixels = vec![0u32; 36 * 32];
        if radius * 2 >= 1600 {
            return Ok(pixels);
        }
        let zoom = if cert {
            (item.zoom2d as f64 * 1.5) as i32
        } else if outline == 2 {
            (item.zoom2d as f64 * 1.04) as i32
        } else {
            item.zoom2d
        };
        let (sp, cp) = sin_cos(item.xAngle2D);
        let offset_y = (sp.wrapping_mul(zoom) >> 16) + item.yOffset2D - min_y / 2;
        let offset_z = item.yOffset2D + (cp.wrapping_mul(zoom) >> 16);
        let projection = project(
            &model.vertices,
            [item.yAngle2D, item.zAngle2D, item.xAngle2D],
            [item.xOffset2D, offset_y, offset_z],
        )?;
        let order = face_order(&model, &projection, &kinds, radius);
        for i in order {
            let face = model.faces[i];
            let points = face.map(|v| projection[v].screen);
            if textures[i] >= 0 {
                let id = textures[i];
                let material = self.materials[id as usize]
                    .as_ref()
                    .ok_or("Missing item material")?;
                // Js5GlTextureProvider.isLowDetail: the texture's flag or the
                // high-detail textures preference turned off, which also
                // empties the provider's cache (setLowDetail).
                let low_detail = texture::low_detail(material);
                let size = if low_detail { 64 } else { 128 };
                if self
                    .textures
                    .get(&id)
                    .is_none_or(|texels| texels.len() != size * size)
                {
                    let graph = texture::Graph::decode(visual.get(&format!("texture/{id}"))?)
                        .map_err(|e| e.to_string())?;
                    // Rasteriser.fillTexturedTriangle asks the texture provider
                    // for pixels at the rasteriser's brightness.
                    self.textures.insert(
                        id,
                        graph.software_pixels(visual, size, f64::from(self.brightness))?,
                    );
                }
                let index = model.texture_indices[i];
                let anchor = if index >= 0 {
                    model.texture_faces[index as usize]
                } else {
                    face
                };
                raster::textured(
                    &mut pixels,
                    [36, 32],
                    &self.palette,
                    &Texture {
                        texels: &self.textures[&id],
                        low_detail,
                        opaque: material.opaque,
                        average: i32::from(material.average_color),
                    },
                    &TexturedTriangle {
                        points,
                        lights: colors[i],
                        view: anchor.map(|v| projection[v].view),
                        alpha: i32::from(model.alpha[i]),
                    },
                )?;
            } else {
                raster::gouraud(
                    &mut pixels,
                    [36, 32],
                    &self.palette,
                    points,
                    colors[i],
                    i32::from(model.alpha[i]),
                )?;
            }
        }
        Ok(pixels)
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Projected {
    pub screen: [i32; 2],
    pub view: [i32; 3],
    pub depth: i32,
}
pub fn project(
    vertices: &[[i32; 3]],
    angles: [i32; 3],
    offset: [i32; 3],
) -> Result<Vec<Projected>, String> {
    let (sy, cy) = sin_cos(angles[0]);
    let (sr, cr) = sin_cos(angles[1]);
    let (sp, cp) = sin_cos(angles[2]);
    let camera_dist = offset[1]
        .wrapping_mul(sp)
        .wrapping_add(offset[2].wrapping_mul(cp))
        >> 16;
    let mut out = Vec::new();
    for &[mut x, mut y, mut z] in vertices {
        if angles[1] != 0 {
            let t = y.wrapping_mul(sr).wrapping_add(x.wrapping_mul(cr)) >> 16;
            y = y.wrapping_mul(cr).wrapping_sub(x.wrapping_mul(sr)) >> 16;
            x = t;
        }
        if angles[0] != 0 {
            let t = z.wrapping_mul(sy).wrapping_add(x.wrapping_mul(cy)) >> 16;
            z = z.wrapping_mul(cy).wrapping_sub(x.wrapping_mul(sy)) >> 16;
            x = t;
        }
        x = x.wrapping_add(offset[0]);
        y = y.wrapping_add(offset[1]);
        z = z.wrapping_add(offset[2]);
        let t = y.wrapping_mul(cp).wrapping_sub(z.wrapping_mul(sp)) >> 16;
        z = y.wrapping_mul(sp).wrapping_add(z.wrapping_mul(cp)) >> 16;
        if z == 0 {
            return Err("Item camera division by zero".into());
        }
        out.push(Projected {
            screen: [
                16 + x.wrapping_shl(9).wrapping_div(z),
                16 + t.wrapping_shl(9).wrapping_div(z),
            ],
            view: [x, t, z],
            depth: z.wrapping_sub(camera_dist),
        });
    }
    Ok(out)
}
pub fn face_order(
    model: &Model,
    projection: &[Projected],
    kinds: &[u8],
    radius: i32,
) -> Vec<usize> {
    let mut sorted = Vec::new();
    for (i, face) in model.faces.iter().enumerate() {
        if kinds[i] == 2 {
            continue;
        }
        let [a, b, c] = face.map(|v| projection[v]);
        let area = a.screen[0]
            .wrapping_sub(b.screen[0])
            .wrapping_mul(c.screen[1].wrapping_sub(b.screen[1]))
            .wrapping_sub(
                a.screen[1]
                    .wrapping_sub(b.screen[1])
                    .wrapping_mul(c.screen[0].wrapping_sub(b.screen[0])),
            );
        if area > 0 {
            sorted.push((
                (a.depth.wrapping_add(b.depth).wrapping_add(c.depth)) / 3 + radius,
                i,
            ));
        }
    }
    sorted.sort_by_key(|&(depth, _)| std::cmp::Reverse(depth));
    if !model.has_priorities {
        return sorted.into_iter().map(|(_, i)| i).collect();
    }
    let mut groups: Vec<Vec<(i32, usize)>> = vec![Vec::new(); 12];
    for entry in sorted {
        let priority = model.priorities[entry.1] as usize;
        if priority < 12 {
            groups[priority].push(entry);
        }
    }
    let average = |a: usize, b: usize| {
        let count = groups[a].len() + groups[b].len();
        if count == 0 {
            0
        } else {
            groups[a]
                .iter()
                .chain(&groups[b])
                .map(|&(d, _)| d)
                .sum::<i32>()
                / count as i32
        }
    };
    let thresholds = [average(1, 2), average(3, 4), average(6, 8)];
    let high: Vec<_> = groups[10].iter().chain(&groups[11]).copied().collect();
    let mut next = 0;
    let mut out = Vec::new();
    for (priority, group) in groups.iter().take(10).enumerate() {
        if let Some(index) = match priority {
            0 => Some(0),
            3 => Some(1),
            5 => Some(2),
            _ => None,
        } {
            while next < high.len() && high[next].0 > thresholds[index] {
                out.push(high[next].1);
                next += 1;
            }
        }
        out.extend(group.iter().map(|&(_, i)| i));
    }
    out.extend(high[next..].iter().map(|&(_, i)| i));
    out
}
