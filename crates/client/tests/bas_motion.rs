use rs09_client::walking::{Movement, PlayerBas};
use serde_json::{Value, json};

fn bytes(hex: &str) -> Vec<u8> {
    hex.as_bytes()
        .chunks_exact(2)
        .map(|c| u8::from_str_radix(std::str::from_utf8(c).unwrap(), 16).unwrap())
        .collect()
}

#[test]
fn cached_and_synthetic_bas_decoding_and_directional_motion_match_original() {
    let fixture: Value =
        serde_json::from_str(include_str!("../../../fixtures/players/bas-motion.json")).unwrap();
    assert_eq!(fixture["cachedCount"], 1431);
    for case in fixture["decoded"].as_array().unwrap() {
        let b = PlayerBas::decode(&bytes(case["bytes"].as_str().unwrap())).unwrap();
        assert_eq!(
            json!([
                b.idle,
                b.walk,
                b.run,
                b.stand[0],
                b.stand[1],
                b.turns[0],
                b.turns[1],
                b.turns[2],
                b.slow,
                b.slow_turns[0],
                b.slow_turns[1],
                b.slow_turns[2],
                b.run_turns[0],
                b.run_turns[1],
                b.run_turns[2],
                b.terrain[0],
                b.terrain[1]
            ]),
            case["expected"],
            "BAS {}",
            case["id"]
        );
    }
    for (scenario, case) in fixture["motions"].as_array().unwrap().iter().enumerate() {
        let b = PlayerBas::decode(&bytes(case["bas"].as_str().unwrap())).unwrap();
        let c: Vec<i32> = case["fields"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_i64().unwrap() as i32)
            .collect();
        let mut motion = Movement::new([50, 50]);
        motion.rotates_on_move = c[0] == 0 || c[1] != 0;
        motion.angle = c[3];
        motion.turn_speed = c[4];
        motion.face_entity = c[5];
        motion.catchup = c[6];
        motion.fine = [c[7], c[8]];
        for e in case["queue"].as_array().unwrap() {
            motion.enqueue(
                [e[0].as_i64().unwrap() as i32, e[1].as_i64().unwrap() as i32],
                e[2].as_u64().unwrap() as u8,
            );
        }
        for (tick, expected) in case["expected"].as_array().unwrap().iter().enumerate() {
            motion.advance_sized(&b, c[2]);
            assert_eq!(
                json!([
                    motion.fine[0],
                    motion.fine[1],
                    motion.angle,
                    motion.target_angle,
                    motion.sequence,
                    motion.catchup,
                    motion.standing_turns,
                    motion.queue.len()
                ]),
                *expected,
                "scenario {scenario}, tick {tick}"
            );
        }
    }
    // Acceleration stays unsupported; this short opcode27 input is truncated.
    for op in [27, 29, 30, 31, 32, 33, 34, 35, 36, 37] {
        assert!(PlayerBas::decode(&[op, 0]).is_err());
    }
    assert!(PlayerBas::decode(&[7, 1]).is_err());
}
