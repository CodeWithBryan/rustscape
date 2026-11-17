#[path = "../../../oracle/script-runtime/native.rs"]
mod native;
use serde_json::Value;
use sha2::{Digest, Sha256};
const CORPUS: &str = include_str!("../../../fixtures/script-runtime/oracle.jsonl");
#[test]
fn canonical_entity_clock_ui_and_node_helpers_match_original_bodies() {
    for (i, line) in CORPUS.lines().enumerate() {
        let row: Value = serde_json::from_str(line).unwrap();
        assert_eq!(
            native::evaluate(&row),
            row["expected"],
            "runtime helper {i}: {row}"
        );
    }
}
#[test]
fn script_runtime_helper_sources_and_observations_are_pinned() {
    let p: Value = serde_json::from_str(include_str!(
        "../../../fixtures/script-runtime/provenance.json"
    ))
    .unwrap();
    assert_eq!(p["caseCount"], 1064);
    assert_eq!(CORPUS.lines().count(), 1064);
    assert_eq!(format!("{:x}", Sha256::digest(CORPUS)), p["fixtureSha256"]);
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    for (name, expected) in p["sources"].as_object().unwrap() {
        let path = root.join(name);
        if path.is_file() {
            let bytes = std::fs::read(path)
                .unwrap()
                .into_iter()
                .filter(|b| *b != b'\r')
                .collect::<Vec<_>>();
            assert_eq!(
                format!("{:x}", Sha256::digest(bytes)),
                expected.as_str().unwrap(),
                "{name}"
            );
        }
    }
}
