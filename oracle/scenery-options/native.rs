use rs09_server::{
    definitions::{item::Shared, option_handler, property::Property, scenery as defs},
    world::{
        door_option,
        doors::{Handle, Location},
        interaction_options::{self as options, Options},
        player_walk::State,
        scenery_node, scenery_options,
    },
};
use serde_json::{Value, json};
struct Other;
impl option_handler::NativeOptionHandler for Other {}
fn names(v: &Value) -> Option<Shared<Vec<Option<String>>>> {
    v.as_array()
        .map(|a| Shared::new(a.iter().map(|v| v.as_str().map(str::to_owned)).collect()))
}
struct Harness {
    state: State,
    registry: scenery_options::Registry,
    definitions: defs::Registry,
    defs: [defs::Handle; 2],
    handlers: [option_handler::Handle; 2],
    object: Handle,
    captures: Vec<Options>,
}
impl Harness {
    fn new(row: &Value) -> Self {
        let mut definitions = defs::Registry::default();
        let d = [
            Shared::new(defs::SceneryDefinition::default()),
            Shared::new(defs::SceneryDefinition::default()),
        ];
        d[0].write().options = names(
            row.get("names")
                .unwrap_or(&json!(["Open", null, null, null, null])),
        );
        d[1].write().options = names(&json!(["Other", null, "Open"]));
        definitions.definitions.insert(12348, d[0].clone());
        definitions.definitions.insert(10327, d[1].clone());
        let registry = scenery_options::Registry::default();
        let handlers = [
            registry.register_doors().unwrap(),
            option_handler::Handler::new("test.other", Other),
        ];
        if let Some(v) = row.get("globalHandler") {
            option_handler::set_option_handler(
                &registry.defaults(),
                Some("open".into()),
                v.as_u64().map(|i| handlers[i as usize].clone()),
            );
        }
        if let Some(v) = row.get("local") {
            d[0].write()
                .handlers
                .write()
                .insert("option:open".into(), property(v, &handlers));
        }
        let object = scenery_node::construct(
            &mut definitions,
            12348,
            Location::new(3207, 3217, 0),
            0,
            1,
            false,
        )
        .unwrap();
        Self {
            state: State::default(),
            registry,
            definitions,
            defs: d,
            handlers,
            object,
            captures: vec![],
        }
    }
    fn captured(&mut self) -> scenery_options::Captured {
        self.registry
            .capture(&mut self.state, &self.object)
            .unwrap()
    }
    fn action(&mut self, a: &Value) -> options::Result<()> {
        let value = &a["value"];
        match a["op"].as_str().unwrap() {
            "default" => self.registry.initialize(
                &mut self.state,
                &mut self.definitions,
                &self.object,
                |s| Ok(rs09_game_core::java_lower::lowercase_root(s)),
            )?,
            "capture" => {
                let c = self.captured();
                self.captures.push(c.table.read().options());
            }
            "register" => {
                option_handler::set_option_handler(
                    &self.registry.defaults(),
                    a["name"].as_str().map(str::to_owned),
                    value.as_u64().map(|i| self.handlers[i as usize].clone()),
                );
            }
            "initialized" => self.captured().table.write().initialized = value.as_bool().unwrap(),
            "id" => self.object.write().id = value.as_i64().unwrap() as i32,
            "cached" => {
                self.object.write().definition =
                    value.as_u64().map(|i| self.defs[i as usize].clone())
            }
            "names" => self.defs[0].write().options = names(value),
            "config" => {
                self.defs[0]
                    .write()
                    .handlers
                    .write()
                    .insert("option:open".into(), property(value, &self.handlers));
            }
            _ => panic!("unknown scenery table action {a}"),
        }
        Ok(())
    }
    fn array(&self, array: &Options) -> Value {
        json!(
            array
                .read()
                .iter()
                .map(|o| o.as_ref().map(|o| {
                    let o = o.read();
                    let handler = o.handler.map(|id| self.registry.handler(id).unwrap());
                    let key =
                        handler.map(|h| self.handlers.iter().position(|v| v.same(&h)).unwrap());
                    json!([o.name(), o.index(), key])
                }))
                .collect::<Vec<_>>()
        )
    }
    fn snapshot(&mut self, error: Option<&str>) -> Value {
        let current = self.captured().table;
        let array = current.read().options();
        let object = self.object.read();
        let cached = object
            .definition
            .as_ref()
            .map(|d| self.defs.iter().position(|v| v.same(d)).unwrap());
        let program = door_option::Program;
        json!({"current":self.array(&array),"initialized":current.read().initialized,"captures":self.captures.iter().map(|a|self.array(a)).collect::<Vec<_>>(),"same":self.captures.iter().map(|a|a.same(&array)).collect::<Vec<_>>(),"id":object.id,"cached":cached,"error":error,"defaults":[program.is_walk(),program.is_walk_for(),program.is_delayed()]})
    }
}
fn property(v: &Value, handlers: &[option_handler::Handle; 2]) -> Property {
    match v {
        Value::Null => Property::Null,
        Value::Number(n) => Property::OptionHandler(handlers[n.as_u64().unwrap() as usize].clone()),
        Value::String(s) => Property::Text(s.clone()),
        _ => panic!("bad property {v}"),
    }
}
pub fn evaluate(row: &Value) -> Value {
    let mut h = Harness::new(row);
    json!(
        row["actions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| {
                let error = match h.action(a) {
                    Ok(()) => None,
                    Err(options::Error::Null) => Some("Null"),
                    Err(options::Error::Index) => Some("Index"),
                    Err(options::Error::Binding(s)) if s == "scenery option handler type" => {
                        Some("Type")
                    }
                    Err(e) => panic!("unexpected {e:?}"),
                };
                h.snapshot(error)
            })
            .collect::<Vec<_>>()
    )
}
