#[path = "../../../oracle/zones/native.rs"]
mod native;
use serde_json::Value;
use sha2::{Digest, Sha256};

const CORPUS: &str = include_str!("../../../fixtures/zones/oracle.jsonl");

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
fn original_zones_monitors_and_region_activity() {
    let mut failures = Vec::new();
    let mut cases = 0;
    for (index, line) in CORPUS.lines().enumerate() {
        let case: Value = serde_json::from_str(line).unwrap();
        let native = native::evaluate(&case);
        if let Some(found) = difference(&native, &case["expected"], String::new()) {
            failures.push(format!("case {index} ({}): {found}", case["group"]));
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
    assert_eq!(cases, 1019);
}

#[test]
fn zones_fixture_and_original_sources_are_pinned() {
    let provenance: Value =
        serde_json::from_str(include_str!("../../../fixtures/zones/provenance.json")).unwrap();
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
    // The harness, its adapters, its cases and the generators are always present.
    assert!(checked >= 5);
}
