//! Test-only observations and cache/player/log bindings for native npc data.
mod fields;
use rs09_protocol::buffer::Reader;
use rs09_server::definitions::{
    item::Shared,
    npc::{Error, Files, Handle, NpcDefinition, PlayerContext, Registry, Result},
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
fn n(v: &Value) -> i32 {
    v.as_i64().unwrap() as i32
}
fn v(a: &Value, k: &str, d: i32) -> i32 {
    a[k].as_i64().map_or(d, |n| n as i32)
}
pub fn bytes(v: &Value) -> Vec<u8> {
    v.as_str()
        .unwrap()
        .as_bytes()
        .chunks_exact(2)
        .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
        .collect()
}
#[derive(Default)]
pub struct Model {
    input: Value,
    retained: BTreeMap<String, Option<Handle>>,
    events: Vec<Value>,
    files: BTreeMap<i32, Vec<u8>>,
    fail_file: Option<i32>,
    parser: rs09_server::definitions::npc::config::Parser,
    logs: i32,
}
pub fn events(model: &Model) -> Value {
    json!(model.events)
}
impl Files for Model {
    fn file(&mut self, _: &mut Registry, id: i32) -> Result<Option<Vec<u8>>> {
        self.events.push(json!(["file", id]));
        if self.fail_file == Some(id) {
            return Err(Error::Binding("file".into()));
        }
        Ok(self.files.get(&id).cloned())
    }
}
impl rs09_server::definitions::npc::config::Context for Model {
    fn log(
        &mut self,
        _: &mut Registry,
        _: rs09_server::definitions::npc::config::Level,
        text: String,
    ) -> Result<()> {
        self.events.push(json!(["log", text]));
        self.logs += 1;
        if self.logs == v(&self.input, "failLog", -1) {
            Err(Error::Binding("log".into()))
        } else {
            Ok(())
        }
    }
}
impl PlayerContext for Model {
    fn varbit(&mut self, _: &mut Registry, id: i32, _: usize) -> Result<Option<i32>> {
        self.events.push(json!(["varbit", id]));
        if self.input["failVarbit"] == true {
            return Err(Error::Binding("varbit".into()));
        }
        Ok(if self.input["missingVarbit"] == true {
            None
        } else {
            Some(v(&self.input, "configValue", 0))
        })
    }
    fn varp(&mut self, _: &mut Registry, id: i32, _: usize) -> Result<i32> {
        self.events.push(json!(["varp", id]));
        if self.input["failVarp"] == true {
            Err(Error::Binding("varp".into()))
        } else {
            Ok(v(&self.input, "configValue", 0))
        }
    }
}
pub fn state(registry: &Registry, model: &Model) -> Value {
    let mut keys: Vec<_> = registry.definitions.keys().copied().collect();
    keys.sort_unstable();
    let mut defs = BTreeMap::new();
    let mut identities = BTreeMap::new();
    let mut ids = BTreeMap::new();
    for k in keys {
        let h = &registry.definitions[&k];
        defs.insert(k.to_string(), serde_json::to_value(&*h.read()).unwrap());
        identities.insert(k.to_string(), fields::references(h, &mut ids));
    }
    let mut retained = BTreeMap::new();
    let mut refs = BTreeMap::new();
    for (k, h) in &model.retained {
        retained.insert(
            k,
            h.as_ref()
                .map(|h| serde_json::to_value(&*h.read()).unwrap()),
        );
        refs.insert(k, h.as_ref().map(|h| fields::references(h, &mut ids)));
    }
    json!({"definitions":defs,"identities":identities,"size":registry.definitions.len(),"retained":retained,"retainedIdentities":refs,"reader":model.parser.reader.is_some()})
}
fn snapshot(h: &Handle) -> Value {
    serde_json::to_value(&*h.read()).unwrap()
}
fn make(value: &Value) -> Handle {
    let h = Shared::new(NpcDefinition::default());
    for (k, v) in value.as_object().unwrap() {
        fields::set(&h, k, v);
    }
    h
}
fn target(model: &mut Model, registry: &mut Registry, a: &Value) -> Result<Handle> {
    if let Some(name) = a["ref"].as_str() {
        model.retained[name].clone().ok_or(Error::Null)
    } else {
        registry.for_id(model, v(a, "slot", 0))
    }
}
fn action(model: &mut Model, registry: &mut Registry, a: &Value) -> Result<Value> {
    match a["op"].as_str().unwrap() {
        "load" => {
            model.files = a["files"]
                .as_object()
                .unwrap()
                .iter()
                .map(|(k, v)| (k.parse().unwrap(), bytes(v)))
                .collect();
            model.fail_file = a["failFile"].as_i64().map(|v| v as i32);
        }
        "preload" => {
            for id in 0..v(a, "count", 0) {
                registry.for_id(model, id)?;
            }
        }
        "lookup" => {
            return Ok(snapshot(&registry.for_id(model, n(&a["id"]))?));
        }
        "config" => {
            let text = a["text"].as_str().unwrap_or(include_str!(
                "../../fixtures/npc-definitions/npc_configs.json"
            ));
            let mut parser = std::mem::take(&mut model.parser);
            let result = parser.load_text(text, registry, model);
            model.parser = parser;
            result?;
        }
        "put" => {
            registry
                .definitions
                .insert(n(&a["slot"]), make(&a["fields"]));
        }
        "parse" => {
            let h = target(model, registry, a)?;
            rs09_server::definitions::npc::parse(&h, &mut Reader::new(&bytes(&a["data"])))?;
        }
        "retain" => {
            let h = target(model, registry, a)?;
            model
                .retained
                .insert(a["name"].as_str().unwrap().into(), Some(h));
        }
        "alias" => {
            let h = target(model, registry, a)?;
            registry.definitions.insert(n(&a["to"]), h);
        }
        "set" => {
            for (k, v) in a["fields"].as_object().unwrap() {
                fields::set(&target(model, registry, a)?, k, v);
            }
        }
        "share" => {
            let h = target(model, registry, a)?;
            let to = registry.for_id(model, n(&a["to"]))?;
            fields::share(&h, &to, a["field"].as_str().unwrap());
        }
        "array" => fields::mutate(
            &target(model, registry, a)?,
            a["field"].as_str().unwrap(),
            n(&a["index"]) as usize,
            &a["value"],
        )?,
        "animation" => {
            let h = target(model, registry, a)?;
            let animation = h.read().combat_animation(v(a, "index", 0))?;
            return Ok(animation.map_or(Value::Null, |a| json!({"$animation":a})));
        }
        "graphics" => {
            let h = target(model, registry, a)?;
            let values = if a.get("values").is_some() {
                Shared::new(serde_json::from_value(a["values"].clone()).unwrap())
            } else {
                h.read().handlers.clone()
            };
            rs09_server::definitions::npc::init_combat_graphics(&h, &values)?;
        }
        "hasOptions" => {
            return Ok(json!(
                target(model, registry, a)?
                    .read()
                    .has_options(a["examine"] == true)?
            ));
        }
        "hasAction" => {
            return Ok(json!(
                target(model, registry, a)?
                    .read()
                    .has_action(a["value"].as_str())
            ));
        }
        "hasAttack" => {
            return Ok(json!(
                target(model, registry, a)?.read().has_attack_option()?
            ));
        }
        "child" => {
            let h = target(model, registry, a)?;
            let child =
                registry.child(model, &h, if a["player"] == true { Some(0) } else { None })?;
            let result = snapshot(&child);
            if let Some(name) = a["name"].as_str() {
                model.retained.insert(name.into(), Some(child));
            }
            return Ok(result);
        }
        _ => unreachable!(),
    };
    Ok(Value::Null)
}
fn class(e: &Error) -> &str {
    match e {
        Error::Decode(_) => "BufferUnderflowException",
        Error::Null => "NullPointerException",
        Error::Index => "ArrayIndexOutOfBoundsException",
        Error::Type => "ClassCastException",
        Error::Number => "NumberFormatException",
        Error::Json(_) => "ParseException",
        _ => "Binding",
    }
}
pub fn evaluate(input: &Value) -> Value {
    let mut model = Model {
        input: input.clone(),
        ..Model::default()
    };
    let mut registry = Registry::default();
    if input["kind"] == "decode" {
        let data = bytes(&input["data"]);
        let mut r = Reader::new(&data);
        let h = Shared::new(NpcDefinition {
            id: v(input, "id", 0),
            ..NpcDefinition::default()
        });
        let error = rs09_server::definitions::npc::parse(&h, &mut r)
            .err()
            .map(|e| class(&e).to_owned());
        let definition = snapshot(&h);
        return json!({"definition":definition,"state":state(&registry,&model),"position":r.position(),"error":error,"events":events(&model)});
    }
    if let Some(rows) = input["initial"].as_array() {
        for row in rows {
            registry
                .definitions
                .insert(n(&row["slot"]), make(&row["fields"]));
        }
    }
    let mut out = vec![];
    for a in input["actions"].as_array().unwrap() {
        model.events.clear();
        let (result, error) = match action(&mut model, &mut registry, a) {
            Ok(v) => (v, Value::Null),
            Err(e) => (Value::Null, json!(class(&e))),
        };
        out.push(json!({"result":result,"error":error,"state":state(&registry,&model),"events":events(&model)}));
    }
    json!(out)
}
