#[path = "../../../oracle/ground-options/native.rs"]
mod native;
use rs09_server::{
    definitions::item::Shared,
    world::{
        ground_options::Registry, ground_spawn, interaction_options as options, player_walk::State,
    },
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
const CORPUS: &str = include_str!("../../../fixtures/ground-options/oracle.jsonl");

#[test]
fn joined_original_default_tables_item_lookup_cached_definitions_and_failed_prefixes() {
    for (i, line) in CORPUS.lines().enumerate() {
        let row: Value = serde_json::from_str(line).unwrap();
        assert_eq!(
            native::evaluate(&row),
            row["expected"],
            "ground option case{i}: {line}"
        );
    }
}
#[test]
fn ground_options_fixture_and_source_hashes_are_pinned() {
    let p: Value = serde_json::from_str(include_str!(
        "../../../fixtures/ground-options/provenance.json"
    ))
    .unwrap();
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
            let bytes: Vec<_> = std::fs::read(&path)
                .unwrap()
                .into_iter()
                .filter(|b| *b != b'\r')
                .collect();
            assert_eq!(
                format!("{:x}", Sha256::digest(bytes)),
                expected.as_str().unwrap(),
                "{}",
                path.display()
            );
        }
    }
}
#[test]
fn handlers_are_actual_objects_and_foreign_worlds_are_rejected_even_with_one_runner() {
    let mut h = native::Harness::new(&json!({}));
    h.initialize().unwrap();
    let captured = h.captured();
    let selected = options::get(&captured.table, 2).unwrap().unwrap();
    let id = selected.read().handler.unwrap();
    let handler = captured.handler(id).unwrap();
    assert!(handler.same(&h.handlers[0]));
    assert!(
        handler
            .read()
            .value::<rs09_server::world::pickup::Program>()
            .is_some()
    );
    assert!(Registry::default().handler(id).is_err());
    let mut foreign = State::with_runner(h.state.runner());
    assert!(!foreign.identity().same(&h.state.identity()));
    assert!(
        h.registry
            .capture(&mut foreign, &h.spawns, &h.ground)
            .is_err()
    );
    assert!(
        h.registry
            .initialize(&mut foreign, &h.spawns, &h.definitions, &h.ground, |s| Ok(
                s.into()
            ))
            .is_err()
    );
    assert!(
        Registry::default()
            .capture(&mut h.state, &h.spawns, &h.ground)
            .is_err()
    );
}
#[test]
fn old_exposed_arrays_keep_handler_arena_without_retaining_dead_ground_or_actor_state() {
    let mut h = native::Harness::new(&json!({}));
    h.initialize().unwrap();
    let capture = h.captured();
    let node = capture.target.node;
    let array = capture.table.read().options();
    let weak_ground = h.ground.downgrade();
    let weak_handler = h.handlers[0].downgrade();
    drop(capture);
    let mut state = std::mem::take(&mut h.state);
    drop(h);
    state.collect();
    assert!(state.ground(node).is_err());
    assert!(weak_ground.upgrade().is_none());
    assert!(weak_handler.upgrade().is_some());
    drop(array);
    assert!(weak_handler.upgrade().is_none());
}
#[test]
fn exposed_table_retains_exact_spawn_owner_then_releases_after_last_capture() {
    let mut h = native::Harness::new(&json!({}));
    let input = h.ground.read().item.clone();
    let spawn = h
        .spawns
        .construct(
            &mut native::Construction(&h.definitions),
            10,
            &input,
            Some(rs09_server::world::doors::Location::new(3207, 3216, 0)),
        )
        .unwrap();
    let ground = ground_spawn::as_ground(&spawn);
    h.registry
        .initialize(&mut h.state, &h.spawns, &h.definitions, &ground, |s| {
            Ok(s.to_lowercase())
        })
        .unwrap();
    let capture = h
        .registry
        .capture(&mut h.state, &h.spawns, &ground)
        .unwrap();
    let weak = spawn.downgrade();
    drop(spawn);
    drop(ground);
    let node = capture.target.node;
    let table = capture.table.clone();
    drop(capture);
    h.state.collect();
    let view = h.state.ground(node).unwrap();
    assert!(h.spawns.get(&view).is_some());
    assert!(weak.upgrade().is_some());
    drop(view);
    drop(table);
    h.state.collect();
    h.spawns.collect();
    assert!(h.state.ground(node).is_err());
    assert!(weak.upgrade().is_none());
}
#[test]
fn handler_ids_are_stable_for_the_same_object_and_unique_across_registries() {
    let h = native::Harness::new(&json!({}));
    let a = h.registry.bind(h.handlers[0].clone()).unwrap();
    let b = h.registry.bind(h.handlers[0].clone()).unwrap();
    assert_eq!(a, b);
    let other = Registry::default();
    let c = other.bind(h.handlers[0].clone()).unwrap();
    assert_ne!(a, c);
    assert!(h.registry.handler(c).is_err());
    assert!(other.handler(a).is_err());
    assert!(Shared::strong_count(&h.handlers[0]) > 1);
}
