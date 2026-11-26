//! Replays api-utils-oracle: the original WeightedTable, PlayerCamera and
//! CameraUtils through `world::api_utils`, with the two generators seeded as
//! the case seeds java.util.Random.
use rs09_game_core::random::JavaRandom;
use rs09_server::world::api_utils::{
    self as au, CameraContext, CameraHost, CameraShakeType, CameraType, Draws, Error, PlayerCamera,
    WeightedTable,
};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

const FIXTURE: &str = include_str!("../../../fixtures/api-utils/oracle.jsonl");
const PROVENANCE: &str = include_str!("../../../fixtures/api-utils/provenance.json");

fn int(v: &Value) -> i32 {
    i32::try_from(v.as_i64().unwrap()).unwrap()
}

fn weight(v: &Value) -> f64 {
    match v.as_str() {
        Some("NaN") => f64::NAN,
        Some("Infinity") => f64::INFINITY,
        Some("-Infinity") => f64::NEG_INFINITY,
        Some("-0.0") => -0.0,
        Some(other) => panic!("weight {other}"),
        None => v.as_f64().unwrap(),
    }
}

fn bits(d: f64) -> Value {
    if d.is_nan() {
        json!("NaN")
    } else {
        json!(format!("{:016x}", d.to_bits()))
    }
}

/// A table element of a case: text or an int (null is `None`).
fn element(v: &Value) -> Option<Value> {
    (!v.is_null()).then(|| v.clone())
}

struct World {
    shuffle: JavaRandom,
    random: JavaRandom,
    events: Vec<Value>,
    calls: i64,
    fail_at: i64,
}

impl World {
    fn record(&mut self, row: Value) -> au::Result<()> {
        self.events.push(row);
        self.calls += 1;
        if self.calls == self.fail_at {
            return Err(Error::Binding("injected".into()));
        }
        Ok(())
    }
}

impl Draws for World {
    fn shuffle_int(&mut self, bound: i32) -> au::Result<i32> {
        self.record(json!(["Collections.shuffle.nextInt", bound]))?;
        Ok(self.shuffle.next_bound(bound).expect("a positive bound"))
    }
    fn next_double(&mut self) -> au::Result<f64> {
        self.record(json!(["RandomFunction.RANDOM.nextDouble"]))?;
        Ok(self.random.next_double())
    }
}

impl CameraHost for World {
    type Player = ();
    fn send_camera(&mut self, _: &(), c: &CameraContext) -> au::Result<()> {
        self.record(json!([
            "PacketRepository.send",
            "CameraViewPacket",
            "player",
            c.kind.name(),
            c.kind.opcode(),
            c.x,
            c.y,
            c.height,
            c.speed,
            c.zoom_speed
        ]))
    }
}

/// An operation's failure: the library's, or the oracle's own null camera.
enum Fail {
    Lib(Error),
    Null,
}
impl From<Error> for Fail {
    fn from(e: Error) -> Self {
        Self::Lib(e)
    }
}

fn op(
    w: &mut World,
    table: &mut WeightedTable<Value>,
    camera: &mut Option<PlayerCamera<()>>,
    o: &[Value],
) -> Result<Value, Fail> {
    let n = |i: usize| int(&o[i]);
    let name = o[0].as_str().unwrap();
    Ok(match name {
        "add" => json!(table.add(element(&o[1]), weight(&o[2]))),
        "addRaw" => {
            table.entries.push((element(&o[1]), weight(&o[2])));
            json!(true)
        }
        "remove" => json!(table.remove(element(&o[1]).as_ref())),
        "removeAt" => {
            let (e, weight) = table.remove_at(n(1))?;
            json!([e, bits(weight)])
        }
        "clear" => {
            table.entries.clear();
            Value::Null
        }
        "roll" => json!(table.roll(w)?),
        "create" => {
            let entries = o[1]
                .as_array()
                .unwrap()
                .iter()
                .map(|e| (element(&e[0]), weight(&e[1])));
            *table = WeightedTable::create(entries);
            Value::Null
        }
        "total" => {
            table.total_weight = weight(&o[1]);
            Value::Null
        }
        "camera" => {
            *camera = Some(PlayerCamera::new((o[1] == true).then_some(())));
            Value::Null
        }
        "shakeTypes" => json!(
            CameraShakeType::ALL
                .iter()
                .map(|t| json!([t.name(), t.ordinal()]))
                .collect::<Vec<_>>()
        ),
        "cameraTypes" => json!(
            CameraType::ALL
                .iter()
                .map(|t| json!([t.name(), t.opcode()]))
                .collect::<Vec<_>>()
        ),
        _ => {
            let camera = camera.as_mut().ok_or(Fail::Null)?;
            match name {
                "setPosition" => camera.set_position(w, n(1), n(2), n(3))?,
                "rotateTo" => camera.rotate_to(w, n(1), n(2), n(3), n(4))?,
                "rotateBy" => camera.rotate_by(w, n(1), n(2), n(3), n(4))?,
                "panTo" => camera.pan_to(w, n(1), n(2), n(3), n(4))?,
                "shake" => camera.shake(w, n(1), n(2), n(3), n(4), n(5))?,
                "reset" => camera.reset(w)?,
                "forget" => camera.ctx = None,
                other => panic!("operation {other}"),
            }
            Value::Null
        }
    })
}

fn replay(row: &Value) -> Value {
    let mut w = World {
        shuffle: JavaRandom::new(row["shuffleSeed"].as_i64().unwrap()),
        random: JavaRandom::new(row["randomSeed"].as_i64().unwrap()),
        events: vec![],
        calls: 0,
        fail_at: row.get("failAt").and_then(Value::as_i64).unwrap_or(-1),
    };
    let mut table = WeightedTable::new();
    let mut camera = None;
    let mut steps = vec![];
    for o in row["ops"].as_array().unwrap() {
        let before = w.events.len();
        let mut step = Map::new();
        match op(&mut w, &mut table, &mut camera, o.as_array().unwrap()) {
            Ok(v) => {
                step.insert("result".into(), v);
            }
            Err(f) => {
                step.insert(
                    "error".into(),
                    json!(match f {
                        Fail::Lib(Error::Index) => "Index",
                        Fail::Lib(Error::Binding(_)) => "Binding",
                        Fail::Null => "Null",
                    }),
                );
            }
        }
        step.insert("events".into(), json!(w.events[before..]));
        let entries: Vec<_> = table
            .entries
            .iter()
            .map(|(e, weight)| json!([e, bits(*weight)]))
            .collect();
        step.insert("table".into(), json!(entries));
        step.insert("total".into(), bits(table.total_weight));
        let context = camera
            .as_ref()
            .and_then(|c| c.ctx)
            .map(|c| json!([c.kind.name(), c.x, c.y, c.height, c.speed, c.zoom_speed]));
        step.insert("camera".into(), json!(context));
        steps.push(Value::Object(step));
    }
    json!({"steps": steps, "calls": w.calls})
}

#[test]
fn original_weighted_table_and_player_camera_match() {
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
    assert_eq!(count, 395);
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
