#[path = "../../../oracle/npc-death/native.rs"]
mod native;
use rs09_server::{
    definitions::npc::{Files, Registry, Result as NpcResult},
    world::drop_tables::{
        LogLevel, NpcDropTable, WeightBasedTable,
        definitions::NativeDefinitions,
        drop,
        parser::{self, Definitions},
        random::JavaSource,
    },
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::sync::Arc;

const CORPUS: &str = include_str!("../../../fixtures/npc-death/oracle.jsonl");
const TABLES: &str = include_str!("../../../fixtures/drop-tables/oracle.jsonl");
const PINNED: &str = include_str!("../../../fixtures/drop-tables/drop_tables.json");

fn difference(a: &Value, b: &Value, path: String) -> Option<String> {
    if a == b {
        return None;
    }
    if let (Some(a), Some(b)) = (a.as_object(), b.as_object()) {
        for key in a.keys().chain(b.keys()) {
            if let Some(found) = difference(
                a.get(key).unwrap_or(&Value::Null),
                b.get(key).unwrap_or(&Value::Null),
                format!("{path}/{key}"),
            ) {
                return Some(found);
            }
        }
    }
    if let (Some(a), Some(b)) = (a.as_array(), b.as_array()) {
        for (index, (a, b)) in a.iter().zip(b).enumerate() {
            if let Some(found) = difference(a, b, format!("{path}/{index}")) {
                return Some(found);
            }
        }
        return Some(format!("{path}: lengths {} != {}", a.len(), b.len()));
    }
    Some(format!("{path}: native={a}, original={b}"))
}

#[test]
fn original_drops_looters_kill_credit_and_death_sequences() {
    let mut actions = 0;
    for (index, line) in CORPUS.lines().enumerate() {
        let case: Value = serde_json::from_str(line).unwrap();
        let native = native::evaluate(&case);
        if let Some(found) = difference(&native, &case["expected"], String::new()) {
            panic!(
                "npc death case {index}: {found}\nactions: {}",
                case["actions"]
            );
        }
        actions += native.as_array().unwrap().len();
    }
    assert!(actions > 0);
}

#[test]
fn npc_death_fixture_and_original_sources_are_pinned() {
    let provenance: Value =
        serde_json::from_str(include_str!("../../../fixtures/npc-death/provenance.json")).unwrap();
    assert_eq!(provenance["caseCount"], CORPUS.lines().count());
    assert_eq!(
        provenance["fixtureSha256"],
        format!("{:x}", Sha256::digest(CORPUS))
    );
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut checked = 0;
    for (path, hash) in provenance["sources"].as_object().unwrap() {
        let path = root.join(path);
        if path.is_file() {
            let bytes: Vec<_> = std::fs::read(&path)
                .unwrap()
                .into_iter()
                .filter(|byte| *byte != b'\r')
                .collect();
            assert_eq!(
                format!("{:x}", Sha256::digest(bytes)),
                hash.as_str().unwrap(),
                "{}",
                path.display()
            );
            checked += 1;
        }
    }
    // The harness, its adapters and the generator are always present.
    assert!(checked >= 4);
}

struct NoCache;
impl Files for NoCache {
    fn file(&mut self, _: &mut Registry, _: i32) -> NpcResult<Option<Vec<u8>>> {
        Ok(None)
    }
}

fn bits(value: f64) -> i64 {
    value.to_bits() as i64
}
fn table_snapshot(table: &WeightBasedTable) -> Value {
    let rows = |items: &[rs09_server::world::drop_tables::WeightedItem]| -> Value {
        items
            .iter()
            .map(|item| {
                json!([
                    item.id,
                    item.min_amt,
                    item.max_amt,
                    bits(item.weight),
                    item.guaranteed
                ])
            })
            .collect()
    };
    json!({
        "items": rows(&table.items),
        "total": bits(table.total_weight),
        "guaranteed": rows(&table.guaranteed_items),
    })
}

/// The complete pinned configuration, loaded onto native NPC definitions,
/// holds what the original definitions hold in the drop-table corpus: the
/// same table for every id, shared between the ids of one row.
#[test]
fn pinned_tables_are_stored_on_native_npc_definitions() {
    let expected = TABLES
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .find(|case| {
            case["actions"][0]["op"] == "load"
                && case["actions"][0]["pinned"] == true
                && case["seeds"]["random"] == 530
                && case["actions"].as_array().unwrap().len() == 2
        })
        .expect("the pinned load case");
    let expected = &expected["expected"]["steps"][1]["value"];

    let mut registry = Registry::default();
    let mut logged = Vec::new();
    let mut source = JavaSource::new(530, 531, 532);
    let count = parser::load(
        PINNED,
        &mut source,
        &mut NativeDefinitions {
            registry: &mut registry,
            files: &mut NoCache,
            log: &mut |level, message| logged.push((level, message.to_owned())),
        },
    )
    .unwrap();
    assert_eq!(
        logged,
        [(LogLevel::Fine, format!("Parsed {count} drop tables."))]
    );

    let mut ids: Vec<_> = registry.definitions.keys().copied().collect();
    ids.sort_unstable();
    let mut seen: Vec<Arc<NpcDropTable>> = Vec::new();
    let mut npcs = Vec::new();
    for id in ids {
        let definition = registry.definitions[&id].read();
        let tables = definition.drop_tables.as_ref().expect("drop tables");
        let index = seen
            .iter()
            .position(|table| Arc::ptr_eq(table, &tables.table))
            .unwrap_or_else(|| {
                seen.push(tables.table.clone());
                seen.len() - 1
            });
        npcs.push(json!([id, index]));
    }
    let tables: Vec<_> = seen
        .iter()
        .map(|table| {
            let mut out = table_snapshot(&table.main);
            out["charms"] = table_snapshot(table.charm_drops());
            out["tertiary"] = table_snapshot(table.tertiary_drops());
            out
        })
        .collect();
    assert_eq!(npcs.len(), 2152);
    if let Some(found) = difference(
        &json!({"tables": tables, "npcs": npcs}),
        expected,
        String::new(),
    ) {
        panic!("{found}");
    }
}

/// A definition whose drop tables were set to null is passed over and not
/// counted, and a new definition starts with an empty table of its own.
#[test]
fn definitions_without_drop_tables_are_passed_over() {
    let mut registry = Registry::default();
    let first = registry.for_id(&mut NoCache, 1).unwrap();
    let second = registry.for_id(&mut NoCache, 2).unwrap();
    {
        let first = first.read();
        let second = second.read();
        let (a, b) = (
            first.drop_tables.as_ref().unwrap(),
            second.drop_tables.as_ref().unwrap(),
        );
        assert!(!Arc::ptr_eq(&a.table, &b.table));
        assert!(a.table.main.items.is_empty() && a.table.main.guaranteed_items.is_empty());
    }
    second.write().drop_tables = None;
    let mut source = JavaSource::new(1, 2, 3);
    let mut definitions = NativeDefinitions {
        registry: &mut registry,
        files: &mut NoCache,
        log: &mut |_, _| {},
    };
    let text = r#"[{"ids":"1,2,3","main":[{"id":"995","minAmount":"1","maxAmount":"5","weight":"2.0"}],"charm":[],"default":[]}]"#;
    assert_eq!(
        parser::load(text, &mut source, &mut definitions).unwrap(),
        2
    );
    assert_eq!(definitions.name(3).unwrap().as_deref(), Some("null"));
    let third = registry.for_id(&mut NoCache, 3).unwrap();
    assert!(second.read().drop_tables.is_none());
    assert!(Arc::ptr_eq(
        &first.read().drop_tables.as_ref().unwrap().table,
        &third.read().drop_tables.as_ref().unwrap().table
    ));
    assert_eq!(
        first
            .read()
            .drop_tables
            .as_ref()
            .unwrap()
            .table
            .main
            .items
            .len(),
        1
    );
}

#[test]
fn native_bounds_and_authored_constants() {
    // Not original behaviour: the bound on one split drop.
    assert_eq!(drop::MAX_SPLIT, 65_536);
    assert_eq!(drop::bone_experience(536), Some(72.0));
    assert_eq!(drop::bone_experience(995), None);
    assert_eq!(drop::formatted_number(1_234_567), "1,234,567");
}
