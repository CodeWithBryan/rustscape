//! Raw dialogue/widget model assembly. Geometry and metadata remain cache inputs.
use super::item::{Catalog, ItemType};
use crate::{
    appearance::{self, IdentityKit},
    model::Model,
    pack::Pack,
};
use std::collections::BTreeMap;

/// ObjType.getModel's cached GL geometry. Normals/UVs are built before the
/// original GL resize, which deliberately leaves them unchanged.
pub fn item_widget(
    item: &ItemType,
    raw: &Model,
    palette: &[u16; 256],
    materials: &[Option<crate::texture::Material>],
) -> Model {
    let mut model = raw.clone();
    if let (Some(src), Some(dst)) = (&item.recol_s, &item.recol_d) {
        for (i, (&a, &b)) in src.iter().zip(dst).enumerate() {
            let color = item
                .recol_p
                .as_ref()
                .and_then(|p| p.get(i))
                .map_or(b as u16, |&p| palette[p as u8 as usize]);
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
    model.lighting = [
        item.ambient.wrapping_add(64),
        item.contrast.wrapping_add(768),
    ];
    model.sort_by_priority = true;
    model.surfaces = crate::surface::prepare(&model, materials);
    for v in &mut model.vertices {
        v[0] = v[0].wrapping_mul(item.resizeX) >> 7;
        v[1] = v[1].wrapping_mul(item.resizeY) >> 7;
        v[2] = v[2].wrapping_mul(item.resizeZ) >> 7;
    }
    model
}

/// GlModel.getMinY returns a short, including the original empty-model sentinel.
pub fn item_memory(model: &Model) -> i32 {
    -i32::from(model.vertices.iter().fold(32767, |y, v| y.min(v[1])) as i16) / 2
}

fn replacements(model: &mut Model, item: &ItemType) {
    if let (Some(src), Some(dst)) = (&item.recol_s, &item.recol_d) {
        for (&src, &dst) in src.iter().zip(dst) {
            model.recolor(&[[src as u16, dst as u16]]);
        }
    }
    if let (Some(src), Some(dst)) = (&item.retex_s, &item.retex_d) {
        for (&src, &dst) in src.iter().zip(dst) {
            for value in &mut model.textures {
                if *value as i16 == src as i16 {
                    *value = dst as i16 as i32;
                }
            }
        }
    }
}

/// ObjType.getHeadModel, including its gender-specific optional second part.
pub fn item_head(
    item: &ItemType,
    models: &BTreeMap<u32, Model>,
    female: bool,
) -> Result<Option<Model>, String> {
    let ids = if female {
        [item.womanhead, item.womanhead2]
    } else {
        [item.manhead, item.manhead2]
    };
    if ids[0] == -1 {
        return Ok(None);
    }
    let mut model = models
        .get(&(ids[0] as u32))
        .cloned()
        .ok_or_else(|| format!("Missing item head model {}", ids[0]))?;
    if ids[1] != -1 {
        let second = models
            .get(&(ids[1] as u32))
            .ok_or_else(|| format!("Missing item head model {}", ids[1]))?;
        model = Model::combine(&[model, second.clone()]);
    }
    replacements(&mut model, item);
    Ok(Some(model))
}

/// PlayerAppearance.getHeadModel's raw assembly after original appearance resolution.
pub fn player_head(
    pack: &Pack,
    models: &BTreeMap<u32, Model>,
    items: &mut Catalog,
    identikit: [i32; 12],
    colors: [i32; 5],
    female: bool,
) -> Result<Model, String> {
    let mut parts = Vec::new();
    for value in identikit {
        if value & 0x40000000 != 0 {
            let item = items.get(pack, value & 0x3fffffff)?;
            if let Some(model) = item_head(&item, models, female)? {
                parts.push(model);
            }
        } else if value & i32::MIN != 0 {
            let id = value & 0x3fffffff;
            let kit =
                IdentityKit::decode(pack.get(&format!("kit/{id}"))?).map_err(|e| e.to_string())?;
            parts.push(kit.head_model(models)?);
        }
    }
    let mut model = Model::combine(&parts);
    appearance::recolor(&mut model, colors);
    model.lighting = [64, 768];
    model.sort_by_priority = true;
    Ok(model)
}

/// Original partial-head assembly order is jaw, beard, then hair.
pub fn partial_head(
    pack: &Pack,
    models: &BTreeMap<u32, Model>,
    ids: [i32; 3],
    colors: [i32; 5],
) -> Result<Model, String> {
    let mut parts = Vec::new();
    for id in ids {
        let kit =
            IdentityKit::decode(pack.get(&format!("kit/{id}"))?).map_err(|e| e.to_string())?;
        parts.push(kit.head_model(models)?);
    }
    let mut model = Model::combine(&parts);
    appearance::recolor(&mut model, colors);
    model.lighting = [64, 768];
    model.sort_by_priority = true;
    Ok(model)
}

/// Original getAppearanceHash used by CS2setPlayerHead1202; not the body CRC.
pub fn appearance_hash(identikit: [i32; 12], colors: [i32; 5], npc_id: i32) -> i32 {
    if npc_id != -1 {
        return npc_id.wrapping_add(305419896);
    }
    identikit[8]
        .wrapping_shl(10)
        .wrapping_add(
            colors[0]
                .wrapping_shl(25)
                .wrapping_add(colors[4].wrapping_shl(20)),
        )
        .wrapping_add(identikit[0].wrapping_shl(15))
        .wrapping_add(identikit[11].wrapping_shl(5))
        .wrapping_add(identikit[1])
}
