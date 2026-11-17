use rs09_server::{
    definitions::item::Shared,
    world::{
        chunk::HandleChunk,
        clipping::Landscape,
        doors::{Handle as Object, Location, Scenery},
        landscape::{Chunks, Mapscape},
        region::{self, Context, Error, Handle, Region},
    },
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
fn v(a: &Value, k: &str, d: i32) -> i32 {
    a[k].as_i64().map_or(d, |n| n as i32)
}
struct Model {
    input: Value,
    region: Handle,
    original: Vec<HandleChunk>,
    old: Vec<Landscape>,
    projectile: Vec<Landscape>,
    nodes: BTreeMap<String, Object>,
    events: Vec<Value>,
    errors: Vec<Value>,
    calls: BTreeMap<String, i32>,
    maps: Vec<Mapscape>,
}
impl Model {
    fn emit(&mut self, name: &str, args: Vec<Value>) -> region::Result<()> {
        let mut row = vec![json!(name)];
        row.extend(args);
        self.events.push(json!(row));
        let call = self.calls.entry(name.into()).or_default();
        *call += 1;
        let call = *call;
        let hooks = self.input["hooks"].as_array().cloned().unwrap_or_default();
        for h in hooks {
            if h["event"] == name && v(&h, "at", 1) == call {
                for a in h["actions"].as_array().unwrap() {
                    self.action(a)?;
                }
            }
        }
        if self.input["failEvent"] == name && v(&self.input, "failAt", 1) == call {
            return Err(Error::Binding("failure".into()));
        }
        Ok(())
    }
    fn map_id(&mut self, map: &Mapscape) -> usize {
        if let Some(i) = self.maps.iter().position(|m| m.same(map)) {
            i
        } else {
            self.maps.push(map.clone());
            self.maps.len() - 1
        }
    }
    fn action(&mut self, a: &Value) -> region::Result<()> {
        let r = self.region.clone();
        match a["op"].as_str().unwrap() {
            "load" => region::load(self, &r, a["build"].as_bool())?,
            "state" => {
                let mut r = r.write();
                if let Some(b) = a["loaded"].as_bool() {
                    r.loaded = b;
                }
                if let Some(b) = a["build"].as_bool() {
                    r.build = b;
                }
                if let Some(b) = a["flags"].as_bool() {
                    r.has_flags = b;
                }
            }
            "nullChunks" => r.write().chunks = None,
            "clear" | "push" | "erase" => {
                let list = if a["add"] == true {
                    r.read().add.clone()
                } else {
                    r.read().remove.clone()
                };
                match a["op"].as_str().unwrap() {
                    "clear" => list.write().clear(),
                    "push" => list
                        .write()
                        .push(self.nodes[a["node"].as_str().unwrap()].clone()),
                    _ => {
                        list.write()
                            .remove(v(a, "index", 0) as usize)
                            .ok_or_else(|| Error::Binding("index".into()))?;
                    }
                }
            }
            "input" => self.input[a["key"].as_str().unwrap()] = a["value"].clone(),
            _ => unreachable!(),
        };
        Ok(())
    }
    fn label(&self, h: &Object) -> String {
        self.nodes
            .iter()
            .find(|(_, v)| v.same(h))
            .unwrap()
            .0
            .clone()
    }
    fn snapshot(&self) -> Value {
        let (mut reset, mut marked, mut proj) = (vec![], vec![], vec![]);
        for (i, c) in self.original.iter().enumerate() {
            let c = c.read();
            let land = c.flags.landscape.as_ref().unwrap();
            if !land.same(&self.old[i]) {
                reset.push(i);
            }
            if land.read()[0].as_ref().unwrap().read()[0] {
                marked.push(i);
            }
            if !c
                .projectile_flags
                .landscape
                .as_ref()
                .unwrap()
                .same(&self.projectile[i])
            {
                proj.push(i);
            }
        }
        let r = self.region.read();
        let adds: Vec<_> = r
            .add
            .read()
            .values()
            .iter()
            .map(|h| self.label(h))
            .collect();
        let removes: Vec<_> = r
            .remove
            .read()
            .values()
            .iter()
            .map(|h| self.label(h))
            .collect();
        let maps: Vec<_> = self
            .maps
            .iter()
            .map(|m| {
                let mut hash = Sha256::new();
                for p in m.read().iter() {
                    for row in p {
                        hash.update(row.iter().map(|b| *b as u8).collect::<Vec<_>>());
                    }
                }
                format!("{:x}", hash.finalize())
            })
            .collect();
        json!({"loaded":r.loaded,"build":r.build,"flags":r.has_flags,"count":r.data.read().object_count,"chunksNull":r.chunks.is_none(),"reset":reset,"marked":marked,"projectileChanged":proj,"add":adds,"remove":removes,"maps":maps,"events":self.events})
    }
}
fn bytes(v: Option<&Value>, default: &str) -> Option<Vec<u8>> {
    let v = v.map_or(Some(default), Value::as_str)?;
    Some(
        v.as_bytes()
            .chunks_exact(2)
            .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
            .collect(),
    )
}
impl Context for Model {
    fn archive_id(&mut self, name: &str) -> region::Result<i32> {
        self.emit("archive", vec![json!(name)])?;
        Ok(v(
            &self.input,
            if name.starts_with('m') {
                "mapId"
            } else {
                "landId"
            },
            5,
        ))
    }
    fn mapscape_bytes(&mut self, id: i32) -> region::Result<Option<Vec<u8>>> {
        self.emit("mapBytes", vec![json!(id)])?;
        Ok(bytes(self.input.get("mapData"), "00"))
    }
    fn xtea(&mut self, id: i32) -> region::Result<[u32; 4]> {
        self.emit("keys", vec![json!(id)])?;
        Ok([u32::MAX, 0, i32::MIN as u32, i32::MAX as u32])
    }
    fn landscape_bytes(&mut self, id: i32, key: [u32; 4]) -> region::Result<Option<Vec<u8>>> {
        self.emit("landBytes", vec![json!(id), json!(key.map(|n| n as i32))])?;
        Ok(bytes(self.input.get("landData"), "00000000"))
    }
    fn parse_mapscape(&mut self, _: &Handle, map: &Mapscape, bytes: &[u8]) -> region::Result<()> {
        let id = self.map_id(map);
        self.emit("mapParse", vec![json!(id), json!(bytes.len())])?;
        map.write()[0][0][0] = 1;
        Ok(())
    }
    fn parse_landscape(
        &mut self,
        r: &Handle,
        chunks: Option<Chunks>,
        map: &Mapscape,
        bytes: &[u8],
        build: bool,
    ) -> region::Result<()> {
        let id = self.map_id(map);
        self.emit(
            "landParse",
            vec![
                json!(id),
                json!(bytes.len()),
                json!(build),
                json!(chunks.is_none()),
            ],
        )?;
        let data = r.read().data.clone();
        let old = data.read().object_count;
        data.write().object_count = old.wrapping_add(1);
        map.write()[0][0][1] = 2;
        Ok(())
    }
    fn clip_mapscape(
        &mut self,
        _: &Handle,
        chunks: Option<Chunks>,
        map: &Mapscape,
    ) -> region::Result<()> {
        let id = self.map_id(map);
        self.emit("clip", vec![json!(id), json!(chunks.is_none())])?;
        map.write()[0][0][2] = 3;
        Ok(())
    }
    fn object(&mut self, l: Location, id: i32, shape: i32) -> region::Result<Option<Object>> {
        self.emit(
            "object",
            vec![json!(l.coordinates()), json!(id), json!(shape)],
        )?;
        Ok(self.input.get("actual").map_or_else(
            || self.nodes.get("real").cloned(),
            |v| v.as_str().and_then(|k| self.nodes.get(k)).cloned(),
        ))
    }
    fn remove(&mut self, h: &Object) -> region::Result<()> {
        self.emit("remove", vec![json!(self.label(h))])
    }
    fn add(&mut self, h: &Object) -> region::Result<()> {
        self.emit("add", vec![json!(self.label(h))])
    }
    fn exception(&mut self, e: &Error, id: Option<i32>) -> region::Result<()> {
        self.errors.push(if let Some(id) = id {
            json!(["landscape", id])
        } else {
            json!([
                "outer",
                match e {
                    Error::Null => "Null",
                    Error::ConcurrentModification => "ConcurrentModification",
                    _ => "Binding",
                }
            ])
        });
        Ok(())
    }
}
pub fn evaluate(input: &Value) -> Value {
    let mut r = Region::new(50, 50);
    r.loaded = input["loaded"] == true;
    r.build = input["build"] == true;
    r.has_flags = input["flags"] == true;
    r.dynamic_template = input["template"].as_i64().map(|v| v as i32);
    r.data.write().object_count = v(input, "count", 0);
    let (mut original, mut old, mut projectile) = (vec![], vec![], vec![]);
    let chunks = r.chunks.clone().unwrap();
    for x in 0..8 {
        for y in 0..8 {
            for z in 0..4 {
                let c = chunks.read()[x][y][z].clone().unwrap();
                c.read().flags.mark_landscape(0, 0, true).unwrap();
                c.read()
                    .projectile_flags
                    .mark_landscape(0, 0, true)
                    .unwrap();
                old.push(c.read().flags.landscape.clone().unwrap());
                projectile.push(c.read().projectile_flags.landscape.clone().unwrap());
                original.push(c);
            }
        }
    }
    if let Some(nulls) = input["nulls"].as_array() {
        for p in nulls {
            chunks.write()[p[0].as_u64().unwrap() as usize][p[1].as_u64().unwrap() as usize]
                [p[2].as_u64().unwrap() as usize] = None;
        }
    }
    if input["nullChunks"] == true {
        r.chunks = None;
    }
    let nodes: BTreeMap<_, _> = [("a", 10, 0), ("b", 11, 1), ("c", 12, 2), ("real", 99, 3)]
        .into_iter()
        .map(|(k, id, x)| {
            (
                k.to_string(),
                Shared::new(
                    Scenery::new(
                        id,
                        Location::new(3200 + x, 3200, 0),
                        x,
                        0,
                        Some("stub".into()),
                        false,
                    )
                    .unwrap(),
                ),
            )
        })
        .collect();
    for name in input
        .get("remove")
        .cloned()
        .unwrap_or(json!(["a", "b"]))
        .as_array()
        .unwrap()
    {
        r.remove.write().push(nodes[name.as_str().unwrap()].clone());
    }
    for name in input
        .get("add")
        .cloned()
        .unwrap_or(json!(["c"]))
        .as_array()
        .unwrap()
    {
        r.add.write().push(nodes[name.as_str().unwrap()].clone());
    }
    let mut m = Model {
        input: input.clone(),
        region: Shared::new(r),
        original,
        old,
        projectile,
        nodes,
        events: vec![],
        errors: vec![],
        calls: BTreeMap::new(),
        maps: vec![],
    };
    let mut out = vec![];
    for a in input["actions"].as_array().unwrap() {
        m.events.clear();
        m.errors.clear();
        m.action(a).unwrap();
        let mut s = m.snapshot();
        s["errors"] = json!(m.errors);
        out.push(s);
    }
    json!(out)
}
