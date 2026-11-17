use rs09_server::definitions::{
    item::{self, Shared},
    option_handler::{self as option, Context, Defaults, Handler, NativeOptionHandler, Result},
    property::Property,
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
fn n(v: &Value, key: &str, d: i32) -> i32 {
    v[key].as_i64().map_or(d, |v| v as i32)
}
struct Program;
impl NativeOptionHandler for Program {}
struct Model {
    input: Value,
    defaults: option::DefaultHandle,
    handlers: Vec<option::Handle>,
    definitions: Vec<item::Handle>,
    cache: BTreeMap<i32, Option<usize>>,
    captures: Vec<option::DefaultHandle>,
    keys: BTreeSet<Option<String>>,
    null_definition: bool,
    events: Vec<Value>,
    calls: BTreeMap<String, i32>,
}
impl Model {
    fn emit(&mut self, event: &str, args: Vec<Value>) -> Result<()> {
        let mut row = vec![json!(event)];
        row.extend(args);
        self.events.push(json!(row));
        let count = self.calls.entry(event.into()).or_default();
        *count += 1;
        let count = *count;
        for hook in self.input["hooks"].as_array().cloned().unwrap_or_default() {
            if hook["event"] == event && n(&hook, "at", 1) == count {
                for a in hook["actions"].as_array().unwrap() {
                    self.act(a)?;
                }
            }
        }
        Ok(())
    }
    fn tag(&self, h: Option<&option::Handle>) -> Value {
        h.map_or(Value::Null, |h| {
            json!(self.handlers.iter().position(|v| v.same(h)).unwrap())
        })
    }
    fn selected(&self, a: &Value) -> Option<option::Handle> {
        a["handler"]
            .as_u64()
            .map(|i| self.handlers[i as usize].clone())
    }
    fn act(&mut self, a: &Value) -> Result<Value> {
        let name = a["name"].as_str().map(str::to_owned);
        let selected = self.selected(a);
        let map = self
            .captures
            .get(n(a, "map", 0) as usize)
            .cloned()
            .unwrap_or_else(|| self.defaults.clone());
        match a["op"].as_str().unwrap() {
            "fail" => return Err(option::Error::Binding("failure".into())),
            "capture" => {
                self.captures.push(self.defaults.clone());
                return Ok(json!(self.captures.len() - 1));
            }
            "register" => {
                self.keys.insert(name.clone());
                return Ok(json!(option::set_option_handler(
                    &self.defaults,
                    name,
                    selected
                )));
            }
            "put" => {
                self.keys.insert(name.clone());
                let prior = map.write().put(name, selected);
                return Ok(self.tag(prior.as_ref()));
            }
            "get" => return Ok(self.tag(map.read().get(name.as_deref()).as_ref())),
            "contains" => return Ok(json!(map.read().contains_key(name.as_deref()))),
            "remove" => return Ok(self.tag(map.write().remove(name.as_deref()).as_ref())),
            "clear" => map.write().clear(),
            "lookup" => {
                let defaults = self.defaults.clone();
                let h = option::item_handler(self, &defaults, n(a, "id", 1), name.as_deref())?;
                return Ok(self.tag(h.as_ref()));
            }
            "config" => {
                let value = if let Some(id) = a["value"].as_u64() {
                    Property::OptionHandler(self.handlers[id as usize].clone())
                } else if a["value"].is_null() {
                    Property::Null
                } else {
                    Property::Text(a["value"].as_str().unwrap().into())
                };
                self.definitions[n(a, "definition", 0) as usize]
                    .write()
                    .handlers
                    .insert(a["key"].as_str().unwrap().into(), value);
            }
            "set" => {
                if let Some(v) = a.get("nullDefinition") {
                    self.null_definition = *v == true;
                }
            }
            "cache" => {
                self.cache
                    .insert(n(a, "id", 1), a["definition"].as_u64().map(|v| v as usize));
            }
            op => panic!("unknown {op}"),
        }
        Ok(Value::Null)
    }
    fn snapshot(&self, result: Value, error: Option<String>) -> Value {
        let mut keys = self.keys.iter().collect::<Vec<_>>();
        keys.sort_by(|a, b| match (a, b) {
            (None, None) => std::cmp::Ordering::Equal,
            (None, _) => std::cmp::Ordering::Less,
            (_, None) => std::cmp::Ordering::Greater,
            (Some(a), Some(b)) => a.encode_utf16().cmp(b.encode_utf16()),
        });
        let global = keys
            .into_iter()
            .filter(|k| self.defaults.read().contains_key(k.as_deref()))
            .map(|k| json!([k, self.tag(self.defaults.read().get(k.as_deref()).as_ref())]))
            .collect::<Vec<_>>();
        let defs = self
            .definitions
            .iter()
            .map(|d| {
                let d = d.read();
                let mut keys = d.handlers.keys().collect::<Vec<_>>();
                keys.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
                keys.into_iter()
                    .map(|k| {
                        let v = &d.handlers[k];
                        json!([
                            k,
                            match v {
                                Property::OptionHandler(h) => json!(["handler", self.tag(Some(h))]),
                                _ => v.observation(),
                            }
                        ])
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        json!({"result":result,"error":error,"events":self.events,"global":global,"size":self.defaults.read().len(),"empty":self.defaults.read().is_empty(),"definitions":defs,"cache":self.cache.iter().map(|(k,v)|json!([k,v])).collect::<Vec<_>>(),"captures":self.captures.len(),"nullDefinition":self.null_definition})
    }
}
impl Context for Model {
    fn definition(&mut self, id: i32) -> Result<Option<item::Handle>> {
        self.emit("lookup", vec![json!(id)])?;
        if self.null_definition {
            return Ok(None);
        }
        if let Some(index) = self.cache.get(&id).copied().flatten() {
            return Ok(Some(self.definitions[index].clone()));
        }
        let d = item::Registry::default().for_id(id);
        self.definitions.push(d.clone());
        Ok(Some(d))
    }
    fn configuration(&mut self, d: &item::Handle, key: &str) -> Result<Option<Property>> {
        let index = self.definitions.iter().position(|v| v.same(d)).unwrap();
        let value = d.read().handlers.get(key).cloned();
        self.emit("configuration", vec![json!(index), json!(key)])?;
        Ok(value)
    }
    fn missing_definition(&mut self, id: i32) -> Result<()> {
        self.emit(
            "log",
            vec![
                json!("core.cache.def.impl.ItemDefinition"),
                json!("ERR"),
                json!(format!("[ItemDefinition] No definition for item id {id}!")),
            ],
        )
    }
}
pub fn evaluate(input: &Value) -> Value {
    let mut m = Model {
        input: input.clone(),
        defaults: Shared::new(Defaults::default()),
        handlers: (0..3)
            .map(|i| Handler::new(format!("h{i}"), Program))
            .collect(),
        definitions: (0..2)
            .map(|_| Shared::new(item::ItemDefinition::default()))
            .collect(),
        cache: BTreeMap::from([(1, Some(0)), (2, Some(1))]),
        captures: Vec::new(),
        keys: BTreeSet::new(),
        null_definition: input["nullDefinition"] == true,
        events: Vec::new(),
        calls: BTreeMap::new(),
    };
    let mut states = Vec::new();
    for a in input["actions"].as_array().unwrap() {
        m.events.clear();
        let (result, error) = match m.act(a) {
            Ok(v) => (v, None),
            Err(e) => (
                Value::Null,
                Some(match e {
                    option::Error::Type => "Type".into(),
                    option::Error::Binding(_) => "Binding".into(),
                }),
            ),
        };
        states.push(m.snapshot(result, error));
    }
    json!(states)
}
