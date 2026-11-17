#[path = "../../../oracle/bank-interface/native.rs"]
mod native;
use serde_json::Value;
use sha2::{Digest, Sha256};
const CORPUS: &str = include_str!("../../../fixtures/bank-interface/oracle.jsonl");
#[test]
fn complete_original_bank_interface_and_listener_objects_match_mutable_services() {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for (i, line) in CORPUS.lines().enumerate() {
            let row: Value = serde_json::from_str(line).unwrap();
            let actual = native::evaluate(&row);
            if actual != row["expected"] {
                tx.send(Err(format!(
                    "bank interface case{i}: first difference: {}",
                    difference(&actual, &row["expected"], String::new()).unwrap()
                )))
                .unwrap();
                return;
            }
        }
        tx.send(Ok(())).unwrap();
    });
    rx.recv_timeout(std::time::Duration::from_secs(60))
        .expect("bank interface callbacks failed or deadlocked")
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
fn bank_interface_original_sources_constants_and_fixtures_are_pinned() {
    let p: Value = serde_json::from_str(include_str!(
        "../../../fixtures/bank-interface/provenance.json"
    ))
    .unwrap();
    assert_eq!(p["caseCount"], CORPUS.lines().count());
    assert_eq!(p["fixtureSha256"], format!("{:x}", Sha256::digest(CORPUS)));
    let initial: Value = serde_json::from_str(include_str!(
        "../../../oracle/bank-interface/initial.lock.json"
    ))
    .unwrap();
    assert_eq!(p["initialBankInterfaceBaseline"], initial);
    let prefix = CORPUS
        .split_inclusive('\n')
        .take(initial["caseCount"].as_u64().unwrap() as usize)
        .collect::<String>();
    assert_eq!(initial["sha256"], format!("{:x}", Sha256::digest(prefix)));
    let packets: Value = serde_json::from_str(include_str!(
        "../../../oracle/bank-interface/packet.lock.json"
    ))
    .unwrap();
    assert_eq!(p["initialInterfacePacketBaseline"], packets);
    let prefix = CORPUS
        .split_inclusive('\n')
        .take(packets["caseCount"].as_u64().unwrap() as usize)
        .collect::<String>();
    assert_eq!(packets["sha256"], format!("{:x}", Sha256::digest(prefix)));
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (path, hash) in p["sources"].as_object().unwrap() {
        let path = root.join(path);
        if path.is_file() {
            let data = std::fs::read(&path)
                .unwrap()
                .into_iter()
                .filter(|b| *b != b'\r')
                .collect::<Vec<_>>();
            assert_eq!(
                hash.as_str().unwrap(),
                format!("{:x}", Sha256::digest(data)),
                "{}",
                path.display()
            );
        }
    }
}
