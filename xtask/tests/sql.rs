use rs09_game_core::java_map;
use rs09_server::auth::storage::AccountStorage;
use rs09_server::auth::{
    account::{FieldValue, UserAccountInfo},
    sql::{self, SqlConfig, SqlStorage},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const CORPUS: &str = include_str!("../../fixtures/sql/oracle.jsonl");
#[test]
fn sql_connection_failure_does_not_sample_a_record_timestamp_before_query_execution() {
    let storage = SqlStorage::unconfigured(|_| false);
    let reads = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let observed = reads.clone();
    let clock = move || {
        observed.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        100
    };
    assert!(
        storage
            .get_account_info_with_clock("alice", &clock)
            .is_err()
    );
    assert_eq!(reads.load(std::sync::atomic::Ordering::Relaxed), 0);
}
fn cases(kind: &str) -> Vec<Value> {
    CORPUS
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .filter(|case| case["kind"] == kind)
        .collect()
}
fn field(value: &FieldValue) -> Value {
    match value {
        FieldValue::Text(text) => json!(text),
        FieldValue::Int(n) => json!(n),
        FieldValue::Long(n) => json!(n),
        FieldValue::Bool(b) => json!(b),
        FieldValue::Timestamp(stamp) => {
            json!({"millis":stamp.millis(),"nanos":stamp.value().nanos})
        }
    }
}
fn snapshot(info: &UserAccountInfo) -> Value {
    json!({"values":info.values().iter().map(field).collect::<Vec<_>>(),"initial":info.initial_values.as_ref().map(|values|values.iter().map(field).collect::<Vec<_>>()),"changed":info.changed_fields().ok().map(|(changed,_)|changed),"hash":info.java_hash(),"default":info.is_default()})
}
#[test]
fn sql_fixtures_pin_the_original_provider_schema_and_real_database_harness() {
    let metadata: Value =
        serde_json::from_str(include_str!("../../fixtures/sql/provenance.json")).unwrap();
    assert_eq!(
        metadata["upstream"],
        serde_json::from_str::<Value>(include_str!("../../upstream.lock.json")).unwrap()
    );
    assert_eq!(
        metadata["fixtureSha256"],
        format!("{:x}", Sha256::digest(CORPUS.as_bytes()))
    );
    assert_eq!(
        metadata["caseCount"].as_u64().unwrap() as usize,
        CORPUS.lines().count()
    );
    assert_eq!(
        ["sequence", "map_order", "contacts", "update_query"]
            .iter()
            .map(|kind| cases(kind).len())
            .sum::<usize>(),
        CORPUS.lines().count()
    );
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    for (path, expected) in metadata["sources"].as_object().unwrap() {
        if path.starts_with("oracle/") || path.starts_with("fixtures/") {
            let bytes: Vec<_> = std::fs::read(root.join(path))
                .unwrap()
                .into_iter()
                .filter(|&b| b != b'\r')
                .collect();
            assert_eq!(*expected, format!("{:x}", Sha256::digest(bytes)), "{path}");
        }
    }
    let schema: Value =
        serde_json::from_str(include_str!("../../fixtures/sql/schema.provenance.json")).unwrap();
    let bytes: Vec<_> = include_bytes!("../../fixtures/sql/schema.sql")
        .iter()
        .copied()
        .filter(|&b| b != b'\r')
        .collect();
    assert_eq!(schema["sha256"], format!("{:x}", Sha256::digest(bytes)));
}
#[test]
fn string_map_iteration_matches_jdk_resize_collision_and_tree_bin_paths() {
    for case in cases("map_order") {
        let keys = case["keys"]
            .as_array()
            .unwrap()
            .iter()
            .map(|key| key.as_str().unwrap().to_owned());
        assert_eq!(json!(java_map::string_key_order(keys)), case["expected"]);
    }
}
#[test]
fn contact_splitting_ranks_and_iteration_match_original_communication_parser() {
    for case in cases("contacts") {
        let result = sql::parse_contact_names(case["text"].as_str().unwrap());
        assert_eq!(result.is_err(), case["error"].as_bool().unwrap(), "{case}");
        if let Ok(names) = result {
            assert_eq!(json!(names), case["expected"], "{case}");
        }
    }
}
#[test]
fn sql_update_column_map_retains_original_omissions_and_order() {
    for case in cases("update_query") {
        let indices: Vec<_> = case["indices"]
            .as_array()
            .unwrap()
            .iter()
            .map(|index| index.as_i64().unwrap() as usize)
            .collect();
        assert_eq!(sql::build_update_query(&indices), case["expected"]);
    }
}
fn sql_row(row: &Value) -> Vec<mysql::Value> {
    row.as_array()
        .unwrap()
        .iter()
        .enumerate()
        .map(|(column, value)| {
            if value.is_null() {
                return mysql::Value::NULL;
            }
            match column {
                2 => mysql::Value::UInt(value.as_u64().unwrap()),
                3 | 4 | 7 | 8 | 14 | 15 | 16 => mysql::Value::Int(value.as_i64().unwrap()),
                17 => {
                    let parts: Vec<_> = value
                        .as_str()
                        .unwrap()
                        .split(['-', ' ', ':', '.'])
                        .map(|part| part.parse::<u32>().unwrap())
                        .collect();
                    mysql::Value::Date(
                        parts[0] as u16,
                        parts[1] as u8,
                        parts[2] as u8,
                        parts[3] as u8,
                        parts[4] as u8,
                        parts[5] as u8,
                        parts[6],
                    )
                }
                _ => mysql::Value::Bytes(value.as_str().unwrap().as_bytes().to_vec()),
            }
        })
        .collect()
}
#[test]
fn decoded_sql_rows_match_jdbc_null_defaults_ranges_timestamps_and_booleans() {
    let mut checked = 0;
    for case in cases("sequence") {
        let mut now = case["now"].as_i64().unwrap();
        for (index, action) in case["actions"].as_array().unwrap().iter().enumerate() {
            if action["op"] == "clock" {
                now = action["value"].as_i64().unwrap();
            }
            if action["op"] != "get" {
                continue;
            }
            let observed = &case["expected"][index];
            let Some(rows) = observed["rows"].as_array() else {
                continue;
            };
            let key = action["key"].as_str().unwrap();
            let Some(row) = rows.iter().find(|row| {
                row[0]
                    .as_str()
                    .is_some_and(|name| name.eq_ignore_ascii_case(key))
            }) else {
                continue;
            };
            let native = sql::decode_row(key, &sql_row(row), now);
            assert_eq!(
                native.is_err(),
                observed["error"].as_bool().unwrap(),
                "{action}"
            );
            if let Ok(info) = native {
                assert_eq!(
                    snapshot(&info),
                    observed["records"][action["id"].as_str().unwrap()]
                );
            }
            checked += 1;
        }
    }
    assert!(checked > 20);
}
#[test]
fn zero_connection_budget_is_rejected_without_opening_a_socket() {
    let mut config = SqlConfig::new(
        "127.0.0.1".into(),
        1,
        "unused".into(),
        "unused".into(),
        String::new(),
    );
    config.max_connections = 0;
    assert!(SqlStorage::new(config, |_| false).is_err());
}
