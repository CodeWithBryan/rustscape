use rs09_game_core::{direction::Direction, java_lower::lowercase_root};
use rs09_server::{
    definitions::{item::Shared, scenery::SceneryDefinition},
    world::{
        door_option,
        doors::{Handle, Location, Scenery},
        scenery_action::{self, Context, Error, Request, Result},
    },
};
use serde_json::{Value, json};
use std::collections::BTreeMap;

fn number(v: &Value, key: &str, default: i32) -> i32 {
    v[key].as_i64().map_or(default, |v| v as i32)
}
fn location(v: &Value) -> Option<Location> {
    let a = v.as_array()?;
    Some(Location::new(
        a[0].as_i64()? as i32,
        a[1].as_i64()? as i32,
        a[2].as_i64()? as i32,
    ))
}
fn coord(l: &Option<Location>) -> Value {
    l.as_ref().map_or(Value::Null, |l| json!(l.coordinates()))
}
fn text(v: &Value) -> Option<String> {
    v.as_str().map(String::from)
}
#[derive(Clone)]
pub struct SelectedOption {
    key: String,
    name: Option<String>,
    index: i32,
}
pub struct Interaction {
    key: String,
    generation: i32,
    options: Vec<Option<SelectedOption>>,
}
struct Links {
    child: Option<String>,
    interaction: Option<String>,
}
struct Model {
    input: Value,
    location: Option<Location>,
    objects: BTreeMap<String, Handle>,
    links: BTreeMap<String, Links>,
    interactions: BTreeMap<String, Shared<Interaction>>,
    lookup: Option<String>,
    varbit: Option<i32>,
    listeners: bool,
    plugin: bool,
    permitted: bool,
    properties: Option<usize>,
    teleports: [Option<Location>; 2],
    registered: BTreeMap<String, usize>,
    created: i32,
    events: Vec<Value>,
    calls: BTreeMap<String, i32>,
}
impl Model {
    fn options(&mut self, key: &str, values: &Value, initial: bool) {
        let mut p = self.interactions[key].write();
        if !initial {
            p.generation += 1;
        }
        p.options = values
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
            .map(|(i, v)| {
                if v.is_null() {
                    None
                } else {
                    Some(SelectedOption {
                        key: format!("{key}:{}:{i}", p.generation),
                        name: text(&v["name"]),
                        index: number(v, "index", i as i32),
                    })
                }
            })
            .collect();
    }
    fn add(&mut self, key: &str, id: i32, l: Location) -> Handle {
        let mut s = Scenery::new(id, l, 10, 0, Some("Door".into()), false).unwrap();
        s.definition = Some(Shared::new(SceneryDefinition::default()));
        let h = Shared::new(s);
        self.objects.insert(key.into(), h.clone());
        self.links.insert(
            key.into(),
            Links {
                child: Some(key.into()),
                interaction: Some(key.into()),
            },
        );
        self.interactions.insert(
            key.into(),
            Shared::new(Interaction {
                key: key.into(),
                generation: 0,
                options: vec![],
            }),
        );
        self.options(key, &json!([{"name":"Open"}, null, null, null, null]), true);
        h
    }
    fn key(&self, h: &Handle) -> String {
        self.objects
            .iter()
            .find(|(_, v)| v.same(h))
            .unwrap()
            .0
            .clone()
    }
    fn set(&mut self, v: &Value, initial: bool) {
        if v.get("location").is_some() {
            self.location = location(&v["location"]);
        }
        if v.get("lookup").is_some() {
            self.lookup = text(&v["lookup"]);
        }
        if v.get("varbit").is_some() {
            self.varbit = v["varbit"].as_i64().map(|v| v as i32);
        }
        if v.get("listeners").is_some() {
            self.listeners = v["listeners"] == true;
        }
        if v.get("plugin").is_some() {
            self.plugin = v["plugin"] == true;
        }
        if v.get("permitted").is_some() {
            self.permitted = v["permitted"] == true;
        }
        if v.get("properties").is_some() {
            self.properties = v["properties"].as_u64().map(|v| v as usize);
        }
        if let Some(values) = v["objects"].as_object() {
            for (key, v) in values {
                let mut s = self.objects[key].write();
                if v.get("id").is_some() {
                    s.id = number(v, "id", 0);
                }
                if v.get("active").is_some() {
                    s.active = v["active"] == true;
                }
                if v.get("name").is_some() {
                    s.name = text(&v["name"]);
                }
                if v.get("shape").is_some() {
                    s.shape = number(v, "shape", 10);
                }
                if v.get("location").is_some() {
                    s.location = location(&v["location"]).unwrap();
                }
                if v.get("direction").is_some() {
                    s.direction = Direction::from_code(number(v, "direction", 0)).unwrap();
                }
                if v.get("definition").is_some() {
                    s.definition = None;
                }
                if v.get("configFile").is_some() {
                    s.definition.as_ref().unwrap().write().config_file_id =
                        number(v, "configFile", -1);
                }
                if v.get("configId").is_some() {
                    s.definition.as_ref().unwrap().write().config_id = number(v, "configId", -1);
                }
                let links = self.links.get_mut(key).unwrap();
                if v.get("child").is_some() {
                    links.child = text(&v["child"]);
                }
                if v.get("interaction").is_some() {
                    links.interaction = text(&v["interaction"]);
                }
            }
        }
        if let Some(values) = v["options"].as_object() {
            for (key, v) in values {
                self.options(key, v, initial);
            }
        }
    }
    fn emit(&mut self, event: &str, args: Vec<Value>) -> Result<()> {
        let mut row = vec![json!(event)];
        row.extend(args);
        self.events.push(json!(row));
        let count = self.calls.entry(event.into()).or_default();
        *count += 1;
        let count = *count;
        for hook in self.input["hooks"].as_array().cloned().unwrap_or_default() {
            if hook["event"] == event && number(&hook, "at", 1) == count {
                for a in hook["actions"].as_array().unwrap() {
                    self.act(a)?;
                }
            }
        }
        Ok(())
    }
    fn act(&mut self, a: &Value) -> Result<Value> {
        match a["op"].as_str().unwrap() {
            "fail" => Err(Error::Binding(String::new())),
            "set" => {
                self.set(a, false);
                Ok(Value::Null)
            }
            "register" => door_option::new_instance(self, 1)
                .map(|v| json!(v))
                .map_err(door_error),
            "doorHandle" | "doorDestination" => {
                let player = if self.input.get("player").is_some_and(Value::is_null) {
                    None
                } else {
                    Some(0)
                };
                let key = a.get("object").cloned().unwrap_or(json!("base"));
                let object = key.as_str().and_then(|key| self.objects.get(key)).cloned();
                if a["op"] == "doorHandle" {
                    door_option::handle(self, player, object.as_ref(), a["option"].as_str())
                        .map(|v| json!(v))
                        .map_err(door_error)
                } else {
                    door_option::destination(self, player, object.as_ref())
                        .map(|v| coord(&v))
                        .map_err(door_error)
                }
            }
            "process" => scenery_action::process(
                self,
                Request {
                    player: if self.input.get("player").is_some_and(Value::is_null) {
                        None
                    } else {
                        Some(0)
                    },
                    id: number(a, "id", number(&self.input, "id", 12348)),
                    x: number(a, "x", 3207),
                    y: number(a, "y", 3217),
                    option: number(a, "index", number(&self.input, "index", 0)),
                },
            )
            .map(|()| Value::Null),
            _ => unreachable!(),
        }
    }
    fn snapshot(&self, result: Value, error: Option<&str>) -> Value {
        let objects: BTreeMap<_, _> = self
            .objects
            .iter()
            .map(|(k, h)| {
                let s = h.read();
                let links = &self.links[k];
                let definition = s.definition.as_ref().map(|d| {
                    let d = d.read();
                    json!([d.config_file_id, d.config_id])
                });
                (
                    k,
                    json!([
                        s.id,
                        s.active,
                        s.location.coordinates(),
                        s.direction.code(),
                        links.child,
                        links.interaction,
                        definition,
                        s.shape,
                        s.name
                    ]),
                )
            })
            .collect();
        let interactions: BTreeMap<_, _> = self
            .interactions
            .iter()
            .map(|(k, p)| {
                let p = p.read();
                let options: Vec<_> = p
                    .options
                    .iter()
                    .map(|o| o.as_ref().map(|o| json!([o.key, o.name, o.index])))
                    .collect();
                (k, json!([p.generation, options]))
            })
            .collect();
        json!({"objects":objects,"interactions":interactions,"events":self.events,"location":coord(&self.location),
               "lookup":self.lookup,"varbit":self.varbit,"listeners":self.listeners,"plugin":self.plugin,"error":error,
               "result":result,"permitted":self.permitted,"properties":self.properties,"registered":self.registered,
               "teleports":self.teleports.iter().map(coord).collect::<Vec<_>>()})
    }
}
impl Context for Model {
    type Interaction = Shared<Interaction>;
    type Option = SelectedOption;
    fn player_location(&mut self, _: usize) -> Result<Option<Location>> {
        Ok(self.location.clone())
    }
    fn object(&mut self, x: i32, y: i32, z: i32, id: i32, shape: i32) -> Result<Option<Handle>> {
        self.emit(
            "lookup",
            vec![json!(x), json!(y), json!(z), json!(id), json!(shape)],
        )?;
        Ok(self
            .lookup
            .as_ref()
            .and_then(|key| self.objects.get(key))
            .cloned())
    }
    fn construct(&mut self, id: i32, location: Location) -> Result<Handle> {
        let key = format!("created{}", self.created);
        self.created += 1;
        let h = self.add(&key, id, location.clone());
        self.emit(
            "construct",
            vec![json!(key), json!(id), json!(location.coordinates())],
        )?;
        Ok(h)
    }
    fn child(&mut self, h: &Handle, _: usize) -> Result<Option<Handle>> {
        let key = self.key(h);
        self.emit("child", vec![json!(key)])?;
        Ok(self.links[&key]
            .child
            .as_ref()
            .and_then(|key| self.objects.get(key))
            .cloned())
    }
    fn interaction(&mut self, h: &Handle) -> Result<Option<Self::Interaction>> {
        let key = self.key(h);
        self.emit("interaction", vec![json!(key)])?;
        Ok(self.links[&key]
            .interaction
            .as_ref()
            .and_then(|key| self.interactions.get(key))
            .cloned())
    }
    fn option(&mut self, p: &Self::Interaction, index: i32) -> Result<Option<Self::Option>> {
        let key = p.read().key.clone();
        self.emit("option", vec![json!(key), json!(index)])?;
        p.read()
            .options
            .get(index as usize)
            .cloned()
            .ok_or(Error::Index)
    }
    fn option_name(o: &Self::Option) -> Option<String> {
        o.name.clone()
    }
    fn option_index(o: &Self::Option) -> i32 {
        o.index
    }
    fn varbit_definition(&mut self, id: i32) -> Result<Option<i32>> {
        self.emit("varbit", vec![json!(id)])?;
        Ok(self.varbit)
    }
    fn debug(&mut self, _: usize, text: &str) -> Result<()> {
        self.emit("debug", vec![json!(text)])
    }
    fn invalid(&mut self, _: usize, h: Option<&Handle>) -> Result<()> {
        self.emit(
            "invalid",
            vec![h.map_or(Value::Null, |h| json!(self.key(h))), json!("NULL")],
        )
    }
    fn clear_minimap(&mut self, _: usize) -> Result<()> {
        self.emit("minimap", vec![])
    }
    fn reset_scripts(&mut self, _: usize) -> Result<()> {
        self.emit("scripts", vec![])
    }
    fn listeners(&mut self, id: i32, name: Option<String>, _: usize, h: &Handle) -> Result<bool> {
        self.emit(
            "listeners",
            vec![json!(id), json!(name), json!(self.key(h))],
        )?;
        Ok(self.listeners)
    }
    fn plugin(&mut self, _: usize, h: &Handle) -> Result<bool> {
        self.emit("plugin", vec![json!(self.key(h))])?;
        Ok(self.plugin)
    }
    fn handle(&mut self, p: &Self::Interaction, _: usize, o: &Self::Option) -> Result<()> {
        let key = p.read().key.clone();
        self.emit(
            "handle",
            vec![json!(key), json!(o.key), json!(o.name), json!(o.index)],
        )
    }
}
type DoorResult<T> = rs09_server::world::doors::Result<T>;
fn door_error(e: rs09_server::world::doors::Error) -> Error {
    match e {
        rs09_server::world::doors::Error::Null => Error::Null,
        rs09_server::world::doors::Error::Index => Error::Index,
        rs09_server::world::doors::Error::Binding(s) => Error::Binding(s),
        other => Error::Binding(format!("{other:?}")),
    }
}
fn binding_error(e: Error) -> rs09_server::world::doors::Error {
    match e {
        Error::Null => rs09_server::world::doors::Error::Null,
        Error::Index => rs09_server::world::doors::Error::Index,
        Error::Binding(s) => rs09_server::world::doors::Error::Binding(s),
    }
}
impl door_option::Context for Model {
    type Properties = usize;
    fn register(&mut self, option: &str, handler: usize) -> DoorResult<()> {
        self.emit("register", vec![json!(option), json!(handler)])
            .map_err(binding_error)?;
        self.registered.insert(option.into(), handler);
        Ok(())
    }
    fn player_location(&mut self, player: Option<usize>) -> DoorResult<Option<Location>> {
        player.ok_or(rs09_server::world::doors::Error::Null)?;
        self.emit("playerLocation", vec![]).map_err(binding_error)?;
        Ok(self.location.clone())
    }
    fn lowercase(&self, text: &str) -> String {
        lowercase_root(text)
    }
    fn audio(&mut self, _: Option<usize>, id: i32) -> DoorResult<()> {
        self.emit("audio", vec![json!(id)]).map_err(binding_error)
    }
    fn animate(&mut self, _: Option<usize>, id: i32, forced: bool) -> DoorResult<()> {
        self.emit("animate", vec![json!(id), json!(forced)])
            .map_err(binding_error)
    }
    fn transform(&mut self, object: &Handle, id: i32) -> DoorResult<Handle> {
        self.emit("transform", vec![json!(self.key(object)), json!(id)])
            .map_err(binding_error)?;
        Ok(self.objects["other"].clone())
    }
    fn replace(
        &mut self,
        object: &Handle,
        replacement: Handle,
        restore: Option<i32>,
    ) -> DoorResult<()> {
        self.emit(
            "replace",
            vec![
                json!(self.key(object)),
                json!(self.key(&replacement)),
                json!(restore),
            ],
        )
        .map_err(binding_error)
    }
    fn teleport_permitted(&mut self, destination: &Location) -> DoorResult<bool> {
        self.emit("permitted", vec![json!(destination.coordinates())])
            .map_err(binding_error)?;
        Ok(self.permitted)
    }
    fn message(&mut self, _: Option<usize>, text: &str) -> DoorResult<()> {
        self.emit("message", vec![json!(text)])
            .map_err(binding_error)
    }
    fn properties(&mut self, player: Option<usize>) -> DoorResult<Option<usize>> {
        player.ok_or(rs09_server::world::doors::Error::Null)?;
        self.emit("properties", vec![]).map_err(binding_error)?;
        Ok(self.properties)
    }
    fn set_teleport(&mut self, properties: &usize, destination: Location) -> DoorResult<()> {
        self.teleports[*properties] = Some(destination.clone());
        self.emit(
            "teleport",
            vec![json!(properties), json!(destination.coordinates())],
        )
        .map_err(binding_error)
    }
    fn door(&mut self, _: Option<usize>, object: &Handle) -> DoorResult<()> {
        self.emit("door", vec![json!(self.key(object))])
            .map_err(binding_error)
    }
    fn door_destination(&mut self, _: Option<usize>, object: &Handle) -> DoorResult<Location> {
        self.emit("doorDestination", vec![json!(self.key(object))])
            .map_err(binding_error)?;
        Ok(Location::new(3208, 3217, 0))
    }
}
pub fn evaluate(input: &Value) -> Value {
    let mut m = Model {
        input: input.clone(),
        location: Some(Location::new(3207, 3216, 0)),
        objects: BTreeMap::new(),
        links: BTreeMap::new(),
        interactions: BTreeMap::new(),
        lookup: Some("base".into()),
        varbit: Some(7),
        listeners: false,
        plugin: false,
        permitted: true,
        properties: Some(0),
        teleports: [None, None],
        registered: BTreeMap::new(),
        created: 0,
        events: vec![],
        calls: BTreeMap::new(),
    };
    m.add("base", 12348, Location::new(3207, 3217, 0));
    m.add("other", 10327, Location::new(3207, 3217, 0));
    m.set(input, true);
    let mut out = vec![];
    for a in input["actions"].as_array().unwrap() {
        m.events.clear();
        let (result, error) = match m.act(a) {
            Ok(v) => (v, None),
            Err(Error::Null) => (Value::Null, Some("Null")),
            Err(Error::Index) => (Value::Null, Some("Index")),
            Err(Error::Binding(_)) => (Value::Null, Some("Binding")),
        };
        out.push(m.snapshot(result, error));
    }
    json!(out)
}
