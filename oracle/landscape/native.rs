//! Explicit native region/definition bindings around the landscape kernel.
use rs09_protocol::buffer::Reader;
use rs09_server::{
    definitions::item::Shared,
    pulses,
    world::{
        chunk::{HandleChunk, RegionChunk},
        clipping::{self, Border, Flags},
        doors::{Handle, Location, Scenery},
        landscape::{self, Definition},
        scenery::{self, ObjectUpdate, SceneryPulse},
    },
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
fn v(a: &Value, k: &str, d: i32) -> i32 {
    a[k].as_i64().map_or(d, |n| n as i32)
}
fn location(a: &Value) -> Location {
    Location::new(
        a[0].as_i64().unwrap() as i32,
        a[1].as_i64().unwrap() as i32,
        a[2].as_i64().unwrap() as i32,
    )
}
struct Model {
    nodes: BTreeMap<String, Handle>,
    chunks: BTreeMap<String, HandleChunk>,
    flags: BTreeMap<String, Flags>,
    definitions: BTreeMap<i32, Shared<Definition>>,
    bound: BTreeMap<usize, Shared<Definition>>,
    actions: BTreeMap<usize, bool>,
    region: Shared<landscape::ParseRegion>,
    map: landscape::Mapscape,
    position: usize,
}
fn object(h: Option<&Handle>) -> Value {
    let Some(h) = h else {
        return Value::Null;
    };
    let o = h.read();
    let [x, y, z] = o.location.coordinates();
    json!([
        o.id,
        x,
        y,
        z,
        o.shape,
        o.rotation,
        o.active,
        o.renderable,
        o.constructed
    ])
}
impl Model {
    fn raw_chunk(&mut self, l: &Location) -> HandleChunk {
        let [x, y, z] = l.coordinates();
        let base = Location::new(x & !7, y & !7, z);
        self.chunks
            .entry(base.to_string())
            .or_insert_with(|| Shared::new(RegionChunk::new(base, 0)))
            .clone()
    }
    fn get_chunk(&mut self, l: &Location) -> HandleChunk {
        let [x, y, z] = l.coordinates();
        let id = ((x >> 6) << 8) | (y >> 6);
        let base = Location::new(
            (((id >> 8) & 255) << 6) + (x & 63),
            ((id & 255) << 6) + (y & 63),
            z,
        );
        self.raw_chunk(&base)
    }
    fn snapshot(&self) -> Value {
        let mut cells = vec![];
        let mut tiles = vec![];
        for (k, c) in &self.chunks {
            let c = c.read();
            let s = c.static_objects.read();
            let d = c.objects.read();
            for x in 0..8 {
                for y in 0..8 {
                    if c.flags.landscape.as_ref().unwrap().read()[x]
                        .as_ref()
                        .unwrap()
                        .read()[y]
                    {
                        tiles.push(json!([k, x, y]));
                    }
                    for i in 0..4 {
                        if s[x][y][i].is_some() || d[x][y][i].is_some() {
                            cells.push(json!([
                                k,
                                x,
                                y,
                                i,
                                object(s[x][y][i].as_ref()),
                                object(d[x][y][i].as_ref())
                            ]));
                        }
                    }
                }
            }
        }
        let flags: BTreeMap<_, _> = self
            .flags
            .iter()
            .map(|(k, f)| (k.clone(), f.read().to_vec()))
            .collect();
        let nodes: BTreeMap<_, _> = self
            .nodes
            .iter()
            .map(|(k, h)| (k.clone(), object(Some(h))))
            .collect();
        json!({"cells":cells,"tiles":tiles,"flags":flags,"nodes":nodes,"position":self.position,"count":self.region.read().object_count})
    }
    fn action(&mut self, a: &Value) -> landscape::Result<Value> {
        let h = a["node"].as_str().and_then(|k| self.nodes.get(k)).cloned();
        let h = || h.clone().ok_or(landscape::Error::Null);
        match a["op"].as_str().unwrap() {
            "add" => landscape::add(self, &h()?, a["landscape"] == true)?,
            "flag" | "apply" => {
                let h = h()?;
                let l = h.read().location.clone();
                let c = self.raw_chunk(&l);
                let [x, y, _] = l.coordinates();
                if a["op"] == "apply" {
                    return Ok(json!(landscape::apply_clipping(
                        self,
                        &c,
                        v(a, "x", x & 7),
                        v(a, "y", y & 7),
                        &h
                    )?));
                }
                landscape::flag(
                    self,
                    &c,
                    v(a, "x", x & 7),
                    v(a, "y", y & 7),
                    &h,
                    a["landscape"] == true,
                    a["store"] == true,
                )?;
            }
            "remove" => return Ok(object(landscape::remove(self, &h()?)?.as_ref())),
            "active" => scenery::set_active(self, &h()?, a["value"] == true)?,
            "renderable" => h()?.write().renderable = a["value"] == true,
            "parse" => {
                let chunks: landscape::Chunks = Shared::new(std::array::from_fn(|x| {
                    std::array::from_fn(|y| {
                        std::array::from_fn(|z| {
                            Some(self.raw_chunk(&Location::new(
                                3200 + (x as i32) * 8,
                                3200 + (y as i32) * 8,
                                z as i32,
                            )))
                        })
                    })
                }));
                let data: Vec<_> = a["data"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_u64().unwrap() as u8)
                    .collect();
                let mut r = Reader::new(&data);
                let region = self.region.clone();
                let map = self.map.clone();
                let result =
                    landscape::parse(self, &region, &chunks, &map, &mut r, a["store"] == true);
                self.position = r.position();
                result?;
            }
            _ => unreachable!(),
        };
        Ok(Value::Null)
    }
}
impl scenery::Context for Model {
    fn construct(
        &mut self,
        id: i32,
        l: Location,
        shape: i32,
        rotation: i32,
    ) -> scenery::Result<Handle> {
        let def = self
            .definitions
            .entry(id)
            .or_insert_with(|| Shared::new(Definition::default()))
            .clone();
        let h = Shared::new(
            Scenery::new(id, l, shape, rotation, Some("Door".into()), false)
                .map_err(|_| scenery::Error::Direction)?,
        );
        self.bound.insert(h.identity(), def);
        Ok(h)
    }
    fn remove_landscape(&mut self, _: &Handle) -> scenery::Result<Option<Handle>> {
        unreachable!()
    }
    fn add_landscape(&mut self, _: &Handle) -> scenery::Result<()> {
        unreachable!()
    }
    fn chunk(&mut self, _: Location) -> scenery::Result<usize> {
        unreachable!()
    }
    fn flag(&mut self, _: usize, _: ObjectUpdate) -> scenery::Result<()> {
        unreachable!()
    }
    fn stop(&mut self, _: &pulses::Handle) -> scenery::Result<()> {
        unreachable!()
    }
    fn pulse_direct(&mut self, _: &pulses::Handle) -> scenery::Result<bool> {
        unreachable!()
    }
    fn submit(&mut self, _: SceneryPulse) -> scenery::Result<()> {
        unreachable!()
    }
    fn ground_create(&mut self, _: Option<usize>) -> scenery::Result<()> {
        unreachable!()
    }
}
impl clipping::Context for Model {
    fn flags(&mut self, key: i32, p: bool) -> clipping::Result<Flags> {
        Ok(self
            .flags
            .entry(format!("{}:{key}", if p { "p" } else { "m" }))
            .or_insert_with(|| Shared::new([if p { 0 } else { -1 }; 64]))
            .clone())
    }
    fn border(&mut self, b: Border) -> clipping::Result<()> {
        let l = Location::new(b.x, b.y, b.z);
        let c = self.get_chunk(&l);
        let flags = if b.projectile {
            c.read().projectile_flags.clone()
        } else {
            c.read().flags.clone()
        };
        let [x, y, _] = l.coordinates();
        flags.flag(self, x & 7, y & 7, b.mask, b.add)
    }
}
impl landscape::Context for Model {
    fn definition(&mut self, h: &Handle) -> landscape::Result<Shared<Definition>> {
        Ok(self.bound[&h.identity()].clone())
    }
    fn child_has_actions(&mut self, d: &Shared<Definition>) -> landscape::Result<bool> {
        Ok(self.actions.get(&d.identity()).copied().unwrap_or(true))
    }
    fn load_region(&mut self, _: &Location) -> landscape::Result<()> {
        Ok(())
    }
    fn region_chunk(&mut self, l: &Location) -> landscape::Result<HandleChunk> {
        Ok(self.get_chunk(l))
    }
}
fn error(e: landscape::Error) -> &'static str {
    match e {
        landscape::Error::Null => "Null",
        landscape::Error::Index
        | landscape::Error::Chunk(rs09_server::world::chunk::Error::Index)
        | landscape::Error::Clipping(clipping::Error::Index) => "Index",
        landscape::Error::Chunk(rs09_server::world::chunk::Error::Capacity(_)) => "State",
        landscape::Error::Decode(_) => "Decode",
        _ => "Binding",
    }
}
pub fn evaluate(input: &Value) -> Value {
    use scenery::Context;
    let mut m = Model {
        nodes: BTreeMap::new(),
        chunks: BTreeMap::new(),
        flags: BTreeMap::new(),
        definitions: BTreeMap::new(),
        bound: BTreeMap::new(),
        actions: BTreeMap::new(),
        region: Shared::new(landscape::ParseRegion {
            x: 50,
            y: 50,
            object_count: v(input, "count", 0),
        }),
        map: Shared::new(Box::new([[[0; 64]; 64]; 4])),
        position: 0,
    };
    if let Some(ds) = input["definitions"].as_object() {
        for (k, d) in ds {
            let def = Shared::new(Definition {
                size_x: v(d, "sx", 1),
                size_y: v(d, "sy", 1),
                clip_type: v(d, "clip", 2),
                interactable: v(d, "interactable", -1),
                second_bool: d["second"] == true,
                projectile_clipped: d["projectile"] != false,
                ..Definition::default()
            });
            m.actions.insert(def.identity(), d["actions"] != false);
            m.definitions.insert(k.parse().unwrap(), def);
        }
    }
    for (k, r) in input["nodes"].as_object().unwrap() {
        let l = r
            .get("location")
            .map(location)
            .unwrap_or_else(|| Location::new(3207, 3217, 0));
        let h = m
            .construct(v(r, "id", 1), l, v(r, "shape", 0), v(r, "rotation", 0))
            .unwrap();
        m.nodes.insert(k.clone(), h);
    }
    if let Some(bs) = input["bridges"].as_array() {
        for b in bs {
            m.map.write()[1][b[0].as_u64().unwrap() as usize][b[1].as_u64().unwrap() as usize] = 2;
        }
    }
    let mut out = vec![];
    for a in input["actions"].as_array().unwrap() {
        let (result, error) = match m.action(a) {
            Ok(v) => (v, Value::Null),
            Err(e) => (Value::Null, json!(error(e))),
        };
        let mut s = m.snapshot();
        s["result"] = result;
        s["error"] = error;
        out.push(s);
    }
    json!(out)
}
