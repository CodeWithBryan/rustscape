#[path = "../../../oracle/world-pulses/native.rs"]
mod native;
use serde_json::Value;
use sha2::{Digest, Sha256};
const CORPUS: &str = include_str!("../../../fixtures/world-pulses/oracle.jsonl");
fn diff(a: &Value, b: &Value, path: String) -> Option<String> {
    if a == b {
        return None;
    }
    if let (Some(a), Some(b)) = (a.as_object(), b.as_object()) {
        for k in a.keys().chain(b.keys()) {
            if let Some(d) = diff(
                a.get(k).unwrap_or(&Value::Null),
                b.get(k).unwrap_or(&Value::Null),
                format!("{path}/{k}"),
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
fn original_actor_and_scenery_pulses_share_one_fixed_tick_prefix() {
    for (i, line) in CORPUS.lines().enumerate() {
        let input: Value = serde_json::from_str(line).unwrap();
        let actual = native::evaluate(&input);
        if let Some(d) = diff(&actual, &input["expected"], String::new()) {
            panic!("world pulse {i}: {d}");
        }
    }
}
#[test]
fn mixed_original_classes_and_expected_data_are_pinned() {
    let p: Value = serde_json::from_str(include_str!(
        "../../../fixtures/world-pulses/provenance.json"
    ))
    .unwrap();
    assert_eq!(p["caseCount"], CORPUS.lines().count());
    assert_eq!(p["fixtureSha256"], format!("{:x}", Sha256::digest(CORPUS)));
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    for (n, h) in p["sources"].as_object().unwrap() {
        let f = root.join(n);
        if f.is_file() {
            let b = std::fs::read(f)
                .unwrap()
                .into_iter()
                .filter(|b| *b != b'\r')
                .collect::<Vec<_>>();
            assert_eq!(
                format!("{:x}", Sha256::digest(b)),
                h.as_str().unwrap(),
                "{n}"
            );
        }
    }
}

#[test]
fn constructor_and_shared_scene_observe_the_same_pending_queue() {
    use rs09_server::{pulses, world::player_walk::State};
    let runner = pulses::Runner::default();
    let state = State::with_runner(runner.clone());
    let p = rs09_server::definitions::item::Shared::new(pulses::Pulse::new(
        1,
        None,
        pulses::Class::Other,
        "probe".into(),
    ));
    runner.submit(Some(p.clone())).unwrap();
    assert!(state.pending()[0].same(&p));
    let (mut state, mut world) = native::create(&serde_json::json!({"scenery":{}}));
    world
        .act(
            &mut state,
            &serde_json::json!({"op":"sceneReplace","ticks":2}),
        )
        .unwrap();
    assert_eq!(state.pending().len(), 1);
    assert!(world.scene.runner.current()[0].same(&state.pending()[0]));
    world.pulses(&mut state).unwrap();
    assert_eq!(state.pending()[0].read().ticks_passed, 1);
}

#[test]
fn scenery_pulses_finish_after_disconnected_actor_cycles_are_collected() {
    use rs09_server::world::doors::Location;
    let (mut state, mut world) = native::create(&serde_json::json!({"scenery":{}}));
    world
        .act(
            &mut state,
            &serde_json::json!({"op":"sceneReplace","ticks":2}),
        )
        .unwrap();
    world.disconnect(&mut state).unwrap();
    state.collect();
    assert_eq!(state.retained_counts(), [0, 0, 0]);
    assert_eq!(state.retained_tasks(), 0);
    assert_eq!(state.pending().len(), 1);
    world.pulses(&mut state).unwrap();
    world.pulses(&mut state).unwrap();
    assert!(state.pending().is_empty());
    assert_eq!(world.scene.retained_task_count(), 0);
    let mut bindings = native::SceneBindings::default();
    let restored = world
        .scene
        .context(&mut bindings)
        .object(Location::new(3250, 3250, 0), -1, 0)
        .unwrap()
        .unwrap();
    assert_eq!(restored.read().id, 12348);
    assert!(world.scene.diagnostics.is_empty());
}

#[test]
fn a_scene_rejects_another_worlds_pulse_without_stopping_it() {
    let (mut state, mut first) = native::create(&serde_json::json!({"scenery":{}}));
    let (_, mut second) = native::create(&serde_json::json!({"scenery":{}}));
    first
        .act(
            &mut state,
            &serde_json::json!({"op":"sceneReplace","ticks":2}),
        )
        .unwrap();
    let p = state.pending().pop().unwrap();
    let mut bindings = native::SceneBindings::default();
    let mut other = second.scene.context(&mut bindings);
    assert!(!other.scene.owns_pulse(&p));
    assert!(other.pulse_body(&p).is_err());
    assert!(other.pulse_inactive(&p).is_err());
    assert!(other.stop_pulse(&p).is_err());
    assert!(p.read().running);
    assert_eq!(p.read().ticks_passed, 0);
}
