#[path = "../../../oracle/player-walk/native.rs"]
mod native;
use serde_json::Value;
use sha2::{Digest, Sha256};
const CORPUS: &str = include_str!("../../../fixtures/player-walk/oracle.jsonl");
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
fn original_requests_managers_runner_entity_ticks_and_walking_joined() {
    for (i, line) in CORPUS.lines().enumerate() {
        let c: Value = serde_json::from_str(line).unwrap();
        if let Some(d) = diff(&native::evaluate(&c), &c["expected"], String::new()) {
            panic!("player walk case{i}:{d}");
        }
    }
}
#[test]
fn player_walk_fixture_sources_and_bytes_are_pinned() {
    let p: Value = serde_json::from_str(include_str!(
        "../../../fixtures/player-walk/provenance.json"
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

#[test]
fn repeated_walk_replacement_reclaims_retired_targets_and_pulses() {
    let (mut state, mut host) = native::create(&serde_json::json!({}));
    for i in 0..512 {
        native::Model::act(
            &mut host,
            &mut state,
            &serde_json::json!({"op":"request","x":3207+(i&1),"y":3216}),
        )
        .unwrap();
        for _ in 0..2 {
            host.act(&mut state, &serde_json::json!({"op":"tick"}))
                .unwrap();
        }
        state.collect();
        assert_eq!(state.retained_counts(), [1, 1, 1], "replacement {i}");
        host.events.clear();
    }
    let node = host.player;
    state.context(&mut host).disconnect(node).unwrap();
    state.context(&mut host).pulses().unwrap();
    state.collect();
    assert_eq!(state.retained_counts(), [0, 0, 0]);
}

#[test]
fn retained_movement_base_checks_queue_manager_and_node_keep_original_owner_until_released() {
    use rs09_server::world::player_walk::{Node, State};
    fn check<T>(retain: impl FnOnce(&mut State, Node) -> T) {
        let (mut state, mut host) = native::create(&serde_json::json!({}));
        host.act(&mut state, &serde_json::json!({"op":"request"}))
            .unwrap();
        let node = host.player;
        let weak = state.current(node).unwrap().unwrap().state.downgrade();
        let root = retain(&mut state, node);
        state.context(&mut host).disconnect(node).unwrap();
        state.context(&mut host).pulses().unwrap();
        state.collect();
        assert!(state.actor(node).is_ok());
        assert!(weak.upgrade().is_some());
        drop(root);
        state.collect();
        assert_eq!(state.retained_counts(), [0, 0, 0]);
        assert!(weak.upgrade().is_none());
    }
    check(|s, n| s.current(n).unwrap().unwrap());
    check(|s, n| s.current(n).unwrap().unwrap().base());
    check(|s, n| {
        let p = s.current(n).unwrap().unwrap().base();
        let checks = p.read().checks.clone().unwrap();
        checks
    });
    check(|s, n| s.actor(n).unwrap().queue.clone());
    check(|s, n| s.actor(n).unwrap().manager.clone());
    check(|s, n| s.retain_node(n).unwrap());
}

#[test]
fn a_reused_wire_index_does_not_rebind_a_retained_old_mover() {
    use rs09_server::world::doors::Location;
    let (mut state, mut host) = native::create(&serde_json::json!({}));
    host.act(&mut state, &serde_json::json!({"op":"request"}))
        .unwrap();
    let old = host.player;
    let pulse = state.current(old).unwrap().unwrap();
    state.context(&mut host).disconnect(old).unwrap();
    state.context(&mut host).pulses().unwrap();
    let new = state.add(1, Location::new(3220, 3221, 0), [1, 2, 3, 4]);
    state.collect();
    assert_ne!(new, old);
    assert_eq!(pulse.mover(), Some(old));
    assert_eq!(
        state
            .actor(old)
            .unwrap()
            .queue
            .entity()
            .location
            .unwrap()
            .coordinates(),
        [3207, 3216, 0]
    );
    assert_eq!(
        state
            .actor(new)
            .unwrap()
            .queue
            .entity()
            .location
            .unwrap()
            .coordinates(),
        [3220, 3221, 0]
    );
    drop(pulse);
    state.collect();
    assert!(state.actor(old).is_err());
}

#[test]
fn anonymous_walk_arrival_retains_its_captured_player_after_base_fields_change() {
    let (mut state, mut host) = native::create(&serde_json::json!({}));
    host.act(
        &mut state,
        &serde_json::json!({"op":"request","running":true}),
    )
    .unwrap();
    let node = host.player;
    let pulse = state.current(node).unwrap().unwrap();
    {
        let mut fields = pulse.state.write();
        fields.mover = None;
        fields.base.write().checks = None;
    }
    state.context(&mut host).disconnect(node).unwrap();
    state.context(&mut host).pulses().unwrap();
    state.collect();
    assert!(state.actor(node).is_ok());
    drop(pulse);
    state.collect();
    assert_eq!(state.retained_counts(), [0, 0, 0]);
}

#[test]
fn npc_children_follow_parent_reachability_and_retained_replaced_arrays() {
    use rs09_server::{definitions::item::Shared, world::player_walk::State};
    let mut state = State::default();
    let parent = state.add_npc_at(1, None, [1, 2, 3, 4]);
    let child = state.add_npc_at(1, None, [1, 2, 3, 4]);
    let replacement = state.add_npc_at(1, None, [1, 2, 3, 4]);
    state.actor_mut(child).unwrap().connected = false;
    state.actor_mut(replacement).unwrap().connected = false;
    let old = Shared::new(vec![Some(child), None, Some(child)]);
    state.set_children(parent, old.clone()).unwrap();
    state
        .set_children(child, Shared::new(vec![Some(parent)]))
        .unwrap();
    state
        .set_children(parent, Shared::new(vec![Some(replacement)]))
        .unwrap();
    state.collect();
    assert!(state.actor(child).is_ok());
    assert!(state.actor(replacement).is_ok());
    // The old array itself is a root, and its child still references the parent.
    state.actor_mut(parent).unwrap().connected = false;
    state.collect();
    assert_eq!(state.retained_counts(), [3, 0, 0]);
    drop(old);
    state.collect();
    assert_eq!(state.retained_counts(), [0, 0, 0]);
}

#[test]
fn an_exposed_child_array_retains_children_without_retaining_its_parent() {
    use rs09_server::{definitions::item::Shared, world::player_walk::State};
    let mut state = State::default();
    let parent = state.add_npc_at(1, None, [1, 2, 3, 4]);
    let child = state.add_npc_at(1, None, [1, 2, 3, 4]);
    state.actor_mut(child).unwrap().connected = false;
    state
        .set_children(parent, Shared::new(vec![Some(child)]))
        .unwrap();
    state.collect();
    assert_eq!(state.retained_counts(), [2, 0, 0]);
    let exposed = state.children(parent).unwrap().unwrap();
    state.actor_mut(parent).unwrap().connected = false;
    state.collect();
    assert!(state.actor(parent).is_err());
    assert!(state.actor(child).is_ok());
    exposed.write().clear();
    state.collect();
    assert_eq!(state.retained_counts(), [0, 0, 0]);
}

#[test]
fn child_cycles_and_shared_arrays_are_internal_edges_unless_exposed() {
    use rs09_server::{definitions::item::Shared, world::player_walk::State};
    let mut state = State::default();
    let a = state.add_npc_at(1, None, [1, 2, 3, 4]);
    let b = state.add_npc_at(2, None, [1, 2, 3, 4]);
    let c = state.add_npc_at(3, None, [1, 2, 3, 4]);
    let children = Shared::new(vec![Some(a), Some(b), Some(c)]);
    state.set_children(a, children.clone()).unwrap();
    state.set_children(b, children.clone()).unwrap();
    state.set_children(c, children).unwrap();
    for node in [a, b, c] {
        state.actor_mut(node).unwrap().connected = false;
    }
    state.collect();
    assert_eq!(state.retained_counts(), [0, 0, 0]);
}

#[test]
fn scenery_arrival_keeps_original_player_target_and_option_after_live_fields_change() {
    use rs09_server::world::movement::Context;
    let (mut state, mut host) = native::create(&serde_json::json!({"scenery":{}}));
    host.act(&mut state, &serde_json::json!({"op":"approach"}))
        .unwrap();
    let node = host.player;
    let target = host.scenery.take().unwrap();
    let target_node = target.node;
    let weak_object = state.scenery(target_node).unwrap().downgrade();
    drop(target);
    let weak_option = host.option.as_ref().unwrap().downgrade();
    host.act(
        &mut state,
        &serde_json::json!({"op":"optionReplace","name":"Search","index":4,"handler":2}),
    )
    .unwrap();
    let mut movement = state.current(node).unwrap().unwrap();
    state.context(&mut host).disconnect(node).unwrap();
    state.context(&mut host).pulses().unwrap();
    {
        let mut fields = movement.state.write();
        fields.mover = None;
        fields.destination = None;
        fields.base.write().checks = None;
    }
    state.collect();
    assert!(state.actor(node).is_ok());
    assert!(state.scenery(target_node).is_ok());
    assert!(weak_option.upgrade().is_some());
    host.events.clear();
    assert!(Context::pulse(&mut state.context(&mut host), &mut movement).unwrap());
    assert_eq!(
        host.events,
        vec![serde_json::json!([
            "interactionArrival",
            "1",
            "object",
            "Open",
            0,
            1
        ])]
    );
    drop(movement);
    state.collect();
    assert_eq!(state.retained_counts(), [0, 0, 0]);
    assert!(weak_option.upgrade().is_none());
    assert!(weak_object.upgrade().is_none());
}

#[test]
fn scenery_identity_is_by_reference_and_survives_duplicate_coordinates() {
    use rs09_server::{definitions::item::Shared, world::doors::Scenery};
    let (mut state, mut host) = native::create(&serde_json::json!({"scenery":{}}));
    let first = host.scenery.take().unwrap();
    let object = state.scenery(first.node).unwrap();
    let again = state.scenery_target(object.clone()).unwrap();
    assert!(first == again);
    let location = object.read().location.clone();
    let mut other = Scenery::new(12348, location, 10, 0, Some("object".into()), false).unwrap();
    other.definition = object.read().definition.clone();
    let other = Shared::new(other);
    let second = state.scenery_target(other).unwrap();
    assert!(first != second);
    assert_ne!(first.node, second.node);
    state.collect();
    assert_eq!(state.retained_counts(), [1, 2, 0]);
    drop((first, again, second, object));
    state.collect();
    assert_eq!(state.retained_counts(), [1, 0, 0]);
    // Reusing the tile after release creates a fresh ID; the index cannot retain
    // a stale address whose allocation the next object happens to reuse.
    let replacement = Scenery::new(
        12348,
        rs09_server::world::doors::Location::new(3211, 3217, 0),
        10,
        0,
        None,
        false,
    )
    .unwrap();
    let third = state.scenery_target(Shared::new(replacement)).unwrap();
    assert!(third.node > 3);
}

#[test]
fn repeated_scenery_approaches_release_retired_targets_options_and_motion_cycles() {
    use rs09_server::{
        definitions::item::Shared,
        world::doors::{Location, Scenery},
    };
    let (mut state, mut host) = native::create(&serde_json::json!({"scenery":{}}));
    let definition = state
        .scenery(host.scenery.as_ref().unwrap().node)
        .unwrap()
        .read()
        .definition
        .clone();
    for i in 0..192 {
        let mut object = Scenery::new(
            12348,
            Location::new(3209 + (i & 1), 3217, 0),
            10,
            0,
            Some("object".into()),
            false,
        )
        .unwrap();
        object.definition = definition.clone();
        host.scenery = Some(state.scenery_target(Shared::new(object)).unwrap());
        host.act(
            &mut state,
            &serde_json::json!({"op":"optionReplace","handler":1}),
        )
        .unwrap();
        host.act(&mut state, &serde_json::json!({"op":"approach"}))
            .unwrap();
        for _ in 0..4 {
            host.act(&mut state, &serde_json::json!({"op":"tick"}))
                .unwrap();
        }
        state.collect();
        assert_eq!(state.retained_counts(), [1, 1, 1], "replacement {i}");
        host.events.clear();
    }
    host.scenery = None;
    let player = host.player;
    state.context(&mut host).disconnect(player).unwrap();
    state.context(&mut host).pulses().unwrap();
    state.collect();
    assert_eq!(state.retained_counts(), [0, 0, 0]);
}

#[test]
fn scheduling_a_movement_from_another_world_cannot_rebind_matching_numeric_ids() {
    let (mut first, mut a) = native::create(&serde_json::json!({"scenery":{}}));
    let (mut second, mut b) = native::create(&serde_json::json!({"scenery":{}}));
    a.act(&mut first, &serde_json::json!({"op":"approach"}))
        .unwrap();
    let movement = first.current(a.player).unwrap().unwrap();
    let node = b.player;
    assert!(second.context(&mut b).run_motion(node, movement).is_err());
    assert!(second.current(node).unwrap().is_none());
    assert!(second.pending().is_empty());
}

#[test]
fn interaction_factory_keeps_evaluated_manager_and_routing_handler() {
    use rs09_server::{
        pulses,
        world::interaction::{Approach, ApproachKind},
    };
    let (mut state, mut host) = native::create(&serde_json::json!({"scenery":{}}));
    let player = host.player;
    let manager = state.actor(player).unwrap().manager.clone();
    let option = host.option.as_ref().unwrap().clone();
    let handler = option.read().handler;
    state.actor_mut(player).unwrap().manager = pulses::Manager::new([1, 2, 3, 4]);
    option.write().handler = Some(2);
    let target = host.scenery.as_ref().map(|h| h.node);
    let movement = state
        .context(&mut host)
        .interaction_motion_captured(
            Approach {
                player,
                target,
                option: Some(option),
                kind: ApproachKind::Option,
            },
            handler,
        )
        .unwrap();
    assert_eq!(movement.option(), Some(1));
    state
        .context(&mut host)
        .run_motion_captured(player, manager.clone(), movement.clone())
        .unwrap();
    assert!(manager.current().unwrap().same(&movement.base()));
    assert!(state.actor(player).unwrap().manager.current().is_none());
}
