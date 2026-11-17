use super::*;
use rs09_server::{
    auth::account::UserAccountInfo,
    definitions::{
        item::{Registry as Items, Shared},
        scenery::SceneryDefinition,
    },
    world::{landscape, player_render, scenery_options},
};
fn fixture() -> (super::super::Probe, Node, [walk::NodeHandle; 2], Handle) {
    let (mut p, node, _) = super::super::tests::probe();
    p.state.actor_mut(node).unwrap().view.active = true;
    p.state
        .actor(node)
        .unwrap()
        .queue
        .state
        .write()
        .entity
        .update_scene = false;
    p.host.ground = Some(super::super::ground_services::Ground::new(Items::default()));
    let other = p.state.add(2, Location::new(3227, 3216, 0), [1, 2, 3, 4]);
    let nodes = [
        p.state.retain_node(node).unwrap(),
        p.state.retain_node(other).unwrap(),
    ];
    for (i, h) in nodes.iter().enumerate() {
        let mut account = UserAccountInfo::create_default(0);
        account.uid = i as i32 + 1;
        account.username = format!("door{i}");
        let render = if i == 0 {
            p.actors[&1].player.clone()
        } else {
            Shared::new(player_render::Player::new(
                2,
                Location::new(3227, 3216, 0),
                2000,
            ))
        };
        render.write().last_scene = Some(Location::new(3207, 3216, 0));
        super::super::ground_services::admit(&mut p.host, h.node, account.shared(), render);
        p.host.maps.players.insert(h.node);
    }
    let registry = scenery_options::Registry::default();
    registry.register_doors().unwrap();
    p.host.scenery = Some(registry);
    p.host.activate(&Location::new(3207, 3217, 0)).unwrap();
    // This portable archive has no terrain. Author a bounded flat walkable
    // patch before installing the real wall; production uses pinned map bytes.
    p.host.scene.region((50 << 8) | 50).write().has_flags = true;
    for x in 3200..3232 {
        for y in 3200..3232 {
            let c = p.host.scene.chunk(&Location::new(x, y, 0)).unwrap();
            let flags = c.read().flags.clone();
            flags
                .clear(&mut p.host.scene.context(&mut p.host.maps), x & 7, y & 7)
                .unwrap();
            flags.mark_landscape(x & 7, y & 7, true).unwrap();
        }
    }
    for (id, option) in [(12348, "Open"), (10327, "Close")] {
        p.host.scene.definitions.definitions.insert(
            id,
            Shared::new(SceneryDefinition {
                id,
                name: Some("Door".into()),
                interactable: 1,
                options: Some(Shared::new(vec![
                    Some(option.into()),
                    None,
                    None,
                    None,
                    None,
                ])),
                ..Default::default()
            }),
        );
    }
    p.host
        .scene
        .doors
        .apply(
            &rs09_game_core::java_json::parse(
                r#"[{"id":"12348","replaceId":"10327","fence":"false","metal":"false"}]"#,
            )
            .unwrap(),
            &mut |_| Ok(()),
        )
        .unwrap();
    let closed = scenery_node::construct(
        &mut p.host.scene.definitions,
        12348,
        Location::new(3207, 3217, 0),
        0,
        1,
        false,
    )
    .unwrap();
    landscape::add(&mut p.host.scene.context(&mut p.host.maps), &closed, true).unwrap();
    for h in &nodes {
        super::super::ground_services::render(&mut p.state, &mut p.host, h).unwrap();
    }
    let closed = lookup(&mut p, 12348, 3217).unwrap();
    p.host.frames.clear();
    (p, node, nodes, closed)
}
fn lookup(p: &mut super::super::Probe, id: i32, y: i32) -> Option<Handle> {
    p.host
        .scene
        .context(&mut p.host.maps)
        .object(Location::new(3207, y, 0), id, 0)
        .unwrap()
}
fn step(p: &mut super::super::Probe, node: Node) {
    p.state.ticks += 1;
    p.state.context(&mut p.host).pulses().unwrap();
    p.state.context(&mut p.host).entity(node).unwrap();
}
fn flags(p: &mut super::super::Probe) -> [i32; 3] {
    std::array::from_fn(|i| {
        p.host
            .scene
            .context(&mut p.host.maps)
            .clipping(Location::new(3207, 3217 + i as i32, 0), false)
            .unwrap()
    })
}
#[test]
fn actual_click_walk_arrival_replaces_collision_sends_sound_and_fans_out_before_reset() {
    let (mut p, node, viewers, closed) = fixture();
    let original_flags = flags(&mut p);
    assert!(
        process(
            &mut p.state,
            &mut p.host,
            node,
            &ClientAction::SceneryAction {
                opt_index: 0,
                id: 12348,
                x: 3207,
                y: 3217
            }
        )
        .unwrap()
    );
    for _ in 0..8 {
        if lookup(&mut p, 10327, 3218).is_some() {
            break;
        }
        step(&mut p, node);
    }
    let opened = lookup(&mut p, 10327, 3218)
        .expect("accepted original DoorManagingPlugin did not open the scene door");
    assert_eq!(opened.read().rotation, 2);
    assert!(opened.read().constructed);
    assert!(opened.read().replaced.as_ref().unwrap().same(&closed));
    assert_ne!(flags(&mut p), original_flags);
    assert!(p.host.errors.is_empty(), "{:?}", p.host.errors);
    assert!(
        p.host.frames[&node]
            .iter()
            .any(|f| *f == updates::synth_sound(62, 1, 0))
    );
    let mut zones = vec![];
    for h in &viewers {
        super::super::ground_services::render(&mut p.state, &mut p.host, h).unwrap();
        zones.push(
            p.host.frames[&h.node]
                .iter()
                .filter(|f| f.opcode == 230)
                .cloned()
                .collect::<Vec<_>>(),
        );
    }
    assert!(!zones[0].is_empty());
    assert_eq!(zones[0], zones[1]);
    assert!(
        p.host
            .frames
            .get(&viewers[1].node)
            .unwrap()
            .iter()
            .all(|f| f.opcode != 172)
    );
    assert!(!p.host.scene.updates.is_empty());
    p.host.scene.clear_updates();
    assert!(p.host.scene.updates.is_empty());
    p.host.frames.clear();
    process(
        &mut p.state,
        &mut p.host,
        node,
        &ClientAction::SceneryAction {
            opt_index: 0,
            id: 10327,
            x: 3207,
            y: 3218,
        },
    )
    .unwrap();
    for _ in 0..8 {
        if lookup(&mut p, 12348, 3217).is_some() {
            break;
        }
        step(&mut p, node);
    }
    // RegionChunk.add wraps a restored plain static copy in Constructed.
    let restored = lookup(&mut p, 12348, 3217).unwrap();
    assert!(rs09_server::world::scenery::equals(
        &restored,
        Some(&closed)
    ));
    assert!(!restored.same(&closed));
    assert_eq!(flags(&mut p), original_flags);
    assert!(
        p.host.frames[&node]
            .iter()
            .any(|f| *f == updates::synth_sound(60, 1, 0))
    );
    assert!(p.host.errors.is_empty(), "{:?}", p.host.errors);
}
#[test]
fn shared_runner_restores_accepted_open_after_500_pulses_without_a_second_scene_tick() {
    let (mut p, node, _, closed) = fixture();
    let original_flags = flags(&mut p);
    process(
        &mut p.state,
        &mut p.host,
        node,
        &ClientAction::SceneryAction {
            opt_index: 0,
            id: 12348,
            x: 3207,
            y: 3217,
        },
    )
    .unwrap();
    for _ in 0..8 {
        if lookup(&mut p, 10327, 3218).is_some() {
            break;
        }
        step(&mut p, node);
    }
    assert!(lookup(&mut p, 10327, 3218).is_some());
    let pulse = lookup(&mut p, 10327, 3218)
        .unwrap()
        .read()
        .restore_pulse
        .clone()
        .unwrap();
    let before = pulse.read().ticks_passed;
    for _ in before..499 {
        p.state.context(&mut p.host).pulses().unwrap();
    }
    assert!(lookup(&mut p, 10327, 3218).is_some());
    p.state.context(&mut p.host).pulses().unwrap();
    let restored = lookup(&mut p, 12348, 3217).unwrap();
    assert!(rs09_server::world::scenery::equals(
        &restored,
        Some(&closed)
    ));
    assert_eq!(flags(&mut p), original_flags);
}
