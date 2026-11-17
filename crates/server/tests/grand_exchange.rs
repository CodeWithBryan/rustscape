#[path = "../../../oracle/grand-exchange/native.rs"]
mod native;
use rs09_server::world::grand_exchange::{bot_prices, db, item_sets, records, ui};
use serde_json::Value;
use sha2::{Digest, Sha256};

const CORPUS: &str = include_str!("../../../fixtures/grand-exchange/oracle.jsonl");
const INPUTS: &str = include_str!("../../../fixtures/grand-exchange/inputs.json");

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
fn original_grand_exchange_cases_replay() {
    let (mut cases, mut steps) = (0, 0);
    let mut failures = Vec::new();
    // RS09_GE_CASES=c0001,c0002 replays only those cases (a development aid).
    let only = std::env::var("RS09_GE_CASES").ok();
    for line in CORPUS.lines() {
        let case: Value = serde_json::from_str(line).unwrap();
        if let Some(only) = &only
            && !only.split(',').any(|name| case["name"] == name)
        {
            continue;
        }
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
            failures.push(format!(
                "case {}: {found}\naction: {action:?}",
                case["name"]
            ));
        }
    }
    let _ = std::fs::remove_dir_all(
        std::env::temp_dir().join(format!("rs09-ge-replay-{}", std::process::id())),
    );
    if !failures.is_empty() {
        panic!(
            "{} of {cases} cases differ; first:\n{}",
            failures.len(),
            failures[..failures.len().min(5)].join("\n")
        );
    }
    println!("{cases} Grand Exchange cases replayed, {steps} steps");
    assert!(cases > 0);
}

#[test]
fn grand_exchange_fixture_and_reference_sources_are_pinned() {
    let provenance: Value = serde_json::from_str(include_str!(
        "../../../fixtures/grand-exchange/provenance.json"
    ))
    .unwrap();
    assert_eq!(provenance["caseCount"], CORPUS.lines().count());
    assert_eq!(
        provenance["fixtureSha256"],
        format!("{:x}", Sha256::digest(CORPUS))
    );
    assert_eq!(
        provenance["inputsSha256"],
        format!("{:x}", Sha256::digest(INPUTS.replace('\r', "")))
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
fn tables_and_formats_are_the_originals() {
    // hashMapOf(player_offers, bot_offers, price_index) iterates price_index first.
    let order: Vec<&str> = db::table_order().iter().map(|(name, _)| *name).collect();
    assert_eq!(order, ["price_index", "player_offers", "bot_offers"]);
    // Integer keys in a resized HashMap: bucket order, not insertion order.
    assert_eq!(db::definition_map_order(vec![3, 1, 2]), [1, 2, 3]);
    let wide = db::definition_map_order(vec![0, 16, 64, 1, 17]);
    assert_eq!(wide, [0, 16, 64, 1, 17]);
    assert_eq!(item_sets::SETS.len(), 70);
    assert_eq!(item_sets::for_id(11967), Some(69));
    assert_eq!(item_sets::for_id(1), None);
    assert_eq!(bot_prices::price_override(1513), Some(750));
    assert_eq!(records::format_us(1234567), "1,234,567");
    assert_eq!(records::format_us(-1000), "-1,000");
    assert_eq!(records::format_us(999), "999");
    // GrandExchangePacket: aborted sale -3, aborted buy 5, removed all zeros.
    let frame = ui::exchange_frame(2, 2, 1511, true, 5, 10, 3, 15);
    assert_eq!(frame.opcode, 116);
    assert_eq!(frame.payload[..2], [2, 0xfd]);
    let frame = ui::exchange_frame(0, 6, 1511, true, 5, 10, 3, 15);
    assert_eq!(frame.payload, [0; 20]);
    let frame = ui::exchange_frame(1, 1, 1511, true, 5, 10, 3, 15);
    assert_eq!(frame.payload[1], 10);
}
