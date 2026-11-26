#[path = "../../../oracle/scenery-definitions/native.rs"]
mod native;
use serde_json::Value;
use sha2::{Digest, Sha256};
const CORPUS: &str = include_str!("../../../fixtures/scenery-definitions/oracle.jsonl");
#[test]
fn archive_definition_limit_leaves_existing_handles_and_logs_untouched() {
    use rs09_cache::{
        Limits,
        archive::Archive,
        store::{StorageMode, Store},
    };
    use rs09_server::definitions::{
        item::Shared,
        scenery::{Registry, SceneryDefinition},
    };
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let limits = Limits::default();
    let archive = Archive::open(
        Store::open(&root.join("fixtures/cache"), StorageMode::Resident, limits).unwrap(),
        3,
        None,
        limits,
    )
    .unwrap();
    let old = Shared::new(SceneryDefinition::default());
    let mut registry = Registry::default();
    registry.definitions.insert(42, old.clone());
    let mut context = native::Model::default();
    assert!(registry.load_archive(&mut context, &archive, 0).is_err());
    assert_eq!(registry.definitions.len(), 1);
    assert!(registry.for_id(42).same(&old));
    assert_eq!(native::events(&context), serde_json::json!([]));
}
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
    if let (Some(a), Some(b)) = (a.as_array(), b.as_array())
        && a.len() == b.len()
    {
        for (i, (a, b)) in a.iter().zip(b).enumerate() {
            if let Some(d) = diff(a, b, format!("{path}/{i}")) {
                return Some(d);
            }
        }
    }
    Some(format!("{path}: native={a}, original={b}"))
}
#[test]
fn scenery_cache_decoder_registry_children_and_queries_match_original() {
    for (i, line) in CORPUS.lines().enumerate() {
        let c: Value = serde_json::from_str(line).unwrap();
        if let Some(d) = diff(&native::evaluate(&c), &c["expected"], String::new()) {
            panic!("scenery-definition case {i}: {d}; actions={}", c["actions"]);
        }
    }
}
#[test]
fn scenery_definition_sources_and_fixture_bytes_are_pinned() {
    let p: Value = serde_json::from_str(include_str!(
        "../../../fixtures/scenery-definitions/provenance.json"
    ))
    .unwrap();
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
            let data: Vec<_> = std::fs::read(&path)
                .unwrap()
                .into_iter()
                .filter(|b| *b != b'\r')
                .collect();
            assert_eq!(
                format!("{:x}", Sha256::digest(data)),
                expected.as_str().unwrap(),
                "{}",
                path.display()
            );
        }
    }
}
