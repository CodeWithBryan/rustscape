use rs09_protocol::game::{self, Direction, rebuild::decode_normal};
use serde_json::Value;
#[test]
fn normal_client_fields_decode_original_server_rebuild_frames_and_key_order() {
    let mut checked = 0;
    for line in include_str!("../../../fixtures/player-render/oracle.jsonl").lines() {
        let case: Value = serde_json::from_str(line).unwrap();
        if case.get("hooks").is_some() {
            continue;
        }
        let expected = case["expected"].as_array().unwrap();
        for (action, state) in case["actions"].as_array().unwrap().iter().zip(expected) {
            if action["op"] != "rebuild" || !state["error"].is_null() {
                continue;
            }
            let wire = state["frames"][0].as_str().unwrap();
            let bytes: Vec<_> = (0..wire.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&wire[i..i + 2], 16).unwrap())
                .collect();
            let mut framer = game::Decoder::new(Direction::ServerToClient, 65536, 65535);
            framer.push(&bytes).unwrap();
            let frame = framer.next_frame().unwrap().unwrap();
            let decoded = decode_normal(&frame).unwrap();
            let loc = &state["nodes"]["self"]["location"];
            let x = loc[0].as_i64().unwrap() as i32;
            let y = loc[1].as_i64().unwrap() as i32;
            let z = loc[2].as_i64().unwrap() as i32;
            assert_eq!(
                [decoded.local_x, decoded.local_y, decoded.plane],
                [48 + (x & 7), 48 + (y & 7), z]
            );
            assert_eq!(
                [decoded.zone_x, decoded.zone_y],
                [(x >> 3) as u16 as i32, (y >> 3) as u16 as i32]
            );
            let keys: Vec<_> = state["events"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|e| e[0] == "keys")
                .map(|e| {
                    let id = e[1].as_i64().unwrap() as i32;
                    [id, -id, i32::MIN, i32::MAX]
                })
                .collect();
            assert_eq!(decoded.keys, keys);
            checked += 1;
            for n in 0..frame.payload.len() {
                // A shorter 9+16k payload is another structurally valid field
                // packet; the transport's original length header detects its
                // truncation. Only incomplete field groups fail here.
                if n >= 9 && (n - 9) % 16 == 0 {
                    continue;
                }
                let mut truncated = frame.clone();
                truncated.payload.truncate(n);
                assert!(decode_normal(&truncated).is_err(), "accepted prefix {n}");
            }
        }
    }
    assert_eq!(checked, 32);
}
