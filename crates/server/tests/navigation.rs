#[path = "../../../oracle/navigation/native.rs"]
mod native;
use serde_json::Value;
use sha2::{Digest, Sha256};
const CORPUS: &str = include_str!("../../../fixtures/navigation/oracle.jsonl");
fn diff(a: &Value, b: &Value, path: String) -> Option<String> {
    if a == b {
        return None;
    }
    if let (Some(a), Some(b)) = (a.as_object(), b.as_object()) {
        for key in a.keys().chain(b.keys()) {
            if let Some(d) = diff(
                a.get(key).unwrap_or(&Value::Null),
                b.get(key).unwrap_or(&Value::Null),
                format!("{path}/{key}"),
            ) {
                return Some(d);
            }
        }
    }
    if let (Some(a), Some(b)) = (a.as_array(), b.as_array()) {
        if a.len() == b.len() {
            for (i, (a, b)) in a.iter().zip(b).enumerate() {
                if let Some(d) = diff(a, b, format!("{path}/{i}")) {
                    return Some(d);
                }
            }
        }
    }
    Some(format!("{path}:native={a},original={b}"))
}
#[test]
fn joined_original_pulse_dispatch_search_queue_movement_and_arrival() {
    for (i, line) in CORPUS.lines().enumerate() {
        let c: Value = serde_json::from_str(line).unwrap();
        if let Some(d) = diff(&native::evaluate(&c), &c["expected"], String::new()) {
            panic!("navigation case{i}:{d}");
        }
    }
}

#[test]
fn npc_water_search_uses_water_supplier_and_can_walk_two_tiles() {
    use rs09_server::world::{
        doors::Location,
        navigation::{Context, Navigator},
        pathfinding::{
            Result,
            dispatch::{Kind, Supplier, View},
        },
    };

    struct Water {
        clips: Vec<Supplier>,
    }
    impl Context for Water {
        fn node(&self, _: usize) -> Result<View> {
            Ok(View {
                kind: Kind::Npc,
                location: Some(Location::new(3200, 3200, 0)),
                size: 1,
                scenery: None,
            })
        }
        fn npc_supplier(&mut self, _: usize) -> Result<Option<Supplier>> {
            panic!("findWater uses the water supplier directly")
        }
        fn teleport_permitted(&mut self, _: Option<Location>) -> Result<bool> {
            panic!("a plain location target doesn't query teleport permission")
        }
        fn clipping(&mut self, supplier: Supplier, _: i32, _: i32, _: i32) -> Result<i32> {
            self.clips.push(supplier);
            Ok(0)
        }
        fn world_clipping(&mut self, _: i32, _: i32, _: i32) -> Result<i32> {
            panic!("a plain location target doesn't use sized reach")
        }
    }
    let mut context = Water { clips: Vec::new() };
    let path = Navigator::default()
        .find_water(&mut context, 1, Location::new(3202, 3200, 0))
        .unwrap();
    assert!(path.successful);
    assert_eq!(path.points, [[3201, 3200], [3202, 3200]]);
    assert_eq!(context.clips, [Supplier::Water; 2]);
}

#[test]
fn navigation_fixture_sources_and_bytes_are_pinned() {
    let p: Value =
        serde_json::from_str(include_str!("../../../fixtures/navigation/provenance.json")).unwrap();
    assert_eq!(p["caseCount"], CORPUS.lines().count());
    assert_eq!(p["fixtureSha256"], format!("{:x}", Sha256::digest(CORPUS)));
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    for (path, expected) in p["sources"].as_object().unwrap() {
        let path = root.join(path);
        if path.is_file() {
            let data: Vec<_> = std::fs::read(&path)
                .unwrap()
                .into_iter()
                .filter(|b| *b != b'\r')
                .collect();
            assert_eq!(
                format!("{:x}", Sha256::digest(data)),
                expected.as_str().unwrap(),
                "{}",
                path.display()
            );
        }
    }
}
