//! Native replay of the books corpus: `world::books` with recording
//! stand-ins for the systems the oracle's adapters record (the listener
//! registries, the player's attributes, interfaces and packet dispatch,
//! storeBookInHouse, the world's names).
use rs09_server::{
    player::attributes::AttributeValue,
    world::{
        books::{self, Callback, Error, Host, Result, Tables, registry},
        interaction_listeners::Kind as IntType,
    },
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap};

fn num(v: &Value) -> i32 {
    v.as_i64().unwrap() as i32
}

/// A node of the replay: the player or a book item.
#[derive(Clone)]
pub struct Node(String);

pub struct Model {
    events: Vec<Value>,
    op_calls: i32,
    fail_at: i32,
    answers: Value,
    answer_calls: HashMap<String, usize>,
    attributes: BTreeMap<String, AttributeValue>,
    settings: String,
    server: String,
    tables: Tables,
    items: HashMap<i32, registry::Interaction>,
    interfaces: HashMap<i32, registry::InterfaceFn>,
}
impl Model {
    fn record(&mut self, name: &str, args: Vec<Value>) -> Result<()> {
        let mut e = vec![json!(name)];
        e.extend(args);
        self.events.push(Value::Array(e));
        self.op_calls += 1;
        if self.op_calls == self.fail_at {
            return Err(Error::Binding("injected".into()));
        }
        Ok(())
    }
    fn answer(&mut self, name: &str, fallback: bool) -> bool {
        let Some(v) = self.answers.get(name) else {
            return fallback;
        };
        if let Some(list) = v.as_array() {
            let calls = self.answer_calls.entry(name.into()).or_insert(0);
            let i = *calls;
            *calls += 1;
            return list
                .get(i)
                .or(list.last())
                .and_then(Value::as_bool)
                .unwrap_or(fallback);
        }
        v.as_bool().unwrap_or(fallback)
    }
    fn value(v: &AttributeValue) -> Value {
        match v {
            AttributeValue::Int(i) => json!(i),
            AttributeValue::Text(t) => json!(t),
            AttributeValue::Object(o) => {
                let c = o
                    .as_any()
                    .downcast_ref::<Callback>()
                    .expect("a book callback");
                json!({ "function": c.class })
            }
            other => panic!("unexpected attribute {:?}", other.java_string()),
        }
    }
}
impl Host for Model {
    type Node = Node;
    fn close_interface(&mut self, p: &Node) -> Result<bool> {
        self.record("closeInterface", vec![json!(p.0)])?;
        Ok(self.answer("closeInterface", true))
    }
    fn open_interface(&mut self, p: &Node, id: i32) -> Result<()> {
        self.record("openInterface", vec![json!(p.0), json!(id)])
    }
    fn get_attribute(&mut self, p: &Node, key: &str) -> Result<Option<AttributeValue>> {
        self.record("getAttribute", vec![json!(p.0), json!(key)])?;
        Ok(self.attributes.get(key).cloned())
    }
    fn set_attribute(&mut self, p: &Node, key: &str, value: AttributeValue) -> Result<()> {
        self.record(
            "setAttribute",
            vec![json!(p.0), json!(key), Self::value(&value)],
        )?;
        self.attributes.insert(key.into(), value);
        Ok(())
    }
    fn send_string(&mut self, p: &Node, text: &str, iface: i32, child: i32) -> Result<()> {
        self.record(
            "sendString",
            vec![json!(p.0), json!(text), json!(iface), json!(child)],
        )
    }
    fn send_interface_config(
        &mut self,
        p: &Node,
        iface: i32,
        child: i32,
        hidden: bool,
    ) -> Result<()> {
        self.record(
            "interfaceConfig",
            vec![json!(p.0), json!(iface), json!(child), json!(hidden)],
        )
    }
    fn send_model_on_interface(
        &mut self,
        p: &Node,
        model: i32,
        iface: i32,
        child: i32,
        zoom: i32,
    ) -> Result<()> {
        self.record(
            "modelOnInterface",
            vec![
                json!(p.0),
                json!(model),
                json!(iface),
                json!(child),
                json!(zoom),
            ],
        )
    }
    fn send_angle_on_interface(
        &mut self,
        p: &Node,
        iface: i32,
        child: i32,
        zoom: i32,
        pitch: i32,
        yaw: i32,
    ) -> Result<()> {
        self.record(
            "angleOnInterface",
            vec![
                json!(p.0),
                json!(iface),
                json!(child),
                json!(zoom),
                json!(pitch),
                json!(yaw),
            ],
        )
    }
    fn send_item_on_interface(
        &mut self,
        p: &Node,
        item: i32,
        amount: i32,
        iface: i32,
        child: i32,
    ) -> Result<()> {
        self.record(
            "itemOnInterface",
            vec![
                json!(p.0),
                json!(item),
                json!(amount),
                json!(iface),
                json!(child),
            ],
        )
    }
    fn store_book_in_house(&mut self, p: &Node, node: &Node) -> Result<bool> {
        self.record("storeBookInHouse", vec![json!(p.0), json!(node.0)])?;
        Ok(self.answer("storeBookInHouse", true))
    }
    fn settings_name(&mut self) -> Result<Option<String>> {
        Ok(Some(self.settings.clone()))
    }
    fn server_name(&mut self) -> Result<String> {
        Ok(self.server.clone())
    }
    fn book_tables(&mut self) -> &mut Tables {
        &mut self.tables
    }
}
impl registry::Registrar for Model {
    type FunctionRef = registry::Interaction;
    type InterfaceRef = registry::InterfaceFn;
    fn function(&mut self, f: registry::Interaction) -> registry::Interaction {
        f
    }
    fn interface_function(&mut self, f: registry::InterfaceFn) -> registry::InterfaceFn {
        f
    }
    fn on(
        &mut self,
        ids: &[i32],
        kind: IntType,
        options: &[&str],
        f: &registry::Interaction,
    ) -> Result<()> {
        assert_eq!(kind, IntType::Item);
        for &id in ids {
            self.record(
                "on",
                vec![json!(id), json!("ITEM"), json!(options), json!(f.class())],
            )?;
            self.items.insert(id, *f);
        }
        Ok(())
    }
    fn on_interface(&mut self, id: i32, f: &registry::InterfaceFn) -> Result<()> {
        self.record("onInterface", vec![json!(id), json!(f.class())])?;
        self.interfaces.insert(id, *f);
        Ok(())
    }
}

fn error_name(e: &Error) -> String {
    match e {
        Error::Null => "Null".into(),
        Error::ClassCast => "ClassCast".into(),
        Error::Index => "Index".into(),
        Error::Binding(m) if m == "injected" => "Failure".into(),
        other => panic!("unexpected native error {other:?}"),
    }
}

pub fn evaluate(input: &Value) -> Value {
    let world = input.get("world").cloned().unwrap_or(json!({}));
    let mut m = Model {
        events: vec![],
        op_calls: 0,
        fail_at: -1,
        answers: input.get("answers").cloned().unwrap_or(Value::Null),
        answer_calls: HashMap::new(),
        attributes: BTreeMap::new(),
        settings: world
            .get("settingsName")
            .and_then(Value::as_str)
            .unwrap_or("2009scape")
            .into(),
        server: world
            .get("serverName")
            .and_then(Value::as_str)
            .unwrap_or("2009scape")
            .into(),
        tables: Tables::new(),
        items: HashMap::new(),
        interfaces: HashMap::new(),
    };
    let player = Node("player".into());
    let mut steps = vec![];
    for a in input["ops"].as_array().unwrap() {
        m.events.clear();
        m.op_calls = 0;
        m.fail_at = a.get("failAt").map_or(-1, num);
        let result: Result<Value> = (|| {
            Ok(match a["op"].as_str().unwrap() {
                "define" => {
                    let class = registry::Class::named(a["class"].as_str().unwrap()).unwrap();
                    registry::define(&mut m, class)?;
                    Value::Null
                }
                "read" => {
                    let f = m.items[&num(&a["item"])];
                    let node = Node(format!("item{}", a["item"]));
                    json!(registry::run_interaction(&mut m, f, &player, &node)?)
                }
                "button" => {
                    let f = m.interfaces[&num(&a["component"])];
                    json!(registry::run_interface(
                        &mut m,
                        f,
                        &player,
                        num(&a["button"])
                    )?)
                }
                "rules" => {
                    books::open_general_rules(&mut m, &player)?;
                    Value::Null
                }
                "model" => {
                    let n = |k: &str| num(&a[k]);
                    books::set_model_on_page(
                        &mut m,
                        &player,
                        n("pageSet"),
                        n("model"),
                        n("component"),
                        n("enable"),
                        n("draw"),
                        n("zoom"),
                        n("pitch"),
                        n("yaw"),
                    )?;
                    Value::Null
                }
                "itemOnPage" => {
                    let n = |k: &str| num(&a[k]);
                    books::set_item_on_page(
                        &mut m,
                        &player,
                        n("pageSet"),
                        n("itemId"),
                        n("component"),
                        n("enable"),
                        n("draw"),
                        n("zoom"),
                        n("pitch"),
                        n("yaw"),
                    )?;
                    Value::Null
                }
                "clearModels" => {
                    books::clear_models_on_page(&mut m, &player, num(&a["component"]))?;
                    Value::Null
                }
                "isLastPage" => json!(books::is_last_page(num(&a["page"]), num(&a["total"]))),
                "attr" => {
                    let key = a["key"].as_str().unwrap().to_owned();
                    match &a["value"] {
                        Value::Null => {
                            m.attributes.remove(&key);
                        }
                        Value::Number(n) => {
                            m.attributes
                                .insert(key, AttributeValue::Int(n.as_i64().unwrap() as i32));
                        }
                        Value::String(s) => {
                            m.attributes.insert(key, AttributeValue::Text(s.clone()));
                        }
                        other => panic!("attribute {other}"),
                    }
                    Value::Null
                }
                other => panic!("unknown operation {other}"),
            })
        })();
        m.fail_at = -1;
        let result = match result {
            Ok(v) => json!({ "value": v }),
            Err(e) => json!({ "error": error_name(&e) }),
        };
        let attributes: serde_json::Map<String, Value> = m
            .attributes
            .iter()
            .map(|(k, v)| (k.clone(), Model::value(v)))
            .collect();
        steps.push(json!({ "result": result, "events": m.events, "attributes": attributes }));
    }
    Value::Array(steps)
}
