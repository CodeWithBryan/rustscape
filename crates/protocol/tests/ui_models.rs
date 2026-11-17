use rs09_protocol::game::{
    Frame,
    updates::{self, ServerUpdate},
};
use serde_json::Value;
#[test]
fn original_server_model_widget_packet_bytes_and_truncated_payload_boundaries_match() {
    let corpus: Value =
        serde_json::from_str(include_str!("../../../fixtures/ui/model-updates.json")).unwrap();
    for (i, case) in corpus["wires"].as_array().unwrap().iter().enumerate() {
        let a: Vec<i32> = case["args"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_i64().unwrap() as i32)
            .collect();
        let f = match a[0] {
            0 => updates::interface_animation(a[1], a[2], a[5]),
            1 => updates::interface_model(a[1], a[2], a[5]),
            2 => updates::interface_npc_head(a[1], a[2], a[5]),
            3 => updates::interface_player_head(a[1], a[5]),
            4 => updates::interface_angle(a[1], a[2], a[4], a[3], a[5]),
            5 => updates::interface_object(a[1], a[2], if a[3] > 0 { a[3] } else { a[4] }, a[5]),
            6 => updates::interface_position(a[1], a[2], a[3], a[5]),
            _ => unreachable!(),
        };
        let mut bytes = vec![f.opcode];
        bytes.extend(&f.payload);
        assert_eq!(
            serde_json::to_value(bytes).unwrap(),
            case["expected"][0],
            "original writer{i}"
        );
        assert!(ServerUpdate::decode(&f).unwrap().is_some());
        for end in 0..f.payload.len() {
            assert!(
                ServerUpdate::decode(&Frame {
                    opcode: f.opcode,
                    payload: f.payload[..end].to_vec()
                })
                .is_err(),
                "truncated{i}:{end}"
            );
        }
    }
}
