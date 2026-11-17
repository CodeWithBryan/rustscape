#[path = "../../../oracle/item-config/native.rs"]
mod native;
use serde_json::Value;
use sha2::{Digest, Sha256};
const CORPUS: &str = include_str!("../../../fixtures/item-config/oracle.jsonl");
fn difference(a: &Value, b: &Value, path: String) -> Option<String> {
    if a == b {
        return None;
    }
    if let (Some(a), Some(b)) = (a.as_object(), b.as_object()) {
        for key in a.keys().chain(b.keys()) {
            if let Some(diff) = difference(
                a.get(key).unwrap_or(&Value::Null),
                b.get(key).unwrap_or(&Value::Null),
                format!("{path}/{key}"),
            ) {
                return Some(diff);
            }
        }
    }
    if let (Some(a), Some(b)) = (a.as_array(), b.as_array()) {
        if a.len() == b.len() {
            for (i, (a, b)) in a.iter().zip(b).enumerate() {
                if let Some(diff) = difference(a, b, format!("{path}/{i}")) {
                    return Some(diff);
                }
            }
        }
    }
    Some(format!("{path}: native={a}, original={b}"))
}
#[test]
fn item_configuration_types_order_partial_failures_and_json_grammar_match_original() {
    for (index, line) in CORPUS.lines().enumerate() {
        let case: Value = serde_json::from_str(line).unwrap();
        let actual = native::evaluate(&case);
        if let Some(diff) = difference(&actual, &case["expected"], String::new()) {
            panic!("item config case {index}: {diff}; steps={}", case["steps"]);
        }
    }
}
#[test]
fn item_configuration_fixtures_pin_original_loader_value_classes_and_adapters() {
    let p: Value = serde_json::from_str(include_str!(
        "../../../fixtures/item-config/provenance.json"
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
#[test]
fn native_file_loading_retains_reader_on_failed_open_and_preserves_utf8() {
    use rs09_server::definitions::{
        item::{ItemDefinition, Registry, Shared},
        item_config::Parser,
    };
    use std::io::Read;
    let base = std::env::temp_dir().join(format!(
        "rs09-config-file-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&base).unwrap();
    let path = base.join("item_configs.json");
    std::fs::write(&path, "[{\"id\":1,\"name\":\"à😀\"}]").unwrap();
    let mut definitions = Registry::default();
    definitions.insert(1, Shared::new(ItemDefinition::default()));
    let mut parser = Parser::default();
    let mut log = native::Log::default();
    assert_eq!(parser.load_file(&path, &definitions, &mut log).unwrap(), 1);
    assert_eq!(
        definitions.for_id(1).read().handlers["name"].as_str(),
        Some("à😀")
    );
    assert_eq!(parser.reader.as_mut().unwrap().read(&mut [0]).unwrap(), 0);
    assert!(
        parser
            .load_file(&base.join("missing"), &definitions, &mut log)
            .is_err()
    );
    assert!(parser.load_file(&base, &definitions, &mut log).is_err());
    assert_eq!(parser.reader.as_mut().unwrap().read(&mut [0]).unwrap(), 0);
    drop(parser);
    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(base).unwrap();
}
#[test]
fn malformed_utf16_and_excessive_nesting_are_bounded_native_errors() {
    use rs09_server::definitions::item_config::json::{ErrorKind, parse};
    assert_eq!(parse("\"\\uD800\"").unwrap_err().kind, ErrorKind::Unicode);
    let deep = format!("{}{}", "[".repeat(258), "]".repeat(258));
    assert_eq!(parse(&deep).unwrap_err().kind, ErrorKind::Limit);
}
