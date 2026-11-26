//! Replays projectiles-oracle: the original Projectile/ProjectileUpdateFlag
//! cases through `world::projectiles` over a recording host.
use rs09_server::world::{
    doors::Location,
    projectiles::{self as pr, Error, Host, Kind, Projectile},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const FIXTURE: &str = include_str!("../../../fixtures/projectiles/oracle.jsonl");
const PROVENANCE: &str = include_str!("../../../fixtures/projectiles/provenance.json");

fn int(v: &Value) -> i32 {
    i32::try_from(v.as_i64().unwrap()).unwrap()
}

fn loc_value(l: Option<&Location>) -> Value {
    l.map_or(Value::Null, |l| json!(l.coordinates()))
}

struct World {
    kinds: Vec<Kind>,
    sizes: Vec<i32>,
    locations: Vec<Option<Location>>,
    indices: Vec<i32>,
    slots: Vec<Projectile<usize>>,
    flags: Vec<Projectile<usize>>,
    events: Vec<Value>,
    calls: i64,
    fail_at: i64,
}

impl World {
    fn record(&mut self, row: Value) -> pr::Result<()> {
        self.events.push(row);
        self.calls += 1;
        if self.calls == self.fail_at {
            return Err(Error::Binding("injected".into()));
        }
        Ok(())
    }
    fn label(&self, p: &Projectile<usize>) -> Value {
        let i = self
            .slots
            .iter()
            .position(|s| s.same(p))
            .expect("a known projectile");
        json!(format!("p{i}"))
    }
    fn keep(&mut self, p: Projectile<usize>) -> Value {
        if let Some(i) = self.slots.iter().position(|s| s.same(&p)) {
            return json!(format!("p{i}"));
        }
        self.slots.push(p);
        json!(format!("p{}", self.slots.len() - 1))
    }
    fn entity(v: &Value) -> Option<usize> {
        v.as_i64().and_then(|i| usize::try_from(i).ok())
    }
    fn place(&self, v: &Value) -> Result<Option<Location>, Fail> {
        let Some(a) = v.as_array() else {
            return Ok(None);
        };
        Ok(match a[0].as_str() {
            Some("ent") => self.locations[a[1].as_u64().unwrap() as usize].clone(),
            Some("src") => slot(&self.slots, int(&a[1]))?
                .read()
                .source_location
                .clone(),
            Some("end") => slot(&self.slots, int(&a[1]))?.read().end_location.clone(),
            _ => Some(Location::new(int(&a[0]), int(&a[1]), int(&a[2]))),
        })
    }
    fn state(&self, p: &Projectile<usize>) -> Value {
        let f = p.read().clone();
        let e = |x: Option<usize>| x.map_or(Value::Null, |i| json!(format!("e{i}")));
        json!({
            "source": e(f.source), "sourceLocation": loc_value(f.source_location.as_ref()),
            "victim": e(f.victim), "projectileId": f.projectile_id, "startHeight": f.start_height,
            "endHeight": f.end_height, "startDelay": f.start_delay, "speed": f.speed, "angle": f.angle,
            "distance": f.distance, "endLocation": loc_value(f.end_location.as_ref()),
            "locationBased": f.is_location_based(),
        })
    }
}

impl Host for World {
    type Entity = usize;
    type Chunk = Option<Location>;
    fn kind(&self, e: &usize) -> Kind {
        self.kinds[*e]
    }
    fn size_field(&self, e: &usize) -> i32 {
        self.sizes[*e]
    }
    fn location_field(&self, e: &usize) -> Option<Location> {
        self.locations[*e].clone()
    }
    fn size(&mut self, e: &usize) -> pr::Result<i32> {
        self.record(json!(["Node.size", format!("e{e}")]))?;
        Ok(self.sizes[*e])
    }
    fn location(&mut self, e: &usize) -> pr::Result<Option<Location>> {
        self.record(json!(["Node.getLocation", format!("e{e}")]))?;
        Ok(self.locations[*e].clone())
    }
    fn index(&mut self, e: &usize) -> pr::Result<i32> {
        self.record(json!(["Node.getIndex", format!("e{e}")]))?;
        Ok(self.indices[*e])
    }
    fn region_chunk(&mut self, location: Option<&Location>) -> pr::Result<Option<Location>> {
        self.record(json!(["RegionManager.getRegionChunk", loc_value(location)]))?;
        Ok(location.cloned())
    }
    fn flag(&mut self, chunk: Option<Location>, p: Projectile<usize>) -> pr::Result<()> {
        let label = self.label(&p);
        self.record(json!([
            "RegionChunk.flag",
            loc_value(chunk.as_ref()),
            label
        ]))?;
        self.flags.push(p);
        Ok(())
    }
}

/// An operation's failure: the library's, or the oracle's own list lookups.
enum Fail {
    Lib(Error),
    Index,
}
impl From<Error> for Fail {
    fn from(e: Error) -> Self {
        Self::Lib(e)
    }
}

fn slot<T: Clone>(v: &[T], i: i32) -> Result<T, Fail> {
    usize::try_from(i)
        .ok()
        .and_then(|i| v.get(i))
        .cloned()
        .ok_or(Fail::Index)
}

fn op(w: &mut World, o: &[Value], bytes: &mut Option<Vec<u8>>) -> Result<Value, Fail> {
    let ent = World::entity;
    let n = |i: usize| int(&o[i]);
    Ok(match o[0].as_str().unwrap() {
        "create" => {
            let (s, v) = (ent(&o[2]), ent(&o[3]));
            let (s, v) = (s.as_ref(), v.as_ref());
            let id = n(4);
            let p = match n(1) {
                3 => pr::create(w, s, v, id)?,
                5 => pr::create_with_heights(w, s, v, id, n(5), n(6))?,
                6 => pr::create_with_delay(w, s, v, id, n(5), n(6), n(7))?,
                7 => pr::create_with_speed(w, s, v, id, n(5), n(6), n(7), n(8))?,
                8 => pr::create_with_angle(w, s, v, id, n(5), n(6), n(7), n(8), n(9))?,
                9 => pr::create_full(w, s, v, id, n(5), n(6), n(7), n(8), n(9), n(10))?,
                other => panic!("arity {other}"),
            };
            w.keep(p)
        }
        "createLoc" => {
            let (a, b) = (w.place(&o[1])?, w.place(&o[2])?);
            let p = pr::create_at(a, b, n(3), n(4), n(5), n(6), n(7), n(8), n(9));
            w.keep(p)
        }
        kind @ ("magic" | "ranged") => {
            let (s, v) = (ent(&o[1]), ent(&o[2]));
            let f = if kind == "magic" {
                pr::magic::<World>
            } else {
                pr::ranged::<World>
            };
            let p = f(w, s.as_ref(), v.as_ref(), n(3), n(4), n(5), n(6), n(7))?;
            w.keep(p)
        }
        "empty" => w.keep(pr::empty()),
        "getSpeed" => {
            let l = w.place(&o[2])?;
            json!(pr::get_speed(w, ent(&o[1]).as_ref(), l.as_ref())?)
        }
        "getLocation" => loc_value(pr::get_location(w, ent(&o[1]).as_ref())?.as_ref()),
        "transform" => {
            let p = slot(&w.slots, n(1))?;
            let r = pr::transform(w, &p, ent(&o[2]).as_ref(), ent(&o[3]).as_ref())?;
            w.keep(r)
        }
        "transform5" => {
            let p = slot(&w.slots, n(1))?;
            let npc = o[4].as_bool().unwrap();
            let r = pr::transform_with(
                w,
                &p,
                ent(&o[2]).as_ref(),
                ent(&o[3]).as_ref(),
                npc,
                n(5),
                n(6),
            )?;
            w.keep(r)
        }
        "transformLoc" => {
            let p = slot(&w.slots, n(1))?;
            let l = w.place(&o[3])?;
            let r = pr::transform_to(w, &p, ent(&o[2]).as_ref(), l)?;
            w.keep(r)
        }
        "transformLoc5" => {
            let p = slot(&w.slots, n(1))?;
            let l = w.place(&o[3])?;
            let npc = o[4].as_bool().unwrap();
            let r = pr::transform_to_with(w, &p, ent(&o[2]).as_ref(), l, npc, n(5), n(6))?;
            w.keep(r)
        }
        "copy" => {
            let p = slot(&w.slots, n(1))?;
            let r = pr::copy(
                w,
                &p,
                ent(&o[2]).as_ref(),
                ent(&o[3]).as_ref(),
                o[4].as_f64().unwrap(),
            )?;
            w.keep(r)
        }
        "send" | "sendStatic" => {
            let p = slot(&w.slots, n(1))?;
            pr::send(w, &p)?;
            Value::Null
        }
        "sendTo" => {
            let p = slot(&w.slots, n(3))?;
            pr::send_to(w, ent(&o[1]).as_ref(), ent(&o[2]).as_ref(), &p)?;
            Value::Null
        }
        "sendAt" => {
            let p = slot(&w.slots, n(3))?;
            let l = w.place(&o[2])?;
            pr::send_at(w, ent(&o[1]).as_ref(), l, &p)?;
            Value::Null
        }
        "set" => {
            let p = slot(&w.slots, n(1))?;
            let v = &o[3];
            let location = w.place(v)?;
            let mut f = p.write();
            match o[2].as_str().unwrap() {
                "source" => f.source = ent(v),
                "sourceLocation" => f.source_location = location,
                "victim" => f.victim = ent(v),
                "projectileId" => f.projectile_id = int(v),
                "startHeight" => f.start_height = int(v),
                "endHeight" => f.end_height = int(v),
                "startDelay" => f.start_delay = int(v),
                "speed" => f.speed = int(v),
                "angle" => f.angle = int(v),
                "distance" => f.distance = int(v),
                "endLocation" => f.end_location = location,
                other => panic!("field {other}"),
            }
            Value::Null
        }
        "write" => {
            let p = slot(&w.flags, n(1))?;
            let mut out = Vec::new();
            let r = pr::write(w, &p, &mut out);
            *bytes = Some(out.clone());
            r?;
            json!({"bytes": out, "data": pr::DATA, "ordinal": pr::ORDINAL})
        }
        "compare" => json!(pr::compare_to(pr::ORDINAL, pr::ORDINAL)),
        "equals" => json!(pr::equals(pr::DATA, pr::ORDINAL, pr::DATA, pr::ORDINAL)),
        "move" => {
            let l = w.place(&o[2])?;
            w.locations[n(1) as usize] = l;
            Value::Null
        }
        "setLoc" => {
            let l = w.locations[n(1) as usize].clone().ok_or(Error::Null)?;
            l.set(n(2), n(3), n(4));
            Value::Null
        }
        "index" => {
            w.indices[n(1) as usize] = n(2);
            Value::Null
        }
        "size" => {
            w.sizes[n(1) as usize] = n(2);
            Value::Null
        }
        other => panic!("operation {other}"),
    })
}

fn replay(row: &Value) -> Value {
    let entities = row["entities"].as_array().unwrap();
    let mut w = World {
        kinds: entities
            .iter()
            .map(|e| match e["kind"].as_str().unwrap() {
                "player" => Kind::Player,
                "npc" => Kind::Npc,
                _ => Kind::Other,
            })
            .collect(),
        sizes: entities.iter().map(|e| int(&e["size"])).collect(),
        locations: vec![],
        indices: entities.iter().map(|e| int(&e["index"])).collect(),
        slots: vec![],
        flags: vec![],
        events: vec![],
        calls: 0,
        fail_at: row.get("failAt").and_then(Value::as_i64).unwrap_or(-1),
    };
    w.locations = entities
        .iter()
        .map(|e| w.place(&e["loc"]).ok().flatten())
        .collect();
    let mut steps = vec![];
    for o in row["ops"].as_array().unwrap() {
        let before = w.events.len();
        let mut bytes = None;
        let mut step = serde_json::Map::new();
        match op(&mut w, o.as_array().unwrap(), &mut bytes) {
            Ok(v) => {
                step.insert("result".into(), v);
            }
            Err(e) => {
                step.insert(
                    "error".into(),
                    json!(match e {
                        Fail::Lib(Error::Null) => "Null",
                        Fail::Lib(Error::Binding(_)) => "Binding",
                        Fail::Index => "Index",
                    }),
                );
                if let Some(b) = bytes {
                    step.insert("bytes".into(), json!(b));
                }
            }
        }
        step.insert("events".into(), json!(w.events[before..]));
        let slots: Vec<_> = w.slots.iter().map(|p| w.state(p)).collect();
        step.insert("projectiles".into(), json!(slots));
        let flags: Vec<_> = w.flags.iter().map(|p| w.label(p)).collect();
        step.insert("flags".into(), json!(flags));
        steps.push(Value::Object(step));
    }
    json!({"steps": steps, "calls": w.calls})
}

#[test]
fn original_projectile_objects_and_flag_bytes_match() {
    let mut count = 0;
    for (n, line) in FIXTURE.lines().enumerate() {
        let row: Value = serde_json::from_str(line).unwrap();
        let got = replay(&row);
        if got != row["expected"] {
            let want = row["expected"]["steps"].as_array().unwrap();
            let have = got["steps"].as_array().unwrap();
            for (i, (a, b)) in want.iter().zip(have).enumerate() {
                assert_eq!(b, a, "case {n} {} step {i} {}", row["name"], row["ops"][i]);
            }
            assert_eq!(got, row["expected"], "case {n} {}", row["name"]);
        }
        count += 1;
    }
    assert_eq!(count, 787);
}

#[test]
fn original_sources_and_fixture_are_pinned() {
    let provenance: Value = serde_json::from_str(PROVENANCE).unwrap();
    assert_eq!(provenance["caseCount"], FIXTURE.lines().count());
    assert_eq!(
        provenance["fixtureSha256"],
        format!("{:x}", Sha256::digest(FIXTURE))
    );
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (name, hash) in provenance["sources"].as_object().unwrap() {
        let path = root.join(name);
        if path.is_file() {
            let bytes: Vec<_> = std::fs::read(path)
                .unwrap()
                .into_iter()
                .filter(|b| *b != b'\r')
                .collect();
            assert_eq!(
                format!("{:x}", Sha256::digest(bytes)),
                hash.as_str().unwrap(),
                "{name}"
            );
        }
    }
}
