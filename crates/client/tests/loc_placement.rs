//! `loc_placement` against `fixtures/locs/oracle.jsonl`, which
//! `cargo xtask loc-oracle` writes by running the unchanged `LocType`,
//! `GlModel`, `RawModel` and `Model` over the pinned cache (a JVM
//! differential). The tests need `assets/cache` and that fixture and report
//! being skipped without them.
//!
//! What the oracle does not run is `SceneGraph.addLoc` and `Loc`: the tests
//! of `placement`, `Plan` and the clock rule live beside the code and are
//! transcription tests.
use rs09_cache::archive::Archive;
use rs09_client::{
    cache_pack::Source,
    interaction::LocationType,
    loc_placement::{self, Bounds, Finish, Heights, Piece},
    location::Definition,
    model::Model,
    texture::{self, Material},
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, path::PathBuf};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

struct World {
    cases: Vec<Value>,
    types: BTreeMap<u32, (LocationType, Definition)>,
    models: Archive,
    loaded: BTreeMap<u32, Model>,
    materials: Vec<Option<Material>>,
}

fn world() -> Option<World> {
    let fixture = std::fs::read(root().join("fixtures/locs/oracle.jsonl")).ok();
    let (Some(fixture), true) = (fixture, root().join("assets/cache").is_dir()) else {
        eprintln!("skipped: no location oracle fixture or no cache");
        return None;
    };
    let cases = fixture
        .split(|&b| b == b'\n')
        .filter(|line| !line.is_empty())
        .map(|line| serde_json::from_slice(line).unwrap())
        .collect();
    let source = Source::open(&root().join("assets/cache")).unwrap();
    let locations = source.archive(16).unwrap();
    let mut types = BTreeMap::new();
    for group in 0..=0xffffu32 {
        if locations.index.group(group).is_none() {
            if group > 4096 {
                break;
            }
            continue;
        }
        let files = locations.group(group, [0; 4]).unwrap().unwrap();
        for (file, data) in files.iter() {
            let id = group << 8 | file;
            let (kind, visual, _) = LocationType::decode_with_visual(data)
                .unwrap_or_else(|e| panic!("location {id}: {e}"));
            types.insert(id, (kind, visual));
        }
    }
    let materials = texture::materials(
        &source
            .archive(26)
            .unwrap()
            .file(0, 0, [0; 4])
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    Some(World {
        cases,
        types,
        models: source.archive(7).unwrap(),
        loaded: BTreeMap::new(),
        materials,
    })
}

impl World {
    fn cases(&self, kind: &str) -> Vec<Value> {
        self.cases
            .iter()
            .filter(|c| c["case"] == kind)
            .cloned()
            .collect()
    }
    /// `LocType.getGlModel`, with the cache standing in for the model archive.
    fn gl_model(&mut self, id: u32, shape: i32, rotation: i32, animated: bool) -> Option<Model> {
        let d = &self.types[&id].1;
        for &model in &d.models {
            if let std::collections::btree_map::Entry::Vacant(entry) = self.loaded.entry(model) {
                let bytes = self.models.file(model, 0, [0; 4]).unwrap().unwrap();
                entry.insert(Model::decode(&bytes).unwrap());
            }
        }
        loc_placement::gl_model(d, shape, rotation, animated, &self.materials, |model| {
            self.loaded.get(&model)
        })
        .unwrap()
    }
}

fn int(value: &Value) -> i32 {
    value.as_i64().unwrap() as i32
}
fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn vertices(model: &Model) -> Vec<u8> {
    let mut bytes = (model.vertices.len() as i32).to_be_bytes().to_vec();
    for v in &model.vertices {
        for n in v {
            bytes.extend(n.to_be_bytes());
        }
    }
    bytes
}
/// The oracle's `canonical`: vertices, GL triangles in GL order (corner
/// vertex and normal, colour, texture, alpha), the two light intensities.
fn canonical(model: &Model) -> Vec<u8> {
    let mut bytes = vertices(model);
    bytes.extend((model.draw_order.len() as i32).to_be_bytes());
    for &face in &model.draw_order {
        for corner in 0..3 {
            bytes.extend((model.faces[face][corner] as i32).to_be_bytes());
            for n in model.surfaces[face].normals[corner] {
                bytes.extend(n.to_be_bytes());
            }
        }
        bytes.extend(model.colors[face].to_be_bytes());
        bytes.extend((model.textures[face] as i16).to_be_bytes());
        bytes.push(model.alpha[face]);
    }
    bytes.extend(model.lighting[0].to_be_bytes());
    bytes.extend(model.lighting[1].to_be_bytes());
    bytes
}

/// The oracle's generator for synthetic height arrays, 105 by 105.
struct Random(i32);
impl Random {
    fn next(&mut self) -> i32 {
        self.0 = self.0.wrapping_mul(1103515245).wrapping_add(12345);
        ((self.0 as u32 >> 16) & 0x7fff) as i32
    }
    fn heights(&mut self, kind: i32, lift: i32) -> Vec<i32> {
        let mut h = vec![0; 105 * 105];
        for x in 0..105 {
            for z in 0..105 {
                h[x * 105 + z] = match kind {
                    0 => -(self.next() % 96) * 8 - lift,
                    1 => -200 - lift,
                    _ => -(x as i32 * 5 + z as i32 * 3) - (self.next() % 4) - lift,
                };
            }
        }
        h
    }
    /// The ground and the level above it, as the oracle builds the pair.
    fn pair(seed: i32, kind: i32) -> [Vec<i32>; 2] {
        let mut random = Self(seed);
        let ground = random.heights(kind, 0);
        [
            ground,
            random.heights(if kind == 1 { 2 } else { kind }, 240),
        ]
    }
}

#[test]
fn placement_fields_of_every_location_type_match_the_original_decoder() {
    let Some(world) = world() else { return };
    let mut all = Sha256::new();
    let mut listed = BTreeMap::new();
    let mut counts = BTreeMap::<&str, i64>::new();
    for (&id, (kind, d)) in &world.types {
        let p = kind.placement;
        all.update(format!(
            "{id},{},{},{},{},{},{},{},{},{},{},{}\n",
            p.wall_offset,
            p.hill_type,
            p.hill_amount,
            i32::from(p.hidden_in_hd),
            i32::from(p.flat),
            i32::from(p.force_decor),
            i32::from(p.dynamic),
            p.block_walk,
            kind.interactable,
            d.width,
            d.length
        ));
        if p.wall_offset != 16 || p.hill_type > 1 || p.hidden_in_hd {
            listed.insert(
                id,
                serde_json::json!({"case": "definition", "id": id, "walloff": p.wall_offset,
                    "hill": [p.hill_type, p.hill_amount], "render": p.hidden_in_hd}),
            );
        }
        for (name, set) in [
            ("walloff", p.wall_offset != 16),
            ("hill1", p.hill_type == 1),
            ("hill2", p.hill_type == 2),
            ("hill3", p.hill_type == 3),
            ("hill4", p.hill_type == 4),
            ("hill5", p.hill_type == 5),
            ("render", p.hidden_in_hd),
            ("clipped", p.flat),
            ("forcedecor", p.force_decor),
            ("dynamic", p.dynamic),
            ("multiloc", kind.children.is_some()),
        ] {
            *counts.entry(name).or_default() += i64::from(set);
        }
    }
    for case in world.cases("definition") {
        let id = case["id"].as_u64().unwrap() as u32;
        assert_eq!(listed.remove(&id), Some(case), "location {id}");
    }
    assert!(listed.is_empty(), "{:?}", listed.keys());
    let summary = &world.cases("definitions")[0];
    assert_eq!(
        summary["count"].as_u64().unwrap() as usize,
        world.types.len()
    );
    for (name, count) in &counts {
        assert_eq!(summary[*name].as_i64().unwrap(), *count, "{name}");
    }
    assert_eq!(
        format!("{:x}", all.finalize()),
        summary["sha256"].as_str().unwrap()
    );
    eprintln!("{} location types: {counts:?}", world.types.len());
}

#[test]
fn gl_models_of_every_shape_and_rotation_match_the_original() {
    let Some(mut world) = world() else { return };
    let cases = world.cases("glmodel");
    let (mut compared, mut shapes) = (0, std::collections::BTreeSet::new());
    for case in &cases {
        let id = case["id"].as_u64().unwrap() as u32;
        let (shape, rotation) = (int(&case["shape"]), int(&case["rotation"]));
        let name = format!("location {id} shape {shape} rotation {rotation}");
        let baked = world.gl_model(id, shape, rotation, false);
        match case["baked"].as_str() {
            None => assert!(baked.is_none(), "{name}: the original has no model"),
            Some("throws") => {}
            Some(expected) => {
                let model = baked.unwrap_or_else(|| panic!("{name}: no model"));
                assert_eq!(sha(&canonical(&model)), expected, "{name} baked");
                shapes.insert(shape);
                compared += 1;
            }
        }
        if let Some(expected) = case["animated"].as_str().filter(|&e| e != "throws") {
            let mut model = world.gl_model(id, shape, rotation, true).unwrap();
            Finish {
                diagonal: shape == 10 && rotation > 3,
                skew: None,
            }
            .turn(&mut model);
            assert_eq!(sha(&canonical(&model)), expected, "{name} animated");
            compared += 1;
        }
    }
    // Every shape a definition can list a model for. Placement shapes 5 to
    // 8 ask for model shape 4 and shape 11 asks for shape 10.
    assert_eq!(
        shapes.into_iter().collect::<Vec<_>>(),
        [
            0, 1, 2, 3, 4, 9, 10, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22
        ]
    );
    eprintln!("{compared} GL models compared over {} cases", cases.len());
}

/// The oracle's `colours`: what `GlModel.packInterleavedVertexData` packs for
/// each GL triangle's corners without high detail lighting, through a palette
/// that maps every index to itself.
fn baked_colours(
    model: &Model,
    sun: [i32; 3],
    materials: &[Option<Material>],
    palette: &[u32],
) -> Vec<u8> {
    let light = rs09_client::sd_light::ModelLight::new(sun, model.lighting[0], model.lighting[1]);
    let mut bytes = (model.draw_order.len() as i32).to_be_bytes().to_vec();
    for &face in &model.draw_order {
        let material = usize::try_from(model.textures[face])
            .ok()
            .and_then(|id| materials.get(id)?.as_ref());
        for corner in 0..3 {
            bytes.extend(rs09_client::surface::pack_color(
                model.colors[face],
                material,
                light.lightness(model.surfaces[face].normals[corner]),
                model.alpha[face],
                palette,
            ));
        }
    }
    bytes
}

/// JVM differential for standard-detail lighting: `sd_light::ModelLight` and
/// `surface::pack_color` against the colours the unchanged `GlModel` packs
/// with `Preferences.highDetailLighting` off.
#[test]
fn colours_baked_without_high_detail_lighting_match_the_original() {
    let Some(mut world) = world() else { return };
    let cases = world.cases("sdcolour");
    assert!(!cases.is_empty(), "the fixture has no sdcolour cases");
    let palette: Vec<u32> = (0..65536).collect();
    let materials = world.materials.clone();
    let (mut compared, mut suns) = (0, std::collections::BTreeSet::new());
    for case in &cases {
        let id = case["id"].as_u64().unwrap() as u32;
        let (shape, rotation) = (int(&case["shape"]), int(&case["rotation"]));
        let sun = [0, 1, 2].map(|axis| int(&case["sun"][axis]));
        let name = format!("location {id} shape {shape} rotation {rotation} sun {sun:?}");
        if let Some(expected) = case["baked"].as_str() {
            assert_ne!(expected, "throws", "{name}: the original threw");
            let model = world.gl_model(id, shape, rotation, false).unwrap();
            assert_eq!(
                sha(&baked_colours(&model, sun, &materials, &palette)),
                expected,
                "{name} baked"
            );
            compared += 1;
        }
        if let Some(expected) = case["animated"].as_str() {
            assert_ne!(expected, "throws", "{name}: the original threw");
            // The animated entity's diagonal turn moves vertices only.
            let model = world.gl_model(id, shape, rotation, true).unwrap();
            assert_eq!(
                sha(&baked_colours(&model, sun, &materials, &palette)),
                expected,
                "{name} animated"
            );
            compared += 1;
        }
        suns.insert(sun);
    }
    assert_eq!(suns.len(), 3);
    eprintln!(
        "{compared} models' baked colours compared over {} cases",
        cases.len()
    );
}

#[test]
fn hill_skewed_entities_match_the_original_over_synthetic_heights() {
    let Some(mut world) = world() else { return };
    let (mut compared, mut thrown) = (0, 0);
    let mut kinds = std::collections::BTreeSet::new();
    for case in world.cases("entity") {
        let id = case["id"].as_u64().unwrap() as u32;
        let (shape, rotation) = (int(&case["shape"]), int(&case["rotation"]));
        let tile = [int(&case["tile"][0]), int(&case["tile"][1])];
        let [ground, above] = Random::pair(int(&case["seed"]), int(&case["heights"]));
        let read = |x: i32, z: i32| ground[x as usize * 105 + z as usize];
        let read_above = |x: i32, z: i32| above[x as usize * 105 + z as usize];
        let heights = Heights {
            size: [105; 2],
            ground: &read,
            above: case["above"]
                .as_bool()
                .unwrap()
                .then_some(&read_above as &dyn Fn(i32, i32) -> i32),
        };
        let (kind, d) = world.types[&id].clone();
        let finish = Finish::new(
            &kind,
            &d,
            Piece {
                shape,
                rotation,
                offset: [0; 2],
                shadow: [0; 2],
            },
        );
        let size = if rotation & 1 != 0 {
            [d.length, d.width]
        } else {
            [d.width, d.length]
        };
        let center = [
            (size[0] << 6) + (tile[0] << 7),
            (size[1] << 6) + (tile[1] << 7),
        ];
        let base = loc_placement::base_height(tile, size, read);
        let name = format!("{case}");
        kinds.insert(finish.skew.unwrap().0);
        // getStaticEntity: turn, then align within the turned model's bounds.
        match case["baked"].as_str() {
            None => {}
            Some("throws") => thrown += 1,
            Some(expected) => {
                let mut model = world.gl_model(id, shape, rotation, false).unwrap();
                finish.turn(&mut model);
                let bounds = Bounds::of(&model);
                finish.align(&mut model, bounds, &heights, center, base);
                assert_eq!(sha(&vertices(&model)), expected, "baked {name}");
                compared += 1;
            }
        }
        // getAnimatedEntity: the bounds are those of the model before the turn.
        let mut model = world.gl_model(id, shape, rotation, true).unwrap();
        let bounds = Bounds::of(&model);
        finish.turn(&mut model);
        finish.align(&mut model, bounds, &heights, center, base);
        match case["animated"].as_str().unwrap() {
            // The original reads outside its height arrays here.
            "throws" => thrown += 1,
            expected => {
                assert_eq!(sha(&vertices(&model)), expected, "animated {name}");
                compared += 1;
            }
        }
    }
    assert_eq!(kinds.into_iter().collect::<Vec<_>>(), [1, 2, 3, 4, 5]);
    eprintln!("{compared} entities compared; the original throws in {thrown}");
}

#[test]
fn terrain_alignment_types_match_the_original_over_synthetic_heights() {
    let Some(mut world) = world() else { return };
    let mut compared = BTreeMap::<i32, usize>::new();
    let mut thrown = 0;
    for case in world.cases("align") {
        let id = case["id"].as_u64().unwrap() as u32;
        let tile = [int(&case["tile"][0]), int(&case["tile"][1])];
        let [ground, above] = Random::pair(int(&case["seed"]), int(&case["heights"]));
        let read = |x: i32, z: i32| ground[x as usize * 105 + z as usize];
        let read_above = |x: i32, z: i32| above[x as usize * 105 + z as usize];
        let heights = Heights {
            size: [105; 2],
            ground: &read,
            above: case["above"]
                .as_bool()
                .unwrap()
                .then_some(&read_above as &dyn Fn(i32, i32) -> i32),
        };
        let mut model = world
            .gl_model(id, int(&case["shape"]), int(&case["rotation"]), false)
            .unwrap();
        let bounds = Bounds::of(&model);
        let kind = int(&case["type"]);
        loc_placement::align_to_terrain(
            &mut model.vertices,
            kind as u8,
            int(&case["amount"]) as i16,
            bounds,
            &heights,
            [64 + (tile[0] << 7), 64 + (tile[1] << 7)],
            loc_placement::base_height(tile, [1, 1], read),
        );
        match case["vertices"].as_str().unwrap() {
            // Division by zero or a read outside the arrays in the original.
            "ArithmeticException" | "ArrayIndexOutOfBoundsException" => thrown += 1,
            expected => {
                assert_eq!(sha(&vertices(&model)), expected, "{case}");
                *compared.entry(kind).or_default() += 1;
            }
        }
    }
    assert_eq!(compared.len(), 5, "{compared:?}");
    eprintln!("alignments compared by type: {compared:?}; the original throws in {thrown}");
}

#[test]
fn multiloc_children_match_the_original_selection() {
    let Some(world) = world() else { return };
    let cases = world.cases("multiloc");
    for case in &cases {
        let id = case["id"].as_u64().unwrap() as u32;
        let kind = &world.types[&id].0;
        assert_eq!(kind.morph_varbit, int(&case["varbit"]), "{case}");
        assert_eq!(kind.morph_varp, int(&case["varp"]), "{case}");
        let value = int(&case["value"]);
        let chosen = loc_placement::resolve(id, kind, |bit, key| {
            assert_eq!((bit, key), loc_placement::variable(kind).unwrap(), "{case}");
            value
        });
        assert_eq!(
            chosen.map_or(-1, |child| child as i32),
            int(&case["child"]),
            "{case}"
        );
    }
    assert!(cases.len() > 500);
}

/// Region location files: `[id, x, z, plane, shape, rotation]` rows in file
/// order, or None when the region has no file or its key is not known.
fn region_rows(
    maps: &Archive,
    keys: &BTreeMap<i32, [u32; 4]>,
    x: i32,
    z: i32,
) -> Option<Vec<[i32; 6]>> {
    let group = maps.index.group_by_ascii_name(&format!("l{x}_{z}"))?;
    let key = keys.get(&((x << 8) | z)).copied().unwrap_or([0; 4]);
    let data = maps.file(group, 0, key).ok()??;
    let mut at = 0;
    let mut smart = || {
        let first = i32::from(*data.get(at)?);
        if first < 128 {
            at += 1;
            Some(first)
        } else {
            let value = (first << 8 | i32::from(*data.get(at + 1)?)) - 32768;
            at += 2;
            Some(value)
        }
    };
    let (mut rows, mut id) = (vec![], -1);
    loop {
        let mut delta = smart()?;
        if delta == 0 {
            return Some(rows);
        }
        while delta == 32767 {
            id += delta;
            delta = smart()?;
        }
        id += delta;
        let mut position = 0;
        loop {
            let delta = smart()?;
            if delta == 0 {
                break;
            }
            position += delta - 1;
            let info = smart()?;
            rows.push([
                id,
                x * 64 + (position >> 6 & 63),
                z * 64 + (position & 63),
                position >> 12,
                info >> 2,
                info & 3,
            ]);
        }
    }
}

/// Not a comparison: sizes what the placement rules change in a few regions
/// of the pinned cache. Run with `--nocapture` to read the table. With
/// `RS09_LOC_AREA=x0,z0,x1,z1,plane` it also lists that area's rows.
#[test]
fn sweep_of_placements_the_rules_change() {
    let Some(world) = world() else { return };
    let keys = std::fs::read(root().join("runs/player-session/xteas.json"))
        .ok()
        .and_then(|bytes| rs09_client::world_pack::Inputs::parse_keys(&bytes).ok());
    let Some(keys) = keys else {
        eprintln!("skipped: no map keys in runs/player-session/xteas.json");
        return;
    };
    let source = Source::open(&root().join("assets/cache")).unwrap();
    let maps = source.archive(5).unwrap();
    let area: Option<Vec<i32>> = std::env::var("RS09_LOC_AREA")
        .ok()
        .map(|text| text.split(',').map(|n| n.parse().unwrap()).collect());
    let mut total = BTreeMap::<&str, usize>::new();
    for (name, x, z) in [
        ("Lumbridge", 50, 50),
        ("Lumbridge swamp", 50, 49),
        ("Draynor", 48, 50),
        ("Varrock", 50, 53),
        ("Falador", 46, 52),
        ("Edgeville", 48, 54),
        ("Ardougne", 41, 51),
        ("Catherby", 43, 53),
    ] {
        let Some(rows) = region_rows(&maps, &keys, x, z) else {
            eprintln!("{name} ({x},{z}): no location file or key");
            continue;
        };
        let lookup = |id: i32| {
            let (kind, d) = world.types.get(&(id as u32))?;
            Some((d, kind))
        };
        let plan = loc_placement::Plan::new(&rows, lookup, true);
        let mut counts = BTreeMap::<&str, usize>::new();
        for (index, &[id, tx, tz, plane, shape, rotation]) in rows.iter().enumerate() {
            let Some((kind, d)) = world.types.get(&(id as u32)) else {
                continue;
            };
            let p = kind.placement;
            let moved = plan.entries[index].is_some_and(|planned| {
                planned
                    .placement
                    .pieces
                    .iter()
                    .flatten()
                    .any(|piece| piece.offset != [0; 2])
            });
            for (label, hit) in [
                ("rows", true),
                ("corner walls (2)", shape == 2),
                ("decor 5", shape == 5),
                ("decor 6", shape == 6),
                ("decor 7", shape == 7),
                ("decor 8", shape == 8),
                ("decor moved", moved),
                ("diagonal (11)", shape == 11),
                ("hidden in HD (82)", p.hidden_in_hd),
                ("wall offset (28)", p.wall_offset != 16),
                ("hill 1", p.hill_type == 1),
                ("hill 2-5", p.hill_type > 1),
                ("flat (90)", p.flat && shape == 22),
                ("multiloc", kind.children.is_some()),
                (
                    "not in scene",
                    plan.entries[index].is_none() && !p.hidden_in_hd,
                ),
            ] {
                *counts.entry(label).or_default() += usize::from(hit);
                *total.entry(label).or_default() += usize::from(hit);
            }
            if kind.children.is_some() && counts["multiloc"] <= 6 {
                eprintln!(
                    "  multiloc {id} {:?} at {tx},{tz},{plane} shape {shape}: {:?} children {:?}",
                    kind.name,
                    loc_placement::variable(kind),
                    kind.children
                );
            }
            if let Some(a) = &area
                && (a[0]..=a[2]).contains(&tx)
                && (a[1]..=a[3]).contains(&tz)
                && plane == a[4]
            {
                eprintln!(
                    "  {tx},{tz} shape {shape} rotation {rotation}: {id} {:?} size {}x{} {:?} children {:?} -> {:?}",
                    kind.name, d.width, d.length, p, kind.children, plan.entries[index]
                );
            }
        }
        eprintln!("{name} ({x},{z}): {counts:?}");
    }
    eprintln!("all: {total:?}");
}
