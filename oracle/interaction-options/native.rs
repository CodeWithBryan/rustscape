use rs09_game_core::java_lower::lowercase_root;
use rs09_server::{
    definitions::item::Shared,
    world::{
        interaction::{OptionData, OptionHandle},
        interaction_options::{
            self as options, Constants, Context, Error, Handle, Kind, Names, NamesKind, Options,
            Result, Table,
        },
        player_walk::Node,
    },
};
use serde_json::{Value, json};
use std::collections::BTreeMap;

fn n(v: &Value, key: &str, default: i32) -> i32 {
    v[key].as_i64().map_or(default, |v| v as i32)
}
struct NodeData {
    id: i32,
    located: bool,
    definition: Option<usize>,
}
struct Model {
    input: Value,
    kind: Kind,
    null_nodes: bool,
    nodes: [NodeData; 2],
    tables: [Handle; 2],
    constants: Constants,
    options: Vec<OptionHandle>,
    arrays: Vec<Options>,
    names: Vec<Names>,
    definition_names: [Option<Names>; 2],
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
        let hooks = self.input["hooks"].as_array().cloned().unwrap_or_default();
        for hook in hooks {
            if hook["event"] == event && n(&hook, "at", 1) == count {
                for action in hook["actions"].as_array().unwrap() {
                    self.act(action)?;
                }
            }
        }
        Ok(())
    }
    fn option_key(&mut self, option: Option<&OptionHandle>) -> Value {
        let Some(option) = option else {
            return Value::Null;
        };
        if let Some(i) = self.options.iter().position(|v| v.same(option)) {
            return json!(i);
        }
        self.options.push(option.clone());
        json!(self.options.len() - 1)
    }
    fn selected(&self, action: &Value) -> Option<OptionHandle> {
        if action.get("option").is_some_and(Value::is_null) {
            None
        } else {
            Some(self.options[n(action, "option", 0) as usize].clone())
        }
    }
    fn new_names(&mut self, value: &Value) -> Option<Names> {
        let values = value.as_array()?;
        let names = Shared::new(
            values
                .iter()
                .map(|v| v.as_str().map(str::to_owned))
                .collect(),
        );
        self.names.push(names.clone());
        Some(names)
    }
    fn act(&mut self, a: &Value) -> Result<Value> {
        let table_index = n(a, "table", 0) as usize;
        let table = self.tables[table_index].clone();
        let selected = self.selected(a);
        match a["op"].as_str().unwrap() {
            "fail" => return Err(Error::Binding("failure".into())),
            "new" => {
                self.options.push(Shared::new(OptionData::new(
                    a["name"].as_str().map(str::to_owned),
                    n(a, "index", 0),
                )));
            }
            "set" => options::set(self, &table, selected.as_ref())?,
            "get" => {
                let option = options::get(&table, n(a, "index", 0))?;
                return Ok(self.option_key(option.as_ref()));
            }
            "remove" => return options::remove(self, &table, selected.as_ref()).map(|v| json!(v)),
            "removeIndex" => options::remove_index(self, &table, n(a, "index", 0))?,
            "default" => options::set_default(self, &table)?,
            "init" => {
                let names = self.new_names(&a["names"]);
                options::init(self, &table, n(a, "id", 995), names)?;
            }
            "handler" => {
                return options::default_handler(
                    self,
                    if self.null_nodes {
                        None
                    } else {
                        Some(table_index)
                    },
                    n(a, "id", 995),
                    a["name"].as_str(),
                )
                .map(|v| json!(v));
            }
            "handlerSet" => {
                selected.ok_or(Error::Null)?.write().handler =
                    a["handler"].as_u64().map(|v| v as usize)
            }
            "setInitialized" => table.write().initialized = a["value"] == true,
            "capture" => {
                self.arrays.push(table.read().options());
                return Ok(json!(self.arrays.len() - 1));
            }
            "arraySet" => {
                let array = self
                    .arrays
                    .get(n(a, "array", 0) as usize)
                    .ok_or(Error::Index)?
                    .clone();
                let mut values = array.write();
                *values
                    .get_mut(n(a, "index", 0) as usize)
                    .ok_or(Error::Index)? = selected;
            }
            "namesSet" => {
                let array = self
                    .names
                    .get(n(a, "array", 0) as usize)
                    .ok_or(Error::Index)?
                    .clone();
                let mut values = array.write();
                *values
                    .get_mut(n(a, "index", 0) as usize)
                    .ok_or(Error::Index)? = a["value"].as_str().map(str::to_owned);
            }
            "setNode" => {
                let node = &mut self.nodes[table_index];
                if a.get("id").is_some() {
                    node.id = n(a, "id", 0);
                }
                if a.get("located").is_some() {
                    node.located = a["located"] == true;
                }
                if a.get("definition").is_some() {
                    node.definition = a["definition"].as_u64().map(|v| v as usize);
                }
            }
            op => panic!("unknown action {op}"),
        }
        Ok(Value::Null)
    }
    fn array(&mut self, array: &Options) -> Value {
        let values = array.read().clone();
        json!(
            values
                .iter()
                .map(|v| self.option_key(v.as_ref()))
                .collect::<Vec<_>>()
        )
    }
    fn snapshot(&mut self, result: Value, error: Option<String>) -> Value {
        let tables = self.tables.clone();
        let mut table_states = Vec::new();
        for table in &tables {
            let (initialized, array) = {
                let t = table.read();
                (t.initialized, t.options())
            };
            table_states.push(json!([initialized, self.array(&array)]));
        }
        let arrays = self.arrays.clone();
        let arrays = arrays.iter().map(|a| self.array(a)).collect::<Vec<_>>();
        let names = self
            .names
            .iter()
            .map(|a| a.read().clone())
            .collect::<Vec<_>>();
        let options = self
            .options
            .iter()
            .map(|o| {
                let o = o.read();
                json!([o.name(), o.index(), o.handler])
            })
            .collect::<Vec<_>>();
        let nodes = self
            .nodes
            .iter()
            .map(|n| {
                if self.null_nodes {
                    Value::Null
                } else {
                    json!([n.id, n.located, n.definition])
                }
            })
            .collect::<Vec<_>>();
        json!({"tables":table_states,"arrays":arrays,"names":names,"options":options,"nodes":nodes,"events":self.events,"result":result,"error":error})
    }
}
impl Context for Model {
    type Definition = usize;
    fn kind(&self, _: Option<Node>) -> Kind {
        self.kind
    }
    fn node_id(&mut self, node: Node) -> Result<i32> {
        self.emit("id", vec![json!(node)])?;
        Ok(self.nodes[node].id)
    }
    fn item_has_location(&mut self, node: Node) -> Result<bool> {
        self.emit("location", vec![json!(node)])?;
        Ok(self.nodes[node].located)
    }
    fn definition(&mut self, node: Node) -> Result<Option<usize>> {
        self.emit("definition", vec![json!(node)])?;
        Ok(self.nodes[node].definition)
    }
    fn names(&mut self, definition: &usize, kind: NamesKind) -> Result<Option<Names>> {
        self.emit("names", vec![json!(definition), json!(format!("{kind:?}"))])?;
        Ok(self.definition_names[*definition].clone())
    }
    fn lowercase(&mut self, name: &str) -> Result<String> {
        Ok(lowercase_root(name))
    }
    fn option_handler(&mut self, kind: Kind, id: i32, name: &str) -> Result<Option<usize>> {
        self.emit(
            "lookup",
            vec![json!(format!("{kind:?}")), json!(id), json!(name)],
        )?;
        Ok(Some(1))
    }
    fn send_option(&mut self, player: Node, index: i32, name: Option<String>) -> Result<()> {
        self.emit("send", vec![json!(player), json!(index), json!(name)])
    }
    fn node_string(&mut self, node: Option<Node>) -> Result<String> {
        Ok(node.map_or("null".into(), |n| format!("node:{n}")))
    }
    fn constants(&self) -> &Constants {
        &self.constants
    }
}
pub fn evaluate(input: &Value) -> Value {
    let name = input["kind"].as_str().unwrap_or("Item");
    let kind = match name {
        "Player" => Kind::Player,
        "Npc" => Kind::Npc,
        "Scenery" => Kind::Scenery,
        "Item" => Kind::Item,
        _ => Kind::Other,
    };
    let null_nodes = name == "Null";
    let constants = Constants::default();
    let all = vec![
        constants.attack.clone(),
        constants.follow.clone(),
        constants.trade.clone(),
        constants.give_to.clone(),
        constants.pickpocket.clone(),
        constants.examine.clone(),
        constants.assist.clone(),
        constants.null.clone(),
    ];
    let mut model = Model {
        input: input.clone(),
        kind,
        null_nodes,
        nodes: std::array::from_fn(|i| NodeData {
            id: 995,
            located: input["located"] != false,
            definition: if i == 0 && input.get("definition").is_some_and(Value::is_null) {
                None
            } else {
                Some(i)
            },
        }),
        tables: std::array::from_fn(|i| Table::new(if null_nodes { None } else { Some(i) })),
        constants,
        options: all,
        arrays: Vec::new(),
        names: Vec::new(),
        definition_names: [None, None],
        events: Vec::new(),
        calls: BTreeMap::new(),
    };
    let default = json!(["Take", "Use", null, "null", "Examine"]);
    let names = model.new_names(input.get("names").unwrap_or(&default));
    model.definition_names = [names.clone(), names];
    let mut states = Vec::new();
    for action in input["actions"].as_array().unwrap() {
        model.events.clear();
        let (result, error) = match model.act(action) {
            Ok(value) => (value, None),
            Err(e) => (
                Value::Null,
                Some(match e {
                    Error::Null => "Null".into(),
                    Error::Index => "Index".into(),
                    Error::Unsupported(s) => format!("Unsupported:{s}"),
                    Error::Binding(_) => "Binding".into(),
                }),
            ),
        };
        states.push(model.snapshot(result, error));
    }
    json!(states)
}
