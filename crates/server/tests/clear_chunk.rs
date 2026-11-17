#[path = "../../../oracle/chunk-sync/native.rs"]
mod native;
use serde_json::Value;
use sha2::{Digest, Sha256};

const CORPUS: &str = include_str!("../../../fixtures/clear-chunk/oracle.jsonl");
fn difference(a: &Value, b: &Value, path: String) -> Option<String> {
    if a == b {
        return None;
    }
    if let (Some(a), Some(b)) = (a.as_object(), b.as_object()) {
        for key in a.keys().chain(b.keys()) {
            if let Some(d) = difference(
                a.get(key).unwrap_or(&Value::Null),
                b.get(key).unwrap_or(&Value::Null),
                format!("{path}/{key}"),
            ) {
                return Some(d);
            }
        }
    }
    if let (Some(a), Some(b)) = (a.as_array(), b.as_array()) {
        for (index, (a, b)) in a.iter().zip(b).enumerate() {
            if let Some(d) = difference(a, b, format!("{path}/{index}")) {
                return Some(d);
            }
        }
        return Some(format!("{path}: lengths {} != {}", a.len(), b.len()));
    }
    Some(format!("{path}: native={a}, original={b}"))
}
#[test]
fn original_clear_chunk_bounds_captured_scene_live_base_and_failed_prefixes() {
    for (index, line) in CORPUS.lines().enumerate() {
        let case: Value = serde_json::from_str(line).unwrap();
        let actual = native::evaluate(&case);
        if let Some(d) = difference(&actual, &case["expected"], String::new()) {
            panic!(
                "clear chunk case {index}: {d}; events native={} original={}",
                actual["events"], case["expected"]["events"]
            );
        }
    }
}
#[test]
fn clear_chunk_fixture_and_original_sources_are_pinned() {
    let provenance: Value = serde_json::from_str(include_str!(
        "../../../fixtures/clear-chunk/provenance.json"
    ))
    .unwrap();
    assert_eq!(provenance["caseCount"], CORPUS.lines().count());
    assert_eq!(
        provenance["fixtureSha256"],
        format!("{:x}", Sha256::digest(CORPUS))
    );
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (path, hash) in provenance["sources"].as_object().unwrap() {
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
