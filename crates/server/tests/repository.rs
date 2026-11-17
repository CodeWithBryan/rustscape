#[path = "../../../oracle/repository/native.rs"]
mod native;
use rs09_server::world::{
    disconnection::{AFK_LOGOUT_TICKS, LOGOUT_TICKS, seconds_to_ticks},
    repository::{Error, JavaHashMap, MAX_CAPACITY, NodeList, Repository},
};
use serde_json::Value;
use sha2::{Digest, Sha256};

const CORPUS: &str = include_str!("../../../fixtures/repository/oracle.jsonl");

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
    }
    let shorten = |value: &Value| value.to_string().chars().take(400).collect::<String>();
    Some(format!(
        "{path}: native {} original {}",
        shorten(a),
        shorten(b)
    ))
}

#[test]
fn original_repository_and_disconnection_queue_cases_replay() {
    let (mut scenarios, mut maps, mut steps) = (0, 0, 0);
    for (number, line) in CORPUS.lines().enumerate() {
        let case: Value = serde_json::from_str(line).unwrap();
        let actual = native::evaluate(&case);
        let expected = &case["expected"];
        if case.get("kind").is_some() {
            maps += 1;
        } else {
            scenarios += 1;
        }
        steps += expected.as_array().unwrap().len();
        assert_eq!(
            actual.as_array().unwrap().len(),
            expected.as_array().unwrap().len(),
            "case {number}"
        );
        if let Some(found) = difference(&actual, expected, String::new()) {
            let step = found
                .split('/')
                .nth(1)
                .and_then(|step| step.parse::<usize>().ok());
            let action = step.map(|step| case["actions"][step].to_string());
            panic!("case {number}: {found}\naction: {action:?}");
        }
    }
    println!("{scenarios} scenarios and {maps} HashMap cases replayed, {steps} steps");
    assert!(scenarios > 0 && maps > 0);
}

#[test]
fn repository_fixture_and_reference_sources_are_pinned() {
    let provenance: Value =
        serde_json::from_str(include_str!("../../../fixtures/repository/provenance.json")).unwrap();
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
fn the_wait_limits_are_the_original_tick_counts() {
    assert_eq!(seconds_to_ticks(30), 50);
    assert_eq!(seconds_to_ticks(5 * 60), 500);
    assert_eq!((AFK_LOGOUT_TICKS, LOGOUT_TICKS), (50, 500));
}

#[test]
fn capacities_the_original_cannot_build_are_refused() {
    // NodeList(-2) is a NegativeArraySizeException, HashMap(-1) an IllegalArgumentException.
    assert!(matches!(
        NodeList::<u8>::new(-2),
        Err(Error::NegativeArraySize)
    ));
    assert!(matches!(
        Repository::<u8>::new(-1, 0),
        Err(Error::IllegalArgument)
    ));
    assert!(matches!(
        Repository::<u8>::new(-2, 0),
        Err(Error::NegativeArraySize)
    ));
    assert!(JavaHashMap::<i32, ()>::with_capacity(-1).is_none());
    // Native bound, not original behaviour.
    assert!(matches!(
        NodeList::<u8>::new(MAX_CAPACITY + 1),
        Err(Error::Capacity)
    ));
    assert!(NodeList::<u8>::new(-1).is_ok());
}

#[test]
fn clones_are_the_same_list_and_queue() {
    let repository = Repository::<u8>::new(3, 3).unwrap();
    let other = repository.clone();
    assert_eq!(repository.players().add(7), Some(1));
    assert_eq!(other.players().get(1), Ok(Some(7)));
    assert!(repository.players().same(&other.players()));
    assert!(!repository.players().same(&repository.npcs()));
    assert!(other.disconnection_queue().is_empty());
}
