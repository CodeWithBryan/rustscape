#[path = "../../../oracle/scripts/native.rs"]
mod native;
use serde_json::Value;
use sha2::{Digest, Sha256};
const CORPUS: &str = include_str!("../../../fixtures/scripts/oracle.jsonl");
fn diff(a: &Value, b: &Value, path: String) -> Option<String> {
    if a == b {
        return None;
    }
    if let (Some(a), Some(b)) = (a.as_object(), b.as_object()) {
        for k in a.keys().chain(b.keys()) {
            if let Some(d) = diff(
                a.get(k).unwrap_or(&Value::Null),
                b.get(k).unwrap_or(&Value::Null),
                format!("{path}/{k}"),
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
fn script_processor_queues_interactions_callbacks_and_api_match_original() {
    for (index, line) in CORPUS.lines().enumerate() {
        let c: Value = serde_json::from_str(line).unwrap();
        let actual = native::evaluate(&c);
        if let Some(d) = diff(&actual, &c["expected"], String::new()) {
            panic!("script case {index}: {d}; actions={}", c["actions"]);
        }
    }
}
#[test]
fn script_fixture_sources_and_explicit_world_bindings_are_pinned() {
    let p: Value =
        serde_json::from_str(include_str!("../../../fixtures/scripts/provenance.json")).unwrap();
    assert_eq!(format!("{:x}", Sha256::digest(CORPUS)), p["fixtureSha256"]);
    assert_eq!(CORPUS.lines().count() as u64, p["caseCount"]);
    let initial: Value =
        serde_json::from_str(include_str!("../../../oracle/scripts/initial.lock.json")).unwrap();
    assert_eq!(p["initialBaseline"], initial);
    assert_eq!(initial["caseCount"], 295);
    assert_eq!(
        initial["sha256"],
        "9116d5713e991288523af07dd392a6f248be0fb1ea647342881abb43e97749d1"
    );
    let prefix = CORPUS
        .split_inclusive('\n')
        .take(initial["caseCount"].as_u64().unwrap() as usize)
        .collect::<String>();
    assert_eq!(format!("{:x}", Sha256::digest(prefix)), initial["sha256"]);
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
