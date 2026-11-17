#[path = "../../../oracle/player-appearance/native.rs"]
mod native;
use serde_json::Value;
use sha2::{Digest, Sha256};
const CORPUS: &str = include_str!("../../../fixtures/player-appearance/oracle.jsonl");
#[test]
fn complete_original_appearance_state_aliases_callbacks_and_failed_prefixes_match() {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for (index, line) in CORPUS.lines().enumerate() {
            let row: Value = serde_json::from_str(line).unwrap();
            let actual = native::evaluate(&row);
            if actual != row["expected"] {
                let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../../runs/player-appearance-investigation")
                    .join(format!("mismatch-{index}.json"));
                if !path.exists() {
                    std::fs::write(
                        &path,
                        serde_json::to_vec_pretty(
                            &serde_json::json!({"case":index,"input":row,"actual":actual}),
                        )
                        .unwrap(),
                    )
                    .unwrap();
                }
                tx.send(Err(format!("Appearance case{index}: {}", path.display())))
                    .unwrap();
                return;
            }
        }
        tx.send(Ok(())).unwrap();
    });
    rx.recv_timeout(std::time::Duration::from_secs(60))
        .expect("Appearance callback failed or deadlocked")
        .unwrap();
}
#[test]
fn original_appearance_classes_and_fixtures_are_pinned() {
    let p: Value = serde_json::from_str(include_str!(
        "../../../fixtures/player-appearance/provenance.json"
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
        let path = root.join(name);
        if path.is_file() {
            let data: Vec<_> = std::fs::read(path)
                .unwrap()
                .into_iter()
                .filter(|b| *b != b'\r')
                .collect();
            assert_eq!(
                format!("{:x}", Sha256::digest(data)),
                hash.as_str().unwrap(),
                "{name}"
            );
        }
    }
}

use rs09_server::{
    definitions::{
        item::{ItemDefinition, Registry, Shared},
        property::Property,
    },
    inventory::{
        container::{ContainerType, Factory as Containers, SortType},
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
fn player(state: &mut State, index: i32) -> (NodeHandle, Player) {
    let mut defs = Registry::default();
    defs.insert(
        3,
        Shared::new(ItemDefinition {
            id: 3,
            equip_id: 777,
            ..Default::default()
        }),
    );
    let defs = std::sync::Arc::new(defs);
    let factory = Containers::new(&defs);
    let location = Location::new(3207, 3216, 0);
    let node = state.add(index, location.clone(), [1, 2, 3, 4]);
    let owner = state.retain_node(node).unwrap();
    let core = Capture::new(
        owner.clone(),
        factory
            .create(28, ContainerType::Default, SortType::Id)
            .unwrap(),
        factory
            .create(14, ContainerType::Default, SortType::Id)
            .unwrap(),
        Default::default(),
        factory.null_item,
    );
    let masks = PreparedMasks::new(
        Shared::new(player_render::Player::new(index, location, 2000)),
        Shared::new(appearance::Data::naked("appearance".into(), 0)),
    );
    let fields = Player::new(
        state.identity(),
        core,
        defs,
        Manager::default(),
        Shared::new(false),
        masks,
    )
    .unwrap();
    state.bind_player_fields(fields.clone()).unwrap();
    (owner, fields)
}
#[test]
fn canonical_appearance_prepares_original_equipment_and_actual_render_info_after_retirement() {
    let mut state = State::default();
    let (owner, fields) = player(&mut state, 1);
    let app = fields.appearance();
    let weak = fields.downgrade();
    let info = fields.render_info();
    fields.equipment().items().write()[0] = Some(Shared::new(
        rs09_server::inventory::item::Item::new(3, 1, 1000, &fields.definitions()),
    ));
    let node = owner.node;
    state.actor_mut(node).unwrap().connected = false;
    drop(owner);
    drop(fields);
    state.collect();
    assert!(state.actor(node).is_ok());
    let fields = state.player_fields(&app.player().unwrap()).unwrap();
    fields.prepare_body(&state).unwrap();
    assert_eq!(app.body().read()[0], 777 + 0x8000);
    assert!(info.read().prepared_appearance);
    fields.equipment().items().write()[0] = None;
    fields.prepare_body(&state).unwrap();
    assert_eq!(app.body().read()[0], 777 + 0x8000);
    info.write().update_information(None);
    fields.prepare_body(&state).unwrap();
    assert_eq!(app.body().read()[0], 0);
    drop(fields);
    drop(app);
    state.collect();
    assert!(state.actor(node).is_ok());
    drop(info);
    state.collect();
    assert!(state.actor(node).is_err());
    assert!(weak.upgrade().is_none());
}
#[test]
fn exposed_appearance_arrays_and_parts_do_not_retain_a_player_backreference() {
    let mut state = State::default();
    let (owner, fields) = player(&mut state, 1);
    let app = fields.appearance();
    let parts = app.parts();
    let body = app.body();
    let animations = app.animations();
    let skin = app.skin().unwrap();
    let weak = fields.downgrade();
    let app_weak = app.downgrade();
    let node = owner.node;
    state.actor_mut(node).unwrap().connected = false;
    drop(owner);
    drop(fields);
    drop(app);
    state.collect();
    assert!(state.actor(node).is_err());
    assert!(weak.upgrade().is_none());
    assert!(app_weak.upgrade().is_none());
    skin.write().color = 88;
    assert_eq!(parts.read()[4].as_ref().unwrap().read().color, 88);
    body.write()[13] = 9;
    animations.write()[6] = 123;
}
#[test]
fn gender_prototypes_and_default_attacks_are_shared_per_world_and_isolated_between_worlds() {
    use rs09_server::player::appearance::Gender;
    let mut first = State::default();
    let (_, a) = player(&mut first, 1);
    let factory = a.appearance().factory();
    factory.gender_cache(Gender::Male).read()[0]
        .as_ref()
        .unwrap()
        .write()
        .look = 91;
    factory.gender_cache(Gender::Male).read()[0]
        .as_ref()
        .unwrap()
        .write()
        .color = 88;
    let (_, b) = player(&mut first, 2);
    assert!(factory.same(&b.appearance().factory()));
    assert_eq!(b.appearance().hair().unwrap().read().look, 91);
    assert_eq!(b.appearance().hair().unwrap().read().color, 0);
    let mut second = State::default();
    let (_, c) = player(&mut second, 1);
    assert!(!factory.same(&c.appearance().factory()));
    assert_eq!(c.appearance().hair().unwrap().read().look, 0);
    assert!(a.prepare_body(&second).is_err());
    assert!(!a.masks().owner().read().info.read().prepared_appearance);
}
#[test]
fn configuration_animation_and_runtime_context_share_mutation_priority_and_cached_definition() {
    use rs09_server::{
        definitions::{
            animation::Definition,
            item_config::{self, Parser},
        },
        world::animation::{self, Animation, Priority},
    };
    struct Log;
    impl item_config::Logger for Log {
        fn parsed(&mut self, _n: i32) -> item_config::Result<()> {
            Ok(())
        }
    }
    let mut defs = Registry::default();
    let d = Shared::new(ItemDefinition {
        id: 1,
        ..Default::default()
    });
    defs.insert(1, d.clone());
    Parser::default()
        .load_text(
            r#"[{"id":1,"defence_anim":"100","attack_anims":"422,423"}]"#,
            &defs,
            &mut Log,
        )
        .unwrap();
    let Property::Animation(config) = d.read().handlers["defence_anim"].clone() else {
        panic!()
    };
    let runtime = Animation::from_shared(config.clone());
    assert!(runtime.shared().same(&config));
    struct Definitions {
        calls: usize,
        definition: rs09_server::definitions::animation::Handle,
    }
    impl animation::Definitions for Definitions {
        fn definition(
            &mut self,
            _a: &Animation,
            _id: i32,
        ) -> Option<rs09_server::definitions::animation::Handle> {
            self.calls += 1;
            Some(self.definition.clone())
        }
    }
    let definition = Shared::new(Definition {
        durations: Some(Shared::new(vec![30])),
        ..Default::default()
    });
    let mut source = Definitions {
        calls: 0,
        definition: definition.clone(),
    };
    assert_eq!(runtime.duration(&mut source), 1);
    runtime.set_id(777);
    runtime.set_priority(None);
    assert_eq!(config.read().id, 777);
    assert_eq!(config.read().priority, None);
    assert_eq!(runtime.duration(&mut source), 1);
    assert_eq!(source.calls, 1);
    assert!(runtime.cached_definition().unwrap().same(&definition));
    let Property::Animations(attacks) = d.read().handlers["attack_anims"].clone() else {
        panic!()
    };
    let attack = Animation::from_shared(attacks.read()[0].clone().unwrap());
    assert_eq!(attack.priority(), Some(Priority::High));
    attack.set_id(888);
    assert_eq!(attacks.read()[0].as_ref().unwrap().read().id, 888);
    attacks.write()[1] = None;
    assert!(attacks.read()[1].is_none());
}
