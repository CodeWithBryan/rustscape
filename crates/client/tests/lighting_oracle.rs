//! JVM differential for point-light selection, light meshes and floor
//! shadows: `fixtures/lighting/oracle.txt` holds what the unchanged RT4
//! `LightingManager`, `LightMesh` and `ShadowManager`, and `SceneGraph`'s
//! verbatim `buildGlLighting`/`applyLightToTile`, produced for seeded inputs
//! (`cargo xtask lighting-oracle`). The inputs are regenerated here from the
//! same seeds, so `Rng` and the generators below mirror
//! `oracle/lighting/LightingOracle.java` draw for draw.
use rs09_client::{
    light_mesh::{self, Level, Source},
    light_select::{Event, Selector},
    lighting::{self, Flicker, Light},
    shadow::{Map, Sprite},
    shadow_rules::{self, FloorSprites, FloorTile},
};
use sha2::{Digest, Sha256};

/// SHA-256 (CR removed) of the reference files this work transcribes from
/// without executing them through the oracle, and of those the oracle runs.
const SOURCES: [(&str, &str); 6] = [
    (
        "SceneGraph.java",
        "eec94c9c65b1044f10f9e3c0cf9bc06c37f7d398f3ac8488bba5f68c26340166",
    ),
    (
        "LightingManager.java",
        "0f3112324d6e7e220c799cdc9fd49731963f7fad1be4dc43b6d7a9ee293cc1f9",
    ),
    (
        "LightMesh.java",
        "b596bd8a502248e36067e61d3a68a893dc5aabd08378f721ad6ff745b674ca21",
    ),
    (
        "ShadowManager.java",
        "45c51ce9bc09a12b90c193f9498ddf3ce319abde12272bd4ce2c67c773ddff3f",
    ),
    (
        "Loc.java",
        "0fcfb4d6387982a745305cd317566bc0098bc464111ead82799d01a74023555f",
    ),
    (
        "FogManager.java",
        "3f088f20fd85e91c51749cba00b8adcca4b2dfb1119fc480a38eff447ebee180",
    ),
];

#[test]
fn transcribed_reference_sources_are_the_pinned_ones() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../reference/client/client/src/main/java/rt4");
    for (name, expected) in SOURCES {
        let text = String::from_utf8(std::fs::read(root.join(name)).unwrap())
            .unwrap()
            .replace("\r\n", "\n");
        assert_eq!(
            format!("{:x}", Sha256::digest(text.as_bytes())),
            expected,
            "{name} changed: review the lighting and shadow transcriptions"
        );
    }
}

struct Rng(i32);
impl Rng {
    fn next(&mut self, bound: i32) -> i32 {
        self.0 = self.0.wrapping_mul(1103515245).wrapping_add(12345);
        ((self.0 as u32 >> 16) & 0x7fff) as i32 % bound
    }
}

fn fixture(kind: &str) -> Vec<String> {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/lighting/oracle.txt");
    let text = std::fs::read_to_string(path).unwrap();
    let prefix = format!("{kind} ");
    text.lines()
        .filter_map(|line| line.strip_prefix(&prefix))
        .map(str::to_owned)
        .collect()
}

fn light(rng: &mut Rng, origin: i32, extent: i32) -> Light {
    let level = rng.next(4) as usize;
    let extends_down = rng.next(4) == 0;
    let extends_up = rng.next(4) == 0;
    let radius = rng.next(5);
    let x = (origin + rng.next(extent)) * 128 + rng.next(128);
    let y = (origin + rng.next(extent)) * 128 + rng.next(128);
    let z = rng.next(600) - 300;
    let mut spans = Vec::new();
    for _ in 0..radius * 2 + 1 {
        let start = if rng.next(8) == 0 {
            -1
        } else if rng.next(3) == 0 {
            rng.next(radius * 2 + 1)
        } else {
            0
        };
        let length = if rng.next(3) == 0 {
            rng.next(radius * 2 + 2)
        } else {
            radius * 2 + 1 - start.max(0)
        };
        spans.push((start << 8 | length) as i16);
    }
    let color = (rng.next(4096) << 12 | rng.next(4096)) as u32;
    Light {
        level,
        extends_down,
        extends_up,
        coordinates: [x, y, z],
        radius,
        spans,
        color,
        preset: 0,
        phase: 0,
        flicker: Flicker::default(),
        quadratic_attenuation: 0.0,
    }
}

fn near(rng: &mut Rng, light: &Light, fine: i32) -> i32 {
    ((fine >> 7) + rng.next(light.radius * 2 + 3) - light.radius - 1).clamp(0, 103)
}

#[test]
fn light_selection_matches_the_original_lighting_manager() {
    let cases = fixture("select");
    assert_eq!(cases.len(), 40);
    let mut operations = 0;
    let mut changes = 0;
    for case in cases {
        let (seed, expected) = case.split_once(" |").unwrap();
        let mut rng = Rng(seed.parse().unwrap());
        let count = 1 + rng.next(60);
        let lights: Vec<Light> = (0..count)
            .map(|_| {
                if rng.next(3) == 0 {
                    light(&mut rng, -3, 110)
                } else {
                    light(&mut rng, 40, 16)
                }
            })
            .collect();
        let cells = lighting::grid(104, 104, &lights);
        let grid = |plane: i32, x: i32, y: i32| {
            u32::from_le_bytes(cells[((plane * 104 + y) * 104 + x) as usize])
        };
        let mut selector = Selector::new(104, 104);
        let mut last = [0; 3];
        for (step, expected) in expected.split(" |").enumerate() {
            let kind = rng.next(10);
            let target = &lights[rng.next(count) as usize];
            let plane = rng.next(4);
            let x = near(&mut rng, target, target.coordinates[0]);
            let y = near(&mut rng, target, target.coordinates[1]);
            match kind {
                0..=3 => {
                    last = [plane, x, y];
                    selector.tile(&grid, plane, x, y);
                }
                4..=5 => {
                    let end_x = (x + rng.next(4)).min(103);
                    let end_y = (y + rng.next(4)).min(103);
                    selector.region(&grid, plane, [x, y], [end_x, end_y]);
                }
                6 => {
                    let flags = if rng.next(10) == 0 {
                        3
                    } else {
                        1 << rng.next(8)
                    };
                    selector.wall_piece(&grid, flags, plane, x, y);
                }
                7 => {
                    let x2 = near(&mut rng, target, target.coordinates[0]);
                    let y2 = near(&mut rng, target, target.coordinates[1]);
                    selector.prune(&grid, plane, [x, y], [x2, y2]);
                }
                8 => {
                    if rng.next(4) == 0 {
                        selector.reset();
                    } else {
                        selector.invalidate();
                    }
                }
                _ => selector.tile(&grid, last[0], last[1], last[2]),
            }
            let mut actual = String::new();
            for event in selector.log.drain(..) {
                changes += 1;
                actual.push_str(&match event {
                    Event::Enable(slot) => format!(" E{slot}"),
                    Event::Disable(slot) => format!(" D{slot}"),
                    Event::Set(slot, light) => format!(" S{slot}:{light}"),
                });
            }
            let [a, b, c, d] = selector.active;
            actual.push_str(&format!(" ; {a},{b},{c},{d}"));
            assert_eq!(actual, expected, "seed {seed} step {step} kind {kind}");
            operations += 1;
        }
    }
    assert_eq!(operations, 40 * 150);
    // The inputs must actually exercise the slots.
    assert!(changes > 4000, "{changes} light changes");
}

fn mix(hash: &mut u64, value: i32) {
    for byte in value.to_le_bytes() {
        *hash ^= u64::from(byte);
        *hash = hash.wrapping_mul(0x100000001b3);
    }
}

#[test]
fn light_meshes_match_the_original_scene_builder() {
    let cases = fixture("mesh");
    let dumps = fixture("meshdump");
    assert_eq!(cases.len(), 24);
    let (mut meshes, mut lit) = (0, 0);
    for case in cases {
        let mut fields = case.split(' ');
        let seed: i32 = fields.next().unwrap().parse().unwrap();
        let mut rng = Rng(seed);
        let level = rng.next(4) as usize;
        let mut heights = vec![0; 105 * 105];
        for height in &mut heights {
            *height = -rng.next(20) * 8;
        }
        let (mut underlays, mut overlays) = (vec![0u8; 104 * 104], vec![0u8; 104 * 104]);
        let (mut shapes, mut rotations) = (vec![0u8; 104 * 104], vec![0u8; 104 * 104]);
        // The first scenery whose south-west tile is the tile itself.
        let mut scenery = vec![None; 104 * 104];
        for tile in 0..104 * 104 {
            underlays[tile] = rng.next(4) as u8;
            let overlay = rng.next(5);
            overlays[tile] = if overlay < 3 { 0 } else { overlay as u8 - 2 };
            shapes[tile] = rng.next(12) as u8;
            rotations[tile] = rng.next(4) as u8;
            if rng.next(12) == 0 {
                if rng.next(6) == 0 {
                    // One anchored on the tile to the west: not this tile's.
                    rng.next(4);
                }
                let shape = if rng.next(4) == 0 { 10 } else { 9 };
                scenery[tile] = Some((shape, rng.next(4)));
            }
        }
        let mut lights = Vec::new();
        for _ in 0..16 {
            let mut light = light(&mut rng, -3, 110);
            if rng.next(4) != 0 {
                light.level = level;
            }
            if rng.next(3) != 0 {
                light.coordinates[1] = -rng.next(300) - 1;
                light.coordinates[2] = rng.next(500);
            }
            lights.push(light);
        }
        let sources: Vec<Source<'_>> = lights
            .iter()
            .map(|light| Source {
                level: light.level,
                tile: [light.coordinates[0] >> 7, light.coordinates[1] >> 7],
                position: light.coordinates,
                radius: light.radius,
                spans: &light.spans,
                color: light.color,
            })
            .collect();
        let built = light_mesh::build(
            level,
            &Level {
                size: [104, 104],
                underlays: &underlays,
                overlays: &overlays,
                shapes: &shapes,
                rotations: &rotations,
                heights: &heights,
            },
            &sources,
            |overlay| overlay == 2,
            |x, y| match scenery[(x * 104 + y) as usize] {
                Some((9, rotation)) => Some(rotation),
                _ => None,
            },
        );
        for (index, (mesh, expected)) in built.iter().zip(fields).enumerate() {
            let Some(mesh) = mesh else {
                assert_eq!(expected, "-", "seed {seed} light {index}");
                continue;
            };
            let mut hash = 0xcbf29ce484222325u64;
            let mut full = String::new();
            for (color, position) in mesh.colors.iter().zip(&mesh.positions) {
                for value in color.map(i32::from).into_iter().chain(*position) {
                    mix(&mut hash, value);
                    full.push_str(&format!(" {value}"));
                }
                lit += usize::from(*color != [0; 3]);
            }
            full.push_str(" /");
            for &index in &mesh.indices {
                mix(&mut hash, index as i32);
                full.push_str(&format!(" {index}"));
            }
            if seed % 8 == 0 {
                let prefix = format!("{seed} {index}");
                let dump = dumps
                    .iter()
                    .find_map(|dump| dump.strip_prefix(&prefix))
                    .unwrap();
                assert_eq!(full, dump, "seed {seed} light {index}");
            }
            assert_eq!(
                format!("{}:{}:{hash:x}", mesh.positions.len(), mesh.indices.len()),
                expected,
                "seed {seed} light {index}"
            );
            meshes += 1;
        }
    }
    // The inputs must include meshes whose vertices the lights reach.
    assert!(meshes > 60, "{meshes} meshes");
    assert!(lit > 300, "{lit} lit vertices");
}

#[test]
fn floor_shadows_and_the_shadow_test_match_the_original_shadow_manager() {
    let cases = fixture("shadow");
    assert_eq!(cases.len(), 12);
    let (mut open, mut covered_tests) = (0, 0);
    for case in cases {
        let fields: Vec<&str> = case.split(' ').collect();
        let mut rng = Rng(fields[0].parse().unwrap());
        let light = [rng.next(600) - 300, rng.next(600) - 300];
        let sprites = FloorSprites {
            size: [16, 16],
            masks: (0..13)
                .map(|_| (0..256).map(|_| rng.next(2) as u8).collect())
                .collect(),
        };
        let mut map = Map::new(13, 13);
        for _ in 0..400 {
            let shape = rng.next(13) as usize;
            let rotation = rng.next(4);
            let overlay_open = rng.next(2) == 0;
            let underlay_open = rng.next(2) == 0;
            let tile = [rng.next(104), rng.next(104)];
            let base = -rng.next(700);
            let heights = std::array::from_fn(|_| base - rng.next(16));
            if let Some((sprite, position)) = shadow_rules::floor(
                &sprites,
                light,
                [map.width, map.height],
                FloorTile {
                    shape,
                    rotation,
                    overlay_open,
                    underlay_open,
                    tile,
                    heights,
                },
            ) {
                map.stamp(&sprite, position, false);
            }
        }
        let mut hash = 0xcbf29ce484222325u64;
        for &pixel in &map.pixels {
            mix(&mut hash, i32::from(pixel));
        }
        let covered = map.pixels.iter().filter(|&&pixel| pixel != 0).count();
        assert_eq!(
            (covered.to_string().as_str(), format!("{hash:x}").as_str()),
            (fields[1], fields[2]),
            "seed {}",
            fields[0]
        );
        for (index, expected) in fields[3].chars().enumerate() {
            let sprite = Sprite {
                width: 1 + rng.next(40) as usize,
                height: 1 + rng.next(40) as usize,
                offset: [rng.next(9) - 4, rng.next(9) - 4],
                pixels: vec![],
            };
            let fine = [rng.next(104 * 128), rng.next(104 * 128)];
            let height = -rng.next(700);
            let position: [i32; 2] =
                std::array::from_fn(|i| (fine[i] - ((height * light[i]) >> 8)) >> 3);
            let actual = shadow_rules::open_to_light(&map, &sprite, position);
            // 'X' is an original read past the map; there is nothing to match.
            if expected != 'X' {
                assert_eq!(actual, expected == 'T', "seed {} test {index}", fields[0]);
            }
            open += usize::from(expected == 'T');
            covered_tests += usize::from(expected == 'F');
        }
    }
    assert!(
        open > 100 && covered_tests > 20,
        "{open} open, {covered_tests} covered"
    );
}
