#[path = "../../../oracle/trade/native.rs"]
mod native;
use rs09_server::world::trade::{Location, module};
use serde_json::Value;
use sha2::{Digest, Sha256};

const CORPUS: &str = include_str!("../../../fixtures/trade/oracle.jsonl");

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
        if a.len() != b.len() {
            return Some(format!(
                "{path}: native length {} original {}",
                a.len(),
                b.len()
            ));
        }
    }
    let shorten = |value: &Value| value.to_string().chars().take(400).collect::<String>();
    Some(format!(
        "{path}: native {} original {}",
        shorten(a),
        shorten(b)
    ))
}

#[test]
fn original_request_and_trade_cases_replay() {
    let (mut cases, mut steps) = (0, 0);
    for (number, line) in CORPUS.lines().enumerate() {
        let case: Value = serde_json::from_str(line).unwrap();
        let actual = native::evaluate(&case);
        let expected = &case["expected"];
        cases += 1;
        steps += expected.as_array().unwrap().len();
        if let Some(found) = difference(&actual, expected, String::new()) {
            let step = found
                .split('/')
                .nth(1)
                .and_then(|step| step.parse::<usize>().ok());
            let action = step
                .and_then(|step| step.checked_sub(1))
                .map(|step| case["actions"][step].to_string());
            panic!("case {number}: {found}\naction: {action:?}");
        }
    }
    println!("{cases} request and trade cases replayed, {steps} steps");
    assert!(cases > 0);
}

#[test]
fn trade_fixture_and_reference_sources_are_pinned() {
    let provenance: Value =
        serde_json::from_str(include_str!("../../../fixtures/trade/provenance.json")).unwrap();
    assert_eq!(provenance["caseCount"], CORPUS.lines().count());
    assert_eq!(
        provenance["fixtureSha256"],
        format!("{:x}", Sha256::digest(CORPUS))
    );
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let mut checked = 0;
    for (path, expected) in provenance["sources"].as_object().unwrap() {
        let path = root.join(path);
        if path.is_file() {
            let data: Vec<_> = std::fs::read(&path)
                .unwrap()
                .into_iter()
                .filter(|byte| *byte != b'\r')
                .collect();
            assert_eq!(
                format!("{:x}", Sha256::digest(data)),
                expected.as_str().unwrap(),
                "{}",
                path.display()
            );
            checked += 1;
        }
    }
    assert!(checked > 0);
}

#[test]
fn distance_and_number_helpers_keep_the_original_arithmetic() {
    let here = Location {
        x: 3222,
        y: 3222,
        z: 0,
    };
    let at = |dx: i32, dy: i32| Location {
        x: 3222i32.wrapping_add(dx),
        y: 3222i32.wrapping_add(dy),
        z: 0,
    };
    assert!(at(15, 0).within_distance(here, 15));
    assert!(!at(16, 0).within_distance(here, 15));
    assert!(at(9, 12).within_distance(here, 15));
    assert!(!at(11, 11).within_distance(here, 15));
    // Int squares wrap: 65536 tiles away squares to zero.
    assert!(at(65536, 0).within_distance(here, 15));
    assert!(!Location { z: 1, ..here }.within_distance(here, 15));
    assert_eq!(module::formatted_number(1234567), "1,234,567");
    assert_eq!(module::formatted_number(999), "999");
    assert_eq!(module::formatted_number(-1000), "-1,000");
    assert_eq!(module::formatted_number(i32::MIN), "-2,147,483,648");
}
