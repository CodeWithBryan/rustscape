use rs09_server::{
    definitions::{
        item::{self as defs, Shared},
        option_handler,
        property::Property,
    },
    inventory::item,
    world::{
        doors::Location,
        ground_item as ground, ground_options, ground_spawn,
        interaction_options::{self as options, Options},
        player_walk::State,
    },
};
use serde_json::{Value, json};

struct Other;
impl option_handler::NativeOptionHandler for Other {}
pub struct Construction<'a>(pub &'a defs::Registry);
impl ground::Context for Construction<'_> {
    fn copy_item(&mut self, id: i32, amount: i32, charge: i32) -> ground::Result<item::Handle> {
        Ok(Shared::new(item::Item::new(id, amount, charge, self.0)))
    }
    fn default_interaction(&mut self, _: &ground::Handle) -> ground::Result<()> {
        Ok(())
    }
    fn uid(&mut self, _: &rs09_server::world::player_walk::NodeHandle) -> ground::Result<i32> {
        Err(ground::Error::Binding("no players in table corpus".into()))
    }
    fn username(
        &mut self,
        _: &rs09_server::world::player_walk::NodeHandle,
    ) -> ground::Result<Option<String>> {
        Err(ground::Error::Binding("no players in table corpus".into()))
    }
    fn ticks(&mut self) -> ground::Result<i32> {
        Ok(0)
    }
}
fn names(value: &Value) -> defs::Actions {
    value
        .as_array()
        .map(|a| Shared::new(a.iter().map(|v| v.as_str().map(str::to_owned)).collect()))
}
pub struct Harness {
    pub state: State,
    pub registry: ground_options::Registry,
    pub spawns: ground_spawn::Registry,
    pub definitions: defs::Registry,
    pub defs: [defs::Handle; 2],
    pub handlers: [option_handler::Handle; 2],
    pub ground: ground::Handle,
    captures: Vec<Options>,
}
impl Harness {
    pub fn new(row: &Value) -> Self {
        let mut definitions = defs::Registry::default();
        let d = [
            Shared::new(defs::ItemDefinition::default()),
            Shared::new(defs::ItemDefinition::default()),
        ];
        d[0].write().ground_actions = names(
            row.get("names")
                .unwrap_or(&json!([null, null, "Take", null, null])),
        );
        d[0].write().options = names(&json!(["Use", null, null, null, "Drop"]));
        d[1].write().ground_actions = names(&json!(["Other", null, "Take"]));
        d[1].write().options = names(&json!(["Other"]));
        definitions.insert(995, d[0].clone());
        let registry = ground_options::Registry::default();
        let handlers = [
            registry.register_pickup().unwrap(),
            option_handler::Handler::new("test.other", Other),
        ];
        if let Some(v) = row.get("globalHandler") {
            option_handler::set_option_handler(
                &registry.defaults(),
                Some("take".into()),
                v.as_u64().map(|i| handlers[i as usize].clone()),
            );
        }
        if let Some(v) = row.get("local") {
            d[0].write()
                .handlers
                .insert("option:take".into(), property(v, &handlers));
        }
        let input = Shared::new(item::Item::new(995, 1, 1000, &definitions));
        let location = (row["located"] != false).then(|| Location::new(3207, 3216, 0));
        let ground =
            ground::construct(&mut Construction(&definitions), &input, location, 200, None)
                .unwrap();
        Self {
            state: State::default(),
            registry,
            spawns: ground_spawn::Registry::default(),
            definitions,
            defs: d,
            handlers,
            ground,
            captures: vec![],
        }
    }
    pub fn initialize(&mut self) -> options::Result<()> {
        self.registry.initialize(
            &mut self.state,
            &self.spawns,
            &self.definitions,
            &self.ground,
            |s| Ok(rs09_game_core::java_lower::lowercase_root(s)),
        )
    }
    pub fn captured(&mut self) -> ground_options::Captured {
        self.registry
            .capture(&mut self.state, &self.spawns, &self.ground)
            .unwrap()
    }
    fn action(&mut self, a: &Value) -> options::Result<()> {
        let value = &a["value"];
        match a["op"].as_str().unwrap() {
            "default" => self.initialize()?,
            "capture" => {
                let capture = self.captured();
                self.captures.push(capture.table.read().options());
            }
            "register" => {
                option_handler::set_option_handler(
                    &self.registry.defaults(),
                    a["name"].as_str().map(str::to_owned),
                    value.as_u64().map(|i| self.handlers[i as usize].clone()),
                );
            }
            "initialized" => self.captured().table.write().initialized = value.as_bool().unwrap(),
            "charge" => ground::as_item(&self.ground)
                .write()
                .set_charge(value.as_i64().unwrap() as i32),
            "cached" => {
                ground::as_item(&self.ground).write().definition =
                    value.as_u64().map(|i| self.defs[i as usize].clone())
            }
            "located" => {
                self.ground.write().location = value
                    .as_bool()
                    .unwrap()
                    .then(|| Location::new(3207, 3216, 0))
            }
            "names" => self.defs[0].write().ground_actions = names(value),
            "config" => {
                self.defs[0]
                    .write()
                    .handlers
                    .insert("option:take".into(), property(value, &self.handlers));
            }
            _ => panic!("unrecognized table action {a}"),
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
        let item = ground::as_item(&self.ground);
        let item = item.read();
        let cached = item
            .definition
            .as_ref()
            .map(|d| self.defs.iter().position(|v| v.same(d)).unwrap());
        json!({"current":self.array(&array),"initialized":current.read().initialized,
            "captures":self.captures.iter().map(|a| self.array(a)).collect::<Vec<_>>(),
            "same":self.captures.iter().map(|a| a.same(&array)).collect::<Vec<_>>(),
            "id":item.id(),"cached":cached,"error":error})
    }
}
fn property(value: &Value, handlers: &[option_handler::Handle; 2]) -> Property {
    match value {
        Value::Null => Property::Null,
        Value::Number(n) => Property::OptionHandler(handlers[n.as_u64().unwrap() as usize].clone()),
        Value::String(s) => Property::Text(s.clone()),
        _ => panic!("bad configuration {value}"),
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
                    Err(options::Error::Binding(s)) if s == "Type" => Some("Type"),
                    Err(e) => panic!("unexpected native failure {e:?}"),
                };
                h.snapshot(error)
            })
            .collect::<Vec<_>>()
    )
}
