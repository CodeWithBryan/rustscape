//! Test bindings for original RegionChunk scenery storage.
use rs09_server::{
    definitions::item::Shared,
    pulses,
    world::{
        chunk::{self, HandleChunk, RegionChunk},
        doors::{Handle, Location, Scenery},
        scenery::{self, Context, Error, ObjectUpdate, SceneryPulse},
    },
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
fn n(v: &Value) -> i32 {
    v.as_i64().unwrap() as i32
}
fn v(a: &Value, k: &str, d: i32) -> i32 {
    a[k].as_i64().map_or(d, |x| x as i32)
}
fn loc(a: &Value) -> Location {
    Location::new(n(&a[0]), n(&a[1]), n(&a[2]))
}
struct Model {
    nodes: BTreeMap<String, Handle>,
    chunks: BTreeMap<String, HandleChunk>,
    locations: BTreeMap<usize, usize>,
    retained_locations: Vec<Location>,
    generated: usize,
    fail_id: Option<i32>,
}
impl Model {
    fn location_id(&mut self, l: &Location) -> usize {
        let next = self.locations.len();
        let k = *self.locations.entry(l.identity()).or_insert(next);
        self.retained_locations.push(l.clone());
        k
    }
    fn label(&mut self, h: &Handle) -> String {
        if let Some((k, _)) = self.nodes.iter().find(|(_, n)| n.same(h)) {
            return k.clone();
        }
        let k = format!("g{}", self.generated);
        self.generated += 1;
        self.nodes.insert(k.clone(), h.clone());
        k
    }
    fn cells(&mut self, a: &chunk::Objects) -> Value {
        let mut out = vec![];
        for x in 0..8 {
            for y in 0..8 {
                for i in 0..4 {
                    let o = a.read()[x][y][i].clone();
                    if let Some(o) = o {
                        out.push(json!([x, y, i, self.label(&o)]));
                    }
                }
            }
        }
        json!(out)
    }
    fn snapshot(&mut self) -> Value {
        let mut cs = BTreeMap::new();
        for (k, c) in self.chunks.clone() {
            let c = c.read();
            cs.insert(k,json!({"static":self.cells(&c.static_objects),"dynamic":self.cells(&c.objects),"base":c.base.coordinates(),"baseRef":self.location_id(&c.base),"currentBase":c.current_base.coordinates(),"currentRef":self.location_id(&c.current_base),"rotation":c.rotation,"flags":[c.flags.base_x,c.flags.base_y,c.flags.base_z],"projectile":[c.projectile_flags.base_x,c.projectile_flags.base_y,c.projectile_flags.base_z]}));
        }
        let mut ns = BTreeMap::new();
        for (k, h) in self.nodes.clone() {
            let o = h.read();
            ns.insert(k,json!({"id":o.id,"shape":o.shape,"rotation":o.rotation,"direction":o.direction as i32,"location":o.location.coordinates(),"locationRef":self.location_id(&o.location),"active":o.active,"renderable":o.renderable,"constructed":o.constructed,"charge":o.charge}));
        }
        json!({"chunks":cs,"nodes":ns})
    }
    fn action(&mut self, a: &Value) -> Result<Value, chunk::Error> {
        let c = self.chunks[a["chunk"].as_str().unwrap_or("main")].clone();
        let o = a["node"].as_str().and_then(|k| self.nodes.get(k)).cloned();
        let h = || o.clone().ok_or(chunk::Error::Scenery(Error::Null));
        match a["op"].as_str().unwrap() {
            "static" => {
                chunk::add_static_dynamic(self, &c, &h()?, v(a, "x", 7), v(a, "y", 1))?;
            }
            "add" => return Ok(json!(chunk::add(self, &c, &h()?)?)),
            "index" => {
                return Ok(json!(chunk::get_index(
                    &c,
                    v(a, "x", 7),
                    v(a, "y", 1),
                    v(a, "id", -1),
                    v(a, "shape", -1)
                )?));
            }
            "copy" => {
                let copy = chunk::copy(self, &c)?;
                self.chunks.insert(a["name"].as_str().unwrap().into(), copy);
            }
            "rebase" => chunk::rebase_objects(&c),
            "base" => c.write().base = loc(&a["value"]),
            "currentBase" => c.write().current_base = loc(&a["value"]),
            "resetFlags" => c.write().reset_clipping_flags(),
            "rotation" => c.write().rotation = n(&a["value"]),
            "rotatePosition" => {
                return Ok(json!(chunk::rotated_position(
                    v(a, "x", 0),
                    v(a, "y", 0),
                    v(a, "sx", 1),
                    v(a, "sy", 1),
                    v(a, "rotation", 0),
                    v(a, "chunkRotation", 0)
                )));
            }
            "cell" => {
                let (x, y, i) = (v(a, "x", 7), v(a, "y", 1), v(a, "slot", 0));
                if !(0..8).contains(&x) || !(0..8).contains(&y) || !(0..4).contains(&i) {
                    return Err(chunk::Error::Index);
                }
                let ar = if a["static"] == true {
                    c.read().static_objects.clone()
                } else {
                    c.read().objects.clone()
                };
                ar.write()[x as usize][y as usize][i as usize] = o;
            }
            "active" => scenery::set_active(self, &h()?, a["value"] == true)?,
            "renderable" => h()?.write().renderable = a["value"] == true,
            "charge" => h()?.write().charge = n(&a["value"]),
            "rotate" => h()?.write().rotation = n(&a["value"]),
            "location" => h()?.write().location = loc(&a["value"]),
            "failId" => self.fail_id = a["value"].as_i64().map(|n| n as i32),
            _ => unreachable!(),
        };
        Ok(Value::Null)
    }
}
impl Context for Model {
    fn construct(
        &mut self,
        id: i32,
        l: Location,
        shape: i32,
        rotation: i32,
    ) -> scenery::Result<Handle> {
        if self.fail_id == Some(id) {
            return Err(Error::Binding("factory".into()));
        }
        Ok(Shared::new(
            Scenery::new(id, l, shape, rotation, Some("Door".into()), false)
                .map_err(|_| Error::Direction)?,
        ))
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
pub fn evaluate(input: &Value) -> Value {
    let mut m = Model {
        nodes: BTreeMap::new(),
        chunks: BTreeMap::new(),
        locations: BTreeMap::new(),
        retained_locations: vec![],
        generated: 0,
        fail_id: None,
    };
    for (k, r) in input["nodes"].as_object().unwrap() {
        let h = m
            .construct(
                n(&r["id"]),
                loc(&r["location"]),
                v(r, "shape", 0),
                v(r, "rotation", 0),
            )
            .unwrap();
        {
            let mut h = h.write();
            h.constructed = r["constructed"] == true;
            h.active = r["active"] != false;
            h.renderable = r["renderable"] != false;
            h.charge = v(r, "charge", 1000);
        }
        m.nodes.insert(k.clone(), h);
    }
    let base = input
        .get("base")
        .map(loc)
        .unwrap_or_else(|| Location::new(3200, 3216, 0));
    m.chunks.insert(
        "main".into(),
        Shared::new(RegionChunk::new(base, v(input, "rotation", 0))),
    );
    let mut out = vec![];
    for a in input["actions"].as_array().unwrap() {
        let (result, error) = match m.action(a) {
            Ok(v) => (v, Value::Null),
            Err(e) => (
                Value::Null,
                json!(match e {
                    chunk::Error::Index => "Index",
                    chunk::Error::Capacity(_) => "State",
                    chunk::Error::Scenery(Error::Null) => "Null",
                    chunk::Error::Scenery(Error::Direction) => "IllegalArgumentException",
                    _ => "Binding",
                }),
            ),
        };
        let mut s = m.snapshot();
        s["result"] = result;
        s["error"] = error;
        out.push(s);
    }
    json!(out)
}
