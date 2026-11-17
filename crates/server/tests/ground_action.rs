#[path = "../../../oracle/chunk-sync/native.rs"]
mod native;
use serde_json::Value;
use sha2::{Digest, Sha256};
const CORPUS: &str = include_str!("../../../fixtures/ground-action/oracle.jsonl");
#[test]
fn actual_take_packet_preserves_canonical_player_and_selected_ground_option() {
    use rs09_protocol::{buffer::Reader, game::actions::ClientAction};
    use rs09_server::world::{doors::Location, ground_action::Request, player_walk::State};
    let mut actors = State::default();
    let node = actors.add(7, Location::new(3207, 3216, 0), [1, 2, 3, 4]);
    let player = actors.retain_node(node).unwrap();
    // RT4 ground Take: ip2(x), p2(id), ip2add(y), opcode66.
    let action = ClientAction::decode(66, &mut Reader::new(&[0x87, 0x0c, 0x03, 0xe3, 0x10, 0x0c]))
        .unwrap()
        .unwrap();
    let request = Request::from_action(player.clone(), &action).unwrap();
    assert!(request.player.as_ref() == Some(&player));
    assert_eq!(
        (request.id, request.x, request.y, request.option),
        (995, 3207, 3216, 2)
    );
}
fn difference(a: &Value, b: &Value, path: String) -> Option<String> {
    if a == b {
        return None;
    }
    if let (Some(a), Some(b)) = (a.as_object(), b.as_object()) {
        for key in a.keys().chain(b.keys()) {
            if let Some(d) = difference(
                a.get(key).unwrap_or(&Value::Null),
                b.get(key).unwrap_or(&Value::Null),
                format!("{path}/{key}"),
            ) {
                return Some(d);
            }
        }
    }
    if let (Some(a), Some(b)) = (a.as_array(), b.as_array()) {
        for (i, (a, b)) in a.iter().zip(b).enumerate() {
            if let Some(d) = difference(a, b, format!("{path}/{i}")) {
                return Some(d);
            }
        }
        return Some(format!("{path}: lengths {} != {}", a.len(), b.len()));
    }
    Some(format!("{path}: native={a}, original={b}"))
}
#[test]
fn original_ground_clicks_native_lookup_selected_options_and_ordered_failures() {
    for (i, line) in CORPUS.lines().enumerate() {
        let row: Value = serde_json::from_str(line).unwrap();
        let actual = native::evaluate(&row);
        if let Some(d) = difference(&actual, &row["expected"], String::new()) {
            panic!("ground action case {i}: {d}");
        }
    }
}
#[test]
fn ground_action_fixture_and_source_hashes_are_pinned() {
    let p: Value = serde_json::from_str(include_str!(
        "../../../fixtures/ground-action/provenance.json"
    ))
    .unwrap();
    assert_eq!(p["caseCount"], CORPUS.lines().count());
    assert_eq!(p["fixtureSha256"], format!("{:x}", Sha256::digest(CORPUS)));
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    for (path, expected) in p["sources"].as_object().unwrap() {
        let path = root.join(path);
        if path.is_file() {
            let data: Vec<_> = std::fs::read(&path)
                .unwrap()
                .into_iter()
                .filter(|b| *b != b'\r')
                .collect();
            assert_eq!(
                format!("{:x}", Sha256::digest(data)),
                expected.as_str().unwrap(),
                "{}",
                path.display()
            );
        }
    }
}
