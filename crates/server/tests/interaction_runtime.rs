#[path = "../../../oracle/interaction-runtime/native.rs"]
mod native;
use serde_json::Value;
use sha2::{Digest, Sha256};
const CORPUS: &str = include_str!("../../../fixtures/interaction-runtime/oracle.jsonl");
#[test]
fn original_interactions_and_movement_share_the_same_tick_scheduler() {
    for (i, line) in CORPUS.lines().enumerate() {
        let input: Value = serde_json::from_str(line).unwrap();
        assert_eq!(
            native::evaluate(&input),
            input["expected"],
            "interaction runtime {i}: {line}"
        );
    }
}
#[test]
fn joined_original_sources_and_fixture_are_pinned() {
    let p: Value = serde_json::from_str(include_str!(
        "../../../fixtures/interaction-runtime/provenance.json"
    ))
    .unwrap();
    assert_eq!(p["caseCount"], CORPUS.lines().count());
    assert_eq!(p["fixtureSha256"], format!("{:x}", Sha256::digest(CORPUS)));
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    for (name, hash) in p["sources"].as_object().unwrap() {
        let file = root.join(name);
        if file.is_file() {
            let bytes = std::fs::read(file)
                .unwrap()
                .into_iter()
                .filter(|b| *b != b'\r')
                .collect::<Vec<_>>();
            assert_eq!(
                format!("{:x}", Sha256::digest(bytes)),
                hash.as_str().unwrap(),
                "{name}"
            );
        }
    }
}

#[test]
fn plain_task_and_detached_check_array_keep_captured_nodes_then_release_cycles() {
    let (mut state, mut host) = native::create(&serde_json::json!({
        "scenery": {}, "handlerConfig": {"walk":false,"delayed":true}
    }));
    host.act(&mut state, &serde_json::json!({"op":"handle"}))
        .unwrap();
    let player = host.base.player;
    let pulse = state.actor(player).unwrap().manager.current().unwrap();
    let checks = pulse.read().checks.clone().unwrap();
    pulse.write().checks = None;
    host.base.scenery = None;
    host.base.option = None;
    state.context(&mut host).disconnect(player).unwrap();
    state.context(&mut host).pulses().unwrap();
    state.collect();
    assert_eq!(state.retained_counts(), [1, 1, 0]);
    assert_eq!(state.retained_tasks(), 1);
    drop(pulse);
    state.collect();
    assert_eq!(state.retained_counts(), [1, 1, 0]);
    drop(checks);
    state.collect();
    assert_eq!(state.retained_counts(), [0, 0, 0]);
    assert_eq!(state.retained_tasks(), 0);
}

#[test]
fn repeated_delayed_and_unhandled_tasks_do_not_accumulate_after_replacement_or_disconnect() {
    let (mut state, mut host) = native::create(&serde_json::json!({"scenery":{}}));
    for i in 0..192 {
        host.act(
            &mut state,
            &serde_json::json!({"op":"set","handlerConfig":{
                "walk":false,"delayed":i%2==0,"handled":false
            }}),
        )
        .unwrap();
        host.act(&mut state, &serde_json::json!({"op":"handle"}))
            .unwrap();
        state.context(&mut host).pulses().unwrap();
        state.collect();
        assert_eq!(state.retained_counts(), [1, 1, 0], "replacement {i}");
        assert_eq!(state.retained_tasks(), 1, "replacement {i}");
        assert!(state.pending().is_empty());
        host.base.events.clear();
    }
    let player = host.base.player;
    host.base.scenery = None;
    host.base.option = None;
    state.context(&mut host).disconnect(player).unwrap();
    state.collect();
    assert_eq!(state.retained_counts(), [0, 0, 0]);
    assert_eq!(state.retained_tasks(), 0);
}

#[test]
fn foreign_plain_pulses_and_captured_manager_receivers_cannot_rebind_numeric_node_ids() {
    use rs09_server::world::{
        legacy_interaction::{Context as _, Kind, Task},
        legacy_runtime,
    };
    let (mut first, mut a) = native::create(&serde_json::json!({"scenery":{}}));
    let (mut second, mut b) = native::create(&serde_json::json!({"scenery":{}}));
    let owner = a.base.player;
    let mut ctx = legacy_runtime::Context::new(&mut first, &mut a, Some(owner), None).unwrap();
    let manager = ctx.manager(Some(owner)).unwrap().unwrap();
    let task = Task {
        player: Some(b.base.player),
        target: b.base.scenery.as_ref().map(|n| n.node),
        option: b.base.option.clone(),
        kind: Kind::Delayed,
    };
    let plain = second
        .context(&mut b)
        .delayed_interaction(task.clone())
        .unwrap();
    assert!(
        first
            .context(&mut a)
            .run_task_captured(
                owner,
                rs09_server::pulses::Manager::new([1, 2, 3, 4]),
                &plain
            )
            .is_err()
    );
    let mut ctx =
        legacy_runtime::Context::new(&mut second, &mut b, task.player, task.target).unwrap();
    let pulse = ctx.delayed(task).unwrap();
    assert!(ctx.run(manager, pulse).is_err());
    assert!(second.pending().is_empty());
}
