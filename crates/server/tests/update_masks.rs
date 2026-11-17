#[path = "../../../oracle/update-masks/native.rs"]
mod native;
use serde_json::Value;
use sha2::{Digest, Sha256};
const CORPUS: &str = include_str!("../../../fixtures/update-masks/oracle.jsonl");
#[test]
fn original_update_masks_match_live_slots_prepare_and_failed_prefixes() {
    for (i, line) in CORPUS.lines().enumerate() {
        let row: Value = serde_json::from_str(line).unwrap();
        let actual = native::evaluate(&row);
        for (step, (actual, expected)) in actual
            .as_array()
            .unwrap()
            .iter()
            .zip(row["expected"].as_array().unwrap())
            .enumerate()
        {
            assert_eq!(
                actual, expected,
                "update mask case {i}, step {step}: {}",
                row["actions"][step]
            );
        }
    }
}
#[test]
fn update_mask_sources_and_fixture_bytes_are_pinned() {
    let p: Value = serde_json::from_str(include_str!(
        "../../../fixtures/update-masks/provenance.json"
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
            let bytes: Vec<_> = std::fs::read(&path)
                .unwrap()
                .into_iter()
                .filter(|b| *b != b'\r')
                .collect();
            assert_eq!(
                format!("{:x}", Sha256::digest(bytes)),
                expected.as_str().unwrap(),
                "{}",
                path.display()
            );
        }
    }
}
