use rs09_server::{
    definitions::item::Shared,
    world::{
        chunk::{HandleChunk, RegionChunk},
        clipping::{Flags, Landscape},
        doors::Location,
        landscape::{Chunks, ParseRegion},
        region,
        region_queries::{self as query, Context, Error, Result},
    },
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap};
fn land(value: &Value) -> Option<Landscape> {
    if value.is_null() {
        return None;
    }
    let rows = if let Some(b) = value.as_bool() {
        (0..8).map(|_| Some(Shared::new(vec![b; 8]))).collect()
    } else {
        value
            .as_array()
            .unwrap()
            .iter()
            .map(|r| {
                r.as_array()
                    .map(|r| Shared::new(r.iter().map(|v| v.as_bool().unwrap()).collect()))
            })
            .collect()
    };
    Some(Shared::new(rows))
}
fn chunks(chunk: HandleChunk) -> Chunks {
    Shared::new(std::array::from_fn(|_| {
        std::array::from_fn(|_| std::array::from_fn(|_| Some(chunk.clone())))
    }))
}
fn region(chunk: HandleChunk, has: bool) -> region::Handle {
    Shared::new(region::Region {
        data: Shared::new(ParseRegion {
            x: 0,
            y: 0,
            object_count: 0,
        }),
        chunks: Some(chunks(chunk)),
        loaded: false,
        build: false,
        has_flags: has,
        dynamic_template: None,
        add: Shared::new(region::Sceneries::default()),
        remove: Shared::new(region::Sceneries::default()),
        tolerances: Shared::new(HashMap::new()),
        activity: Shared::new(Default::default()),
    })
}
pub struct Harness {
    input: Value,
    regions: [region::Handle; 2],
    chunks: [HandleChunk; 2],
    selected: usize,
    point: Option<Location>,
    events: Vec<Value>,
    calls: HashMap<String, i32>,
    last_key: i32,
    last_projectile: bool,
    flags: BTreeMap<(bool, i32), Flags>,
}
impl Harness {
    pub fn new(input: &Value) -> Self {
        let cs =
            std::array::from_fn(|_| Shared::new(RegionChunk::new(Location::new(999, 1000, 2), 0)));
        cs[0].write().flags.landscape = land(input.get("landscape").unwrap_or(&Value::Bool(true)));
        cs[1].write().flags.landscape = land(&Value::Bool(false));
        let regions = [
            region(cs[0].clone(), input["hasFlags"] != false),
            region(cs[1].clone(), true),
        ];
        let point = input
            .get("location")
            .unwrap_or(&json!([3207, 3216, 0]))
            .as_array()
            .map(|a| {
                Location::new(
                    a[0].as_i64().unwrap() as i32,
                    a[1].as_i64().unwrap() as i32,
                    a[2].as_i64().unwrap() as i32,
                )
            });
        let mut flags = BTreeMap::new();
        if let Some(p) = &point {
            let [x, y, z] = p.coordinates();
            let key = z.wrapping_shl(22) | (x >> 3).wrapping_shl(11) | (y >> 3);
            flags.insert(
                (false, key),
                Shared::new([input["flag"].as_i64().unwrap_or(0) as i32; 64]),
            );
        }
        Self {
            input: input.clone(),
            regions,
            chunks: cs,
            selected: 0,
            point,
            events: vec![],
            calls: HashMap::new(),
            last_key: 0,
            last_projectile: false,
            flags,
        }
    }
    fn emit(&mut self, event: &str, args: Vec<Value>) -> Result<()> {
        let mut row = vec![json!(event)];
        row.extend(args);
        self.events.push(json!(row));
        let at = self.calls.entry(event.into()).or_default();
        *at += 1;
        let at = *at;
        for h in self.input["hooks"].as_array().cloned().unwrap_or_default() {
            if h["event"] == event && h["at"].as_i64().unwrap_or(1) == i64::from(at) {
                for a in h["actions"].as_array().unwrap() {
                    self.action(a)?;
                }
            }
        }
        Ok(())
    }
    fn query(&mut self, name: &str) -> Result<Value> {
        let l = self.point.clone();
        let l = l.as_ref();
        Ok(match name {
            "clip" => json!(query::clipping_flag(self, l, false)?),
            "projectile" => json!(query::clipping_flag(self, l, true)?),
            "landscape" => json!(query::is_landscape(self, l)?),
            "teleport" => json!(query::teleport_permitted(self, l)?),
            "clipped" => json!(query::is_clipped(self, l)?),
            "water" => json!(query::water_flag(self, l)?),
            _ => panic!("unknown query {name}"),
        })
    }
    fn action(&mut self, a: &Value) -> Result<Value> {
        let v = &a["value"];
        match a["op"].as_str().unwrap() {
            q @ ("clip" | "projectile" | "landscape" | "teleport" | "clipped" | "water") => {
                return self.query(q);
            }
            "query" => return self.query(a["name"].as_str().unwrap()),
            "fail" => return Err(Error::Binding("callback".into())),
            "select" => self.selected = v.as_u64().unwrap() as usize,
            "has" => {
                self.regions[a["region"].as_u64().unwrap_or(0) as usize]
                    .write()
                    .has_flags = v.as_bool().unwrap()
            }
            "location" => self.point.as_ref().ok_or(Error::Null)?.set(
                v[0].as_i64().unwrap() as i32,
                v[1].as_i64().unwrap() as i32,
                v[2].as_i64().unwrap() as i32,
            ),
            "land" => {
                self.chunks[a["chunk"].as_u64().unwrap_or(0) as usize]
                    .write()
                    .flags
                    .landscape = land(v)
            }
            "chunks" => {
                self.regions[a["region"].as_u64().unwrap_or(0) as usize]
                    .write()
                    .chunks = v.as_u64().map(|i| chunks(self.chunks[i as usize].clone()))
            }
            "fill" => {
                let p = self.last_projectile;
                let k = self.last_key;
                self.flags
                    .entry((p, k))
                    .or_insert_with(|| Shared::new([if p { 0 } else { -1 }; 64]))
                    .write()
                    .fill(v.as_i64().unwrap() as i32);
            }
            "replace" => {
                self.flags.insert(
                    (self.last_projectile, self.last_key),
                    Shared::new([v.as_i64().unwrap() as i32; 64]),
                );
            }
            _ => panic!("unknown action {a}"),
        }
        Ok(Value::Null)
    }
    fn snapshot(&self, result: Value, error: Option<&str>) -> Value {
        let matrix = |c: &HandleChunk| {
            c.read().flags.landscape.as_ref().map(|a| {
                a.read()
                    .iter()
                    .map(|r| r.as_ref().map(|r| r.read().clone()))
                    .collect::<Vec<_>>()
            })
        };
        let arrays = self
            .regions
            .iter()
            .map(|r| {
                r.read().chunks.as_ref().map(|a| {
                    usize::from(!a.read()[0][0][0].as_ref().unwrap().same(&self.chunks[0]))
                })
            })
            .collect::<Vec<_>>();
        json!({"result":result,"error":error,"events":self.events,"selected":self.selected,
            "has":self.regions.iter().map(|r|r.read().has_flags).collect::<Vec<_>>(),"chunks":arrays,
            "landscape":self.chunks.iter().map(matrix).collect::<Vec<_>>(),
            "flags":self.flags.iter().map(|((p,k),v)|json!([k,p,v.read().to_vec()])).collect::<Vec<_>>(),
            "location":self.point.as_ref().map(Location::coordinates)})
    }
}
impl Context for Harness {
    fn region(&mut self, l: &Location) -> Result<region::Handle> {
        let [x, y, _] = l.coordinates();
        let r = self.regions[self.selected].clone();
        let key = self.selected;
        self.emit(
            "region",
            vec![json!(((x >> 6) << 8) | (y >> 6)), json!(key)],
        )?;
        Ok(r)
    }
    fn load(&mut self, r: &region::Handle) -> Result<()> {
        let key = self.regions.iter().position(|v| v.same(r)).unwrap();
        self.emit("load", vec![json!(key)])
    }
    fn chunk(&mut self, l: &Location) -> Result<HandleChunk> {
        let r = self.region(l)?;
        let a = r.read().chunks.clone();
        let key = self.regions.iter().position(|v| v.same(&r)).unwrap();
        self.emit("chunks", vec![json!(key)])?;
        let a = a.ok_or(Error::Null)?;
        let [x, y, z] = l.coordinates();
        let c = a.read()[((x & 63) >> 3) as usize][((y & 63) >> 3) as usize][z as usize]
            .clone()
            .ok_or(Error::Null)?;
        Ok(c)
    }
    fn flags(&mut self, key: i32, p: bool) -> Result<Flags> {
        let f = self
            .flags
            .entry((p, key))
            .or_insert_with(|| Shared::new([if p { 0 } else { -1 }; 64]))
            .clone();
        self.last_key = key;
        self.last_projectile = p;
        self.emit("flags", vec![json!(key), json!(p)])?;
        Ok(f)
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
                h.events.clear();
                let (result, error) = match h.action(a) {
                    Ok(v) => (v, None),
                    Err(Error::Null) => (Value::Null, Some("Null")),
                    Err(Error::Index) => (Value::Null, Some("Index")),
                    Err(Error::Binding(_)) => (Value::Null, Some("Binding")),
                };
                h.snapshot(result, error)
            })
            .collect::<Vec<_>>()
    )
}
