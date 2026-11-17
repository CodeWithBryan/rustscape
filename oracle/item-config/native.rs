use rs09_server::definitions::{
    item::{ItemDefinition, Registry, Shared},
    item_config::{Error, Logger, Parser, json::ErrorKind},
    property::Property,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
type Ids = BTreeMap<usize, usize>;
fn tag<T>(value: &Shared<T>, ids: &mut Ids) -> Value {
    let next = ids.len();
    json!(ids.entry(value.identity()).or_insert(next))
}
pub fn property(value: &Property, ids: &mut Ids) -> Value {
    match value {
        Property::Null => json!({"type":"null"}),
        Property::Int(v) => json!({"type":"int","value":v}),
        Property::Long(v) => json!({"type":"long","value":v}),
        Property::Bool(v) => json!({"type":"bool","value":v}),
        Property::Text(v) => json!({"type":"text","value":v}),
        Property::Double(v) => {
            json!({"type":"double","bits":format!("{:016x}",if v.is_nan(){f64::NAN.to_bits()}else{v.to_bits()})})
        }
        Property::IntArray(v) => json!({"type":"ints","ref":tag(v,ids),"values":&*v.read()}),
        Property::Requirements(v) => {
            let reference = tag(v, ids);
            let v = v.read();
            json!({"type":"requirements","ref":reference,"values":v.values(),"order":v.order()})
        }
        Property::Animation(v) => {
            let reference = tag(v, ids);
            let v = v.read();
            json!({"type":"animation","ref":reference,"id":v.id,"delay":v.delay,"priority":v.priority})
        }
        Property::Animations(v) => {
            let reference = tag(v, ids);
            let values: Vec<_> = v
                .read()
                .iter()
                .map(|v| match v {
                    Some(v) => property(&Property::Animation(v.clone()), ids),
                    None => property(&Property::Null, ids),
                })
                .collect();
            json!({"type":"animations","ref":reference,"values":values})
        }
        Property::Audios(v) => {
            let reference = tag(v, ids);
            let values:Vec<_>=v.read().iter().map(|v|{let reference=tag(v,ids);let v=v.read();json!({"type":"audio","ref":reference,"id":v.id,"delay":v.delay,"loops":v.loops,"radius":v.radius})}).collect();
            json!({"type":"audios","ref":reference,"values":values})
        }
        Property::CombatStyle(_) | Property::ClueLevel(_) => {
            panic!("NPC metadata outside item configuration corpus")
        }
        Property::OptionHandler(_) => {
            panic!("content option handler outside item configuration corpus")
        }
        Property::Plugin(_) => panic!("native plugin outside item configuration corpus"),
        Property::Json(v) => panic!("untyped property outside config corpus {v}"),
    }
}
#[derive(Default)]
pub struct Log {
    pub logs: Vec<String>,
    pub fail: bool,
}
impl Logger for Log {
    fn parsed(&mut self, count: i32) -> Result<(), Error> {
        self.logs.push(format!("Parsed {count} item configs."));
        if self.fail {
            Err(Error::Binding("IllegalStateException".into()))
        } else {
            Ok(())
        }
    }
}
pub fn state(parser: &Parser, definitions: &Registry, retained: &[Property], log: &Log) -> Value {
    let mut ids = Ids::new();
    let mut rows = BTreeMap::new();
    let mut keys: Vec<_> = definitions.definitions().keys().copied().collect();
    keys.sort_unstable();
    for key in keys {
        let handle = definitions.for_id(key);
        let def = handle.read();
        let handlers: BTreeMap<_, _> = def
            .handlers
            .iter()
            .map(|(key, value)| (key.clone(), property(value, &mut ids)))
            .collect();
        rows.insert(key.to_string(),json!({"id":def.id,"name":def.name,"value":def.value,"examine":def.examine,"handlers":handlers}));
    }
    let retained: Vec<_> = retained
        .iter()
        .map(|value| property(value, &mut ids))
        .collect();
    json!({"reader":parser.reader.is_some(),"definitions":rows,"retained":retained,"logs":log.logs})
}
pub fn initial(value: Option<&Value>) -> Registry {
    let default = json!({"0":{"id":0,"name":"zero"},"1":{"id":1,"name":"original","value":100,"examine":"cached","handlers":{"old":true}},"99":{"id":99}});
    let mut definitions = Registry::default();
    for (id, fields) in value.unwrap_or(&default).as_object().unwrap() {
        definitions.insert(
            id.parse().unwrap(),
            Shared::new(serde_json::from_value::<ItemDefinition>(fields.clone()).unwrap()),
        );
    }
    definitions
}
pub fn error(error: Error) -> (String, Option<usize>) {
    match error {
        Error::Json(error) => match error.kind {
            ErrorKind::Number => ("NumberFormatException".into(), None),
            ErrorKind::Unicode => ("NativeInvalidUnicode".into(), None),
            ErrorKind::Limit => ("NativeLimit".into(), None),
            _ => ("ParseException".into(), Some(error.position)),
        },
        Error::Null => ("NullPointerException".into(), None),
        Error::Type => ("ClassCastException".into(), None),
        Error::Number => ("NumberFormatException".into(), None),
        Error::Bounds => ("IndexOutOfBoundsException".into(), None),
        Error::Io(_) => ("FileNotFoundException".into(), None),
        Error::Binding(value) => (value, None),
    }
}
pub fn evaluate(case: &Value) -> Value {
    if case["kind"] == "json" {
        return match rs09_server::definitions::item_config::json::parse(
            case["text"].as_str().unwrap(),
        ) {
            Ok(value) => {
                use rs09_server::definitions::item_config::json::Json;
                let kind = match &value {
                    Json::Null => "null",
                    Json::Bool(_) => "bool",
                    Json::Int(_) => "long",
                    Json::Double(_) => "double",
                    Json::Text(_) => "text",
                    Json::Array(_) => "array",
                    Json::Object(_) => "object",
                };
                json!({"error":null,"position":null,"type":kind,"text":value.text()})
            }
            Err(e) => {
                let (e, position) = error(Error::Json(e));
                json!({"error":e,"position":position})
            }
        };
    }
    if case["kind"] == "constants" {
        return json!(
            rs09_server::definitions::item_config::constants::ALL
                .iter()
                .copied()
                .collect::<BTreeMap<_, _>>()
        );
    }
    let definitions = initial(case.get("initial"));
    let mut parser = Parser::default();
    let mut log = Log {
        fail: case["logFail"] == true,
        ..Log::default()
    };
    let mut retained = Vec::new();
    let mut output = Vec::new();
    for step in case["steps"].as_array().unwrap() {
        let result = if let Some(keys) = step["capture"].as_array() {
            for key in keys {
                retained.push(
                    definitions
                        .for_id(1)
                        .read()
                        .handlers
                        .get(key.as_str().unwrap())
                        .cloned()
                        .unwrap_or(Property::Null),
                );
            }
            Ok(0)
        } else if let Some(index) = step["mutate"].as_u64() {
            if let Property::Requirements(map) = &retained[index as usize] {
                map.write().insert(
                    step["key"].as_i64().unwrap() as i32,
                    step["value"].as_i64().unwrap() as i32,
                );
                Ok(0)
            } else {
                Err(Error::Type)
            }
        } else if step["missing"] == true {
            // A fresh owned empty directory makes File::open fail without touching
            // any existing user path. No recursive deletion is performed.
            let path = std::env::temp_dir().join(format!(
                "rs09-item-config-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            std::fs::create_dir(&path).unwrap();
            let result = parser.load_file(&path.join("missing.json"), &definitions, &mut log);
            std::fs::remove_dir(&path).unwrap();
            result
        } else {
            parser.load_text(step["text"].as_str().unwrap(), &definitions, &mut log)
        };
        let (error, position) = match result {
            Ok(_) => (None, None),
            Err(e) => {
                let (e, p) = error(e);
                (Some(e), p)
            }
        };
        output.push(json!({"error":error,"position":position,"state":state(&parser,&definitions,&retained,&log)}));
    }
    json!(output)
}
