//! Test-only observations and cache/player/log bindings for native scenery data.
mod fields;
use rs09_protocol::buffer::Reader;
use rs09_server::definitions::{
    item::Shared,
    scenery::{Error, Handle, LogContext, PlayerContext, Registry, Result, SceneryDefinition},
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
    logs: i32,
}
pub fn events(model: &Model) -> Value {
    json!(model.events)
}
impl LogContext for Model {
    fn log(&mut self, _: &mut Registry, message: String) -> Result<()> {
        self.events.push(json!(["log", message]));
        self.logs += 1;
        if self.logs == v(&self.input, "failLog", -1) {
            Err(Error::Binding("log".into()))
        } else {
            Ok(())
        }
    }
}
impl PlayerContext for Model {
    fn varbit(&mut self, id: i32, _: usize) -> Result<Option<i32>> {
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
    fn varp(&mut self, id: i32, _: usize) -> Result<i32> {
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
    json!({"definitions":defs,"identities":identities,"size":registry.definitions.len(),"retained":retained,"retainedIdentities":refs})
}
fn snapshot(h: &Handle) -> Value {
    serde_json::to_value(&*h.read()).unwrap()
}
fn make(value: &Value) -> Handle {
    let h = Shared::new(SceneryDefinition::default());
    for (k, v) in value.as_object().unwrap() {
        fields::set(&h, k, v);
    }
    h
}
fn target(model: &Model, registry: &mut Registry, a: &Value) -> Result<Handle> {
    if let Some(name) = a["ref"].as_str() {
        model.retained[name].clone().ok_or(Error::Null)
    } else {
        Ok(registry.for_id(v(a, "slot", 0)))
    }
}
fn action(model: &mut Model, registry: &mut Registry, a: &Value) -> Result<Value> {
    match a["op"].as_str().unwrap() {
        "load" => {
            registry.parse_files(model, v(a, "count", 0), |id| {
                if a["failFile"].as_i64().map(|v| v as i32) == Some(id) {
                    return Err(Error::Binding("file".into()));
                }
                Ok(a["files"][id.to_string()]
                    .as_str()
                    .map(|_| bytes(&a["files"][id.to_string()])))
            })?;
        }
        "put" => {
            registry
                .definitions
                .insert(n(&a["slot"]), make(&a["fields"]));
        }
        "decode" => {
            let h = registry.decode(model, v(a, "id", 0), &mut Reader::new(&bytes(&a["data"])))?;
            registry.definitions.insert(n(&a["slot"]), h);
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
            let to = registry.for_id(n(&a["to"]));
            fields::share(&h, &to, a["field"].as_str().unwrap());
        }
        "array" => fields::mutate(
            &target(model, registry, a)?,
            a["field"].as_str().unwrap(),
            n(&a["index"]) as usize,
            &a["value"],
        )?,
        "configure" => {
            let h = target(model, registry, a)?;
            registry.configure(&h)?;
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
        "hasActions" => {
            let h = target(model, registry, a)?;
            return Ok(json!(registry.has_actions(&h)?));
        }
        "child" | "childIndex" => {
            let h = target(model, registry, a)?;
            let child = if a["op"] == "child" {
                registry.child(model, &h, if a["player"] == true { Some(0) } else { None })?
            } else {
                registry.child_at_index(&h, n(&a["index"]))
            };
            let result = snapshot(&child);
            if let Some(name) = a["name"].as_str() {
                model.retained.insert(name.into(), Some(child));
            }
            return Ok(result);
        }
        "container" => return Ok(json!(SceneryDefinition::container_id(n(&a["id"])))),
        _ => unreachable!(),
    };
    Ok(Value::Null)
}
fn class(e: &Error) -> &str {
    match e {
        Error::Decode(_) => "BufferUnderflowException",
        Error::Null => "NullPointerException",
        Error::Index => "ArrayIndexOutOfBoundsException",
        Error::Position => "IllegalArgumentException",
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
        let (definition, error) = match registry.decode(&mut model, v(input, "id", 0), &mut r) {
            Ok(h) => (snapshot(&h), Value::Null),
            Err(e) => (Value::Null, json!(class(&e))),
        };
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
