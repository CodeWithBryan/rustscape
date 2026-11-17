use rs09_client::{
    actor_animation::{State, Track},
    actor_motion::Context,
    animation::Sequence,
    walking::{Movement, PlayerBas},
};
use serde_json::{Value, json};
use std::collections::BTreeMap;

fn track(t: &Track) -> Value {
    json!([t.id, t.frame, t.next, t.clock, t.delay, t.repeats])
}
#[test]
fn destination_clear_retains_y_like_original() {
    use rs09_client::walking::DestinationFlag;
    use rs09_protocol::game::Frame;
    let fixture: Value =
        serde_json::from_str(include_str!("../../../fixtures/players/destination.json")).unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let mut flag = DestinationFlag {
            tile: [
                case["flag"][0].as_i64().unwrap() as i32,
                case["flag"][1].as_i64().unwrap() as i32,
            ],
        };
        let opcode = case["opcode"].as_u64().unwrap() as u8;
        let handled = flag
            .apply(&Frame {
                opcode,
                payload: Vec::new(),
            })
            .unwrap();
        assert_eq!(
            json!([
                flag.tile[0],
                flag.tile[1],
                if handled { -1 } else { i32::from(opcode) },
                handled
            ]),
            case["expected"]
        );
    }
    assert!(
        DestinationFlag::default()
            .apply(&Frame {
                opcode: 153,
                payload: vec![1]
            })
            .is_err()
    );
}
#[test]
fn default_player_complete_entity_phases_match_original() {
    let fixture: Value =
        serde_json::from_str(include_str!("../../../fixtures/players/motion.json")).unwrap();
    let bas = PlayerBas {
        idle: 0,
        walk: 1,
        run: 2,
        stand: [3, 4],
        turns: [5, 6, 7],
        ..PlayerBas::default()
    };
    for (scenario, case) in fixture["cases"].as_array().unwrap().iter().enumerate() {
        let pair = |v: &Value| [v[0].as_i64().unwrap() as i32, v[1].as_i64().unwrap() as i32];
        let origin = pair(&case["origin"]);
        let global = |p: [i32; 2]| [p[0] + origin[0], p[1] + origin[1]];
        let fine_global = |p: [i32; 2]| [p[0] + origin[0] * 128, p[1] + origin[1] * 128];
        let local = |p: [i32; 2]| [p[0] - origin[0], p[1] - origin[1]];
        let mut motion = Movement::new(global(pair(&case["start"])));
        let mut state = State::default();
        let sequences: BTreeMap<_, _> = case["sequences"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
            .map(|(id, s)| {
                let hex = s.as_str().unwrap();
                let bytes: Vec<_> = (0..hex.len())
                    .step_by(2)
                    .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
                    .collect();
                (id as u32, Sequence::decode(&bytes).unwrap())
            })
            .collect();
        let mut targets = [[0; 2]; 2];
        let mut sounds = Vec::new();
        let mut tick = 0;
        for command in case["commands"].as_array().unwrap() {
            let a = command.as_array().unwrap();
            let n = |i: usize| a[i].as_i64().unwrap() as i32;
            match a[0].as_str().unwrap() {
                "M" => {
                    motion.cancel_on_move(&mut state, &sequences).unwrap();
                    let delta = [
                        [-1, 1],
                        [0, 1],
                        [1, 1],
                        [-1, 0],
                        [1, 0],
                        [-1, -1],
                        [0, -1],
                        [1, -1],
                    ][n(2) as usize];
                    motion.enqueue(
                        [motion.head[0] + delta[0], motion.head[1] + delta[1]],
                        n(1) as u8,
                    );
                }
                "P" => {
                    motion.cancel_on_move(&mut state, &sequences).unwrap();
                    motion.teleport_actor(global([n(1), n(2)]), n(3) != 0, 1, &mut state);
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
                "B" => targets = [fine_global([n(1), n(2)]), fine_global([n(3), n(4)])],
                "T" => {
                    let target = match motion.face_entity {
                        8 => Some(targets[1]),
                        32791 => Some(targets[0]),
                        _ => None,
                    };
                    motion
                        .advance_actor(
                            &mut state,
                            Context {
                                bas: &bas,
                                size: 1,
                                origin,
                                is_self: case["is_self"].as_bool().unwrap(),
                                cycle: n(1),
                                face_target: target,
                                sequences: &sequences,
                            },
                        )
                        .unwrap();
                    state.movement.id = motion.sequence;
                    sounds.extend(state.tick(n(1), &sequences, &BTreeMap::new()).unwrap());
                    let head = local(motion.head);
                    let actual = json!({"fine":[motion.fine[0]-origin[0]*128,motion.fine[1]-origin[1]*128],"head":[head[0],head[1],motion.head_speed],"queue":motion.queue.iter().map(|(p,s)|{let p=local(*p);[p[0],p[1],i32::from(*s)]}).collect::<Vec<_>>(),"catchup":motion.catchup,"angles":[motion.angle,motion.target_angle,motion.standing_turns],"face":[motion.face_entity,motion.face_tile[0],motion.face_tile[1]],"force":[state.force_start,state.force_end],"steps":state.movement_steps,"movement":track(&state.movement),"action":track(&state.action),"sounds":sounds.iter().map(|s|[s.sequence,s.frame]).collect::<Vec<_>>()});
                    assert_eq!(
                        actual, case["expected"][tick],
                        "scenario {scenario}, tick {tick}, command {command}"
                    );
                    sounds.clear();
                    tick += 1;
                }
                _ => panic!("unknown command {command}"),
            }
        }
    }
}
