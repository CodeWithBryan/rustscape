//! Oracle-only observation/command adapter, shared by regression tests and full-cache audit.
use rs09_protocol::buffer::Reader;
use rs09_server::definitions::item::{self, Handle, ItemDefinition, Registry, Shared};
use serde_json::{Value, json};
use std::collections::BTreeMap;

fn integer(value: &Value) -> i32 {
    value.as_i64().unwrap() as i32
}
pub fn bytes(value: &Value) -> Vec<u8> {
    let hex = value.as_str().unwrap().as_bytes();
    hex.chunks_exact(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}
fn reference<T>(value: &Option<Shared<T>>, ids: &mut BTreeMap<usize, usize>) -> Value {
    value.as_ref().map_or(Value::Null, |value| tag(value, ids))
}
fn tag<T>(value: &Shared<T>, ids: &mut BTreeMap<usize, usize>) -> Value {
    let next = ids.len();
    json!(ids.entry(value.identity()).or_insert(next))
}
pub fn state(registry: &Registry, select: Option<&Value>) -> Value {
    let mut keys: Vec<i32> = select.map_or_else(
        || registry.definitions().keys().copied().collect(),
        |value| value.as_array().unwrap().iter().map(integer).collect(),
    );
    keys.sort_unstable();
    let mut records = BTreeMap::new();
    let mut identities = BTreeMap::new();
    let mut ids = BTreeMap::new();
    for key in keys {
        let Some(handle) = registry.definitions().get(&key) else {
            continue;
        };
        let def = handle.read();
        records.insert(key.to_string(), serde_json::to_value(&*def).unwrap());
        let mut refs = BTreeMap::new();
        refs.insert("self", tag(handle, &mut ids));
        // Match Java's sorted declared-field traversal, independent of JSON map order.
        macro_rules! field {
            ($name:literal, $field:ident) => {
                refs.insert($name, reference(&def.$field, &mut ids));
            };
        }
        field!("clientScriptData", client_script_data);
        field!("groundActions", ground_actions);
        field!("itemRequirements", item_requirements);
        field!("modifiedModelColors", modified_model_colors);
        field!("options", options);
        field!("originalModelColors", original_model_colors);
        field!("stackAmounts", stack_amounts);
        field!("stackIds", stack_ids);
        field!("textureColour1", texture_colour1);
        field!("textureColour2", texture_colour2);
        field!("unknownArray1", unknown_array1);
        field!("unknownArray2", unknown_array2);
        field!("unknownArray3", unknown_array3);
        identities.insert(key.to_string(), refs);
    }
    json!({"definitions":records,"identities":identities,"size":registry.definitions().len()})
}
fn query(handle: Handle) -> Value {
    let def = handle.read();
    json!({"stackable":def.is_stackable(),"playerType":def.is_player_type(),"destroy":def.has_destroy_action(),"wear":def.has_wear_action(),"special":def.has_special_bar(),"quest":def.quest_id(),"archive":def.archive_id(),"file":def.file_id(),"nullAction":def.has_action(None),"take":def.has_action(Some("take"))})
}
fn mutate(handle: Handle, action: &Value) -> std::result::Result<(), &'static str> {
    let def = handle.read();
    let field = action["field"].as_str().unwrap();
    if field == "clientScriptData" {
        def.client_script_data
            .as_ref()
            .ok_or("NullPointerException")?
            .write()
            .insert(
                action["key"].as_str().unwrap().parse().unwrap(),
                action["value"].clone(),
            );
        return Ok(());
    }
    let index = integer(&action["index"]) as usize;
    macro_rules! array {
        ($field:ident,$value:expr) => {{
            let shared = def.$field.as_ref().ok_or("NullPointerException")?;
            let mut values = shared.write();
            let value = values
                .get_mut(index)
                .ok_or("ArrayIndexOutOfBoundsException")?;
            *value = $value;
        }};
    }
    match field {
        "groundActions" => array!(ground_actions, action["value"].as_str().map(str::to_owned)),
        "options" => array!(options, action["value"].as_str().map(str::to_owned)),
        "originalModelColors" => array!(original_model_colors, integer(&action["value"]) as i16),
        "modifiedModelColors" => array!(modified_model_colors, integer(&action["value"]) as i16),
        "textureColour1" => array!(texture_colour1, integer(&action["value"]) as i16),
        "textureColour2" => array!(texture_colour2, integer(&action["value"]) as i16),
        "unknownArray1" => array!(unknown_array1, integer(&action["value"]) as i8),
        _ => panic!("unhandled array {field}"),
    }
    Ok(())
}
fn action(registry: &mut Registry, action: &Value) -> std::result::Result<Value, &'static str> {
    match action["op"].as_str().unwrap() {
        "note" | "lend" | "recolour" => {
            let target = registry.for_id(integer(&action["to"]));
            let base = registry.for_id(integer(&action["base"]));
            let template = registry.for_id(integer(&action["template"]));
            match action["op"].as_str().unwrap() {
                "note" => item::transfer_note(&target, &base, &template),
                "lend" => item::transfer_lend(&target, &base, &template),
                _ => item::transfer_recolour(&target, &base, &template),
            }
        }
        "templates" => registry.define_templates(integer(&action["count"])),
        "query" => return Ok(query(registry.for_id(integer(&action["slot"])))),
        "load" => registry
            .parse_files(integer(&action["count"]), |id| {
                Ok(action["files"][id.to_string()]
                    .as_str()
                    .map(|_| bytes(&action["files"][id.to_string()])))
            })
            .map_err(|_| "BufferUnderflowException")?,
        "missing" => {
            let first = registry.for_id(integer(&action["id"]));
            let second = registry.for_id(integer(&action["id"]));
            first.write().name = Some("changed".into());
            return Ok(
                json!({"same":first.same(&second),"first":&*first.read(),"second":&*second.read()}),
            );
        }
        "alias" => {
            registry.insert(
                integer(&action["slot"]),
                registry.for_id(integer(&action["source"])),
            );
        }
        "mutate" => mutate(registry.for_id(integer(&action["slot"])), action)?,
        op => panic!("unknown action {op}"),
    }
    Ok(Value::Null)
}
pub fn evaluate(case: &Value) -> Value {
    if case["kind"] == "casePairs" {
        return json!(
            (0..=u16::MAX)
                .filter_map(|code| {
                    let folded = rs09_game_core::java_string::fold_case_unit(code);
                    (code != folded).then_some([code, folded])
                })
                .collect::<Vec<_>>()
        );
    }
    if case["kind"] == "caseCompare" {
        return json!(rs09_game_core::java_string::equals_ignore_case(
            case["left"].as_str().unwrap(),
            case["right"].as_str().unwrap()
        ));
    }
    if case["kind"] == "decode" {
        let input = bytes(&case["data"]);
        let mut reader = Reader::new(&input);
        let result = ItemDefinition::decode(integer(&case["id"]), &mut reader);
        return match result {
            Ok(def) => json!({"definition":def,"position":reader.position(),"error":null}),
            Err(_) => {
                json!({"definition":null,"position":reader.position(),"error":"BufferUnderflowException"})
            }
        };
    }
    let mut registry = Registry::default();
    for row in case["initial"].as_array().unwrap() {
        registry.insert(
            integer(&row["slot"]),
            Shared::new(serde_json::from_value(row["fields"].clone()).unwrap()),
        );
    }
    let mut trace = Vec::new();
    for command in case["actions"].as_array().unwrap() {
        let (result, error) = match action(&mut registry, command) {
            Ok(result) => (result, None),
            Err(error) => (Value::Null, Some(error)),
        };
        trace.push(
            json!({"result":result,"error":error,"state":state(&registry,case.get("select"))}),
        );
    }
    json!(trace)
}
