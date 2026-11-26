#[path = "../../../oracle/scene/native.rs"]
mod native;
use serde_json::Value;
use sha2::{Digest, Sha256};
const CORPUS: &str = include_str!("../../../fixtures/scene/oracle.jsonl");
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
    if let (Some(a), Some(b)) = (a.as_array(), b.as_array())
        && a.len() == b.len()
    {
        for (i, (a, b)) in a.iter().zip(b).enumerate() {
            if let Some(d) = diff(a, b, format!("{path}/{i}")) {
                return Some(d);
            }
        }
    }
    Some(format!("{path}: native={a}, original={b}"))
}
#[test]
fn joined_region_door_collision_packets_and_restoration_match_original() {
    for (i, line) in CORPUS.lines().enumerate() {
        let c: Value = serde_json::from_str(line).unwrap();
        if let Some(d) = diff(&native::evaluate(&c), &c["expected"], String::new()) {
            panic!("joined scene case {i}: {d}");
        }
    }
}
#[test]
fn scene_sources_and_fixture_bytes_are_pinned() {
    let p: Value =
        serde_json::from_str(include_str!("../../../fixtures/scene/provenance.json")).unwrap();
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

#[test]
fn queued_and_retained_restore_pulses_own_their_captures_without_cycles() {
    use rs09_server::world::{doors::Location, scenery};
    let input = serde_json::json!({"land":"b03d81d2010000"});
    let mut scene = native::create(&input);
    let mut services = native::Bindings::default();
    let closed = scene
        .context(&mut services)
        .object(Location::new(3207, 3217, 0), -1, 0)
        .unwrap()
        .unwrap();
    scene.context(&mut services).door(0, &closed).unwrap();
    let opened = scene
        .context(&mut services)
        .object(Location::new(3207, 3218, 0), -1, 0)
        .unwrap()
        .unwrap();
    let weak = opened.downgrade();
    let pulse = scene.restore_pulse(&opened).unwrap().unwrap();
    let weak_pulse = pulse.downgrade();
    // Removal leaves the original scheduled restore body intact. Retaining the
    // public pulse must keep its scenery capture alive after the queue consumes it.
    scenery::remove(&mut scene.context(&mut services), Some(&opened)).unwrap();
    scene.clear_updates();
    drop(opened);
    for _ in 0..500 {
        scene.context(&mut services).tick().unwrap();
    }
    assert!(scene.runner.current().is_empty());
    assert!(weak.upgrade().is_some());
    drop(pulse);
    assert!(weak.upgrade().is_none());
    assert!(weak_pulse.upgrade().is_none());
    scene.context(&mut services).tick().unwrap();
    assert_eq!(scene.retained_task_count(), 0);
    assert!(scene.diagnostics.is_empty());
}

#[test]
fn a_queued_restore_keeps_removed_scenery_until_execution_then_releases_it() {
    use rs09_server::world::{doors::Location, scenery};
    let mut scene = native::create(&serde_json::json!({"land":"b03d81d2010000"}));
    let mut services = native::Bindings::default();
    let closed = scene
        .context(&mut services)
        .object(Location::new(3207, 3217, 0), -1, 0)
        .unwrap()
        .unwrap();
    scene.context(&mut services).door(0, &closed).unwrap();
    let opened = scene
        .context(&mut services)
        .object(Location::new(3207, 3218, 0), -1, 0)
        .unwrap()
        .unwrap();
    let weak = opened.downgrade();
    scenery::remove(&mut scene.context(&mut services), Some(&opened)).unwrap();
    scene.clear_updates();
    drop(opened);
    assert!(weak.upgrade().is_some());
    for _ in 0..500 {
        scene.context(&mut services).tick().unwrap();
    }
    assert!(weak.upgrade().is_none());
    assert!(scene.runner.current().is_empty());
    assert_eq!(scene.retained_task_count(), 0);
    assert!(scene.diagnostics.is_empty());
}

#[test]
fn absent_map_regions_return_unknown_collision_for_movement_and_projectiles() {
    use rs09_server::world::doors::Location;
    let mut scene = native::create(&serde_json::json!({"land":"00000000","missing":true}));
    let mut services = native::Bindings::default();
    for projectile in [false, true] {
        assert_eq!(
            scene
                .context(&mut services)
                .clipping(Location::new(3207, 3217, 0), projectile)
                .unwrap(),
            -1
        );
    }
    assert!(scene.flags.is_empty());
}

#[test]
fn native_navigation_routes_follow_door_collision_and_500_tick_restore() {
    use rs09_server::world::{
        doors::Location,
        movement::Target,
        navigation::{self, Navigator},
        pathfinding::{self, dispatch},
    };
    let mut scene = native::create(&serde_json::json!({"land":"b03d81d2010000"}));
    let mut services = native::Bindings::default();
    struct Bound<'a, C>(&'a mut C);
    impl<C: pathfinding::Context> navigation::Context for Bound<'_, C> {
        fn node(&self, id: usize) -> pathfinding::Result<dispatch::View> {
            assert_eq!(id, usize::MAX);
            Ok(dispatch::View {
                kind: dispatch::Kind::Entity,
                location: Some(Location::new(3207, 3217, 0)),
                size: 1,
                scenery: None,
            })
        }
        fn npc_supplier(&mut self, _: usize) -> pathfinding::Result<Option<dispatch::Supplier>> {
            panic!("non-NPC route must not consult behavior")
        }
        fn teleport_permitted(&mut self, _: Option<Location>) -> pathfinding::Result<bool> {
            panic!("location target must not consult ground-item permission")
        }
        fn clipping(
            &mut self,
            supplier: dispatch::Supplier,
            z: i32,
            x: i32,
            y: i32,
        ) -> pathfinding::Result<i32> {
            assert_eq!(supplier, dispatch::Supplier::World);
            self.0.clipping(z, x, y)
        }
        fn world_clipping(&mut self, z: i32, x: i32, y: i32) -> pathfinding::Result<i32> {
            self.0.world_clipping(z, x, y)
        }
    }
    let target = Target::Location(Location::new(3207, 3218, 0));
    let mut finder = Navigator::default();
    let closed = finder
        .find(
            &mut Bound(&mut scene.context(&mut services)),
            Some(usize::MAX),
            target.clone(),
            Some(0),
            true,
        )
        .unwrap();
    let door = scene
        .context(&mut services)
        .object(Location::new(3207, 3217, 0), -1, 0)
        .unwrap()
        .unwrap();
    scene.context(&mut services).door(0, &door).unwrap();
    let opened = finder
        .find(
            &mut Bound(&mut scene.context(&mut services)),
            Some(usize::MAX),
            target.clone(),
            Some(0),
            true,
        )
        .unwrap();
    assert!(opened.successful);
    assert_eq!(opened.points, vec![[3207, 3218], [3207, 3218]]);
    assert!(closed.points.len() > opened.points.len());
    for _ in 0..500 {
        scene.context(&mut services).tick().unwrap();
    }
    let restored = finder
        .find(
            &mut Bound(&mut scene.context(&mut services)),
            Some(usize::MAX),
            target,
            Some(0),
            true,
        )
        .unwrap();
    assert_eq!(restored, closed);
    assert!(scene.diagnostics.is_empty());
}
