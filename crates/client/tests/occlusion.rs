//! JVM differential for the occluders: their construction from the marks
//! `addLoc` and `buildScene` leave, the ones active for a camera, the four
//! occlusion queries, and the scene walk with them.
//! `fixtures/occlusion/oracle.txt` is written by `cargo xtask
//! occlusion-oracle` from text copied unchanged out of SceneGraph.
use rs09_client::{
    occlusion::{Active, Flags, Frame, Occluder, Removed},
    scene_order::{Draw, Ground, Occlusion, Scene},
};
use sha2::{Digest, Sha256};

const ORACLE: &str = include_str!("../../../fixtures/occlusion/oracle.txt");
/// `FloType.occludeUnderlay` of the oracle's overlays 1 to 6.
const OVERLAYS: [bool; 6] = [true, true, false, true, false, true];

fn numbers(text: &str) -> Vec<i32> {
    text.split_ascii_whitespace()
        .map(|n| n.parse().unwrap())
        .collect()
}

fn mix(seed: i32, level: i32, x: i32, y: i32) -> u32 {
    let mut h =
        x.wrapping_mul(73856093) ^ y.wrapping_mul(19349663) ^ level.wrapping_mul(83492791) ^ seed;
    h ^= ((h as u32) >> 15) as i32;
    h = h.wrapping_mul(0x2c1b3c6d);
    h ^= ((h as u32) >> 12) as i32;
    (h as u32) >> 8
}

/// The oracle's ground: level floors 240 apart with bumps.
fn height(seed: i32, rough: i32, level: usize, x: i32, y: i32) -> i32 {
    let mut h = -240 * level as i32;
    if (mix(seed, level as i32, x, y) % 100) < rough as u32 {
        h -= (mix(seed ^ 0x5bd1e995, level as i32, x, y) % 64) as i32;
    }
    h
}

/// An `L` or `P` operation: a mark of `addLoc` or of `buildScene`.
fn mark(flags: &mut Flags, kind: &str, f: &[i32], height: impl Fn(usize, i32, i32) -> i32) -> bool {
    let level = f[0] as usize;
    let (x, y) = (f[1], f[2]);
    match kind {
        // A type hidden in HD returns before any mark.
        "L" => {
            if f[6] == 0 {
                flags.location(level, [x, y], f[3], f[4], f[5] != 0);
            }
        }
        "P" => flags.floor(
            level,
            [x, y],
            f[3] as u8,
            f[4] as u8,
            f[5] as u8,
            f[4] > 0 && OVERLAYS[f[4] as usize - 1],
            [
                height(level, x, y),
                height(level, x + 1, y),
                height(level, x + 1, y + 1),
                height(level, x, y + 1),
            ],
        ),
        _ => return false,
    }
    true
}

fn occluders(text: &str) -> Vec<Occluder> {
    numbers(text)
        .chunks_exact(7)
        .map(|o| Occluder {
            kind: o[0] as u8,
            min_x: o[1],
            max_x: o[2],
            min_y: o[3],
            max_y: o[4],
            min_z: o[5],
            max_z: o[6],
        })
        .collect()
}

struct Groups(Vec<[i32; 5]>);

impl Groups {
    /// `count` and then height, min x, max x, min z, max z of each.
    fn parse(f: &[i32]) -> (Self, usize) {
        let count = f[0] as usize;
        let groups = f[1..1 + count * 5]
            .chunks_exact(5)
            .map(|g| [g[0], g[1], g[2], g[3], g[4]])
            .collect();
        (Self(groups), 1 + count * 5)
    }
    fn columns(&self) -> [Vec<i32>; 5] {
        std::array::from_fn(|i| self.0.iter().map(|g| g[i]).collect())
    }
}

fn removed(columns: &[Vec<i32>; 5]) -> Removed<'_> {
    Removed {
        max_height: &columns[0],
        min_x: &columns[1],
        max_x: &columns[2],
        min_z: &columns[3],
        max_z: &columns[4],
    }
}

#[test]
fn oracle_sources_remain_pinned() {
    let provenance: serde_json::Value =
        serde_json::from_str(include_str!("../../../fixtures/occlusion/provenance.json")).unwrap();
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

/// `addLoc`'s and `buildScene`'s marks and the occluders made of them, then
/// for three cameras each: the active occluders with their slopes,
/// `isOccluded` at forty points, and seventy questions of one frame.
#[test]
fn occluders_and_queries_match_scene_graph() {
    let (mut builds, mut views, mut built, mut active_total) = (0, 0, 0, 0);
    let (mut points, mut hidden_points, mut questions, mut hidden) = (0, 0, 0, [0; 4]);
    let mut scene: Option<(i32, i32, Vec<Occluder>)> = None;
    for line in ORACLE.lines() {
        if let Some(rest) = line.strip_prefix("build ") {
            let (ops, expected) = rest.split_once('|').unwrap();
            let mut ops = ops.split(';');
            let header = numbers(ops.next().unwrap());
            let (seed, rough) = (header[0], header[1]);
            let ground = |level: usize, x: i32, y: i32| height(seed, rough, level, x, y);
            let mut flags = Flags::new();
            for op in ops {
                let (kind, fields) = op.split_once(' ').unwrap();
                assert!(mark(&mut flags, kind, &numbers(fields), ground), "{op}");
            }
            let actual = flags.build(ground);
            assert_eq!(actual, occluders(expected), "build {builds}");
            built += actual.len();
            builds += 1;
            scene = Some((seed, rough, actual));
        } else if let Some(rest) = line.strip_prefix("view ") {
            let (seed, rough, occluders) = scene.as_ref().unwrap();
            let ground = |level: usize, x: i32, y: i32| height(*seed, *rough, level, x, y);
            let sections: Vec<&str> = rest.split('|').collect();
            let f = numbers(sections[0]);
            let (eye, visibility, mask, percent) = ([f[0], f[1], f[2]], f[3], f[4], f[5] as u32);
            let columns = Groups::parse(&f[6..]).0.columns();
            let active = Active::new(
                occluders,
                removed(&columns),
                eye,
                visibility,
                |column, row| mix(mask, 0, column, row) % 100 < percent,
            );
            let actual: Vec<i32> = active
                .shades()
                .flat_map(|(o, direction, slopes)| {
                    [
                        i32::from(o.kind),
                        o.min_x,
                        o.max_x,
                        o.min_y,
                        o.max_y,
                        o.min_z,
                        o.max_z,
                        i32::from(direction),
                    ]
                    .into_iter()
                    .chain(slopes)
                })
                .collect();
            assert_eq!(actual, numbers(sections[1]), "view {views}: active");
            active_total += active.len();
            for p in numbers(sections[2]).chunks_exact(4) {
                let actual = active.occluded(p[0], p[1], p[2]);
                assert_eq!(i32::from(actual), p[3], "view {views}: point {p:?}");
                points += 1;
                hidden_points += p[3];
            }
            let mut frame = Frame::new(&active, ground);
            let tokens: Vec<&str> = sections[3].split_ascii_whitespace().collect();
            let mut at = 0;
            while at < tokens.len() {
                let kind = tokens[at];
                let count = match kind {
                    "T" => 4,
                    "O" | "W" => 5,
                    _ => 7,
                };
                let f: Vec<i32> = tokens[at + 1..at + 1 + count]
                    .iter()
                    .map(|n| n.parse().unwrap())
                    .collect();
                let level = f[0] as usize;
                let (actual, slot) = match kind {
                    "T" => (frame.tile(level, f[1], f[2]), 0),
                    "O" => (frame.object(level, f[1], f[2], f[3]), 1),
                    "W" => (frame.wall(level, f[1], f[2], f[3]), 2),
                    "A" => (frame.area(level, [f[1], f[3]], [f[2], f[4]], f[5]), 3),
                    other => panic!("unknown question {other}"),
                };
                assert_eq!(
                    i32::from(actual),
                    f[count - 1],
                    "view {views}: {kind} {f:?} (question {questions})"
                );
                hidden[slot] += usize::from(actual);
                questions += 1;
                at += 1 + count;
            }
            views += 1;
        }
    }
    println!(
        "{built} occluders, {active_total} active, {hidden_points} hidden points, hidden answers {hidden:?}"
    );
    assert_eq!((builds, views), (200, 600));
    assert_eq!((points, questions), (600 * 40, 600 * 130));
    // The cases reach what they are meant to.
    assert!(
        built > 1200 && active_total > 1000,
        "{built} {active_total}"
    );
    assert!(hidden_points > 5000, "{hidden_points}");
    assert!(hidden.iter().all(|&n| n > 250), "{hidden:?}");
}

/// Records a model's getMinY().
fn note(min_y: &mut Vec<i32>, part: i32, height: i32) {
    if part >= 0 {
        let part = part as usize;
        if min_y.len() <= part {
            min_y.resize(part + 1, 0);
        }
        min_y[part] = height;
    }
}

/// The models' heights and the frame's answers, as the walk asks for them.
struct Cull<'a, H> {
    frame: Frame<'a, H>,
    min_y: &'a [i32],
    culled: Vec<u32>,
}

impl<H: Fn(usize, i32, i32) -> i32> Occlusion for Cull<'_, H> {
    fn tile(&mut self, level: usize, x: i32, y: i32) -> bool {
        self.frame.tile(level, x, y)
    }
    fn wall(&mut self, level: usize, x: i32, y: i32, flags: i32) -> bool {
        self.frame.wall(level, x, y, flags)
    }
    fn decor(&mut self, level: usize, x: i32, y: i32, part: u32) -> bool {
        self.frame.object(level, x, y, self.min_y[part as usize])
    }
    fn area(&mut self, level: usize, min: [i32; 2], max: [i32; 2], part: u32) -> bool {
        self.frame.area(level, min, max, self.min_y[part as usize])
    }
    fn culled(&mut self, part: u32) {
        self.culled.push(part);
    }
}

/// `renderTile` with the original's own occlusion queries: what is drawn,
/// in order, in scenes with occluders among their walls, decorations,
/// scenery, item piles and actors.
#[test]
fn drawing_with_occluders_matches_render_tile() {
    let part = |n: i32| (n >= 0).then_some(n as u32);
    let (mut cases, mut frames, mut drawn, mut culled, mut culling_frames) = (0, 0, 0, 0, 0);
    let mut both = 0;
    for (case, line) in ORACLE
        .lines()
        .filter_map(|line| line.strip_prefix("scene "))
        .enumerate()
    {
        let mut ops = line.split(';');
        let header = numbers(ops.next().unwrap());
        let (seed, rough) = (header[0], header[1]);
        // `H` operations replace single corner heights.
        let raised = std::cell::RefCell::new(Vec::<[i32; 4]>::new());
        let ground = |level: usize, x: i32, y: i32| {
            raised
                .borrow()
                .iter()
                .find(|h| h[..3] == [level as i32, x, y])
                .map_or_else(|| height(seed, rough, level, x, y), |h| h[3])
        };
        let mut scene = Scene::new();
        let mut flags = Flags::new();
        let mut occluders = Vec::new();
        let mut min_y: Vec<i32> = Vec::new();
        for op in ops {
            let (kind, rest) = op.split_once(' ').unwrap_or((op, ""));
            let (fields, expected) = rest.split_once(':').unwrap_or((rest, ""));
            let f = numbers(fields);
            let level = f.first().copied().unwrap_or(0) as usize;
            match kind {
                "B" => scene.fill_level(0),
                // Ground from a seed over a rectangle, with a chance per level.
                "G" => {
                    for x in f[1]..=f[2] {
                        for y in f[3]..=f[4] {
                            for level in 0..3 {
                                let roll = mix(f[0], level, x, y);
                                if roll % 100 >= f[5 + level as usize] as u32 {
                                    continue;
                                }
                                let ground = match roll / 100 % 10 {
                                    0..=6 => Ground::Plain { coloured: true },
                                    7 => Ground::Plain { coloured: false },
                                    _ => Ground::Shaped,
                                };
                                scene.set_ground(level as usize, x, y, ground);
                            }
                        }
                    }
                }
                "H" => raised.borrow_mut().push([f[0], f[1], f[2], f[3]]),
                "g" => scene.set_ground(
                    level,
                    f[1],
                    f[2],
                    match f[3] {
                        0 => Ground::Plain { coloured: true },
                        1 => Ground::Plain { coloured: false },
                        2 => Ground::Shaped,
                        _ => Ground::None,
                    },
                ),
                "L" | "P" => assert!(mark(&mut flags, kind, &f, ground)),
                "K" => occluders = std::mem::take(&mut flags).build(ground),
                "W" => {
                    note(&mut min_y, f[3], 0);
                    note(&mut min_y, f[4], 0);
                    scene.set_wall(level, f[1], f[2], f[3] as u32, part(f[4]), f[5], f[6]);
                }
                "D" => {
                    note(&mut min_y, f[3], f[7]);
                    note(&mut min_y, f[4], 0);
                    scene.set_wall_decor(level, f[1], f[2], f[3] as u32, part(f[4]), f[5], f[6]);
                }
                "F" => {
                    note(&mut min_y, f[3], 0);
                    scene.set_ground_decor(level, f[1], f[2], f[3] as u32, f[4] != 0);
                }
                "O" => {
                    for part in &f[3..6] {
                        note(&mut min_y, *part, 0);
                    }
                    scene.set_obj_stack(
                        level,
                        f[1],
                        f[2],
                        [part(f[3]), part(f[4]), part(f[5])],
                        f[6] != 0,
                    );
                }
                "S" => {
                    note(&mut min_y, f[7], f[8]);
                    assert_eq!(
                        scene.add_scenery(
                            level,
                            [f[1], f[2]],
                            [f[3], f[4]],
                            [f[5], f[6]],
                            f[7] as u32,
                            true,
                            false
                        ),
                        f[9] != 0,
                        "case {case}: {op}"
                    );
                }
                "A" => {
                    note(&mut min_y, f[4], f[7]);
                    assert_eq!(
                        scene.add_actor(level, [f[1], f[2]], f[3], f[4] as u32, f[5], f[6] != 0),
                        f[8] != 0,
                        "case {case}: {op}"
                    );
                }
                "R" => scene.bridge(f[0], f[1]),
                "C" => scene.clear_temporary(),
                "X" => {
                    let (eye, visibility, walk) = ([f[0], f[1], f[2]], f[3], f[4]);
                    let (seed, percent) = (f[5], f[6] as u32);
                    let (mask, shown) = (f[7], f[8] as u32);
                    let (groups, used) = Groups::parse(&f[9..]);
                    let columns = groups.columns();
                    let visible = |column: i32, row: i32| mix(mask, 0, column, row) % 100 < shown;
                    let active =
                        Active::new(&occluders, removed(&columns), eye, visibility, visible);
                    assert_eq!(active.len() as i32, f[9 + used], "case {case}: active");
                    let tile = [eye[0] / 128, eye[2] / 128];
                    let mut cull = Cull {
                        frame: Frame::new(&active, ground),
                        min_y: &min_y,
                        culled: Vec::new(),
                    };
                    let mut actual = Vec::new();
                    scene.draw_occluded(
                        [eye[0], eye[2]],
                        visibility,
                        |level, x, y| {
                            visible(x + visibility - tile[0], y + visibility - tile[1])
                                && mix(seed, level as i32, x, y) % 100 >= percent
                        },
                        (walk >= 0).then_some(walk as usize),
                        &mut cull,
                        |draw| actual.push(draw),
                    );
                    let text: Vec<String> = actual
                        .iter()
                        .map(|draw| match draw {
                            Draw::Ground { level, tile } => {
                                format!("g{level},{},{}", tile[0], tile[1])
                            }
                            Draw::Model(part) => format!("m{part}"),
                        })
                        .collect();
                    assert_eq!(text.join(" "), expected, "case {case}: {fields}");
                    // What the walk reports as left out was not drawn in
                    // its place; a wall decoration is asked about twice and
                    // may be drawn once from one side.
                    for part in &cull.culled {
                        if actual.contains(&Draw::Model(*part)) {
                            both += 1;
                        }
                    }
                    drawn += actual.len();
                    culled += cull.culled.len();
                    culling_frames += usize::from(!cull.culled.is_empty());
                    frames += 1;
                }
                other => panic!("unknown operation {other}"),
            }
        }
        cases += 1;
    }
    println!("{drawn} drawn, {culled} left out in {culling_frames} frames");
    assert_eq!((cases, frames), (624, 1224));
    assert!(drawn > 70_000, "{drawn}");
    // Occluders took something out of a good share of the frames.
    assert!(
        culled > 600 && culling_frames > 300,
        "{culled} {culling_frames}"
    );
    assert_eq!(both, 0);
}
