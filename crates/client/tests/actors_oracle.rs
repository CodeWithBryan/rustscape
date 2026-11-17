//! JVM differential tests against `fixtures/actors/oracle.jsonl`, which
//! `cargo xtask actors-oracle` writes from the reference client's own code
//! (see `fixtures/actors/provenance.json` for what is unchanged, what is
//! copied and what is a stub).
use rs09_client::{
    actor_animation::{State, Track},
    actor_motion::Context,
    actor_turning,
    animation::Sequence,
    crowd, projectile, recolor,
    walking::{Movement, PlayerBas},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

fn cases(kind: &str) -> Vec<Value> {
    include_str!("../../../fixtures/actors/oracle.jsonl")
        .lines()
        .filter(|line| line.starts_with(&format!("{{\"case\":\"{kind}\"")))
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}
fn int(v: &Value) -> i32 {
    v.as_i64().unwrap() as i32
}
fn ints(v: &Value) -> Vec<i32> {
    v.as_array().unwrap().iter().map(int).collect()
}
fn bytes(v: &Value) -> Vec<u8> {
    let hex = v.as_str().unwrap();
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect()
}
fn track(t: &Track) -> Value {
    json!([t.id, t.frame, t.next, t.clock, t.delay, t.repeats])
}

/// The opcodes of a well-formed BAS record, in order.
fn opcodes(wire: &[u8]) -> Vec<u8> {
    let (mut at, mut out) = (0, Vec::new());
    while at < wire.len() && wire[at] != 0 {
        let opcode = wire[at];
        out.push(opcode);
        at += 1 + match opcode {
            1 => 4,
            26 => 2,
            27 => 13,
            29 | 31 | 34 | 37 => 1,
            2..=9 | 30 | 32 | 33 | 35 | 36 | 38..=45 => 2,
            _ => 0,
        };
    }
    out
}

#[test]
fn every_cached_and_seeded_bas_record_decodes_like_the_original() {
    let cases = cases("bas");
    let (mut cached, mut seeded, mut cached_accelerated) = (0, 0, 0);
    for case in &cases {
        let wire = bytes(&case["hex"]);
        let id = int(&case["id"]);
        let b = PlayerBas::decode(&wire).unwrap_or_else(|e| panic!("bas {id}: {e}"));
        let t = b.turning;
        let terrain = b.terrain.map(|n| n as i16 as i32);
        assert_eq!(
            vec![
                b.idle,
                b.walk,
                b.slow,
                b.slow_turns[0],
                b.slow_turns[1],
                b.slow_turns[2],
                b.run,
                b.run_turns[0],
                b.run_turns[1],
                b.run_turns[2],
                terrain[0],
                terrain[1],
                t.yaw_acceleration,
                t.yaw_max_speed,
                t.roll_acceleration,
                t.roll_max_speed,
                t.roll_target,
                t.pitch_acceleration,
                t.pitch_max_speed,
                t.pitch_target,
                t.movement_acceleration,
                b.stand[0],
                b.stand[1],
                b.turns[0],
                b.turns[1],
                b.turns[2],
            ],
            ints(&case["fields"]),
            "bas {id}"
        );
        let transforms: Vec<Value> = b
            .body_transforms
            .unwrap_or([None; 12])
            .iter()
            .enumerate()
            .filter_map(|(slot, values)| values.map(|v| json!([slot, v])))
            .collect();
        assert_eq!(Value::from(transforms), case["transforms"], "bas {id}");
        if id >= 0 {
            cached += 1;
            if opcodes(&wire).iter().any(|op| (29..=37).contains(op)) {
                cached_accelerated += 1;
            }
        } else {
            seeded += 1;
        }
    }
    assert_eq!((cached, seeded), (1431, 400));
    // The sweep the audit asked for: in the pinned cache no BAS record uses
    // acceleration, accelerated yaw or lean, so no NPC or player does.
    assert_eq!(cached_accelerated, 0);
}

const DIRECTIONS: [[i32; 2]; 8] = [
    [-1, 1],
    [0, 1],
    [1, 1],
    [-1, 0],
    [1, 0],
    [-1, -1],
    [0, -1],
    [1, -1],
];

fn lean_observation(
    turning: &mut actor_turning::State,
    bas: &actor_turning::Bas,
    minimum: i32,
) -> Value {
    match turning.lean(bas) {
        Err(_) => json!({"throws": true}),
        Ok(lean) => {
            let half = minimum / 2;
            let mut ops = Vec::new();
            for (code, angle) in [(2, lean.roll), (1, lean.pitch)] {
                if let Some(angle) = angle {
                    ops.extend([[0, -half], [code, angle], [0, half]]);
                }
            }
            let (r, p) = (turning.roll, turning.pitch);
            json!({
                "lean": [
                    r.target, r.velocity, r.angle, r.midpoint, i32::from(r.accelerating),
                    p.target, p.velocity, p.angle, p.midpoint, i32::from(p.accelerating),
                ],
                "ops": ops,
            })
        }
    }
}

#[test]
fn accelerated_movement_turning_and_lean_match_the_original_entity_phases() {
    let cases = cases("motion");
    assert_eq!(cases.len(), 60);
    let (mut ticks, mut renders, mut throws) = (0, 0, 0);
    let (mut accelerated_moves, mut accelerated_turns, mut leaned) = (0, 0, 0);
    for case in &cases {
        let scenario = int(&case["id"]);
        let pair = |v: &Value| [int(&v[0]), int(&v[1])];
        let origin = pair(&case["origin"]);
        let global = |p: [i32; 2]| [p[0] + origin[0], p[1] + origin[1]];
        let fine_global = |p: [i32; 2]| [p[0] + origin[0] * 128, p[1] + origin[1] * 128];
        let local = |p: [i32; 2]| [p[0] - origin[0], p[1] - origin[1]];
        let npc = case["npc"].as_bool().unwrap();
        let size = int(&case["size"]);
        let mut bas = PlayerBas::decode(&bytes(&case["bas"])).unwrap();
        if case["remap"].as_bool().unwrap() {
            bas = PlayerBas {
                idle: 0,
                walk: 1,
                run: 2,
                stand: [3, 4],
                turns: [5, 6, 7],
                turning: bas.turning,
                terrain: bas.terrain,
                body_transforms: bas.body_transforms,
                ..PlayerBas::default()
            };
        }
        let mut motion = Movement::new(global(pair(&case["start"])));
        motion.fine = motion.head.map(|n| n * 128 + size * 64);
        motion.turn_speed = int(&case["turn"]);
        motion.rotates_on_move = case["rotates"].as_bool().unwrap();
        let mut state = State::default();
        let sequences: BTreeMap<_, _> = case["sequences"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
            .map(|(id, s)| (id as u32, Sequence::decode(&bytes(s)).unwrap()))
            .collect();
        let mut targets = [[0; 2]; 3];
        let mut sounds = Vec::new();
        let mut observed = 0;
        for command in case["commands"].as_array().unwrap() {
            let a = command.as_array().unwrap();
            let n = |i: usize| int(&a[i]);
            match a[0].as_str().unwrap() {
                "M" => {
                    motion.cancel_on_move(&mut state, &sequences).unwrap();
                    let delta = DIRECTIONS[n(2) as usize];
                    motion.enqueue(
                        [motion.head[0] + delta[0], motion.head[1] + delta[1]],
                        n(1) as u8,
                    );
                }
                "P" => {
                    motion.cancel_on_move(&mut state, &sequences).unwrap();
                    motion.teleport_actor(global([n(1), n(2)]), n(3) != 0, size, &mut state);
                }
                "A" => sounds.extend(
                    state
                        .request(n(1), n(2), motion.queue.len() as i32, &sequences)
                        .unwrap(),
                ),
                "F" => motion.force_actor(
                    global([n(1), n(2)]),
                    global([n(3), n(4)]),
                    [n(5), n(6)],
                    n(7),
                    &mut state,
                ),
                "E" => motion.face_entity = n(1),
                "L" => motion.face_tile = [n(1), n(2)],
                "X" => motion.fine = fine_global([n(1), n(2)]),
                "B" => {
                    targets = [
                        fine_global([n(1), n(2)]),
                        fine_global([n(3), n(4)]),
                        fine_global([n(5), n(6)]),
                    ]
                }
                "T" => {
                    let target = match motion.face_entity {
                        8 => Some(targets[1]),
                        32791 => Some(targets[0]),
                        // The own player: this entity itself for a player,
                        // a third actor for an NPC.
                        32785 if npc => Some(targets[2]),
                        _ => None,
                    };
                    let before = (motion.turning.movement_speed, motion.turning.precise_yaw);
                    motion
                        .advance_actor(
                            &mut state,
                            Context {
                                bas: &bas,
                                size,
                                origin,
                                is_self: case["is_self"].as_bool().unwrap(),
                                cycle: n(1),
                                face_target: target,
                                sequences: &sequences,
                            },
                        )
                        .unwrap();
                    accelerated_moves += usize::from(motion.turning.movement_speed != before.0);
                    accelerated_turns += usize::from(motion.turning.precise_yaw != before.1);
                    state.movement.id = motion.sequence;
                    sounds.extend(state.tick(n(1), &sequences, &BTreeMap::new()).unwrap());
                    let head = local(motion.head);
                    let t = motion.turning;
                    let actual = json!({
                        "fine": [motion.fine[0] - origin[0] * 128, motion.fine[1] - origin[1] * 128],
                        "head": [head[0], head[1], motion.head_speed],
                        "queue": motion.queue.iter().map(|(p, s)| {
                            let p = local(*p);
                            [p[0], p[1], i32::from(*s)]
                        }).collect::<Vec<_>>(),
                        "catchup": motion.catchup,
                        "angles": [motion.angle, motion.target_angle, motion.standing_turns],
                        "face": [motion.face_entity, motion.face_tile[0], motion.face_tile[1]],
                        "force": [state.force_start, state.force_end],
                        "steps": state.movement_steps,
                        "movement": track(&state.movement),
                        "action": track(&state.action),
                        "sounds": sounds.iter().map(|s| [s.sequence, s.frame]).collect::<Vec<_>>(),
                        "speed": t.movement_speed,
                        "yaw": [
                            t.yaw_velocity, t.precise_yaw, t.precise_target_yaw, t.yaw_distance,
                            t.yaw_decel_threshold, i32::from(t.yaw_accelerating),
                        ],
                    });
                    assert_eq!(
                        actual, case["expected"][observed],
                        "scenario {scenario}, observation {observed}, command {command}"
                    );
                    sounds.clear();
                    observed += 1;
                    ticks += 1;
                }
                "R" => {
                    let actual = lean_observation(&mut motion.turning, &bas.turning, n(1));
                    assert_eq!(
                        actual, case["expected"][observed],
                        "scenario {scenario}, observation {observed}, command {command}"
                    );
                    throws += usize::from(actual.get("throws").is_some());
                    leaned += usize::from(actual["ops"].as_array().is_some_and(|o| !o.is_empty()));
                    observed += 1;
                    renders += 1;
                }
                _ => panic!("unknown command {command}"),
            }
        }
        assert_eq!(observed, case["expected"].as_array().unwrap().len());
    }
    // The corpus must reach the code under test, not merely agree on idling.
    assert!(ticks > 7000, "{ticks} ticks");
    assert!(
        accelerated_moves > 1000,
        "{accelerated_moves} speed changes"
    );
    assert!(
        accelerated_turns > 1000,
        "{accelerated_turns} precise yaw changes"
    );
    assert!(
        renders > 500 && leaned > 100,
        "{renders} renders, {leaned} leaned"
    );
    assert!(throws > 0, "no zero-acceleration lean record");
}

#[test]
fn body_lean_alone_matches_the_original() {
    let cases = cases("lean");
    assert_eq!(cases.len(), 40);
    let (mut steps, mut rolled, mut pitched, mut throws) = (0, 0, 0, 0);
    for case in &cases {
        let bas = PlayerBas::decode(&bytes(&case["bas"])).unwrap().turning;
        let mut turning = actor_turning::State::default();
        let expected = case["expected"].as_array().unwrap();
        for (i, step) in case["steps"].as_array().unwrap().iter().enumerate() {
            let step = ints(step);
            turning.yaw_accelerating = step[0] != 0;
            turning.yaw_velocity = step[1];
            let actual = lean_observation(&mut turning, &bas, step[2]);
            assert_eq!(actual, expected[i], "case {}, step {i}", case["id"]);
            steps += 1;
            throws += usize::from(actual.get("throws").is_some());
            if let Some(ops) = actual["ops"].as_array() {
                rolled += usize::from(ops.iter().any(|op| op[0] == 2));
                pitched += usize::from(ops.iter().any(|op| op[0] == 1));
            }
        }
    }
    assert!(steps > 5000 && rolled > 1000 && pitched > 1000 && throws > 0);
}

#[test]
fn scene_insertion_thins_crowds_like_the_original() {
    let cases = cases("crowd");
    assert_eq!(cases.len(), 200);
    let (mut inserted, mut left_out, mut low, mut attached) = (0, 0, 0, 0);
    for case in &cases {
        let id = int(&case["id"]);
        let cycle = int(&case["loop"]);
        let rows: Vec<Vec<i32>> = case["players"]
            .as_array()
            .unwrap()
            .iter()
            .map(ints)
            .collect();
        let player = |r: &[i32]| crowd::Player {
            index: r[0],
            visible: r[3] != 0,
            size: r[4],
            fine: [r[5], r[6]],
            idle: r[7] == r[8],
            attached: r[9] != 0 && !(cycle < r[10] || r[11] <= cycle),
        };
        let own_row = rows.iter().find(|r| r[0] == 2047).unwrap();
        let listed: Vec<&Vec<i32>> = rows.iter().filter(|r| r[1] != 0).collect();
        let players: Vec<Option<crowd::Player>> = listed
            .iter()
            .map(|r| (r[2] != 0).then(|| player(r)))
            .collect();
        let npc_rows: Vec<Vec<i32>> = case["npcs"].as_array().unwrap().iter().map(ints).collect();
        let npcs: Vec<Option<crowd::Npc>> = npc_rows
            .iter()
            .map(|r| {
                Some(crowd::Npc {
                    index: r[0],
                    visible: r[1] != 0 && r[3] != 0,
                    top: r[2] != 0,
                    interactive: r[4] != 0,
                    size: r[5],
                    fine: [r[6], r[7]],
                })
            })
            .collect();
        let flag = ints(&case["flag"]);
        let frame = crowd::frame(&crowd::Scene {
            own: player(own_row),
            own_listed: listed.iter().position(|r| r[0] == 2047),
            players: &players,
            npcs: &npcs,
            player_count: players.len() as i32,
            // `(pref && size > 200 || size > 50)` reads the preference but
            // cannot depend on it; the oracle varies it without recording it.
            many_idle_animations: id % 2 == 0,
            minimap_only: case["minimap"].as_bool().unwrap(),
            flag: [flag[0], flag[1]],
        });
        let actual: Vec<Value> = frame
            .inserted
            .iter()
            .map(|i| {
                let (fine, angle, stretches) = match i.who {
                    crowd::Who::Own => ([own_row[5], own_row[6]], own_row[12], own_row[13]),
                    crowd::Who::Player(p) => {
                        ([listed[p][5], listed[p][6]], listed[p][12], listed[p][13])
                    }
                    crowd::Who::Npc(p) => (
                        [npc_rows[p][6], npc_rows[p][7]],
                        npc_rows[p][8],
                        npc_rows[p][9],
                    ),
                };
                // The attached insertion takes no radius or stretch flag.
                json!([
                    i32::from(i.attached),
                    (i.key as u64) >> 32,
                    i.key as u64 & 0xffff_ffff,
                    fine[0],
                    fine[1],
                    if i.attached { 0 } else { i.radius },
                    angle,
                    if i.attached { 0 } else { stretches },
                ])
            })
            .collect();
        assert_eq!(Value::from(actual), case["added"], "case {id}");
        assert_eq!(
            frame
                .low_detail
                .iter()
                .map(|&b| i32::from(b))
                .collect::<Vec<_>>(),
            ints(&case["low"]),
            "case {id}"
        );
        assert_eq!(
            i32::from(frame.own_low_detail),
            int(&case["self_low"]),
            "case {id}"
        );
        assert_eq!(frame.flag_x, int(&case["flag_x"]), "case {id}");
        assert_eq!(
            frame
                .counts
                .iter()
                .map(|&(x, y, n)| vec![x, y, n])
                .collect::<Vec<_>>(),
            case["counts"]
                .as_array()
                .unwrap()
                .iter()
                .map(ints)
                .collect::<Vec<_>>(),
            "case {id}"
        );
        inserted += frame.inserted.len();
        left_out += players.iter().flatten().filter(|p| p.visible).count()
            + npcs.iter().flatten().filter(|n| n.visible).count()
            - frame
                .inserted
                .iter()
                .filter(|i| i.who != crowd::Who::Own)
                .count();
        low += frame.low_detail.iter().filter(|&&b| b).count();
        attached += frame.inserted.iter().filter(|i| i.attached).count();
    }
    assert!(
        inserted > 3000 && left_out > 3000,
        "{inserted} inserted, {left_out} left out"
    );
    assert!(
        low > 500 && attached > 50,
        "{low} low detail, {attached} attached"
    );
}

#[test]
fn projectile_flights_match_the_original() {
    let cases = cases("proj");
    assert_eq!(cases.len(), 300);
    let (mut steps, mut tweened, mut exact, mut arcs) = (0, 0, 0, 0);
    for case in &cases {
        let id = int(&case["id"]);
        let args = ints(&case["args"]);
        let [sx, sy, sz, first, last, elevation, arc, target, _base] = args[..] else {
            panic!("arguments");
        };
        let initial = ints(&case["initial"]);
        let sequence = (!case["sequence"].is_null())
            .then(|| Sequence::decode(&bytes(&case["sequence"])).unwrap());
        let mut flight = projectile::Flight::launch(
            rs09_protocol::game::updates::Projectile {
                offset: [0; 2],
                delta: [0; 2],
                target,
                spot: 3,
                start_height: 0,
                end_height: 0,
                start: first,
                end: last,
                angle: elevation,
                distance: arc,
            },
            0,
            0,
            [sx, sy],
            [initial[0], initial[1]],
            sz,
        );
        arcs += usize::from(elevation != -1);
        let expected = case["expected"].as_array().unwrap();
        for (i, step) in case["steps"].as_array().unwrap().iter().enumerate() {
            let step = ints(step);
            let entity = (step[2] != 0).then_some([step[3], step[4], step[5]]);
            flight.step(step[0], step[1], initial[2], entity, sequence.as_ref());
            assert!(flight.visible(step[0]));
            let [x, y, z] = flight.position;
            assert_eq!(
                vec![
                    x as i32,
                    y as i32,
                    z as i32,
                    flight.yaw,
                    flight.pitch,
                    flight.frame as i32,
                    flight.next_frame,
                    flight.clock
                ],
                ints(&expected[i]["at"]),
                "case {id}, step {i}"
            );
            // What ProjAnim.render asked for: constructModel(next frame,
            // frame, frame clock) and then rotateX(pitch) on the result.
            assert_eq!(
                vec![
                    flight.next_frame,
                    flight.frame as i32,
                    flight.clock,
                    1,
                    flight.pitch
                ],
                ints(&expected[i]["model"]),
                "case {id}, step {i}"
            );
            let bits: Vec<u64> = expected[i]["bits"]
                .as_array()
                .unwrap()
                .iter()
                .map(|b| b.as_str().unwrap().parse::<i64>().unwrap() as u64)
                .collect();
            for (ours, theirs) in flight.position.into_iter().zip(&bits) {
                let theirs = f64::from_bits(*theirs);
                assert!(
                    ours.to_bits() == theirs.to_bits()
                        || (ours - theirs).abs() <= 1e-9 * theirs.abs().max(1.0),
                    "case {id}, step {i}: {ours} against {theirs}"
                );
            }
            exact += usize::from(flight.position.map(f64::to_bits)[..] == bits[..]);
            tweened += usize::from(flight.next_frame != -1);
            steps += 1;
        }
    }
    println!(
        "{steps} projectile steps, {exact} with bit-identical positions, {tweened} with a tween frame"
    );
    assert!(steps > 5000 && tweened > 1000 && arcs > 100);
}

#[test]
fn rgb_to_hsl_matches_the_original_for_every_colour() {
    let case = &cases("hsl")[0];
    let mut digest = Sha256::new();
    let mut block = vec![0u8; 2 * 65536];
    for high in 0..256 {
        for low in 0..65536 {
            let value = recolor::rgb_to_hsl(high << 16 | low);
            block[low as usize * 2] = (value >> 8) as u8;
            block[low as usize * 2 + 1] = value as u8;
        }
        digest.update(&block);
    }
    assert_eq!(
        format!("{:x}", digest.finalize()),
        case["sha256"].as_str().unwrap()
    );
    let mut palette = recolor::Palette::default();
    for (i, sample) in case["samples"].as_array().unwrap().iter().enumerate() {
        let sample = ints(sample);
        assert_eq!(
            recolor::rgb_to_hsl(sample[0]),
            sample[1],
            "rgb {}",
            sample[0]
        );
        palette.set((i % 256) as i32, sample[0]).unwrap();
        assert_eq!(i32::from(palette.colors[i % 256] as i16), sample[2]);
    }
    assert_eq!(palette.revision, 4000);
}

/// SHA-256 (CR removed) of the reference files that the actor work
/// transcribes by hand rather than through the JVM oracle: the tile hint
/// (MiniMap.renderHeadHints), the foot arrow, shadow sequence choice and
/// low-detail reads (Player.render), HintArrowManager.getModel with
/// GlModel.rotateY, the insertion order of ScriptRunner.renderGameScene,
/// updateSceneProjectiles and CS2 5401, the lean call order in Npc.render,
/// NpcType's opcode 99 and palette recolours, the previous-body fallback of
/// PlayerAppearance.getBodyModel and the shared palette array in client.
/// (`tests/server_events.rs` pins other transcribed files the same way.)
const TRANSCRIBED: [(&str, &str); 9] = [
    (
        "MiniMap.java",
        "a179ec6a1ba55d7d66a6f5e71e4107a7839205952afaa9544bc439a66adf217c",
    ),
    (
        "Player.java",
        "3b92fb4c4fd38e8796f2d309f0da0a301713f35b2bc22afed52c9471676998ed",
    ),
    (
        "HintArrowManager.java",
        "61a7464a1234760d66880c89d7bf600943ab5890ac4a5ac2a3e4c8ae59642cc2",
    ),
    (
        "ScriptRunner.java",
        "86e1e9553fbc7a1c7b91cbf0510a59d5d0f112f531cac5d6b77d5a99623ddbc1",
    ),
    (
        "Npc.java",
        "b661a948703b7bc399a4423f8f138dcde7e6f242de79f5433f96f736c7d6419e",
    ),
    (
        "NpcType.java",
        "a13acf5f344708fda80d4ce34cfcffa25c5fb41ab21036f26771865e639a723b",
    ),
    (
        "PlayerAppearance.java",
        "c49a5f3fb187e90b9d60e15f1495416732361fce55b91101612ff59f100f5a0d",
    ),
    (
        "GlModel.java",
        "901a0e9e620407bb979b3bdd76d892a5d4d878c4e57369b8854fceae3b011d48",
    ),
    (
        "client.java",
        "8e5a0c086a1e95a23f2a688ee8d6ba3e3850ab571dad9d553bb69789514abceb",
    ),
];

#[test]
fn transcribed_actor_sources_are_the_pinned_ones() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../reference/client/client/src/main/java/rt4");
    for (name, expected) in TRANSCRIBED {
        let text = String::from_utf8(std::fs::read(root.join(name)).unwrap())
            .unwrap()
            .replace("\r\n", "\n");
        assert_eq!(
            format!("{:x}", Sha256::digest(text.as_bytes())),
            expected,
            "{name} changed: review the actor transcriptions"
        );
    }
}
/// Transcription test (no JVM): NpcType.getBodyModel 337-345 recolours pair
/// `i` to `client.npcRecolorPalette[recol_p[i] & 0xFF]` when `recol_p` has an
/// entry `i`, and to `recol_d[i]` otherwise. No NPC type of the pinned cache
/// has a `recol_p` (see `tests/actors_cache.rs`), so the record is synthetic.
#[test]
fn npc_palette_recolours_read_the_script_palette() {
    use rs09_client::{model::Model, npc::Definition};
    let definition = Definition::decode(&[
        1, 1, 0, 5, // one model, id 5
        40, 2, 0, 100, 0, 200, 1, 44, 1, 144, // recolours 100 -> 200 and 300 -> 400
        42, 1, 200, // recol_p = [(byte) 200]
        99,  // top render priority
        0,
    ])
    .unwrap();
    assert!(definition.top_render_priority);
    assert_eq!(definition.palette, [200]);
    let models = BTreeMap::from([(
        5,
        Model {
            colors: vec![100, 300, 7],
            ..Model::default()
        },
    )]);
    let bas = PlayerBas::default();
    // Before any script runs the palette is all zero.
    let body = definition.assemble_with_bas(&models, &bas).unwrap();
    assert_eq!(body.colors, [0, 400, 7]);
    let mut palette = recolor::Palette::default();
    palette.set(200, 0x00ff00).unwrap();
    let body = definition
        .assemble_with_palette(&models, &bas, &palette.colors)
        .unwrap();
    assert_eq!(body.colors, [recolor::rgb_to_hsl(0x00ff00) as u16, 400, 7]);
    let head = Definition::decode(&[60, 1, 0, 5, 40, 1, 0, 100, 0, 200, 42, 1, 200, 0])
        .unwrap()
        .assemble_head(&models, &palette.colors)
        .unwrap()
        .unwrap();
    assert_eq!(head.colors[0], recolor::rgb_to_hsl(0x00ff00) as u16);
}
