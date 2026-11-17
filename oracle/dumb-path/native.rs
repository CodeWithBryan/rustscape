use rs09_server::world::{
    doors::Location,
    pathfinding::{Context, Dumb, Error, Request, Result},
};
use serde_json::{Value, json};
use std::sync::atomic::Ordering::Relaxed;
fn n(v: &Value, k: &str, d: i32) -> i32 {
    v[k].as_i64().map_or(d, |n| n as i32)
}
fn loc(v: &Value) -> Option<Location> {
    let a = v.as_array()?;
    Some(Location::new(
        a[0].as_i64()? as i32,
        a[1].as_i64()? as i32,
        a[2].as_i64()? as i32,
    ))
}
struct Model {
    input: Value,
    finder: Dumb,
    start: Option<Location>,
    end: Option<Location>,
    queries: Vec<Value>,
    nested: Vec<Value>,
    calls: i32,
}
impl Model {
    fn find(&mut self, a: &Value) -> Value {
        self.start = loc(a.get("start").unwrap_or(&json!([3207, 3216, 0])));
        self.end = loc(a.get("end").unwrap_or(&json!([3211, 3219, 0])));
        if let Some(offset) = a["endOffset"].as_array() {
            let [x, y, z] = self.start.as_ref().unwrap().coordinates();
            self.end = Some(Location::new(
                x.wrapping_add(offset[0].as_i64().unwrap() as i32),
                y.wrapping_add(offset[1].as_i64().unwrap() as i32),
                z,
            ));
        }
        let size = a["size"].as_array().map_or([0, 0], |a| {
            [a[0].as_i64().unwrap() as i32, a[1].as_i64().unwrap() as i32]
        });
        let request = Request {
            start: self.start.clone(),
            end: self.end.clone(),
            mover_size: n(a, "mover", 1),
            size,
            rotation: n(a, "rotation", 0),
            kind: n(a, "kind", 0),
            walking_flag: n(a, "walkingFlag", 0),
            near: a["near"].as_bool().unwrap_or(false),
            bfs: false,
            max_distance: 25,
            has_supplier: a["supplier"].as_bool().unwrap_or(true),
        };
        let (path, error) = match self.finder.clone().find(self, &request) {
            Ok(p) => (json!([p.successful, p.move_near, p.points]), Value::Null),
            Err(e) => (
                Value::Null,
                json!(match e {
                    Error::Binding(_) => "Binding".into(),
                    e => e.to_string(),
                }),
            ),
        };
        json!({"path":path,"error":error,"state":self.finder.state()})
    }
    fn act(&mut self, a: &Value) {
        match a["op"].as_str().unwrap() {
            "find" => {
                let v = self.find(a);
                self.nested.push(v);
            }
            "fields" => {
                for (k, v) in a["values"].as_object().unwrap() {
                    match k.as_str() {
                        "x" => self
                            .finder
                            .fields
                            .x
                            .store(v.as_i64().unwrap() as i32, Relaxed),
                        "y" => self
                            .finder
                            .fields
                            .y
                            .store(v.as_i64().unwrap() as i32, Relaxed),
                        "z" => self
                            .finder
                            .fields
                            .z
                            .store(v.as_i64().unwrap() as i32, Relaxed),
                        "found" => self
                            .finder
                            .fields
                            .found
                            .store(v.as_bool().unwrap(), Relaxed),
                        _ => panic!("unknown field"),
                    }
                }
            }
            "location" => {
                let l = if a["field"] == "start" {
                    self.start.as_ref()
                } else {
                    self.end.as_ref()
                }
                .unwrap();
                let [x, y, z] = loc(&a["value"]).unwrap().coordinates();
                l.set(x, y, z);
            }
            _ => panic!("unknown hook"),
        }
    }
    fn clip(&mut self, z: i32, x: i32, y: i32, world: bool) -> Result<i32> {
        let mut value = n(
            &self.input,
            if world { "worldDefault" } else { "default" },
            0,
        );
        for row in self.input[if world { "worldFlags" } else { "flags" }]
            .as_array()
            .cloned()
            .unwrap_or_default()
        {
            let a = row.as_array().unwrap();
            if a[0].as_i64().unwrap() as i32 == x && a[1].as_i64().unwrap() as i32 == y {
                value = a[2].as_i64().unwrap() as i32;
            }
        }
        self.queries.push(json!([world, z, x, y, value]));
        self.calls += 1;
        let at = self.calls;
        for hook in self.input["hooks"].as_array().cloned().unwrap_or_default() {
            if n(&hook, "at", 1) == at {
                for a in hook["actions"].as_array().unwrap() {
                    self.act(a);
                }
            }
        }
        if at == n(&self.input, "failAt", -1) {
            return Err(Error::Binding(String::new()));
        }
        Ok(value)
    }
}
impl Context for Model {
    fn clipping(&mut self, z: i32, x: i32, y: i32) -> Result<i32> {
        self.clip(z, x, y, false)
    }
    fn world_clipping(&mut self, z: i32, x: i32, y: i32) -> Result<i32> {
        self.clip(z, x, y, true)
    }
}
pub fn evaluate(input: &Value) -> Value {
    let mut m = Model {
        input: input.clone(),
        finder: Dumb::default(),
        start: None,
        end: None,
        queries: vec![],
        nested: vec![],
        calls: 0,
    };
    let mut out = vec![];
    for a in input["actions"].as_array().unwrap() {
        m.queries.clear();
        m.nested.clear();
        let mut result = m.find(a);
        result["queries"] = json!(m.queries);
        result["nested"] = json!(m.nested);
        out.push(result);
    }
    json!(out)
}
