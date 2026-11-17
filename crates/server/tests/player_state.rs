//! The persisted state of the systems behind `save::Host`, replayed against
//! the cases player-save-oracle appends after the whole-save corpus.
#[path = "../../../oracle/player-save/state_native.rs"]
mod state_native;
use serde_json::Value;
use sha2::{Digest, Sha256};
const CORPUS: &str = include_str!("../../../fixtures/player-save/oracle.jsonl");
/// The whole-save corpus before any state case was appended: its case count
/// and the SHA-256 of fixtures/player-save/oracle.jsonl as it then was.
const EARLIER_CASES: usize = 2637;
const EARLIER_SHA256: &str = "42f3733f003a41f4e06d09beb1c15566d9d37f0b87065252e31d16ace3437106";

fn state_cases() -> impl Iterator<Item = (usize, Value)> {
    CORPUS
        .lines()
        .enumerate()
        .map(|(index, line)| (index, serde_json::from_str::<Value>(line).unwrap()))
        .filter(|(_, case)| case["state"] == true)
}
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
fn earlier_whole_save_cases_are_an_unchanged_prefix() {
    let mut end = 0;
    for (index, line) in CORPUS.split_inclusive('\n').enumerate() {
        let case: Value = serde_json::from_str(line).unwrap();
        if case["state"] == true {
            assert_eq!(
                index, EARLIER_CASES,
                "state cases start after the earlier corpus"
            );
            break;
        }
        assert!(
            index < EARLIER_CASES,
            "an earlier case after the state cases began"
        );
        end += line.len();
    }
    assert_eq!(
        format!("{:x}", Sha256::digest(&CORPUS.as_bytes()[..end])),
        EARLIER_SHA256
    );
    // Nothing but state cases follows.
    assert!(
        CORPUS
            .lines()
            .skip(EARLIER_CASES)
            .all(|line| serde_json::from_str::<Value>(line).unwrap()["state"] == true)
    );
}

#[test]
fn persisted_state_saves_loads_and_failures_match_original() {
    let mut failures = Vec::new();
    let mut count = 0;
    for (index, case) in state_cases() {
        count += 1;
        let actual = state_native::evaluate(&case, index);
        if let Some(d) = difference(&actual, &case["expected"], String::new()) {
            failures.push(format!(
                "case {index} ({}): {d}",
                case["label"].as_str().unwrap_or("")
            ));
        }
    }
    assert!(count > 0, "no state cases in the corpus");
    assert!(
        failures.is_empty(),
        "{} of {count} state cases differ:\n{}",
        failures.len(),
        failures
            .iter()
            .take(40)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// The case that matters most, checked on its own: a brand-new character's
/// whole file written by the original and written natively from native state
/// alone, byte for byte, before and after loading it into a new character.
#[test]
fn brand_new_character_file_matches_byte_for_byte() {
    let mut texts = 0;
    for (index, case) in state_cases() {
        let label = case["label"].as_str().unwrap();
        if !label.starts_with("fresh character: ") {
            continue;
        }
        let actual = state_native::evaluate(&case, index);
        for (step, (action, expected)) in case["actions"]
            .as_array()
            .unwrap()
            .iter()
            .zip(case["expected"].as_array().unwrap())
            .enumerate()
        {
            let key = match action["op"].as_str().unwrap() {
                "populate" => "text",
                "profileSave" => "file",
                _ => continue,
            };
            let original = expected[key].as_str().unwrap();
            assert_eq!(expected["error"], Value::Null, "{label} step {step}");
            assert_eq!(actual[step]["error"], Value::Null, "{label} step {step}");
            assert_eq!(
                actual[step][key].as_str().unwrap().as_bytes(),
                original.as_bytes(),
                "{label} step {step}"
            );
            texts += 1;
        }
    }
    // Four registries, two populates and two written files each.
    assert_eq!(texts, 16);
}
