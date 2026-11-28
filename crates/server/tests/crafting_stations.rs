#[path = "../../../oracle/crafting-stations/native.rs"]
mod native;
use serde_json::Value;
use sha2::{Digest, Sha256};

const CORPUS: &str = include_str!("../../../fixtures/crafting-stations/oracle.jsonl");
const PROVENANCE: &str = include_str!("../../../fixtures/crafting-stations/provenance.json");

#[test]
fn the_fixture_is_the_one_its_provenance_names() {
    let provenance: Value = serde_json::from_str(PROVENANCE).unwrap();
    let digest: String = Sha256::digest(CORPUS.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert_eq!(provenance["fixtureSha256"], digest);
    assert_eq!(
        provenance["caseCount"].as_u64().unwrap() as usize,
        CORPUS.lines().count()
    );
}

/// Every original spinning and silver casting case replayed over the native library with the same declared
/// state.
#[test]
fn native_crafting_stations_match_the_original_cases() {
    let mut operations = 0;
    for line in CORPUS.lines() {
        let case: Value = serde_json::from_str(line).unwrap();
        let actual = native::replay(&case);
        let (expected, actual) = (
            case["expected"].as_array().unwrap(),
            actual.as_array().unwrap(),
        );
        assert_eq!(expected.len(), actual.len(), "case {}", case["id"]);
        for (n, (e, a)) in expected.iter().zip(actual).enumerate() {
            assert_eq!(e, a, "case {} operation {n} {}", case["id"], case["ops"][n]);
            operations += 1;
        }
    }
    assert!(operations > 8000, "{operations} operations compared");
}
