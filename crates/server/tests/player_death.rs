#[path = "../../../oracle/player-death/native.rs"]
mod native;
use serde_json::Value;

const CORPUS: &str = include_str!("../../../fixtures/player-death/oracle.jsonl");

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
fn original_kept_items_deaths_graves_and_server_store() {
    let mut cases = 0;
    let mut actions = 0;
    for (index, line) in CORPUS.lines().enumerate() {
        let case: Value = serde_json::from_str(line).unwrap();
        let native = native::evaluate(&case);
        if let Some(found) = difference(&native, &case["expected"], String::new()) {
            panic!(
                "player death case {index}: {found}\nactions: {}",
                case["actions"]
            );
        }
        cases += 1;
        actions += native.as_array().unwrap().len();
    }
    assert_eq!((cases, actions), (938, 3216));
}
