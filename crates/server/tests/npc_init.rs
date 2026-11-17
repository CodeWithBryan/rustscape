#[path = "../../../oracle/npc-init/native.rs"]
mod native;
use serde_json::Value;
use sha2::{Digest, Sha256};
const CORPUS: &str = include_str!("../../../fixtures/npc-init/oracle.jsonl");
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
    Some(format!("{path}: native={a}, JVM={b}"))
}
#[test]
fn original_npc_factory_init_configuration_and_callback_prefixes() {
    for (i, l) in CORPUS.lines().enumerate() {
        let c: Value = serde_json::from_str(l).unwrap();
        if let Some(d) = diff(&native::evaluate(&c), &c["expected"], String::new()) {
            panic!("NPC init case{i}: {d}");
        }
    }
}
#[test]
fn npc_init_sources_and_fixture_are_pinned() {
    let p: Value =
        serde_json::from_str(include_str!("../../../fixtures/npc-init/provenance.json")).unwrap();
    assert_eq!(p["caseCount"], CORPUS.lines().count());
    assert_eq!(p["fixtureSha256"], format!("{:x}", Sha256::digest(CORPUS)));
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
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
