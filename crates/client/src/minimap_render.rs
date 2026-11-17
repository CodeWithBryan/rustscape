//! Pinned minimap assets and scene raster presented by the common Windows/WASM GPU path.
use macroquad::prelude::*;
use rs09_client::{
    floor,
    interaction::LocationType,
    minimap::{self, Indexed, MapScene, View},
    pack::Pack,
    ui::{
        Component,
        sprite::Sheet,
        state::{DrawNode, State},
        vm::Vm,
    },
    world,
};
use rs09_game_core::random::JavaRandom;
use std::collections::BTreeMap;

struct Sprite {
    texture: Texture2D,
    size: [i32; 2],
    inner: [i32; 2],
    offset: [i32; 2],
    power: [i32; 2],
}
impl Sprite {
    fn load(
        sheet: &Sheet,
        frame: usize,
        trim: bool,
        alpha: bool,
        adjust: Option<[i32; 3]>,
    ) -> Result<Self, String> {
        let f = sheet
            .frames
            .get(frame)
            .ok_or_else(|| format!("Minimap sprite frame{frame} unavailable"))?;
        let inner = sheet.size.map(|v| v as i32);
        let size = if trim {
            inner
        } else {
            f.size.map(|v| v as i32)
        };
        let power = size.map(|v| (v as usize).next_power_of_two() as i32);
        let offset = if trim {
            [0, 0]
        } else {
            f.offset.map(|v| v as i32)
        };
        let mut rgba = vec![0; (power[0] * power[1] * 4) as usize];
        for y in 0..f.size[1] {
            for x in 0..f.size[0] {
                let i = y * f.size[0] + x;
                let mut rgb = sheet.palette[f.indices[i] as usize];
                let opacity = if alpha && f.has_alpha {
                    f.alpha[i]
                } else {
                    u8::from(rgb != 0) * 255
                };
                if let Some(delta) = adjust {
                    if rgb != 0 {
                        rgb = (0..3).fold(0, |out, k| {
                            let shift = 16 - k * 8;
                            out | ((((rgb >> shift) & 255) as i32 + delta[k]).clamp(1, 255) as u32)
                                << shift
                        });
                    }
                }
                let [ox, oy] = if trim { f.offset } else { [0, 0] };
                let j = ((y + oy) * power[0] as usize + x + ox) * 4;
                rgba[j..j + 4].copy_from_slice(&[
                    (rgb >> 16) as u8,
                    (rgb >> 8) as u8,
                    rgb as u8,
                    opacity,
                ]);
            }
        }
        let texture = Texture2D::from_rgba8(power[0] as u16, power[1] as u16, &rgba);
        texture.set_filter(FilterMode::Nearest);
        Ok(Self {
            texture,
            size,
            inner,
            offset,
            power,
        })
    }
}
struct Mask {
    sprite: Sprite,
    rows: Vec<[i32; 2]>,
}
#[derive(Clone, Copy)]
pub struct Marker {
    pub group: i32,
    pub frame: usize,
    pub fine: [i32; 2],
}

pub struct Renderer {
    material: Material,
    map: Texture2D,
    map_pixels: Vec<u32>,
    assets: BTreeMap<String, i32>,
    definitions: BTreeMap<u32, LocationType>,
    mapscenes: BTreeMap<i32, MapScene>,
    indexed: BTreeMap<(i32, i32), Indexed>,
    sheets: BTreeMap<i32, Sheet>,
    sprites: BTreeMap<(i32, usize, bool), Sprite>,
    masks: BTreeMap<i32, Mask>,
    underlays: Vec<floor::Underlay>,
    overlays: Vec<floor::Overlay>,
    palette: Vec<u32>,
    random: JavaRandom,
    mapscene_jitter: [i32; 3],
    function_jitter: [i32; 3],
    floor_jitter: [i32; 2],
    revision: Option<(u64, [i32; 2], usize)>,
    functions: Vec<(u32, [i32; 2])>,
    pub view: View,
    pub fine: [i32; 2],
    pub origin: [i32; 2],
    pub state: i32,
    pub flag: Option<[i32; 2]>,
    pub markers: Vec<Marker>,
    pub hints: Vec<(usize, [i32; 2])>,
    /// Last eligible left click, relative to the component; right-menu actions retain it.
    pub click_offset: [i32; 2],
    /// Immediate selected location types; no asset substitutions on missing variables.
    selected: BTreeMap<u32, Option<u32>>,
    equipment: Option<Vec<i32>>,
    /// `LoginManager.mapElementList` candidates; None when no pack or cache has them.
    labels: Option<minimap::Labels>,
    text: crate::minimap_labels::Text,
    /// Raster calls the labels of the last drawn frame made.
    pub label_draws: usize,
    /// Times the map raster has been built, for captures.
    pub rasters: u32,
}
impl Renderer {
    pub fn new(workbench: &crate::Workbench, seed: i64) -> Result<Self, String> {
        use macroquad::miniquad::{
            BlendFactor, BlendState, BlendValue, Equation, PipelineParams, UniformDesc, UniformType,
        };
        let pack = &workbench.pack;
        let assets: BTreeMap<String, i32> =
            serde_json::from_slice(pack.get("minimap/assets")?).map_err(|e| e.to_string())?;
        let material = load_material(
            ShaderSource::Glsl {
                vertex: VERTEX,
                fragment: FRAGMENT,
            },
            MaterialParams {
                pipeline_params: PipelineParams {
                    color_blend: Some(BlendState::new(
                        Equation::Add,
                        BlendFactor::Value(BlendValue::SourceAlpha),
                        BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
                    )),
                    ..Default::default()
                },
                textures: vec!["MaskTexture".into()],
                uniforms: vec![UniformDesc::new("MaskBounds", UniformType::Float4)],
            },
        )
        .map_err(|e| e.to_string())?;
        let mut definitions = BTreeMap::new();
        let mut mapscenes = BTreeMap::new();
        let mut underlays = vec![floor::Underlay::default(); 256];
        let mut overlays = vec![floor::Overlay::default(); 256];
        for name in pack.manifest.files.keys() {
            if let Some(id) = name.strip_prefix("loc/") {
                definitions.insert(
                    id.parse().map_err(|_| "Minimap location ID")?,
                    LocationType::decode(pack.get(name)?).map_err(|e| e.to_string())?,
                );
            }
            if let Some(id) = name.strip_prefix("mapscene/") {
                mapscenes.insert(
                    id.parse().map_err(|_| "Minimap mapscene ID")?,
                    MapScene::decode(pack.get(name)?).map_err(|e| e.to_string())?,
                );
            }
            if let Some(id) = name.strip_prefix("underlay/") {
                underlays[id.parse::<usize>().map_err(|_| "Minimap underlay ID")?] =
                    floor::Underlay::decode(pack.get(name)?).map_err(|e| e.to_string())?;
            }
            if let Some(id) = name.strip_prefix("overlay/") {
                overlays[id.parse::<usize>().map_err(|_| "Minimap overlay ID")?] =
                    floor::Overlay::decode(pack.get(name)?).map_err(|e| e.to_string())?;
            }
        }
        let mut random = JavaRandom::new(seed);
        // Sprites.load and MsiTypeList.init each own four distinct random draws.
        let a = (random.next_double() * 21.0) as i32 - 10;
        let b = (random.next_double() * 21.0) as i32 - 10;
        let c = (random.next_double() * 21.0) as i32 - 10;
        let base = (random.next_double() * 41.0) as i32 - 20;
        let function_jitter = [b + base, a + base, c + base];
        let mapscene_jitter = minimap::jitter(std::array::from_fn(|_| random.next_double()));
        // SceneGraph's static initialisers; their place among the other
        // Math.random() users is not fixed by the original either.
        let floor_jitter = minimap::Drift::initial_floor(|| random.next_double());
        #[allow(unused_mut)]
        let mut labels = minimap::Labels::from_pack(pack)?;
        #[cfg(not(target_arch = "wasm32"))]
        if labels.is_none() {
            let directory = crate::option("--cache").unwrap_or_else(|| "assets/cache".into());
            labels = minimap::Labels::from_cache(directory.as_ref())
                .map_err(|e| eprintln!("Minimap labels unavailable: {e}"))
                .ok();
        }
        // Diagnostic only: the pinned cache marks no label for the minimap.
        if std::env::args().any(|arg| arg == "--minimap-all-labels") {
            if let Some(labels) = &mut labels {
                labels.show_all();
            }
        }
        let map = Texture2D::from_rgba8(512, 512, &vec![0; 512 * 512 * 4]);
        map.set_filter(FilterMode::Linear);
        Ok(Self {
            material,
            map,
            map_pixels: Vec::new(),
            assets,
            definitions,
            mapscenes,
            indexed: BTreeMap::new(),
            sheets: BTreeMap::new(),
            sprites: BTreeMap::new(),
            masks: BTreeMap::new(),
            underlays,
            overlays,
            palette: rs09_client::surface::palette(0.7),
            random,
            mapscene_jitter,
            function_jitter,
            floor_jitter,
            revision: None,
            functions: Vec::new(),
            view: View::default(),
            fine: [0; 2],
            origin: [0; 2],
            state: 0,
            flag: None,
            markers: Vec::new(),
            hints: Vec::new(),
            click_offset: [0; 2],
            selected: BTreeMap::new(),
            equipment: None,
            labels,
            text: crate::minimap_labels::Text::new()?,
            label_draws: 0,
            rasters: 0,
        })
    }
    /// `LightingManager.minimapRenderedPlane = -1`: draw the map image again
    /// at the next opportunity.
    pub fn invalidate(&mut self) {
        self.revision = None;
    }
    pub fn asset(&self, name: &str) -> Result<i32, String> {
        self.assets
            .get(name)
            .copied()
            .ok_or_else(|| format!("Minimap named asset {name} unavailable"))
    }
    /// The `Sprites.load` map-function offsets and the `MsiTypeList`
    /// map-scene offsets drawn at start-up; the world map uses the same ones.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn sprite_jitters(&self) -> ([i32; 3], [i32; 3]) {
        (self.function_jitter, self.mapscene_jitter)
    }
    /// `SceneGraph.hueJitter` and `lightnessJitter` as of the last floor build.
    pub fn scene_jitter(&self) -> [i32; 2] {
        self.floor_jitter
    }
    pub fn player_team(
        &mut self,
        ui: &mut State,
        raw: &rs09_protocol::game::players::RawAppearance,
    ) -> Result<i32, String> {
        let mut team = raw.npc_team;
        if raw.npc_id != -1 {
            return Ok(team);
        }
        if raw.slots.iter().any(|&v| v >= 32768) && self.equipment.is_none() {
            let mut ids = ui
                .pack
                .manifest
                .files
                .keys()
                .filter_map(|s| s.strip_prefix("item/")?.parse::<i32>().ok())
                .collect::<Vec<_>>();
            ids.sort();
            let mut types = Vec::new();
            for id in ids {
                let item = ui.items.get(&ui.pack, id)?;
                types.push((id, item.manwear, item.womanwear));
            }
            self.equipment = Some(minimap::equipment_ids(types));
        }
        for value in raw.slots {
            if value >= 32768 {
                let id = *self
                    .equipment
                    .as_ref()
                    .and_then(|ids| ids.get((value - 32768) as usize))
                    .ok_or("Minimap appearance equipment index unavailable")?;
                let value = ui.items.get(&ui.pack, id)?.team;
                if value != 0 {
                    team = value;
                }
            }
        }
        Ok(team)
    }
    fn sheet(&mut self, pack: &Pack, id: i32) -> Result<&Sheet, String> {
        if let std::collections::btree_map::Entry::Vacant(e) = self.sheets.entry(id) {
            e.insert(Sheet::decode(pack.get(&format!("sprite/{id}"))?).map_err(|e| e.to_string())?);
        }
        Ok(&self.sheets[&id])
    }
    fn sprite(&mut self, pack: &Pack, id: i32, frame: usize, trim: bool) -> Result<(), String> {
        let key = (id, frame, trim);
        if self.sprites.contains_key(&key) {
            return Ok(());
        }
        let adjust = (id == self.asset("mapfunction")?).then_some(self.function_jitter);
        let alpha = id == self.asset("hint_mapmarkers")?;
        let sprite = Sprite::load(self.sheet(pack, id)?, frame, trim, alpha, adjust)?;
        self.sprites.insert(key, sprite);
        Ok(())
    }
    fn mask(&mut self, pack: &Pack, c: &Component) -> Result<(), String> {
        if self.masks.contains_key(&c.sprite_id) {
            return Ok(());
        }
        let sheet = self.sheet(pack, c.sprite_id)?;
        let mut indexed = Indexed::first(sheet);
        indexed.trim();
        let rows = minimap::click_mask(
            indexed.size[0] as usize,
            indexed.size[1] as usize,
            &indexed.indices,
        );
        let sprite = Sprite::load(sheet, 0, true, false, None)?;
        self.masks.insert(c.sprite_id, Mask { sprite, rows });
        Ok(())
    }
    fn ensure_indexed(&mut self, pack: &Pack, key: (i32, i32)) -> Result<(), String> {
        if self.indexed.contains_key(&key) {
            return Ok(());
        }
        let mut s = Indexed::first(self.sheet(pack, key.0)?);
        s.adjust_palette(self.mapscene_jitter);
        s.inner = s.size;
        for _ in 0..key.1 {
            s.rotate();
        }
        self.indexed.insert(key, s);
        Ok(())
    }
    pub fn hover(
        &mut self,
        pack: &Pack,
        c: &Component,
        node: &DrawNode,
        point: [i32; 2],
    ) -> Result<Option<[i32; 2]>, String> {
        if !matches!(self.state, 0 | 3) || !node.clip.contains(point) {
            return Ok(None);
        }
        self.mask(pack, c)?;
        Ok(self.view.click(
            [node.rect.w, node.rect.h],
            &self.masks[&c.sprite_id].rows,
            [point[0] - node.rect.x, point[1] - node.rect.y],
            self.fine,
        ))
    }
    fn source_plane(workbench: &crate::Workbench, plane: usize, x: i32, y: i32) -> Option<usize> {
        if crate::tile(&workbench.regions, 1, x, y).flags & 2 != 0 {
            (plane < 3).then_some(plane + 1)
        } else {
            Some(plane)
        }
    }
    fn loc<'a>(
        &'a self,
        workbench: &'a crate::Workbench,
        plane: usize,
        x: i32,
        y: i32,
        layer: u8,
    ) -> Option<(world::Location, &'a LocationType)> {
        let source = Self::source_plane(workbench, plane, x, y)?;
        let loc = *workbench.world.locations.get(&world::Slot {
            plane: source,
            tile: [x, y],
            layer,
        })?;
        Some((
            loc,
            workbench
                .location_types
                .get(&(loc.id as u32))
                .or_else(|| self.definitions.get(&(loc.id as u32)))?,
        ))
    }
    pub fn prepare(
        &mut self,
        workbench: &crate::Workbench,
        vm: &Vm,
        ui: &State,
    ) -> Result<(), String> {
        self.origin = workbench.navigation.origin;
        let view_plane = workbench.ground.plane.clamp(0, 3) as usize;
        self.selected.clear();
        for (&id, d) in &self.definitions {
            if d.children.is_some() {
                let mut error = None;
                let selected = d.select_morph(|bit, key| {
                    if bit {
                        vm.varbit(ui, key).unwrap_or_else(|e| {
                            error = Some(e);
                            0
                        })
                    } else {
                        vm.varps.get(&key).copied().unwrap_or(0)
                    }
                });
                if let Some(error) = error {
                    return Err(error);
                }
                self.selected.insert(id, selected);
            }
        }
        // SceneGraph.checkPlaneChange: MiniMap.renderMap runs when the plane
        // differs from LightingManager.minimapRenderedPlane, which a scene
        // rebuild resets. Locations the server changes afterwards do not
        // reach the raster until one of the two happens.
        let drawn = self.revision.map(|r| ((r.0, r.1), r.2));
        let origin = self.origin;
        match minimap::redraw(drawn, (workbench.scene_revision, origin), view_plane) {
            minimap::Redraw::Keep => return Ok(()),
            minimap::Redraw::Plane => {}
            // The floor jitter moves in SceneGraph.buildScene, so not on a plane change.
            minimap::Redraw::Scene => {
                let random = &mut self.random;
                minimap::Drift::step_floor(&mut self.floor_jitter, || random.next_double());
            }
        }
        let mut flat = Vec::new();
        for p in 0..4 {
            let ids = (0..104)
                .flat_map(|x| (0..104).map(move |y| (x, y)))
                .map(|(x, y)| {
                    crate::tile(&workbench.regions, p, x + origin[0], y + origin[1]).underlay
                })
                .collect::<Vec<_>>();
            let blended = floor::blend(104, 104, &ids, &self.underlays);
            for x in 0..104 {
                for y in 0..104 {
                    let t = crate::tile(&workbench.regions, p, x + origin[0], y + origin[1]);
                    let colors = minimap::floor_rgb(
                        (t.underlay != 0).then_some(blended[(x * 104 + y) as usize]),
                        (t.overlay != 0)
                            .then(|| &self.overlays[t.overlay.wrapping_sub(1) as usize]),
                        &workbench.textures.materials,
                        self.floor_jitter,
                        &self.palette,
                    );
                    flat.push(
                        (t.underlay != 0 || t.overlay != 0).then_some(minimap::Tile {
                            shape: if t.overlay == 0 {
                                0
                            } else {
                                t.shape as usize + 1
                            },
                            rotation: t.rotation as usize,
                            underlay: colors[0],
                            overlay: colors[1],
                        }),
                    );
                }
            }
        }
        let mut pixels = minimap::background(
            view_plane,
            |p, x, y| {
                Self::source_plane(workbench, p, x as i32 + origin[0], y as i32 + origin[1])
                    .and_then(|p| flat[p * 104 * 104 + x * 104 + y])
            },
            |p, x, y| {
                crate::tile(
                    &workbench.regions,
                    p,
                    x as i32 + origin[0],
                    y as i32 + origin[1],
                )
                .flags
            },
        );
        self.functions.clear();
        for x in 0..104 {
            for y in 0..104 {
                let Some((loc, d)) =
                    self.loc(workbench, view_plane, x + origin[0], y + origin[1], 3)
                else {
                    continue;
                };
                let mut function = d.map_function;
                if let Some(children) = &d.children {
                    for &child in children {
                        if child != -1 {
                            let child = workbench
                                .location_types
                                .get(&(child as u32))
                                .or_else(|| self.definitions.get(&(child as u32)))
                                .ok_or_else(|| {
                                    format!("Minimap location child{child} unavailable")
                                })?;
                            if child.map_function >= 0 {
                                function = child.map_function;
                                break;
                            }
                        }
                    }
                }
                if function < 0 {
                    continue;
                }
                let mut point = [x, y];
                if ![22, 29, 34, 36, 46, 47, 48].contains(&function) {
                    for _ in 0..10 {
                        let direction = (self.random.next_double() * 4.0) as i32;
                        let map = &workbench.navigation.maps[view_plane];
                        let [px, py] = point;
                        if direction == 0
                            && px > 0
                            && x - 3 < px
                            && map
                                .get(px - 1, py)
                                .is_some_and(|flags| flags & 0x12c0108 == 0)
                        {
                            point[0] -= 1;
                        }
                        if direction == 1
                            && px < 103
                            && x + 3 > px
                            && map
                                .get(px + 1, py)
                                .is_some_and(|flags| flags & 0x12c0180 == 0)
                        {
                            point[0] += 1;
                        }
                        if direction == 2
                            && py > 0
                            && y - 3 < py
                            && map
                                .get(point[0], py - 1)
                                .is_some_and(|flags| flags & 0x12c0102 == 0)
                        {
                            point[1] -= 1;
                        }
                        if direction == 3
                            && py < 103
                            && py < y + 3
                            && map
                                .get(point[0], py + 1)
                                .is_some_and(|flags| flags & 0x12c0120 == 0)
                        {
                            point[1] += 1;
                        }
                    }
                }
                self.functions
                    .push((loc.id as u32, [point[0] + origin[0], point[1] + origin[1]]));
            }
        }
        // Streamed regions can refer to map functions absent from the startup
        // pack. Retain their definitions for the later UI draw pass.
        for &(id, _) in &self.functions {
            if let Some(definition) = workbench.location_types.get(&id) {
                if let Some(children) = &definition.children {
                    for child in children.iter().filter_map(|id| u32::try_from(*id).ok()) {
                        if let Some(variant) = workbench.location_types.get(&child) {
                            self.definitions
                                .entry(child)
                                .or_insert_with(|| variant.clone());
                        }
                    }
                }
                self.definitions
                    .entry(id)
                    .or_insert_with(|| definition.clone());
            }
        }
        let colors = [
            (((self.random.next_double() * 20.0) as u32 + 228) << 8)
                + (((self.random.next_double() * 20.0) as u32 + 228) << 16)
                + (self.random.next_double() * 20.0) as u32
                + 228,
            ((self.random.next_double() * 20.0) as u32 + 228) << 16,
        ];
        for y in 1..103 {
            for x in 1..103 {
                for p in view_plane..=(view_plane + 1).min(3) {
                    let flags =
                        crate::tile(&workbench.regions, p, x + origin[0], y + origin[1]).flags;
                    if p == view_plane && flags & 0x18 != 0 || p > view_plane && flags & 8 == 0 {
                        continue;
                    }
                    for layer in [0, 2, 3] {
                        let Some((loc, d)) =
                            self.loc(workbench, p, x + origin[0], y + origin[1], layer)
                        else {
                            continue;
                        };
                        let d = d.clone();
                        if d.map_scene == -1 {
                            minimap::stroke(
                                &mut pixels,
                                [x as usize, y as usize],
                                layer,
                                loc.shape,
                                loc.rotation,
                                d.interactable != 0,
                                colors,
                            );
                            continue;
                        }
                        let msi = self
                            .mapscenes
                            .get(&d.map_scene)
                            .ok_or_else(|| format!("Mapscene{} unavailable", d.map_scene))?
                            .clone();
                        if msi.sprite == -1 {
                            continue;
                        }
                        let angle = if d.map_scene_rotated {
                            (i32::from(loc.rotation) + d.map_scene_angle) & 3
                        } else {
                            0
                        };
                        let key = (msi.sprite, angle);
                        self.ensure_indexed(&workbench.pack, key)?;
                        minimap::scenery(
                            &mut pixels,
                            [x, y],
                            &d,
                            i32::from(loc.rotation),
                            &msi,
                            &self.indexed[&key],
                        )
                        .map_err(|e| e.to_string())?;
                    }
                }
            }
        }
        let bytes = pixels
            .iter_mut()
            .flat_map(|color| {
                if *color == 0 {
                    *color = 1;
                }
                [(*color >> 16) as u8, (*color >> 8) as u8, *color as u8, 255]
            })
            .collect::<Vec<_>>();
        self.map.update_from_bytes(512, 512, &bytes);
        self.map_pixels = pixels;
        self.revision = Some((workbench.scene_revision, origin, view_plane));
        self.rasters += 1;
        Ok(())
    }
    fn quad(
        &self,
        texture: &Texture2D,
        rect: [f32; 4],
        uv: [[f32; 2]; 4],
        mask: &Sprite,
        mask_origin: [i32; 2],
    ) {
        self.material
            .set_texture("MaskTexture", mask.texture.clone());
        self.material.set_uniform(
            "MaskBounds",
            vec4(
                mask_origin[0] as f32,
                mask_origin[1] as f32,
                mask.power[0] as f32,
                mask.power[1] as f32,
            ),
        );
        let [x, y, w, h] = rect;
        let positions = [[x + w, y], [x, y], [x, y + h], [x + w, y + h]];
        let mesh = Mesh {
            vertices: positions
                .into_iter()
                .zip(uv)
                .map(|(p, uv)| Vertex::new(p[0], p[1], 0., uv[0], uv[1], WHITE))
                .collect(),
            indices: vec![0, 1, 2, 0, 2, 3],
            texture: Some(texture.clone()),
        };
        gl_use_material(&self.material);
        draw_mesh(&mesh);
        gl_use_default_material();
    }
    fn icon(
        &mut self,
        pack: &Pack,
        node: &DrawNode,
        mask_id: i32,
        id: i32,
        frame: usize,
        delta: [i32; 2],
    ) -> Result<(), String> {
        self.sprite(pack, id, frame, false)?;
        let s = &self.sprites[&(id, frame, false)];
        if let Some(at) = self.view.icon([node.rect.w, node.rect.h], s.inner, delta) {
            let at = [
                at[0] + node.rect.x + s.offset[0],
                at[1] + node.rect.y + s.offset[1],
            ];
            let u = s.size[0] as f32 / s.power[0] as f32;
            let v = s.size[1] as f32 / s.power[1] as f32;
            self.quad(
                &s.texture,
                [
                    at[0] as f32,
                    at[1] as f32,
                    s.size[0] as f32,
                    s.size[1] as f32,
                ],
                [[u, 0.], [0., 0.], [0., v], [u, v]],
                &self.masks[&mask_id].sprite,
                [node.rect.x, node.rect.y],
            );
        }
        Ok(())
    }
    pub fn draw(&mut self, ui: &State, pack: &Pack, node: &DrawNode) -> Result<bool, String> {
        let c = &ui.components[&node.key];
        if !matches!(c.client_code, 1338 | 1339) {
            return Ok(false);
        }
        self.mask(pack, c)?;
        let size = [node.rect.w, node.rect.h];
        let at = [node.rect.x, node.rect.y];
        if c.client_code == 1339 {
            if self.state >= 3 {
                let m = &self.masks[&c.sprite_id].sprite;
                draw_texture_ex(
                    &m.texture,
                    at[0] as f32,
                    at[1] as f32,
                    WHITE,
                    DrawTextureParams {
                        source: Some(Rect::new(0., 0., m.size[0] as f32, m.size[1] as f32)),
                        ..Default::default()
                    },
                );
                return Ok(true);
            }
            let id = self.asset("compass")?;
            self.sprite(pack, id, 0, true)?;
            let s = &self.sprites[&(id, 0, true)];
            self.quad(
                &s.texture,
                [at[0] as f32, at[1] as f32, size[0] as f32, size[1] as f32],
                minimap::rotated_uv(size, s.size.map(|v| v / 2), self.view.yaw, 256, s.power),
                &self.masks[&c.sprite_id].sprite,
                at,
            );
            return Ok(true);
        }
        if matches!(self.state, 2 | 5) {
            let m = &self.masks[&c.sprite_id].sprite;
            draw_texture_ex(
                &m.texture,
                at[0] as f32,
                at[1] as f32,
                WHITE,
                DrawTextureParams {
                    source: Some(Rect::new(0., 0., m.size[0] as f32, m.size[1] as f32)),
                    ..Default::default()
                },
            );
            return Ok(true);
        }
        if self.map_pixels.is_empty() {
            return Ok(true);
        }
        let local = [
            self.fine[0] - self.origin[0] * 128,
            self.fine[1] - self.origin[1] * 128,
        ];
        self.quad(
            &self.map,
            [at[0] as f32, at[1] as f32, size[0] as f32, size[1] as f32],
            minimap::rotated_uv(
                size,
                [local[0] / 32 + 48, 464 - local[1] / 32],
                self.view.angle(),
                self.view.zoom_offset + 256,
                [512, 512],
            ),
            &self.masks[&c.sprite_id].sprite,
            at,
        );
        self.label_draws = 0;
        if let Some(list) = self
            .labels
            .as_ref()
            .and_then(|labels| labels.for_origin(self.origin))
        {
            let mask = &self.masks[&c.sprite_id].sprite;
            self.label_draws = self.text.draw(
                &ui.pack,
                pack,
                list,
                self.view,
                size,
                self.origin,
                local,
                at,
                (&mask.texture, mask.power),
            )?;
        }
        for (id, tile) in self.functions.clone() {
            let d = self
                .definitions
                .get(&id)
                .ok_or("Minimap location metadata unavailable")?;
            let d = if d.children.is_some() {
                if let Some(Some(child)) = self.selected.get(&id) {
                    self.definitions
                        .get(child)
                        .ok_or("Minimap selected location unavailable")?
                } else {
                    continue;
                }
            } else {
                d
            };
            if d.map_function < 0 {
                continue;
            }
            self.icon(
                pack,
                node,
                c.sprite_id,
                self.asset("mapfunction")?,
                d.map_function as usize,
                [
                    tile[0] * 4 + 2 - self.fine[0] / 32,
                    tile[1] * 4 + 2 - self.fine[1] / 32,
                ],
            )?;
        }
        for marker in self.markers.clone() {
            self.icon(
                pack,
                node,
                c.sprite_id,
                marker.group,
                marker.frame,
                [
                    marker.fine[0] / 32 - self.fine[0] / 32,
                    marker.fine[1] / 32 - self.fine[1] / 32,
                ],
            )?;
        }
        for (frame, fine) in self.hints.clone() {
            let id = self.asset("hint_mapmarkers")?;
            self.sprite(pack, id, frame, false)?;
            let s = &self.sprites[&(id, frame, false)];
            let delta = [
                fine[0] / 32 - self.fine[0] / 32,
                fine[1] / 32 - self.fine[1] / 32,
            ];
            match self.view.hint(size, s.inner, delta) {
                Some(minimap::HintPosition::Icon(_)) => {
                    self.icon(pack, node, c.sprite_id, id, frame, delta)?
                }
                Some(minimap::HintPosition::Edge { center, angle }) => {
                    let edge = self.asset("hint_mapedge")?;
                    self.sprite(pack, edge, frame, true)?;
                    let s = &self.sprites[&(edge, frame, true)];
                    s.texture.set_filter(FilterMode::Linear);
                    let [cx, cy, degrees, px, py] = minimap::edge_transform(center, angle);
                    let center = vec2(at[0] as f32 + cx, at[1] as f32 + cy);
                    draw_texture_ex(
                        &s.texture,
                        center.x - px,
                        center.y - py,
                        WHITE,
                        DrawTextureParams {
                            source: Some(Rect::new(0., 0., s.size[0] as f32, s.size[1] as f32)),
                            rotation: -degrees.to_radians(),
                            pivot: Some(center),
                            ..Default::default()
                        },
                    );
                }
                None => {}
            }
        }
        if let Some(tile) = self.flag.filter(|t| t[0] != self.origin[0]) {
            self.icon(
                pack,
                node,
                c.sprite_id,
                self.asset("mapflag")?,
                0,
                [
                    tile[0] * 4 + 2 - self.fine[0] / 32,
                    tile[1] * 4 + 2 - self.fine[1] / 32,
                ],
            )?;
        }
        draw_rectangle(
            (at[0] + size[0] / 2 - 1) as f32,
            (at[1] + size[1] / 2 - 1) as f32,
            3.,
            3.,
            WHITE,
        );
        Ok(true)
    }
    #[cfg(not(target_arch = "wasm32"))]
    /// Writes the map background. False when none has been built yet, which
    /// is the case until a minimap component has been drawn.
    pub fn capture(&self, path: &str) -> Result<bool, String> {
        if self.map_pixels.len() != 512 * 512 {
            return Ok(false);
        }
        let data = self
            .map_pixels
            .iter()
            .flat_map(|&p| [(p >> 16) as u8, (p >> 8) as u8, p as u8, 255])
            .collect::<Vec<_>>();
        let image = Image {
            bytes: data,
            width: 512,
            height: 512,
        };
        image.export_png(path);
        Ok(true)
    }
}

const VERTEX: &str = r#"#version 100
attribute vec3 position;attribute vec2 texcoord;attribute vec4 color0;
uniform mat4 Model;uniform mat4 Projection;
varying highp vec2 uv;varying highp vec2 screen_position;
void main(){gl_Position=Projection*Model*vec4(position,1.0);uv=texcoord;screen_position=position.xy;}
"#;
const FRAGMENT: &str = r#"#version 100
precision highp float;varying highp vec2 uv;varying highp vec2 screen_position;
uniform sampler2D Texture;uniform sampler2D MaskTexture;uniform vec4 MaskBounds;
void main(){vec4 color=texture2D(Texture,uv);float mask=texture2D(MaskTexture,(screen_position-MaskBounds.xy)/MaskBounds.zw).a;gl_FragColor=vec4(color.rgb,color.a*mask);}
"#;
