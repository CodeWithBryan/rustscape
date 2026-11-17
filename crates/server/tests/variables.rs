#[path = "../../../oracle/variables/native.rs"]
mod native;
use serde_json::Value;
use sha2::{Digest, Sha256};
const CORPUS: &str = include_str!("../../../fixtures/variables/oracle.jsonl");
#[test]
fn original_variable_algorithms_packets_aliases_and_failed_prefixes_match() {
    for (i, line) in CORPUS.lines().enumerate() {
        let row: Value = serde_json::from_str(line).unwrap();
        let actual = native::evaluate(&row);
        if actual != row["expected"] {
            let index = actual
                .as_array()
                .unwrap()
                .iter()
                .zip(row["expected"].as_array().unwrap())
                .position(|(a, b)| a != b)
                .unwrap();
            assert_eq!(
                actual[index], row["expected"][index],
                "original variable case{i} action{index}: {}",
                row["actions"]
            );
        }
    }
}
#[test]
fn original_variable_sources_and_fixture_are_pinned() {
    let p: Value =
        serde_json::from_str(include_str!("../../../fixtures/variables/provenance.json")).unwrap();
    assert_eq!(p["caseCount"], CORPUS.lines().count());
    assert_eq!(p["fixtureSha256"], format!("{:x}", Sha256::digest(CORPUS)));
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    for (name, hash) in p["sources"].as_object().unwrap() {
        let path = root.join(name);
        if path.is_file() {
            let data: Vec<_> = std::fs::read(path)
                .unwrap()
                .into_iter()
                .filter(|b| *b != b'\r')
                .collect();
            assert_eq!(
                hash.as_str().unwrap(),
                format!("{:x}", Sha256::digest(data)),
                "{name}"
            );
        }
    }
    assert_eq!(
        p["customDefinitions"]["sha256"],
        format!(
            "{:x}",
            Sha256::digest(include_bytes!(
                "../../../fixtures/variables/varbit_definitions.json"
            ))
        )
    );
}
