//! Roof removal against the original's flood fill.
//!
//! `fixtures/roofs/oracle.jsonl` is written by `cargo xtask roof-oracle`: the
//! roof methods of ScriptRunner and the tile bookkeeping of SceneGraph, copied
//! unchanged out of the pinned sources and run on the JVM over three scenes of
//! the pinned cache and one invented scene. `fixtures/roofs/provenance.json` says exactly which parts
//! ran unchanged and which inputs came from the Rust decoders.
//!
//! The last two tests are transcription tests: their expectations were worked
//! out by hand from SceneGraph.renderTile, GlTile.renderTiles and
//! LocType.getGlModel and do not run the original.
use rs09_client::roof::{self, Camera, Mask, Mode, Placement, Roofs, Scene};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

fn fixture() -> Vec<Value> {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/roofs/oracle.jsonl");
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}
fn int(value: &Value) -> i32 {
    value.as_i64().unwrap() as i32
}
fn ints<const N: usize>(value: &Value) -> [i32; N] {
    let list: Vec<i32> = value.as_array().unwrap().iter().map(int).collect();
    list.try_into().unwrap()
}
fn fnv(bytes: impl IntoIterator<Item = u8>) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
    }
    format!("{hash:x}")
}
fn scene(case: &Value) -> Scene {
    let flags = case["flags"].as_str().unwrap().as_bytes();
    let flag = |plane: usize, x: usize, y: usize| {
        let at = (plane * 10816 + x * 104 + y) * 2;
        u8::from_str_radix(std::str::from_utf8(&flags[at..at + 2]).unwrap(), 16).unwrap()
    };
    let heights: Vec<i32> = case["heights"]
        .as_array()
        .unwrap()
        .iter()
        .map(int)
        .collect();
    let mut scene = Scene::new(flag, |plane, x, y| heights[plane * 105 * 105 + x * 105 + y]);
    for loc in case["locs"].as_array().unwrap() {
        let [plane, x, y, shape, rotation, width, length, entity] = ints(loc);
        scene.add(Placement {
            plane: plane as usize,
            tile: [x, y],
            shape: shape as u8,
            rotation: rotation as u8,
            size: [width, length],
            entity: entity == 1,
        });
    }
    scene.bridge();
    scene
}

#[test]
fn the_tile_array_and_every_frame_match_the_original_fill() {
    let mut scenes = BTreeMap::new();
    let mut states = BTreeMap::<String, Roofs>::new();
    let (mut steps, mut hiding) = (0, 0);
    for case in fixture() {
        if case["case"] == "scene" {
            let built = scene(&case);
            let dump = built.dump();
            assert_eq!(
                dump.lines().count() as u64,
                case["tiles"].as_u64().unwrap(),
                "{}",
                case["name"]
            );
            assert_eq!(
                format!("{:x}", Sha256::digest(dump.as_bytes())),
                case["tilesSha256"].as_str().unwrap(),
                "{}: walls and scenery of the tile array",
                case["name"]
            );
            scenes.insert(case["name"].as_str().unwrap().to_owned(), built);
            continue;
        }
        let name = case["scene"].as_str().unwrap();
        let scene = &scenes[name];
        let roofs = states.entry(name.to_owned()).or_default();
        let mode = Mode::new(
            case["never"].as_bool().unwrap(),
            case["selective"].as_bool().unwrap(),
        );
        let plane = int(&case["plane"]);
        roofs.cycle = int(&case["cycle"]);
        if case["update"].as_bool().unwrap() {
            roofs.update_mode(mode, scene, plane);
        }
        roofs.frame(
            mode,
            scene,
            plane,
            ints(&case["player"]),
            &Camera {
                follow: case["follow"].as_bool().unwrap(),
                position: ints(&case["camera"]),
                pitch: int(&case["pitch"]),
            },
        );
        let context = format!("{name} step {steps}: {case}");
        let marks = roofs
            .marks()
            .map_or("null".to_owned(), |m| fnv(m.iter().copied()));
        assert_eq!(marks, case["marks"].as_str().unwrap(), "{context}");
        let hidden: Vec<u32> = roofs
            .mask(mode, plane)
            .tiles()
            .into_iter()
            .map(|[level, x, y]| (level * 10816 + x * 104 + y) as u32)
            .collect();
        assert_eq!(
            hidden.len() as u64,
            case["hidden"].as_u64().unwrap(),
            "{context}"
        );
        assert_eq!(
            fnv(hidden.iter().flat_map(|n| n.to_le_bytes())),
            case["hiddenHash"].as_str().unwrap(),
            "{context}"
        );
        let g = &roofs.groups;
        let groups = [&g.max_height, &g.min_x, &g.max_x, &g.min_z, &g.max_z];
        for (native, original) in groups.into_iter().zip(case["groups"].as_array().unwrap()) {
            let original: Vec<i32> = original.as_array().unwrap().iter().map(int).collect();
            assert_eq!(*native, original, "{context}");
        }
        steps += 1;
        hiding += usize::from(!hidden.is_empty());
    }
    assert_eq!(scenes.len(), 4);
    assert!(steps >= 2500 && hiding >= 1600, "{steps} {hiding}");
}

/// A scene of flat ground with a bridge column at (10, 10) and a forced-ground
/// tile on plane 2 at (20, 20).
fn small() -> Scene {
    Scene::new(
        |plane, x, y| match (plane, x, y) {
            (1, 10, 10) => 2,
            (2, 20, 20) => 8,
            _ => 0,
        },
        |plane, _, _| -240 * plane as i32,
    )
}
/// What a selective fill hides when the player has stood, within one frame's
/// stamp, on each of these roofed tiles of an otherwise open plane 0: levels 1
/// to 3 above the tile and above the ring of open tiles around it, which have
/// no wall or roof edge to keep them.
fn mask(roofed: &[[usize; 2]]) -> Mask {
    let scene = Scene::new(
        |plane, x, y| u8::from(plane == 0 && roofed.contains(&[x, y])) * 4,
        |_, _, _| 0,
    );
    let mut roofs = Roofs::default();
    roofs.update_mode(Mode::Selective, &scene, 0);
    let overhead = Camera {
        follow: true,
        position: [0; 3],
        pitch: 383,
    };
    for &[x, y] in roofed {
        let player = [x as i32 * 128, y as i32 * 128];
        roofs.frame(Mode::Selective, &scene, 0, player, &overhead);
    }
    roofs.mask(Mode::Selective, 0)
}

#[test]
fn floors_follow_their_render_level_and_scenery_any_visible_tile() {
    let scene = small();
    let hidden = mask(&[[10, 10], [20, 20], [30, 30], [31, 30]]);
    for level in 1..4 {
        for x in 29..=32 {
            for y in 29..=31 {
                assert!(hidden.hidden(level, x, y), "{level} {x} {y}");
            }
        }
        assert!(!hidden.hidden(level, 28, 30) && !hidden.hidden(level, 33, 30));
        assert!(!hidden.hidden(level, 30, 28) && !hidden.hidden(level, 30, 32));
    }
    assert!(!hidden.hidden(0, 30, 30));

    // GlTile faces use SceneGraph's render level: the plane, one lower over a
    // bridge, and 0 for a forced-ground tile.
    assert!(scene.floor_hidden(&hidden, 1, [30, 30]));
    assert!(!scene.floor_hidden(&hidden, 0, [30, 30]));
    assert!(!scene.floor_hidden(&hidden, 1, [10, 10]), "the bridge deck");
    assert!(scene.floor_hidden(&hidden, 2, [10, 10]), "level 1 there");
    assert!(!scene.floor_hidden(&hidden, 2, [20, 20]), "forced ground");
    assert!(scene.floor_hidden(&hidden, 3, [20, 20]));
    assert!(!scene.floor_hidden(&hidden, 1, [104, 3]), "outside");

    // Locations stay in the tile array at their plane, one lower over a
    // bridge, forced-ground flag or not; the ground under a bridge is linked
    // to the deck's tile and never hidden.
    let hide = |plane, tile, shape, rotation, size| {
        scene.location_hidden(&hidden, plane, tile, shape, rotation, size)
    };
    assert!(hide(1, [30, 30], 0, 0, [1, 1]));
    assert!(!hide(0, [30, 30], 0, 0, [1, 1]));
    assert!(!hide(1, [10, 10], 22, 0, [1, 1]));
    assert!(!hide(0, [10, 10], 22, 0, [1, 1]));
    assert!(hide(2, [10, 10], 22, 0, [1, 1]));
    assert!(hide(2, [20, 20], 10, 0, [1, 1]));
    // Scenery is drawn from any visible tile under it; a wall or decoration
    // only with its own tile. Rotations 1 and 3 swap the footprint.
    assert!(hide(1, [30, 30], 10, 0, [3, 1]));
    assert!(!hide(1, [30, 30], 10, 0, [1, 3]), "covers (30, 32)");
    assert!(!hide(1, [30, 30], 10, 1, [3, 1]), "covers (30, 32)");
    assert!(hide(1, [30, 30], 10, 3, [1, 3]));
    assert!(!hide(1, [27, 30], 11, 0, [3, 1]), "covers (27, 30)");
    assert!(hide(1, [30, 30], 4, 1, [9, 9]), "a decoration is one tile");
    assert!(hide(1, [30, 30], 17, 0, [9, 9]), "and so is a roof piece");
    assert!(!hide(1, [103, 30], 10, 0, [2, 1]), "partly outside");
    assert!(!hide(4, [30, 30], 10, 0, [1, 1]));
}
#[test]
fn a_location_gets_an_entity_where_add_loc_finds_one() {
    // No shape list: only a centrepiece (10, or 11 drawn as 10) has a model.
    assert!(roof::has_entity(10, None, 1, false, false));
    assert!(roof::has_entity(11, None, 2, false, false));
    assert!(!roof::has_entity(10, None, 0, false, false));
    assert!(!roof::has_entity(0, None, 1, false, false));
    assert!(!roof::has_entity(22, None, 1, false, false));
    // A shape list: the requested shape must be in it.
    assert!(roof::has_entity(0, Some(&[0, 2]), 2, false, false));
    assert!(!roof::has_entity(1, Some(&[0, 2]), 2, false, false));
    assert!(roof::has_entity(11, Some(&[10]), 1, false, false));
    assert!(!roof::has_entity(11, Some(&[11]), 1, false, false));
    assert!(roof::has_entity(6, Some(&[4]), 1, false, false));
    assert!(roof::has_entity(17, Some(&[12, 17]), 2, false, false));
    // An animated, alternating or opcode-98 type is a Loc whatever its models.
    assert!(roof::has_entity(3, None, 0, true, false));
    // Opcode 82: HD places nothing.
    assert!(!roof::has_entity(10, None, 1, true, true));
}
