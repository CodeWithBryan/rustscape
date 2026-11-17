#[path = "../../../oracle/map-chunk-render/native.rs"]
mod native;
use serde_json::Value;
use sha2::{Digest, Sha256};
const CORPUS: &str = include_str!("../../../fixtures/map-chunk-render/oracle.jsonl");
fn diff(a: &Value, b: &Value, path: String) -> Option<String> {
    if a == b {
        return None;
    }
    if let (Some(a), Some(b)) = (a.as_object(), b.as_object()) {
        for key in a.keys().chain(b.keys()) {
            if let Some(d) = diff(
                a.get(key).unwrap_or(&Value::Null),
                b.get(key).unwrap_or(&Value::Null),
                format!("{path}/{key}"),
            ) {
                return Some(d);
            }
        }
    }
    if let (Some(a), Some(b)) = (a.as_array(), b.as_array()) {
        for (i, (a, b)) in a.iter().zip(b).enumerate() {
            if let Some(d) = diff(a, b, format!("{path}/{i}")) {
                return Some(d);
            }
        }
        return Some(format!(
            "{path}: native length={}, original length={}",
            a.len(),
            b.len()
        ));
    }
    Some(format!("{path}:native={a},original={b}"))
}
#[test]
fn original_viewport_traversal_identity_planes_and_partial_publication() {
    for (i, line) in CORPUS.lines().enumerate() {
        let c: Value = serde_json::from_str(line).unwrap();
        if let Some(d) = diff(&native::evaluate(&c), &c["expected"], String::new()) {
            panic!("map viewport case{i}:{d}");
        }
    }
}
#[test]
fn map_viewport_fixture_and_original_sources_are_pinned() {
    let p: Value = serde_json::from_str(include_str!(
        "../../../fixtures/map-chunk-render/provenance.json"
    ))
    .unwrap();
    assert_eq!(p["caseCount"], CORPUS.lines().count());
    assert_eq!(p["fixtureSha256"], format!("{:x}", Sha256::digest(CORPUS)));
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (path, hash) in p["sources"].as_object().unwrap() {
        let p = root.join(path);
        if p.is_file() {
            let b: Vec<_> = std::fs::read(&p)
                .unwrap()
                .into_iter()
                .filter(|b| *b != b'\r')
                .collect();
            assert_eq!(
                format!("{:x}", Sha256::digest(b)),
                hash.as_str().unwrap(),
                "{}",
                p.display()
            );
        }
    }
}
