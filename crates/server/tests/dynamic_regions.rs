#[path = "../../../oracle/dynamic-regions/native.rs"]
mod native;
use serde_json::Value;
use sha2::{Digest, Sha256};

const CORPUS: &str = include_str!("../../../fixtures/dynamic-regions/oracle.jsonl");
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
fn original_dynamic_regions_over_real_cache_regions() {
    let mut failures = Vec::new();
    let mut cases = 0;
    let mut pinned = 0;
    for (index, line) in CORPUS.lines().enumerate() {
        let case: Value = serde_json::from_str(line).unwrap();
        let mut native = native::evaluate(&case, CACHE, index);
        let mut expected = case["expected"].clone();
        // The one pinned difference: toggleMulticombat on an instance that was
        // never given borders. The original stores a RegionZone with null
        // borders (and fails later, at the first zone check); the native
        // RegionZone has no null borders, so the toggle is refused with Null
        // and the case is compared up to that step. A closed gap fails here.
        if case["name"] == "an unregistered copy" {
            assert_eq!(
                native["steps"][2]["result"],
                serde_json::json!({"error": "Null"})
            );
            assert_eq!(
                expected["steps"][2]["result"],
                serde_json::json!({"value": null})
            );
            native["steps"].as_array_mut().unwrap().truncate(2);
            expected["steps"].as_array_mut().unwrap().truncate(2);
            pinned += 1;
        }
        if let Some(found) = difference(&native, &expected, String::new()) {
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
    assert_eq!(cases, 36);
    assert_eq!(pinned, 1);
}

#[test]
fn dynamic_regions_fixture_and_original_sources_are_pinned() {
    let provenance: Value = serde_json::from_str(include_str!(
        "../../../fixtures/dynamic-regions/provenance.json"
    ))
    .unwrap();
    assert_eq!(provenance["caseCount"], CORPUS.lines().count());
    assert_eq!(
        provenance["fixtureSha256"],
        format!("{:x}", Sha256::digest(CORPUS))
    );
    assert_eq!(
        provenance["cacheSha256"],
        format!("{:x}", Sha256::digest(CACHE))
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
