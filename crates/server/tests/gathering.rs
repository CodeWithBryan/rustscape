#[path = "../../../oracle/gathering/native.rs"]
mod native;
use rs09_server::world::gathering::{
    fishing::{Fish, FishingOption, FishingSpot},
    nodes::{MiningNode, SkillingResource, WoodcuttingNode},
    service::Service,
    tools::Tool,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

const CORPUS: &str = include_str!("../../../fixtures/gathering/oracle.jsonl");
const PROVENANCE: &str = include_str!("../../../fixtures/gathering/provenance.json");
const BINDING: &str = include_str!("../../../fixtures/gathering/binding.jsonl");
const BINDING_PROVENANCE: &str =
    include_str!("../../../fixtures/gathering/binding.provenance.json");
/// Host answers that the binding corpus exercises; see docs/gathering.md for the rest.
const BOUND_AND_EXERCISED: &[&str] = &[
    "skills.getLevel",
    "skills.getStaticLevel",
    "skills.addExperience3",
    "api.getDynLevel",
    "api.hasLevelDyn",
    "api.rewardXP",
    "inventory.freeSlots",
    "inventory.contains",
    "inventory.add",
    "inventory.hasSpaceFor",
    "api.freeSlots",
    "api.inInventory",
    "api.hasSpaceFor",
    "api.addItemOrDrop",
    "api.removeItem:INVENTORY",
    "api.removeItem:EQUIPMENT",
    "equipment.getNew",
    "api.getItemFromEquipment",
    "api.getItemName",
    "itemDefinition.name",
    "item.name",
];
/// The woodcutting/mining corpus as first recorded: its line count and SHA-256.
const EARLIER_CASES: usize = 9281;
const EARLIER_SHA256: &str = "45f37cb6ce2ce423021d16677f9e88b56d8e531d9324fa2f5b9c1235184570ff";

fn difference(a: &Value, b: &Value, path: String) -> Option<String> {
    if a == b {
        return None;
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
    if let (Some(a), Some(b)) = (a.as_array(), b.as_array()) {
        for (index, (a, b)) in a.iter().zip(b).enumerate() {
            if let Some(d) = difference(a, b, format!("{path}/{index}")) {
                return Some(d);
            }
        }
        return Some(format!("{path}: lengths {} != {}", a.len(), b.len()));
    }
    Some(format!("{path}: native={a}, original={b}"))
}
/// The first recorded call a native operation did not reproduce, for diagnosis.
fn first_event_gap(actual: &Value, expected: &Value) -> String {
    let wrap = |v: &Value| match v {
        Value::Array(ops) => ops.clone(),
        other => vec![other.clone()],
    };
    for (op, (a, e)) in wrap(actual).iter().zip(wrap(expected)).enumerate() {
        let (a, e) = (&a["events"], &e["events"]);
        let (a, e) = (
            a.as_array().cloned().unwrap_or_default(),
            e.as_array().cloned().unwrap_or_default(),
        );
        for at in 0..a.len().max(e.len()) {
            if a.get(at) != e.get(at) {
                return format!(
                    "operation {op}, call {at}: native={} original={} (after {})",
                    a.get(at).unwrap_or(&Value::Null),
                    e.get(at).unwrap_or(&Value::Null),
                    e.get(at.wrapping_sub(1)).unwrap_or(&Value::Null)
                );
            }
        }
    }
    "recorded calls agree".into()
}
fn replay(select: impl Fn(&Value) -> bool) -> usize {
    let mut count = 0;
    for (index, line) in CORPUS.lines().enumerate() {
        let case: Value = serde_json::from_str(line).unwrap();
        if !select(&case) {
            continue;
        }
        count += 1;
        let actual = native::evaluate(&case);
        if let Some(d) = difference(&actual, &case["expected"], String::new()) {
            let mut shown = case.clone();
            shown.as_object_mut().unwrap().remove("expected");
            shown.as_object_mut().unwrap().remove("names");
            panic!(
                "gathering case {index}: {d}\n{}\ncase={shown}",
                first_event_gap(&actual, &case["expected"])
            );
        }
    }
    count
}
fn kind(name: &'static str) -> impl Fn(&Value) -> bool {
    move |case| case["kind"] == name
}
fn subject(name: &'static str) -> impl Fn(&Value) -> bool {
    move |case| case["kind"] == "scenario" && case["subject"] == name
}

#[test]
fn every_original_node_resource_and_tool_row_matches() {
    assert_eq!(replay(kind("counts")), 1);
    assert_eq!(
        replay(kind("woodcuttingNode")),
        WoodcuttingNode::values().len()
    );
    assert_eq!(replay(kind("miningNode")), MiningNode::values().len());
    assert_eq!(replay(kind("resource")), SkillingResource::values().len());
    assert_eq!(replay(kind("tool")), Tool::values().count());
}
#[test]
fn original_lookup_order_shadowing_and_respawn_durations_match() {
    assert!(replay(kind("lookup")) > 0);
    assert!(replay(kind("respawn")) > 0);
}
#[test]
fn original_random_function_draws_and_generators_match() {
    assert!(replay(kind("random")) > 0);
}
#[test]
fn original_tool_selection_order_and_level_gates_match() {
    assert!(replay(kind("selectTool")) > 0);
}
#[test]
fn original_woodcutting_listener_calls_draws_and_stops_match() {
    assert!(replay(subject("woodcutting")) > 0);
}
#[test]
fn original_woodcutting_pulse_calls_draws_and_stops_match() {
    assert!(replay(subject("woodcuttingPulse")) > 0);
}
#[test]
fn original_mining_listener_calls_draws_and_stops_match() {
    assert!(replay(subject("mining")) > 0);
}
#[test]
fn original_mining_pulse_and_prospect_calls_match() {
    assert!(replay(subject("miningPulse")) > 0);
    assert!(replay(subject("prospect")) > 0);
}
#[test]
fn every_original_fish_option_and_spot_row_and_lookup_matches() {
    assert_eq!(replay(kind("fishingCounts")), 1);
    assert_eq!(replay(kind("fish")), Fish::values().len());
    assert_eq!(replay(kind("fishingOption")), FishingOption::values().len());
    assert_eq!(replay(kind("fishingSpot")), FishingSpot::values().len());
    assert_eq!(replay(kind("fishingLookup")), 5);
}
#[test]
fn original_fishing_listener_calls_draws_and_stops_match() {
    assert!(replay(subject("fishing")) > 0);
}
#[test]
fn original_fishing_pulse_calls_draws_and_stops_match() {
    assert!(replay(subject("fishingPulse")) > 0);
}
/// The cases recorded before fishing was added are a byte prefix of the fixture.
#[test]
fn earlier_recorded_cases_are_unchanged() {
    let end = CORPUS
        .match_indices('\n')
        .nth(EARLIER_CASES - 1)
        .map(|(at, _)| at + 1)
        .unwrap();
    assert_eq!(
        format!("{:x}", Sha256::digest(&CORPUS.as_bytes()[..end])),
        EARLIER_SHA256
    );
}
/// The second corpus: the unmodified listeners and fishing pulse over the original
/// Skills, Container and Item, replayed over the native skills and containers.
#[test]
fn original_skills_and_containers_match_the_native_binding() {
    let provenance: Value = serde_json::from_str(BINDING_PROVENANCE).unwrap();
    assert_eq!(provenance["caseCount"], BINDING.lines().count());
    assert_eq!(
        provenance["fixtureSha256"],
        format!("{:x}", Sha256::digest(BINDING))
    );
    let mut subjects = BTreeMap::new();
    for (index, line) in BINDING.lines().enumerate() {
        let case: Value = serde_json::from_str(line).unwrap();
        let actual = native::binding::evaluate(&case, &provenance["itemNames"]);
        if let Some(d) = difference(&actual, &case["expected"], String::new()) {
            let mut shown = case.clone();
            shown.as_object_mut().unwrap().remove("expected");
            panic!(
                "binding case {index}: {d}\n{}\ncase={shown}",
                first_event_gap(&actual, &case["expected"])
            );
        }
        *subjects
            .entry(case["subject"].as_str().unwrap().to_owned())
            .or_insert(0usize) += 1;
    }
    for subject in ["woodcutting", "mining", "fishing", "fishingPulse"] {
        assert!(subjects.get(subject).is_some_and(|&n| n > 0), "{subject}");
    }
    // Every request the host answers from native state was made by a replayed body.
    let requested = native::binding::requested();
    for bound in BOUND_AND_EXERCISED {
        assert!(requested.get(*bound).is_some_and(|&n| n > 0), "{bound}");
    }
}
#[test]
fn every_recorded_service_name_is_a_declared_service() {
    let names: Vec<&str> = Service::ALL.iter().map(|s| s.name()).collect();
    let mut seen = BTreeMap::new();
    for line in CORPUS.lines() {
        let case: Value = serde_json::from_str(line).unwrap();
        let mut visit = |events: &Value| {
            for event in events.as_array().into_iter().flatten() {
                *seen
                    .entry(event[0].as_str().unwrap().to_owned())
                    .or_insert(0usize) += 1;
            }
        };
        match &case["expected"] {
            Value::Array(ops) => ops.iter().for_each(|op| visit(&op["events"])),
            other => visit(&other["events"]),
        }
    }
    for name in seen.keys() {
        assert!(names.contains(&name.as_str()), "undeclared service {name}");
    }
    // Not interceptable in the JVM, so never on the tape.
    for hidden in [
        "rng.threadLocal.nextDouble",
        "rng.threadLocal.nextInt",
        "string.lowercaseDefault",
    ] {
        assert!(!seen.contains_key(hidden));
    }
}
#[test]
fn gathering_fixture_and_original_sources_are_pinned() {
    let provenance: Value = serde_json::from_str(PROVENANCE).unwrap();
    assert_eq!(provenance["caseCount"], CORPUS.lines().count());
    assert_eq!(
        provenance["fixtureSha256"],
        format!("{:x}", Sha256::digest(CORPUS))
    );
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut checked = 0;
    for (path, hash) in provenance["sources"].as_object().unwrap() {
        let path = root.join(path);
        if path.is_file() {
            let bytes: Vec<_> = std::fs::read(&path)
                .unwrap()
                .into_iter()
                .filter(|&b| b != b'\r')
                .collect();
            assert_eq!(hash, &format!("{:x}", Sha256::digest(bytes)), "{path:?}");
            checked += 1;
        }
    }
    assert!(checked > 0);
}
