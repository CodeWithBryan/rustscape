use rs09_server::world::plain_dialogue as plain;
#[path = "../../../oracle/plain-dialogue/native.rs"]
mod native;
use serde_json::Value;
use sha2::{Digest, Sha256};
const CORPUS: &str = include_str!("../../../fixtures/plain-dialogue/oracle.jsonl");
#[test]
fn original_splitting_mutable_plain_message_arrays_and_continue_prefixes_match() {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for (i, line) in CORPUS.lines().enumerate() {
            let row: Value = serde_json::from_str(line).unwrap();
            let actual = native::evaluate(&row);
            if let Some(diff) = difference(&actual, &row["expected"], String::new()) {
                tx.send(Err(format!("plain dialogue case{i}: {diff}")))
                    .unwrap();
                return;
            }
        }
        tx.send(Ok(())).unwrap();
    });
    rx.recv_timeout(std::time::Duration::from_secs(60))
        .expect("plain dialogue/Continue callback deadlocked")
        .unwrap();
}
fn difference(a: &Value, b: &Value, path: String) -> Option<String> {
    if a == b {
        return None;
    }
    if let (Some(a), Some(b)) = (a.as_array(), b.as_array()) {
        if a.len() == b.len() {
            for (i, (a, b)) in a.iter().zip(b).enumerate() {
                if let Some(d) = difference(a, b, format!("{path}/{i}")) {
                    return Some(d);
                }
            }
        }
    }
    if let (Some(a), Some(b)) = (a.as_object(), b.as_object()) {
        for key in a.keys().chain(b.keys()) {
            if let Some(d) = difference(
                a.get(key).unwrap_or(&Value::Null),
                b.get(key).unwrap_or(&Value::Null),
                format!("{path}/{key}"),
            ) {
                return Some(d);
            }
        }
    }
    Some(format!("{path}: native={a}, original={b}"))
}
#[test]
fn original_plain_dialogue_sources_fixtures_and_initial_observations_are_pinned() {
    let p: Value = serde_json::from_str(include_str!(
        "../../../fixtures/plain-dialogue/provenance.json"
    ))
    .unwrap();
    assert_eq!(p["caseCount"], CORPUS.lines().count());
    assert_eq!(p["fixtureSha256"], format!("{:x}", Sha256::digest(CORPUS)));
    let initial: Value = serde_json::from_str(include_str!(
        "../../../oracle/plain-dialogue/initial.lock.json"
    ))
    .unwrap();
    assert_eq!(p["initialBaseline"], initial);
    let prefix = CORPUS
        .split_inclusive('\n')
        .take(initial["caseCount"].as_u64().unwrap() as usize)
        .collect::<String>();
    assert_eq!(initial["sha256"], format!("{:x}", Sha256::digest(prefix)));
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (path, hash) in p["sources"].as_object().unwrap() {
        let file = root.join(path);
        if file.is_file() {
            let data = std::fs::read(file)
                .unwrap()
                .into_iter()
                .filter(|c| *c != b'\r')
                .collect::<Vec<_>>();
            assert_eq!(
                hash.as_str().unwrap(),
                format!("{:x}", Sha256::digest(data)),
                "{path}"
            );
        }
    }
}
#[test]
fn invalid_line_limits_and_nonprogressing_tagged_text_fail_explicitly() {
    assert!(plain::split_lines(&[65], 0).is_err());
    assert!(plain::split_lines(&"<b> abcdef".encode_utf16().collect::<Vec<_>>(), 4).is_err());
}
