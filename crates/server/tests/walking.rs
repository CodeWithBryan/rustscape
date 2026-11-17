#[path = "../../../oracle/walking/native.rs"]
mod native;
use serde_json::Value;
use sha2::{Digest, Sha256};
const CORPUS: &str = include_str!("../../../fixtures/walking/oracle.jsonl");
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
        if a.len() == b.len() {
            for (i, (a, b)) in a.iter().zip(b).enumerate() {
                if let Some(d) = diff(a, b, format!("{path}/{i}")) {
                    return Some(d);
                }
            }
        }
    }
    Some(format!("{path}: native={a}, original={b}"))
}
#[test]
fn unchanged_jvm_walking_queue_state_callbacks_aliases_and_failures() {
    for (i, line) in CORPUS.lines().enumerate() {
        let case: Value = serde_json::from_str(line).unwrap();
        if let Some(d) = diff(&native::evaluate(&case), &case["expected"], String::new()) {
            panic!("walking case{i}: {d}");
        }
    }
}
#[test]
fn walking_fixture_sources_and_bytes_are_pinned() {
    let p: Value =
        serde_json::from_str(include_str!("../../../fixtures/walking/provenance.json")).unwrap();
    assert_eq!(p["caseCount"], CORPUS.lines().count());
    assert_eq!(p["fixtureSha256"], format!("{:x}", Sha256::digest(CORPUS)));
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    for (path, expected) in p["sources"].as_object().unwrap() {
        let path = root.join(path);
        if path.is_file() {
            let bytes: Vec<_> = std::fs::read(&path)
                .unwrap()
                .into_iter()
                .filter(|b| *b != b'\r')
                .collect();
            assert_eq!(
                format!("{:x}", Sha256::digest(bytes)),
                expected.as_str().unwrap(),
                "{}",
                path.display()
            );
        }
    }
}

#[test]
fn malformed_path_expansion_is_bounded_without_losing_the_existing_queue() {
    let states = native::evaluate(&serde_json::json!({"actions":[
        {"op":"reset"},{"op":"add","x":100_000,"y":3216}
    ]}));
    assert_eq!(states[1]["error"], "Limit");
    assert_eq!(states[1]["points"], states[0]["points"]);
}
