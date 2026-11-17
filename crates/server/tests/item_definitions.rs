#[path = "../../../oracle/items/native.rs"]
mod native;
use serde_json::Value;
use sha2::{Digest, Sha256};
const CORPUS: &str = include_str!("../../../fixtures/items/oracle.jsonl");

#[test]
fn cache_decoder_matches_original_fields_read_positions_and_failures() {
    check("decode");
}
#[test]
fn template_transfers_match_original_mutations_aliases_and_bulk_loads() {
    check("sequence");
}
#[test]
fn jdk_case_comparison_matches_code_units_and_original_strings() {
    for line in CORPUS.lines() {
        let case: Value = serde_json::from_str(line).unwrap();
        if case["kind"] == "casePairs" || case["kind"] == "caseCompare" {
            assert_eq!(native::evaluate(&case), case["expected"], "{case}");
        }
    }
}
#[test]
fn archive_record_limit_preserves_existing_registry() {
    use rs09_cache::{
        Limits,
        archive::Archive,
        store::{StorageMode, Store},
    };
    use rs09_server::definitions::item::{ItemDefinition, Registry, Shared};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let archive = Archive::open(
        Store::open(
            &root.join("fixtures/cache"),
            StorageMode::Resident,
            Limits::default(),
        )
        .unwrap(),
        3,
        None,
        Limits::default(),
    )
    .unwrap();
    let mut registry = Registry::default();
    let existing = Shared::new(ItemDefinition::default());
    registry.insert(42, existing.clone());
    assert!(registry.load_archive(&archive, 0).is_err());
    assert_eq!(registry.definitions().len(), 1);
    assert!(registry.for_id(42).same(&existing));
}
fn check(kind: &str) {
    let mut count = 0;
    for (index, line) in CORPUS.lines().enumerate() {
        let case: Value = serde_json::from_str(line).unwrap();
        if case["kind"] == kind {
            assert_eq!(
                native::evaluate(&case),
                case["expected"],
                "item case {index}: {}",
                serde_json::to_string(&case).unwrap()
            );
            count += 1;
        }
    }
    assert!(count > 100, "empty or unexpectedly small item corpus");
}
#[test]
fn item_fixtures_and_reference_hashes_are_pinned() {
    let provenance: Value =
        serde_json::from_str(include_str!("../../../fixtures/items/provenance.json")).unwrap();
    assert_eq!(
        format!("{:x}", Sha256::digest(CORPUS.as_bytes())),
        provenance["fixtureSha256"]
    );
    assert_eq!(
        CORPUS.lines().count() as u64,
        provenance["caseCount"].as_u64().unwrap()
    );
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    for (source, expected) in provenance["sources"].as_object().unwrap() {
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
