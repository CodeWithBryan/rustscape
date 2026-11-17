#[path = "../../../oracle/legacy-interaction/native.rs"]
mod native;
use serde_json::Value;
use sha2::{Digest, Sha256};
const CORPUS: &str = include_str!("../../../fixtures/legacy-interaction/oracle.jsonl");
#[test]
fn original_legacy_orchestration_and_real_arrival_bodies_match() {
    for (i, line) in CORPUS.lines().enumerate() {
        let v: Value = serde_json::from_str(line).unwrap();
        assert_eq!(
            native::evaluate(&v),
            v["expected"],
            "legacy interaction {i}: {line}"
        );
    }
}
#[test]
fn legacy_fixture_and_complete_original_sources_are_pinned() {
    let p: Value = serde_json::from_str(include_str!(
        "../../../fixtures/legacy-interaction/provenance.json"
    ))
    .unwrap();
    assert_eq!(p["caseCount"], CORPUS.lines().count());
    assert_eq!(p["fixtureSha256"], format!("{:x}", Sha256::digest(CORPUS)));
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    for (name, h) in p["sources"].as_object().unwrap() {
        let path = root.join(name);
        if path.is_file() {
            let bytes: Vec<_> = std::fs::read(&path)
                .unwrap()
                .into_iter()
                .filter(|b| *b != b'\r')
                .collect();
            assert_eq!(
                format!("{:x}", Sha256::digest(bytes)),
                h.as_str().unwrap(),
                "{}",
                path.display()
            );
        }
    }
}
