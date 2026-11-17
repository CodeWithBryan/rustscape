//! JVM differential for the title screen's 3D scene.
//! `fixtures/title/scene.txt` is written by `cargo xtask title-oracle` from
//! the unchanged Camera, MathUtils and CollisionMap and from text copied
//! unchanged out of LoginManager, client and PathingEntity; each line is one
//! case, its inputs before " -> " and what the original did after it.
use rs09_client::{
    camera::Script,
    title_scene::{self, Central, Slots},
    ui::vm::CameraRequest,
};
use rs09_game_core::collision::CollisionMap;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

const ORACLE: &str = include_str!("../../../fixtures/title/scene.txt");

/// The regions of the pinned cache that have an NPC file, as the oracle's
/// name table has them.
const TITLE: [[i32; 2]; 30] = [
    [25, 83],
    [26, 82],
    [29, 66],
    [39, 44],
    [39, 51],
    [40, 44],
    [40, 51],
    [41, 51],
    [42, 50],
    [42, 83],
    [43, 66],
    [45, 53],
    [45, 58],
    [46, 49],
    [46, 50],
    [47, 155],
    [47, 160],
    [48, 155],
    [49, 71],
    [50, 50],
    [50, 51],
    [50, 52],
    [50, 53],
    [51, 49],
    [51, 50],
    [53, 54],
    [53, 154],
    [54, 54],
    [56, 152],
    [59, 44],
];

type Fields = BTreeMap<&'static str, &'static str>;

/// The cases of one kind: the fields before and after " -> ".
fn cases(kind: &'static str) -> impl Iterator<Item = (Fields, Fields)> {
    ORACLE.lines().filter_map(move |line| {
        let line = line.strip_prefix(kind)?.strip_prefix(' ')?;
        let (input, output) = line.split_once(" -> ")?;
        let fields = |text: &'static str| -> Fields {
            text.split(' ')
                .filter_map(|field| field.split_once('='))
                .collect()
        };
        Some((fields(input), fields(output)))
    })
}

fn numbers(text: &str, separator: char) -> Vec<i32> {
    if text == "-" {
        return Vec::new();
    }
    text.split(separator)
        .map(|n| n.parse().unwrap_or_else(|_| panic!("number {n:?}")))
        .collect()
}
fn pair(text: &str) -> [i32; 2] {
    numbers(text, ',').try_into().unwrap()
}
fn records(text: &str) -> Vec<Vec<i32>> {
    if text == "-" {
        return Vec::new();
    }
    text.split(';').map(|record| numbers(record, ':')).collect()
}
fn has_spawns(x: i32, y: i32) -> bool {
    TITLE.contains(&[x, y])
}
/// The oracle's stand-in ground (`SceneGraph.getTileHeight`).
fn ground(fine: [i32; 2]) -> i32 {
    -((fine[0] >> 5) + (fine[1] >> 6)) - 200
}

#[test]
fn oracle_sources_remain_pinned() {
    let provenance: serde_json::Value =
        serde_json::from_str(include_str!("../../../fixtures/title/provenance.json")).unwrap();
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

/// `setupLoadingScreenRegion` and `loadRegion(.., fromLogin = true, ..)`:
/// the regions asked for, the new scene base, the camera and its locked
/// targets moved back, and the NPCs kept.
#[test]
fn the_scene_is_set_up_around_the_camera_as_the_original_does() {
    let mut seen = [0; 2];
    for (input, output) in cases("region") {
        let origin = pair(input["origin"]);
        let render = pair(input["render"]);
        let mut central = Central {
            zone: pair(input["central"]),
            origin,
        };
        let selection = title_scene::select(render, origin, has_spawns);
        assert!(selection.regions.len() <= 18);
        let mut packed = vec![0; 18];
        let mut files = vec![0; 18];
        for (i, &(region, listed)) in selection.regions.iter().enumerate() {
            packed[i] = region;
            // The NPC file, the terrain and the scenery; a region without
            // the first is asked for none of them.
            files[i] = if listed { 7 } else { 0 };
        }
        assert_eq!(packed, numbers(output["packed"], ','), "{input:?}");
        assert_eq!(files, numbers(output["files"], ','), "{input:?}");

        // The locked camera's tiles, through the script commands.
        let mut script = Script::default();
        script.render = [render[0], render[1], 0];
        for (name, look) in [("target", false), ("look", true)] {
            let tile = pair(input[name]);
            let tile = [tile[0] + origin[0], tile[1] + origin[1]];
            let (height, min_step, speed) = (0, 0, 0);
            script.script(
                if look {
                    CameraRequest::LookAt {
                        tile,
                        height,
                        min_step,
                        speed,
                    }
                } else {
                    CameraRequest::MoveTo {
                        tile,
                        height,
                        min_step,
                        speed,
                    }
                },
                origin,
                ground,
            );
        }
        let mut npcs = records(input["npcs"]);
        let loaded = central.load(selection.zone);
        let (state, fades) = match loaded {
            None => (10, 0),
            Some(loaded) => {
                script.rebase(loaded.delta);
                // The original walks its NPC table in index order.
                npcs.sort();
                npcs.retain_mut(|npc| {
                    npc[1] -= loaded.delta[0] * 128;
                    npc[2] -= loaded.delta[1] * 128;
                    npc[3] -= loaded.delta[0];
                    npc[4] -= loaded.delta[1];
                    title_scene::npc_kept([npc[1], npc[2]])
                });
                (28, i32::from(loaded.instant_fade))
            }
        };
        seen[usize::from(loaded.is_some())] += 1;
        assert_eq!(state.to_string(), output["state"], "{input:?}");
        assert_eq!(central.origin, pair(output["origin"]), "{input:?}");
        assert_eq!(central.zone, pair(output["central"]), "{input:?}");
        assert_eq!(
            [script.render[0], script.render[1]],
            pair(output["render"]),
            "{input:?}"
        );
        let [target, look] = script.locked_tiles();
        assert_eq!(target, pair(output["target"]), "{input:?}");
        assert_eq!(look, pair(output["look"]), "{input:?}");
        assert_eq!(npcs, records(output["npcs"]), "{input:?}");
        assert_eq!(fades.to_string(), output["fades"], "{input:?}");
        // loadRegion keeps the camera's type when it comes from the title.
        assert_eq!(input["type"], output["type"]);
        if loaded.is_some() {
            let local = central.self_tile();
            assert_eq!(
                [local[0] - central.origin[0], local[1] - central.origin[1]],
                pair(output["self"])
            );
            assert_eq!(output["plane"], "0");
        }
    }
    assert!(seen[0] > 0 && seen[1] > 500, "{seen:?}");
}

/// `decodeNpcFiles`: which records become NPCs, under which index, where
/// and facing which way.
#[test]
fn npc_files_place_the_npcs_the_original_places() {
    let mut placed_total = 0;
    for (input, output) in cases("spawn") {
        let origin = pair(input["origin"]);
        let mut slots = Slots::default();
        for region in numbers(input["slots"], ',') {
            slots.slot(region).unwrap();
        }
        let mut occupied: BTreeSet<usize> = numbers(input["occupied"], ',')
            .into_iter()
            .map(|slot| slot as usize)
            .collect();
        // id -> properties, size, direction, turn speed, BAS.
        let types: BTreeMap<i32, Vec<i32>> = records(input["types"])
            .into_iter()
            .map(|record| (record[0], record[1..].to_vec()))
            .collect();
        let mut placed = Vec::new();
        for file in input["files"].split(';') {
            let (region, data) = file.split_once(':').unwrap();
            let region: i32 = region.parse().unwrap();
            let data: Vec<u8> = if data == "-" {
                Vec::new()
            } else {
                (0..data.len() / 2)
                    .map(|i| u8::from_str_radix(&data[i * 2..i * 2 + 2], 16).unwrap())
                    .collect()
            };
            let slot = slots.slot(region).unwrap();
            for spawn in title_scene::spawns(&data, region, slot, origin) {
                // A type the table does not have is the default one, which
                // is not shown on the login screen.
                let Some(kind) = types.get(&spawn.npc) else {
                    continue;
                };
                let (properties, size, direction, turn, bas) =
                    (kind[0], kind[1], kind[2], kind[3], kind[4]);
                if occupied.contains(&spawn.index)
                    || !title_scene::spawn_placed(&spawn, properties, size)
                {
                    continue;
                }
                occupied.insert(spawn.index);
                let angle = title_scene::spawn_angle(direction).unwrap();
                placed.push(vec![
                    spawn.index as i32,
                    spawn.npc,
                    spawn.tile[0] * 128 + size * 64,
                    spawn.tile[1] * 128 + size * 64,
                    spawn.tile[0],
                    spawn.tile[1],
                    angle,
                    if turn == 0 { 0 } else { angle },
                    turn,
                    bas,
                    size,
                    1,
                ]);
            }
        }
        assert_eq!(slots.len().to_string(), output["count"], "{input:?}");
        assert_eq!(placed, records(output["placed"]), "{input:?}");
        placed_total += placed.len();
    }
    assert!(placed_total > 400, "{placed_total}");
}

struct Wanderer {
    queue: [i32; 2],
    queued: i32,
    fine: [i32; 2],
    size: i32,
    properties: i32,
}

/// The NPC pass of `client.mainUpdate`: the wander test of every NPC in
/// index order against a collision map the NPCs themselves mark, with the
/// oracle's stand-in for the walk (a queued step is taken at once).
#[test]
fn title_npcs_wander_as_the_original_makes_them() {
    let (mut walked, mut skipped, mut stayed) = (0, 0, 0);
    for (input, output) in cases("wander") {
        let mut map = CollisionMap::new(104, 104).unwrap();
        // The tiles whose terrain has been read are not "not loaded".
        let loaded = numbers(input["loaded"], ',');
        for x in loaded[0]..=loaded[2] {
            for y in loaded[1]..=loaded[3] {
                map.set_mask(x, y, 0x100_0000, false).unwrap();
            }
        }
        for flag in records(input["flags"]) {
            map.set_mask(flag[0], flag[1], flag[2], true).unwrap();
        }
        let mut order = Vec::new();
        let mut npcs = BTreeMap::new();
        for npc in records(input["npcs"]) {
            let (slot, tile, size) = (npc[0], [npc[1], npc[2]], npc[3]);
            order.push(slot);
            npcs.insert(
                slot,
                Wanderer {
                    queue: tile,
                    queued: 0,
                    fine: tile.map(|n| n * 128 + size * 64),
                    size,
                    properties: npc[4],
                },
            );
        }
        let randoms: Vec<f64> = if input["randoms"] == "-" {
            Vec::new()
        } else {
            input["randoms"]
                .split(',')
                .map(|bits| f64::from_bits(u64::from_str_radix(bits, 16).unwrap()))
                .collect()
        };
        let mut drawn = 0;
        let mut hashes = Vec::new();
        for _ in 0..input["cycles"].parse::<usize>().unwrap() {
            for npc in npcs.values_mut() {
                let wander = title_scene::wander(
                    &mut map,
                    npc.fine,
                    npc.size,
                    npc.properties,
                    npc.queued == 0,
                    || {
                        drawn += 1;
                        randoms[drawn - 1]
                    },
                );
                if let Some(head) = wander.head {
                    npc.queue = head;
                    stayed += usize::from(!wander.walk && !wander.skip);
                }
                if wander.walk {
                    npc.queued = 1;
                    walked += 1;
                }
                if wander.skip {
                    skipped += 1;
                    continue;
                }
                // NpcList.processMovement, as the oracle stands in for it.
                if npc.queued > 0 {
                    npc.fine = npc.queue.map(|n| n * 128 + npc.size * 64);
                    npc.queued = 0;
                }
                title_scene::mark(&mut map, npc.fine, npc.size);
            }
            let mut hash = 17i32;
            let mut add = |value: i32| hash = hash.wrapping_mul(31).wrapping_add(value);
            for npc in npcs.values() {
                for value in [
                    npc.queue[0],
                    npc.queue[1],
                    npc.queued,
                    npc.fine[0],
                    npc.fine[1],
                ] {
                    add(value);
                }
            }
            map.flags().iter().copied().for_each(&mut add);
            hashes.push(hash);
        }
        assert_eq!(
            hashes,
            numbers(output["hashes"], ','),
            "{:?}",
            input["npcs"]
        );
        assert_eq!(drawn, randoms.len(), "random numbers drawn");
        let last: Vec<Vec<i32>> = order
            .iter()
            .map(|slot| {
                let npc = &npcs[slot];
                vec![*slot, npc.fine[0], npc.fine[1], npc.queue[0], npc.queue[1]]
            })
            .collect();
        assert_eq!(last, records(output["npcs"]));
    }
    // Steps taken, steps refused with the queue's head left moved, and the
    // larger NPCs' skipped turns all occur.
    assert!(
        walked > 300 && stayed > 300 && skipped > 20,
        "{walked} {stayed} {skipped}"
    );
}

/// `setupLoadingScreenRegion` as `client.mainUpdate` and
/// `resetToLoginScreen` call it. Returns the game state it leaves.
fn reload(script: &mut Script, central: &mut Central) -> i32 {
    let selection = title_scene::select(
        [script.render[0], script.render[1]],
        central.origin,
        has_spawns,
    );
    match central.load(selection.zone) {
        Some(loaded) => {
            script.rebase(loaded.delta);
            28
        }
        None => 10,
    }
}

fn pose(script: &Script, state: i32, central: &Central) -> Vec<i32> {
    vec![
        script.render[0],
        script.render[1],
        script.render[2],
        script.pitch,
        script.yaw,
        state,
        central.origin[0],
        central.origin[1],
    ]
}

/// One `client.mainUpdate` for the camera: it stands still while a login
/// or an account creation runs; otherwise it advances, and a camera that
/// has left the middle of the scene has the scene set up around it.
fn camera_cycle(script: &mut Script, central: &mut Central, busy: bool) -> i32 {
    if busy {
        return 10;
    }
    script.spline_cycle(central.origin);
    script.cycle(ground);
    if title_scene::camera_outside([script.render[0], script.render[1]]) {
        reload(script, central)
    } else {
        10
    }
}

/// The spline camera (type 3) from `resetToLoginScreen` on: every cycle's
/// position, angles (as integers and as the floats the renderer aims
/// with), progress and finish flags, and each scene change it causes.
#[test]
fn the_spline_camera_flies_and_moves_the_scene_as_the_original_does() {
    let (mut reloads, mut finished) = (0, 0);
    for (input, output) in cases("spline") {
        let paths = [records(input["path0"]), records(input["path1"])];
        let segment = |path: i32, index: i32| -> [[i32; 3]; 4] {
            std::array::from_fn(|i| {
                paths[path as usize][index as usize * 2 + i]
                    .clone()
                    .try_into()
                    .unwrap()
            })
        };
        let moving = numbers(input["move"], ',');
        let looking = numbers(input["look"], ',');
        let mut script = Script::default();
        // CS2 5502 as `Vm` turns it into a request.
        script.script(
            CameraRequest::Spline {
                position: segment(moving[0], moving[1]),
                look: segment(looking[0], looking[1]),
                speed_start: moving[2],
                speed_end: moving[3],
            },
            [0, 0],
            ground,
        );
        let mut central = Central::default();
        script.title_reset(central.origin);
        let state = reload(&mut script, &mut central);
        assert_eq!(
            pose(&script, state, &central),
            numbers(output["first"], ':'),
            "{input:?}"
        );
        let expected = records(output["steps"]);
        assert_eq!(expected.len(), input["busy"].len());
        for (step, busy) in input["busy"].bytes().enumerate() {
            let state = camera_cycle(&mut script, &mut central, busy == b'1');
            reloads += usize::from(state == 28);
            let spline = script.spline.as_ref().unwrap();
            finished += usize::from(spline.just_finished && busy == b'0');
            let mut got = pose(&script, state, &central);
            got.extend([
                spline.pitch_radians().to_bits() as i32,
                spline.yaw_radians().to_bits() as i32,
                spline.progress,
                i32::from(spline.finished),
                i32::from(spline.just_finished),
            ]);
            assert_eq!(got, expected[step], "step {step} of {input:?}");
        }
    }
    assert!(reloads > 10 && finished > 10, "{reloads} {finished}");
}

/// The locked camera (type 2, CS2 5500 and 5501) through the same cycles.
#[test]
fn the_locked_camera_eases_and_moves_the_scene_as_the_original_does() {
    let mut reloads = 0;
    for (input, output) in cases("locked") {
        let origin = pair(input["origin"]);
        let mut central = Central {
            zone: pair(input["central"]),
            origin,
        };
        let start = numbers(input["pose"], ',');
        let mut script = Script::default();
        script.render = [start[0], start[1], start[2]];
        script.pitch = start[3];
        script.yaw = start[4];
        let moving = numbers(input["move"], ',');
        script.script(
            CameraRequest::MoveTo {
                tile: [moving[0] + origin[0], moving[1] + origin[1]],
                height: moving[2],
                min_step: moving[3],
                speed: moving[4],
            },
            origin,
            ground,
        );
        let looking = numbers(input["look"], ',');
        script.script(
            CameraRequest::LookAt {
                tile: [looking[0] + origin[0], looking[1] + origin[1]],
                height: looking[2],
                min_step: looking[3],
                speed: looking[4],
            },
            origin,
            ground,
        );
        let expected = records(output["steps"]);
        assert_eq!(expected.len(), input["busy"].len());
        for (step, busy) in input["busy"].bytes().enumerate() {
            let state = camera_cycle(&mut script, &mut central, busy == b'1');
            reloads += usize::from(state == 28);
            assert_eq!(
                pose(&script, state, &central),
                expected[step],
                "step {step} of {input:?}"
            );
        }
    }
    assert!(reloads > 10, "{reloads}");
}
