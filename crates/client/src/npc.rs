//! Unmorphed, single-sequence NPC bodies from the pinned NpcType renderer.
//! Per-part BAS transforms precede animation; slot animations remain an
//! explicit boundary in the normal NPC presentation path. Palette recolours
//! (`recol_p`) take the caller's script palette (`crate::recolor`).
use crate::{
    animation::{Base, Frame, Sequence, animate_hd},
    model::Model,
    walking::PlayerBas,
};
use rs09_protocol::{Result, buffer::Reader};
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub struct Definition {
    pub name: String,
    pub models: Vec<u32>,
    pub head_models: Option<Vec<u32>>,
    pub size: i32,
    pub bas: i32,
    pub resize: [i32; 2],
    pub lighting: [i32; 2],
    pub recolor: Vec<[u16; 2]>,
    pub retexture: Vec<[u16; 2]>,
    pub offsets: BTreeMap<usize, [i32; 3]>,
    pub turn_speed: i32,
    pub rotates: bool,
    pub shadow: bool,
    pub shadow_colors: [u16; 2],
    pub shadow_alpha: [u8; 2],
    pub options: [Option<String>; 5],
    pub combat_level: i32,
    pub interactive: bool,
    pub minimap_display: bool,
    pub minimap_marker: i32,
    pub cursors: [i32; 4],
    pub attack_cursor: i32,
    /// Morph/palette bodies need additional client state before assembly.
    pub children: Option<Vec<i32>>,
    pub morph_varbit: i32,
    pub morph_varp: i32,
    pub palette: Vec<u8>,
    pub sounds: [i32; 4],
    pub sound_radius: i32,
    pub headicon: i32,
    pub hit_bar_id: i32,
    pub icon_height: i32,
    pub params: BTreeMap<i32, crate::ui::HookArg>,
    /// Opcode 99, `toprenderpriority`: inserted into the scene before players.
    pub top_render_priority: bool,
    /// Opcode 119, `loginscreenproperties`: bit 0 places the NPC in the
    /// title screen's scene, bit 1 lets it wander there.
    pub login_screen: i32,
    /// Opcode 125, `spawndirection`: the way a title-screen NPC faces.
    pub spawn_direction: i32,
}
impl Definition {
    pub fn decode(data: &[u8]) -> Result<Self> {
        fn string(r: &mut Reader<'_>) -> Result<String> {
            let mut out = String::new();
            loop {
                let c = r.g1()?;
                if c == 0 {
                    return Ok(out);
                }
                out.push(char::from(c as u8));
            }
        }
        let mut r = Reader::new(data);
        let mut d = Self {
            name: "null".into(),
            models: Vec::new(),
            head_models: None,
            size: 1,
            bas: -1,
            resize: [128; 2],
            lighting: [64, 850],
            recolor: Vec::new(),
            retexture: Vec::new(),
            offsets: BTreeMap::new(),
            turn_speed: 32,
            rotates: true,
            shadow: true,
            shadow_colors: [0; 2],
            shadow_alpha: [160, 240],
            options: Default::default(),
            combat_level: -1,
            interactive: true,
            minimap_display: true,
            minimap_marker: -1,
            cursors: [-1; 4],
            attack_cursor: -1,
            children: None,
            morph_varbit: -1,
            morph_varp: -1,
            palette: Vec::new(),
            sounds: [-1; 4],
            sound_radius: 0,
            headicon: -1,
            hit_bar_id: -1,
            icon_height: -1,
            params: BTreeMap::new(),
            top_render_priority: false,
            login_screen: 0,
            spawn_direction: 7,
        };
        loop {
            match r.g1()? {
                0 => return Ok(d),
                1 => {
                    for _ in 0..r.g1()? {
                        let id = r.g2()?;
                        d.models
                            .push(if id == 65535 { u32::MAX } else { id as u32 });
                    }
                }
                2 => d.name = string(&mut r)?,
                12 => d.size = r.g1()?,
                op @ 30..=34 => {
                    let option = string(&mut r)?;
                    d.options[op as usize - 30] = if option.eq_ignore_ascii_case("Hidden") {
                        None
                    } else {
                        Some(option)
                    };
                }
                op @ (40 | 41) => {
                    let pairs = if op == 40 {
                        &mut d.recolor
                    } else {
                        &mut d.retexture
                    };
                    for _ in 0..r.g1()? {
                        pairs.push([r.g2()? as u16, r.g2()? as u16]);
                    }
                }
                60 => {
                    let mut heads = Vec::new();
                    for _ in 0..r.g1()? {
                        heads.push(r.g2()? as u32);
                    }
                    d.head_models = Some(heads);
                }
                42 => {
                    let count = r.g1()? as usize;
                    d.palette = r.take(count)?.to_vec();
                }
                op @ (106 | 118) => {
                    d.morph_varbit = r.g2()?;
                    d.morph_varp = r.g2()?;
                    if d.morph_varbit == 65535 {
                        d.morph_varbit = -1;
                    }
                    if d.morph_varp == 65535 {
                        d.morph_varp = -1;
                    }
                    let fallback = if op == 118 { r.g2()? } else { 65535 };
                    let count = r.g1()?;
                    let mut children = Vec::new();
                    for _ in 0..=count {
                        let id = r.g2()?;
                        children.push(if id == 65535 { -1 } else { id });
                    }
                    children.push(if fallback == 65535 { -1 } else { fallback });
                    d.children = Some(children);
                }
                93 => d.minimap_display = false,
                99 => d.top_render_priority = true,
                107 => d.interactive = false,
                95 => d.combat_level = r.g2()?,
                137 => d.attack_cursor = r.g2()?,
                102 => d.headicon = r.g2()?,
                122 => d.hit_bar_id = r.g2()?,
                123 => d.icon_height = r.g2()?,
                126 => d.minimap_marker = r.g2()?,
                97 => d.resize[0] = r.g2()?,
                98 => d.resize[1] = r.g2()?,
                100 => d.lighting[0] = 64 + r.g1b()?,
                101 => d.lighting[1] = 850 + r.g1b()? * 5,
                103 => d.turn_speed = r.g2()?,
                109 => d.rotates = false,
                111 => d.shadow = false,
                113 => d.shadow_colors = [r.g2()? as u16, r.g2()? as u16],
                114 => d.shadow_alpha = [r.g1b()? as u8, r.g1b()? as u8],
                115 => {
                    r.take(2)?;
                }
                119 => d.login_screen = r.g1b()?,
                125 => d.spawn_direction = r.g1b()?,
                128 => {
                    r.g1()?;
                }
                121 => {
                    for _ in 0..r.g1()? {
                        let index = r.g1()? as usize;
                        d.offsets.insert(index, [r.g1b()?, r.g1b()?, r.g1b()?]);
                    }
                }
                127 => d.bas = r.g2()?,
                134 => {
                    for id in &mut d.sounds {
                        let value = r.g2()?;
                        *id = if value == 65535 { -1 } else { value };
                    }
                    d.sound_radius = r.g1()?;
                }
                op @ (135 | 136) => {
                    let i = if op == 135 { 0 } else { 2 };
                    d.cursors[i] = r.g1()?;
                    d.cursors[i + 1] = r.g2()?;
                }
                249 => d.params.extend(crate::ui::targeting::read_params(&mut r)?),
                // NpcType.decode ignores unknown opcode bytes, including cache138.
                _ => {}
            }
        }
    }
    pub fn assemble(&self, models: &BTreeMap<u32, Model>) -> std::result::Result<Model, String> {
        self.assemble_with_bas(models, &PlayerBas::default())
    }
    /// Assemble the selected NPC using its own BAS, before animation and resize,
    /// with the script recolour palette as it is before any script sets it
    /// (all zero).
    pub fn assemble_with_bas(
        &self,
        models: &BTreeMap<u32, Model>,
        bas: &PlayerBas,
    ) -> std::result::Result<Model, String> {
        self.assemble_with_palette(models, bas, &[0; 256])
    }
    /// [`Self::assemble_with_bas`] with the current script recolour palette
    /// (`client.npcRecolorPalette`, written by CS2 5401).
    pub fn assemble_with_palette(
        &self,
        models: &BTreeMap<u32, Model>,
        bas: &PlayerBas,
        palette: &[u16; 256],
    ) -> std::result::Result<Model, String> {
        if self.children.is_some() {
            return Err("NPC morph body requires the selected child type".into());
        }
        self.assemble_body(models, bas.body_transforms.unwrap_or([None; 12]), palette)
    }
    /// NpcType body assembly after morph selection, including per-part BAS
    /// transforms and the current client recolour palette.
    pub fn assemble_body(
        &self,
        models: &BTreeMap<u32, Model>,
        transforms: crate::player_body::Transforms,
        palette: &[u16; 256],
    ) -> std::result::Result<Model, String> {
        if self.children.is_some() {
            return Err("Unresolved NPC body morph".into());
        }
        let mut parts = Vec::new();
        for (index, id) in self.models.iter().enumerate() {
            if *id == u32::MAX {
                parts.push(None);
                continue;
            }
            let mut model = models
                .get(id)
                .cloned()
                .ok_or_else(|| format!("Missing NPC model {id}"))?;
            if let Some(offset) = self.offsets.get(&index) {
                for v in &mut model.vertices {
                    for axis in 0..3 {
                        v[axis] = v[axis].wrapping_add(offset[axis]);
                    }
                }
            }
            if let Some(transform) = transforms.get(index).copied().flatten() {
                crate::player_body::rotate_translate(&mut model, transform)?;
            }
            parts.push(Some(model));
        }
        let mut model = if self.models.len() == 1 {
            parts[0].clone().ok_or("Missing sole NPC body model")?
        } else {
            Model::combine_slots(&parts.iter().map(Option::as_ref).collect::<Vec<_>>())
        };
        model.part_matrices = parts
            .iter()
            .enumerate()
            .map(|(i, part)| {
                if part.is_some() {
                    transforms
                        .get(i)
                        .copied()
                        .flatten()
                        .map(crate::animation_parts::basis)
                } else {
                    None
                }
            })
            .collect();
        for (i, &[src, dst]) in self.recolor.iter().enumerate() {
            model.recolor(&[[
                src,
                self.palette.get(i).map_or(dst, |&id| palette[id as usize]),
            ]]);
        }
        for &[src, dst] in &self.retexture {
            for t in &mut model.textures {
                if *t as i16 == src as i16 {
                    *t = dst as i16 as i32;
                }
            }
        }
        model.lighting = self.lighting;
        model.sort_by_priority = true;
        Ok(model)
    }
    /// NpcType.getHeadModel after the caller resolves the original morph chain.
    /// Heads use64/768 lighting and omit body offsets/resize/BAS transforms.
    pub fn assemble_head(
        &self,
        models: &BTreeMap<u32, Model>,
        palette: &[u16; 256],
    ) -> std::result::Result<Option<Model>, String> {
        if self.children.is_some() {
            return Err("Unresolved NPC head morph".into());
        }
        let Some(ids) = &self.head_models else {
            return Ok(None);
        };
        let mut parts = ids
            .iter()
            .map(|id| {
                models
                    .get(id)
                    .cloned()
                    .ok_or_else(|| format!("Missing NPC head model {id}"))
            })
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let mut model = if parts.len() == 1 {
            parts.pop().unwrap()
        } else {
            Model::combine(&parts)
        };
        for (i, &[src, dst]) in self.recolor.iter().enumerate() {
            model.recolor(&[[
                src,
                self.palette.get(i).map_or(dst, |&id| palette[id as usize]),
            ]]);
        }
        for &[src, dst] in &self.retexture {
            for value in &mut model.textures {
                if *value as i16 == src as i16 {
                    *value = dst as i16 as i32;
                }
            }
        }
        model.lighting = [64, 768];
        model.sort_by_priority = true;
        Ok(Some(model))
    }
    /// NpcType applies GL animation before resize; resizing leaves normals unchanged.
    pub fn pose(
        &self,
        model: &Model,
        seq: &Sequence,
        base: &Base,
        frame: &Frame,
        next: Option<&Frame>,
        (progress, length): (i32, i32),
    ) -> Model {
        let mut out = animate_hd(
            model,
            base,
            frame,
            next,
            progress,
            length,
            seq.transform_normals,
        );
        let scale = [self.resize[0], self.resize[1], self.resize[0]];
        for v in &mut out.vertices {
            for axis in 0..3 {
                v[axis] = v[axis].wrapping_mul(scale[axis]) >> 7;
            }
        }
        out
    }

    /// NpcType.getMultiNpc; callers distinguish unmorphed types via `children`.
    /// The bool selects a varbit service (true) or active varp (false).
    pub fn select_morph(&self, mut read: impl FnMut(bool, i32) -> i32) -> Option<u32> {
        let ids = self.children.as_ref()?;
        let value = if self.morph_varbit != -1 {
            read(true, self.morph_varbit)
        } else if self.morph_varp != -1 {
            read(false, self.morph_varp)
        } else {
            -1
        };
        let selected = usize::try_from(value)
            .ok()
            .filter(|&i| i < ids.len().saturating_sub(1))
            .and_then(|i| (ids[i] != -1).then_some(ids[i]))
            .or_else(|| ids.last().copied())?;
        (selected != -1).then_some(selected as u32)
    }
    pub fn morph_valid(&self, read: impl FnMut(bool, i32) -> i32) -> bool {
        self.children.is_none() || self.select_morph(read).is_some()
    }
}

#[derive(Clone, Debug)]
pub struct Bas {
    pub movement: PlayerBas,
    pub terrain: [i32; 2],
}
impl Bas {
    pub fn decode(data: &[u8]) -> Result<Self> {
        let movement = PlayerBas::decode(data)?;
        Ok(Self {
            terrain: movement.terrain,
            movement,
        })
    }
}
