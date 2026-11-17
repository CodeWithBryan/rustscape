#[path = "../../../oracle/bank-pin/native.rs"]
mod native;
use serde_json::Value;
use sha2::{Digest, Sha256};
const CORPUS: &str = include_str!("../../../fixtures/bank-pin/oracle.jsonl");
#[test]
fn original_bank_pin_state_interfaces_callbacks_and_failure_prefixes_match() {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for (i, line) in CORPUS.lines().enumerate() {
            let row: Value = serde_json::from_str(line).unwrap();
            let actual = native::evaluate(&row);
            if actual != row["expected"] {
                let index = actual
                    .as_array()
                    .unwrap()
                    .iter()
                    .zip(row["expected"].as_array().unwrap())
                    .position(|(a, b)| a != b)
                    .unwrap();
                tx.send(Err(format!(
                    "PIN original case{i} action{index}: {}\nnative={}\noriginal={}",
                    row["actions"], actual[index], row["expected"][index]
                )))
                .unwrap();
                return;
            }
        }
        tx.send(Ok(())).unwrap();
    });
    rx.recv_timeout(std::time::Duration::from_secs(60))
        .expect("bank PIN worker failed or deadlocked")
        .unwrap();
}
#[test]
fn original_bank_pin_sources_and_fixtures_are_pinned() {
    let p: Value =
        serde_json::from_str(include_str!("../../../fixtures/bank-pin/provenance.json")).unwrap();
    assert_eq!(p["caseCount"], CORPUS.lines().count());
    assert_eq!(p["fixtureSha256"], format!("{:x}", Sha256::digest(CORPUS)));
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let initial: Value =
        serde_json::from_str(include_str!("../../../oracle/bank-pin/initial.lock.json")).unwrap();
    assert_eq!(p["initialBaseline"], initial);
    let prefix = CORPUS
        .split_inclusive('\n')
        .take(initial["caseCount"].as_u64().unwrap() as usize)
        .collect::<String>();
    assert_eq!(initial["sha256"], format!("{:x}", Sha256::digest(prefix)));
    for (name, hash) in p["sources"].as_object().unwrap() {
        let path = root.join(name);
        if path.is_file() {
            let data: Vec<_> = std::fs::read(path)
                .unwrap()
                .into_iter()
                .filter(|b| *b != b'\r')
                .collect();
            assert_eq!(
                hash.as_str().unwrap(),
                format!("{:x}", Sha256::digest(data)),
                "{name}"
            );
        }
    }
}
