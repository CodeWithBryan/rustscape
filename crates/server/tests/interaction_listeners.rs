#[path = "../../../oracle/interaction-listeners/native.rs"]
mod native;
use serde_json::Value;
use sha2::{Digest, Sha256};
#[test]
fn complete_original_interaction_registry_and_scheduling_prefixes_match() {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for (i, line) in include_str!("../../../fixtures/interaction-listeners/oracle.jsonl")
            .lines()
            .enumerate()
        {
            let row: Value = serde_json::from_str(line).unwrap();
            let actual = native::evaluate(&row);
            if actual != row["expected"] {
                let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
                    "../../runs/interaction-listeners-investigation/native-first-difference.json",
                );
                std::fs::write(
                    path,
                    serde_json::to_vec_pretty(
                        &serde_json::json!({"case":i,"input":row,"actual":actual}),
                    )
                    .unwrap(),
                )
                .unwrap();
                tx.send(Err(format!(
                    "interaction listener case{i} differs; see native-first-difference.json"
                )))
                .unwrap();
                return;
            }
        }
        tx.send(Ok(())).unwrap();
    });
    rx.recv_timeout(std::time::Duration::from_secs(60))
        .expect("interaction listener callback deadlocked")
        .unwrap();
}
#[test]
fn original_sources_declared_reflection_and_initial_observations_are_pinned() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let corpus = include_str!("../../../fixtures/interaction-listeners/oracle.jsonl");
    let p: Value = serde_json::from_str(include_str!(
        "../../../fixtures/interaction-listeners/provenance.json"
    ))
    .unwrap();
    assert_eq!(p["caseCount"], corpus.lines().count());
    assert_eq!(p["fixtureSha256"], format!("{:x}", Sha256::digest(corpus)));
    let initial: Value = serde_json::from_str(include_str!(
        "../../../oracle/interaction-listeners/initial.lock.json"
    ))
    .unwrap();
    assert_eq!(p["initialBaseline"], initial);
    let prefix = corpus
        .split_inclusive('\n')
        .take(initial["caseCount"].as_u64().unwrap() as usize)
        .collect::<String>();
    assert_eq!(initial["sha256"], format!("{:x}", Sha256::digest(prefix)));
    let reflect: Value = serde_json::from_str(include_str!(
        "../../../oracle/input-prompt/reflect.lock.json"
    ))
    .unwrap();
    assert_eq!(p["declaredReflectionArtifact"], reflect);
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
