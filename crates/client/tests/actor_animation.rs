use rs09_client::{
    actor_animation::{Sound, State, Track},
    animation::Sequence,
};
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
fn snapshot(s: &State, sounds: &[Sound]) -> Value {
    json!({"movement":track(&s.movement),"action":track(&s.action),"spot":track(&s.spot),"spot_start":s.spot_start,"spot_height":s.spot_height,"steps":s.movement_steps,"stretches":s.stretches,
        "slots":s.slots.iter().map(|s|s.as_ref().map(track)).collect::<Vec<_>>(),"sounds":sounds.iter().map(|s|[s.sequence,s.frame]).collect::<Vec<_>>()})
}
#[test]
fn animation_priorities_delays_replays_spots_slots_and_sound_trigger_order_match_original() {
    let fixture: Value =
        serde_json::from_str(include_str!("../../../fixtures/players/animation.json")).unwrap();
    for (scenario, case) in fixture["cases"].as_array().unwrap().iter().enumerate() {
        let sequences: BTreeMap<_, _> = case["sequences"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
            .map(|(i, s)| {
                (
                    i as u32,
                    Sequence::decode(&bytes(s.as_str().unwrap())).unwrap(),
                )
            })
            .collect();
        let spots: BTreeMap<_, _> = case["spots"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(id, s)| (id.parse::<i32>().unwrap(), s.as_i64().unwrap() as i32))
            .collect();
        for (id, s) in &sequences {
            assert_eq!(
                json!([
                    s.frames,
                    s.delays,
                    s.replay_offset.map_or(-1, |n| n as i32),
                    s.priority,
                    s.replay_count,
                    s.loop_type,
                    s.move_type,
                    s.exact_move,
                    s.mainhand,
                    s.offhand,
                    s.secondary_frames
                        .iter()
                        .map(|&n| n as i32)
                        .collect::<Vec<_>>(),
                    s.frame_group,
                    s.stretches,
                    s.transform_normals,
                    s.tween,
                    s.update_shadows,
                    s.sounds
                ]),
                case["metadata"][*id as usize],
                "scenario{scenario} sequence{id}"
            );
        }
        let mut state = State::default();
        let mut sounds = vec![];
        let mut tick = 0;
        for command in case["commands"].as_array().unwrap() {
            let c = command.as_array().unwrap();
            let n = |i: usize| c[i].as_i64().unwrap() as i32;
            match c[0].as_str().unwrap() {
                "A" => sounds.extend(state.request(n(1), n(2), n(3), &sequences).unwrap()),
                "L" => sounds.extend(
                    state
                        .request_slots(&[(n(1), n(2), n(3))], &sequences)
                        .unwrap(),
                ),
                "G" => sounds.extend(
                    state
                        .request_spot(n(1), n(2), n(3), &spots, &sequences)
                        .unwrap(),
                ),
                "M" => state.movement.id = n(1),
                "Q" => {
                    state.movement_steps = n(1);
                    state.force_start = n(2);
                    state.force_end = n(3);
                }
                "T" => {
                    sounds.extend(state.tick(n(1), &sequences, &spots).unwrap());
                    assert_eq!(
                        snapshot(&state, &sounds),
                        case["expected"][tick],
                        "scenario{scenario} tick{tick}"
                    );
                    tick += 1;
                    sounds.clear();
                }
                _ => panic!("unknown command"),
            }
        }
    }
}
