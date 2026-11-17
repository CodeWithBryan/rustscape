#[path = "../../../oracle/inventory-listener/native.rs"]
mod native;
use serde_json::Value;
use sha2::{Digest, Sha256};
const CORPUS: &str = include_str!("../../../fixtures/inventory-listener/oracle.jsonl");
#[test]
fn original_inventory_listener_packets_live_arrays_receivers_and_failure_prefixes() {
    for (i, line) in CORPUS.lines().enumerate() {
        let row: Value = serde_json::from_str(line).unwrap();
        assert_eq!(
            native::evaluate(&row),
            row["expected"],
            "inventory listener case{i}:op={} hooks={}",
            row["op"],
            row["hooks"]
        );
        let expected: Vec<i32> = row["pouchKeys"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_i64().unwrap() as i32)
            .collect();
        assert_eq!(rs09_server::inventory::pouch_ids::IDS, expected);
    }
}
#[test]
fn inventory_listener_original_source_and_fixture_bytes_are_pinned() {
    let p: Value = serde_json::from_str(include_str!(
        "../../../fixtures/inventory-listener/provenance.json"
    ))
    .unwrap();
    assert_eq!(p["caseCount"], CORPUS.lines().count());
    assert_eq!(p["fixtureSha256"], format!("{:x}", Sha256::digest(CORPUS)));
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    for (name, digest) in p["sources"].as_object().unwrap() {
        let path = root.join(name);
        if !path.is_file() {
            continue;
        }
        let data: Vec<_> = std::fs::read(path)
            .unwrap()
            .into_iter()
            .filter(|b| *b != b'\r')
            .collect();
        assert_eq!(
            digest.as_str().unwrap(),
            format!("{:x}", Sha256::digest(data)),
            "{name}"
        );
    }
}
