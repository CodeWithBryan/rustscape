//! Player/NpcType appearance delegation. Movement BAS remains the player's BAS.
use crate::{model::Model, npc::Definition, walking::Movement};
use rs09_protocol::game::players::RawAppearance;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
pub struct Form {
    pub npc_id: i32,
    pub render_id: Option<u32>,
    pub size: i32,
    pub resize: [i32; 2],
    pub shadow: bool,
}
impl Default for Form {
    fn default() -> Self {
        Self {
            npc_id: -1,
            render_id: None,
            size: 1,
            resize: [128; 2],
            shadow: true,
        }
    }
}
impl Form {
    pub fn visible(self) -> bool {
        self.npc_id == -1 || self.render_id.is_some()
    }
    /// NpcType resizes after all animation. GL normals are not resized.
    pub fn resize_model(self, model: &mut Model) {
        if self.resize == [128; 2] {
            return;
        }
        for vertex in &mut model.vertices {
            for (value, scale) in
                vertex
                    .iter_mut()
                    .zip([self.resize[0], self.resize[1], self.resize[0]])
            {
                *value = value.wrapping_mul(scale) >> 7;
            }
        }
    }
}
fn definition(types: &BTreeMap<u32, Definition>, id: i32) -> Result<&Definition, String> {
    types
        .get(&(id as u32))
        .ok_or_else(|| format!("Player transform NPC{id}: definition unavailable"))
}
pub fn size(raw: &RawAppearance, types: &BTreeMap<u32, Definition>) -> Result<i32, String> {
    Ok(if raw.npc_id == -1 {
        raw.size()
    } else {
        definition(types, raw.npc_id)?.size
    })
}
pub fn resolve(
    raw: &RawAppearance,
    types: &BTreeMap<u32, Definition>,
    mut read: impl FnMut(bool, i32) -> Result<i32, String>,
) -> Result<Form, String> {
    let mut form = Form {
        npc_id: raw.npc_id,
        size: size(raw, types)?,
        ..Form::default()
    };
    if raw.npc_id == -1 {
        return Ok(form);
    }
    // Player.render uses the base definition's shadow flag and default player
    // shadow style, even when getBodyModel selects a differently styled child.
    form.shadow = definition(types, raw.npc_id)?.shadow;
    let mut id = raw.npc_id as u32;
    let mut seen = BTreeSet::new();
    loop {
        if !seen.insert(id) || seen.len() > 128 {
            return Err("Cyclic/deep player NPC morph".into());
        }
        let d = definition(types, id as i32)?;
        if d.children.is_none() {
            form.render_id = Some(id);
            form.resize = d.resize;
            return Ok(form);
        }
        let mut error = None;
        let next = d.select_morph(|bit, key| {
            read(bit, key).unwrap_or_else(|e| {
                error = Some(e);
                0
            })
        });
        if let Some(e) = error {
            return Err(e);
        }
        match next {
            Some(next) => id = next,
            None => return Ok(form),
        }
    }
}
/// Player.decodeAppearance's two position mutations, including repeated forms.
/// At the first mutation setSize has run but the old NPC appearance still exists.
pub fn appearance_position(
    motion: &mut Movement,
    previous: Option<&RawAppearance>,
    raw: &RawAppearance,
    types: &BTreeMap<u32, Definition>,
) -> Result<(), String> {
    let previous_size = previous.map_or(1, RawAppearance::size);
    let previous_npc = previous.map_or(-1, |p| p.npc_id);
    let intermediate_size = if previous_npc == -1 {
        raw.size()
    } else {
        definition(types, previous_npc)?.size
    };
    let delta = intermediate_size
        .wrapping_sub(previous_size)
        .wrapping_mul(64);
    for value in &mut motion.fine {
        *value = value.wrapping_add(delta);
    }
    if previous_npc != raw.npc_id {
        let size = size(raw, types)?;
        motion.fine = motion
            .head
            .map(|n| n.wrapping_mul(128).wrapping_add(size.wrapping_mul(64)));
    }
    Ok(())
}
