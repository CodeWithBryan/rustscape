use rs09_server::world::{
    doors::Location,
    pathfinding::{
        Error, Result,
        dispatch::{self, Context, Definition, Invocation, Kind, Supplier, View},
    },
};
use serde_json::{Value, json};
fn n(v: &Value, key: &str, default: i32) -> i32 {
    v[key].as_i64().map_or(default, |n| n as i32)
}
fn loc(v: &Value) -> Option<Location> {
    let a = v.as_array()?;
    Some(Location::new(
        a[0].as_i64()? as i32,
        a[1].as_i64()? as i32,
        a[2].as_i64()? as i32,
    ))
}
fn coord(l: Option<&Location>) -> Value {
    l.map_or(Value::Null, |l| json!(l.coordinates()))
}
fn custom(v: &Value) -> Option<Supplier> {
    v.as_u64().map(|n| Supplier::Custom(n as usize))
}
struct Model {
    input: Value,
    nodes: [Option<View>; 2],
    events: Vec<Value>,
    invocation: Value,
}
impl Model {
    fn event(&mut self, name: &str, args: Vec<Value>) -> Result<()> {
        let mut e = vec![json!(name)];
        e.extend(args);
        self.events.push(json!(e));
        if self.input["mutateAt"].as_str() == Some(name) {
            for (id, key) in [(0, "mutateMover"), (1, "mutateTarget")] {
                if self.input.get(key).is_some() {
                    self.nodes[id].as_mut().unwrap().location = loc(&self.input[key]);
                }
            }
        }
        if self.input["fail"].as_str() == Some(name) {
            return Err(Error::Binding("oracle".into()));
        }
        Ok(())
    }
}
impl Context for Model {
    type Output = Value;
    fn node(&self, id: usize) -> Result<View> {
        self.nodes[id].clone().ok_or(Error::Null)
    }
    fn npc_supplier(&mut self, _: usize) -> Result<Option<Supplier>> {
        if self.input["behavior"] == false {
            return Err(Error::Null);
        }
        self.event("npc", vec![])?;
        Ok(custom(&self.input["npcSupplier"]))
    }
    fn teleport_permitted(&mut self, l: Option<Location>) -> Result<bool> {
        self.event("permitted", vec![coord(l.as_ref())])?;
        Ok(self.input["permitted"] != false)
    }
    fn invoke(&mut self, i: Invocation) -> Result<Value> {
        let finder = i.finder.ok_or(Error::Null)?;
        let s = match i.supplier {
            None => Value::Null,
            Some(Supplier::World) => json!("world"),
            Some(Supplier::Water) => json!("water"),
            Some(Supplier::Custom(n)) => json!(n),
        };
        self.invocation = json!([
            finder,
            coord(i.start.as_ref()),
            i.mover_size,
            coord(i.end.as_ref()),
            i.size[0],
            i.size[1],
            i.rotation,
            i.kind,
            i.walking_flag,
            i.near,
            s
        ]);
        self.event("invoke", vec![self.invocation.clone()])?;
        Ok(self.invocation.clone())
    }
}
pub fn evaluate(input: &Value) -> Value {
    let nodes = std::array::from_fn(|id| {
        let (key, default, location, size) = if id == 0 {
            ("moverKind", "entity", "start", "moverSize")
        } else {
            ("targetKind", "other", "end", "size")
        };
        if input.get(key) == Some(&Value::Null) {
            return None;
        }
        let kind = match input[key].as_str().unwrap_or(default) {
            "entity" => Kind::Entity,
            "npc" => Kind::Npc,
            "scenery" => Kind::Scenery,
            "item" => Kind::GroundItem,
            _ => Kind::Other,
        };
        let l = input.get(location).cloned().unwrap_or_else(|| {
            if id == 0 {
                json!([3207, 3216, 0])
            } else {
                json!([3210, 3218, 0])
            }
        });
        let dims = input.get("dimensions").cloned().unwrap_or(json!([1, 1]));
        Some(View {
            kind,
            location: loc(&l),
            size: n(input, size, 1),
            scenery: if kind == Kind::Scenery {
                Some(dispatch::Scenery {
                    kind: n(input, "kind", 10),
                    rotation: n(input, "rotation", 0),
                    definition: if input.get("definition") == Some(&Value::Null) {
                        None
                    } else {
                        Some(Definition {
                            size: [
                                dims[0].as_i64().unwrap() as i32,
                                dims[1].as_i64().unwrap() as i32,
                            ],
                            walking_flag: n(input, "walkingFlag", 0),
                        })
                    },
                })
            } else {
                None
            },
        })
    });
    let mover = nodes[0].as_ref().map(|_| 0);
    let target = nodes[1].as_ref().map(|_| 1);
    let start = nodes[0].as_ref().and_then(|n| n.location.clone());
    let explicit_start = loc(&input
        .get("start")
        .cloned()
        .unwrap_or(json!([3207, 3216, 0])));
    let mut m = Model {
        input: input.clone(),
        nodes,
        events: vec![],
        invocation: Value::Null,
    };
    let near = input["near"] != false;
    let finder = if input.get("finder") == Some(&Value::Null) {
        None
    } else {
        Some(n(input, "finder", 0) as usize)
    };
    let supplier = custom(&input["supplier"]);
    let result = match input["mode"].as_str().unwrap_or("entity") {
        "entityDefault" => dispatch::entity(&mut m, mover, target, true, Some(0)),
        "water" => dispatch::entity_water(&mut m, mover, target, near, finder),
        "explicit" => dispatch::entity_with_supplier(&mut m, mover, target, near, finder, supplier),
        "start" => dispatch::find(
            &mut m,
            explicit_start,
            n(input, "moverSize", 1),
            target,
            near,
            finder,
            supplier,
        ),
        "locationDefault" => dispatch::location(&mut m, start, target),
        "locationSize" => dispatch::find(
            &mut m,
            start,
            n(input, "moverSize", 1),
            target,
            true,
            Some(0),
            Some(Supplier::World),
        ),
        "locationFinder" => dispatch::find(
            &mut m,
            start,
            1,
            target,
            near,
            finder,
            Some(Supplier::World),
        ),
        _ => dispatch::entity(&mut m, mover, target, near, finder),
    };
    let (result, error) = match result {
        Ok(v) => (v, None),
        Err(Error::Null) => (Value::Null, Some("Null")),
        Err(Error::Binding(_)) => (Value::Null, Some("Binding")),
        Err(e) => panic!("{e:?}"),
    };
    json!({"result":result,"error":error,"invocation":m.invocation,"events":m.events,"mover":coord(m.nodes[0].as_ref().and_then(|n|n.location.as_ref())),"target":coord(m.nodes[1].as_ref().and_then(|n|n.location.as_ref()))})
}
