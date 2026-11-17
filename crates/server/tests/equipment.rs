#[path = "../../../oracle/equipment/native.rs"]
mod native;
use serde_json::Value;
use sha2::{Digest, Sha256};
const CORPUS: &str = include_str!("../../../fixtures/equipment/oracle.jsonl");
#[test]
fn complete_original_equipment_control_flow_packets_and_failed_prefixes_match() {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for (i, line) in CORPUS.lines().enumerate() {
            let row: Value = serde_json::from_str(line).unwrap();
            let actual = native::evaluate(&row);
            if actual != row["expected"] {
                tx.send(Err(format!(
                    "equipment original case{i}, actions={}\nnative={actual}\noriginal={}",
                    row["actions"], row["expected"]
                )))
                .unwrap();
                return;
            }
        }
        tx.send(Ok(())).unwrap();
    });
    rx.recv_timeout(std::time::Duration::from_secs(60))
        .expect("equipment callbacks failed or deadlocked")
        .unwrap();
}
#[test]
fn original_equipment_sources_fixture_and_inventory_baseline_are_pinned() {
    let p: Value =
        serde_json::from_str(include_str!("../../../fixtures/equipment/provenance.json")).unwrap();
    assert_eq!(p["caseCount"], CORPUS.lines().count());
    assert_eq!(p["fixtureSha256"], format!("{:x}", Sha256::digest(CORPUS)));
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    assert_eq!(
        p["unchangedInventoryBaseline"],
        format!(
            "{:x}",
            Sha256::digest(std::fs::read(root.join("fixtures/inventory/oracle.jsonl")).unwrap())
        )
    );
    for (name, hash) in p["sources"].as_object().unwrap() {
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

use rs09_server::{
    definitions::item::{Registry, Shared},
    inventory::{
        container::{Container, ContainerType, Factory, SortType},
        equipment,
        player_binding::Capture,
    },
    player::owned::Player,
    world::{
        appearance,
        doors::Location,
        interfaces::Manager,
        player_render,
        player_walk::{NodeHandle, State},
        prepared_masks::PreparedMasks,
    },
};
fn player(state: &mut State) -> (NodeHandle, Player, Container) {
    let defs = std::sync::Arc::new(Registry::default());
    let factory = Factory::new(&defs);
    let location = Location::new(3207, 3216, 0);
    let node = state.add(1, location.clone(), [1, 2, 3, 4]);
    let owner = state.retain_node(node).unwrap();
    let inventory = factory
        .create(28, ContainerType::Default, SortType::Id)
        .unwrap();
    let equipment = equipment::create(&factory, Some(owner.clone())).unwrap();
    let core = Capture::new(
        owner.clone(),
        inventory,
        equipment.clone(),
        Default::default(),
        factory.null_item,
    );
    let masks = PreparedMasks::new(
        Shared::new(player_render::Player::new(1, location, 2000)),
        Shared::new(appearance::Data::naked("equipment".into(), 0)),
    );
    let player = Player::new(
        state.identity(),
        core,
        defs,
        Manager::default(),
        Shared::new(false),
        masks,
    )
    .unwrap();
    state.bind_player_fields(player.clone()).unwrap();
    (owner, player, equipment)
}
fn disconnect(state: &mut State, owner: NodeHandle) -> usize {
    let node = owner.node;
    state.actor_mut(node).unwrap().connected = false;
    drop(owner);
    node
}
#[test]
fn disconnected_equipment_owner_and_listener_cycle_releases_without_external_references() {
    let mut state = State::default();
    let (owner, fields, equipment) = player(&mut state);
    let weak = fields.downgrade();
    let c = equipment.downgrade();
    drop(fields);
    drop(equipment);
    let node = disconnect(&mut state, owner);
    state.collect();
    assert!(state.actor(node).is_err());
    assert!(weak.upgrade().is_none());
    assert!(c.upgrade().is_none());
}
#[test]
fn raw_equipment_keeps_its_actual_player_even_after_all_listeners_are_cleared() {
    let mut state = State::default();
    let (owner, fields, equipment) = player(&mut state);
    let weak = fields.downgrade();
    equipment.listener_list().clear();
    drop(fields);
    let node = disconnect(&mut state, owner);
    state.collect();
    assert!(state.actor(node).is_ok());
    let fields = state
        .player_fields(&equipment.equipment_owner().unwrap().unwrap())
        .unwrap();
    fields.settings().fields.write().weight = 8.5;
    assert_eq!(weak.upgrade().unwrap().settings().fields.read().weight, 8.5);
    drop(fields);
    drop(equipment);
    state.collect();
    assert!(state.actor(node).is_err());
    assert!(weak.upgrade().is_none());
}
#[test]
fn exposed_equipment_list_roots_the_actual_listener_until_cleared() {
    let mut state = State::default();
    let (owner, fields, equipment) = player(&mut state);
    let list = equipment.listener_list();
    let weak = fields.downgrade();
    drop(fields);
    drop(equipment);
    let node = disconnect(&mut state, owner);
    state.collect();
    assert!(state.actor(node).is_ok());
    list.clear();
    state.collect();
    assert!(state.actor(node).is_err());
    assert!(weak.upgrade().is_none());
}
#[test]
fn retained_typed_equipment_listener_survives_list_retirement_and_then_releases() {
    let mut state = State::default();
    let (owner, fields, equipment) = player(&mut state);
    let object = equipment.listener_list().get(0).unwrap().unwrap();
    let listener = object.value::<equipment::Listener>().unwrap();
    let weak = fields.downgrade();
    equipment.listener_list().clear();
    drop(object);
    drop(fields);
    drop(equipment);
    let node = disconnect(&mut state, owner);
    state.collect();
    assert!(state.actor(node).is_ok());
    assert!(state.player_fields(&listener.owner().unwrap()).is_ok());
    drop(listener);
    state.collect();
    assert!(state.actor(node).is_err());
    assert!(weak.upgrade().is_none());
}

#[test]
fn native_plugin_identity_and_typed_receiver_are_independent_of_equal_labels() {
    use rs09_server::definitions::plugin::{Handle, NativePlugin};
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    struct Plugin(AtomicUsize);
    impl NativePlugin for Plugin {
        fn label(&self) -> &str {
            "same label"
        }
    }
    let object = Handle::new(Plugin(AtomicUsize::new(0)));
    let alias = object.clone();
    let other = Handle::new(Plugin(AtomicUsize::new(0)));
    assert!(object.same(&alias));
    assert!(!object.same(&other));
    let receiver = object.value::<Plugin>().unwrap();
    assert!(Arc::ptr_eq(&receiver, &alias.value::<Plugin>().unwrap()));
    receiver.0.store(27, Ordering::Relaxed);
    assert_eq!(
        alias.value::<Plugin>().unwrap().0.load(Ordering::Relaxed),
        27
    );
    let weak = Arc::downgrade(&receiver);
    drop(object);
    drop(alias);
    assert!(weak.upgrade().is_some());
    drop(receiver);
    assert!(weak.upgrade().is_none());
}
