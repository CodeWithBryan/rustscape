#[path = "../../../oracle/player-save/native.rs"]
mod native;
use serde_json::Value;
use sha2::{Digest, Sha256};
const CORPUS: &str = include_str!("../../../fixtures/player-save/oracle.jsonl");
fn difference(a: &Value, b: &Value, path: String) -> Option<String> {
    if a == b {
        return None;
    }
    if let (Some(a), Some(b)) = (a.as_object(), b.as_object()) {
        for k in a.keys().chain(b.keys()) {
            if let Some(d) = difference(
                a.get(k).unwrap_or(&Value::Null),
                b.get(k).unwrap_or(&Value::Null),
                format!("{path}/{k}"),
            ) {
                return Some(d);
            }
        }
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
    if let (Some(a), Some(b)) = (a.as_str(), b.as_str()) {
        // Saved texts are long; show the first place they part.
        let (a, b) = (a.chars().collect::<Vec<_>>(), b.chars().collect::<Vec<_>>());
        let at = a.iter().zip(&b).take_while(|(a, b)| a == b).count();
        let window = |text: &[char]| {
            text[at.saturating_sub(60)..text.len().min(at + 60)]
                .iter()
                .collect::<String>()
        };
        return Some(format!(
            "{path}: texts part at char {at}: native=...{:?}..., original=...{:?}...",
            window(&a),
            window(&b)
        ));
    }
    Some(format!("{path}: native={a}, original={b}"))
}
#[test]
fn whole_saves_parsed_state_hand_offs_and_failures_match_original() {
    let mut failures = Vec::new();
    for (index, line) in CORPUS.lines().enumerate() {
        let case: Value = serde_json::from_str(line).unwrap();
        // The appended persisted-state cases are replayed by tests/player_state.rs.
        if case["state"] == true {
            continue;
        }
        let actual = native::evaluate(&case, index);
        if let Some(d) = difference(&actual, &case["expected"], String::new()) {
            failures.push(format!(
                "case {index} ({}): {d}",
                case["label"].as_str().unwrap_or("")
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} cases differ:\n{}",
        failures.len(),
        CORPUS.lines().count(),
        failures
            .iter()
            .take(40)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}
#[test]
fn player_save_fixtures_pin_original_sources_and_adapters() {
    let p: Value = serde_json::from_str(include_str!(
        "../../../fixtures/player-save/provenance.json"
    ))
    .unwrap();
    assert_eq!(
        format!("{:x}", Sha256::digest(CORPUS.as_bytes())),
        p["fixtureSha256"]
    );
    assert_eq!(
        CORPUS.lines().count() as u64,
        p["caseCount"].as_u64().unwrap()
    );
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    for (source, expected) in p["sources"].as_object().unwrap() {
        let path = root.join(source);
        if path.is_file() {
            let data = std::fs::read(path)
                .unwrap()
                .into_iter()
                .filter(|b| *b != b'\r')
                .collect::<Vec<_>>();
            assert_eq!(
                format!("{:x}", Sha256::digest(data)),
                expected.as_str().unwrap(),
                "{source}"
            );
        }
    }
}
/// Not differential: these inputs load in the original and are rejected here.
/// docs/player-save.md lists them as differences; this pins what the native
/// side does instead of leaving it unobserved.
#[test]
fn documented_native_rejections_are_reported_as_native_limits() {
    use rs09_server::player::save::{Category, parser::parse_text};
    let unpaired = parse_text(r#"{"attributes":[{"type":"str","key":"k","value":"\ud800"}]}"#);
    assert_eq!(unpaired.unwrap_err().category(), Category::NativeLimit);
    let nested = format!("{{\"a\":{}{}}}", "[".repeat(300), "]".repeat(300));
    assert_eq!(
        parse_text(&nested).unwrap_err().category(),
        Category::NativeLimit
    );
    // A pair written as two escapes is ordinary text in both.
    assert!(parse_text(r#"{"a":"\ud83d\ude00"}"#).is_ok());
}
