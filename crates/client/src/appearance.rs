//! Identity-kit body assembly and GAME0 appearance recoloring from PlayerAppearance.
use crate::{appearance_colors::*, model::Model, pack::Pack};
use rs09_protocol::{Error, Result, buffer::Reader};
use std::collections::BTreeMap;

/// Original GAME0 palette size used while resolving appearance update colors.
pub fn body_color_count(index: usize) -> usize {
    GAME0_DESTINATION_BODY_COLORS[index].len()
}
#[derive(Debug)]
pub struct IdentityKit {
    pub feature: usize,
    /// An absent opcode2 returns null; an explicit empty array builds an empty model.
    pub body_present: bool,
    pub models: Vec<u32>,
    pub heads: [i32; 5],
    pub recolor: Vec<[u16; 2]>,
    pub retexture: Vec<[u16; 2]>,
}
impl Default for IdentityKit {
    fn default() -> Self {
        Self {
            feature: 0,
            body_present: false,
            models: Vec::new(),
            heads: [-1; 5],
            recolor: Vec::new(),
            retexture: Vec::new(),
        }
    }
}
impl IdentityKit {
    pub fn decode(data: &[u8]) -> Result<Self> {
        let mut r = Reader::new(data);
        let mut kit = Self::default();
        loop {
            match r.g1()? {
                0 => return Ok(kit),
                1 => kit.feature = r.g1()? as usize,
                2 => {
                    let n = r.g1()?;
                    kit.body_present = true;
                    kit.models.clear();
                    for _ in 0..n {
                        kit.models.push(r.g2()? as u32);
                    }
                }
                3 => {}
                op @ (40 | 41) => {
                    let n = r.g1()?;
                    let pairs = if op == 40 {
                        &mut kit.recolor
                    } else {
                        &mut kit.retexture
                    };
                    for _ in 0..n {
                        pairs.push([r.g2()? as u16, r.g2()? as u16]);
                    }
                }
                op @ 60..=69 => {
                    let id = r.g2()?;
                    let slot = kit.heads.get_mut((op - 60) as usize).ok_or(Error::new(
                        r.position() - 3,
                        "identity kit head slot outside original array",
                    ))?;
                    *slot = id;
                }
                _ => return Err(Error::new(r.position() - 1, "unknown identity kit opcode")),
            }
        }
    }
}

impl IdentityKit {
    /// IdkType.getHeadModel always combines the selected head parts, even one.
    pub fn head_model(&self, models: &BTreeMap<u32, Model>) -> std::result::Result<Model, String> {
        let parts = self
            .heads
            .iter()
            .filter(|&&id| id != -1)
            .map(|&id| {
                models
                    .get(&(id as u32))
                    .cloned()
                    .ok_or_else(|| format!("Missing identity-kit head model {id}"))
            })
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let mut model = Model::combine(&parts);
        for &pair in &self.recolor {
            model.recolor(&[pair]);
        }
        for &[src, dst] in &self.retexture {
            for value in &mut model.textures {
                if *value as i16 == src as i16 {
                    *value = dst as i16 as i32;
                }
            }
        }
        Ok(model)
    }
}

pub fn recolor(model: &mut Model, colors: [i32; 5]) {
    for (i, n) in colors.into_iter().enumerate() {
        if let Some(&dst) = usize::try_from(n)
            .ok()
            .and_then(|n| GAME0_DESTINATION_BODY_COLORS[i].get(n))
        {
            model.recolor(&[[GAME0_SOURCE_BODY_COLORS[i], dst]]);
        }
        if let Some(&dst) = usize::try_from(n)
            .ok()
            .and_then(|n| GAME0_DESTINATION_SKIN_COLORS[i].get(n))
        {
            model.recolor(&[[GAME0_SOURCE_SKIN_COLORS[i], dst]]);
        }
    }
}
pub fn assemble(
    pack: &Pack,
    models: &BTreeMap<u32, Model>,
    kit_ids: &[u32],
    colors: [usize; 5],
) -> std::result::Result<Model, String> {
    let mut kits = Vec::new();
    for id in kit_ids {
        kits.push(IdentityKit::decode(pack.get(&format!("kit/{id}"))?).map_err(|e| e.to_string())?);
    }
    let slots = [8, 11, 4, 6, 9, 7, 10];
    kits.sort_by_key(|k| slots[k.feature % 7]);
    assemble_kits(kits, models, colors)
}
fn assemble_kits(
    kits: Vec<IdentityKit>,
    models: &BTreeMap<u32, Model>,
    colors: [usize; 5],
) -> std::result::Result<Model, String> {
    let mut parts = Vec::new();
    for kit in kits {
        let meshes: Vec<_> = kit
            .models
            .iter()
            .map(|id| {
                models
                    .get(id)
                    .cloned()
                    .ok_or_else(|| format!("Missing appearance model {id}"))
            })
            .collect::<std::result::Result<_, _>>()?;
        let mut model = if meshes.len() == 1 {
            meshes.into_iter().next().unwrap()
        } else {
            Model::combine(&meshes)
        };
        // RT4 applies each replacement in order; destinations may be sources of later replacements.
        for pair in kit.recolor {
            model.recolor(&[pair]);
        }
        for [src, dst] in kit.retexture {
            for texture in &mut model.textures {
                if *texture == i32::from(src) {
                    *texture = i32::from(dst);
                }
            }
        }
        parts.push(model);
    }
    let mut model = Model::combine(&parts);
    for (i, color) in colors.into_iter().enumerate() {
        if let Some(&dst) = GAME0_DESTINATION_BODY_COLORS[i].get(color) {
            model.recolor(&[[GAME0_SOURCE_BODY_COLORS[i], dst]]);
        }
        if let Some(&dst) = GAME0_DESTINATION_SKIN_COLORS[i].get(color) {
            model.recolor(&[[GAME0_SOURCE_SKIN_COLORS[i], dst]]);
        }
    }
    model.lighting = [64, 850];
    Ok(model)
}

/// Live identity-kit appearances. Equipment/NPC models require additional cache
/// assets; report that boundary instead of rendering an unrelated default body.
pub fn assemble_player(
    pack: &Pack,
    models: &BTreeMap<u32, Model>,
    appearance: &rs09_protocol::game::players::RawAppearance,
) -> std::result::Result<Model, String> {
    if appearance.npc_id != -1 {
        return Err(format!(
            "NPC appearance {} needs live cache models",
            appearance.npc_id
        ));
    }
    if appearance.slots.iter().any(|&n| n >= 32768) {
        return Err("Equipment appearance needs live cache wearable models".into());
    }
    let ids: Vec<u32> = appearance
        .slots
        .iter()
        .filter(|&&n| n >= 256)
        .map(|&n| (n - 256) as u32)
        .collect();
    let colors = std::array::from_fn(|i| {
        let n = appearance.colors[i] as usize;
        if n < GAME0_DESTINATION_BODY_COLORS[i].len() {
            n
        } else {
            0
        }
    });
    let kits = ids
        .iter()
        .map(|id| IdentityKit::decode(pack.get(&format!("kit/{id}"))?).map_err(|e| e.to_string()))
        .collect::<std::result::Result<_, String>>()?;
    assemble_kits(kits, models, colors)
}
