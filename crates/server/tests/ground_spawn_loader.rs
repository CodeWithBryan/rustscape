#[path = "../../../oracle/chunk-sync/native.rs"]
mod native;
use serde_json::Value;
use sha2::{Digest, Sha256};
const CORPUS: &str = include_str!("../../../fixtures/ground-spawn-loader/oracle.jsonl");
fn difference(a: &Value, b: &Value, path: String) -> Option<String> {
    if a == b {
        return None;
    }
    if let (Some(a), Some(b)) = (a.as_object(), b.as_object()) {
        for k in a.keys().chain(b.keys()) {
            if let Some(d) = difference(
                a.get(k).unwrap_or(&Value::Null),
                b.get(k).unwrap_or(&Value::Null),
                format!("{path}/{k}"),
            ) {
                return Some(d);
            }
        }
    }
    if let (Some(a), Some(b)) = (a.as_array(), b.as_array()) {
        for (i, (a, b)) in a.iter().zip(b).enumerate() {
            if let Some(d) = difference(a, b, format!("{path}/{i}")) {
                return Some(d);
            }
        }
        return Some(format!("{path}: lengths {} != {}", a.len(), b.len()));
    }
    Some(format!("{path}: native={a}, original={b}"))
}
#[test]
fn original_complete_loader_pinned_data_partial_rows_and_reentrant_failures() {
    for (index, line) in CORPUS.lines().enumerate() {
        let case: Value = serde_json::from_str(line).unwrap();
        let actual = native::evaluate(&case);
        if let Some(diff) = difference(&actual, &case["expected"], String::new()) {
            panic!("loader case {index}: {diff}");
        }
    }
}
#[test]
fn complete_loader_fixture_and_original_sources_are_pinned() {
    let p: Value = serde_json::from_str(include_str!(
        "../../../fixtures/ground-spawn-loader/provenance.json"
    ))
    .unwrap();
    assert_eq!(p["caseCount"], CORPUS.lines().count());
    assert_eq!(p["fixtureSha256"], format!("{:x}", Sha256::digest(CORPUS)));
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (n, h) in p["sources"].as_object().unwrap() {
        let path = root.join(n);
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
