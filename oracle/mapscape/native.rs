//! Canonical full-state observations for the native terrain/floor loops.
use rs09_protocol::buffer::Reader;
use rs09_server::{
    definitions::item::Shared,
    world::{
        chunk::{HandleChunk, RegionChunk},
        clipping::{self, Border, Flags},
        doors::Location,
        landscape::{Chunks, Mapscape},
        mapscape::{self, Error},
    },
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
fn v(a: &Value, k: &str, d: i32) -> i32 {
    a[k].as_i64().map_or(d, |n| n as i32)
}
fn table(base: i32) -> Chunks {
    Shared::new(std::array::from_fn(|x| {
        std::array::from_fn(|y| {
            std::array::from_fn(|z| {
                Some(Shared::new(RegionChunk::new(
                    Location::new(base + x as i32 * 8, base + y as i32 * 8, z as i32),
                    0,
                )))
            })
        })
    }))
}

struct Model {
    input: Value,
    source: Chunks,
    region: Option<Chunks>,
    map: Mapscape,
    flags: BTreeMap<i32, Flags>,
    flag_calls: i32,
    chunk_calls: i32,
    position: usize,
}
impl clipping::Context for Model {
    fn flags(&mut self, key: i32, _: bool) -> clipping::Result<Flags> {
        self.flag_calls += 1;
        if self.flag_calls == v(&self.input, "failAt", -1) {
            return Err(clipping::Error::Binding("failure".into()));
        }
        let flags = self
            .flags
            .entry(key)
            .or_insert_with(|| Shared::new([-1; 64]))
            .clone();
        if let Some(rows) = self.input["mutations"].as_array() {
            for a in rows {
                if self.flag_calls == v(a, "at", 0) {
                    self.map.write()[v(a, "z", 0) as usize][v(a, "x", 0) as usize]
                        [v(a, "y", 0) as usize] = v(a, "value", 0) as i8;
                }
            }
        }
        if self.flag_calls == v(&self.input, "nullRegionAt", -1) {
            self.region = None;
        }
        Ok(flags)
    }
    fn border(&mut self, _: Border) -> clipping::Result<()> {
        unreachable!()
    }
}
impl mapscape::Context for Model {
    fn region_chunks(&mut self) -> mapscape::Result<Option<Chunks>> {
        self.chunk_calls += 1;
        Ok(self.region.clone())
    }
}
fn int(out: &mut Vec<u8>, n: i32) {
    out.extend(n.to_be_bytes());
}
fn table_bytes(out: &mut Vec<u8>, table: Option<&Chunks>) {
    out.push(u8::from(table.is_some()));
    let Some(table) = table else {
        return;
    };
    for x in 0..8 {
        for y in 0..8 {
            for z in 0..4 {
                let chunk: Option<HandleChunk> = table.read()[x][y][z].clone();
                out.push(u8::from(chunk.is_some()));
                let Some(chunk) = chunk else {
                    continue;
                };
                let c = chunk.read();
                for n in c.current_base.coordinates() {
                    int(out, n);
                }
                let land = c.flags.landscape.clone();
                int(out, land.as_ref().map_or(-1, |a| a.read().len() as i32));
                if let Some(land) = land {
                    for row in land.read().iter() {
                        int(out, row.as_ref().map_or(-1, |r| r.read().len() as i32));
                        if let Some(row) = row {
                            for value in row.read().iter() {
                                out.push(u8::from(*value));
                            }
                        }
                    }
                }
            }
        }
    }
}
impl Model {
    fn snapshot(&self) -> Value {
        let mut out = vec![];
        for plane in self.map.read().iter() {
            for row in plane {
                out.extend(row.iter().map(|n| *n as u8));
            }
        }
        table_bytes(&mut out, Some(&self.source));
        table_bytes(&mut out, self.region.as_ref());
        int(&mut out, self.flags.len() as i32);
        for (k, f) in &self.flags {
            int(&mut out, *k);
            for n in f.read().iter() {
                int(&mut out, *n);
            }
        }
        json!({"hash":format!("{:x}",Sha256::digest(out)),"position":self.position,"flagCalls":self.flag_calls,"chunkCalls":self.chunk_calls})
    }
    fn action(&mut self, a: &Value) -> mapscape::Result<()> {
        if a["op"] == "clip" {
            let source = self.source.clone();
            let map = self.map.clone();
            mapscape::clip(self, &source, &map)
        } else {
            let data: Vec<_> = a["data"]
                .as_str()
                .unwrap()
                .as_bytes()
                .chunks_exact(2)
                .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
                .collect();
            let mut r = Reader::new(&data);
            let map = self.map.clone();
            let result = mapscape::parse(self, &map, &mut r);
            self.position = r.position();
            result
        }
    }
}
fn class(e: &Error) -> &str {
    match e {
        Error::Null => "Null",
        Error::Decode(_) => "Decode",
        Error::Clipping(clipping::Error::Null) => "Null",
        Error::Clipping(clipping::Error::Index) => "Index",
        _ => "Binding",
    }
}
pub fn evaluate(input: &Value) -> Value {
    let source = table(3200);
    let region = if input["nullRegion"] == true {
        None
    } else if input["separate"] == true {
        Some(table(3264))
    } else {
        Some(source.clone())
    };
    if let Some(nulls) = input["nulls"].as_array() {
        for p in nulls {
            source.write()[p[0].as_u64().unwrap() as usize][p[1].as_u64().unwrap() as usize]
                [p[2].as_u64().unwrap() as usize] = None;
        }
    }
    if input["nullLandscape"] == true {
        source.read()[0][0][0]
            .as_ref()
            .unwrap()
            .write()
            .flags
            .landscape = None;
    }
    if input["aliasLandscape"] == true {
        let a = source.read()[0][0][0]
            .as_ref()
            .unwrap()
            .read()
            .flags
            .landscape
            .clone();
        source.read()[7][7][3]
            .as_ref()
            .unwrap()
            .write()
            .flags
            .landscape = a;
    }
    let map: Mapscape = Shared::new(Box::new([[[0; 64]; 64]; 4]));
    if let Some(rows) = input["map"].as_array() {
        for a in rows {
            map.write()[a[0].as_u64().unwrap() as usize][a[1].as_u64().unwrap() as usize]
                [a[2].as_u64().unwrap() as usize] = a[3].as_i64().unwrap() as i8;
        }
    }
    let mut m = Model {
        input: input.clone(),
        source,
        region,
        map,
        flags: BTreeMap::new(),
        flag_calls: 0,
        chunk_calls: 0,
        position: 0,
    };
    if input.get("initialFlag").is_some() {
        m.flags.insert(
            (3200 >> 3 << 11) | (3200 >> 3),
            Shared::new([v(input, "initialFlag", 0); 64]),
        );
    }
    let mut out = vec![];
    for a in input["actions"].as_array().unwrap() {
        let error = m.action(a).err().map(|e| class(&e).to_owned());
        let mut s = m.snapshot();
        s["error"] = json!(error);
        out.push(s);
    }
    json!(out)
}
