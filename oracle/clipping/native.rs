//! Test-only flag store and boundary routing; region loading remains a world binding.
use rs09_server::{
    definitions::item::Shared,
    world::clipping::{Border, ChunkFlags, Context, Error, Flags, Result},
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
fn n(v: &Value) -> i32 {
    v.as_i64().unwrap() as i32
}
fn val(v: &Value, k: &str, d: i32) -> i32 {
    v[k].as_i64().map_or(d, |v| v as i32)
}
struct Model {
    chunk: ChunkFlags,
    tables: BTreeMap<String, Flags>,
    events: Vec<Value>,
    calls: BTreeMap<String, i32>,
    hooks: Vec<Value>,
}
impl Model {
    fn emit(&mut self, event: &str, args: Vec<Value>) -> Result<()> {
        let mut row = vec![json!(event)];
        row.extend(args);
        self.events.push(json!(row));
        let n = self.calls.entry(event.into()).or_default();
        *n += 1;
        let n = *n;
        for hook in self.hooks.clone() {
            if hook["event"] == event && val(&hook, "at", 1) == n {
                for a in hook["actions"].as_array().unwrap() {
                    self.act(a)?;
                }
            }
        }
        Ok(())
    }
    fn act(&mut self, a: &Value) -> Result<Value> {
        let c = self.chunk.clone();
        let x = val(a, "x", 0);
        let y = val(a, "y", 0);
        let mask = val(a, "mask", 0);
        let add = a["add"] != false;
        match a["op"].as_str().unwrap() {
            "get" => return Ok(json!(c.get(self, x, y)?)),
            "flag" => c.flag(self, x, y, mask, add)?,
            "addFlag" => c.add_flag(self, x, y, mask)?,
            "removeFlag" => c.remove_flag(self, x, y, mask)?,
            "clear" => c.clear(self, x, y)?,
            "invalidate" => c.invalidate(self, x, y)?,
            "door" => c.door(
                self,
                x,
                y,
                val(a, "rotation", 0),
                val(a, "shape", 0),
                a["clipped"] == true,
                add,
            )?,
            "solid" => c.solid_object(
                self,
                x,
                y,
                val(a, "sx", 1),
                val(a, "sy", 1),
                a["clipped"] == true,
                add,
            )?,
            "tile" => c.tile_object(self, x, y, add)?,
            "empty" => c.empty_tile(self, x, y)?,
            "solidTile" => c.solid_tile(self, x, y)?,
            "members" => self.chunk.members = a["value"] == true,
            "landscape" => self.chunk.landscape = None,
            "fail" => return Err(Error::Binding("callback".into())),
            name => panic!("unknown {name}"),
        }
        Ok(Value::Null)
    }
    fn snapshot(&self, result: Value, error: Value) -> Value {
        let tables: BTreeMap<_, _> = self
            .tables
            .iter()
            .map(|(k, v)| (k.clone(), v.read().to_vec()))
            .collect();
        let landscape = self.chunk.landscape.as_ref().map(|l| {
            l.read()
                .iter()
                .map(|row| row.as_ref().map(|row| row.read().clone()))
                .collect::<Vec<_>>()
        });
        json!({"flags":tables,"landscape":landscape,"members":self.chunk.members,"baseZ":self.chunk.base_z,"events":self.events,"result":result,"error":error})
    }
}
impl Context for Model {
    fn flags(&mut self, key: i32, p: bool) -> Result<Flags> {
        self.emit("flags", vec![json!(key), json!(p)])?;
        Ok(self
            .tables
            .entry(format!("{}:{key}", if p { "p" } else { "m" }))
            .or_insert_with(|| Shared::new([if p { 0 } else { -1 }; 64]))
            .clone())
    }
    fn border(&mut self, c: Border) -> Result<()> {
        self.emit(
            "border",
            vec![
                json!(c.x),
                json!(c.y),
                json!(c.z),
                json!(c.projectile),
                json!(c.mask),
                json!(c.add),
            ],
        )?;
        let key = c.z.wrapping_shl(22) | ((c.x >> 3).wrapping_shl(11)) | (c.y >> 3);
        let f = self.flags(key, c.projectile)?;
        let i = ((c.x & 7) * 8 + (c.y & 7)) as usize;
        let mut f = f.write();
        if c.add {
            f[i] = f[i].max(0) | c.mask;
        } else if f[i] & c.mask != 0 {
            f[i] = f[i].max(0) & !c.mask;
        }
        Ok(())
    }
}
pub fn evaluate(c: &Value) -> Value {
    let base = c.get("base").cloned().unwrap_or(json!([3200, 3216, 0]));
    let mut s = Model {
        chunk: ChunkFlags::new(
            n(&base[0]),
            n(&base[1]),
            n(&base[2]),
            c["projectile"] == true,
        ),
        tables: BTreeMap::new(),
        events: Vec::new(),
        calls: BTreeMap::new(),
        hooks: c["hooks"].as_array().cloned().unwrap_or_default(),
    };
    let mut out = Vec::new();
    for a in c["actions"].as_array().unwrap() {
        s.events.clear();
        let (v, e) = match s.act(a) {
            Ok(v) => (v, Value::Null),
            Err(e) => (
                Value::Null,
                json!(match e {
                    Error::Null => "Null",
                    Error::Index => "Index",
                    Error::Limit => "Limit",
                    Error::Binding(_) => "Binding",
                }),
            ),
        };
        out.push(s.snapshot(v, e));
    }
    json!(out)
}
