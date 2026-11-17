#[path = "../../../oracle/npc-spawns/native.rs"]
mod native;
use serde_json::Value;
use sha2::{Digest, Sha256};
const CORPUS: &str = include_str!("../../../fixtures/npc-spawns/oracle.jsonl");
#[test]
fn original_full_pinned_spawns_and_partial_publication() {
    for (i, line) in CORPUS.lines().enumerate() {
        let c: Value = serde_json::from_str(line).unwrap();
        let actual = native::evaluate(&c);
        assert_eq!(
            actual["error"], c["expected"]["error"],
            "NPC spawn case {i} error"
        );
        for field in ["npcs", "events"] {
            let a = actual[field].as_array().unwrap();
            let e = c["expected"][field].as_array().unwrap();
            assert_eq!(a.len(), e.len(), "NPC spawn case {i} {field} length");
            for (j, (a, e)) in a.iter().zip(e).enumerate() {
                assert_eq!(a, e, "NPC spawn case {i} {field}[{j}]");
            }
        }
    }
}
#[test]
fn npc_spawn_fixture_and_original_sources_are_pinned() {
    let p: Value =
        serde_json::from_str(include_str!("../../../fixtures/npc-spawns/provenance.json")).unwrap();
    assert_eq!(p["caseCount"], CORPUS.lines().count());
    assert_eq!(p["fixtureSha256"], format!("{:x}", Sha256::digest(CORPUS)));
    assert_eq!(
        p["configSha256"],
        format!(
            "{:x}",
            Sha256::digest(include_bytes!(
                "../../../fixtures/npc-spawns/npc_spawns.json"
            ))
        )
    );
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
