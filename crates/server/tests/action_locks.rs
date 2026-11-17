#[path = "../../../oracle/action-locks/native.rs"]
mod native;
use serde_json::Value;
use sha2::{Digest, Sha256};
const CORPUS: &str = include_str!("../../../fixtures/action-locks/oracle.jsonl");
#[test]
fn immutable_message_identity_and_equipment_aliases_survive_field_replacement() {
    use rs09_server::world::action_locks::{ActionLocks, Kind, Lock, Text};
    use std::sync::Arc;
    let message: Text = vec![65, 0, 0xd800].into();
    let lock = Lock::<usize>::new(Some(message.clone()));
    let retained = lock.message().unwrap();
    assert!(Arc::ptr_eq(&message, &retained));
    let actions = ActionLocks::default();
    actions.set_equipment(Some(lock.clone()));
    lock.set_message(Some(vec![66].into()));
    assert!(actions.get(Kind::Equipment).unwrap().same(&lock));
    assert_eq!(
        actions
            .get(Kind::Equipment)
            .unwrap()
            .message()
            .unwrap()
            .as_ref(),
        &[66]
    );
    assert_eq!(retained.as_ref(), &[65, 0, 0xd800]);
    actions.set_equipment(None);
    assert!(actions.get(Kind::Equipment).is_none());
    assert_eq!(lock.message().unwrap().as_ref(), &[66]);
}
#[test]
fn complete_original_locks_clock_alias_callback_reentry_and_failure_prefixes_match() {
    for (i, line) in CORPUS.lines().enumerate() {
        let row: Value = serde_json::from_str(line).unwrap();
        assert_eq!(
            native::evaluate(&row),
            row["expected"],
            "action locks {i}: {}",
            row["actions"]
        );
    }
}
#[test]
fn action_locks_reference_sources_and_fixture_bytes_are_pinned() {
    let p: Value = serde_json::from_str(include_str!(
        "../../../fixtures/action-locks/provenance.json"
    ))
    .unwrap();
    assert_eq!(p["caseCount"], CORPUS.lines().count());
    assert_eq!(p["fixtureSha256"], format!("{:x}", Sha256::digest(CORPUS)));
    let initial: Value = serde_json::from_str(include_str!(
        "../../../oracle/action-locks/initial.lock.json"
    ))
    .unwrap();
    assert_eq!(p["initialBaseline"], initial);
    assert_eq!(initial["caseCount"], 309);
    assert_eq!(
        initial["sha256"],
        "bbb6a6d4d307c6ea191807925bc6b022ed71f19e323a1840a2129c4e43405d42"
    );
    let prefix = CORPUS.split_inclusive('\n').take(309).collect::<String>();
    assert_eq!(format!("{:x}", Sha256::digest(prefix)), initial["sha256"]);
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    for (name, expected) in p["sources"].as_object().unwrap() {
        let path = root.join(name);
        if path.is_file() {
            let bytes = std::fs::read(path)
                .unwrap()
                .into_iter()
                .filter(|b| *b != b'\r')
                .collect::<Vec<_>>();
            assert_eq!(
                format!("{:x}", Sha256::digest(bytes)),
                expected.as_str().unwrap(),
                "{name}"
            );
        }
    }
}
