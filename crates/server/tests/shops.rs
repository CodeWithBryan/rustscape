#[path = "../../../oracle/shops/native.rs"]
mod native;
use rs09_server::world::shops::{fixed_price, java_hash};
use serde_json::Value;
use sha2::{Digest, Sha256};

const CORPUS: &str = include_str!("../../../fixtures/shops/oracle.jsonl");
const INPUTS: &str = include_str!("../../../fixtures/shops/inputs.json");
const PINNED: &str = include_str!("../../../fixtures/startup-loaders/shops.json");

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
fn original_shop_cases_replay() {
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
    println!("{cases} shop cases replayed, {steps} steps");
    assert!(cases > 0);
}

#[test]
fn shops_fixture_and_reference_sources_are_pinned() {
    let provenance: Value =
        serde_json::from_str(include_str!("../../../fixtures/shops/provenance.json")).unwrap();
    assert_eq!(provenance["caseCount"], CORPUS.lines().count());
    assert_eq!(
        provenance["fixtureSha256"],
        format!("{:x}", Sha256::digest(CORPUS))
    );
    assert_eq!(
        provenance["inputsSha256"],
        format!("{:x}", Sha256::digest(INPUTS.replace('\r', "")))
    );
    assert_eq!(
        provenance["pinnedShopsSha256"],
        format!("{:x}", Sha256::digest(PINNED.replace('\r', "")))
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
fn shared_stock_key_and_fixed_prices_are_the_originals() {
    // ServerConstants.SERVER_NAME.hashCode(): Java String.hashCode over UTF-16 units.
    assert_eq!(java_hash(""), 0);
    assert_eq!(java_hash("2009scape"), -1031243425);
    assert_eq!(java_hash("Aa"), java_hash("BB"));
    assert_eq!(java_hash("polygenelubricants"), i32::MIN);
    assert_eq!(fixed_price(12183), Some(25));
    assert_eq!(fixed_price(12155), Some(1));
    assert_eq!(fixed_price(9762), Some(99000));
    assert_eq!(fixed_price(9763), Some(99000));
    assert_eq!(fixed_price(995), None);
}
