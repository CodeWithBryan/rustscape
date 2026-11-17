#[path = "../../../oracle/timers/native.rs"]
mod native;
use serde_json::Value;
use sha2::{Digest, Sha256};
const CORPUS: &str = include_str!("../../../fixtures/timers/oracle.jsonl");
#[test]
fn timer_scheduling_callbacks_persistence_and_partial_failures_match_original() {
    for (index, line) in CORPUS.lines().enumerate() {
        let case: Value = serde_json::from_str(line).unwrap();
        let actual = native::evaluate(&case);
        let expected = case["expected"].as_array().unwrap();
        for (step, (a, b)) in actual.as_array().unwrap().iter().zip(expected).enumerate() {
            assert_eq!(
                a, b,
                "timer case {index} step {step}, action {}",
                case["actions"][step]
            );
        }
        assert_eq!(actual.as_array().unwrap().len(), expected.len());
    }
}
#[test]
fn timer_fixtures_pin_complete_original_classes_and_explicit_adapters() {
    let p: Value =
        serde_json::from_str(include_str!("../../../fixtures/timers/provenance.json")).unwrap();
    assert_eq!(
        format!("{:x}", Sha256::digest(CORPUS.as_bytes())),
        p["fixtureSha256"]
    );
    assert_eq!(
        CORPUS.lines().count() as u64,
        p["caseCount"].as_u64().unwrap()
    );
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    for (source, expected) in p["sources"].as_object().unwrap() {
        let path = root.join(source);
        if path.is_file() {
            let data = std::fs::read(path)
                .unwrap()
                .into_iter()
                .filter(|b| *b != b'\r')
                .collect::<Vec<_>>();
            assert_eq!(
                format!("{:x}", Sha256::digest(data)),
                expected.as_str().unwrap(),
                "{source}"
            );
        }
    }
}
