use rs09_protocol::game::{Frame, npcs::State};
use serde_json::{Value, json};
fn bytes(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}
#[test]
fn original_npc32_membership_types_and_queue_heads_without_graphics() {
    let corpus: Value =
        serde_json::from_str(include_str!("../../../fixtures/players/npcs.json")).unwrap();
    for (s, c) in corpus["cases"].as_array().unwrap().iter().enumerate() {
        let pair = |v: &Value| [v[0].as_i64().unwrap() as i32, v[1].as_i64().unwrap() as i32];
        let origin = pair(&c["origin"]);
        let start = pair(&c["start"]);
        let tile = [origin[0] + start[0], origin[1] + start[1]];
        let mut state = State::default();
        for (p, r) in c["packets"].as_array().unwrap().iter().enumerate() {
            state
                .decode(
                    &Frame {
                        opcode: 32,
                        payload: bytes(r["wire"].as_str().unwrap()),
                    },
                    tile,
                    r["cycle"].as_i64().unwrap() as i32,
                )
                .unwrap();
            let actual = state
                .actors
                .iter()
                .map(|(i, n)| {
                    json!([
                        i,
                        n.type_id,
                        n.last_seen,
                        [n.tile[0] - origin[0], n.tile[1] - origin[1]]
                    ])
                })
                .collect::<Vec<_>>();
            let expected = r["expected"]["actors"]
                .as_array()
                .unwrap()
                .iter()
                .map(|a| json!([a[0], a[1], a[2], [a[8][0].clone(), a[8][1].clone()]]))
                .collect::<Vec<_>>();
            assert_eq!(actual, expected, "scenario{s} packet{p}");
            assert_eq!(
                json!(state.local_ids),
                r["expected"]["locals"],
                "scenario{s} packet{p}"
            );
        }
    }
}
