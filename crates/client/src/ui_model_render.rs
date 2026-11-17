//! Cached type6 model-widget presentation using the original camera/pose contracts.
use crate::display_host::{screen_height, screen_width};
use macroquad::prelude::*;
use rs09_client::{
    animation::{Base, Frame, Sequence, animate_hd},
    appearance::{self, IdentityKit},
    model::Model,
    pack::Pack,
    ui::{
        Component, model,
        model_animation::{self, Framesets},
        model_assets,
        state::{DrawNode, Key, State},
        vm::Vm,
    },
};
use rs09_protocol::game::players::{Appearance, AppearanceLookup, RawAppearance, SELF};
use std::collections::{BTreeMap, BTreeSet};

pub struct Context<'a> {
    pub world: &'a crate::Workbench,
    pub vm: &'a Vm,
    pub players: &'a BTreeMap<usize, RawAppearance>,
    pub self_index: usize,
    pub live: bool,
}
struct View {
    matrix: Mat4,
    viewport: [i32; 2],
}
impl macroquad::camera::Camera for View {
    fn matrix(&self) -> Mat4 {
        self.matrix
    }
    fn depth_enabled(&self) -> bool {
        true
    }
    fn render_pass(&self) -> Option<RenderPass> {
        None
    }
    fn viewport(&self) -> Option<(i32, i32, i32, i32)> {
        Some(crate::display_host::canvas_viewport(self.viewport))
    }
}
#[derive(Default)]
struct AnimationAssets {
    sequences: BTreeMap<i32, Sequence>,
    frames: BTreeMap<u32, Frame>,
    bases: BTreeMap<u16, Base>,
    groups: BTreeSet<i32>,
}
fn bytes<'a>(ui: &'a Pack, world: &'a Pack, name: &str) -> Result<&'a [u8], String> {
    ui.get(name).or_else(|_| world.get(name))
}
impl AnimationAssets {
    fn sequence(&mut self, ui: &Pack, world: &Pack, id: i32) -> Result<Sequence, String> {
        if let std::collections::btree_map::Entry::Vacant(entry) = self.sequences.entry(id) {
            let s = Sequence::decode(bytes(ui, world, &format!("seq/{id}"))?)
                .map_err(|e| e.to_string())?;
            entry.insert(s);
        }
        Ok(self.sequences[&id].clone())
    }
    fn group(&mut self, ui: &Pack, world: &Pack, id: i32) -> Result<(), String> {
        if self.groups.contains(&id) {
            return Ok(());
        }
        let prefix = format!("frame/{id}/");
        let names: BTreeSet<_> = ui
            .manifest
            .files
            .keys()
            .chain(world.manifest.files.keys())
            .filter(|n| n.starts_with(&prefix))
            .cloned()
            .collect();
        if names.is_empty() {
            return Err(format!("Missing UI animation frameset {id}"));
        }
        for name in names {
            let raw = bytes(ui, world, &name)?;
            let bid =
                u16::from_be_bytes(raw.get(..2).ok_or("UI frame header")?.try_into().unwrap());
            if let std::collections::btree_map::Entry::Vacant(entry) = self.bases.entry(bid) {
                entry.insert(
                    Base::decode(bid, bytes(ui, world, &format!("base/{bid}"))?)
                        .map_err(|e| e.to_string())?,
                );
            }
            let low: u32 = name[prefix.len()..].parse().map_err(|_| "UI frame index")?;
            self.frames.insert(
                (id as u32) << 16 | low,
                Frame::decode(raw, &self.bases[&bid]).map_err(|e| e.to_string())?,
            );
        }
        self.groups.insert(id);
        Ok(())
    }
    fn pose(
        &mut self,
        ui: &Pack,
        world: &Pack,
        base: &Model,
        s: &Sequence,
        c: &Component,
        entity: bool,
    ) -> Result<Model, String> {
        for id in s.frames.iter().chain(if entity {
            s.secondary_frames.iter()
        } else {
            [].iter()
        }) {
            if *id != 65535 {
                self.group(ui, world, (*id as i32) >> 16)?;
            }
        }
        let mut sequence = s.clone();
        if !entity {
            sequence.secondary_frames.clear();
        }
        let plan = model_animation::plan(
            &sequence,
            c.seq_frame,
            c.seq_next_frame,
            c.seq_cycle,
            true,
            self,
        )?;
        let mut result = base.clone();
        for step in plan.steps {
            let frame = self
                .frames
                .get(&((step.group as u32) << 16 | step.frame as u32))
                .ok_or("Missing current widget frame")?;
            let next = step.next_group.and_then(|g| {
                self.frames
                    .get(&((g as u32) << 16 | step.next_frame as u32))
            });
            result = animate_hd(
                &result,
                &self.bases[&frame.base],
                frame,
                next,
                step.delta,
                step.length,
                step.normals,
            );
        }
        Ok(result)
    }
}
impl Framesets for AnimationAssets {
    fn ready(&self, g: i32) -> bool {
        self.groups.contains(&g)
    }
    fn alpha(&self, g: i32, f: i32) -> bool {
        self.frames
            .get(&((g as u32) << 16 | f as u32))
            .is_some_and(|frame| {
                frame
                    .transforms
                    .iter()
                    .any(|t| self.bases[&frame.base].types[t.index] == 5)
            })
    }
    fn color(&self, g: i32, f: i32) -> bool {
        self.frames
            .get(&((g as u32) << 16 | f as u32))
            .is_some_and(|frame| {
                frame
                    .transforms
                    .iter()
                    .any(|t| self.bases[&frame.base].types[t.index] == 7)
            })
    }
}

struct Resolved {
    model: Model,
    entity: bool,
    scale: Option<[i32; 2]>,
}
pub struct Renderer {
    // Macroquad material samplers retain GPU IDs, so keep the texture alive.
    _placeholder: Texture2D,
    raw: BTreeMap<u32, Model>,
    cache: BTreeMap<Vec<i32>, Model>,
    animation: AnimationAssets,
    equipment: Option<Vec<i32>>,
    materials: [Material; 2],
    pub last: Vec<serde_json::Value>,
    pub draws: u64,
    clear_depth: bool,
    /// The script recolour palette the cached NPC and object models used.
    palette: rs09_client::recolor::Palette,
}
impl Renderer {
    /// Adopt the script recolour palette. CS2 5401 clears the original's
    /// object and NPC model caches, so a changed palette empties this one.
    pub fn set_palette(&mut self, palette: &rs09_client::recolor::Palette) {
        if self.palette.revision != palette.revision {
            self.palette = palette.clone();
            self.cache.clear();
        }
    }
    pub fn new(textures: &crate::render::Textures) -> Result<Self, String> {
        let materials = [
            crate::render::material(textures, true).map_err(|e| e.to_string())?,
            crate::render::material(textures, false).map_err(|e| e.to_string())?,
        ];
        let blank = Texture2D::from_rgba8(1, 1, &[0, 0, 0, 0]);
        for m in &materials {
            for name in ["ShadowMap", "LightGrid", "LightData"] {
                m.set_texture(name, blank.clone());
            }
            m.set_uniform("ShadowRegion", vec4(0., 0., 1., 1.));
            m.set_uniform("ShadowsEnabled", 0f32);
            m.set_uniform("LightRegion", Vec4::ZERO);
            m.set_uniform("AmbientLight", Vec3::splat((3f32 * 0.1 + 0.7) * 1.1523438));
            m.set_uniform("DiffuseLight0", Vec3::splat(0.69921875));
            m.set_uniform("DiffuseLight1", Vec3::splat(-0.69921875));
            m.set_uniform("LightDirection", vec3(-50., -60., -50.));
            m.set_uniform("CameraPosition", Vec3::ZERO);
            m.set_uniform("CameraForward", Vec3::Z);
            m.set_uniform("FogRange", vec2(1e6, 1e7));
            m.set_uniform("FogColor", Vec3::ZERO);
        }
        Ok(Self {
            _placeholder: blank,
            raw: BTreeMap::new(),
            cache: BTreeMap::new(),
            animation: AnimationAssets::default(),
            equipment: None,
            materials,
            last: Vec::new(),
            draws: 0,
            clear_depth: true,
            palette: Default::default(),
        })
    }
    /// `FogManager.init(Preferences.brightness)`, which the original calls
    /// before drawing an interface model.
    pub fn set_brightness(&self, brightness: i32) {
        for m in &self.materials {
            m.set_uniform(
                "AmbientLight",
                Vec3::splat((brightness as f32 * 0.1 + 0.7) * 1.1523438),
            );
            // GlRenderer.restoreLighting (Cs1ScriptRunner.java:956): lighting
            // is off without high detail lighting.
            m.set_uniform(
                "LightRegion",
                Vec4::from_array(rs09_client::sd_light::light_region([0.0; 4])),
            );
        }
    }
    pub fn begin(&mut self) {
        self.clear_depth = true;
        self.last.clear();
    }
    fn load(
        &mut self,
        ui: &Pack,
        world: &Pack,
        ids: impl IntoIterator<Item = u32>,
    ) -> Result<(), String> {
        for id in ids {
            if let std::collections::btree_map::Entry::Vacant(entry) = self.raw.entry(id) {
                let m = Model::decode(bytes(ui, world, &format!("model/{id}"))?)
                    .map_err(|e| e.to_string())?;
                entry.insert(m);
            }
        }
        Ok(())
    }
    fn resolved(&mut self, state: &mut State, raw: &RawAppearance) -> Result<Appearance, String> {
        if self.equipment.is_none() && raw.slots.iter().any(|&n| n >= 32768) {
            let mut ids: Vec<_> = state
                .pack
                .manifest
                .files
                .keys()
                .filter_map(|n| n.strip_prefix("item/")?.parse::<i32>().ok())
                .collect();
            ids.sort();
            let mut types = Vec::new();
            for id in ids {
                let t = state.items.get(&state.pack, id)?;
                types.push((id, t.manwear, t.womanwear));
            }
            self.equipment = Some(rs09_client::minimap::equipment_ids(types));
        }
        struct Lookup<'a> {
            ids: &'a [i32],
            types: &'a rs09_client::ui::item::Catalog,
        }
        impl AppearanceLookup for Lookup<'_> {
            fn equipment_id(&self, n: usize) -> rs09_protocol::Result<i32> {
                self.ids
                    .get(n)
                    .copied()
                    .ok_or(rs09_protocol::Error::new(0, "Widget equipment index"))
            }
            fn item_team(&self, id: i32) -> rs09_protocol::Result<i32> {
                Ok(self.types.types.get(&id).map_or(0, |i| i.team))
            }
            fn body_color_count(&self, i: usize) -> usize {
                appearance::body_color_count(i)
            }
        }
        raw.resolve(&Lookup {
            ids: self.equipment.as_deref().unwrap_or(&[]),
            types: &state.items,
        })
        .map_err(|e| e.to_string())
    }
    pub fn sync_self(&mut self, state: &mut State, raw: &RawAppearance) -> Result<i32, String> {
        let a = self.resolved(state, raw)?;
        Ok(model_assets::appearance_hash(
            a.identikit,
            a.colors,
            a.raw.npc_id,
        ))
    }
    pub fn player_team(&mut self, state: &mut State, raw: &RawAppearance) -> Result<i32, String> {
        Ok(self.resolved(state, raw)?.team)
    }
    pub fn player_form(
        &self,
        state: &State,
        world: &crate::Workbench,
        vm: &Vm,
        raw: &RawAppearance,
    ) -> Result<rs09_client::player_form::Form, String> {
        rs09_client::player_form::resolve(raw, &world.npc_definitions, |bit, key| {
            if bit {
                vm.varbit(state, key)
            } else {
                Ok(vm.varps.get(&key).copied().unwrap_or(0))
            }
        })
    }
    pub fn player_body(
        &mut self,
        state: &mut State,
        world: &crate::Workbench,
        raw: &RawAppearance,
        overrides: [i32; 2],
        form: rs09_client::player_form::Form,
    ) -> Result<Model, String> {
        if raw.npc_id != -1 {
            let id = form
                .render_id
                .ok_or("Hidden transformed player has no body")?;
            let key = vec![9, id as i32];
            if let Some(m) = self.cache.get(&key) {
                return Ok(m.clone());
            }
            let d = world
                .npc_definitions
                .get(&id)
                .ok_or("Missing player NPC metadata")?;
            self.load(
                &state.pack,
                &world.pack,
                d.models.iter().copied().filter(|&id| id != u32::MAX),
            )?;
            let transforms = if d.bas == -1 {
                [None; 12]
            } else {
                rs09_client::player_body::bas_transforms(bytes(
                    &state.pack,
                    &world.pack,
                    &format!("bas/{}", d.bas),
                )?)
                .map_err(|e| e.to_string())?
                .unwrap_or([None; 12])
            };
            let mut m = d.assemble_body(&self.raw, transforms, &self.palette.colors)?;
            m.draw_order = rs09_client::surface::face_order(&m, &world.textures.materials);
            m.surfaces = rs09_client::surface::prepare(&m, &world.textures.materials);
            if self.cache.len() >= 50 {
                self.cache.pop_first();
            }
            self.cache.insert(key, m.clone());
            return Ok(m);
        }
        let a = self.resolved(state, raw)?;
        let mut slots = a.identikit;
        let mut key = vec![8, raw.bas, i32::from(raw.female())];
        key.extend(slots);
        key.extend(a.colors);
        key.extend(overrides);
        if let Some(m) = self.cache.get(&key) {
            return Ok(m.clone());
        }
        for (slot, id) in [5, 3].into_iter().zip(overrides) {
            if id >= 0 {
                slots[slot] = if id == 65535 { 0 } else { id | 0x40000000 };
            }
        }
        for value in slots {
            if value & 0x40000000 != 0 {
                let t = state.items.get(&state.pack, value & 0x3fffffff)?;
                let ids = if raw.female() {
                    [t.womanwear, t.womanwear2, t.womanwear3]
                } else {
                    [t.manwear, t.manwear2, t.manwear3]
                };
                if ids[0] != -1 {
                    self.load(
                        &state.pack,
                        &world.pack,
                        ids.into_iter().filter(|&n| n != -1).map(|n| n as u32),
                    )?;
                }
            } else if value & i32::MIN != 0 {
                let k = IdentityKit::decode(bytes(
                    &state.pack,
                    &world.pack,
                    &format!("kit/{}", value & 0x3fffffff),
                )?)
                .map_err(|e| e.to_string())?;
                self.load(&state.pack, &world.pack, k.models)?;
            }
        }
        let transforms = if raw.bas == -1 {
            [None; 12]
        } else {
            rs09_client::player_body::bas_transforms(bytes(
                &state.pack,
                &world.pack,
                &format!("bas/{}", raw.bas),
            )?)
            .map_err(|e| e.to_string())?
            .unwrap_or([None; 12])
        };
        let mut m = rs09_client::player_body::human_body(
            &state.pack,
            &self.raw,
            &mut state.items,
            a.identikit,
            a.colors,
            raw.female(),
            overrides,
            transforms,
        )?;
        m.draw_order = rs09_client::surface::face_order(&m, &world.textures.materials);
        m.surfaces = rs09_client::surface::prepare(&m, &world.textures.materials);
        if self.cache.len() >= 50 {
            self.cache.pop_first();
        }
        self.cache.insert(key, m.clone());
        Ok(m)
    }
    fn preview(&self, state: &State, ctx: &Context) -> Result<([i32; 12], [i32; 5], bool), String> {
        let scene: serde_json::Value =
            serde_json::from_slice(ctx.world.pack.get("scene.json")?).map_err(|e| e.to_string())?;
        let mut slots = [0; 12];
        for id in scene["avatar_kits"]
            .as_array()
            .ok_or("Missing explicit preview kits")?
        {
            let id = id.as_i64().ok_or("Preview kit")? as i32;
            let kit =
                IdentityKit::decode(bytes(&state.pack, &ctx.world.pack, &format!("kit/{id}"))?)
                    .map_err(|e| e.to_string())?;
            slots[[8, 11, 4, 6, 9, 7, 10][kit.feature % 7]] = id | i32::MIN;
        }
        Ok((slots, [0; 5], false))
    }
    fn npc(&self, state: &State, ctx: &Context, mut id: u32) -> Result<Option<u32>, String> {
        let mut seen = BTreeSet::new();
        loop {
            if !seen.insert(id) {
                return Err("Cyclic UI NPC morph".into());
            }
            let d = ctx
                .world
                .npc_definitions
                .get(&id)
                .ok_or("Missing UI NPC metadata")?;
            if d.children.is_none() {
                return Ok(Some(id));
            }
            let mut error = None;
            let next = d.select_morph(|varbit, n| {
                if varbit {
                    ctx.vm.varbit(state, n).unwrap_or_else(|e| {
                        error = Some(e);
                        0
                    })
                } else {
                    ctx.vm.varps.get(&n).copied().unwrap_or(0)
                }
            });
            if let Some(e) = error {
                return Err(e);
            }
            match next {
                Some(n) => id = n,
                None => return Ok(None),
            }
        }
    }
    fn head_parts(
        &mut self,
        state: &mut State,
        ctx: &Context,
        slots: [i32; 12],
        female: bool,
    ) -> Result<(), String> {
        for value in slots {
            if value & 0x40000000 != 0 {
                let item = state.items.get(&state.pack, value & 0x3fffffff)?;
                let ids = if female {
                    [item.womanhead, item.womanhead2]
                } else {
                    [item.manhead, item.manhead2]
                };
                self.load(
                    &state.pack,
                    &ctx.world.pack,
                    ids.into_iter().filter(|&n| n != -1).map(|n| n as u32),
                )?;
            } else if value & i32::MIN != 0 {
                let id = value & 0x3fffffff;
                let kit =
                    IdentityKit::decode(bytes(&state.pack, &ctx.world.pack, &format!("kit/{id}"))?)
                        .map_err(|e| e.to_string())?;
                self.load(
                    &state.pack,
                    &ctx.world.pack,
                    kit.heads.into_iter().filter(|&n| n != -1).map(|n| n as u32),
                )?;
            }
        }
        Ok(())
    }
    /// ObjStack.render and object widgets use the same ObjType.getModel path.
    pub fn item_model(
        &mut self,
        state: &mut State,
        world: &crate::Workbench,
        id: i32,
        count: i32,
    ) -> Result<Model, String> {
        let item = state.items.get(&state.pack, id)?;
        let item = state.items.get(&state.pack, item.count_variant(count))?;
        let key = vec![4, item.id];
        if let Some(m) = self.cache.get(&key) {
            return Ok(m.clone());
        }
        self.load(&state.pack, &world.pack, [item.model as u32])?;
        let m = model_assets::item_widget(
            &item,
            &self.raw[&(item.model as u32)],
            &self.palette.colors,
            &world.textures.materials,
        );
        if self.cache.len() >= 50 {
            self.cache.pop_first();
        }
        self.cache.insert(key, m.clone());
        if self.raw.len() > 512 {
            self.raw.clear();
        }
        Ok(m)
    }
    fn base(
        &mut self,
        state: &mut State,
        ctx: &Context,
        c: &Component,
    ) -> Result<Option<Resolved>, String> {
        if c.obj_id != -1 || c.model_type == 4 {
            let (id, count) = if c.obj_id != -1 {
                (c.obj_id, c.obj_count)
            } else {
                (c.model_id, 10)
            };
            let model = self.item_model(state, ctx.world, id, count)?;
            return Ok(Some(Resolved {
                model,
                entity: true,
                scale: None,
            }));
        }
        let mut key = vec![c.model_type, c.model_id];
        let mut scale = None;
        let mut entity = true;
        let mut model = match c.model_type {
            0 => return Ok(None),
            1 => {
                if c.model_id == -1 {
                    return Ok(None);
                }
                if let Some(m) = self.cache.get(&key) {
                    return Ok(Some(Resolved {
                        model: m.clone(),
                        entity: true,
                        scale: None,
                    }));
                }
                self.load(&state.pack, &ctx.world.pack, [c.model_id as u32])?;
                let mut m = self.raw[&(c.model_id as u32)].clone();
                m.lighting = [64, 768];
                m.sort_by_priority = true;
                m
            }
            2 | 6 => {
                if c.model_id < 0 {
                    return Ok(None);
                }
                let Some(id) = self.npc(state, ctx, c.model_id as u32)? else {
                    return Ok(None);
                };
                key = vec![c.model_type, id as i32];
                if c.model_type == 6 {
                    entity = false;
                    let body = ctx
                        .world
                        .npc_types
                        .get(&id)
                        .ok_or("NPC body widget assets not prepared")?;
                    scale = Some(body.definition.resize);
                    body.model.clone()
                } else {
                    if let Some(m) = self.cache.get(&key) {
                        return Ok(Some(Resolved {
                            model: m.clone(),
                            entity: true,
                            scale: None,
                        }));
                    }
                    let d = &ctx.world.npc_definitions[&id];
                    self.load(
                        &state.pack,
                        &ctx.world.pack,
                        d.head_models.as_deref().unwrap_or(&[]).iter().copied(),
                    )?;
                    let Some(m) = d.assemble_head(&self.raw, &self.palette.colors)? else {
                        return Ok(None);
                    };
                    m
                }
            }
            3 | 7 => {
                let (slots, colors, female) = if let Some(raw) = ctx.players.get(&SELF) {
                    let a = self.resolved(state, raw)?;
                    if a.raw.npc_id != -1 {
                        let mut other = c.clone();
                        other.model_type = 2;
                        other.model_id = a.raw.npc_id;
                        return self.base(state, ctx, &other);
                    }
                    (a.identikit, a.colors, a.raw.female())
                } else if !ctx.live {
                    self.preview(state, ctx)?
                } else {
                    return Ok(None);
                };
                if c.model_type == 7 {
                    key.extend([c.model_frame_cycle]);
                } else {
                    key.extend(slots);
                    key.extend(colors);
                    key.push(i32::from(female));
                }
                if let Some(m) = self.cache.get(&key) {
                    return Ok(Some(Resolved {
                        model: m.clone(),
                        entity: true,
                        scale: None,
                    }));
                }
                if c.model_type == 7 {
                    let ids = [
                        ((c.model_id as u32) >> 16) as i32,
                        c.model_id & 65535,
                        c.model_frame_cycle,
                    ];
                    let mut parts = Vec::new();
                    for id in ids {
                        let kit = IdentityKit::decode(bytes(
                            &state.pack,
                            &ctx.world.pack,
                            &format!("kit/{id}"),
                        )?)
                        .map_err(|e| e.to_string())?;
                        self.load(
                            &state.pack,
                            &ctx.world.pack,
                            kit.heads.into_iter().filter(|&n| n != -1).map(|n| n as u32),
                        )?;
                        parts.push(kit.head_model(&self.raw)?);
                    }
                    let mut m = Model::combine(&parts);
                    appearance::recolor(&mut m, colors);
                    m.lighting = [64, 768];
                    m.sort_by_priority = true;
                    m
                } else {
                    self.head_parts(state, ctx, slots, female)?;
                    model_assets::player_head(
                        &state.pack,
                        &self.raw,
                        &mut state.items,
                        slots,
                        colors,
                        female,
                    )?
                }
            }
            5 => {
                entity = false;
                if c.model_id == -1 {
                    if ctx.live {
                        return Err("Character-design default appearance service pending".into());
                    }
                    ctx.world
                        .avatar
                        .first()
                        .cloned()
                        .ok_or("No explicit preview body")?
                } else {
                    let mut id = (c.model_id & 2047) as usize;
                    if id == ctx.self_index {
                        id = SELF;
                    }
                    let Some(raw) = ctx.players.get(&id) else {
                        return Ok(None);
                    };
                    if (raw.name37 as i32).wrapping_shl(11) != (c.model_id & (0xfffff800u32 as i32))
                    {
                        return Ok(None);
                    }
                    let overrides = if c.model_seq_id == -1 {
                        [-1; 2]
                    } else {
                        let s = self.animation.sequence(
                            &state.pack,
                            &ctx.world.pack,
                            c.model_seq_id,
                        )?;
                        [s.mainhand, s.offhand]
                    };
                    let form = self.player_form(state, ctx.world, ctx.vm, raw)?;
                    if !form.visible() {
                        return Ok(None);
                    }
                    scale = (raw.npc_id != -1).then_some(form.resize);
                    self.player_body(state, ctx.world, raw, overrides, form)?
                }
            }
            _ => return Ok(None),
        };
        if model.surfaces.is_empty() {
            model.surfaces = rs09_client::surface::prepare(&model, &ctx.world.textures.materials);
        }
        if c.model_type != 5 && c.model_type != 6 {
            if self.cache.len() >= 50 {
                self.cache.pop_first();
            }
            self.cache.insert(key, model.clone());
        }
        if self.raw.len() > 512 {
            self.raw.clear();
        }
        Ok(Some(Resolved {
            model,
            entity,
            scale,
        }))
    }
    pub fn draw(
        &mut self,
        state: &mut State,
        node: &DrawNode,
        ctx: &Context,
    ) -> Result<(), String> {
        let original = state.get(node.key)?.clone();
        let active = rs09_client::ui::cs1::is_true(state, ctx.vm, &original)?;
        let Some(c) = model::select(&original, active) else {
            return Ok(());
        };
        let Some(Resolved {
            mut model,
            entity,
            scale,
        }) = self.base(state, ctx, &c)?
        else {
            return Ok(());
        };
        let player_body = c.obj_id == -1 && c.model_type == 5;
        if c.model_seq_id != -1 && !(player_body && c.model_id == -1) {
            let seq = self
                .animation
                .sequence(&state.pack, &ctx.world.pack, c.model_seq_id)?;
            let mut clock = c.clone();
            if player_body {
                // The original UI body call passes no tween and a zero delay
                // clock, while its frame number still advances normally.
                clock.seq_next_frame = -1;
                clock.seq_cycle = 0;
            }
            model =
                self.animation
                    .pose(&state.pack, &ctx.world.pack, &model, &seq, &clock, entity)?;
        }
        if let Some([x, y]) = scale {
            for v in &mut model.vertices {
                v[0] = v[0].wrapping_mul(x) >> 7;
                v[1] = v[1].wrapping_mul(y) >> 7;
                v[2] = v[2].wrapping_mul(x) >> 7;
            }
        }
        let size = [screen_width() as i32, screen_height() as i32];
        let memory = if c.obj_id != -1 {
            model_assets::item_memory(&model)
        } else {
            0
        };
        let plan = model::Camera::new(&c, [node.rect.x, node.rect.y], size, memory)?;
        let mut mv = Mat4::IDENTITY;
        let mut normal = Mat4::IDENTITY;
        for (i, op) in plan.operations.iter().enumerate() {
            if op.len() == 3 {
                mv *= Mat4::from_translation(vec3(op[0], op[1], op[2]));
            } else {
                let r = Mat4::from_axis_angle(vec3(op[1], op[2], op[3]), op[0].to_radians());
                mv *= r;
                if i > 0 {
                    normal *= r;
                }
            }
        }
        if self.clear_depth {
            unsafe {
                let mut gl = get_internal_gl();
                gl.flush();
                gl.quad_context
                    .begin_default_pass(macroquad::miniquad::PassAction::Clear {
                        color: None,
                        depth: Some(1.),
                        stencil: None,
                    });
                gl.quad_context.end_render_pass();
            }
            self.clear_depth = false;
        }
        set_camera(&View {
            matrix: Mat4::from_cols_array(&plan.projection) * mv,
            viewport: size,
        });
        crate::display_host::scissor(Some((
            node.paint_clip.x,
            node.paint_clip.y,
            node.paint_clip.w,
            node.paint_clip.h,
        )));
        let material = &self.materials[usize::from(c.model_transparent)];
        material.set_uniform("TextureTime", ctx.vm.cycle as f32);
        let mut mesh = ctx.world.textures.mesh();
        // FogManager.init (Cs1ScriptRunner.java:959) puts the light back at
        // its default before the model is drawn, which is when a model not
        // yet drawn in the scene has its colours packed.
        rs09_client::sd_light::with_sun(rs09_client::sd_light::DEFAULT_SUN, || {
            crate::render::model(
                &mut mesh,
                &model,
                &ctx.world.textures,
                |v| vec3(v[0] as f32, v[1] as f32, v[2] as f32),
                |n| normal.transform_vector3(vec3(n[0] as f32, n[1] as f32, n[2] as f32)),
            )
        });
        gl_use_material(material);
        draw_mesh(&mesh);
        gl_use_default_material();
        crate::display_host::camera();
        crate::display_host::scissor(Some((
            node.paint_clip.x,
            node.paint_clip.y,
            node.paint_clip.w,
            node.paint_clip.h,
        )));
        self.draws += 1;
        let checksum = model
            .vertices
            .iter()
            .flatten()
            .fold(0xcbf29ce484222325u64, |hash, &n| {
                (hash ^ (n as u32 as u64)).wrapping_mul(0x100000001b3)
            });
        self.last.push(serde_json::json!({"component":node.key.id,"child":node.key.child,"type":c.model_type,"id":c.model_id,"sequence":c.model_seq_id,"frame":c.seq_frame,"next":c.seq_next_frame,"cycle":c.seq_cycle,"item":c.obj_id,"count":c.obj_count,"memory":memory,"vertex_checksum":format!("{checksum:016x}"),"vertices":model.vertices.len(),"faces":model.faces.len()}));
        Ok(())
    }
    pub fn advance(
        &mut self,
        state: &mut State,
        world: &Pack,
        vm: &Vm,
        delta: i32,
    ) -> Result<(), String> {
        fn group(
            state: &mut State,
            g: i32,
            depth: usize,
            out: &mut Vec<Key>,
        ) -> Result<(), String> {
            state.load_group(g)?;
            let keys: Vec<_> = state
                .components
                .iter()
                .filter(|(k, c)| k.child == -1 && k.id >> 16 == g && c.overlayer == -1)
                .map(|(&k, _)| k)
                .collect();
            for k in keys {
                node(state, k, depth, out)?;
            }
            Ok(())
        }
        fn node(state: &mut State, k: Key, depth: usize, out: &mut Vec<Key>) -> Result<(), String> {
            if depth > 100 {
                return Err("Cyclic UI animation hierarchy".into());
            }
            let c = state.get(k)?.clone();
            if c.if3 && c.hidden {
                return Ok(());
            }
            if c.kind == 0 {
                if !c.if3 && c.hidden {
                    return Ok(());
                }
                let static_keys: Vec<_> = state
                    .components
                    .iter()
                    .filter(|(q, d)| {
                        q.child == -1 && q.id >> 16 == k.id >> 16 && d.overlayer == c.id
                    })
                    .map(|(&q, _)| q)
                    .collect();
                for child in static_keys {
                    node(state, child, depth + 1, out)?;
                }
                let created: Vec<_> = state
                    .components
                    .keys()
                    .filter(|q| q.id == k.id && q.child >= 0)
                    .copied()
                    .collect();
                for child in created {
                    node(state, child, depth + 1, out)?;
                }
                if let Some(&(g, _)) = state.subinterfaces.get(&k.id) {
                    group(state, g, depth + 1, out)?;
                }
            }
            if c.kind == 6 {
                out.push(k);
            }
            Ok(())
        }
        if state.root < 0 {
            return Ok(());
        }
        let mut keys = Vec::new();
        group(state, state.root, 0, &mut keys)?;
        for key in keys {
            let c = state.get(key)?.clone();
            let active = rs09_client::ui::cs1::is_true(state, vm, &c)?;
            let id = if active {
                c.active_model_seq_id
            } else {
                c.model_seq_id
            };
            let seq = if id != -1 {
                Some(self.animation.sequence(&state.pack, world, id)?)
            } else {
                None
            };
            model::advance(state.get_mut(key)?, active, seq.as_ref(), delta)?;
        }
        Ok(())
    }
}
