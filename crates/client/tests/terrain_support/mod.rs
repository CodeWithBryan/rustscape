//! Shared by `tests/terrain.rs` and `examples/terrain_oracle.rs`: 104 x 104
//! scenes cut from the pinned cache, the input file the JVM oracle reads, and
//! the Rust ports' results in the oracle's section format.
#![allow(dead_code)]
use rs09_cache::{
    Limits,
    archive::Archive,
    store::{StorageMode, Store},
};
use rs09_client::{
    floor::{self, Overlay, Underlay},
    gl_tile,
    interaction::LocationType,
    location,
    model::Model,
    normal_merge::{self, Body, Piece, Placed},
    surface,
    terrain::{Region, Tile},
    texture, tile_shade,
};
use rs09_protocol::buffer::Reader;
use std::{collections::BTreeMap, path::Path};

pub mod underwater;

pub const SIZE: usize = 104;
/// Scene name and the absolute tile of scene tile (0, 0), which the client
/// derives from the player's zone: `(zone - 6) * 8`.
pub const SCENES: [(&str, [i32; 2]); 4] = [
    ("lumbridge", [3168, 3168]),
    ("varrock", [3160, 3376]),
    ("falador", [2912, 3328]),
    ("draynor", [3040, 3216]),
];
/// `clearTerrainRegion` arguments in the original's order: level, the y
/// start, the x start, the y extent, the x extent.
pub const CLEARS: [[i32; 5]; 4] = [
    [0, 40, 0, 64, 64],
    [1, 8, 16, 8, 8],
    [2, 96, 88, 8, 8],
    [3, 100, 100, 8, 8],
];

pub struct Source {
    maps: Archive,
    locations: Archive,
    models: Archive,
    keys: BTreeMap<[i32; 2], [u32; 4]>,
    pub underlay_bytes: Vec<Option<Vec<u8>>>,
    pub overlay_bytes: Vec<Option<Vec<u8>>>,
    pub underlays: Vec<Underlay>,
    pub overlays: Vec<Overlay>,
    pub materials: Vec<Option<texture::Material>>,
    pub textures: Vec<[u8; 2]>,
    pub palette: Vec<u32>,
}

#[derive(serde::Deserialize)]
struct KeyRow {
    #[serde(rename = "regionId")]
    region_id: String,
    keys: String,
}
#[derive(serde::Deserialize)]
struct Keys {
    xteas: Vec<KeyRow>,
}

/// The pinned cache and the recorded map keys, or why they are unavailable.
pub fn open(root: &Path) -> Result<Source, String> {
    let limits = Limits::default();
    let store = Store::open(&root.join("assets/cache"), StorageMode::File, limits)
        .map_err(|e| e.to_string())?;
    let archive = |id| Archive::open(store.clone(), id, None, limits).map_err(|e| e.to_string());
    let raw = std::fs::read(root.join("runs/player-session/xteas.json"))
        .map_err(|e| format!("map keys: {e}"))?;
    let rows: Keys = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
    let mut keys = BTreeMap::new();
    for row in rows.xteas {
        let id = row.region_id.parse::<i32>().map_err(|e| e.to_string())?;
        let key: Vec<u32> = row
            .keys
            .split(',')
            .map(|word| word.trim().parse::<i32>().map(|v| v as u32))
            .collect::<Result<_, _>>()
            .map_err(|e| e.to_string())?;
        keys.insert(
            [id >> 8, id & 255],
            key.try_into().map_err(|_| "invalid map key length")?,
        );
    }
    let configs = archive(2)?;
    let group = |group: u32| -> Vec<Option<Vec<u8>>> {
        (0..256)
            .map(|id| {
                configs
                    .file(group, id, [0; 4])
                    .ok()
                    .flatten()
                    .map(|file| file.to_vec())
            })
            .collect()
    };
    let underlay_bytes = group(1);
    let overlay_bytes = group(4);
    let underlays = underlay_bytes
        .iter()
        .map(|raw| match raw {
            Some(raw) => Underlay::decode(raw).map_err(|e| e.to_string()),
            None => Ok(Underlay::default()),
        })
        .collect::<Result<_, _>>()?;
    let overlays = overlay_bytes
        .iter()
        .map(|raw| match raw {
            Some(raw) => Overlay::decode(raw).map_err(|e| e.to_string()),
            None => Ok(Overlay::default()),
        })
        .collect::<Result<_, _>>()?;
    let materials = archive(26)?
        .file(0, 0, [0; 4])
        .map_err(|e| e.to_string())?
        .ok_or("no material table")?;
    let materials = texture::materials(&materials).map_err(|e| e.to_string())?;
    let textures = materials
        .iter()
        .map(|m| m.as_ref().map_or([0; 2], |m| [m.brightness, m.speed]))
        .collect();
    Ok(Source {
        maps: archive(5)?,
        locations: archive(16)?,
        models: archive(7)?,
        keys,
        underlay_bytes,
        overlay_bytes,
        underlays,
        overlays,
        materials,
        textures,
        palette: surface::palette(0.7),
    })
}

/// What the oracle is told about one location type.
pub struct Kind {
    pub size: [i32; 2],
    pub active: bool,
    pub mergeable: bool,
    /// Its scene entity is a `Loc`: animated or multi-location.
    pub animated: bool,
    pub hill: bool,
    /// Placed models by `shape * 8 + rotation`, as `addLoc` asks for them.
    pub models: BTreeMap<u16, (Body, bool)>,
}

pub struct Scene {
    pub name: String,
    pub underlays: [Vec<u8>; 4],
    pub overlays: [Vec<u8>; 4],
    pub shapes: [Vec<u8>; 4],
    pub rotations: [Vec<u8>; 4],
    pub flags: [Vec<u8>; 4],
    pub heights: [Vec<i32>; 4],
    pub colors: [Vec<i32>; 4],
    /// Scene-relative region origin and its decrypted landscape stream.
    pub landscapes: Vec<([i32; 2], Vec<u8>)>,
    /// Id, scene x, scene y, level, shape, rotation, as `readLocs` admits them.
    pub placements: Vec<[i32; 6]>,
    pub kinds: BTreeMap<i32, Kind>,
}

fn landscape(origin: [i32; 2], bytes: &[u8]) -> rs09_protocol::Result<Vec<[i32; 6]>> {
    let mut r = Reader::new(bytes);
    let mut out = Vec::new();
    let mut id = -1i32;
    loop {
        let offset = r.g_var_smart()?;
        if offset == 0 {
            return Ok(out);
        }
        id = id.wrapping_add(offset);
        let mut coordinate = 0;
        loop {
            let offset = r.gsmarts()?;
            if offset == 0 {
                break;
            }
            coordinate += offset - 1;
            let flags = r.g1()?;
            let x = origin[0] + (coordinate >> 6 & 63);
            let y = origin[1] + (coordinate & 63);
            if x > 0 && y > 0 && x < 103 && y < 103 {
                out.push([id, x, y, coordinate >> 12, flags >> 2, flags & 3]);
            }
        }
    }
}

/// The `(shape, rotation)` arguments `addLoc` passes to `getStaticEntity`
/// for a placement, primary piece first.
pub fn requests(shape: i32, rotation: i32) -> Vec<(i32, i32)> {
    match shape {
        2 => vec![(2, rotation + 4), (2, (rotation + 1) & 3)],
        11 => vec![(10, rotation + 4)],
        4 | 5 => vec![(4, rotation)],
        6 => vec![(4, rotation + 4)],
        7 => vec![(4, ((rotation + 2) & 3) + 4)],
        8 => vec![(4, rotation + 4), (4, ((rotation + 2) & 3) + 4)],
        _ => vec![(shape, rotation)],
    }
}

impl Source {
    fn model(&self, id: u32) -> Option<Model> {
        let bytes = self.models.file(id, 0, [0; 4]).ok().flatten()?;
        Model::decode(&bytes).ok()
    }
    /// The placed model of a type for one `getStaticEntity` request, built
    /// the way the client's scene builder places locations today: the first
    /// model of the shape, mirrored, turned by the quadrant, resized and
    /// offset. The extra 45 degree turns of rotations 4..=7 are not applied;
    /// both sides of the differential are given the same vertices.
    fn placed(
        &self,
        definition: &location::Definition,
        shape: i32,
        rotation: i32,
        cache: &mut BTreeMap<u32, Option<Model>>,
    ) -> Option<Model> {
        let mut load = |id: u32| cache.entry(id).or_insert_with(|| self.model(id)).clone();
        let mut model = match &definition.shapes {
            None => {
                if shape != 10 || definition.models.is_empty() {
                    return None;
                }
                let parts = definition
                    .models
                    .iter()
                    .map(|&id| load(id))
                    .collect::<Option<Vec<_>>>()?;
                if parts.len() == 1 {
                    parts[0].clone()
                } else {
                    Model::combine(&parts)
                }
            }
            Some(shapes) => {
                let index = shapes.iter().position(|&s| s == shape)?;
                load(*definition.models.get(index)?)?
            }
        };
        model.draw_order = surface::face_order(&model, &self.materials);
        model.surfaces = surface::prepare(&model, &self.materials);
        model.lighting = [64 + definition.ambient, 768 + definition.contrast * 5];
        for pair in &definition.recolor {
            model.recolor(&[*pair]);
        }
        for &[src, dst] in &definition.retexture {
            for texture in &mut model.textures {
                if *texture == i32::from(src) {
                    *texture = i32::from(dst);
                }
            }
        }
        if definition.mirror != (shape == 2 && rotation > 3) {
            for v in &mut model.vertices {
                v[2] = -v[2];
            }
            for (face, surface) in model.faces.iter_mut().zip(&mut model.surfaces) {
                face.swap(0, 2);
                surface.uv.swap(0, 2);
                surface.normals.swap(0, 2);
                for n in &mut surface.normals {
                    n[2] = i32::from((-n[2]) as i16);
                }
            }
        }
        location::rotate(&mut model, rotation & 3, true);
        for v in &mut model.vertices {
            for (axis, value) in v.iter_mut().enumerate() {
                *value = (value.wrapping_mul(definition.resize[axis]) >> 7)
                    .wrapping_add(definition.offset[axis]);
            }
        }
        Some(model)
    }

    pub fn scene(&self, name: &str, base: [i32; 2]) -> Result<Scene, String> {
        let span = |axis: usize| (base[axis] >> 6)..=((base[axis] + SIZE as i32 - 1) >> 6);
        let mut regions = Vec::new();
        let mut landscapes = Vec::new();
        let mut placements = Vec::new();
        for x in span(0) {
            for z in span(1) {
                let named = |name: String| self.maps.index.group_by_ascii_name(&name);
                if let Some(raw) = named(format!("m{x}_{z}"))
                    .and_then(|id| self.maps.file(id, 0, [0; 4]).ok().flatten())
                {
                    regions.push(Region::decode(x, z, &raw).map_err(|e| format!("m{x}_{z}: {e}"))?);
                }
                // A region without a recorded key keeps its terrain only.
                let stream = self.keys.get(&[x, z]).and_then(|&key| {
                    named(format!("l{x}_{z}"))
                        .and_then(|id| self.maps.file(id, 0, key).ok().flatten())
                });
                if let Some(stream) = stream {
                    let origin = [x * 64 - base[0], z * 64 - base[1]];
                    placements
                        .extend(landscape(origin, &stream).map_err(|e| format!("l{x}_{z}: {e}"))?);
                    landscapes.push((origin, stream.to_vec()));
                }
            }
        }
        let tile = |level: usize, x: i32, y: i32| -> Tile {
            let (x, y) = (base[0] + x, base[1] + y);
            regions
                .iter()
                .find(|r| r.x == x >> 6 && r.z == y >> 6)
                .map_or(Tile::default(), |r| {
                    r.tile(level, (x & 63) as usize, (y & 63) as usize)
                })
        };
        let grid = |level: usize, field: fn(&Tile) -> u8| -> Vec<u8> {
            (0..SIZE as i32)
                .flat_map(|x| (0..SIZE as i32).map(move |y| (x, y)))
                .map(|(x, y)| field(&tile(level, x, y)))
                .collect()
        };
        let underlays: [Vec<u8>; 4] = std::array::from_fn(|l| grid(l, |t| t.underlay));
        let colors = std::array::from_fn(|l| {
            floor::blend(SIZE, SIZE, &underlays[l], &self.underlays)
                .into_iter()
                .map(i32::from)
                .collect()
        });
        let mut kinds = BTreeMap::new();
        let mut models = BTreeMap::new();
        for &[id, _, _, _, shape, rotation] in &placements {
            let raw = self
                .locations
                .file(id as u32 >> 8, id as u32 & 255, [0; 4])
                .ok()
                .flatten();
            let decoded = raw.and_then(|raw| LocationType::decode_with_visual(&raw).ok());
            let kind = kinds.entry(id).or_insert_with(|| match &decoded {
                // A type the cache lacks is the original's default type.
                None => Kind {
                    size: [1, 1],
                    active: true,
                    mergeable: false,
                    animated: false,
                    hill: false,
                    models: BTreeMap::new(),
                },
                Some((interaction, definition, _)) => Kind {
                    size: [definition.width, definition.length],
                    active: interaction.active,
                    mergeable: interaction.mergeable,
                    animated: ![-1, 65535].contains(&definition.animation)
                        || interaction.children.is_some(),
                    hill: definition.hill,
                    models: BTreeMap::new(),
                },
            });
            let Some((_, definition, _)) = &decoded else {
                continue;
            };
            if kind.animated {
                continue;
            }
            for (shape, rotation) in requests(shape, rotation) {
                let key = (shape * 8 + rotation) as u16;
                if kind.models.contains_key(&key) {
                    continue;
                }
                let Some(model) = self.placed(definition, shape, rotation, &mut models) else {
                    continue;
                };
                // Only merging and centrepiece shade read the geometry; any
                // other model need only exist.
                let body = if kind.mergeable || shape == 10 {
                    let faces: Vec<usize> = model
                        .draw_order
                        .iter()
                        .copied()
                        .filter(|&i| surface::visible(&model, i, &self.materials))
                        .collect();
                    Body::from_model(&model, &faces, kind.mergeable).0
                } else {
                    // One vertex at the centre, so that hill skew has bounds.
                    let mut body = Body::solid();
                    body.vertices = vec![[0; 3]];
                    body.lookup = vec![Vec::new()];
                    body.uses = vec![0];
                    body
                };
                kind.models.insert(key, (body, kind.mergeable));
            }
        }
        Ok(Scene {
            name: name.to_owned(),
            overlays: std::array::from_fn(|l| grid(l, |t| t.overlay)),
            shapes: std::array::from_fn(|l| grid(l, |t| t.shape)),
            rotations: std::array::from_fn(|l| grid(l, |t| t.rotation)),
            flags: std::array::from_fn(|l| grid(l, |t| t.flags)),
            heights: std::array::from_fn(|l| {
                (0..=SIZE as i32)
                    .flat_map(|x| (0..=SIZE as i32).map(move |y| (x, y)))
                    .map(|(x, y)| tile(l, x, y).height)
                    .collect()
            }),
            underlays,
            colors,
            landscapes,
            placements,
            kinds,
        })
    }
}

fn put_bytes(out: &mut Vec<u8>, bytes: Option<&[u8]>) {
    match bytes {
        None => out.extend((-1i32).to_be_bytes()),
        Some(bytes) => {
            out.extend((bytes.len() as i32).to_be_bytes());
            out.extend(bytes);
        }
    }
}
fn put_utf(out: &mut Vec<u8>, text: &str) {
    out.extend((text.len() as u16).to_be_bytes());
    out.extend(text.as_bytes());
}

/// The file `TerrainOracle` reads.
pub fn input(source: &Source, scenes: &[Scene]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend(0x54455231u32.to_be_bytes());
    for table in [&source.underlay_bytes, &source.overlay_bytes] {
        out.extend((table.len() as u16).to_be_bytes());
        for raw in table {
            put_bytes(&mut out, raw.as_deref());
        }
    }
    out.extend((source.textures.len() as u16).to_be_bytes());
    for texture in &source.textures {
        out.extend(texture);
    }
    out.extend((CLEARS.len() as i32).to_be_bytes());
    for call in CLEARS {
        for value in call {
            out.extend(value.to_be_bytes());
        }
    }
    out.push(scenes.len() as u8);
    for scene in scenes {
        put_utf(&mut out, &scene.name);
        for level in 0..4 {
            out.extend(&scene.underlays[level]);
            out.extend(&scene.overlays[level]);
            out.extend(&scene.shapes[level]);
            out.extend(&scene.rotations[level]);
            out.extend(&scene.flags[level]);
            for grid in [&scene.heights[level], &scene.colors[level]] {
                for value in grid {
                    out.extend(value.to_be_bytes());
                }
            }
        }
        out.extend((scene.kinds.len() as i32).to_be_bytes());
        for (id, kind) in &scene.kinds {
            out.extend(id.to_be_bytes());
            out.extend(kind.size[0].to_be_bytes());
            out.extend(kind.size[1].to_be_bytes());
            out.push(
                u8::from(kind.active)
                    | u8::from(kind.mergeable) << 1
                    | u8::from(kind.animated) << 2
                    | u8::from(kind.hill) << 3,
            );
            out.extend((kind.models.len() as u16).to_be_bytes());
            for (key, (body, _)) in &kind.models {
                out.extend(key.to_be_bytes());
                out.extend((body.vertices.len() as i32).to_be_bytes());
                for v in &body.vertices {
                    for value in v {
                        out.extend(value.to_be_bytes());
                    }
                }
                out.extend((body.normals.len() as i32).to_be_bytes());
                for normal in &body.normals {
                    for value in normal {
                        out.extend(value.to_be_bytes());
                    }
                }
                for (lookup, &uses) in body.lookup.iter().zip(&body.uses) {
                    out.extend((lookup.len() as u16).to_be_bytes());
                    out.extend((uses as u16).to_be_bytes());
                    for &index in lookup {
                        out.extend((index as u16).to_be_bytes());
                    }
                }
                out.extend((body.triangles as i32).to_be_bytes());
            }
        }
        out.extend((scene.landscapes.len() as i32).to_be_bytes());
        for (origin, stream) in &scene.landscapes {
            out.extend(origin[0].to_be_bytes());
            out.extend(origin[1].to_be_bytes());
            put_bytes(&mut out, Some(stream));
        }
    }
    out
}

fn tiles(out: &mut Vec<u8>, groups: &[gl_tile::GlTile]) {
    out.extend((groups.len() as i32).to_be_bytes());
    for tile in groups {
        out.extend(tile.key.to_be_bytes());
        out.extend(tile.texture.to_be_bytes());
        out.push(u8::from(tile.blend));
        out.extend(tile.water_color.to_be_bytes());
        out.extend((tile.x.len() as i32).to_be_bytes());
        // GlTile.uploadVertexData.
        for i in 0..tile.x.len() {
            out.extend((tile.x[i] as f32).to_be_bytes());
            out.extend((tile.y[i] as f32).to_be_bytes());
            out.extend((tile.z[i] as f32).to_be_bytes());
            out.extend(tile.colors[i].to_be_bytes());
            for value in tile.normals[i] {
                out.extend(value.to_be_bytes());
            }
            out.extend((tile.x[i] as f32 / tile.scale).to_be_bytes());
            out.extend((tile.z[i] as f32 / tile.scale).to_be_bytes());
        }
        out.extend((tile.faces.len() as i32).to_be_bytes());
        for face in &tile.faces {
            out.extend(face.level.to_be_bytes());
            out.extend(face.x.to_be_bytes());
            out.extend(face.y.to_be_bytes());
            out.push(u8::from(face.blend));
            out.extend((face.vertices.len() as i32).to_be_bytes());
            for &index in &face.vertices {
                out.extend((index as i32).to_be_bytes());
            }
            match &face.blend_indices {
                None => out.extend((-1i32).to_be_bytes()),
                Some(indices) => {
                    out.extend((indices.len() as i32).to_be_bytes());
                    for &index in indices {
                        out.extend((index as i32).to_be_bytes());
                    }
                }
            }
        }
    }
}

fn shadow_bytes(map: &tile_shade::ShadowMap) -> Vec<u8> {
    map.levels
        .iter()
        .flat_map(|level| level.iter().map(|&v| v as u8))
        .collect()
}

/// `clearTerrainRegion` over fresh maps, one map set per call.
pub fn clear_section() -> Vec<u8> {
    let mut out = Vec::new();
    for [level, y, x, depth, width] in CLEARS {
        let mut map = tile_shade::ShadowMap::new(SIZE, SIZE);
        map.clear_region(level as usize, x, y, width, depth);
        out.extend(shadow_bytes(&map));
    }
    out
}

/// How much of each mechanism a scene exercises.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    /// Corners with any baked shade.
    pub shaded: usize,
    /// Underlay faces that are a second or later texture layer of their tile.
    pub layered: usize,
    /// Blended overlay fans and overlay faces with fade triangles.
    pub blended: usize,
    /// Mergeable models, and those whose normals merging changed.
    pub mergeable: usize,
    pub merged: usize,
}

/// What the Rust ports make of a scene, as the oracle's named sections.
pub fn sections(source: &Source, scene: &Scene) -> (Vec<(String, Vec<u8>)>, Stats) {
    let mut stats = Stats::default();
    let size = SIZE as i32;
    let mut out = Vec::new();
    let mut shade = tile_shade::ShadowMap::new(SIZE, SIZE);
    let mut slots = normal_merge::Scene::new(SIZE, SIZE);
    let height = |level: usize, x: i32, y: i32| {
        if (0..=size).contains(&x) && (0..=size).contains(&y) {
            scene.heights[level][x as usize * (SIZE + 1) + y as usize]
        } else {
            0
        }
    };
    // Whether an entity is a model (dumped with its data) or a Loc.
    let mut is_model = Vec::new();
    for &[id, x, y, level, shape, rotation] in &scene.placements {
        let kind = &scene.kinds[&id];
        let level = level as usize;
        let mut pieces = [Piece::None, Piece::None];
        for (slot, (model_shape, model_rotation)) in
            requests(shape, rotation).into_iter().enumerate()
        {
            pieces[slot] = if kind.animated {
                Piece::Animated
            } else {
                match kind
                    .models
                    .get(&((model_shape * 8 + model_rotation) as u16))
                {
                    None => Piece::None,
                    Some((body, mergeable)) => {
                        let mut body = body.clone();
                        body.mergeable = *mergeable;
                        if kind.hill {
                            normal_merge::stand(
                                &mut body,
                                |x, y| height(level, x, y),
                                [size, size],
                                [x, y],
                                rotation as u8,
                                kind.size,
                            );
                        }
                        Piece::Model(body)
                    }
                }
            };
        }
        // Wall decorations take no slot here; their entities are not kept.
        if (4..=8).contains(&shape) {
            pieces = [Piece::None, Piece::None];
        }
        let animated = kind.animated;
        let entities = normal_merge::add_loc(
            &mut slots,
            &mut shade,
            Placed {
                level,
                tile: [x, y],
                shape: shape as u8,
                rotation: rotation as u8,
                size: kind.size,
                active: kind.active,
                pieces,
            },
        );
        for entity in entities.into_iter().flatten() {
            if is_model.len() <= entity {
                is_model.resize(entity + 1, false);
            }
            is_model[entity] = !animated;
        }
    }
    stats.shaded = shade.levels.iter().flatten().filter(|&&v| v != 0).count();
    out.push(("shadow".to_owned(), shadow_bytes(&shade)));
    let tables = gl_tile::Tables {
        underlays: &source.underlays,
        overlays: &source.overlays,
        palette: &source.palette,
        textures: &source.textures,
    };
    for level in 0..4 {
        let lightness = shade.lightness(level);
        out.push((
            format!("light{level}"),
            lightness.iter().flat_map(|v| v.to_be_bytes()).collect(),
        ));
        let floor = gl_tile::build(
            &gl_tile::Level {
                width: SIZE,
                depth: SIZE,
                level,
                underlays: &scene.underlays[level],
                overlays: &scene.overlays[level],
                shapes: &scene.shapes[level],
                rotations: &scene.rotations[level],
                flags: &scene.flags[level],
                bridge_flags: &scene.flags[1],
                heights: &scene.heights[level],
                lightness: &lightness,
                colors: &scene.colors[level],
                water: None,
            },
            &tables,
        );
        let mut seen = std::collections::BTreeSet::new();
        stats.layered += floor
            .plain
            .iter()
            .flat_map(|group| &group.faces)
            .filter(|face| !seen.insert((face.x, face.y)))
            .count();
        stats.blended += floor
            .shaped
            .iter()
            .flat_map(|group| &group.faces)
            .filter(|face| face.blend || face.blend_indices.is_some())
            .count();
        let mut bytes = Vec::new();
        tiles(&mut bytes, &floor.plain);
        out.push((format!("plain{level}"), bytes));
        let mut bytes = Vec::new();
        tiles(&mut bytes, &floor.shaped);
        out.push((format!("shaped{level}"), bytes));
        let diagonals = (0..SIZE * SIZE)
            .map(|tile| {
                let (x, y) = (tile / SIZE, tile % SIZE);
                if scene.underlays[level][tile] != 0
                    && scene.overlays[level][tile] == 0
                    && (1..=SIZE - 2).contains(&x)
                    && (1..=SIZE - 2).contains(&y)
                {
                    floor.diagonals[tile]
                } else {
                    255
                }
            })
            .collect();
        out.push((format!("diag{level}"), diagonals));
    }
    let before: Vec<_> = slots
        .bodies
        .iter()
        .map(|body| body.mergeable.then(|| body.normals.clone()))
        .collect();
    slots.merge(height);
    stats.mergeable = before.iter().flatten().count();
    stats.merged = before
        .iter()
        .zip(&slots.bodies)
        .filter(|(old, body)| old.as_ref().is_some_and(|old| *old != body.normals))
        .count();
    let mut bytes = Vec::new();
    let entity = |bytes: &mut Vec<u8>, entity: Option<usize>| match entity {
        None => bytes.push(0),
        Some(entity) if !is_model[entity] => bytes.push(2),
        Some(entity) => {
            let body = &slots.bodies[entity];
            bytes.push(1);
            bytes.extend((body.vertices.len() as i32).to_be_bytes());
            for v in &body.vertices {
                for value in v {
                    bytes.extend(value.to_be_bytes());
                }
            }
            bytes.extend((body.normals.len() as i32).to_be_bytes());
            for normal in &body.normals {
                for value in normal {
                    bytes.extend(value.to_be_bytes());
                }
            }
        }
    };
    for level in 0..4 {
        for x in 0..size {
            for y in 0..size {
                let (wall, scenery, ground) = slots.slots(level, x, y);
                let slot = |bytes: &mut Vec<u8>, kind: u8| {
                    bytes.extend([kind, level as u8, x as u8, y as u8]);
                };
                if let Some([primary, secondary]) = wall {
                    slot(&mut bytes, 1);
                    entity(&mut bytes, primary);
                    entity(&mut bytes, secondary);
                }
                for (id, min, max) in scenery {
                    if min == [x, y] {
                        slot(&mut bytes, 2);
                        bytes.extend(max[0].to_be_bytes());
                        bytes.extend(max[1].to_be_bytes());
                        entity(&mut bytes, Some(id));
                    }
                }
                if ground.is_some() {
                    slot(&mut bytes, 3);
                    entity(&mut bytes, ground);
                }
            }
        }
    }
    out.push(("merge".to_owned(), bytes));
    // Without high detail lighting: buildScene's other tileLightness loop
    // under each light position, and under the first the floor it gives with
    // the palette of brightness preference 4.
    let palette = surface::palette(SD_BRIGHTNESS[2].1);
    let tables = gl_tile::Tables {
        palette: &palette,
        ..tables
    };
    for (index, sun) in SD_SUNS.into_iter().enumerate() {
        for level in 0..4 {
            let lightness = shade.lightness_sd(level, &scene.heights[level], sun);
            out.push((
                format!("sdlight{index}_{level}"),
                lightness.iter().flat_map(|v| v.to_be_bytes()).collect(),
            ));
            if index != 0 {
                continue;
            }
            let floor = gl_tile::build(
                &gl_tile::Level {
                    width: SIZE,
                    depth: SIZE,
                    level,
                    underlays: &scene.underlays[level],
                    overlays: &scene.overlays[level],
                    shapes: &scene.shapes[level],
                    rotations: &scene.rotations[level],
                    flags: &scene.flags[level],
                    bridge_flags: &scene.flags[1],
                    heights: &scene.heights[level],
                    lightness: &lightness,
                    colors: &scene.colors[level],
                    water: None,
                },
                &tables,
            );
            let mut bytes = Vec::new();
            tiles(&mut bytes, &floor.plain);
            out.push((format!("sdplain{level}"), bytes));
            let mut bytes = Vec::new();
            tiles(&mut bytes, &floor.shaped);
            out.push((format!("sdshaped{level}"), bytes));
        }
    }
    (out, stats)
}

/// `FogManager.light0Position` values the standard-detail sections use.
pub const SD_SUNS: [[i32; 3]; 2] = [[-50, -60, -50], [-30, -50, 60]];
/// The brightness preferences other than 3 with the palette brightness
/// `Rasteriser.setBrightness` is given for each.
pub const SD_BRIGHTNESS: [(i32, f32); 3] = [(1, 0.9), (2, 0.8), (4, 0.6)];

/// `Rasteriser.palette` at each of those, after the `clear` section.
pub fn palette_sections() -> Vec<(String, Vec<u8>)> {
    SD_BRIGHTNESS
        .into_iter()
        .map(|(preference, brightness)| {
            (
                format!("palette/brightness{preference}"),
                surface::palette(brightness)
                    .iter()
                    .flat_map(|v| v.to_be_bytes())
                    .collect(),
            )
        })
        .collect()
}

/// The sections of an oracle output file, in order.
pub fn read_sections(data: &[u8]) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    let mut at = 0;
    while at < data.len() {
        let length = usize::from(u16::from_be_bytes([data[at], data[at + 1]]));
        let name = String::from_utf8_lossy(&data[at + 2..at + 2 + length]).into_owned();
        at += 2 + length;
        let size = u32::from_be_bytes(data[at..at + 4].try_into().unwrap()) as usize;
        out.push((name, data[at + 4..at + 4 + size].to_vec()));
        at += 4 + size;
    }
    out
}
