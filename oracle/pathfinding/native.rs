use rs09_server::world::{
    doors::Location,
    pathfinding::{Context, Error, Reach, Request, Result, Smart},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::VecDeque;
fn n(a: &Value, k: &str, d: i32) -> i32 {
    a[k].as_i64().map_or(d, |n| n as i32)
}
fn array<const N: usize>(a: &Value, k: &str, default: [i32; N]) -> [i32; N] {
    a[k].as_array().map_or(default, |v| {
        std::array::from_fn(|i| v[i].as_i64().unwrap() as i32)
    })
}
fn location(a: &Value, k: &str, default: [i32; 3]) -> Option<Location> {
    if a.get(k) == Some(&Value::Null) {
        None
    } else {
        let [x, y, z] = array(a, k, default);
        Some(Location::new(x, y, z))
    }
}
fn coord(l: &Option<Location>) -> Value {
    l.as_ref().map_or(Value::Null, |l| json!(l.coordinates()))
}
fn hash(data: &[i32]) -> String {
    let mut h = Sha256::new();
    for n in data {
        h.update(n.to_be_bytes());
    }
    format!("{:x}", h.finalize())
}
struct Model {
    input: Value,
    probes: usize,
    digest: Sha256,
    prefix: Vec<[i32; 4]>,
    suffix: VecDeque<[i32; 4]>,
    start: Option<Location>,
    end: Option<Location>,
}
impl Model {
    fn clip(&mut self, z: i32, x: i32, y: i32, global: bool) -> Result<i32> {
        self.probes += 1;
        let event = [i32::from(global), z, x, y];
        for n in event {
            self.digest.update(n.to_be_bytes());
        }
        if self.prefix.len() < 24 {
            self.prefix.push(event);
        }
        self.suffix.push_back(event);
        if self.suffix.len() > 24 {
            self.suffix.pop_front();
        }
        if self.probes == n(&self.input, "failAt", -1) as usize {
            return Err(Error::Binding("oracle".into()));
        }
        if self.probes == n(&self.input, "mutateAt", -1) as usize {
            for (k, l) in [("mutateStart", &self.start), ("mutateEnd", &self.end)] {
                if self.input.get(k).is_some() {
                    let [x, y, z] = array(&self.input, k, [0; 3]);
                    l.as_ref().unwrap().set(x, y, z);
                }
            }
        }
        let mut value = n(
            &self.input,
            if global { "worldDefault" } else { "default" },
            0,
        );
        for row in self.input[if global { "worldFlags" } else { "flags" }]
            .as_array()
            .into_iter()
            .flatten()
        {
            let a = row.as_array().unwrap();
            if a[0].as_i64().unwrap() as i32 == x
                && a[1].as_i64().unwrap() as i32 == y
                && (a.len() < 4 || a[3].as_i64().unwrap() as i32 == z)
            {
                value = a[2].as_i64().unwrap() as i32;
            }
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
    let mut model = Model {
        input: input.clone(),
        probes: 0,
        digest: Sha256::new(),
        prefix: vec![],
        suffix: VecDeque::new(),
        start: location(input, "start", [3207, 3216, 0]),
        end: location(input, "end", [3210, 3218, 0]),
    };
    let reach = Reach {
        from: array(input, "from", [3207, 3216, 0]),
        mover: array(input, "mover", [1, 1]),
        to: array(input, "to", [3208, 3216]),
        size: array(input, "size", [1, 1]),
        kind: n(input, "kind", 0),
        rotation: n(input, "rotation", 0),
        walking_flag: n(input, "walkingFlag", 0),
    };
    let mut smart = Smart::default();
    let find = input["op"].as_str().unwrap_or("find") == "find";
    let result: Result<Value> = match input["op"].as_str().unwrap_or("find") {
        "door" => reach.door(&mut model).map(|v| json!(v)),
        "decoration" => reach.decoration(&mut model).map(|v| json!(v)),
        "standing" => reach.standing(&mut model).map(|v| json!(v)),
        "rectangle" => reach.rectangle(&mut model).map(|v| json!(v)),
        "sized" => reach.sized(&mut model).map(|v| json!(v)),
        "find" => {
            let r = Request {
                start: model.start.clone(),
                end: model.end.clone(),
                mover_size: reach.mover[0],
                size: reach.size,
                rotation: reach.rotation,
                kind: reach.kind,
                walking_flag: reach.walking_flag,
                near: input["near"].as_bool().unwrap_or(false),
                bfs: input["bfs"].as_bool().unwrap_or(false),
                max_distance: n(input, "maxDistance", 25),
                has_supplier: !input["nullSupplier"].as_bool().unwrap_or(false),
            };
            smart
                .find(&mut model, &r)
                .map(|p| json!([p.successful, p.move_near, p.points]))
        }
        op => panic!("unknown{op}"),
    };
    let (result, error) = match result {
        Ok(v) => (v, None),
        Err(e) => (
            Value::Null,
            Some(match e {
                Error::Null => "Null",
                Error::Index => "Index",
                Error::Limit => "Limit",
                Error::Binding(_) => "Binding",
            }),
        ),
    };
    let state = if find {
        let s = smart.state();
        json!({"curX":s.current[0],"curY":s.current[1],"dstX":s.destination[0],"dstY":s.destination[1],"writePathPosition":s.write_position,"foundPath":s.found,"queueX":hash(s.queue_x),"queueY":hash(s.queue_y),"via":hash(s.via),"cost":hash(s.cost)})
    } else {
        Value::Null
    };
    json!({"result":result,"error":error,"probes":model.probes,"probeHash":format!("{:x}",model.digest.finalize()),"prefix":model.prefix,"suffix":model.suffix,"start":coord(&model.start),"end":coord(&model.end),"state":state})
}
