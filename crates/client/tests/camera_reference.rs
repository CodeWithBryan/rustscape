//! JVM differential for the follow camera, the viewport and the projection:
//! `fixtures/camera/oracle.jsonl` holds what the unchanged `rt4.Camera` (with
//! the copied pieces listed in `fixtures/camera/provenance.json`) produced
//! for seeded inputs; every case is replayed here. Regenerate with
//! `cargo xtask camera-oracle`.
use rs09_client::{
    camera::Pose,
    follow_camera::{Follow, Held, Occlusion, Scene},
    ground_geometry::tile_height,
    projection,
    terrain::Region,
    ui::vm::Viewport,
};
use serde_json::Value;
use sha2::{Digest, Sha256};

fn cases(kind: &str) -> Vec<Value> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/camera/oracle.jsonl"
    );
    let text = std::fs::read_to_string(path).expect("fixtures/camera/oracle.jsonl");
    let cases: Vec<Value> = text
        .lines()
        .filter(|line| !line.is_empty())
        .map(|line| serde_json::from_str(line).unwrap())
        .filter(|case: &Value| case["case"] == kind)
        .collect();
    assert!(!cases.is_empty(), "no {kind} cases");
    cases
}
fn int(value: &Value) -> i32 {
    i32::try_from(value.as_i64().unwrap()).unwrap()
}
fn ints(value: &Value) -> Vec<i32> {
    value.as_array().unwrap().iter().map(int).collect()
}
fn double(value: &Value) -> f64 {
    f64::from_bits(value.as_i64().unwrap() as u64)
}

/// `CameraOracle.mix`.
fn mix(seed: i32, a: i32, b: i32, c: i32) -> i32 {
    let shift = |h: i32, by: u32| ((h as u32) >> by) as i32;
    let mut h = seed ^ a.wrapping_mul(0x9E37_79B1_u32 as i32);
    h = (h ^ shift(h, 15))
        .wrapping_mul(0x85EB_CA6B_u32 as i32)
        .wrapping_add(b.wrapping_mul(0xC2B2_AE35_u32 as i32));
    h ^= shift(h, 13);
    h = h
        .wrapping_mul(0x27D4_EB2F)
        .wrapping_add(c.wrapping_mul(0x1656_67B1));
    h ^ shift(h, 16)
}

/// `CameraOracle.scene`: the synthetic `SceneGraph` arrays.
struct Synthetic {
    heights: Vec<i32>,
    flags: Vec<u8>,
    occlusion: Vec<u8>,
}
impl Synthetic {
    fn new(seed: i32, mask: i32, occlusion: bool) -> Self {
        let mut out = Self {
            heights: vec![0; 4 * 105 * 105],
            flags: vec![0; 4 * 104 * 104],
            occlusion: vec![0; 4 * 104 * 104],
        };
        for level in 0..4 {
            for x in 0..105 {
                for y in 0..105 {
                    out.heights[(level * 105 * 105 + x * 105 + y) as usize] =
                        -(mix(seed, level, x, y) & mask) - level * 240;
                    if x < 104 && y < 104 {
                        let at = (level * 104 * 104 + x * 104 + y) as usize;
                        if level == 1 && (mix(seed ^ 0x55, 1, x, y) & 0x7fff_ffff) % 11 == 0 {
                            out.flags[at] = 2;
                        }
                        if occlusion {
                            out.occlusion[at] = mix(seed ^ 0xaa, level, x >> 2, y >> 2) as u8;
                        }
                    }
                }
            }
        }
        out
    }
    fn inside(level: i32, x: i32, y: i32, size: i32) -> Option<usize> {
        ((0..4).contains(&level) && (0..size).contains(&x) && (0..size).contains(&y))
            .then(|| (level * size * size + x * size + y) as usize)
    }
}
impl Scene for Synthetic {
    fn ground(&self, plane: i32, fine: [i32; 2]) -> i32 {
        // The interpolation the client's own scene uses.
        tile_height(plane, fine, |level, x, y| {
            (self.height(level, x, y), self.flags(level, x, y))
        })
    }
    fn flags(&self, level: i32, x: i32, y: i32) -> u8 {
        Self::inside(level, x, y, 104).map_or(0, |at| self.flags[at])
    }
    fn height(&self, level: i32, x: i32, y: i32) -> i32 {
        Self::inside(level, x, y, 105).map_or(0, |at| self.heights[at])
    }
    fn occlusion(&self, level: i32, x: i32, y: i32) -> u8 {
        Self::inside(level, x, y, 104).map_or(0, |at| self.occlusion[at])
    }
}

fn state(camera: &Follow) -> Vec<i64> {
    vec![
        camera.pitch_target.to_bits() as i64,
        camera.yaw_target.to_bits() as i64,
        camera.position[0].into(),
        camera.position[1].into(),
        camera.max_pitch_distance.into(),
        camera.offset[0].into(),
        camera.offset[1].into(),
        camera.offset_delta[0].into(),
        camera.offset_delta[1].into(),
        camera.yaw_drift.into(),
        camera.yaw_drift_delta.into(),
        camera.jitter_timer.into(),
    ]
}
fn longs(value: &Value) -> Vec<i64> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n.as_i64().unwrap())
        .collect()
}

#[test]
fn follow_camera_matches_the_unchanged_java_step_for_step() {
    let (mut steps, mut renders) = (0, 0);
    for case in cases("follow") {
        let scene = Synthetic::new(
            int(&case["seed"]),
            int(&case["mask"]),
            case["occlusion"].as_bool().unwrap(),
        );
        let start = &case["start"];
        let mut camera = Follow {
            pitch_target: double(&start[0]),
            yaw_target: double(&start[1]),
            position: [int(&start[2]), int(&start[3])],
            max_pitch_distance: int(&start[4]),
            offset: [int(&start[5]), int(&start[6])],
            offset_delta: [int(&start[7]), int(&start[8])],
            yaw_drift: int(&start[9]),
            yaw_drift_delta: int(&start[10]),
            jitter_timer: int(&start[11]),
            zoom: int(&case["zoom"]),
        };
        let mut plane = int(&case["plane"]);
        let mut player = [int(&case["player"][0]), int(&case["player"][1])];
        for (index, step) in case["steps"].as_array().unwrap().iter().enumerate() {
            let at = format!("case {} step {index} {}", case["index"], step["op"]);
            match step["op"].as_str().unwrap() {
                "cycle" => {
                    player = [int(&step["player"][0]), int(&step["player"][1])];
                    let keys = ints(&step["keys"]);
                    let typed = step["keyboard"].as_bool().unwrap().then_some(&keys[..]);
                    camera.cycle(player, plane, typed, &scene);
                }
                "keys" => {
                    let held = step["held"].as_array().unwrap();
                    let held = Held {
                        up: held[0].as_bool().unwrap(),
                        down: held[1].as_bool().unwrap(),
                        left: held[2].as_bool().unwrap(),
                        right: held[3].as_bool().unwrap(),
                    };
                    camera.keys(step["delta"].as_i64().unwrap(), held, plane, &scene);
                }
                "angles" => camera.set_angles(
                    f64::from(int(&step["pitch"])),
                    f64::from(int(&step["yaw"])),
                    plane,
                    &scene,
                ),
                "drift" => {
                    let random = double(&step["random"]);
                    camera.drift(|| random);
                }
                "plane" => plane = int(&step["plane"]),
                "render" => {
                    let floor = step["shake"]
                        .as_bool()
                        .unwrap()
                        .then(|| int(&step["amplitude"]) + 128);
                    let distance = ints(&step["distance"]);
                    let pose = camera.pose(
                        int(&step["height"]),
                        [distance[0], distance[1]],
                        player,
                        plane,
                        floor,
                        &scene,
                    );
                    let expected = ints(&step["pose"]);
                    assert_eq!(
                        [
                            pose.position[0],
                            pose.position[1],
                            pose.position[2],
                            pose.pitch,
                            pose.yaw
                        ],
                        expected[..],
                        "{at}"
                    );
                    renders += 1;
                }
                other => panic!("unknown step {other}"),
            }
            assert_eq!(state(&camera), longs(&step["state"]), "{at}");
            steps += 1;
        }
    }
    assert!(
        steps > 5000 && renders > 500,
        "{steps} steps, {renders} renders"
    );
}

#[test]
fn viewport_bounds_and_frustum_edges_match_the_java() {
    let (mut barred, mut compared) = (0, 0);
    for case in cases("viewport") {
        let rect = ints(&case["rect"]);
        let limits = ints(&case["limits"]);
        let limits = Viewport {
            near_zoom: limits[0],
            far_zoom: limits[1],
            min_zoom: limits[2],
            max_zoom: limits[3],
            min_fov: limits[4],
            max_fov: limits[5],
            ..Viewport::default()
        };
        // The Rust never panics where the original divides by zero.
        let bounds = projection::bounds(rect[0], rect[1], rect[2], rect[3], &limits);
        if case["throws"] == true {
            continue;
        }
        let at = format!("case {}", case["index"]);
        assert_eq!(
            vec![
                bounds.left,
                bounds.top,
                bounds.width,
                bounds.height,
                bounds.zoom
            ],
            ints(&case["bounds"]),
            "{at}"
        );
        let bars: Vec<Vec<i32>> = case["bars"].as_array().unwrap().iter().map(ints).collect();
        assert_eq!(
            bounds.bars.iter().map(|b| b.to_vec()).collect::<Vec<_>>(),
            bars,
            "{at}"
        );
        barred += usize::from(!bars.is_empty());
        let edges = bounds
            .edges()
            .map(|e| vec![e.left, e.right, e.top, e.bottom]);
        let expected = (!case["edges"].is_null()).then(|| ints(&case["edges"]));
        assert_eq!(edges, expected, "{at}");
        compared += 1;
    }
    assert!(
        compared > 300 && barred > 20,
        "{compared} cases, {barred} with bars"
    );
}

#[test]
fn project_to_screen_matches_the_java() {
    let (mut visible, mut hidden) = (0, 0);
    for case in cases("project") {
        let scene = Synthetic::new(int(&case["seed"]), int(&case["mask"]), false);
        let plane = int(&case["plane"]);
        let view = ints(&case["view"]);
        let pose = ints(&case["pose"]);
        let pose = Pose {
            position: [pose[0], pose[1], pose[2]],
            pitch: pose[3],
            yaw: pose[4],
        };
        let bounds = projection::Bounds {
            left: 0,
            top: 0,
            width: view[0],
            height: view[1],
            zoom: view[2],
            bars: vec![],
        };
        for point in case["points"].as_array().unwrap() {
            let p = ints(point);
            let screen = bounds.project(&pose, [p[0], p[1]], p[2], plane, &scene);
            assert_eq!(screen, [p[3], p[4]], "case {} point {p:?}", case["index"]);
            if screen == [-1, -1] {
                hidden += 1;
            } else {
                visible += 1;
            }
        }
    }
    assert!(
        visible > 800 && hidden > 300,
        "{visible} visible, {hidden} hidden"
    );
}

#[test]
fn occlusion_heights_are_laid_out_as_read_terrain_does() {
    let mut filled = 0;
    for case in cases("occlusion") {
        let seed = int(&case["seed"]);
        let mut grid = Occlusion::default();
        for (file, entry) in case["files"].as_array().unwrap().iter().enumerate() {
            // A map file: 4 x 64 x 64 tiles of opcode 0, then the records.
            let mut bytes = vec![0u8; 4 * 64 * 64];
            for (record, kinds) in entry["records"].as_array().unwrap().iter().enumerate() {
                bytes.push(129);
                for (level, kind) in ints(kinds).into_iter().enumerate() {
                    bytes.push(kind as u8);
                    if kind == 1 {
                        bytes.extend(
                            (0..256).map(|k| {
                                mix(seed, (file * 8 + record) as i32, level as i32, k) as u8
                            }),
                        );
                    }
                }
            }
            let region = Region::decode(50, 50, &bytes).unwrap();
            let at = ints(&entry["at"]);
            grid.read(at[0], at[1], &region.occlusion);
        }
        let mut digest = Sha256::new();
        let mut nonzero = 0;
        for level in 0..4 {
            for x in 0..104 {
                let row: Vec<u8> = (0..104).map(|y| grid.get(level, x, y)).collect();
                nonzero += row.iter().filter(|&&n| n != 0).count();
                digest.update(&row);
            }
        }
        assert_eq!(nonzero as i64, case["nonzero"].as_i64().unwrap());
        assert_eq!(
            format!("{:x}", digest.finalize()),
            case["sha256"].as_str().unwrap(),
            "case {}",
            case["index"]
        );
        filled += usize::from(nonzero > 0);
    }
    assert!(filled > 30, "{filled} cases with heights");
}
