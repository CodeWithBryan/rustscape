//! Replays the JVM dialogue DSL corpus (DialogueLabeller, DialogueBuilder,
//! SkillDialogueHandler/SkillDialoguePlugin, ActivateEnchantedGem and the
//! banker conversation) against `world::dialogue_dsl`, and pins the fixture
//! and its sources.
#[path = "../../../oracle/dialogue-dsl/native.rs"]
mod native;
use serde_json::Value;
use sha2::{Digest, Sha256};
const CORPUS: &str = include_str!("../../../fixtures/dialogue-dsl/oracle.jsonl");

fn difference(a: &Value, b: &Value, path: String) -> Option<String> {
    if a == b {
        return None;
    }
    if let (Some(a), Some(b)) = (a.as_array(), b.as_array())
        && a.len() == b.len()
    {
        for (i, (a, b)) in a.iter().zip(b).enumerate() {
            if let Some(d) = difference(a, b, format!("{path}/{i}")) {
                return Some(d);
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
fn original_dialogue_dsl_observations_match() {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .stack_size(32 << 20)
        .spawn(move || {
            let (mut cases, mut failures) = (0usize, 0usize);
            for (i, line) in CORPUS.lines().enumerate() {
                let row: Value = serde_json::from_str(line).unwrap();
                let actual = native::evaluate(&row);
                if let Some(diff) = difference(&actual, &row["expected"], String::new()) {
                    tx.send(Err(format!(
                        "dialogue DSL case {i} ({}): {diff}",
                        row["name"]
                    )))
                    .unwrap();
                    return;
                }
                cases += 1;
                failures += usize::from(row["onlyLast"] == Value::Bool(true));
            }
            tx.send(Ok((cases, failures))).unwrap();
        })
        .unwrap();
    let (cases, failures) = rx
        .recv_timeout(std::time::Duration::from_secs(300))
        .expect("dialogue DSL replay deadlocked")
        .unwrap();
    let p: Value = serde_json::from_str(include_str!(
        "../../../fixtures/dialogue-dsl/provenance.json"
    ))
    .unwrap();
    assert_eq!(p["caseCount"], cases);
    assert_eq!(p["coverage"]["failureCases"], failures);
}

#[test]
fn original_dialogue_dsl_sources_and_fixture_are_pinned() {
    let p: Value = serde_json::from_str(include_str!(
        "../../../fixtures/dialogue-dsl/provenance.json"
    ))
    .unwrap();
    assert_eq!(p["caseCount"], CORPUS.lines().count());
    assert_eq!(p["fixtureSha256"], format!("{:x}", Sha256::digest(CORPUS)));
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
