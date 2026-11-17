#[path = "../../../oracle/bank/native.rs"]
mod native;
use serde_json::Value;
use sha2::{Digest, Sha256};
const CORPUS: &str = include_str!("../../../fixtures/bank/oracle.jsonl");
fn difference(a: &Value, b: &Value, path: String) -> Option<String> {
    if a == b {
        return None;
    }
    if let (Some(a), Some(b)) = (a.as_array(), b.as_array()) {
        if a.len() != b.len() {
            return Some(format!("{path}: array length {} != {}", a.len(), b.len()));
        }
        for (i, (a, b)) in a.iter().zip(b).enumerate() {
            if let Some(d) = difference(a, b, format!("{path}[{i}]")) {
                return Some(d);
            }
        }
    } else if let (Some(a), Some(b)) = (a.as_object(), b.as_object()) {
        for key in a.keys().chain(b.keys()) {
            if let Some(d) = difference(
                a.get(key).unwrap_or(&Value::Null),
                b.get(key).unwrap_or(&Value::Null),
                format!("{path}.{key}"),
            ) {
                return Some(d);
            }
        }
    }
    Some(format!(
        "{path}: native={} original={}",
        a.to_string().chars().take(200).collect::<String>(),
        b.to_string().chars().take(200).collect::<String>()
    ))
}
#[test]
fn complete_original_bank_class_packets_aliases_and_failure_prefixes_match() {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for (i, line) in CORPUS.lines().enumerate() {
            let row: Value = serde_json::from_str(line).unwrap();
            let actual = native::evaluate(&row);
            if let Some(d) = difference(&actual, &row["expected"], "$".into()) {
                tx.send(Err(format!(
                    "bank original case{i}: {}\n{d}",
                    row["actions"]
                )))
                .unwrap();
                return;
            }
        }
        tx.send(Ok(())).unwrap();
    });
    rx.recv_timeout(std::time::Duration::from_secs(60))
        .expect("bank differential worker failed or deadlocked")
        .unwrap();
}
#[test]
fn original_bank_sources_fixtures_and_prior_inventory_are_pinned() {
    let p: Value =
        serde_json::from_str(include_str!("../../../fixtures/bank/provenance.json")).unwrap();
    assert_eq!(p["caseCount"], CORPUS.lines().count());
    assert_eq!(p["fixtureSha256"], format!("{:x}", Sha256::digest(CORPUS)));
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    assert_eq!(
        p["unchangedInventoryBaseline"],
        format!(
            "{:x}",
            Sha256::digest(std::fs::read(root.join("fixtures/inventory/oracle.jsonl")).unwrap())
        )
    );
    let constants: Value =
        serde_json::from_str(include_str!("../../../oracle/bank/constants.lock.json")).unwrap();
    assert_eq!(p["externalConstants"], constants);
    let initial: Value =
        serde_json::from_str(include_str!("../../../oracle/bank/initial.lock.json")).unwrap();
    assert_eq!(p["initialBankBaseline"], initial);
    let prefix = CORPUS
        .split_inclusive('\n')
        .take(initial["caseCount"].as_u64().unwrap() as usize)
        .collect::<String>();
    assert_eq!(initial["sha256"], format!("{:x}", Sha256::digest(prefix)));
    assert_eq!(
        constants["constants"]["VARBIT_IFACE_BANK_INSERT_MODE"],
        rs09_server::inventory::bank::INSERT_MODE
    );
    assert_eq!(
        constants["constants"]["VARBIT_IFACE_BANK_NOTE_MODE"],
        rs09_server::inventory::bank::NOTE_MODE
    );
    for (name, hash) in p["sources"].as_object().unwrap() {
        let path = root.join(name);
        if path.is_file() {
            let raw: Vec<_> = std::fs::read(path)
                .unwrap()
                .into_iter()
                .filter(|b| *b != b'\r')
                .collect();
            assert_eq!(
                hash.as_str().unwrap(),
                format!("{:x}", Sha256::digest(raw)),
                "{name}"
            );
        }
    }
}
