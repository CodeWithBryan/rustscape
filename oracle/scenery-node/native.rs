#[allow(dead_code)]
#[path = "../scenery-definitions/fields.rs"]
mod fields;
use rs09_server::{
    definitions::{
        item::Shared,
        scenery::{self as defs, Registry},
    },
    world::{
        doors::{self, Handle, Location},
        scenery_node::{self as node, Children, Error},
    },
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
fn v(a: &Value, k: &str, d: i32) -> i32 {
    a[k].as_i64().map_or(d, |n| n as i32)
}
fn loc(a: &Value) -> Location {
    Location::new(
        a[0].as_i64().unwrap() as i32,
        a[1].as_i64().unwrap() as i32,
        a[2].as_i64().unwrap() as i32,
    )
}
struct Model {
    input: Value,
    nodes: BTreeMap<String, Handle>,
    views: BTreeMap<String, Option<Children>>,
    events: Vec<Value>,
    generated: usize,
}
impl defs::PlayerContext for Model {
    fn varbit(&mut self, id: i32, _: usize) -> defs::Result<Option<i32>> {
        self.events.push(json!(["varbit", id]));
        Ok(if self.input["missingVarbit"] == true {
            None
        } else {
            Some(v(&self.input, "configValue", 0))
        })
    }
    fn varp(&mut self, id: i32, _: usize) -> defs::Result<i32> {
        self.events.push(json!(["varp", id]));
        Ok(v(&self.input, "configValue", 0))
    }
}
impl node::Context for Model {
    fn varbit_exists(&mut self, id: i32) -> node::Result<bool> {
        self.events.push(json!(["varbit", id]));
        Ok(self.input["missingVarbit"] != true)
    }
    fn set_varbit(&mut self, _: usize, id: i32, value: i32) -> node::Result<()> {
        self.events.push(json!(["setVarbit", id, value]));
        Ok(())
    }
    fn set_varp(&mut self, _: usize, id: i32, value: i32) -> node::Result<()> {
        self.events.push(json!(["setVarp", id, value]));
        Ok(())
    }
    fn log_invalid_shape(&mut self, shape: i32, constructed: bool) -> node::Result<()> {
        self.events.push(json!([
            "log",
            if constructed {
                "core.game.node.scenery.Constructed"
            } else {
                "core.game.node.scenery.Scenery"
            },
            format!("Invalid scenery type {shape}")
        ]));
        Ok(())
    }
}
fn tag(id: usize, ids: &mut BTreeMap<usize, usize>) -> usize {
    let next = ids.len();
    *ids.entry(id).or_insert(next)
}
fn def(d: &defs::Handle) -> Value {
    let d = d.read();
    json!({"id":d.id,"name":d.name,"options":d.options,"sx":d.size_x,"sy":d.size_y,"file":d.config_file_id,"config":d.config_id,"children":d.children_ids})
}
impl Model {
    fn label(&mut self, h: &Handle) -> String {
        if let Some((k, _)) = self.nodes.iter().find(|(_, v)| v.same(h)) {
            return k.clone();
        }
        let k = format!("g{}", self.generated);
        self.generated += 1;
        self.nodes.insert(k.clone(), h.clone());
        k
    }
    fn snapshot(&mut self, registry: &Registry) -> Value {
        for view in self.views.clone().into_values().flatten() {
            for i in 0..view.len() {
                self.label(&view.get(i).unwrap());
            }
        }
        let mut todo: Vec<_> = self.nodes.keys().cloned().collect();
        let mut i = 0;
        while i < todo.len() {
            let h = self.nodes[&todo[i]].clone();
            if let Some(children) = node::children(&h) {
                for j in 0..children.len() {
                    let k = self.label(&children.get(j).unwrap());
                    if !todo.contains(&k) {
                        todo.push(k);
                    }
                }
            }
            let wrapper = doors::wrapper(&h);
            if !wrapper.same(&h) {
                let k = self.label(&wrapper);
                if !todo.contains(&k) {
                    todo.push(k);
                }
            }
            i += 1;
        }
        let (mut ids, mut locations, mut attrs, mut arrays) = (
            BTreeMap::new(),
            BTreeMap::new(),
            BTreeMap::new(),
            BTreeMap::new(),
        );
        let mut ds = BTreeMap::new();
        let mut keys: Vec<_> = registry.definitions.keys().copied().collect();
        keys.sort_unstable();
        for k in keys {
            let d = &registry.definitions[&k];
            ds.insert(
                k.to_string(),
                json!({"data":def(d),"ref":tag(d.identity(),&mut ids)}),
            );
        }
        let mut ns = BTreeMap::new();
        for (k, h) in self.nodes.clone() {
            let children = node::children(&h);
            let names = children.as_ref().map(|c| {
                (0..c.len())
                    .map(|i| self.label(&c.get(i).unwrap()))
                    .collect::<Vec<_>>()
            });
            let wrapper = doors::wrapper(&h);
            let wrapper = if wrapper.same(&h) {
                None
            } else {
                Some(self.label(&wrapper))
            };
            let attrs_h = node::attributes(&h);
            let d = node::definition(&h).unwrap();
            let h = h.read();
            ns.insert(k,json!({"id":h.id,"name":h.name,"shape":h.shape,"rotation":h.rotation,"direction":h.direction.code(),"location":h.location.coordinates(),"locationRef":tag(h.location.identity(),&mut locations),"size":h.node_size,"active":h.active,"renderable":h.renderable,"constructed":h.constructed,"charge":h.charge,"definition":def(&d),"definitionRef":tag(d.identity(),&mut ids),"attributesRef":tag(attrs_h.identity(),&mut attrs),"children":names,"childrenRef":children.map(|c|tag(c.identity(),&mut arrays)),"wrapper":wrapper}));
        }
        let mut views = BTreeMap::new();
        for (k, view) in self.views.clone() {
            let items = view.as_ref().map(|c| {
                (0..c.len())
                    .map(|i| self.label(&c.get(i).unwrap()))
                    .collect::<Vec<_>>()
            });
            let r = view.map(|c| tag(c.identity(), &mut arrays));
            views.insert(k, json!({"ref":r,"items":items}));
        }
        json!({"nodes":ns,"definitions":ds,"views":views})
    }
    fn action(&mut self, registry: &mut Registry, a: &Value) -> node::Result<Value> {
        let h = a["node"].as_str().and_then(|k| self.nodes.get(k)).cloned();
        let get = || h.clone().ok_or(Error::Null);
        match a["op"].as_str().unwrap() {
            "new" => {
                let l = a
                    .get("location")
                    .map(loc)
                    .unwrap_or_else(|| Location::new(3207, 3217, 0));
                let h = node::construct(
                    registry,
                    v(a, "id", 1),
                    l,
                    v(a, "shape", 0),
                    v(a, "rotation", 0),
                    a["constructed"] == true,
                )?;
                self.nodes.insert(a["name"].as_str().unwrap().into(), h);
            }
            "transform" => {
                let result = node::transform(
                    registry,
                    &get()?,
                    v(a, "id", 1),
                    a["rotation"].as_i64().map(|v| v as i32),
                    a.get("location").map(loc),
                    if a.get("location").is_some() {
                        None
                    } else {
                        a["shape"].as_i64().map(|v| v as i32)
                    },
                )?;
                self.nodes
                    .insert(a["name"].as_str().unwrap().into(), result);
            }
            "asConstructed" => {
                let h = node::as_constructed(registry, &get()?)?;
                self.nodes.insert(a["name"].as_str().unwrap().into(), h);
            }
            "child" => {
                let h = node::child(
                    registry,
                    self,
                    &get()?,
                    if a["player"] == true { Some(0) } else { None },
                )?;
                self.nodes.insert(a["name"].as_str().unwrap().into(), h);
            }
            "children" => {
                self.views
                    .insert(a["name"].as_str().unwrap().into(), node::children(&get()?));
            }
            "viewGet" => {
                let view = self
                    .views
                    .get(a["view"].as_str().unwrap())
                    .and_then(Option::as_ref)
                    .ok_or(Error::Null)?;
                let h = view.get(v(a, "index", 0) as usize).ok_or(Error::Index)?;
                self.nodes.insert(a["name"].as_str().unwrap().into(), h);
            }
            "wrapper" => {
                let wrapper = a["other"].as_str().and_then(|k| self.nodes.get(k)).cloned();
                node::set_wrapper(&get()?, wrapper);
            }
            "rotate" => get()?.write().rotation = v(a, "value", 0),
            "location" => get()?.write().location = loc(&a["value"]),
            "active" => get()?.write().active = a["value"] == true,
            "renderable" => get()?.write().renderable = a["value"] == true,
            "charge" => get()?.write().charge = v(a, "value", 0),
            "size" => return Ok(json!([node::size_x(&get()?)?, node::size_y(&get()?)?])),
            "center" => return Ok(json!(node::center(&get()?)?.coordinates())),
            "occupied" => {
                return Ok(json!(
                    node::occupied(&get()?)?
                        .iter()
                        .map(Location::coordinates)
                        .collect::<Vec<_>>()
                ));
            }
            "layer" => {
                return Ok(json!(node::layer(self, &get()?)?.map(|l| match l {
                    node::Layer::Wall => "WALL",
                    node::Layer::WallDecor => "WALL_DECOR",
                    node::Layer::Ground => "GROUND",
                    node::Layer::GroundDecor => "GROUND_DECOR",
                })));
            }
            "permanent" => return Ok(json!(!get()?.read().constructed)),
            "equals" => {
                return Ok(json!(node::equals(
                    &get()?,
                    a["other"].as_str().and_then(|k| self.nodes.get(k))
                )));
            }
            "setChildIndex" => node::set_child_index(self, &get()?, 0, v(a, "value", 0))?,
            "setDef" => {
                let d = if a.get("node").is_some() {
                    node::definition(&get()?)?
                } else {
                    registry.for_id(v(a, "id", 0))
                };
                for (k, v) in a["fields"].as_object().unwrap() {
                    fields::set(&d, k, v);
                }
            }
            "replaceDef" => {
                let d = Shared::new(defs::SceneryDefinition::default());
                for (k, v) in a["fields"].as_object().unwrap() {
                    fields::set(&d, k, v);
                }
                registry.definitions.insert(v(a, "id", 0), d);
            }
            _ => unreachable!(),
        };
        Ok(Value::Null)
    }
}
fn class(e: &Error) -> &str {
    match e {
        Error::Null => "Null",
        Error::Index => "Index",
        Error::Scenery(doors::Error::Direction) => "Direction",
        _ => "Binding",
    }
}
pub fn evaluate(input: &Value) -> Value {
    let mut registry = Registry::default();
    for (k, d) in input["definitions"].as_object().unwrap() {
        let h = registry.for_id(k.parse().unwrap());
        for (k, v) in d.as_object().unwrap() {
            fields::set(&h, k, v);
        }
    }
    let mut m = Model {
        input: input.clone(),
        nodes: BTreeMap::new(),
        views: BTreeMap::new(),
        events: vec![],
        generated: 0,
    };
    let mut out = vec![];
    for a in input["actions"].as_array().unwrap() {
        m.events.clear();
        let (result, error) = match m.action(&mut registry, a) {
            Ok(v) => (v, Value::Null),
            Err(e) => (Value::Null, json!(class(&e))),
        };
        let state = m.snapshot(&registry);
        out.push(json!({"result":result,"error":error,"state":state,"events":m.events}));
    }
    json!(out)
}
