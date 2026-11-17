//! Replays `fixtures/wield-requirements` (the unchanged original
//! `ItemDefinition.hasRequirement` over 1,146 cases, `cargo xtask
//! wield-requirements-oracle`) against
//! `world::packet_handlers::requirement::has_requirement`: the result, the
//! exception class and every message.
use rs09_game_core::java_lower::lowercase_root;
use rs09_server::{
    definitions::{
        item::Shared,
        property::{Property, Requirements},
    },
    world::packet_handlers::{Error, requirement::has_requirement},
};
use serde_json::Value;

const FIXTURE: &str = include_str!("../../../fixtures/wield-requirements/oracle.jsonl");

#[test]
fn has_requirement_matches_the_original_over_every_case() {
    let mut cases = 0;
    for line in FIXTURE.lines().filter(|l| !l.is_empty()) {
        let row: Value = serde_json::from_str(line).unwrap();
        let configuration = match &row["requirements"] {
            Value::Null => None,
            Value::String(_) => Some(Property::Text("text".into())),
            Value::Array(pairs) => {
                let mut r = Requirements::default();
                for pair in pairs {
                    r.insert(
                        pair[0].as_i64().unwrap() as i32,
                        pair[1].as_i64().unwrap() as i32,
                    );
                }
                Some(Property::Requirements(Shared::new(r)))
            }
            other => panic!("unexpected configuration {other}"),
        };
        let levels: Vec<i32> = row["levels"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_i64().unwrap() as i32)
            .collect();
        let mut messages = Vec::new();
        let result = has_requirement(
            configuration.as_ref(),
            |skill| levels.get(skill).copied().ok_or(Error::Index(None)),
            row["wield"].as_bool().unwrap(),
            row["message"].as_bool().unwrap(),
            |text| {
                messages.push(text.to_owned());
                Ok(())
            },
            lowercase_root,
        );
        let expected = &row["expected"];
        let (result, error) = match result {
            Ok(b) => (Value::Bool(b), Value::Null),
            Err(Error::Cast) => (Value::Null, Value::String("ClassCastException".into())),
            Err(e) => (Value::Null, Value::String(format!("{e:?}"))),
        };
        assert_eq!(result, expected["result"], "{line}");
        assert_eq!(error, expected["error"], "{line}");
        let expected_messages: Vec<String> = expected["messages"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| m.as_str().unwrap().to_owned())
            .collect();
        assert_eq!(messages, expected_messages, "{line}");
        cases += 1;
    }
    assert_eq!(cases, 1146);
}
