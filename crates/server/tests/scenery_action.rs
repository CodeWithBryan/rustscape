#[path = "../../../oracle/scenery-action/native.rs"]
mod native;
use rs09_protocol::{buffer::Reader, game::actions::ClientAction};
use rs09_server::world::scenery_action::Request;
use serde_json::Value;
use sha2::{Digest, Sha256};
const CORPUS: &str = include_str!("../../../fixtures/scenery-action/oracle.jsonl");

#[test]
fn original_scenery_dispatch_levers_morphs_options_reentrancy_and_failures() {
    for (i, line) in CORPUS.lines().enumerate() {
        let row: Value = serde_json::from_str(line).unwrap();
        assert_eq!(
            native::evaluate(&row),
            row["expected"],
            "scenery action case {i}: {line}"
        );
    }
}

#[test]
fn real_rt4_open_click_decodes_into_authoritative_scenery_request() {
    // RT4 interaction action 42: ip2(x), p2add(id), p2(y), packet 254.
    let bytes = [0x87, 0x0c, 0x30, 0xbc, 0x0c, 0x91];
    let action = ClientAction::decode(254, &mut Reader::new(&bytes))
        .unwrap()
        .unwrap();
    let r = Request::from_action(42, &action).unwrap();
    assert_eq!(
        (r.player, r.id, r.x, r.y, r.option),
        (Some(42), 12348, 3207, 3217, 0)
    );
}

#[test]
fn scenery_action_fixture_bytes_and_source_hashes_are_pinned() {
    let p: Value = serde_json::from_str(include_str!(
        "../../../fixtures/scenery-action/provenance.json"
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
