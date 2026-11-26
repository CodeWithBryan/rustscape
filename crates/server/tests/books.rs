//! The book interface and the five global books replayed against the
//! unchanged BookInterface, GeneralRuleBook, GoblinBook, GrimDiary,
//! SecurityBookPlugin and StrongholdNotes.
#[path = "../../../oracle/books/native.rs"]
mod native;
use rs09_server::world::{
    books::registry::{self, Class},
    interaction_listeners::{self as interactions, Casing},
    interface_listeners,
};
use serde_json::Value;

const CORPUS: &str = include_str!("../../../fixtures/books/oracle.jsonl");

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
        return Some(format!("{path}: lengths {} != {}", a.len(), b.len()));
    }
    Some(format!("{path}: native={a}, original={b}"))
}

#[test]
fn original_books_every_page_and_button() {
    let mut failures = Vec::new();
    let mut cases = 0;
    for (index, line) in CORPUS.lines().enumerate() {
        let case: Value = serde_json::from_str(line).unwrap();
        let native = native::evaluate(&case);
        if let Some(found) = difference(&native, &case["expected"], String::new()) {
            failures.push(format!("case {index} ({}): {found}", case["name"]));
        }
        cases += 1;
    }
    assert!(
        failures.is_empty(),
        "{} of {cases} cases differ:\n{}",
        failures.len(),
        failures
            .iter()
            .take(25)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
    assert_eq!(cases, 1239);
}

#[test]
fn native_registrar_installs_the_book_listeners_in_scan_order() {
    struct Ascii;
    impl Casing for Ascii {
        fn lowercase_default(&self, s: &[u16]) -> Vec<u16> {
            String::from_utf16_lossy(s)
                .to_lowercase()
                .encode_utf16()
                .collect()
        }
        fn lowercase_root(&self, s: &[u16]) -> Vec<u16> {
            self.lowercase_default(s)
        }
    }
    let interactions = interactions::Registry::default();
    let interfaces = interface_listeners::Registry::default();
    let mut native = registry::Native {
        interactions: &interactions,
        interfaces: &interfaces,
        casing: &Ascii,
    };
    let done = registry::register_scanned(&mut native).unwrap();
    assert_eq!(
        done,
        [
            Class::BookInterface,
            Class::Goblin,
            Class::GrimDiary,
            Class::Security,
            Class::StrongholdNotes
        ]
    );
    for component in [26, 27, 49] {
        assert!(
            interfaces
                .button_listeners
                .read()
                .contains_key(&component.to_string())
        );
    }
}
