//! The underwater half of the terrain differential: 104 x 104 scenes with
//! underwater map files cut from the pinned cache, the input file
//! `oracle/underwater/rt4/UnderwaterOracle.java` reads, and what the Rust
//! port (`rs09_client::underwater`, `gl_tile` with its water arguments) makes
//! of the same scenes, in the oracle's section format.
use super::{SIZE, Source, put_bytes, put_utf, requests};
use rs09_client::{
    gl_tile,
    interaction::LocationType,
    loc_placement::{self, Finish, Piece},
    location,
    model::Model,
    terrain::{Region, Tile},
    underwater::{self, Layer},
};
use std::collections::BTreeMap;

/// Scene name and the absolute tile of scene tile (0, 0).
///
/// - `lumbridge`: the default scene; its east column of regions has no
///   underwater files, so `clearTerrainRegion` runs.
/// - `salve`, `phasmatys`: rivers and a coast with hundreds of underwater
///   locations, walls and wall decoration among them.
/// - `coast`: ground decoration, a type hidden in HD and hill skew type 5.
/// - `cave`: off the surface map (`centralZoneY >= 800`, so nothing is
///   cleared), with four hill skew type 5 locations.
/// - `west`: multi-tile scenery with hill skew type 4.
/// - `south`: wall decoration and a field of mergeable ground decoration.
pub const SCENES: [(&str, [i32; 2]); 7] = [
    ("lumbridge", [3168, 3168]),
    ("salve", [3376, 3312]),
    ("phasmatys", [3568, 3504]),
    ("coast", [3760, 3504]),
    ("cave", [2672, 10096]),
    ("west", [2672, 3696]),
    ("south", [1936, 4776]),
];

/// A location type of a scene's underwater files.
pub struct Kind {
    /// None for a type the cache lacks or cannot decode: the original's
    /// default type, which has no models.
    pub decoded: Option<(LocationType, location::Definition)>,
    /// The model of each `getStaticEntity` request (`shape * 8 + rotation`)
    /// as `getGlModel` and the diagonal turn leave it, before alignment.
    pub models: BTreeMap<u16, Model>,
}

/// A region's scene-relative origin and its raw `um` and `ul` files.
pub type RegionFiles = ([i32; 2], Option<Vec<u8>>, Option<Vec<u8>>);

pub struct Scene {
    pub name: String,
    pub base: [i32; 2],
    /// The surface's ground level on the tile grid.
    pub underlays: Vec<u8>,
    pub overlays: Vec<u8>,
    pub shapes: Vec<u8>,
    pub rotations: Vec<u8>,
    /// `renderFlags[0]` and `renderFlags[1]`.
    pub flags: [Vec<u8>; 2],
    /// `surfaceTileHeights[0]` on the corner grid.
    pub heights: Vec<i32>,
    /// Scene-relative origin and the raw `um` and `ul` files of every region
    /// the scene touches, in the order the original lists them.
    pub regions: Vec<RegionFiles>,
    pub kinds: BTreeMap<i32, Kind>,
}

/// Every region of the pinned cache with an underwater file, as
/// `(x, z, has um, has ul)`.
pub fn regions(source: &Source) -> Vec<(i32, i32, bool, bool)> {
    let mut out = Vec::new();
    for x in 0..256 {
        for z in 0..256 {
            let has = |prefix: &str| {
                source
                    .maps
                    .index
                    .group_by_ascii_name(&format!("{prefix}{x}_{z}"))
                    .is_some()
            };
            let (terrain, locations) = (has("um"), has("ul"));
            if terrain || locations {
                out.push((x, z, terrain, locations));
            }
        }
    }
    out
}

pub fn scene(source: &Source, name: &str, base: [i32; 2]) -> Result<Scene, String> {
    let span = |axis: usize| (base[axis] >> 6)..=((base[axis] + SIZE as i32 - 1) >> 6);
    let file = |name: String| {
        source
            .maps
            .index
            .group_by_ascii_name(&name)
            .and_then(|id| source.maps.file(id, 0, [0; 4]).ok().flatten())
            .map(|file| file.to_vec())
    };
    let mut surface = Vec::new();
    let mut regions = Vec::new();
    for x in span(0) {
        for z in span(1) {
            if let Some(raw) = file(format!("m{x}_{z}")) {
                surface.push(Region::decode(x, z, &raw).map_err(|e| format!("m{x}_{z}: {e}"))?);
            }
            regions.push((
                [x * 64 - base[0], z * 64 - base[1]],
                file(format!("um{x}_{z}")),
                file(format!("ul{x}_{z}")),
            ));
        }
    }
    let tile = |level: usize, x: i32, y: i32| -> Tile {
        let (x, y) = (base[0] + x, base[1] + y);
        surface
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

    let mut kinds = BTreeMap::<i32, Kind>::new();
    let mut models = BTreeMap::<u32, Model>::new();
    for (_, _, locations) in &regions {
        let Some(locations) = locations else {
            continue;
        };
        let layer = Layer::decode([0, 0], None, Some(locations)).map_err(|e| e.to_string())?;
        for [id, _, _, _, shape, rotation] in layer.locations {
            let kind = kinds.entry(id).or_insert_with(|| Kind {
                decoded: source
                    .locations
                    .file(id as u32 >> 8, id as u32 & 255, [0; 4])
                    .ok()
                    .flatten()
                    .and_then(|raw| LocationType::decode_with_visual(&raw).ok())
                    .map(|(interaction, definition, _)| (interaction, definition)),
                models: BTreeMap::new(),
            });
            let Some((interaction, definition)) = &kind.decoded else {
                continue;
            };
            for (shape, rotation) in requests(shape, rotation) {
                let key = (shape * 8 + rotation) as u16;
                if kind.models.contains_key(&key) {
                    continue;
                }
                for &id in &definition.models {
                    if let std::collections::btree_map::Entry::Vacant(entry) = models.entry(id) {
                        if let Some(model) = source.model(id) {
                            entry.insert(model);
                        }
                    }
                }
                let piece = Piece {
                    shape,
                    rotation,
                    offset: [0; 2],
                    shadow: [0; 2],
                };
                let built = loc_placement::gl_model(
                    definition,
                    shape,
                    rotation,
                    false,
                    &source.materials,
                    |id| models.get(&id),
                )
                .map_err(|id| format!("location {} lacks model {id}", interaction.name))?;
                if let Some(mut model) = built {
                    Finish::new(interaction, definition, piece).turn(&mut model);
                    kind.models.insert(key, model);
                }
            }
        }
    }
    Ok(Scene {
        name: name.to_owned(),
        base,
        underlays: grid(0, |t| t.underlay),
        overlays: grid(0, |t| t.overlay),
        shapes: grid(0, |t| t.shape),
        rotations: grid(0, |t| t.rotation),
        flags: [grid(0, |t| t.flags), grid(1, |t| t.flags)],
        heights: (0..=SIZE as i32)
            .flat_map(|x| (0..=SIZE as i32).map(move |y| (x, y)))
            .map(|(x, y)| tile(0, x, y).height)
            .collect(),
        regions,
        kinds,
    })
}

/// `SceneGraph.centralZoneY` of a scene: its base is `(zone - 6) * 8`.
fn central_zone_y(base: [i32; 2]) -> i32 {
    (base[1] >> 3) + 6
}

/// The file `UnderwaterOracle` reads.
pub fn input(source: &Source, scenes: &[Scene]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend(0x554E4431u32.to_be_bytes());
    for table in [&source.underlay_bytes, &source.overlay_bytes] {
        out.extend((table.len() as u16).to_be_bytes());
        for raw in table {
            put_bytes(&mut out, raw.as_deref());
        }
    }
    out.extend((source.materials.len() as u16).to_be_bytes());
    for material in &source.materials {
        out.extend(
            material
                .as_ref()
                .map_or([0; 3], |m| [m.brightness, m.speed, m.material_type]),
        );
    }
    out.push(scenes.len() as u8);
    for scene in scenes {
        put_utf(&mut out, &scene.name);
        out.extend(central_zone_y(scene.base).to_be_bytes());
        for grid in [
            &scene.underlays,
            &scene.overlays,
            &scene.shapes,
            &scene.rotations,
            &scene.flags[0],
            &scene.flags[1],
        ] {
            out.extend(grid);
        }
        for value in &scene.heights {
            out.extend(value.to_be_bytes());
        }
        out.extend((scene.regions.len() as i32).to_be_bytes());
        for (origin, terrain, locations) in &scene.regions {
            out.extend(origin[0].to_be_bytes());
            out.extend(origin[1].to_be_bytes());
            put_bytes(&mut out, terrain.as_deref());
            put_bytes(&mut out, locations.as_deref());
        }
        out.extend((scene.kinds.len() as i32).to_be_bytes());
        for (id, kind) in &scene.kinds {
            out.extend(id.to_be_bytes());
            let (size, flags, hill) = match &kind.decoded {
                None => ([1, 1], 0, (0u8, -1i16)),
                Some((interaction, definition)) => {
                    let rules = interaction.placement;
                    // As `Finish::new` reads them: a prepared definition
                    // carries opcode 21 only.
                    let hill = if rules.hill_type == 0 && definition.hill {
                        1
                    } else {
                        rules.hill_type
                    };
                    (
                        [definition.width, definition.length],
                        u8::from(rules.hidden_in_hd)
                            | u8::from(interaction.interactable != 0) << 1
                            | u8::from(rules.force_decor) << 2
                            | u8::from(rules.block_walk == 1) << 3
                            | u8::from(interaction.mergeable) << 4
                            | u8::from(loc_placement::is_entity(definition, interaction)) << 5,
                        (hill, rules.hill_amount),
                    )
                }
            };
            out.extend(size[0].to_be_bytes());
            out.extend(size[1].to_be_bytes());
            out.push(flags);
            out.push(hill.0);
            out.extend(hill.1.to_be_bytes());
            out.extend((kind.models.len() as u16).to_be_bytes());
            for (key, model) in &kind.models {
                out.extend(key.to_be_bytes());
                out.extend((model.vertices.len() as i32).to_be_bytes());
                for v in &model.vertices {
                    for value in v {
                        out.extend(value.to_be_bytes());
                    }
                }
            }
        }
    }
    out
}

/// How much of each mechanism a scene exercises.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    /// Surface tiles with water data, and tiles underwater scenery gave some.
    pub watered: usize,
    pub spread: usize,
    /// Regions without an underwater map that were cleared.
    pub cleared: usize,
    /// Underwater tiles below their surface corner.
    pub deep: usize,
    /// Floor faces, and those of tiles an overlay shape cuts.
    pub faces: usize,
    pub cut: usize,
    /// Floor vertices below the surface.
    pub sunk: usize,
    /// Entities in the scene, and those the terrain alignment moved.
    pub entities: usize,
    pub aligned: usize,
    /// Entities by hill skew type (0 for none).
    pub skews: [usize; 6],
    /// Walls, wall decorations and ground decorations among the placements.
    pub walls: usize,
    pub decorations: usize,
    pub ground: usize,
}

fn ints(values: &[i32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_be_bytes()).collect()
}

fn tiles(out: &mut Vec<u8>, groups: &[gl_tile::GlTile]) {
    out.extend((groups.len() as i32).to_be_bytes());
    for tile in groups {
        out.extend(tile.key.to_be_bytes());
        out.extend(tile.texture.to_be_bytes());
        out.push(u8::from(tile.blend));
        out.push(u8::from(tile.underwater));
        out.extend(tile.water_color.to_be_bytes());
        out.extend((tile.x.len() as i32).to_be_bytes());
        // GlTile.uploadVertexData with the underwater coordinate.
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
            out.extend(tile.depths[i].to_be_bytes());
        }
        out.extend((tile.faces.len() as i32).to_be_bytes());
        for face in &tile.faces {
            out.extend(face.level.to_be_bytes());
            out.extend(face.x.to_be_bytes());
            out.extend(face.y.to_be_bytes());
            out.extend((face.vertices.len() as i32).to_be_bytes());
            for &index in &face.vertices {
                out.extend((index as i32).to_be_bytes());
            }
        }
    }
}

fn entity(out: &mut Vec<u8>, model: Option<&Model>) {
    match model {
        None => out.push(0),
        Some(model) => {
            out.push(1);
            out.extend((model.vertices.len() as i32).to_be_bytes());
            for v in &model.vertices {
                for value in v {
                    out.extend(value.to_be_bytes());
                }
            }
        }
    }
}

/// What the Rust port makes of a scene, as the oracle's named sections.
pub fn sections(source: &Source, scene: &Scene) -> (Vec<(String, Vec<u8>)>, Stats) {
    let mut stats = Stats::default();
    let mut out = Vec::new();
    let water = underwater::water_data(
        SIZE,
        SIZE,
        &scene.underlays,
        &scene.overlays,
        &source.overlays,
        |texture| {
            source
                .materials
                .get(texture as usize)
                .and_then(Option::as_ref)
                .is_some_and(|m| m.material_type == rs09_client::materials::WATER)
        },
    );
    stats.watered = water.iter().filter(|&&v| v != 0).count();
    out.push(("water0".to_owned(), ints(&water)));

    let mut under = underwater::Scene::new(SIZE, SIZE, water.clone());
    let mut placements = Vec::new();
    for (origin, terrain, locations) in &scene.regions {
        let layer = Layer::decode([0, 0], terrain.as_deref(), locations.as_deref()).unwrap();
        if let Some(tiles) = &layer.tiles {
            stats.deep += tiles.iter().filter(|tile| tile.height != 0).count();
            under.read(*origin, tiles, &scene.heights);
        }
        placements.extend(
            layer
                .locations
                .iter()
                .map(|&[id, x, y, level, shape, rotation]| {
                    [id, origin[0] + x, origin[1] + y, level, shape, rotation]
                }),
        );
    }
    for (origin, terrain, _) in &scene.regions {
        if terrain.is_none() && central_zone_y(scene.base) < 800 {
            under.clear(*origin);
            stats.cleared += 1;
        }
    }
    out.push(("heights".to_owned(), ints(&under.heights)));
    out.push((
        "tiles".to_owned(),
        [
            &under.underlays,
            &under.overlays,
            &under.shapes,
            &under.rotations,
        ]
        .into_iter()
        .flatten()
        .copied()
        .collect(),
    ));

    let planned = under.add_locations(
        &placements,
        |id| {
            let (interaction, definition) = scene.kinds.get(&id)?.decoded.as_ref()?;
            Some((definition, interaction))
        },
        true,
    );
    stats.spread = under
        .water
        .iter()
        .zip(&water)
        .filter(|(after, before)| after != before)
        .count();
    out.push(("water1".to_owned(), ints(&under.water)));
    out.push((
        "shadow".to_owned(),
        under.shade.levels[0].iter().map(|&v| v as u8).collect(),
    ));
    out.push(("light".to_owned(), ints(&under.shade.lightness(0))));
    out.push((
        "colors".to_owned(),
        rs09_client::floor::blend(SIZE, SIZE, &under.underlays, &source.underlays)
            .into_iter()
            .flat_map(|v| i32::from(v).to_be_bytes())
            .collect(),
    ));

    let tables = gl_tile::Tables {
        underlays: &source.underlays,
        overlays: &source.overlays,
        palette: &source.palette,
        textures: &source.textures,
    };
    let floor = under.floor(&scene.heights, [&scene.flags[0], &scene.flags[1]], &tables);
    assert!(floor.shaped.is_empty());
    for group in &floor.plain {
        stats.faces += group.faces.len();
        stats.cut += group
            .faces
            .iter()
            .filter(|face| under.overlays[face.x as usize * SIZE + face.y as usize] != 0)
            .count();
        stats.sunk += group.depths.iter().filter(|&&depth| depth != 0.0).count();
    }
    let mut bytes = Vec::new();
    tiles(&mut bytes, &floor.plain);
    out.push(("plain".to_owned(), bytes));
    out.push((
        "diag".to_owned(),
        (0..SIZE * SIZE)
            .map(|tile| {
                let (x, y) = (tile / SIZE, tile % SIZE);
                if under.underlays[tile] != 0
                    && under.overlays[tile] == 0
                    && (1..=SIZE - 2).contains(&x)
                    && (1..=SIZE - 2).contains(&y)
                {
                    floor.diagonals[tile]
                } else {
                    255
                }
            })
            .collect(),
    ));

    // The entity slots of every tile, as the oracle walks them.
    under.finish();
    #[derive(Default)]
    struct Slots {
        wall: Option<(i32, [Option<Model>; 2])>,
        decoration: Option<([i32; 2], i32, [Option<Model>; 2])>,
        scenery: Vec<([i32; 2], [i32; 3], Model)>,
        ground: Option<(i32, Model)>,
    }
    let mut slots = BTreeMap::<[i32; 2], Slots>::new();
    let mut models = BTreeMap::<u32, Model>::new();
    for (entry, &[id, x, y, _, shape, _]) in planned.iter().zip(&placements) {
        let Some(planned) = entry else {
            continue;
        };
        let (interaction, definition) = scene.kinds[&id].decoded.as_ref().unwrap();
        for &id in &definition.models {
            if let std::collections::btree_map::Entry::Vacant(entry) = models.entry(id) {
                if let Some(model) = source.model(id) {
                    entry.insert(model);
                }
            }
        }
        let entities = under
            .entities(
                &scene.heights,
                [x, y],
                planned,
                definition,
                interaction,
                &source.materials,
                |id| models.get(&id),
            )
            .unwrap();
        stats.entities += entities.len();
        let skew = Finish::new(
            interaction,
            definition,
            planned.placement.pieces[0].unwrap(),
        )
        .skew
        .map_or(0, |(kind, _)| usize::from(kind));
        stats.skews[skew.min(5)] += entities.len();
        match shape {
            0..=3 => stats.walls += 1,
            4..=8 => stats.decorations += 1,
            22 => stats.ground += 1,
            _ => {}
        }
        for built in &entities {
            let key = (planned.placement.pieces[built.piece].unwrap().shape * 8
                + planned.placement.pieces[built.piece].unwrap().rotation)
                as u16;
            if scene.kinds[&id].models[&key].vertices != built.model.vertices {
                stats.aligned += 1;
            }
        }
        let base = entities.first().map_or(0, |e| e.base);
        let mut pair: [Option<Model>; 2] = [None, None];
        let mut offset = [0; 2];
        for built in entities {
            if built.piece == 0 {
                offset = built.offset;
            }
            pair[built.piece] = Some(built.model);
        }
        let slot = slots.entry([x, y]).or_default();
        match shape {
            0..=3 => slot.wall = Some((base, pair)),
            4..=8 => slot.decoration = Some((offset, base, pair)),
            22 => slot.ground = pair[0].take().map(|model| (base, model)),
            _ => {
                let size = if matches!(shape, 10 | 11) {
                    planned.placement.size
                } else {
                    [1, 1]
                };
                if let Some(model) = pair[0].take() {
                    slot.scenery.push((
                        [x + size[0] - 1, y + size[1] - 1],
                        [x * 128 + size[0] * 64, y * 128 + size[1] * 64, base],
                        model,
                    ));
                }
            }
        }
    }
    out.push((
        "exists".to_owned(),
        under.tiles.iter().map(|&exists| u8::from(exists)).collect(),
    ));
    let mut bytes = Vec::new();
    for (&[x, y], slot) in &slots {
        let head = |bytes: &mut Vec<u8>, kind: u8| bytes.extend([kind, x as u8, y as u8]);
        if let Some((base, pair)) = &slot.wall {
            head(&mut bytes, 1);
            bytes.extend(base.to_be_bytes());
            entity(&mut bytes, pair[0].as_ref());
            entity(&mut bytes, pair[1].as_ref());
        }
        if let Some((offset, base, pair)) = &slot.decoration {
            head(&mut bytes, 4);
            bytes.extend(offset[0].to_be_bytes());
            bytes.extend(offset[1].to_be_bytes());
            bytes.extend(base.to_be_bytes());
            entity(&mut bytes, pair[0].as_ref());
            entity(&mut bytes, pair[1].as_ref());
        }
        for (max, fine, model) in &slot.scenery {
            head(&mut bytes, 2);
            for value in max.iter().chain(fine) {
                bytes.extend(value.to_be_bytes());
            }
            entity(&mut bytes, Some(model));
        }
        if let Some((base, model)) = &slot.ground {
            head(&mut bytes, 3);
            bytes.extend(base.to_be_bytes());
            entity(&mut bytes, Some(model));
        }
    }
    out.push(("locs".to_owned(), bytes));
    (out, stats)
}
