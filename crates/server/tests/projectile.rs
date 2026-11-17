use rs09_server::world::{
    doors::Location,
    pathfinding::{self, projectile},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

const FIXTURE: &str = include_str!("../../../fixtures/projectile/oracle.jsonl");
const PROVENANCE: &str = include_str!("../../../fixtures/projectile/provenance.json");

fn coordinate(value: &Value) -> i32 {
    i32::try_from(value.as_i64().unwrap()).unwrap()
}

fn location(value: &Value) -> Option<Location> {
    value
        .as_array()
        .map(|a| Location::new(coordinate(&a[0]), coordinate(&a[1]), coordinate(&a[2])))
}

struct Field {
    size: i32,
    location: Option<Location>,
}
impl projectile::Footprint for Field {
    fn size(&self) -> pathfinding::Result<i32> {
        Ok(self.size)
    }
    fn location(&self) -> pathfinding::Result<Option<Location>> {
        Ok(self.location.clone())
    }
}

struct Replay<'a> {
    input: &'a Value,
    start: Option<Location>,
    end: Option<Location>,
    flags: BTreeMap<(bool, i32, i32), i32>,
    queries: Vec<Value>,
}
impl Replay<'_> {
    fn int(&self, name: &str, fallback: i32) -> i32 {
        self.input.get(name).map_or(fallback, coordinate)
    }
}
impl projectile::Context for Replay<'_> {
    fn flag(&mut self, z: i32, x: i32, y: i32, projectile: bool) -> pathfinding::Result<i32> {
        self.queries.push(json!([projectile, z, x, y]));
        let n = i32::try_from(self.queries.len()).unwrap();
        if self.int("mutateAt", -1) == n {
            let target = if self.input["mutateTarget"] == "start" {
                &self.start
            } else {
                &self.end
            };
            let [x, y, z] = location(&self.input["replace"]).unwrap().coordinates();
            target.as_ref().unwrap().set(x, y, z);
        }
        if self.int("failAt", -1) == n {
            return Err(pathfinding::Error::Binding("flag read".into()));
        }
        let default = if projectile {
            "defaultProjectile"
        } else {
            "defaultWalking"
        };
        Ok(self
            .flags
            .get(&(projectile, x, y))
            .copied()
            .unwrap_or_else(|| self.int(default, 0)))
    }
}

fn evaluate(i: &Value) -> Value {
    let mut replay = Replay {
        input: i,
        start: location(&i["start"]),
        end: location(&i["end"]),
        flags: i["flags"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|a| {
                (
                    (
                        a[0].as_bool().unwrap(),
                        coordinate(&a[1]),
                        coordinate(&a[2]),
                    ),
                    coordinate(&a[3]),
                )
            })
            .collect(),
        queries: vec![],
    };
    let result = if let Some(kind) = i["kind"].as_str() {
        let source = Field {
            size: replay.int("sourceSize", 1),
            location: replay.start.clone(),
        };
        let victim = Field {
            size: replay.int("victimSize", 1),
            location: replay.end.clone(),
        };
        let victim = victim
            .location
            .is_some()
            .then_some(&victim as &dyn projectile::Footprint);
        let close = kind != "los" && i["close"].as_bool().unwrap_or(false);
        projectile::can_reach(&mut replay, &source, victim, close)
            .map(|reachable| json!({"reachable": reachable}))
    } else {
        let start = replay.start.clone();
        let end = replay.end.clone();
        projectile::find(&mut replay, start.as_ref(), end.as_ref()).map(|p| json!({
            "successful": p.successful, "moveNear": p.move_near,
            "points": p.points.into_iter().map(|s| [s.x, s.y, s.direction.code()]).collect::<Vec<_>>(),
        }))
    };
    let mut output = result.unwrap_or_else(|e| match e {
        pathfinding::Error::Null => json!({"error": "Null"}),
        pathfinding::Error::Binding(_) => json!({"error": "Binding"}),
        other => panic!("unexpected error: {other:?}"),
    });
    output["queries"] = json!(replay.queries);
    output
}

#[test]
fn original_projectile_paths_and_footprint_dispatch_match() {
    for (n, line) in FIXTURE.lines().enumerate() {
        let row: Value = serde_json::from_str(line).unwrap();
        assert_eq!(
            evaluate(&row["input"]),
            row["expected"],
            "projectile case {n}: {}",
            row["input"]
        );
    }
}

#[test]
fn original_sources_and_fixture_are_pinned() {
    let provenance: Value = serde_json::from_str(PROVENANCE).unwrap();
    assert_eq!(provenance["caseCount"], 403);
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
