//! The activity and cutscene framework replayed against the unchanged
//! ActivityManager, ActivityPlugin, CutscenePlugin and Cutscene.kt over the
//! real cache regions of fixtures/dynamic-regions/cache.json.
#[path = "../../../oracle/activities/native.rs"]
mod native;
#[allow(dead_code)]
#[path = "../../../oracle/dynamic-regions/native.rs"]
mod regions;
use serde_json::Value;

const CORPUS: &str = include_str!("../../../fixtures/activities/oracle.jsonl");
const CACHE: &str = include_str!("../../../fixtures/dynamic-regions/cache.json");

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
fn original_activities_and_cutscenes_over_real_cache_regions() {
    let mut failures = Vec::new();
    let mut cases = 0;
    for (index, line) in CORPUS.lines().enumerate() {
        let case: Value = serde_json::from_str(line).unwrap();
        let native = native::evaluate(&case, CACHE);
        if let Some(found) = difference(&native, &case["expected"], String::new()) {
            failures.push(format!("case {index} ({}): {found}", case["name"]));
        }
        cases += 1;
    }
    assert!(
        failures.is_empty(),
        "{} of {cases} cases differ:\n{}",
        failures.len(),
        failures
            .iter()
            .take(25)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
    assert_eq!(cases, 513);
}
