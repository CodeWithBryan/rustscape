#[path = "../../../oracle/player-walk/native.rs"]
mod native;
use native::ground_target::Constructor;
use rs09_server::{
    definitions::item::Shared,
    inventory::item,
    world::{
        doors::Location,
        ground_item as ground, ground_spawn as spawn,
        interaction::{Approach, ApproachKind, OptionData},
        player_walk::State,
    },
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
const CORPUS: &str = include_str!("../../../fixtures/ground-target/oracle.jsonl");
fn difference(a: &Value, b: &Value, path: String) -> Option<String> {
    if a == b {
        return None;
    }
    if let (Some(a), Some(b)) = (a.as_object(), b.as_object()) {
        for key in a.keys().chain(b.keys()) {
            if let Some(d) = difference(
                a.get(key).unwrap_or(&Value::Null),
                b.get(key).unwrap_or(&Value::Null),
                format!("{path}/{key}"),
            ) {
                return Some(d);
            }
        }
    }
    if let (Some(a), Some(b)) = (a.as_array(), b.as_array()) {
        for (i, (a, b)) in a.iter().zip(b).enumerate() {
            if let Some(d) = difference(a, b, format!("{path}/{i}")) {
                return Some(d);
            }
        }
        return Some(format!("{path}: lengths {} != {}", a.len(), b.len()));
    }
    Some(format!("{path}: native={a}, original={b}"))
}
#[test]
fn original_ground_geometry_activity_path_dispatch_approaches_and_mutations() {
    for (i, line) in CORPUS.lines().enumerate() {
        let row: Value = serde_json::from_str(line).unwrap();
        let actual = native::evaluate(&row);
        if let Some(d) = difference(&actual, &row["expected"], String::new()) {
            panic!(
                "ground target case{i}: {d}; first tick events {}",
                actual[1]["events"]
            );
        }
    }
}
#[test]
fn ground_target_fixture_sources_and_bytes_are_pinned() {
    let p: Value = serde_json::from_str(include_str!(
        "../../../fixtures/ground-target/provenance.json"
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
fn input(ctx: &Constructor) -> item::Handle {
    Shared::new(item::Item::new(1, 1, 1000, &ctx.definitions))
}
#[test]
fn ground_identity_duplicate_tiles_and_spawn_registry_are_canonical() {
    let mut state = State::default();
    let registry = spawn::Registry::default();
    let mut ctx = Constructor::default();
    let i = input(&ctx);
    let item =
        ground::construct(&mut ctx, &i, Some(Location::new(3211, 3217, 0)), 200, None).unwrap();
    let first = state.ground_target(item.clone(), &registry).unwrap();
    assert!(first == state.ground_target(item.clone(), &registry).unwrap());
    let other = ground::construct(&mut ctx, &i, item.read().location.clone(), 200, None).unwrap();
    let second = state.ground_target(other, &registry).unwrap();
    assert!(first != second);
    item.write().location = None;
    item.write().node_size = 3;
    assert!(state.node_location(first.node).unwrap().is_none());
    assert_eq!(state.node_size(first.node).unwrap(), 3);
    let spawn = registry
        .construct(&mut ctx, 3, &i, Some(Location::new(3211, 3217, 0)))
        .unwrap();
    let foreign = spawn::Registry::default();
    assert!(
        state
            .ground_target(spawn::as_ground(&spawn), &foreign)
            .is_err()
    );
    let target = state
        .ground_target(spawn::as_ground(&spawn), &registry)
        .unwrap();
    drop((first, second, item, spawn, target));
    state.collect();
    assert_eq!(state.retained_counts(), [0, 0, 0]);
}
#[test]
fn disconnected_dropper_current_approach_cycle_releases_after_external_views_drop() {
    for subtype in [false, true] {
        let (mut state, mut host) = native::create(&json!({}));
        let player = host.player;
        let registry = spawn::Registry::default();
        let mut ctx = Constructor::default();
        let i = input(&ctx);
        let item = if subtype {
            let s = registry
                .construct(&mut ctx, 3, &i, Some(Location::new(3211, 3217, 0)))
                .unwrap();
            spawn::as_ground(&s)
        } else {
            ground::construct(&mut ctx, &i, Some(Location::new(3211, 3217, 0)), 200, None).unwrap()
        };
        item.write().dropper = Some(state.retain_node(player).unwrap());
        let weak = item.downgrade();
        let target = state.ground_target(item.clone(), &registry).unwrap();
        let approach = Approach {
            player,
            target: Some(target.node),
            option: Some(Shared::new(OptionData::new(Some("Take".into()), 2))),
            kind: ApproachKind::Option,
        };
        let movement = state
            .context(&mut host)
            .interaction_motion(approach)
            .unwrap();
        state
            .context(&mut host)
            .run_motion(player, movement)
            .unwrap();
        let exposed = ground::as_item(&item);
        drop((item, target));
        state.context(&mut host).disconnect(player).unwrap();
        state.context(&mut host).pulses().unwrap();
        state.collect();
        assert_eq!(state.retained_counts(), [1, 1, 1]);
        drop(exposed);
        state.collect();
        assert_eq!(state.retained_counts(), [0, 0, 0]);
        assert!(weak.upgrade().is_none());
    }
}
#[test]
fn ground_targets_retain_subtype_after_base_removal_and_reject_foreign_droppers() {
    let mut state = State::default();
    let node = state.add(1, Location::new(3207, 3216, 0), [1, 2, 3, 4]);
    let registry = spawn::Registry::default();
    let mut ctx = Constructor::default();
    let i = input(&ctx);
    let s = registry
        .construct(&mut ctx, 3, &i, Some(Location::new(3211, 3217, 0)))
        .unwrap();
    let weak = s.downgrade();
    let ground = spawn::as_ground(&s);
    let target = state.ground_target(ground.clone(), &registry).unwrap();
    ground.write().removed = true;
    ground.write().decay_time = -1;
    drop((ground, s));
    state.collect();
    assert!(weak.upgrade().is_some());
    assert!(state.node_view(target.node).unwrap().active);
    let view = ground::as_item(&state.ground(target.node).unwrap());
    drop(target);
    state.actor_mut(node).unwrap().connected = false;
    state.collect();
    assert_eq!(state.retained_counts(), [0, 1, 0]);
    drop(view);
    state.collect();
    assert!(weak.upgrade().is_none());
    let mut foreign = State::default();
    let other = foreign.add(1, Location::new(3207, 3216, 0), [1, 2, 3, 4]);
    let item =
        ground::construct(&mut ctx, &i, Some(Location::new(3211, 3217, 0)), 200, None).unwrap();
    item.write().dropper = Some(foreign.retain_node(other).unwrap());
    assert!(state.ground_target(item, &registry).is_err());
}
#[test]
fn weak_ground_and_item_views_upgrade_the_actual_subtype_owner() {
    let registry = spawn::Registry::default();
    let mut ctx = Constructor::default();
    let i = input(&ctx);
    let s = registry
        .construct(&mut ctx, 3, &i, Some(Location::new(3211, 3217, 0)))
        .unwrap();
    let weak_spawn = s.downgrade();
    let g = spawn::as_ground(&s);
    let weak_g = g.downgrade();
    let view = ground::as_item(&g);
    let weak_item = view.downgrade();
    drop((g, view));
    // Weak ownership must refer to the actual owner, surviving retirement of a
    // particular anchor while another reference still owns the same subtype.
    let restored = weak_g.upgrade().unwrap();
    let restored_item = weak_item.upgrade().unwrap();
    drop(s);
    assert!(registry.get(&restored).is_some());
    assert!(weak_spawn.upgrade().is_some());
    let mut state = State::default();
    let target = state.ground_target(restored, &registry).unwrap();
    drop(target);
    state.collect();
    assert_eq!(state.retained_counts(), [0, 1, 0]);
    drop(restored_item);
    state.collect();
    assert_eq!(state.retained_counts(), [0, 0, 0]);
    assert!(weak_spawn.upgrade().is_none());
    assert!(weak_g.upgrade().is_none());
    assert!(weak_item.upgrade().is_none());
}
#[test]
fn cross_actor_dropper_approach_cycles_release_and_exposed_checks_retain_exact_nodes() {
    let (mut state, mut host) = native::create(&json!({}));
    let first = host.player;
    let second = state.add(2, Location::new(3208, 3216, 0), [1, 2, 3, 4]);
    let registry = spawn::Registry::default();
    let mut ctx = Constructor::default();
    let i = input(&ctx);
    let mut weak = Vec::new();
    let mut checks = None;
    for (player, dropper) in [(first, second), (second, first)] {
        let g = ground::construct(
            &mut ctx,
            &i,
            Some(Location::new(3211, 3217, 0)),
            200,
            Some(state.retain_node(dropper).unwrap()),
        )
        .unwrap();
        weak.push(g.downgrade());
        let target = state.ground_target(g, &registry).unwrap();
        let approach = Approach {
            player,
            target: Some(target.node),
            option: Some(Shared::new(OptionData::new(Some("Take".into()), 2))),
            kind: ApproachKind::Option,
        };
        let p = state
            .context(&mut host)
            .interaction_motion(approach)
            .unwrap();
        if checks.is_none() {
            checks = p.base().read().checks.clone();
        }
        state.context(&mut host).run_motion(player, p).unwrap();
    }
    state.context(&mut host).disconnect(first).unwrap();
    state.context(&mut host).disconnect(second).unwrap();
    state.context(&mut host).pulses().unwrap();
    state.collect();
    assert_eq!(state.retained_counts(), [2, 2, 2]);
    assert!(weak.iter().all(|g| g.upgrade().is_some()));
    drop(checks);
    state.collect();
    assert_eq!(state.retained_counts(), [0, 0, 0]);
    assert!(weak.iter().all(|g| g.upgrade().is_none()));
}
