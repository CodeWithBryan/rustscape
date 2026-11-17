use rs09_game_core::java_lower::{lowercase_root, root_word_boundaries};
use serde_json::Value;
use sha2::{Digest, Sha256};
const CORPUS: &str = include_str!("../../fixtures/string-case/oracle.jsonl");
#[test]
fn root_lowercase_and_word_boundaries_match_jdk11() {
    for (index, line) in CORPUS.lines().enumerate() {
        let case: Value = serde_json::from_str(line).unwrap();
        let text = case["text"].as_str().unwrap();
        assert_eq!(
            lowercase_root(text),
            case["lower"],
            "lower case {index}: {text:?}"
        );
        assert_eq!(
            serde_json::to_value(root_word_boundaries(text)).unwrap(),
            case["boundaries"],
            "boundary case {index}: {text:?}"
        );
    }
}
#[test]
fn exhaustive_codepoint_data_and_fixture_sources_are_pinned() {
    let p: Value =
        serde_json::from_str(include_str!("../../fixtures/string-case/provenance.json")).unwrap();
    assert_eq!(format!("{:x}", Sha256::digest(CORPUS)), p["fixtureSha256"]);
    assert_eq!(
        format!(
            "{:x}",
            Sha256::digest(include_bytes!("../../fixtures/string-case/data.json"))
        ),
        p["dataSha256"]
    );
    assert_eq!(CORPUS.lines().count() as u64, p["caseCount"]);
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    for (name, expected) in p["sources"].as_object().unwrap() {
        let file = root.join(name);
        let bytes = std::fs::read(file)
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
