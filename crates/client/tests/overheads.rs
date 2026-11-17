use rs09_client::overheads::{self, Actor, Chat, Config, State};
use serde_json::{Value, json};
fn fixture() -> Value {
    serde_json::from_str(include_str!("../../../fixtures/players/overheads.json")).unwrap()
}
#[test]
fn original_overhead_order_layout_colors_clips_and_filters_match() {
    for (case, row) in fixture()["cases"].as_array().unwrap().iter().enumerate() {
        let actors: Vec<Actor> = serde_json::from_value(row["actors"].clone()).unwrap();
        let config: Config = serde_json::from_value(row["config"].clone()).unwrap();
        let positions: Vec<[i32; 2]> = serde_json::from_value(row["positions"].clone()).unwrap();
        let out = overheads::layout(
            &actors,
            &config,
            |text| text.len() as i32 * 6,
            |i, h| [positions[i][0], positions[i][1] - h / 4],
        );
        assert_eq!(
            serde_json::to_value(out).unwrap(),
            row["expected"],
            "scene {case}"
        );
    }
}
#[test]
fn original_first_free_hit_slots_wrapping_and_chat_expiry_match() {
    let mut state = State::default();
    for row in fixture()["states"].as_array().unwrap() {
        let cmd = row["command"].as_array().unwrap();
        match cmd[0].as_str().unwrap() {
            "R" => state = State::default(),
            "H" => state.hit(
                cmd[1].as_i64().unwrap() as i32,
                cmd[2].as_i64().unwrap() as i32,
                cmd[3].as_i64().unwrap() as i32,
            ),
            "C" => {
                state.chat = Some(Chat {
                    text: cmd[2].as_str().unwrap().into(),
                    loops: cmd[1].as_i64().unwrap() as i32,
                    color: 0,
                    effect: 0,
                })
            }
            "T" => state.tick_chat(),
            _ => unreachable!(),
        }
        let hits: Vec<_> = state
            .hits
            .iter()
            .map(|h| [h.kind, h.damage, h.until])
            .collect();
        assert_eq!(
            json!([hits, state.chat.as_ref().map(|c| json!([c.text, c.loops]))]),
            row["expected"]
        );
    }
}
#[test]
fn original_wave_wave2_and_shake_offsets_match() {
    for row in fixture()["offsets"].as_array().unwrap() {
        let i = |n: &str| row[n].as_i64().unwrap() as i32;
        let offsets =
            overheads::glyph_offsets(i("effect"), i("length") as usize, i("tick"), i("age"));
        assert_eq!(json!(offsets), row["expected"]);
    }
}

#[test]
fn packaged_overhead_replay_has_valid_hit_types_and_complete_lifecycle() {
    use rs09_client::{
        animation::Sequence,
        npc_updates::{self, Event, Metadata},
    };
    use rs09_protocol::game::Frame;
    use std::collections::BTreeMap;
    let input: Value = serde_json::from_str(include_str!(
        "../../../fixtures/players/npc-overheads-replay.json"
    ))
    .unwrap();
    let seq = Sequence {
        frames: vec![0],
        delays: vec![1],
        ..Default::default()
    };
    let sequences = BTreeMap::from([(863, seq.clone()), (866, seq)]);
    let mut state = npc_updates::State::default();
    let mut hits = Vec::new();
    let mut saw_chat = false;
    let mut saw_remove = false;
    for row in input["frames"].as_array().unwrap() {
        if row["opcode"] != 32 {
            continue;
        }
        let cycle = row["tick"].as_i64().unwrap() as i32;
        let payload: Vec<u8> = serde_json::from_value(row["payload"].clone()).unwrap();
        let update = state
            .decode(
                &Frame {
                    opcode: 32,
                    payload,
                },
                npc_updates::Context {
                    cycle,
                    self_tile: [3222, 3218],
                    origin: [3152, 3168],
                    sequences: &sequences,
                    spots: &BTreeMap::new(),
                },
                |id| {
                    Ok(Metadata {
                        size: if id == 81 { 2 } else { 1 },
                        bas: 1426,
                        turn_speed: 32,
                        rotates: true,
                        area_sound: false,
                    })
                },
            )
            .unwrap();
        assert!(update.diagnostics.is_empty());
        saw_remove |= !update.removed.is_empty();
        for event in update.events {
            if let Event::Hit { kind, damage, .. } = event {
                assert!((0..=1).contains(&kind));
                hits.push((kind, damage));
            }
        }
        saw_chat |= state.actors.values().any(|n| !n.chat.is_empty());
    }
    assert!(hits.contains(&(1, 3)) && hits.contains(&(0, 0)) && hits.contains(&(1, 11)));
    assert!(saw_chat && saw_remove);
    assert_eq!(state.local_ids, vec![30, 20, 10]);
}
