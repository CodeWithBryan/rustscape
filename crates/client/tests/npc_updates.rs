use rs09_client::{
    actor_animation::{Sound, Track},
    actor_motion,
    animation::Sequence,
    npc_updates::{Context, Event, Metadata, State, Update},
    walking::PlayerBas,
};
use rs09_protocol::game::Frame;
use serde_json::{Value, json};
use std::collections::BTreeMap;
fn bytes(hex: &str) -> Vec<u8> {
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect()
}
fn track(t: &Track) -> Value {
    json!([t.id, t.frame, t.next, t.clock, t.delay, t.repeats])
}
#[test]
fn all_cached_npc_stream_metadata_matches_original() {
    let corpus: Value = serde_json::from_str(include_str!(
        "../../../fixtures/players/npc-definitions.json"
    ))
    .unwrap();
    for case in corpus["cases"].as_array().unwrap() {
        let wire = bytes(case["wire"].as_str().unwrap());
        let d = rs09_client::npc::Definition::decode(&wire)
            .unwrap_or_else(|e| panic!("NPC{}: {e}; byte {:?}", case["id"], wire.get(e.offset)));
        assert_eq!(
            json!([
                d.minimap_display,
                d.minimap_marker,
                d.morph_varbit,
                d.morph_varp,
                d.headicon,
                d.hit_bar_id,
                d.icon_height,
                d.size,
                d.bas,
                d.turn_speed,
                d.rotates,
                d.children,
                d.palette,
                d.sounds,
                d.name
                    .chars()
                    .map(|c| format!("{:02x}", c as u8))
                    .collect::<String>()
            ]),
            case["expected"],
            "NPC{}",
            case["id"]
        );
    }
}
fn snapshot(state: &State, update: &Update, origin: [i32; 2]) -> Value {
    let local = |p: [i32; 2]| [p[0] - origin[0], p[1] - origin[1]];
    let events: Vec<_> = update
        .events
        .iter()
        .map(|e| match e {
            Event::Type { index, id } => json!(["type", index, id]),
            Event::AreaRemove(i) => json!(["remove", i]),
            Event::AreaAdd { index, tile } => {
                let p = local(*tile);
                json!(["add", index, p[0], p[1]])
            }
            Event::Hit {
                index,
                damage,
                kind,
                cycle,
            } => json!(["hit", index, damage, kind, cycle]),
            Event::KillingBlow { definition, tile } => {
                let p = local(*tile);
                json!(["kill", definition, p[0], p[1]])
            }
            Event::Sound { sound, .. } => json!(["sound", sound.sequence, sound.frame]),
        })
        .collect();
    let actors: Vec<_> = state
        .actors
        .iter()
        .map(|(i, n)| {
            let m = &n.motion;
            let a = &n.animation;
            let head = local(m.head);
            json!([
                i,
                n.type_id,
                n.last_seen,
                n.metadata.size,
                n.metadata.bas,
                m.turn_speed,
                m.rotates_on_move,
                [m.fine[0] - origin[0] * 128, m.fine[1] - origin[1] * 128],
                [head[0], head[1], m.head_speed],
                m.queue
                    .iter()
                    .map(|(p, s)| {
                        let p = local(*p);
                        [p[0], p[1], i32::from(*s)]
                    })
                    .collect::<Vec<_>>(),
                m.catchup,
                [m.angle, m.target_angle, m.standing_turns],
                [m.face_entity, m.face_tile[0], m.face_tile[1]],
                a.movement_steps,
                track(&a.movement),
                track(&a.action),
                track(&a.spot),
                [a.spot_start, a.spot_height],
                a.slots
                    .iter()
                    .map(|t| t.as_ref().map(track))
                    .collect::<Vec<_>>(),
                n.chat
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<String>(),
                n.chat_loops,
                [n.hit_bar, n.hit_bar_until]
            ])
        })
        .collect();
    json!({"actors":actors,"locals":state.local_ids,"removed":update.removed_candidates,"events":events})
}
#[test]
fn original_npc32_membership_masks_requests_motion_and_rotation_match() {
    let corpus: Value =
        serde_json::from_str(include_str!("../../../fixtures/players/npcs.json")).unwrap();
    let bas = PlayerBas {
        idle: 0,
        walk: 1,
        run: 2,
        stand: [3, 4],
        turns: [5, 6, 7],
        ..PlayerBas::default()
    };
    for (scenario, case) in corpus["cases"].as_array().unwrap().iter().enumerate() {
        let pair = |v: &Value| [v[0].as_i64().unwrap() as i32, v[1].as_i64().unwrap() as i32];
        let origin = pair(&case["origin"]);
        let start = pair(&case["start"]);
        let self_tile = [start[0] + origin[0], start[1] + origin[1]];
        let sequences: BTreeMap<_, _> = case["sequences"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
            .map(|(id, s)| {
                (
                    id as u32,
                    Sequence::decode(&bytes(s.as_str().unwrap())).unwrap(),
                )
            })
            .collect();
        let spots = BTreeMap::from([(17, 0), (18, 4), (19, -1)]);
        let mut state = State::default();
        for (packet, row) in case["packets"].as_array().unwrap().iter().enumerate() {
            let cycle = row["cycle"].as_i64().unwrap() as i32;
            let mut update = state
                .decode(
                    &Frame {
                        opcode: 32,
                        payload: bytes(row["wire"].as_str().unwrap()),
                    },
                    Context {
                        cycle,
                        self_tile,
                        origin,
                        sequences: &sequences,
                        spots: &spots,
                    },
                    |id| {
                        Ok(Metadata {
                            size: 1 + id % 3,
                            bas: 1000 + id,
                            turn_speed: [0, 16, 32, 64][id as usize % 4],
                            rotates: id % 2 == 0,
                            area_sound: id % 3 == 1,
                        })
                    },
                )
                .unwrap();
            assert!(update.diagnostics.is_empty(), "{:?}", update.diagnostics);
            assert_eq!(
                snapshot(&state, &update, origin),
                row["expected"],
                "scenario{scenario} packet{packet}"
            );
            update.events.clear();
            for &index in &state.local_ids {
                let n = &state.actors[&index];
                let target = if n.motion.face_entity >= 32768 {
                    (n.motion.face_entity == 32785).then_some(self_tile.map(|v| v * 128 + 64))
                } else if n.motion.face_entity >= 0 && n.motion.face_entity as usize != index {
                    state
                        .actors
                        .get(&(n.motion.face_entity as usize))
                        .map(|n| n.motion.fine)
                } else {
                    None
                };
                let n = state.actors.get_mut(&index).unwrap();
                n.motion
                    .advance_actor(
                        &mut n.animation,
                        actor_motion::Context {
                            bas: &bas,
                            size: n.metadata.size,
                            origin,
                            is_self: false,
                            cycle: cycle + 1,
                            face_target: target,
                            sequences: &sequences,
                        },
                    )
                    .unwrap();
                n.animation.movement.id = n.motion.sequence;
                let sounds: Vec<Sound> = n.animation.tick(cycle + 1, &sequences, &spots).unwrap();
                update.events.extend(
                    sounds
                        .into_iter()
                        .map(|sound| Event::Sound { index, sound }),
                );
            }
            assert_eq!(
                snapshot(&state, &update, origin),
                row["tick_expected"],
                "scenario{scenario} packet{packet} tick"
            );
        }
    }
}
