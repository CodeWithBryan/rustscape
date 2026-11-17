//! JVM differential for mouse picking and the scene drawing order.
//! `fixtures/picking/oracle.txt` is written by `cargo xtask picking-oracle`
//! from text copied unchanged out of GlModel and SceneGraph.
use rs09_client::{
    picking::{self, Bounds, Rule, View},
    scene_order::{Draw, Frustum, Ground, Scene},
};
use sha2::{Digest, Sha256};

const ORACLE: &str = include_str!("../../../fixtures/picking/oracle.txt");

fn lines(kind: &str) -> impl Iterator<Item = &'static str> + '_ {
    ORACLE
        .lines()
        .filter_map(move |line| line.strip_prefix(kind)?.strip_prefix(' '))
}

fn numbers(text: &str) -> Vec<i32> {
    text.split_ascii_whitespace()
        .map(|n| n.parse().unwrap())
        .collect()
}

#[test]
fn oracle_sources_remain_pinned() {
    let provenance: serde_json::Value =
        serde_json::from_str(include_str!("../../../fixtures/picking/provenance.json")).unwrap();
    assert_eq!(
        format!("{:x}", Sha256::digest(ORACLE.replace('\r', "").as_bytes())),
        provenance["fixtureSha256"].as_str().unwrap()
    );
    for (path, expected) in provenance["sources"].as_object().unwrap() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../{path}"));
        let text = std::fs::read_to_string(path).unwrap().replace('\r', "");
        assert_eq!(
            format!("{:x}", Sha256::digest(text.as_bytes())),
            expected.as_str().unwrap()
        );
    }
}

#[test]
fn sine_tables_match_math_utils() {
    for (kind, table) in [
        ("sin", picking::sin as fn(i32) -> i32),
        ("cos", picking::cos),
    ] {
        let expected = numbers(lines(kind).next().unwrap());
        assert_eq!(expected.len(), 2048);
        for (angle, value) in expected.into_iter().enumerate() {
            assert_eq!(table(angle as i32), value, "{kind} {angle}");
        }
    }
}

#[test]
fn location_keys_match_add_loc() {
    let mut cases = 0;
    for line in lines("key") {
        let (fields, key) = line.rsplit_once(' ').unwrap();
        let f = numbers(fields);
        let actual = picking::location_key(
            [f[0], f[1]],
            f[2],
            f[3],
            f[4],
            f[5] != 0,
            f[6] == 1,
            f[7] != 0,
        );
        assert_eq!(format!("{actual:x}"), key, "{line}");
        cases += 1;
    }
    assert_eq!(cases, 1280);
}

#[test]
fn hover_matches_gl_model_render() {
    let (mut cases, mut hits) = (0, 0);
    for line in lines("hover") {
        let f = numbers(line);
        let mut view = View::new([0; 3], f[0], f[1]);
        view.lower = [f[2], f[3]];
        view.upper = [f[4], f[5]];
        view.pick = (f[8] != 0).then_some([f[6], f[7]]);
        let (positive_key, yaw, at, boxed) = (f[9] != 0, f[10], [f[11], f[12], f[13]], f[14] != 0);
        let count = f[15] as usize;
        let vertices: Vec<[i32; 3]> = f[16..16 + count * 3]
            .chunks_exact(3)
            .map(|v| [v[0], v[1], v[2]])
            .collect();
        let rest = &f[16 + count * 3..];
        let faces: Vec<[usize; 3]> = rest[1..1 + rest[0] as usize * 3]
            .chunks_exact(3)
            .map(|v| [v[0] as usize, v[1] as usize, v[2] as usize])
            .collect();
        let expected = &rest[1 + faces.len() * 3..];
        let bounds = Bounds::of(&vertices);
        assert_eq!(
            [
                bounds.min.map(i32::from).to_vec(),
                bounds.max.map(i32::from).to_vec(),
                vec![i32::from(bounds.radius)]
            ]
            .concat(),
            expected[1..],
            "{line}"
        );
        let rule = if boxed { Rule::Box } else { Rule::Triangles };
        let hit =
            positive_key && picking::hover(&view, &bounds, &vertices, Some(&faces), yaw, at, rule);
        assert_eq!(i32::from(hit), expected[0], "{line}");
        // The bounds-only part never rejects a model the whole test accepts,
        // nor do bounds with a larger cylinder.
        if hit {
            assert!(picking::near(&view, &bounds, at, false));
            let loose = Bounds {
                radius: bounds.radius.saturating_add(40),
                ..bounds
            };
            if (128..384).contains(&f[0]) {
                assert!(picking::near(&view, &loose, at, true), "{line}");
            }
        }
        // A triangle list read three vertices at a time gives the same answer.
        let soup: Vec<[i32; 3]> = faces.iter().flatten().map(|&i| vertices[i]).collect();
        if !boxed && soup.len() == vertices.len() * 3 {
            let bounds = Bounds::of(&soup);
            let _ = picking::hover(&view, &bounds, &soup, None, yaw, at, rule);
        }
        cases += 1;
        hits += expected[0];
    }
    assert_eq!(cases, 6000);
    assert!(hits > 1500, "{hits}");
}

#[test]
fn walk_test_matches_draw_plain_tile() {
    let (mut cases, mut hits) = (0, 0);
    for line in lines("tile") {
        let f = numbers(line);
        let view = View::new([f[2], f[3], f[4]], f[0], f[1]);
        let hit = picking::plain_tile(
            &view,
            [f[11], f[12]],
            [f[5], f[6]],
            [f[7], f[8], f[9], f[10]],
        );
        assert_eq!(i32::from(hit), f[13], "{line}");
        cases += 1;
        hits += f[13];
    }
    assert_eq!(cases, 3000);
    assert!(hits > 500, "{hits}");
}

fn mix(seed: i32, level: i32, x: i32, y: i32) -> u32 {
    let mut h =
        x.wrapping_mul(73856093) ^ y.wrapping_mul(19349663) ^ level.wrapping_mul(83492791) ^ seed;
    h ^= ((h as u32) >> 15) as i32;
    h = h.wrapping_mul(0x2c1b3c6d);
    h ^= ((h as u32) >> 12) as i32;
    (h as u32) >> 8
}

#[test]
fn frustum_matches_set_plain_tile() {
    let mut cases = 0;
    for line in lines("frustum") {
        let (fields, map) = line.rsplit_once(' ').unwrap();
        let f = numbers(fields);
        let mut view = View::new([f[2], f[3], f[4]], f[0], f[1]);
        view.lower = [f[5], f[6]];
        view.upper = [f[7], f[8]];
        let (visibility, seed) = (f[9], f[10]);
        let frustum = Frustum::new(
            &view,
            visibility,
            |x, y| -((mix(seed, 0, x, y) % 600) as i32),
            |x, y| -720 - (mix(seed, 3, x, y) % 200) as i32,
        );
        let tile = [f[2], f[4]].map(|n| n.clamp(0, 104 * 128 - 1) / 128);
        let width = visibility * 2 + 1;
        let actual: String = (0..width * width)
            .map(|i| {
                let (x, y) = (
                    tile[0] + i / width - visibility,
                    tile[1] + i % width - visibility,
                );
                if frustum.contains(x, y) { '1' } else { '0' }
            })
            .collect();
        assert_eq!(actual, map, "{fields}");
        cases += 1;
    }
    assert_eq!(cases, 120);
}

#[test]
fn drawing_order_matches_render_tile() {
    let part = |n: i32| (n >= 0).then_some(n as u32);
    let (mut cases, mut frames, mut drawn) = (0, 0, 0);
    for (case, line) in lines("scene").enumerate() {
        let mut scene = Scene::new();
        for op in line.split(';') {
            let (kind, rest) = op.split_once(' ').unwrap_or((op, ""));
            let (fields, expected) = rest.split_once(':').unwrap_or((rest, ""));
            let f = numbers(fields);
            let level = f.first().copied().unwrap_or(0) as usize;
            match kind {
                "B" => scene.fill_level(0),
                "G" => scene.set_ground(
                    level,
                    f[1],
                    f[2],
                    match f[3] {
                        0 => Ground::Plain { coloured: true },
                        1 => Ground::Plain { coloured: false },
                        _ => Ground::Shaped,
                    },
                ),
                "W" => scene.set_wall(level, f[1], f[2], f[3] as u32, part(f[4]), f[5], f[6]),
                "D" => scene.set_wall_decor(level, f[1], f[2], f[3] as u32, part(f[4]), f[5], f[6]),
                "F" => scene.set_ground_decor(level, f[1], f[2], f[3] as u32, f[4] != 0),
                "O" => scene.set_obj_stack(
                    level,
                    f[1],
                    f[2],
                    [part(f[3]), part(f[4]), part(f[5])],
                    f[6] != 0,
                ),
                "S" => assert_eq!(
                    scene.add_scenery(
                        level,
                        [f[1], f[2]],
                        [f[3], f[4]],
                        [f[5], f[6]],
                        f[7] as u32,
                        f[8] != 0,
                        f[9] != 0
                    ),
                    f[10] != 0,
                    "case {case}: {op}"
                ),
                "A" => assert_eq!(
                    scene.add_actor(level, [f[1], f[2]], f[3], f[4] as u32, f[5], f[6] != 0),
                    f[7] != 0,
                    "case {case}: {op}"
                ),
                "R" => scene.bridge(f[0], f[1]),
                "C" => scene.clear_temporary(),
                "X" => {
                    let (seed, percent) = (f[4], f[5] as u32);
                    let mut actual = Vec::new();
                    scene.draw(
                        [f[0], f[1]],
                        f[2],
                        |level, x, y| mix(seed, level as i32, x, y) % 100 >= percent,
                        (f[3] >= 0).then_some(f[3] as usize),
                        |draw| {
                            actual.push(match draw {
                                Draw::Ground { level, tile } => {
                                    format!("g{level},{},{}", tile[0], tile[1])
                                }
                                Draw::Model(part) => format!("m{part}"),
                            })
                        },
                    );
                    drawn += actual.len();
                    assert_eq!(actual.join(" "), expected, "case {case}: {fields}");
                    frames += 1;
                }
                other => panic!("unknown operation {other}"),
            }
        }
        cases += 1;
    }
    assert_eq!((cases, frames), (700, 1400));
    assert!(drawn > 80_000, "{drawn}");
}

/// An upright box of triangles standing on its origin.
fn box_model(half: i32, height: i32) -> Vec<[i32; 3]> {
    let corner = |i: usize| {
        [
            if i & 1 == 0 { -half } else { half },
            if i & 2 == 0 { 0 } else { -height },
            if i & 4 == 0 { -half } else { half },
        ]
    };
    [
        [0, 1, 3],
        [0, 3, 2],
        [4, 6, 7],
        [4, 7, 5],
        [0, 4, 5],
        [0, 5, 1],
        [2, 3, 7],
        [2, 7, 6],
        [0, 2, 6],
        [0, 6, 4],
        [1, 5, 7],
        [1, 7, 3],
    ]
    .into_iter()
    .flatten()
    .map(corner)
    .collect()
}

/// A staged scene through the whole pipeline (no JVM): two objects in line
/// with the mouse and one on the floor above. Everything under the mouse is
/// listed, far before near; consecutive repeats of a key are dropped; and
/// `SceneGraph.isLocValid` offers only what stands on the player's level.
#[test]
fn staged_scene_lists_everything_under_the_mouse_in_drawing_order() {
    let model = box_model(50, 300);
    let bounds = Bounds::of(&model);
    // (level, tile, height of the floor it stands on)
    let objects = [(0usize, [50, 54], 0), (0, [50, 55], 0), (1, [50, 55], -480)];
    let keys: Vec<i64> = objects
        .iter()
        .enumerate()
        .map(|(id, (_, tile, _))| {
            picking::location_key(*tile, 10, 0, 100 + id as i32, true, false, false)
        })
        .collect();
    let mut scene = Scene::new();
    for x in 0..104 {
        for y in 0..104 {
            scene.set_ground(0, x, y, Ground::Plain { coloured: true });
        }
    }
    scene.set_ground(1, 50, 55, Ground::Plain { coloured: true });
    let centre = |tile: [i32; 2]| [tile[0] * 128 + 64, tile[1] * 128 + 64];
    for (part, (level, tile, _)) in objects.iter().enumerate() {
        assert!(scene.add_scenery(
            *level,
            *tile,
            [1, 1],
            centre(*tile),
            part as u32,
            true,
            false
        ));
    }
    let mut view = View::new([50 * 128 + 64, -1500, 50 * 128 + 64 - 500], 256, 0);
    view.lower = [-256, -167];
    view.upper = [256, 167];
    let position = |part: usize| {
        let [x, y] = centre(objects[part].1);
        [x, objects[part].2, y]
    };
    let aim = |view: &View, point: [i32; 3]| {
        let [x, y, depth] = view.rotate(std::array::from_fn(|i| point[i] - view.eye[i]));
        Some([(x << 9) / depth, (y << 9) / depth])
    };
    let picked = |scene: &mut Scene, view: &View, top: usize, player: usize| {
        let mut hits = Vec::new();
        scene.draw(
            [view.eye[0], view.eye[2]],
            25,
            |level, _, _| level <= top,
            Some(player),
            |draw| {
                if let Draw::Model(part) = draw {
                    let part = part as usize;
                    if picking::hover(
                        view,
                        &bounds,
                        &model,
                        None,
                        0,
                        position(part),
                        Rule::Triangles,
                    ) {
                        // A model drawn in two pieces reports its key twice in a row.
                        hits.extend([keys[part], keys[part]]);
                    }
                }
            },
        );
        hits.dedup();
        let valid = |key: &i64| {
            let [x, y] = [(key & 0x7f) as i32, (key >> 7 & 0x7f) as i32];
            scene
                .parts_at(player, x, y)
                .into_iter()
                .any(|part| keys[part as usize] == *key)
        };
        (
            hits.clone(),
            hits.into_iter().filter(valid).collect::<Vec<_>>(),
        )
    };
    // Through the top of the near box: the ray goes on into the far one.
    let near_top = [50 * 128 + 64, -280, 54 * 128 + 64];
    view.pick = aim(&view, near_top);
    let (hits, offered) = picked(&mut scene, &view, 0, 0);
    assert_eq!(hits, [keys[1], keys[0]]);
    assert_eq!(offered, [keys[1], keys[0]]);
    // At the box on the upper floor, with that floor drawn.
    view.pick = aim(&view, [50 * 128 + 64, -480 - 200, 55 * 128 + 64]);
    let (hits, offered) = picked(&mut scene, &view, 1, 0);
    assert!(hits.contains(&keys[2]), "{hits:x?}");
    // From the ground floor it is not a valid target; from its own it is,
    // and what is under it on the ground floor is then not.
    assert!(!offered.contains(&keys[2]));
    let (hits, offered) = picked(&mut scene, &view, 1, 1);
    assert_eq!(offered, [keys[2]], "{hits:x?}");
    // With the upper floor hidden (roof removal) its box is not drawn.
    let (hits, _) = picked(&mut scene, &view, 0, 1);
    assert!(!hits.contains(&keys[2]));
    // Outside the viewport nothing is picked.
    view.pick = None;
    assert!(picked(&mut scene, &view, 1, 0).0.is_empty());
}
