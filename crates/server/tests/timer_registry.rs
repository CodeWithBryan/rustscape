#[path = "../../../oracle/timers/native.rs"]
mod native;
use serde_json::Value;
use sha2::{Digest, Sha256};
const CORPUS: &str = include_str!("../../../fixtures/timer-registry/oracle.jsonl");
#[test]
fn registry_factories_automatic_timers_api_order_and_failures_match_original() {
    for (index, line) in CORPUS.lines().enumerate() {
        let case: Value = serde_json::from_str(line).unwrap();
        let actual = native::evaluate(&case);
        assert_eq!(
            actual.as_array().unwrap().len(),
            case["expected"].as_array().unwrap().len()
        );
        for (step, (a, b)) in actual
            .as_array()
            .unwrap()
            .iter()
            .zip(case["expected"].as_array().unwrap())
            .enumerate()
        {
            assert_eq!(
                a, b,
                "registry case {index} step {step}: {}",
                case["actions"][step]
            );
        }
    }
}
#[test]
fn timer_registry_fixtures_pin_complete_original_and_content_wrappers() {
    let p: Value = serde_json::from_str(include_str!(
        "../../../fixtures/timer-registry/provenance.json"
    ))
    .unwrap();
    assert_eq!(format!("{:x}", Sha256::digest(CORPUS)), p["fixtureSha256"]);
    assert_eq!(CORPUS.lines().count() as u64, p["caseCount"]);
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    for (name, expected) in p["sources"].as_object().unwrap() {
        let file = root.join(name);
        if file.is_file() {
            let bytes = std::fs::read(file)
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
