//! `world::npc_masks` (NPCFlags530) replayed against the original's corpus
//! (`cargo xtask npc-masks-oracle`): every provider's presence flag, ordinal
//! and flag, and every `writeTo` with its bytes, entity reads, log line and
//! exception.
#[path = "../../../oracle/npc-masks/native.rs"]
mod native;
use serde_json::Value;

const CORPUS: &str = include_str!("../../../fixtures/npc-masks/oracle.jsonl");

#[test]
fn original_npc_flags_530_encoders() {
    let mut cases = 0;
    let mut written = 0;
    for (index, line) in CORPUS.lines().enumerate() {
        let case: Value = serde_json::from_str(line).unwrap();
        let native = native::evaluate(&case);
        assert_eq!(
            native, case["expected"],
            "npc mask case {index}: {}",
            case["context"]
        );
        cases += 1;
        written += native["bytes"].as_array().unwrap().len();
    }
    assert_eq!(cases, 1545);
    assert!(written > 0);
}
