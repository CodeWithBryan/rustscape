#[path = "../../../oracle/inventory/native.rs"]
mod native;
use serde_json::Value;
use sha2::{Digest, Sha256};
const CORPUS: &str = include_str!("../../../fixtures/inventory/oracle.jsonl");
fn first_difference(a: &Value, b: &Value, path: String) -> Option<String> {
    if a == b {
        return None;
    }
    if let (Some(a), Some(b)) = (a.as_object(), b.as_object()) {
        for key in a.keys().chain(b.keys()) {
            if let Some(diff) = first_difference(
                a.get(key).unwrap_or(&Value::Null),
                b.get(key).unwrap_or(&Value::Null),
                format!("{path}/{key}"),
            ) {
                return Some(diff);
            }
        }
    }
    if let (Some(a), Some(b)) = (a.as_array(), b.as_array()) {
        if a.len() == b.len() {
            for (index, (a, b)) in a.iter().zip(b).enumerate() {
                if let Some(diff) = first_difference(a, b, format!("{path}/{index}")) {
                    return Some(diff);
                }
            }
        }
    }
    Some(format!("{path}: native={a}, original={b}"))
}
fn check(filter: impl Fn(&Value) -> bool) {
    let mut count = 0;
    for (index, line) in CORPUS.lines().enumerate() {
        let case: Value = serde_json::from_str(line).unwrap();
        if !filter(&case) {
            continue;
        }
        let actual = native::evaluate(&case);
        if let Some(diff) = first_difference(&actual, &case["expected"], String::new()) {
            panic!("inventory case {index}: {diff}");
        }
        count += 1;
    }
    assert!(count > 0);
}
#[test]
fn item_state_values_charge_note_and_copy_behavior_match_original() {
    check(|case| case["kind"] == "sequence" && case["containers"].as_array().unwrap().is_empty());
}
#[test]
fn container_mutations_events_aliases_and_callbacks_match_original() {
    check(|case| case["kind"] == "sequence" && !case["containers"].as_array().unwrap().is_empty());
}
#[test]
fn integer_hash_map_order_matches_jdk_tree_bins_and_resize() {
    check(|case| case["kind"] == "integerMap");
}
#[test]
fn inventory_fixtures_pin_complete_original_classes_and_adapter_scope() {
    let p: Value =
        serde_json::from_str(include_str!("../../../fixtures/inventory/provenance.json")).unwrap();
    assert_eq!(
        format!("{:x}", Sha256::digest(CORPUS.as_bytes())),
        p["fixtureSha256"]
    );
    assert_eq!(
        CORPUS.lines().count() as u64,
        p["caseCount"].as_u64().unwrap()
    );
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
#[test]
fn malformed_container_sizes_and_cyclic_notes_return_bounded_errors() {
    use rs09_server::{
        definitions::item::{ItemDefinition, Registry, Shared},
        inventory::{
            Error,
            container::{ContainerType, Factory, SortType},
            item::definition_alchemy,
        },
    };
    let mut definitions = Registry::default();
    let note = Shared::new(ItemDefinition {
        id: 1,
        unnoted: false,
        note_id: 1,
        ..ItemDefinition::default()
    });
    definitions.insert(1, note.clone());
    assert_eq!(
        definition_alchemy(&note, true, &definitions),
        Err(Error::CyclicNote)
    );
    let factory = Factory::new(&definitions);
    for capacity in [-1, i32::MAX] {
        assert!(matches!(
            factory.create(capacity, ContainerType::Default, SortType::Id),
            Err(Error::Bounds)
        ));
    }
}
