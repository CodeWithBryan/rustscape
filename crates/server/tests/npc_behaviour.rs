//! Replays the npc-behaviour corpus (fixtures/npc-behaviour) through
//! `world::npc_behaviour` joined to the `world::npc` tick kernel and the
//! native `entity_regions::is_tolerated`.
#[path = "../../../oracle/npc-behaviour/native.rs"]
mod native;
use serde_json::Value;
use sha2::{Digest, Sha256};

const CORPUS: &str = include_str!("../../../fixtures/npc-behaviour/oracle.jsonl");
const PROVENANCE: &str = include_str!("../../../fixtures/npc-behaviour/provenance.json");

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
fn original_aggression_behaviour_hooks_and_combat_members() {
    let mut cases = 0;
    let mut actions = 0;
    for (index, line) in CORPUS.lines().enumerate() {
        let case: Value = serde_json::from_str(line).unwrap();
        let native = native::evaluate(&case);
        if let Some(found) = difference(&native, &case["expected"], String::new()) {
            panic!(
                "npc behaviour case {index} ({}): {found}\nops: {}",
                case["input"]["group"], case["input"]["ops"]
            );
        }
        cases += 1;
        actions += native.as_array().unwrap().len();
    }
    let provenance: Value = serde_json::from_str(PROVENANCE).unwrap();
    assert_eq!(cases, provenance["caseCount"].as_u64().unwrap() as usize);
    assert_eq!(
        actions,
        provenance["coverage"]["actions"].as_u64().unwrap() as usize
    );
}

#[test]
fn fixture_matches_its_provenance() {
    let provenance: Value = serde_json::from_str(PROVENANCE).unwrap();
    let digest = format!("{:x}", Sha256::digest(CORPUS.as_bytes()));
    assert_eq!(digest, provenance["fixtureSha256"].as_str().unwrap());
    for path in [
        "reference/server/Server/src/main/core/game/node/entity/npc/agg/AggressiveHandler.java",
        "reference/server/Server/src/main/core/game/node/entity/npc/agg/AggressiveBehavior.java",
        "reference/server/Server/src/main/core/game/node/entity/npc/NPCBehavior.kt",
        "reference/server/Server/src/main/core/game/node/entity/npc/NPC.java",
        "reference/server/Server/src/main/core/game/node/entity/Entity.java",
        "reference/server/Server/src/main/core/game/node/entity/player/Player.java",
    ] {
        assert!(
            provenance["sources"][path].is_string(),
            "{path} is not pinned by the fixture"
        );
    }
}
