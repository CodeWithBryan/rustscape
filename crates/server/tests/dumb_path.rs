#[path = "../../../oracle/dumb-path/native.rs"]
mod native;
use serde_json::Value;
use sha2::{Digest, Sha256};
const CORPUS: &str = include_str!("../../../fixtures/dumb-path/oracle.jsonl");
#[test]
fn original_dumb_paths_ordered_clipping_state_reentry_and_failures() {
    for (i, line) in CORPUS.lines().enumerate() {
        let c: Value = serde_json::from_str(line).unwrap();
        let actual = native::evaluate(&c);
        assert_eq!(actual, c["expected"], "dumb case{i}");
    }
}
#[test]
fn dumb_fixture_and_source_hashes_are_pinned() {
    let p: Value =
        serde_json::from_str(include_str!("../../../fixtures/dumb-path/provenance.json")).unwrap();
    assert_eq!(p["caseCount"], CORPUS.lines().count());
    assert_eq!(p["fixtureSha256"], format!("{:x}", Sha256::digest(CORPUS)));
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    for (path, hash) in p["sources"].as_object().unwrap() {
        let path = root.join(path);
        if path.is_file() {
            let bytes: Vec<_> = std::fs::read(&path)
                .unwrap()
                .into_iter()
                .filter(|b| *b != b'\r')
                .collect();
            assert_eq!(
                format!("{:x}", Sha256::digest(bytes)),
                hash.as_str().unwrap(),
                "{}",
                path.display()
            );
        }
    }
}
