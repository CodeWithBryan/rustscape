#[path = "../../../oracle/npc-tick/native.rs"]
mod native;
use serde_json::Value;
use sha2::{Digest, Sha256};
const CORPUS: &str = include_str!("../../../fixtures/npc-tick/oracle.jsonl");
#[test]
fn original_npc_tick_wander_rng_respawn_and_partial_failures() {
    for (i, line) in CORPUS.lines().enumerate() {
        let c: Value = serde_json::from_str(line).unwrap();
        let actual = native::evaluate(&c);
        assert_eq!(
            actual.as_array().unwrap().len(),
            c["expected"].as_array().unwrap().len(),
            "NPC tick case{i} action count"
        );
        for (j, (a, e)) in actual
            .as_array()
            .unwrap()
            .iter()
            .zip(c["expected"].as_array().unwrap())
            .enumerate()
        {
            assert_eq!(a, e, "NPC tick case{i} action{j}");
        }
    }
}
#[test]
fn npc_tick_fixture_and_original_sources_are_pinned() {
    let p: Value =
        serde_json::from_str(include_str!("../../../fixtures/npc-tick/provenance.json")).unwrap();
    assert_eq!(p["caseCount"], CORPUS.lines().count());
    assert_eq!(p["fixtureSha256"], format!("{:x}", Sha256::digest(CORPUS)));
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    for (path, hash) in p["sources"].as_object().unwrap() {
        let path = root.join(path);
        if path.is_file() {
            let b: Vec<_> = std::fs::read(&path)
                .unwrap()
                .into_iter()
                .filter(|b| *b != b'\r')
                .collect();
            assert_eq!(
                format!("{:x}", Sha256::digest(b)),
                hash.as_str().unwrap(),
                "{}",
                path.display()
            );
        }
    }
}
