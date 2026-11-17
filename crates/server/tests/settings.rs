#[path = "../../../oracle/settings/native.rs"]
mod native;
use serde_json::Value;
use sha2::{Digest, Sha256};
const CORPUS: &str = include_str!("../../../fixtures/settings/oracle.jsonl");
fn difference(a: &Value, b: &Value, path: String) -> Option<String> {
    if a == b {
        return None;
    }
    if let (Some(a), Some(b)) = (a.as_object(), b.as_object()) {
        for key in a.keys().chain(b.keys()) {
            if let Some(diff) = difference(
                a.get(key).unwrap_or(&Value::Null),
                b.get(key).unwrap_or(&Value::Null),
                format!("{path}/{key}"),
            ) {
                return Some(diff);
            }
        }
    } else if let (Some(a), Some(b)) = (a.as_array(), b.as_array()) {
        if a.len() == b.len() {
            for (i, (a, b)) in a.iter().zip(b).enumerate() {
                if let Some(diff) = difference(a, b, format!("{path}/{i}")) {
                    return Some(diff);
                }
            }
        }
    }
    Some(format!(
        "{path}: expected {}, actual {}",
        a.to_string().chars().take(180).collect::<String>(),
        b.to_string().chars().take(180).collect::<String>()
    ))
}
#[test]
fn original_settings_defaults_parse_toggles_energy_weight_packets_and_reentry() {
    for (index, line) in CORPUS.lines().enumerate() {
        let expected: Value = serde_json::from_str(line).unwrap();
        let actual = native::run(&expected["request"]);
        assert!(
            difference(&expected, &actual, String::new()).is_none(),
            "case {index}: {}",
            difference(&expected, &actual, String::new()).unwrap_or_default()
        );
    }
}
#[test]
fn settings_fixture_and_original_sources_remain_pinned() {
    let provenance: Value =
        serde_json::from_str(include_str!("../../../fixtures/settings/provenance.json")).unwrap();
    assert_eq!(provenance["caseCount"], CORPUS.lines().count());
    assert_eq!(
        provenance["fixtureSha256"],
        format!("{:x}", Sha256::digest(CORPUS))
    );
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (path, hash) in provenance["sources"].as_object().unwrap() {
        let path = root.join(path);
        if path.exists() {
            let raw: Vec<_> = std::fs::read(&path)
                .unwrap()
                .into_iter()
                .filter(|b| *b != b'\r')
                .collect();
            assert_eq!(
                format!("{:x}", Sha256::digest(raw)),
                hash.as_str().unwrap(),
                "{}",
                path.display()
            );
        }
    }
}
