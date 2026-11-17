use rs09_client::{player_body, walking::PlayerBas};
use serde_json::{Value, json};

fn bytes(text: &str) -> Vec<u8> {
    text.as_bytes()
        .chunks_exact(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}

#[test]
fn original_bas_fields_and_signed_duplicate_body_slots_match() {
    let fixture: Value =
        serde_json::from_str(include_str!("../../../fixtures/ui/npc-bas.json")).unwrap();
    for (index, case) in fixture["cases"].as_array().unwrap().iter().enumerate() {
        let raw = bytes(case["bytes"].as_str().unwrap());
        let bas = PlayerBas::decode(&raw).unwrap();
        assert_eq!(
            json!([
                [
                    bas.idle,
                    bas.walk,
                    bas.run,
                    bas.stand[0],
                    bas.stand[1],
                    bas.turns[0],
                    bas.turns[1],
                    bas.turns[2],
                    bas.slow,
                    bas.slow_turns[0],
                    bas.slow_turns[1],
                    bas.slow_turns[2],
                    bas.run_turns[0],
                    bas.run_turns[1],
                    bas.run_turns[2],
                    bas.terrain[0],
                    bas.terrain[1]
                ],
                bas.body_transforms,
                raw.len()
            ]),
            case["expected"],
            "original BAS case{index}"
        );
        assert_eq!(
            bas.body_transforms,
            player_body::bas_transforms(&raw).unwrap()
        );
    }
}

#[test]
fn acceleration_opcodes_decode_and_malformed_body_slots_fail_explicitly() {
    // Opcodes 29-37 were refused before the accelerated phases were ported;
    // fixtures/actors holds the JVM's decoding of every one of them.
    for opcode in 29..=37 {
        let mut raw = vec![opcode, 7];
        if matches!(opcode, 30 | 32 | 33 | 35 | 36) {
            raw.push(9);
        }
        raw.push(0);
        let t = PlayerBas::decode(&raw).unwrap().turning;
        let wide = 7 * 256 + 9;
        let expected = match opcode {
            29 => (t.yaw_acceleration, 7),
            30 => (t.yaw_max_speed, wide),
            31 => (t.roll_acceleration, 7),
            32 => (t.roll_max_speed, wide),
            33 => (t.roll_target, wide),
            34 => (t.pitch_acceleration, 7),
            35 => (t.pitch_max_speed, wide),
            36 => (t.pitch_target, wide),
            _ => (t.movement_acceleration, 7),
        };
        assert_eq!(expected.0, expected.1, "opcode {opcode}");
    }
    assert_eq!(PlayerBas::default().turning.movement_acceleration, -1);
    for slot in [12, 255] {
        let mut raw = vec![27, slot];
        raw.extend([0; 13]);
        assert!(PlayerBas::decode(&raw).is_err());
    }
    for length in 1..14 {
        assert!(PlayerBas::decode(&vec![27; length]).is_err());
    }
}
